<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 写入路径

![Chapter Overview](./lsm-tutorial/week1-05-overview.svg)

在本章中，你将：

* 使用 L0 刷新实现 LSM 写入路径。
* 实现正确更新 LSM 状态的逻辑。

要将测试用例复制到起始代码并运行它们：

```
cargo x copy-test --week 1 --day 6
cargo x scheck
```

## 任务 1：将内存表刷新到 SST

此时，我们已经准备好了所有内存中的内容和磁盘上的文件，存储引擎能够读取和合并所有这些结构中的数据。现在，我们将实现将数据从内存移动到磁盘的逻辑（即刷新），并完成 Mini-LSM 第 1 周的课程。

在此任务中，你需要修改：

```
src/lsm_storage.rs
src/mem_table.rs
```

你需要修改 `LSMStorageInner::force_flush_next_imm_memtable` 和 `MemTable::flush`。在 `LSMStorageInner::open` 中，如果 LSM 数据库目录不存在，你需要创建它。要将内存表刷新到磁盘，我们需要做三件事：

* 选择一个内存表进行刷新。
* 创建一个对应于内存表的 SST 文件。
* 从不可变内存表列表中移除内存表，并将 SST 文件添加到 L0 SST 中。

我们尚未解释什么是 L0（第 0 级）SST。通常，它们是作为内存表刷新结果直接创建的 SST 文件集。在本课程的第 1 周，我们只在磁盘上有 L0 SST。我们将在第 2 周深入探讨如何使用分层或分级结构在磁盘上高效地组织它们。

注意，创建 SST 文件是一个计算密集且昂贵的操作。同样，我们不希望长时间持有 `state` 读/写锁，因为它可能会阻塞其他操作并在 LSM 操作中产生巨大的延迟峰值。此外，我们使用 `state_lock` 互斥锁来序列化 LSM 树中的状态修改操作。在此任务中，你需要仔细思考如何使用这些锁来使 LSM 状态修改无竞态条件，同时最小化关键部分。

我们没有并发测试用例，你需要仔细思考你的实现。此外，请记住，不可变内存表列表中的最后一个内存表是最早的，也是你应该刷新的那个。

<details>

<summary>剧透：刷新 L0 伪代码</summary>

```rust,no_run
fn flush_l0(&self) {
    let _state_lock = self.state_lock.lock();

    let memtable_to_flush;
    let snapshot = {
        let guard = self.state.read();
        memtable_to_flush = guard.imm_memtables.last();
    };

    let sst = memtable_to_flush.flush()?;

    {
        let guard = self.state.write();
        guard.imm_memtables.pop();
        guard.l0_sstables.insert(0, sst);
    };

}
```

</details>

## 任务 2：刷新触发器

在此任务中，你需要修改：

```
src/lsm_storage.rs
src/compact.rs
```

当内存中的内存表数量（不可变 + 可变）超过 LSM 存储选项中的 `num_memtable_limit` 时，你应该将最早的内存表刷新到磁盘。这是由后台的刷新线程完成的。刷新线程将随 `MiniLSM` 结构启动。我们已经实现了必要的代码来启动线程并正确停止线程。

在此任务中，你需要在 `compact.rs` 中实现 `LsmStorageInner::trigger_flush`，并在 `lsm_storage.rs` 中实现 `MiniLsm::close`。`trigger_flush` 将每 50 毫秒执行一次。如果内存表数量超过限制，你应该调用 `force_flush_next_imm_memtable` 来刷新内存表。当用户调用 `close` 函数时，你应该等待刷新线程（以及第 2 周的压缩线程）完成。

## 任务 3：过滤 SST

现在你有了一个完全工作的存储引擎，你可以使用 mini-lsm-cli 与你的存储引擎交互。

```shell
cargo run --bin mini-lsm-cli -- --compaction none
```

然后：

```
fill 1000 3000
get 2333
flush
fill 1000 3000
get 2333
flush
get 2333
scan 2000 2333
```

如果你填充更多数据，你可以看到你的刷新线程工作并自动刷新 L0 SST，而无需使用 `flush` 命令。

最后，在结束本周之前，让我们实现一个简单的优化，在过滤 SST 方面。基于用户提供的键范围，我们可以轻松过滤掉一些不包含键范围的 SST，这样我们就不需要在合并迭代器中读取它们。

在此任务中，你需要修改：

```
src/lsm_storage.rs
src/iterators/*
src/lsm_iterator.rs
```

你需要更改读取路径函数，以跳过不可能包含键/键范围的 SST。你需要为迭代器实现 `num_active_iterators`，以便测试用例可以检查你的实现是否正确。对于 `MergeIterator` 和 `TwoMergeIterator`，它是所有子迭代器的 `num_active_iterators` 之和。注意，如果你没有修改 `MergeIterator` 起始代码中的字段，记得也要考虑 `MergeIterator::current`。对于 `LsmIterator` 和 `FusedIterator`，只需从内部迭代器返回活动迭代器的数量。

你可以实现像 `range_overlap` 和 `key_within` 这样的辅助函数来简化你的代码。

## 测试你的理解

* 如果用户请求删除一个键两次会发生什么？
* 当迭代器初始化时，同时加载到内存中的内存（或块数）是多少？
* 一些疯狂的用户想要*分叉*他们的 LSM 树。他们想要启动引擎以摄取一些数据，然后分叉它，以便他们获得两个相同的数据集，然后分别对它们进行操作。一个简单但低效的实现方法是简单地将所有 SST 和内存结构复制到一个新目录并启动引擎。然而，请注意，我们从不修改磁盘上的文件，实际上我们可以重用父引擎的 SST 文件。你认为如何在不复制数据的情况下高效地实现此分叉功能？（查看 [Neon Branching](https://neon.tech/docs/introduction/branching)）。
* 想象你正在构建一个多租户 LSM 系统，在单个 128GB 内存的机器上托管 10k 个数据库。内存表大小限制设置为 256MB。对于此设置，你需要多少内存用于内存表？
  * 显然，你没有足够的内存用于所有这些内存表。假设每个用户仍然有自己的内存表，你如何设计内存表刷新策略以使其工作？让所有这些用户共享同一个内存表（即通过将租户 ID 编码为键前缀）是否有意义？

我们不提供问题的参考答案，欢迎在 Discord 社区中讨论它们。

## 额外任务

* **实现写入/L0 暂停。** 当内存表数量超过最大数量太多时，你可以阻止用户写入存储引擎。你也可以在第 2 周实现压缩后为 L0 表实现写入暂停。
* **前缀扫描。** 你可以通过实现前缀扫描接口并使用前缀信息来过滤更多 SST。

{{#include copyright.md}}