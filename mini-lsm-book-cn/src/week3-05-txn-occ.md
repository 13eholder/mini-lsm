<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 事务和乐观并发控制

在本章中，你将实现 `Transaction` 的所有接口。你的实现将为事务内的修改维护一个私有工作空间，并批量提交它们，以便事务的所有修改在提交前只对事务本身可见。我们只在提交时检查冲突（即可序列化冲突），这是乐观并发控制。

要运行测试用例：

```
cargo x copy-test --week 3 --day 5
cargo x scheck
```

## 任务 1：本地工作空间 + Put 和 Delete

在此任务中，你需要修改：

```
src/mvcc/txn.rs
```

你现在可以通过将相应的键/值插入到 `local_storage` 来实现 `put` 和 `delete`，这是一个没有键时间戳的跳表内存表。请注意，对于删除，你仍然需要将其实现为插入空值，而不是从跳表中移除值。

## 任务 2：Get 和 Scan

在此任务中，你需要修改：

```
src/mvcc/txn.rs
```

对于 `get`，你应该首先探测本地存储。如果找到值，则返回值或 `None`，具体取决于它是否是删除标记。对于 `scan`，你将需要为跳表实现一个 `TxnLocalIterator`，就像在第 1.1 章中为没有键时间戳的内存表实现迭代器一样。你将需要在 `TxnIterator` 中存储一个 `TwoMergeIterator<TxnLocalIterator, FusedIterator<LsmIterator>>`。最后，鉴于 `TwoMergeIterator` 将保留子迭代器中的删除标记，你将需要修改 `TxnIterator` 实现以正确处理删除。

## 任务 3：提交

在此任务中，你需要修改：

```
src/mvcc/txn.rs
```

我们假设事务只会在单个线程上使用。一旦你的事务进入提交阶段，你应该将 `self.committed` 设置为 true，以便用户无法在事务上执行任何其他操作。如果你的 `put`、`delete`、`scan` 和 `get` 实现应该出错，如果事务已经提交。

你的提交实现应该简单地从本地存储收集所有键值对，并向存储引擎提交一个写入批次。

## 任务 4：原子 WAL

在此任务中，你需要修改：

```
src/wal.rs
src/mem_table.rs
src/lsm_storage.rs
```

请注意，`commit` 涉及生成一个写入批次，目前写入批次不保证原子性。你将需要更改 WAL 实现以为写入批次生成头部和尾部。

新的 WAL 编码如下：

```
|   HEADER   |                          BODY                                      |  FOOTER  |
|     u32    |   u16   | var | u64 |    u16    |  var  |           ...            |    u32   |
| batch_size | key_len | key | ts  | value_len | value | more key-value pairs ... | checksum |
```

`batch_size` 是 `BODY` 部分的大小。`checksum` 是 `BODY` 部分的校验和。

没有测试用例来验证你的实现。只要你通过所有现有测试用例并实现上述 WAL 格式，一切都应该没问题。

你应该实现 `Wal::put_batch` 和 `MemTable::put_batch`。原始的 `put` 函数应将单个键值对视为一个批次。也就是说，此时，你的 `put` 函数应调用 `put_batch`。

一个批次应在同一个内存表和同一个 WAL 中处理，即使它超过内存表大小限制。

## 测试你的理解

* 根据我们到目前为止实现的所有内容，系统是否满足快照隔离？如果不满足，我们还需要做什么来支持快照隔离？（注意：快照隔离不同于我们将在下一章讨论的可序列化快照隔离）
* 如果用户想要批量导入数据（即 1TB？），如果他们使用事务 API 来执行此操作，你会给他们什么建议？是否有机会优化这种情况？
* 什么是乐观并发控制？如果我们在 Mini-LSM 中实现悲观并发控制，系统会是什么样子？
* 如果你的系统崩溃并在磁盘上留下损坏的 WAL，会发生什么？你如何处理这种情况？
* 当你提交事务时，是否有必要将一切批量放入内存表，或者你可以简单地逐个键放入？为什么？

## 额外任务

* **溢出到磁盘**。如果事务的私有工作空间变得太大，你可以将一些数据刷新到磁盘。

{{#include copyright.md}}