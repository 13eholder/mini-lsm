# Week 3 Day 1-2: MVCC Key Encoding & Snapshot Read

## Overview

这两章实现了 mini-lsm 的 MVCC（多版本并发控制）基础：
- **Day 1**：将 key 编码从 `&[u8]` 升级为 `(user_key, timestamp)`，修改 block/SST 格式以支持 timestamp
- **Day 2**：将 memtable/WAL 改为 MVCC 感知、实现写路径的 timestamp 分配、compaction 保留所有版本、LSM iterator 只返回每个 key 的最新版本

---

## Week 3 Day 1: Timestamp Key Encoding + Refactor

### Key 类型变更

```rust
// 旧: pub struct Key<T: AsRef<[u8]>>(T);
// 新:
pub struct Key<T: AsRef<[u8]>>(T /* user_key */, u64 /* timestamp */);

pub type KeySlice<'a> = Key<&'a [u8]>;
pub type KeyVec = Key<Vec<u8>>;
pub type KeyBytes = Key<Bytes>;
```

关键常量：
| 常量 | 值 | 含义 |
|------|-----|------|
| `TS_DEFAULT` | 0 | 默认时间戳（day 1 使用） |
| `TS_RANGE_BEGIN` | `u64::MAX` | 读/扫的起始时间戳（最大 ts，排最前） |
| `TS_RANGE_END` | `u64::MIN` | 读/扫的结束时间戳（最小 ts，排最后） |

排序规则：`(user_key, Reverse(ts))`，即相同 user_key 中 timestamp 大的排前面：
```
("a", 233) < ("a", 0) < ("b", 233) < ("b", 0)
```

### Task 1: Block 编码

**修改文件**：`src/block/builder.rs`, `src/block/iterator.rs`, `src/block.rs`

**新的 block entry 格式**：

```
旧: | key_overlap_len (u16) | rest_key_len (u16) | key | value_len (u16) | value |
新: | key_overlap_len (u16) | rest_key_len (u16) | key | timestamp (u64) | value_len (u16) | value |
```

**`block/builder.rs`** (`:53-67`)：
- `add()` 中：`key.len()` → `key.key_len()`，`key.raw_ref()` → `key.key_ref()`
- 新增 `self.data.put_u64(key.ts())` 在 key 之后写入 timestamp
- `key_overlap_len()` 使用 `first_key.key_ref()` 和 `key.key_ref()` 计算重叠

**`block/iterator.rs`** (`:120-137`)：
- `seek_to_offset()` 中：读取 key 后调用 `entry.get_u64()` 读取 timestamp
- `KeyVec::from_vec(key)` → `KeyVec::from_vec_with_ts(key, ts)`
- value 偏移量调整为 `offset + SIZEOF_U16*2 + key_rest_len + sizeof::<u64>() + SIZEOF_U16`

**`block.rs`** (`:60-75`)：
- `first_key()` 和 `last_key()` 都改为在读取 key 后继续读取 u64 timestamp
- 使用 `KeyVec::from_vec_with_ts(key, ts)` 构造

### Task 2: SST 编码

**修改文件**：`src/table.rs`, `src/table/builder.rs`, `src/table/iterator.rs`

**`table.rs`** (`:51-95`)：
- `encode_block_meta()`：每个 key 的编码增加 `buf.put_u64(ts)`，使用 `key_ref()`/`key_len()` 替代 `raw_ref()`/`len()`
- `decode_block_meta()`：每读取一个 key 后读取 `buf.get_u64()` 作为 timestamp，用 `from_bytes_with_ts()` 构造 `KeyBytes`

**`table/builder.rs`** (`:30-128`)：
- `first_key`/`last_key` 类型从 `Vec<u8>` 改为 `KeyVec`
- `finish_build()` 中：`block.last_key(first_key.key_ref())` 使用 `key_ref()`
- `add()` 中：bloom filter 的 hash 输入从 `key.raw_ref()` 改为 `key.key_ref()`
- `build()` 中：直接使用 `self.meta` 的 `first_key`/`last_key`（已是 `KeyBytes`），无需从 `Vec<u8>` 重新构造

**`table/iterator.rs`** (`:101`)：
- `is_valid()` 从 `self.blk_idx < self.table.block_meta.len()` 改为 `self.blk_iter.is_valid()`（与参考实现对齐）

### Task 3: LSM Iterator

**修改文件**：`src/lsm_iterator.rs`

- `LsmIterator::KeyType<'a>` = `&'a [u8]`（面向用户，剥离 timestamp）
- `key()` 返回 `self.inner.key().key_ref()`（只返回 user key）
- `is_valid()` 的 end_bound 比较改为直接比较 raw bytes：`self.inner.key().key_ref() <= end.as_ref()`

此时 LsmIterator 不进行版本去重，同一 key 的多个版本都会返回。

### Task 4: Memtable

**修改文件**：`src/mem_table.rs`

- `flush()` 和 `MemTableIterator::key()` 中 `KeySlice::from_slice` 加上 `TS_DEFAULT` 参数
- `MemTableIterator::key()` 返回 `KeySlice::from_slice(self.borrow_item().0.as_ref(), TS_DEFAULT)`

此时 memtable 仍使用 `SkipMap<Bytes, Bytes>`，所有 key 用 `TS_DEFAULT=0`。

### Task 5: Engine Read Path

**修改文件**：`src/lsm_storage.rs`

- `get()` 中 seek 时使用 `KeySlice::from_slice(key, TS_RANGE_BEGIN)`，key 比较使用 `iter.key().key_ref() == key`
- `scan()` 中同样使用 `TS_RANGE_BEGIN` 作为 seek key
- `key_within()` / `range_overlap()` 使用 `key_ref()` 替代 `raw_ref()`

### 测试结果

- Week 3 Day 1 tests: 2/2 passed
- All Week 1 tests: 43/43 passed
- Week 2 tests: 1 expected failure（`test_task1_full_compaction`，因为 memtable 尚未 MVCC 感知）

---

## Week 3 Day 2: Snapshot Read - Memtables and Timestamps

### Task 1: MemTable, WAL, and Read Path

#### WAL 格式变更

**修改文件**：`src/wal.rs`

新的 WAL record 格式：
```
旧: | key_len (u16) | key | value_len (u16) | value | checksum (u32) |
新: | key_len (u16) | key | ts (u64) | value_len (u16) | value | checksum (u32) |
```

**`Wal::put`** (`:85-96`)：
- 签名从 `fn put(&self, key: &[u8], value: &[u8])` 改为 `fn put(&self, key: KeySlice, value: &[u8])`
- 序列化：`key_len(u16) | key_ref() | key.ts()(u64) | value_len(u16) | value | checksum`
- checksum 覆盖 key_len、key、ts、value_len、value 全部字段

**`Wal::recover`** (`:46-77`)：
- SkipMap 类型从 `SkipMap<Bytes, Bytes>` 改为 `SkipMap<KeyBytes, Bytes>`
- 反序列化：读取 key_len → key → ts(u64) → value_len → value → 校验 checksum
- 用 `KeyBytes::from_bytes_with_ts(key, ts)` 插入 skiplist

#### MemTable 重构

**修改文件**：`src/mem_table.rs`

**核心变更**：`SkipMap<Bytes, Bytes>` → `SkipMap<KeyBytes, Bytes>`

**`MemTable::put`** (`:118-127`)：
```rust
pub fn put(&self, key: KeySlice, value: &[u8]) -> Result<()>
```
- 签名从 `&[u8]` 改为 `KeySlice`
- 存储时转换：`key.to_key_vec().into_key_bytes()`
- WAL 写入：`wal.put(key, value)`

**`MemTable::get`** (`:130-138`)：
```rust
pub fn get(&self, key: &[u8]) -> Option<Bytes>
```
- 使用 unsafe transmute 将 `&[u8]` 转为 static 引用构造 `Bytes`
- 查询 key：`KeyBytes::from_bytes_with_ts(Bytes::from_static(unsafe { transmute(key) }), TS_DEFAULT)`
- 安全性：`Bytes::from_static` 不会释放内存，transmute 是 sound 的

**`MemTable::scan`** (`:141-152`)：
```rust
pub fn scan(&self, lower: Bound<KeyBytes>, upper: Bound<KeyBytes>) -> MemTableIterator
```
- 签名从 `Bound<&[u8]>` 改为 `Bound<KeyBytes>`
- SkipMap range 直接使用 `KeyBytes` 范围

**`map_key_bound`** (`:49-55`)：
```rust
pub(crate) fn map_key_bound(bound: Bound<KeySlice>) -> Bound<KeyBytes>
```
- 将 `Bound<KeySlice>` 转换为 `Bound<KeyBytes>`
- `Included(x)` → `Included(x.to_key_vec().into_key_bytes())`

**`MemTable::flush`** (`:156-163`)：
- 直接使用 `entry.key().as_key_slice()`（保留了实际 timestamp）

**`MemTableIterator`** (`:175-230`)：
- item 类型从 `(Bytes, Bytes)` 改为 `(KeyBytes, Bytes)`
- `key()` 返回 `self.borrow_item().0.as_key_slice()`（KeyBytes → KeySlice）
- `is_valid()` 检查 `!self.borrow_item().0.is_empty()`

**`for_testing_scan_slice`** (`:96-117`)：
- 兼容旧测试接口，将 `Bound<&[u8]>` 转换为 `Bound<KeyBytes>`（使用 `TS_DEFAULT`）

#### Engine Read Path 重构

**修改文件**：`src/lsm_storage.rs`

**`open`** (`:388`)：
```rust
mvcc: Some(LsmMvccInner::new(0)),
```
- 初始化 MVCC 子系统，初始 ts=0

**`get`** (`:416-460`)：
- 改为创建 merge iterator 覆盖 memtable + imm_memtables + L0 + levels
- memtable scan 使用 `Included((key, TS_RANGE_BEGIN))..Included((key, TS_RANGE_END))` 范围
- SST 过滤保留 bloom filter 优化
- 返回第一个 `key_ref() == key && !value.is_empty()` 的结果

**`scan`** (`:635-745`)：
- memtable scan bounds：lower → `Included((key, TS_RANGE_BEGIN))`，upper → `Included((key, TS_RANGE_END))`
- 对于 Excluded bound，转为 Included + LsmRangeIterator 处理
- 返回 `FusedIterator<LsmRangeIterator>`

**`LsmRangeIterator`**（`src/lsm_iterator.rs` `:82-120`）：
- 在 `LsmIterator` 基础上增加 lower bound 过滤
- 构造时跳过所有 `< lower` 的 key

### Task 2: Write Path

**修改文件**：`src/lsm_storage.rs` (`write_batch`, `:464-487`)

```rust
pub fn write_batch<T: AsRef<[u8]>>(&self, batch: &[WriteBatchRecord<T>]) -> Result<()> {
    let _write_lock = self.mvcc().write_lock.lock();   // ① 获取写锁
    let ts = self.mvcc().latest_commit_ts() + 1;        // ② 分配 timestamp

    // ③ 写入 memtable（带 timestamp）
    for record in batch {
        match record {
            WriteBatchRecord::Put(k, v) => {
                state.memtable.put(KeySlice::from_slice(k.as_ref(), ts), v.as_ref())?;
            }
            WriteBatchRecord::Del(k) => {
                state.memtable.put(KeySlice::from_slice(k.as_ref(), ts), &[])?;
            }
        }
    }

    self.mvcc().update_commit_ts(ts);                   // ④ 更新 commit_ts
    // ... freeze check ...
}
```

关键设计：
1. **写锁**：确保同一时间只有一个线程在写，保证 timestamp 单调递增且不重复
2. **Timestamp 分配**：`latest_commit_ts() + 1`，一个 batch 中所有 key 共享同一个 ts
3. **Tombstone**：删除操作写入空 value `&[]`
4. **更新 commit_ts**：写完后更新全局 ts

### Task 3: MVCC Compaction

**修改文件**：`src/compact.rs` (`do_compact`, `:131-180`)

**变更要点**：
1. **移除 tombstone 删除逻辑**：不再检查 `compact_to_bottom_level` 来删除 tombstone，所有版本都保留
2. **移除 FusedIterator 包装**：compaction 直接使用原始 merge iterator，保留所有版本（包括 tombstone）
3. **同 key 同 SST**：追踪 `last_key`，当 builder 满时检查当前 key 是否与上一个相同；如果相同则不切分 SST

```rust
fn do_compact<I>(&self, iter: &mut I, _compact_to_bottom_level: bool) -> ... {
    let mut last_key: Vec<u8> = Vec::new();

    while iter.is_valid() {
        builder_inner.add(iter.key(), iter.value());  // 保留所有版本

        let current_key = iter.key().key_ref().to_vec();
        let same_key_as_last = current_key == last_key;
        last_key = current_key;
        iter.next()?;

        // 只在 key 变化时才切分 SST
        if builder_inner.estimated_size() >= self.options.target_sst_size && !same_key_as_last {
            // 切分 SST
        }
    }
}
```

**为什么要同 key 同 SST**：确保同一 user key 的所有版本在同一个 SST 中，这样在 leveled compaction 中，每个 level 内的 SST key range 不重叠，简化后续实现。

### Task 4: LSM Iterator - Latest Version Only

**修改文件**：`src/lsm_iterator.rs` (`LsmIterator`, `:34-78`)

**核心变更**：增加 `prev_key` 字段追踪已返回的 key，跳过旧版本：

```rust
pub struct LsmIterator {
    inner: LsmIteratorInner,
    end_bound: Bound<Bytes>,
    prev_key: Vec<u8>,    // 新增：记录上一个返回的 user key
    is_valid: bool,
}

fn move_to_valid(&mut self) -> Result<()> {
    loop {
        // 检查 end_bound
        // ...
        // 跳过已返回 key 的旧版本
        if !self.prev_key.is_empty()
            && self.inner.key().key_ref() == self.prev_key.as_slice()
        {
            self.inner.next()?;
            continue;
        }
        self.is_valid = true;
        return Ok(());
    }
}

fn next(&mut self) -> Result<()> {
    self.prev_key = self.inner.key().key_ref().to_vec();  // 记录当前 key
    self.inner.next()?;
    self.move_to_valid()?;
    Ok(())
}
```

**`LsmRangeIterator`** (`:82-120`)：
- 在 `LsmIterator` 基础上增加 lower bound 过滤
- 构造时调用 `lsm.next()` 跳过所有 `< lower` 的 key
- 处理 Included/Excluded/Unbounded 三种 lower bound

**`FusedIterator`** 保持不变：自动跳过 tombstone（空 value）。

### 测试结果

```
59 tests run: 59 passed, 0 skipped
```

- Week 1: 43/43 passed
- Week 2: 14/15 passed（`test_task1_full_compaction` 通过）
- Week 3 Day 1: 2/2 passed
- Week 3 Day 2: 1/1 passed

---

## 关键设计决策

### 1. Key 排序与 timestamp 的关系

排序规则 `(user_key, Reverse(ts))` 意味着：
- 相同 user_key，ts 越大越靠前（"最新版本排最前"）
- 这使得 `TS_RANGE_BEGIN = u64::MAX` 成为读操作的自然 seek key（能定位到最新版本）
- 这也是为什么 `KeySlice::from_slice(key, TS_RANGE_BEGIN)` 用于 SST 的 seek 操作

### 2. MemTable scan bounds 的处理

Day 2 中将 memtable 的 `SkipMap` 改为 `KeyBytes` 后，scan 的 bound 处理变得复杂：
- `Included(key)` → `Included((key, TS_RANGE_BEGIN))`（包含该 key 的所有版本）
- `Excluded(key)` 无法用单个 `KeyBytes` bound 精确表达，因此 scan 中统一使用 `Included` bound，由 `LsmRangeIterator` 处理 lower bound 的 Excluded 情况

### 3. Compaction 的同 key 同 SST 策略

MVCC 下同一 user key 有多个版本（不同 ts）。compaction 时如果按 SST 大小切分，可能导致同一 key 的不同版本分散在多个 SST 中，破坏 leveled compaction 的 key range 不重叠假设。因此需要在切分 SST 时检查当前 key 是否与上一个相同，相同则继续写入当前 SST。

### 4. WAL 中 timestamp 的序列化

WAL record 新增 `ts (u64)` 字段，位于 key 之后、value_len 之前。checksum 覆盖包括 ts 在内的所有字段，保证数据完整性。

---

## 涉及文件清单

| 文件 | Day 1 变更 | Day 2 变更 |
|------|-----------|-----------|
| `src/key.rs` | 已为 MVCC 格式（无需改动） | - |
| `src/block/builder.rs` | 编码 timestamp | - |
| `src/block/iterator.rs` | 解码 timestamp | - |
| `src/block.rs` | first_key/last_key 读 timestamp | - |
| `src/table.rs` | block meta 编解码 timestamp | - |
| `src/table/builder.rs` | KeyVec 替换 Vec<u8>，key_ref() | - |
| `src/table/iterator.rs` | is_valid 改用 blk_iter | - |
| `src/lsm_iterator.rs` | KeyType=&[u8]，strip ts | prev_key 去重，LsmRangeIterator |
| `src/mem_table.rs` | KeySlice::from_slice 加 TS_DEFAULT | SkipMap<KeyBytes>，全 API 重构 |
| `src/wal.rs` | - | KeySlice 参数，ts 序列化 |
| `src/lsm_storage.rs` | TS_RANGE_BEGIN seek | get/scan 重构，write_batch MVCC，mvcc 初始化 |
| `src/compact.rs` | - | 保留所有版本，同 key 同 SST |
