# mini-lsm 模块架构分析

## 1. 项目目录结构

```
mini-lsm/                              # 工作区根目录
├── Cargo.toml                         # 工作区配置 (4 crates)
├── rust-toolchain.toml                # nightly 工具链
├── rustfmt.toml.nightly               # 格式化配置
├── .cargo/config.toml                 # cargo 别名 (x, xtask)
├── .config/nextest.toml               # 测试配置
│
├── mini-lsm/                          # 参考解决方案 (Week 1-2)
│   ├── Cargo.toml                     # 包配置 + 依赖
│   └── src/
│       ├── lib.rs                     # 库入口 (29 行)
│       ├── lsm_storage.rs             # ★ 核心存储引擎 (825 行)
│       ├── key.rs                     # 键类型定义 (183 行)
│       ├── mem_table.rs               # 内存表 (219 行)
│       ├── table.rs                   # SST 表 (264 行)
│       │   ├── builder.rs             # SST 构建器 (129 行)
│       │   ├── iterator.rs            # SST 迭代器 (119 行)
│       │   └── bloom.rs               # 布隆过滤器 (134 行)
│       ├── block.rs                   # 数据块 (57 行)
│       │   ├── builder.rs             # Block 构建器 (108 行)
│       │   └── iterator.rs            # Block 迭代器 (151 行)
│       ├── iterators.rs               # StorageIterator trait (40 行)
│       │   ├── merge_iterator.rs      # 多路合并迭代器 (167 行)
│       │   ├── two_merge_iterator.rs  # 双路合并迭代器 (106 行)
│       │   └── concat_iterator.rs     # 串联迭代器 (137 行)
│       ├── lsm_iterator.rs            # LSM 迭代器 (156 行)
│       ├── compact.rs                 # 压缩控制器 (440 行)
│       │   ├── leveled.rs             # 分层压缩 (247 行)
│       │   ├── simple_leveled.rs      # 简单分层压缩 (149 行)
│       │   └── tiered.rs              # 分层压缩 (155 行)
│       ├── wal.rs                     # 写前日志 (109 行)
│       ├── manifest.rs                # 清单文件 (99 行)
│       ├── mvcc.rs                    # MVCC 入口 (73 行)
│       │   ├── txn.rs                 # 事务管理 (145 行)
│       │   └── watermark.rs           # 水位线 (48 行)
│       ├── debug.rs                   # 调试输出 (37 行)
│       ├── tests.rs                   # 测试模块入口 (28 行)
│       │   └── tests/
│       │       ├── harness.rs         # 测试工具 (462 行)
│       │       ├── week1_day1.rs      # Memtable 测试 (162 行)
│       │       ├── week1_day2.rs      # 合并迭代器测试 (331 行)
│       │       ├── week1_day3.rs      # Block 测试 (161 行)
│       │       ├── week1_day4.rs      # SST 测试 (160 行)
│       │       ├── week1_day5.rs      # 读路径测试 (278 行)
│       │       ├── week1_day6.rs      # 写路径测试 (211 行)
│       │       ├── week1_day7.rs      # SST 优化测试 (105 行)
│       │       ├── week2_day1.rs      # 压缩测试 (265 行)
│       │       ├── week2_day2.rs      # 简单分层压缩测试 (41 行)
│       │       ├── week2_day3.rs      # 分层压缩测试 (43 行)
│       │       ├── week2_day4.rs      # 分层压缩测试 (42 行)
│       │       ├── week2_day5.rs      # Manifest 测试 (168 行)
│       │       └── week2_day6.rs      # WAL 测试 (92 行)
│       └── bin/
│           ├── mini-lsm-cli.rs        # CLI REPL 工具 (358 行)
│           ├── compaction-simulator.rs # 压缩模拟器 (692 行)
│           └── wrapper.rs             # 包装模块 (20 行)
│
├── mini-lsm-mvcc/                     # 参考解决方案 (Week 3 MVCC)
├── mini-lsm-starter/                  # 学生起始代码
├── mini-lsm-book/                     # 课程文档 (mdBook)
└── xtask/                             # 构建自动化
    └── src/main.rs                    # xtask 命令实现 (245 行)
```

---

## 2. 核心模块 vs 辅助模块

### 2.1 核心模块 (5 个)

这些模块构成 LSM 存储引擎的基础，缺一不可：

| 模块 | 行数 | 职责 | 核心原因 |
|------|------|------|----------|
| **lsm_storage.rs** | 825 | 存储引擎核心，编排读写路径、状态管理、线程调度 | 系统入口，依赖所有其他模块 |
| **mem_table.rs** | 219 | 内存表，接收写入，提供内存读取 | 写路径第一站，跳表实现 |
| **table.rs** (+子模块) | 646 | SST 文件编码/解码/遍历 | 磁盘持久化的基本单位 |
| **compact.rs** (+子模块) | 991 | 压缩控制器，三种压缩策略 | 控制读/写放大平衡 |
| **iterators/** | 450 | 迭代器体系，统一数据遍历接口 | 读写路径的遍历基础 |

### 2.2 关键辅助模块 (4 个)

这些模块提供必要的持久化和优化功能：

| 模块 | 行数 | 职责 | 辅助原因 |
|------|------|------|----------|
| **wal.rs** | 109 | 写前日志，崩溃恢复 | 持久性保证，但逻辑简单 |
| **manifest.rs** | 99 | 清单文件，元数据持久化 | 状态恢复，逻辑简单 |
| **block.rs** (+子模块) | 316 | 数据块编码/解码 | SST 的子结构，粒度较小 |
| **key.rs** | 183 | 键类型定义 | 基础类型，无业务逻辑 |

### 2.3 扩展模块 (3 个)

| 模块 | 行数 | 职责 | 定位 |
|------|------|------|------|
| **mvcc/** | 266 | 多版本并发控制 | 第 3 周扩展功能 |
| **debug.rs** | 37 | 调试输出 | 纯辅助功能 |
| **bin/** | 1070 | CLI 工具和模拟器 | 独立可执行文件 |

### 2.4 测试模块

| 模块 | 行数 | 职责 |
|------|------|------|
| **tests/harness.rs** | 462 | 测试工具函数 |
| **tests/week1_*** | 1408 | 第 1 周测试 (7 个文件) |
| **tests/week2_*** | 651 | 第 2 周测试 (6 个文件) |

---

## 3. 模块职责与边界

### 3.1 lsm_storage.rs — 存储引擎核心

**职责**:
- 提供 `MiniLsm` 公共 API (`put`, `get`, `delete`, `scan`, `sync`, `close`)
- 管理 `LsmStorageState` 不可变快照 (Memtable + SSTs 的完整状态)
- 编排读写路径：路由请求到正确的 Memtable/SST
- 触发后台刷盘和压缩任务
- 管理 Flush/Compaction 后台线程生命周期
- 管理 Block Cache

**边界**:
- 向上：提供公共 API 给 CLI 和外部使用者
- 向下：委托给 mem_table、table、compact 等模块执行具体操作
- 不直接操作文件 I/O，通过 table/wal/manifest 模块间接操作

### 3.2 mem_table.rs — 内存表

**职责**:
- 基于 `crossbeam_skiplist::SkipMap` 实现有序内存存储
- 提供 `put`, `get`, `scan` 操作
- 管理 WAL 的写入 (可选)
- 跟踪近似大小 (`approximate_size`) 用于刷盘判断
- 支持从 WAL 恢复

**边界**:
- 输入：接收来自 lsm_storage 的写入请求
- 输出：提供迭代器给 lsm_iterator 合并
- 依赖：wal.rs (持久化), table.rs (刷盘时构建 SST)

### 3.3 table.rs — SST 表 (含 builder/iterator/bloom)

**职责**:
- `SsTable`: SST 文件的抽象，支持打开、读取元数据
- `SsTableBuilder`: 构建 SST 文件 (写入 Block、BlockMeta、Bloom)
- `SsTableIterator`: 遍历 SST 中的键值对
- `Bloom`: 布隆过滤器实现 (FarmHash)
- `BlockMeta`: 块的元数据 (offset, first_key, last_key)
- `FileObject`: 文件 I/O 抽象

**边界**:
- 输入：从 mem_table 接收数据构建 SST
- 输出：提供迭代器给读路径
- 依赖：block.rs (内部结构), key.rs (键比较)
- 循环依赖：引用 lsm_storage 的 `BlockCache` 类型

### 3.4 block.rs — 数据块 (含 builder/iterator)

**职责**:
- `Block`: 最小存储单位，包含数据和偏移量数组
- `BlockBuilder`: 构建数据块，支持大小限制
- `BlockIterator`: 遍历块内的键值对

**边界**:
- 纯数据编码/解码，无外部依赖
- 被 table.rs 完全封装，不直接暴露给上层

### 3.5 iterators/ — 迭代器体系

**职责**:
- `StorageIterator`: 统一迭代器 trait (key, value, is_valid, next)
- `MergeIterator`: 多路合并 (最小堆实现)
- `TwoMergeIterator`: 双路合并 (优化版本)
- `SstConcatIterator`: 非重叠 SST 串联

**边界**:
- 提供统一的遍历抽象
- 不关心数据来源 (Memtable/SST/其他迭代器)
- 被 lsm_storage 和 compact 广泛使用

### 3.6 lsm_iterator.rs — LSM 迭代器

**职责**:
- `LsmIterator`: 顶层迭代器，合并 Memtable 和 SST 迭代器
- `FusedIterator`: 包装迭代器，确保无效后不再访问
- 处理删除标记 (tombstone) 过滤

**边界**:
- 组合 iterators/ 和 mem_table 的迭代器
- 为 lsm_storage 的 `scan()` 提供返回值

### 3.7 compact.rs — 压缩控制器 (含 leveled/simple_leveled/tiered)

**职责**:
- `CompactionController`: 统一压缩控制器 (enum 分发)
- `CompactionTask`: 描述压缩任务的数据结构
- `CompactionOptions`: 压缩策略配置
- 三种策略实现：
  - `LeveledCompactionController`: RocksDB 分层压缩
  - `SimpleLeveledCompactionController`: 经典分层压缩
  - `TieredCompactionController`: RocksDB Universal 压缩
- 后台压缩线程调度 (50ms 轮询)

**边界**:
- 输入：从 lsm_storage 接收状态快照
- 输出：生成 CompactionTask，应用后返回新状态
- 依赖：iterators/ (合并 SST), table.rs (读写 SST), manifest.rs (记录变更)
- 循环依赖：与 lsm_storage 互相引用

### 3.8 wal.rs — 写前日志

**职责**:
- 追加写入键值对 (带 CRC32 校验)
- 从日志恢复数据到 SkipMap

**边界**:
- 仅被 mem_table 使用
- 简单追加格式，无复杂逻辑

### 3.9 manifest.rs — 清单文件

**职责**:
- 记录 LSM 状态变更 (Flush, NewMemtable, Compaction)
- 使用 JSON 序列化 + CRC32 校验
- 从日志恢复历史记录

**边界**:
- 仅被 lsm_storage 和 compact 使用
- 纯日志追加，无随机访问

### 3.10 key.rs — 键类型

**职责**:
- 泛型 `Key<T>` 类型，支持 `KeySlice`, `KeyBytes`, `KeyVec`
- 为 Week 3 预留时间戳编码 (`TS_ENABLED` 常量)

**边界**:
- 零内部依赖，纯基础类型
- 被几乎所有模块使用

### 3.11 mvcc/ — 多版本并发控制

**职责**:
- `LsmMvccInner`: MVCC 状态管理 (时间戳、水位线、已提交事务)
- `Transaction`: 事务工作区 (Week 3 实现)
- `Watermark`: 水位线计算

**边界**:
- 依赖 lsm_storage 获取存储状态
- 为 Week 3 事务 API 提供基础

---

## 4. 模块依赖关系

### 4.1 依赖图 (箭头 A → B 表示 A 依赖 B)

```
                           ┌─────────────┐
                           │   debug.rs  │
                           └──────┬──────┘
                                  │
                           ┌──────▼──────────┐
                    ┌──────│  lsm_storage.rs │──────┐
                    │      │   (核心引擎)     │      │
                    │      └──┬──┬──┬──┬──┬──┘      │
                    │         │  │  │  │  │         │
              ┌─────▼────┐ ┌──▼──▼┐│ ┌▼──▼─┐  ┌────▼────┐
              │mem_table │ │compact││ │table│  │manifest │
              │  .rs     │ │  .rs  ││ │ .rs │  │  .rs    │
              └────┬─────┘ └──┬──┬─┘│ └┬──┬─┘  └────┬────┘
                   │          │  │  │  │  │          │
              ┌────▼─────┐ ┌──▼──▼┐│ ┌▼──▼─┐         │
              │  wal.rs  │ │iterators/││block.rs│     │
              └──────────┘ │  .rs   ││ └──────┘     │
                           └───┬────┘│              │
                               │     │              │
                          ┌────▼─────▼┐             │
                          │  key.rs   │◄────────────┘
                          └───────────┘

                    ┌─────────────────────────────┐
                    │        mvcc/                │
                    │  ┌─────────┐ ┌───────────┐  │
                    │  │ txn.rs  │ │watermark.rs│  │
                    │  └────┬────┘ └───────────┘  │
                    │       │                     │
                    │  ┌────▼────────────┐        │
                    │  │   mvcc.rs       │        │
                    │  └────────┬────────┘        │
                    └───────────┼─────────────────┘
                                │
                          ┌─────▼──────┐
                          │lsm_storage │
                          └────────────┘
```

### 4.2 依赖层次

```
第 0 层 (叶子模块，无内部依赖):
  key.rs, iterators.rs (仅定义 trait), block.rs, mvcc/watermark.rs

第 1 层 (依赖第 0 层):
  wal.rs → key
  block/builder.rs, block/iterator.rs → block
  iterators/merge_iterator.rs → key
  iterators/concat_iterator.rs → key, table

第 2 层 (依赖第 0-1 层):
  mem_table.rs → iterators, key, table, wal
  table.rs → block, key, lsm_storage (BlockCache 类型)
  lsm_iterator.rs → iterators, mem_table, table

第 3 层 (依赖第 0-2 层):
  compact.rs → iterators, key, lsm_storage, manifest, table
  manifest.rs → compact
  mvcc/txn.rs → iterators, lsm_iterator, lsm_storage

第 4 层 (根模块，依赖最多):
  lsm_storage.rs → block, compact, iterators, key, lsm_iterator,
                   manifest, mem_table, mvcc, table
```

### 4.3 循环依赖

项目中存在 **2 个循环依赖**:

1. **lsm_storage ↔ compact**
   - lsm_storage 依赖 compact 的 `CompactionController`
   - compact 依赖 lsm_storage 的 `LsmStorageInner` 和 `LsmStorageState`
   - 原因：压缩需要访问引擎状态，引擎需要控制器来触发压缩

2. **lsm_storage ↔ table**
   - lsm_storage 依赖 table 的 `SsTable`, `SsTableBuilder`
   - table 依赖 lsm_storage 的 `BlockCache` 类型别名
   - 原因：SST 构建器需要缓存引用，缓存类型定义在 lsm_storage 中

### 4.4 模块内聚性分析

| 模块 | 内聚性 | 说明 |
|------|--------|------|
| key.rs | 高 | 单一职责：键类型 |
| block.rs | 高 | 单一职责：块编码 |
| wal.rs | 高 | 单一职责：WAL 写入/恢复 |
| manifest.rs | 高 | 单一职责：元数据日志 |
| iterators/ | 高 | 统一 trait，不同实现 |
| table.rs | 中高 | SST 文件 + Bloom + Builder + Iterator |
| mem_table.rs | 中高 | 跳表 + WAL 集成 |
| compact.rs | 中 | 3 种策略 + 控制器 + 执行逻辑 |
| lsm_storage.rs | 低 | 编排所有模块，职责广泛 |

---

## 5. 代码量统计

### 5.1 按模块统计

| 模块 | 文件数 | 行数 | 占比 |
|------|--------|------|------|
| **lsm_storage.rs** | 1 | 825 | 15.1% |
| **compact/** | 4 | 991 | 18.1% |
| **table/** | 4 | 646 | 11.8% |
| **iterators/** | 4 | 450 | 8.2% |
| **tests/** | 14 | 2521 | 46.1% |
| **bin/** | 3 | 1070 | 19.6% |
| **mem_table.rs** | 1 | 219 | 4.0% |
| **key.rs** | 1 | 183 | 3.3% |
| **lsm_iterator.rs** | 1 | 156 | 2.9% |
| **wal.rs** | 1 | 109 | 2.0% |
| **block/** | 3 | 316 | 5.8% |
| **manifest.rs** | 1 | 99 | 1.8% |
| **mvcc/** | 3 | 266 | 4.9% |
| **debug.rs** | 1 | 37 | 0.7% |
| **lib.rs** | 1 | 29 | 0.5% |
| **总计** | **38** | **5477** | **100%** |

> 注：库代码总计约 5477 行 (不含测试和 bin)，测试代码 2521 行，CLI 工具 1070 行。

### 5.2 按周统计 (核心库代码)

| 周 | 涉及模块 | 行数 | 占比 |
|----|----------|------|------|
| Week 1 | mem_table, key, iterators, block, table, lsm_iterator | 1994 | 36.4% |
| Week 2 | compact, wal, manifest | 1199 | 21.9% |
| Week 3 | mvcc | 266 | 4.9% |
| 核心 | lsm_storage | 825 | 15.1% |
| 辅助 | debug, lib | 66 | 1.2% |

### 5.3 平均文件大小

| 类别 | 平均行数 | 说明 |
|------|----------|------|
| 核心模块 | 274 | 业务逻辑较复杂 |
| 子模块 | 130 | 职责单一 |
| 测试文件 | 180 | 测试用例较多 |
| 总体 | 144 | 合理的文件大小 |

---

## 6. 代码量分布分析

### 6.1 分布特征

```
代码量分布直方图:

0-100 行   ████████████████  12 个文件  (31.6%)
100-200 行 ████████████       9 个文件  (23.7%)
200-300 行 ████████           6 个文件  (15.8%)
300-500 行 ████               3 个文件  (7.9%)
500+ 行   ██████             4 个文件  (10.5%)
                                 (lsm_storage, compact, compaction-sim, harness)
```

### 6.2 开发重点说明

**1. lsm_storage.rs (825 行) — 最大单文件**
- 说明：作为引擎核心，需要编排所有子模块
- 原因：包含打开/关闭、读写路径、状态管理、线程管理、配置等全部逻辑
- 评价：文件偏大，但作为教学项目可以接受

**2. compact/ 模块 (991 行) — 最大模块组**
- 说明：压缩是 LSM 最复杂的子系统
- 三种策略实现各 150-250 行，加上控制器的分发逻辑
- 说明：压缩算法的正确性和性能优化是核心挑战

**3. tests/ 目录 (2521 行) — 占比最大**
- 测试代码与库代码比例约为 1:2
- harness.rs 462 行说明测试基础设施复杂
- 说明：项目高度重视正确性验证

**4. table/ 模块 (646 行)**
- SST 是磁盘持久化的核心
- Builder + Iterator + Bloom 三个子模块职责清晰
- 说明：存储格式的设计是教学重点

**5. mvcc/ 模块 (266 行) — 最小核心模块**
- 仅包含框架代码，Week 3 需要学生填充
- 说明：MVCC 是增量扩展，不改变核心存储结构

### 6.3 代码质量指标

| 指标 | 值 | 评价 |
|------|-----|------|
| 平均文件大小 | 144 行 | 合理 |
| 最大文件 | 825 行 | 偏大 (lsm_storage) |
| 测试覆盖率 | 测试:代码 ≈ 1:2 | 良好 |
| 模块深度 | 最多 3 层 | 扁平，易理解 |
| 子模块数量 | 6 个目录 | 适度 |

---

## 7. 模块拆分与耦合分析

### 7.1 可独立拆分的模块

这些模块具有清晰的边界和低耦合，可以拆分为独立的 crate：

| 模块 | 拆分可行性 | 理由 |
|------|-----------|------|
| **key.rs** | 高 | 零内部依赖，纯类型定义 |
| **block.rs** | 高 | 仅依赖 bytes crate，纯编码逻辑 |
| **iterators/** | 高 | 仅定义 trait 和通用组合器 |
| **wal.rs** | 中高 | 仅依赖 key，简单追加格式 |
| **manifest.rs** | 中 | 依赖 compact 的 CompactionTask 类型 |
| **table/bloom.rs** | 高 | 纯算法，仅依赖 farmhash |

### 7.2 强耦合模块组

这些模块之间存在紧密耦合，不适合拆分：

| 模块组 | 耦合原因 |
|--------|----------|
| **lsm_storage ↔ compact** | 互相引用：引擎需要控制器，控制器需要状态快照 |
| **lsm_storage ↔ table** | 循环依赖：BlockCache 类型定义在 lsm_storage |
| **lsm_storage ↔ mem_table** | 引擎直接管理 Memtable 生命周期 |
| **compact ↔ manifest** | 压缩结果需要记录到 Manifest |
| **mem_table ↔ wal** | Memtable 封装 WAL 的写入 |

### 7.3 建议的拆分方案

如果要将项目拆分为多个 crate，建议如下：

```
mini-lsm-key          # key.rs (基础类型)
mini-lsm-block        # block.rs (块编码)
mini-lsm-iterators    # iterators/ (迭代器 trait + 组合器)
mini-lsm-core         # mem_table + table + wal + manifest
mini-lsm-engine       # lsm_storage + compact + lsm_iterator (主 crate)
mini-lsm-mvcc         # mvcc/ (并发控制)
```

但考虑到这是教学项目，当前单 crate 结构更利于学习，不建议实际拆分。

### 7.4 耦合度矩阵

```
                key  block  iter  table  mem  wal  manifest  compact  lsm  mvcc
key             -     -      -      -     -    -     -         -       -     -
block           ●     -      -      -     -    -     -         -       -     -
iterators       ●     -      -      ○     ○    -     -         ○       ●     -
table           ●     ●      -      -     ○    -     -         -       ○     -
mem_table       ●     -      ●      ●     -    ●     -         -       ●     -
wal             ●     -      -      -     -    -     -         -       -     -
manifest        -     -      -      -     -    -     -         ●       ○     -
compact         ●     -      ●      ●     -    -     ●         -       ●     -
lsm_storage     ●     ●      ●      ●     ●    -     ●         ●       -     ●
mvcc            -     -      -      -     -    -     -         -       ●     -

● = 强依赖 (直接使用类型/函数)
○ = 弱依赖 (仅通过 trait 或间接引用)
```

---

## 8. 架构总结

### 8.1 架构风格

mini-lsm 采用 **分层架构 + 组合模式**:

```
┌─────────────────────────────────────────────┐
│             公共 API 层 (MiniLsm)            │  lsm_storage.rs
├─────────────────────────────────────────────┤
│           业务逻辑层 (读写/压缩)              │  compact, lsm_iterator
├─────────────────────────────────────────────┤
│           数据结构层 (Memtable/SST)           │  mem_table, table, block
├─────────────────────────────────────────────┤
│           持久化层 (WAL/Manifest)             │  wal, manifest
├─────────────────────────────────────────────┤
│           基础类型层 (Key/Iterator)           │  key, iterators
└─────────────────────────────────────────────┘
```

### 8.2 设计亮点

1. **不可变状态快照**: `LsmStorageState` 用 `Arc` 包装，读操作无锁获取快照
2. **迭代器组合**: 通过 trait + 组合器实现灵活的数据遍历
3. **策略模式**: `CompactionController` enum 分发不同压缩策略
4. **Builder 模式**: Block/SST 构建器使用 Builder 模式
5. **教学友好**: 每周代码量递增，从简单到复杂

### 8.3 架构弱点

1. **lsm_storage.rs 过大**: 825 行包含过多职责，可考虑拆分
2. **循环依赖**: lsm_storage ↔ compact 和 lsm_storage ↔ table 的循环依赖
3. **BlockCache 位置**: 类型别名在 lsm_storage 中导致 table 反向依赖
4. **compact.rs 职责混杂**: 控制器 + 执行逻辑 + 线程管理混在一起
