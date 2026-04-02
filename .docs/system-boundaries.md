# mini-lsm 系统边界分析

## 1. 项目目标

**项目定位**: 一个用于教学的 LSM-Tree 键值存储引擎实现。

**核心目标**:
- 教授如何用 Rust 构建基于 LSM-Tree 的存储引擎
- 覆盖存储格式、压缩算法、持久化、多版本并发控制等核心数据库概念
- 提供完整的课程体系和测试框架，帮助学生循序渐进地学习

**解决的问题**:
- 数据库系统教学中缺乏从零构建存储引擎的实践课程
- 帮助学生理解 LSM-Tree 的设计权衡（读放大、写放大、空间放大）
- 为分布式数据库（如 TiDB、CockroachDB）的底层存储引擎学习提供基础

**课程结构**:
| 阶段 | 内容 | 章节数 |
|------|------|--------|
| 第 1 周 | 存储格式 + 引擎骨架（Memtable、SST、读写路径） | 7 章 |
| 第 2 周 | 压缩算法 + 持久化（Leveled/Tiered/Simple 压缩、WAL、Manifest） | 7 章 |
| 第 3 周 | 多版本并发控制（MVCC、事务、快照隔离） | 7 章 |
| 额外周 | 优化（进行中） | TBD |

## 2. 平台和运行环境

**支持平台**:
- **操作系统**: Linux (CI 使用 Ubuntu 22.04)，理论上支持所有 Rust 支持的平台
- **架构**: x86_64 (CI 环境)，无特定架构限制

**运行环境要求**:
- **Rust 工具链**: nightly 版本 (见 `rust-toolchain.toml`)
- **最低 Rust 版本**: 1.74
- **Rust Edition**: 2024

**开发工具**:
- `cargo-nextest`: 测试运行器
- `mdbook` + `mdbook-toc`: 文档构建
- `cargo-semver-checks`: 语义版本兼容性检查
- `cargo-clippy`: 代码 lint
- `rustfmt`: 代码格式化 (nightly 配置)

## 3. 对外接口

### 3.1 Rust API (库接口)

核心存储引擎通过 `MiniLsm` 结构体提供以下公共 API:

```rust
// 基本操作
pub fn put(&self, key: &[u8], value: &[u8]) -> Result<()>
pub fn get(&self, key: &[u8]) -> Result<Option<Bytes>>
pub fn delete(&self, key: &[u8]) -> Result<()>

// 范围扫描
pub fn scan(&self, begin: Bound<&[u8]>, end: Bound<&[u8]>) -> Result<LsmIterator>

// 持久化控制
pub fn sync(&self) -> Result<()>
pub fn force_flush(&self) -> Result<()>
pub fn force_full_compaction(&self) -> Result<()>
pub fn close(&self) -> Result<()>

// 调试/诊断
pub fn dump_structure(&self)
```

**配置选项** (`LsmStorageOptions`):
- `block_size`: SST 块大小
- `target_sst_size`: 目标 SST 文件大小
- `num_memtable_limit`: 内存表数量限制
- `compaction_options`: 压缩策略配置
- `enable_wal`: 是否启用预写日志
- `serializable`: 是否启用可序列化隔离

### 3.2 CLI 工具

项目提供两个可执行二进制文件:

**mini-lsm-cli-ref**: 交互式 REPL 命令行工具
```bash
cargo run --bin mini-lsm-cli-ref -- --path lsm.db --compaction leveled --enable-wal
```

支持的命令:
| 命令 | 说明 |
|------|------|
| `put <key> <value>` | 写入键值对 |
| `get <key>` | 查询键值 |
| `del <key>` | 删除键 |
| `scan [begin] [end]` | 范围扫描 |
| `fill <begin> <end>` | 批量填充数据 |
| `dump` | 打印 LSM 结构信息 |
| `flush` | 强制刷盘 |
| `full_compaction` | 强制完全压缩 |
| `close` / `quit` | 关闭存储引擎 |

**compaction-simulator-ref**: 压缩算法模拟器
```bash
cargo run --bin compaction-simulator-ref -- simple --iterations 50
cargo run --bin compaction-simulator-ref -- leveled --max-levels 4
cargo run --bin compaction-simulator-ref -- tiered --num-tiers 3
```

支持的压缩策略:
- `simple`: 简单分层压缩 (Simple Leveled Compaction)
- `leveled`: 分层压缩 (RocksDB Leveled Compaction)
- `tiered`: 分层压缩 (RocksDB Universal Compaction)

### 3.3 xtask 自动化命令

通过 `cargo x` 别名提供开发自动化:

| 命令 | 说明 |
|------|------|
| `cargo x check` | 格式化 + 类型检查 + 测试 + clippy (完整工作区) |
| `cargo x scheck` | 同上，但仅针对 starter 代码 |
| `cargo x ci` | CI 完整检查 (含文档构建) |
| `cargo x copy-test --week N --day N` | 复制测试用例到 starter |
| `cargo x sync` | 检查 starter 与 solution 的语义兼容性 |
| `cargo x book` | 构建并服务课程文档 |
| `cargo x install-tools` | 安装开发工具链 |

### 3.4 迭代器接口

统一的迭代器 trait 用于遍历数据:

```rust
pub trait StorageIterator {
    fn key(&self) -> &[u8];
    fn value(&self) -> &[u8];
    fn is_valid(&self) -> bool;
    fn next(&mut self) -> Result<()>;
}
```

### 3.5 内部模块结构

```
mini-lsm/
├── block.rs        # SST 数据块
├── compact.rs      # 压缩控制器
├── iterators.rs    # 迭代器实现
├── key.rs          # 键类型与编码
├── lsm_iterator.rs # LSM 迭代器
├── lsm_storage.rs  # 核心存储引擎
├── manifest.rs     # 元数据管理
├── mem_table.rs    # 内存表
├── mvcc.rs         # 多版本控制
├── table.rs        # SST 表
└── wal.rs          # 预写日志
```

## 4. 技术栈依赖

### 4.1 核心依赖

| 类别 | 依赖 | 用途 |
|------|------|------|
| **语言** | Rust (nightly 2024 edition) | 主要编程语言 |
| **错误处理** | `anyhow` | 错误处理和上下文 |
| **字节操作** | `bytes` | 高效的字节缓冲区管理 |
| **并发原语** | `parking_lot` | 高性能 Mutex 和 RwLock |
| **原子操作** | `arc-swap` | 原子 Arc 交换 |
| **无锁数据结构** | `crossbeam-skiplist` | 跳表实现 (Memtable) |
| **无锁数据结构** | `crossbeam-epoch` | 内存回收 |
| **无锁数据结构** | `crossbeam-channel` | 无锁通道 |
| **缓存** | `moka` | 块缓存 (Block Cache) |
| **自引用结构** | `ouroboros` | 自引用结构体处理 |
| **序列化** | `serde` + `serde_json` | 数据序列化 |
| **CLI 解析** | `clap` | 命令行参数解析 |
| **REPL** | `rustyline` | 交互式命令行编辑 |
| **解析器** | `nom` | 组合子解析器 (CLI 命令解析) |
| **随机数** | `rand` | 随机数生成 |
| **哈希** | `farmhash` | 布隆过滤器哈希 |
| **校验和** | `crc32fast` | CRC32 校验和计算 |

### 4.2 开发依赖

| 依赖 | 用途 |
|------|------|
| `tempfile` | 测试临时目录 |

### 4.3 工具链依赖

| 工具 | 用途 |
|------|------|
| `cargo-nextest` | 并行测试运行器 |
| `mdbook` | 文档生成 |
| `mdbook-toc` | 文档目录生成 |
| `cargo-semver-checks` | 语义版本兼容性检查 |

### 4.4 无外部服务依赖

本项目是**纯本地存储引擎**，不依赖:
- 无外部数据库
- 无网络服务
- 无云服务
- 所有数据存储在本地文件系统

## 5. 安装和使用

### 5.1 前置条件

- **Rust 工具链**: 通过 [rustup](https://rustup.rs) 安装
- **Git**: 克隆仓库
- **磁盘空间**: 编译需要约 2-3 GB
- **内存**: 建议 4GB+ (编译期间)

### 5.2 安装步骤

```bash
# 1. 克隆仓库
git clone https://github.com/skyzh/mini-lsm
cd mini-lsm

# 2. 安装开发工具
cargo x install-tools

# 3. 验证环境
cargo x check
```

### 5.3 使用方式

**作为学生 (实现练习)**:

```bash
# 进入 starter 目录
cd mini-lsm-starter

# 复制测试用例 (例如第 1 周第 1 天)
cargo x copy-test --week 1 --day 1

# 运行检查 (格式化 + 类型检查 + 测试 + clippy)
cargo x scheck

# 运行 CLI 交互工具
cargo run --bin mini-lsm-cli

# 运行压缩模拟器
cargo run --bin compaction-simulator
```

**作为课程开发者**:

```bash
# 运行完整检查
cargo x check

# 运行 CI 完整流程
cargo x ci

# 构建课程文档
cargo x book

# 同步 starter 和 solution
cargo x sync
```

**直接运行参考实现**:

```bash
# 交互式 CLI
cargo run --bin mini-lsm-cli-ref -- --path mydb --compaction leveled

# 压缩模拟
cargo run --bin compaction-simulator-ref -- leveled --iterations 100
```

### 5.4 测试运行

```bash
# 运行所有测试
cargo nextest run

# 运行特定测试模块
cargo nextest run week1_day1

# 运行单个测试
cargo nextest run week1_day1::test_basic_get_put

# 仅运行 starter 测试
cargo x scheck
```

### 5.5 项目结构

```
mini-lsm/                    # 工作区根目录
├── mini-lsm/                # 参考解决方案 (第 1-2 周)
├── mini-lsm-mvcc/           # 参考解决方案 (第 3 周 MVCC)
├── mini-lsm-starter/        # 学生起始代码
├── mini-lsm-book/           # 课程文档 (mdBook)
├── xtask/                   # 构建自动化
├── .docs/                   # 项目分析文档
├── Cargo.toml               # 工作区配置
└── rust-toolchain.toml      # Rust 工具链版本
```

## 6. 系统架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                     用户接口层                                │
│  ┌──────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │  Rust API    │  │  CLI (REPL)      │  │  Compaction    │ │
│  │  (MiniLsm)   │  │  mini-lsm-cli    │  │  Simulator     │ │
│  └──────┬───────┘  └────────┬─────────┘  └───────┬────────┘ │
└─────────┼──────────────────┼─────────────────────┼──────────┘
          │                  │                     │
┌─────────▼──────────────────▼─────────────────────▼──────────┐
│                     存储引擎核心                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  LsmStorage (lsm_storage.rs)                            │ │
│  │  - Put / Get / Delete / Scan                            │ │
│  │  - 读写路径管理                                          │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌────────────┐  ┌──────────────┐  ┌─────────────────────┐  │
│  │  MemTable  │  │  SsTable     │  │  Compaction         │  │
│  │  (内存表)   │  │  (磁盘表)     │  │  Controller         │  │
│  │            │  │              │  │  - Simple/Leveled/  │  │
│  │  SkipMap   │  │  Block       │  │    Tiered           │  │
│  │  WAL       │  │  Bloom       │  │                     │  │
│  └────────────┘  └──────────────┘  └─────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  MVCC (第 3 周)                                          │ │
│  │  - 时间戳编码 / 快照读 / 事务 / OCC / SSI               │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
          │
┌─────────▼──────────────────────────────────────────────────┐
│                     持久化层                                 │
│  ┌────────────┐  ┌──────────────┐  ┌─────────────────────┐ │
│  │  SST 文件   │  │  WAL 日志     │  │  Manifest           │ │
│  │  (*.sst)   │  │  (*.wal)     │  │  (元数据 JSON)       │ │
│  └────────────┘  └──────────────┘  └─────────────────────┘ │
│                     本地文件系统                              │
└─────────────────────────────────────────────────────────────┘
```

## 7. 关键设计特性

### 7.1 存储结构
- **LSM-Tree 架构**: 内存表 (Memtable) + 排序字符串表 (SST) 的多层结构
- **分层压缩**: 支持 Simple、Leveled、Tiered 三种压缩策略
- **布隆过滤器**: 优化不存在键的查询性能
- **块缓存**: 使用 Moka 实现高性能块缓存

### 7.2 并发模型
- **线程安全**: 使用 `Arc` + `parking_lot` 实现跨线程共享
- **无锁组件**: 使用 crossbeam 跳表实现无锁内存表
- **MVCC**: 支持多版本并发控制和快照隔离

### 7.3 持久化保证
- **WAL**: 预写日志确保崩溃恢复
- **Manifest**: 元数据持久化跟踪 SST 文件状态
- **校验和**: CRC32 校验和确保数据完整性

### 7.4 课程特性
- **渐进式学习**: 每周 7 章，每章 2-3 小时
- **完整测试**: 每章提供测试用例验证实现
- **参考答案**: 提供完整参考解决方案
- **CLI 工具**: 交互式工具验证引擎行为
