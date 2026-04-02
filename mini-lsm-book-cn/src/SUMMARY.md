<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 一周学会 LSM

[前言](./00-preface.md)
[Mini-LSM 概述](./00-overview.md)
[环境设置](./00-get-started.md)

- [第一周概述: Mini-LSM](./week1-overview.md)
  - [内存表](./week1-01-memtable.md)
  - [合并迭代器](./week1-02-merge-iterator.md)
  - [块](./week1-03-block.md)
  - [排序字符串表 (SST)](./week1-04-sst.md)
  - [读取路径](./week1-05-read-path.md)
  - [写入路径](./week1-06-write-path.md)
  - [零食时间: SST 优化](./week1-07-sst-optimizations.md)

- [第二周概述: 合并 + 持久化](./week2-overview.md)
  - [合并实现](./week2-01-compaction.md)
  - [简单合并策略](./week2-02-simple.md)
  - [分层合并策略](./week2-03-tiered.md)
  - [层级合并策略](./week2-04-leveled.md)
  - [清单文件](./week2-05-manifest.md)
  - [预写日志 (WAL)](./week2-06-wal.md)
  - [零食时间: 批量写入和校验和](./week2-07-snacks.md)

- [第三周概述: MVCC](./week3-overview.md)
  - [时间戳编码 + 重构](./week3-01-ts-key-refactor.md)
  - [快照 - 内存表和时间戳](./week3-02-snapshot-read-part-1.md)
  - [快照 - 事务 API](./week3-03-snapshot-read-part-2.md)
  - [水位线和垃圾回收](./week3-04-watermark.md)
  - [事务和乐观并发控制](./week3-05-txn-occ.md)
  - [可序列化快照隔离](./week3-06-serializable.md)
  - [零食时间: 合并过滤器](./week3-07-compaction-filter.md)
- [后续内容 (待定)](./week4-overview.md)

---