# mini-lsm 核心概念体系

## 1. 关键术语和概念清单

### 1.1 核心存储概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 1 | LSM-Tree | Log-Structured Merge Tree | 架构 |
| 2 | 键值存储 | Key-Value Store | 架构 |
| 3 | 内存表 | Memtable | mem_table |
| 4 | 不可变内存表 | Immutable Memtable | mem_table |
| 5 | 排序字符串表 | Sorted String Table (SST) | table |
| 6 | 数据块 | Block | block |
| 7 | 写前日志 | Write-Ahead Log (WAL) | wal |
| 8 | 清单文件 | Manifest | manifest |
| 9 | 键 | Key / KeySlice | key |
| 10 | 值 | Value / Bytes | - |

### 1.2 读写路径概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 11 | 写路径 | Write Path | lsm_storage |
| 12 | 读路径 | Read Path | lsm_storage |
| 13 | 查找 | Lookup | iterators |
| 14 | 范围扫描 | Scan | iterators |
| 15 | 刷盘 | Flush | lsm_storage |
| 16 | 同步 | Sync | lsm_storage |

### 1.3 迭代器概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 17 | 存储迭代器 | StorageIterator | iterators |
| 18 | 合并迭代器 | MergeIterator | iterators/merge_iterator |
| 19 | 双路合并迭代器 | TwoMergeIterator | iterators/two_merge_iterator |
| 20 | 串联迭代器 | SstConcatIterator | iterators/concat_iterator |
| 21 | 融合迭代器 | FusedIterator | lsm_iterator |
| 22 | LSM 迭代器 | LsmIterator | lsm_iterator |

### 1.4 压缩概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 23 | 压缩 | Compaction | compact |
| 24 | 排序运行 | Sorted Run | compact |
| 25 | 分层压缩 | Leveled Compaction | compact/leveled |
| 26 | 简单分层压缩 | Simple Leveled Compaction | compact/simple_leveled |
| 27 | 分层压缩 (Universal) | Tiered Compaction | compact/tiered |
| 28 | 完全压缩 | Full Compaction | compact |
| 29 | 压缩任务 | CompactionTask | compact |
| 30 | 压缩控制器 | CompactionController | compact |

### 1.5 性能指标概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 31 | 读放大 | Read Amplification | 指标 |
| 32 | 写放大 | Write Amplification | 指标 |
| 33 | 空间放大 | Space Amplification | 指标 |

### 1.6 MVCC 概念 (第 3 周)

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 34 | 多版本并发控制 | MVCC | mvcc |
| 35 | 时间戳 | Timestamp | key |
| 36 | 快照读 | Snapshot Read | mvcc |
| 37 | 事务 | Transaction | mvcc |
| 38 | 乐观并发控制 | OCC | mvcc |
| 39 | 可序列化快照隔离 | SSI | mvcc |
| 40 | 水位线 | Watermark | mvcc |
| 41 | 垃圾回收 | Garbage Collection | compact |
| 42 | 压缩过滤器 | Compaction Filter | compact |
| 43 | 删除标记 | Tombstone | - |
| 44 | 托管模式 | Managed Mode | mvcc |
| 45 | 非托管模式 | Un-managed Mode | mvcc |

### 1.7 SST 优化概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 46 | 布隆过滤器 | Bloom Filter | table/bloom |
| 47 | 块缓存 | Block Cache | lsm_storage |
| 48 | 前缀键编码 | Prefix Key Encoding | block |
| 49 | 校验和 | Checksum | table/wal/manifest |
| 50 | 块元数据 | BlockMeta | table |

### 1.8 系统概念

| 序号 | 概念 | 英文 | 所属模块 |
|------|------|------|----------|
| 51 | 存储引擎状态 | LsmStorageState | lsm_storage |
| 52 | 存储引擎内部 | LsmStorageInner | lsm_storage |
| 53 | 存储选项 | LsmStorageOptions | lsm_storage |
| 54 | 层级 | Level | lsm_storage |
| 55 | L0 层 | Level 0 | lsm_storage |
| 56 | 层 | Tier | compact/tiered |
| 57 | 写入批处理 | Write Batch | lsm_storage |

---

## 2. 概念定义与必要性

### 2.1 核心存储概念

#### LSM-Tree (Log-Structured Merge Tree)
- **定义**: 一种数据结构，通过将随机写入转换为顺序追加来优化写入性能。所有写操作延迟应用到存储，批量合并为 SST 文件写入磁盘。
- **为什么需要**: 相比 B-Tree 的原位更新，LSM-Tree 的追加友好设计使并发控制更简单，支持云原生存储，且可通过调整压缩算法平衡读写性能。

#### 键值存储 (Key-Value Store)
- **定义**: 一种存储模型，数据以键值对形式组织，支持通过键快速查找、插入、删除值。
- **为什么需要**: 提供简单高效的存储抽象，是分布式数据库 (TiDB, CockroachDB) 的底层存储引擎基础。

#### 内存表 (Memtable)
- **定义**: 内存中的有序数据结构 (基于跳表 SkipMap)，用于接收新的写入操作。数据先写入 WAL，再写入 Memtable。
- **为什么需要**: 避免直接写入磁盘的高延迟，将随机写入缓存在内存中，达到阈值后批量刷盘为 SST 文件。

#### 不可变内存表 (Immutable Memtable)
- **定义**: 当 Memtable 达到大小限制时，转换为只读状态，等待刷盘到磁盘。新的写入会创建新的 Memtable。
- **为什么需要**: 允许在后台刷盘的同时继续接收新写入，实现读写并行，提升吞吐量。

#### 排序字符串表 (SST - Sorted String Table)
- **定义**: 磁盘上的有序键值对文件，由多个 Block 组成，包含数据块、元数据块和布隆过滤器。SST 文件一旦写入即不可变。
- **为什么需要**: 提供持久化的有序存储结构，支持高效的范围查询和二分查找。

#### 数据块 (Block)
- **定义**: SST 文件中的最小读写和缓存单位，包含一组排序的键值对及其偏移量数组。
- **为什么需要**: 将大文件分块可以实现按需加载，减少 I/O，提高缓存命中率。

#### 写前日志 (WAL - Write-Ahead Log)
- **定义**: 在数据写入 Memtable 之前，先将操作持久化到日志文件中。每条记录包含键、值和 CRC32 校验和。
- **为什么需要**: 确保系统崩溃后可以恢复未刷盘的数据，提供持久性保证。

#### 清单文件 (Manifest)
- **定义**: 记录 LSM 状态变更的日志文件，包含 Flush、NewMemtable、Compaction 等操作记录，每条记录带 CRC32 校验。
- **为什么需要**: 持久化 LSM 树的元数据状态，使引擎重启后能够重建完整的 LSM 结构。

#### 键 (Key / KeySlice / KeyBytes / KeyVec)
- **定义**: 泛型键类型 `Key<T>`，支持三种形态：`KeySlice` (借用引用)、`KeyBytes` (拥有的 Bytes)、`KeyVec` (拥有的 Vec)。第 3 周加入时间戳编码。
- **为什么需要**: 提供灵活的键表示，避免不必要的内存分配，支持 MVCC 扩展。

---

### 2.2 读写路径概念

#### 写路径 (Write Path)
- **定义**: 数据写入的完整流程：(1) 写入 WAL → (2) 写入 Memtable → (3) 通知用户完成 → (4) 后台刷盘 → (5) 后台压缩。
- **为什么需要**: 确保写入的持久性和性能，通过 WAL 保证崩溃恢复，通过 Memtable 缓冲减少磁盘 I/O。

#### 读路径 (Read Path)
- **定义**: 数据读取的完整流程：(1) 从最新到最旧探测所有 Memtables → (2) 若未找到，搜索整个 LSM 树的 SST 文件。
- **为什么需要**: 保证能读取到最新的值，同时通过布隆过滤器和块元数据优化不存在键的查询。

#### 刷盘 (Flush)
- **定义**: 将不可变 Memtable 中的数据写入磁盘，生成 L0 层的 SST 文件。
- **为什么需要**: 将内存数据持久化，释放内存空间，维持系统的内存使用上限。

#### 同步 (Sync)
- **定义**: 确保所有在 sync 之前的操作都已持久化到磁盘。
- **为什么需要**: 为用户提供持久性保证，确保数据不会因系统崩溃而丢失。

---

### 2.3 迭代器概念

#### 存储迭代器 (StorageIterator)
- **定义**: 统一迭代器 trait，提供 `key()`, `value()`, `is_valid()`, `next()` 接口，关联类型 `KeyType` 支持泛型键。
- **为什么需要**: 为所有数据源 (Memtable, SST, 合并结果) 提供统一的遍历接口，实现代码复用和组合。

#### 合并迭代器 (MergeIterator)
- **定义**: 将多个有序迭代器合并为一个有序迭代器，使用最小堆 (BinaryHeap) 维护当前最小元素。
- **为什么需要**: L0 层的多个 SST 文件 key 范围可能重叠，需要合并迭代器来正确遍历所有数据。

#### 双路合并迭代器 (TwoMergeIterator)
- **定义**: 专门合并两个有序迭代器，比较两者的当前元素并返回较小者。
- **为什么需要**: 优化两路合并的性能，避免通用合并迭代器的堆操作开销。

#### 串联迭代器 (SstConcatIterator)
- **定义**: 将多个 key 范围不重叠的 SST 迭代器串联为一个迭代器。
- **为什么需要**: L1+ 层的 SST 文件 key 范围不重叠，可以直接串联而无需比较，提升性能。

#### LSM 迭代器 (LsmIterator)
- **定义**: 引擎顶层迭代器，合并 Memtable 迭代器和 SST 迭代器，处理删除标记 (tombstone)。
- **为什么需要**: 为用户提供统一的范围扫描接口，隐藏底层多层数据结构的复杂性。

---

### 2.4 压缩概念

#### 压缩 (Compaction)
- **定义**: 后台任务，将多个 SST 文件合并重组，应用更新和删除，减少读放大。
- **为什么需要**: 随着写入增加，L0 层 SST 文件增多导致读放大严重。压缩通过合并文件减少需要探测的文件数量。

#### 排序运行 (Sorted Run)
- **定义**: 一组 key 范围不重叠的 SST 文件集合。分层压缩中每个 Level 是一个 sorted run，分层压缩中每个 Tier 是一个 sorted run。
- **为什么需要**: 抽象压缩的组织单位，不同压缩策略通过控制 sorted run 数量来平衡读放大。

#### 分层压缩 (Leveled Compaction)
- **定义**: 维护固定数量的层级 (Levels)，每层大小呈指数增长。压缩时将上层少量 SST 与下层大量 SST 合并。RocksDB 默认策略。
- **为什么需要**: 保持较低的读放大 (通常 6 层 = 6 次 I/O)，适合读密集型工作负载。

#### 简单分层压缩 (Simple Leveled Compaction)
- **定义**: 经典的分层压缩算法，当某层大小超过比例阈值时触发压缩，将整层与下一层合并。
- **为什么需要**: 实现简单，易于理解，是学习其他压缩策略的基础。

#### 分层压缩 (Tiered Compaction / Universal Compaction)
- **定义**: 动态调整 sorted run 数量，新 SST 可作为新 tier 写入，当 tier 数量过多时合并相邻 tier。RocksDB Universal Compaction。
- **为什么需要**: 最小化写放大，适合写密集型工作负载，如批量数据导入。

#### 压缩任务 (CompactionTask)
- **定义**: 描述一次压缩操作的数据结构，包含上层 SST ID 列表、下层 SST ID 列表、目标层级等信息。
- **为什么需要**: 将压缩决策与压缩执行解耦，便于测试和模拟。

---

### 2.5 性能指标概念

#### 读放大 (Read Amplification)
- **定义**: 一次 get 操作需要向磁盘发送的 I/O 请求数量。等于需要探测的 SST 文件/块数量。
- **为什么需要**: 衡量读取效率的关键指标。读放大过高会导致读取性能下降。

#### 写放大 (Write Amplification)
- **定义**: 实际写入磁盘的数据量与用户写入数据量的比值。计算公式：总写入 SST 数 / 刷盘 SST 数。
- **为什么需要**: 衡量压缩开销的关键指标。写放大过高会消耗更多 I/O 带宽和 SSD 寿命。

#### 空间放大 (Space Amplification)
- **定义**: 实际存储空间与用户数据大小的比值。包括删除标记、多版本数据、压缩临时文件。
- **为什么需要**: 衡量存储效率的关键指标。空间放大过高会浪费存储资源。

#### 压缩权衡三角形
- **定义**: 读放大、写放大、空间放大三者之间存在权衡关系，无法同时最小化。
- **为什么需要**: 帮助理解压缩策略选择的本质——根据工作负载特点选择最优策略。

---

### 2.6 MVCC 概念

#### 多版本并发控制 (MVCC)
- **定义**: 通过为键添加时间戳维护多版本数据，使不同事务可以读取不同时间点的快照。
- **为什么需要**: 支持并发事务的隔离性，使读取不会阻塞写入，写入不会阻塞读取。

#### 时间戳 (Timestamp)
- **定义**: 单调递增的 u64 值，附加到键的末尾。键格式变为 `user_key + timestamp`。时间戳越大表示版本越新。
- **为什么需要**: 区分同一键的不同版本，是 MVCC 的基础。

#### 快照读 (Snapshot Read)
- **定义**: 在给定读时间戳下，读取该时间点之前最新版本的键值对。
- **为什么需要**: 为事务提供一致性的数据视图，不受其他并发写入的影响。

#### 事务 (Transaction)
- **定义**: 一组原子操作，提供隔离性。事务创建时获得快照，所有操作基于该快照执行。
- **为什么需要**: 保证并发操作的正确性，提供 ACID 特性中的隔离性。

#### 乐观并发控制 (OCC)
- **定义**: 事务在私有工作区中执行操作，提交时检查冲突。若冲突则中止事务。
- **为什么需要**: 避免锁的开销，在冲突较少的工作负载下提供更高的并发性能。

#### 可序列化快照隔离 (SSI)
- **定义**: 在快照隔离的基础上，增加序列化检查，确保并发执行的结果等价于某种串行执行顺序。
- **为什么需要**: 提供最强的隔离级别，防止所有并发异常 (脏读、不可重复读、幻读、写偏斜)。

#### 水位线 (Watermark)
- **定义**: 一个时间戳阈值，低于此时间戳的版本不再被任何活跃事务使用，可以被安全删除。
- **为什么需要**: 确定哪些旧版本可以垃圾回收，防止无限积累旧版本导致空间膨胀。

#### 垃圾回收 (Garbage Collection)
- **定义**: 在压缩过程中删除低于水位线的旧版本数据，释放存储空间。
- **为什么需要**: 回收不再使用的旧版本数据，控制空间放大。

#### 删除标记 (Tombstone)
- **定义**: 空值 (empty value) 表示该键已被删除。在压缩过程中才会真正移除该键。
- **为什么需要**: 支持延迟删除，确保并发读取能正确感知删除操作。

---

### 2.7 SST 优化概念

#### 布隆过滤器 (Bloom Filter)
- **定义**: 一种概率数据结构，用于快速判断一个键是否不存在于 SST 中。使用 FarmHash 哈希函数。
- **为什么需要**: 避免对不存在键的不必要磁盘 I/O，显著提升读性能。

#### 块缓存 (Block Cache)
- **定义**: 使用 Moka 实现的 LRU 缓存，缓存频繁访问的 SST 数据块。键为 `(sst_id, block_idx)`。
- **为什么需要**: 减少重复磁盘读取，利用局部性原理提升热数据的访问速度。

#### 前缀键编码 (Prefix Key Encoding)
- **定义**: 利用块内键的有序性，只存储与前一个键的差异部分，减少键的存储开销。
- **为什么需要**: 减少 SST 文件大小，提高磁盘利用率和缓存效率。

#### 校验和 (Checksum)
- **定义**: 使用 CRC32 算法计算数据块的校验值，存储在块末尾。读取时验证数据完整性。
- **为什么需要**: 检测磁盘损坏或传输错误，确保数据可靠性。

---

### 2.8 系统概念

#### 存储引擎状态 (LsmStorageState)
- **定义**: 不可变的 LSM 树快照，包含当前 Memtable、不可变 Memtables 列表、L0 SST 列表、各层 SST 列表、所有 SST 对象映射。
- **为什么需要**: 使用 Arc 包装实现无锁读取，通过原子替换实现状态更新，保证并发安全。

#### 层级 (Level)
- **定义**: 分层压缩中的逻辑层，L0 层 SST 可能重叠，L1+ 层 SST 不重叠。每层大小呈指数增长。
- **为什么需要**: 组织 SST 文件的层次结构，控制读放大和压缩频率。

#### 写入批处理 (Write Batch)
- **定义**: 将多个 Put/Delete 操作打包为一次原子写入操作。
- **为什么需要**: 减少 WAL 写入次数，提升批量写入性能，为 MVCC 事务做准备。

---

## 3. 概念关系图

### 3.1 层级关系

```
LSM-Tree (存储引擎)
├── 内存层
│   ├── Memtable (可写)
│   │   └── WAL (持久化)
│   └── Immutable Memtables (只读，等待刷盘)
├── 磁盘层
│   ├── L0 SSTs (可能重叠)
│   └── L1+ SSTs (不重叠，排序运行)
│       └── SST 文件
│           ├── Block (数据块)
│           ├── BlockMeta (元数据)
│           └── Bloom Filter (布隆过滤器)
└── 元数据
    ├── Manifest (状态日志)
    └── Block Cache (缓存)
```

### 3.2 依赖关系

```
StorageIterator (基础接口)
    ↑
    ├── MemtableIterator
    ├── SsTableIterator
    ├── BlockIterator
    ├── MergeIterator (依赖多个 StorageIterator)
    ├── TwoMergeIterator (依赖两个 StorageIterator)
    ├── SstConcatIterator (依赖多个 SsTableIterator)
    ├── FusedIterator (包装任意 StorageIterator)
    └── LsmIterator (依赖 TwoMergeIterator + MemtableIterator)

CompactionController (压缩控制器)
    ├── SimpleLeveledCompactionController
    ├── LeveledCompactionController
    └── TieredCompactionController
        依赖: StorageIterator, SstConcatIterator, MergeIterator, TwoMergeIterator

LsmStorageInner (核心引擎)
    ├── 依赖: Memtable, SsTable, CompactionController
    ├── 依赖: Manifest, BlockCache
    └── 依赖: 各种 Iterator
```

### 3.3 组合关系

```
LsmIterator = TwoMergeIterator(MemtableIterator, SstIterator)
SstIterator = MergeIterator(SsTableIterator) 或 SstConcatIterator
MergeIterator = BinaryHeap<StorageIterator>
TwoMergeIterator = A + B (比较合并)

Compaction = MergeIterator/SstConcatIterator → compact_generate_sst_from_iter → 新 SST 文件

Write Path = WAL → Memtable → (满时) → Flush → L0 SST → (触发时) → Compaction
Read Path = Memtables (新→旧) → L0 SSTs → L1+ SSTs
```

### 3.4 读写路径数据流

```
写入流程:
用户 Put(k, v)
  → WAL.write(k, v)          [持久化]
  → Memtable.put(k, v)       [内存写入]
  → 返回成功
  → [后台] Memtable 满 → 冻结为 Immutable Memtable
  → [后台] Flush → L0 SST 文件
  → [后台] Compaction 触发 → 合并到下层

读取流程:
用户 Get(k)
  → Memtable.get(k)          [最新数据]
  → Immutable Memtables.get(k) [从新到旧]
  → L0 SSTs.get(k)           [从新到旧，可能重叠]
  → L1+ SSTs.get(k)          [二分查找，不重叠]
  → 返回找到的第一个值或 None
```

---

## 4. 核心概念 vs 辅助概念

### 4.1 核心概念 (必须理解)

这些概念构成了 LSM 存储引擎的基础，缺少任何一个都无法理解系统的工作原理：

1. **LSM-Tree** - 整体架构思想
2. **Memtable** - 内存写入缓冲
3. **SST** - 磁盘持久化结构
4. **Block** - 最小存储单位
5. **WAL** - 崩溃恢复机制
6. **Write Path** - 数据写入流程
7. **Read Path** - 数据读取流程
8. **Compaction** - 数据重组机制
9. **StorageIterator** - 统一遍历接口
10. **LsmStorageState** - 引擎状态快照
11. **Manifest** - 元数据持久化
12. **读放大/写放大/空间放大** - 性能指标

### 4.2 辅助概念 (优化和扩展)

这些概念在核心基础上提供优化或高级功能：

1. **Bloom Filter** - 读路径优化
2. **Block Cache** - 缓存优化
3. **Prefix Key Encoding** - 存储优化
4. **Checksum** - 可靠性增强
5. **Write Batch** - 批量写入优化
6. **MergeIterator** - 多路合并实现
7. **TwoMergeIterator** - 两路合并优化
8. **SstConcatIterator** - 非重叠串联优化

### 4.3 高级概念 (第 3 周 MVCC)

这些概念在核心存储引擎之上添加并发控制：

1. **MVCC** - 多版本控制
2. **Timestamp** - 版本标识
3. **Snapshot Read** - 一致性读取
4. **Transaction** - 事务抽象
5. **OCC** - 乐观并发控制
6. **SSI** - 可序列化隔离
7. **Watermark** - 版本回收阈值
8. **Garbage Collection** - 旧版本清理
9. **Compaction Filter** - 通用压缩过滤

---

## 5. 概念到代码结构的映射

### 5.1 模块与概念映射表

| 代码模块 | 核心概念 | 关键类型/函数 |
|----------|----------|---------------|
| `lsm_storage.rs` | 存储引擎核心、读写路径、状态管理 | `MiniLsm`, `LsmStorageInner`, `LsmStorageState`, `put()`, `get()`, `scan()`, `sync()` |
| `mem_table.rs` | 内存表、不可变内存表 | `MemTable`, `create()`, `put()`, `get()`, `scan()` |
| `table.rs` | SST 文件、块元数据、布隆过滤器 | `SsTable`, `SsTableBuilder`, `SsTableIterator`, `BlockMeta`, `Bloom` |
| `block.rs` | 数据块 | `Block`, `BlockBuilder`, `BlockIterator` |
| `wal.rs` | 写前日志 | `Wal`, `create()`, `recover()`, `put()` |
| `manifest.rs` | 清单文件 | `Manifest`, `ManifestRecord`, `add_record()`, `recover()` |
| `key.rs` | 键类型、时间戳编码 | `Key<T>`, `KeySlice`, `KeyBytes`, `KeyVec`, `TS_ENABLED` |
| `iterators.rs` | 存储迭代器接口 | `StorageIterator` trait |
| `iterators/merge_iterator.rs` | 合并迭代器 | `MergeIterator` |
| `iterators/two_merge_iterator.rs` | 双路合并迭代器 | `TwoMergeIterator` |
| `iterators/concat_iterator.rs` | 串联迭代器 | `SstConcatIterator` |
| `lsm_iterator.rs` | LSM 迭代器、融合迭代器 | `LsmIterator`, `FusedIterator` |
| `compact.rs` | 压缩控制器、压缩任务 | `CompactionController`, `CompactionTask`, `CompactionOptions` |
| `compact/leveled.rs` | 分层压缩 | `LeveledCompactionController`, `LeveledCompactionOptions` |
| `compact/simple_leveled.rs` | 简单分层压缩 | `SimpleLeveledCompactionController` |
| `compact/tiered.rs` | 分层压缩 | `TieredCompactionController` |
| `mvcc.rs` | 多版本并发控制 | `LsmMvccInner`, `CommittedTxnData` |

### 5.2 代码结构层次

```
mini-lsm/src/
│
├── [核心层] 存储引擎入口
│   └── lsm_storage.rs          # MiniLsm, LsmStorageInner, LsmStorageState
│       ├── 读写路径编排
│       ├── 状态管理 (Arc<RwLock>)
│       ├── 刷盘/压缩触发
│       └── 线程管理 (flush/compaction threads)
│
├── [数据层] 存储结构
│   ├── mem_table.rs            # MemTable (SkipMap + WAL)
│   ├── table.rs                # SsTable (文件级)
│   │   ├── builder.rs          # SsTableBuilder
│   │   ├── iterator.rs         # SsTableIterator
│   │   └── bloom.rs            # Bloom Filter
│   └── block.rs                # Block (块级)
│       ├── builder.rs          # BlockBuilder
│       └── iterator.rs         # BlockIterator
│
├── [持久化层] 崩溃恢复
│   ├── wal.rs                  # Write-Ahead Log
│   └── manifest.rs             # Manifest (状态日志)
│
├── [迭代器层] 数据遍历
│   ├── iterators.rs            # StorageIterator trait
│   ├── iterators/
│   │   ├── merge_iterator.rs   # MergeIterator (多路合并)
│   │   ├── two_merge_iterator.rs # TwoMergeIterator (双路合并)
│   │   └── concat_iterator.rs  # SstConcatIterator (串联)
│   └── lsm_iterator.rs         # LsmIterator, FusedIterator
│
├── [压缩层] 数据重组
│   ├── compact.rs              # CompactionController, CompactionTask
│   └── compact/
│       ├── leveled.rs          # Leveled Compaction
│       ├── simple_leveled.rs   # Simple Leveled Compaction
│       └── tiered.rs           # Tiered Compaction
│
├── [MVCC 层] 并发控制 (第 3 周)
│   ├── key.rs                  # 键 + 时间戳编码
│   └── mvcc.rs                 # MVCC 状态管理
│
└── [工具层] 辅助功能
    └── debug.rs                # 调试输出
```

### 5.3 关键数据流映射

#### 写入数据流
```
用户调用 MiniLsm::put()
  → lsm_storage.rs: LsmStorageInner::put()
    → mem_table.rs: MemTable::put()
      → wal.rs: Wal::put()           [WAL 持久化]
      → SkipMap::insert()             [内存写入]
    → lsm_storage.rs: trigger_flush()
      → compact.rs: force_flush_next_imm_memtable()
        → table.rs: SsTable::create() [刷盘为 SST]
        → manifest.rs: Manifest::add_record(Flush)
    → compact.rs: trigger_compaction()
      → compact/leveled.rs: generate_compaction_task()
      → compact.rs: compact()
      → compact/leveled.rs: apply_compaction_result()
```

#### 读取数据流
```
用户调用 MiniLsm::get()
  → lsm_storage.rs: LsmStorageInner::get()
    → mem_table.rs: MemTable::get()   [当前 Memtable]
    → mem_table.rs: MemTable::get()   [不可变 Memtables]
    → table.rs: SsTable::get()        [L0 SSTs]
      → block.rs: Block::decode()     [按需加载块]
      → table/bloom.rs: Bloom::may_contain() [布隆过滤]
    → table.rs: SsTable::get()        [L1+ SSTs]
```

#### 扫描数据流
```
用户调用 MiniLsm::scan()
  → lsm_storage.rs: LsmStorageInner::scan()
    → mem_table.rs: MemTable::scan()  [Memtable 迭代器]
    → iterators/merge_iterator.rs     [合并多个 Memtable 迭代器]
    → iterators/two_merge_iterator.rs [合并 Memtable + SST]
    → iterators/concat_iterator.rs    [L1+ 串联迭代器]
    → lsm_iterator.rs: LsmIterator    [顶层 LSM 迭代器]
```

### 5.4 并发模型映射

```
LsmStorageInner
├── state: Arc<RwLock<Arc<LsmStorageState>>>    # 无锁读取状态快照
├── state_lock: Arc<Mutex<()>>                  # 状态写锁 (序列化写操作)
├── memtable: Arc<MemTable>                     # 共享内存表
├── block_cache: Arc<BlockCache>                # 共享块缓存
├── compaction_thread: Option<JoinHandle>       # 后台压缩线程
├── flush_thread: Option<JoinHandle>            # 后台刷盘线程
└── close_notifier: crossbeam_channel::Sender   # 关闭通知

并发策略:
- 读操作: 获取 state.read() 快照，无锁访问
- 写操作: 获取 state_lock，修改状态后原子替换
- 后台线程: 定期轮询 (50ms)，通过 channel 通知关闭
```

### 5.5 测试结构映射

```
mini-lsm/src/tests/
├── harness.rs                  # 测试工具函数
├── week1_day1.rs               # Memtable 基本测试
├── week1_day2.rs               # 合并迭代器测试
├── week1_day3.rs               # Block 编码测试
├── week1_day4.rs               # SST 编码测试
├── week1_day5.rs               # 读路径测试
├── week1_day6.rs               # 写路径测试
├── week1_day7.rs               # SST 优化测试
├── week2_day1.rs               # 压缩实现测试
├── week2_day2.rs               # 简单分层压缩测试
├── week2_day3.rs               # 分层压缩测试
├── week2_day4.rs               # 分层压缩测试
├── week2_day5.rs               # Manifest 测试
└── week2_day6.rs               # WAL 测试
```

---

## 6. 概念学习路径建议

### 第 1 周：存储格式 + 引擎骨架
```
Day 1:  Memtable → Key → StorageIterator
Day 2:  MergeIterator → TwoMergeIterator
Day 3:  Block → BlockBuilder → BlockIterator
Day 4:  SST → SsTableBuilder → SsTableIterator → Bloom Filter
Day 5:  Read Path → LsmIterator → FusedIterator
Day 6:  Write Path → Flush → LsmStorageState
Day 7:  Block Cache → Prefix Key Encoding
```

### 第 2 周：压缩 + 持久化
```
Day 1:  Compaction → CompactionTask → SstConcatIterator
Day 2:  Simple Leveled Compaction → 读/写/空间放大
Day 3:  Tiered Compaction → 压缩权衡
Day 4:  Leveled Compaction → 动态层级
Day 5:  Manifest → 状态恢复
Day 6:  WAL → 崩溃恢复
Day 7:  Write Batch → Checksum
```

### 第 3 周：MVCC
```
Day 1:  Timestamp → Key 重构
Day 2:  Snapshot Read → 多版本 Memtable
Day 3:  Transaction API → 快照隔离
Day 4:  Watermark → Garbage Collection
Day 5:  OCC → 事务工作区
Day 6:  SSI → 序列化检查
Day 7:  Compaction Filter
```
