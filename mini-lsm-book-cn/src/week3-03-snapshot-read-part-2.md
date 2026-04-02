<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 快照读取 - 引擎读取路径和事务 API

在本章中，你将：

* 基于前一章完成读取路径以支持快照读取。
* 实现事务 API 以支持快照读取。
* 实现引擎恢复过程以正确恢复提交时间戳。

在本天结束时，你的引擎将能够为用户提供存储键空间的一致视图。

在重构期间，你可能需要根据需要将某些函数的签名从 `&self` 更改为 `self: &Arc<Self>`。

要运行测试用例：

```
cargo x copy-test --week 3 --day 3
cargo x scheck
```

**注意：完成本章后，你还需要通过 2.5 和 2.6 的测试用例。**

## 任务 1：带有读取时间戳的 LSM 迭代器

本章的目标是拥有类似这样的东西：

```rust,no_run
let snapshot1 = engine.new_txn();
// 向引擎写入一些东西
let snapshot2 = engine.new_txn();
// 向引擎写入一些东西
snapshot1.get(/* ... */); // 我们可以检索引擎先前状态的一致快照
```

为了实现这一点，我们可以在创建事务时记录读取时间戳（这是最新的提交时间戳）。当我们在事务上执行读取操作时，我们将只读取低于或等于读取时间戳的所有键版本。

在此任务中，你需要修改：

```
src/lsm_iterator.rs
```

为此，你需要在 `LsmIterator` 中记录一个读取时间戳。

```rust,no_run
impl LsmIterator {
    pub(crate) fn new(
        iter: LsmIteratorInner,
        end_bound: Bound<Bytes>,
        read_ts: u64,
    ) -> Result<Self> {
        // ...
    }
}
```

你需要更改 LSM 迭代器的 `next` 逻辑以找到正确的键。

## 任务 2：多版本扫描和获取

在此任务中，你需要修改：

```
src/mvcc.rs
src/mvcc/txn.rs
src/lsm_storage.rs
```

现在我们在 LSM 迭代器中有了 `read_ts`，我们可以在事务结构上实现 `scan` 和 `get`，以便我们可以在存储引擎的给定点读取数据。

我们建议你在 `LsmStorageInner` 结构中创建辅助函数，如 `scan_with_ts(/* 原始参数 */, read_ts: u64)` 和 `get_with_ts`（如果需要）。存储引擎上的原始 get/scan 应实现为创建事务（快照）并在该事务上执行 get/scan。调用路径将类似于：

```
LsmStorageInner::scan -> new_txn 和 Transaction::scan -> LsmStorageInner::scan_with_ts
```

要在 `LsmStorageInner::scan` 中创建事务，我们需要向事务构造函数提供 `Arc<LsmStorageInner>`。因此，我们可以将 `scan` 的签名更改为接受 `self: &Arc<Self>` 而不是简单的 `&self`，这样我们就可以使用 `let txn = self.mvcc().new_txn(self.clone(), /* ... */)` 创建事务。

你还需要更改 `scan` 函数以返回 `TxnIterator`。我们必须确保快照在用户迭代引擎时是活动的，因此 `TxnIterator` 存储快照对象。在 `TxnIterator` 内部，我们现在可以存储一个 `FusedIterator<LsmIterator>`。当我们实现 OCC 时，我们稍后会将其更改为其他东西。

你现在不需要实现 `Transaction::put/delete`，所有修改仍将通过引擎进行。

## 任务 3：在 SST 中存储最大时间戳

在此任务中，你需要修改：

```
src/table.rs
src/table/builder.rs
```

在你的 SST 编码中，你应该在块元数据之后存储最大时间戳，并在加载 SST 时恢复它。这将帮助系统在恢复系统时决定最新的提交时间戳。

## 任务 4：恢复提交时间戳

现在我们在 SST 中有了最大时间戳信息，在 WAL 中有了时间戳信息，我们可以获取引擎启动之前提交的最大时间戳，并在创建 `mvcc` 对象时使用该时间戳作为最新的提交时间戳。

如果未启用 WAL，你可以简单地通过查找 SST 中的最大时间戳来计算最新的提交时间戳。如果启用了 WAL，你应该进一步迭代所有恢复的内存表并找到最大时间戳。

在此任务中，你需要修改：

```
src/lsm_storage.rs
```

我们没有此部分的测试用例。完成此部分后，你应该通过前面章节的所有持久性测试（包括 2.5 和 2.6）。

## 测试你的理解

* 到目前为止，我们假设我们的 SST 文件使用单调递增的 id 作为文件名。使用 `<level>_<begin_key>_<end_key>_<max_ts>.sst` 作为 SST 文件名可以吗？这可能会有什么潜在问题？
* 考虑事务/快照的替代实现。在我们的实现中，我们在迭代器和事务上下文中拥有 `read_ts`，因此用户始终可以基于时间戳访问数据库一个版本的一致视图。将当前 LSM 状态直接存储在事务上下文中以获得一致快照是否可行？（即所有 SST id、它们的级别信息以及所有内存表 + ts）这样做的优缺点是什么？如果引擎没有内存表怎么办？如果引擎在像 S3 对象存储这样的分布式存储系统上运行怎么办？
* 假设你正在实现 MVCC Mini-LSM 引擎的备份实用程序。仅仅复制所有 SST 文件而不备份 LSM 状态是否足够？为什么或为什么不？

我们不提供问题的参考答案，欢迎在 Discord 社区中讨论它们。

{{#include copyright.md}}