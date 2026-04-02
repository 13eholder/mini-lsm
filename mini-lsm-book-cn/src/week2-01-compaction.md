<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 压缩实现

![Chapter Overview](./lsm-tutorial/week2-01-full.svg)

在本章中，你将：

* 实现合并一些文件并产生新文件的压缩逻辑。
* 实现更新 LSM 状态和管理文件系统上 SST 文件的逻辑。
* 更新 LSM 读取路径以整合 LSM 级别。

要将测试用例复制到起始代码并运行它们：

```
cargo x copy-test --week 2 --day 1
cargo x scheck
```

<div class="warning">

在阅读本章之前，查看[第 2 周概述](./week2-overview.md)可能有助于对压缩有一个总体了解。

</div>

## 任务 1：压缩实现

在此任务中，你将实现执行压缩的核心逻辑——将一组 SST 文件归并排序为一个排序运行。你需要修改：

```
src/compact.rs
```

具体来说，`force_full_compaction` 和 `compact` 函数。`force_full_compaction` 是决定压缩哪些文件并更新 LSM 状态的压缩触发器。`compact` 执行实际的压缩工作，合并一些 SST 文件并返回一组新的 SST 文件。

你的压缩实现应该获取存储引擎中的所有 SST，使用 `MergeIterator` 对它们进行合并，然后使用 SST 构建器将结果写入新文件。如果文件太大，你需要拆分 SST 文件。压缩完成后，你可以更新 LSM 状态，将所有新的排序运行添加到 LSM 树的第一级。并且，你需要移除 LSM 树中未使用的文件。在你的实现中，SST 应该只存储在两个地方：L0 SST 和 L1 SST。也就是说，LSM 状态中的 `levels` 结构应该只有一个向量。在 `LsmStorageState` 中，我们已经初始化 LSM 在 `levels` 字段中具有 L1。

压缩不应阻塞 L0 刷新，因此你不应在合并文件时持有状态锁。你应该只在压缩过程结束时更新 LSM 状态时持有状态锁，并在完成状态修改后立即释放锁。

你可以假设用户将确保只有一次压缩在进行。`force_full_compaction` 在任何时候只会在一个线程中被调用。放入第 1 级的 SST 应按其第一个键排序，并且不应有重叠的键范围。

<details>

<summary>剧透：压缩伪代码</summary>

```rust,no_run
fn force_full_compaction(&self) {
    let ssts_to_compact = {
        let state = self.state.read();
        state.l0_sstables + state.levels[0]
    };
    let new_ssts = self.compact(FullCompactionTask(ssts_to_compact))?;
    {
        let state_lock = self.state_lock.lock();
        let state = self.state.write();
        state.l0_sstables.remove(/* 正在压缩的那些 */);
        state.levels[0] = new_ssts; // 新 SST 添加到 L1
    };
    std::fs::remove(ssts_to_compact)?;
}
```

</details>

在你的压缩实现中，目前只需要处理 `FullCompaction`，其中任务信息包含你需要压缩的 SST。你还需要确保 SST 的顺序正确，以便键的最新版本被放入新的 SST。

因为我们总是压缩所有 SST，如果我们找到一个键的多个版本，我们可以简单地保留最新的一个。如果最新版本是删除标记，我们不需要将其保留在产生的 SST 文件中。这不适用于接下来几章中的压缩策略。

有一些事情你可能需要考虑。

* 你的实现如何处理与压缩并行的 L0 刷新？（执行压缩时不持有状态锁，还需要考虑压缩进行时产生的新 L0 文件。）
* 如果你的实现在压缩完成后立即移除原始 SST 文件，会导致系统出现问题吗？（通常在 macOS/Linux 上不会，因为操作系统在没有任何文件句柄持有之前不会实际删除文件。）

## 任务 2：连接迭代器

在此任务中，你需要修改：

```
src/iterators/concat_iterator.rs
```

现在你已经在系统中创建了排序运行，可以对读取路径进行简单的优化。你并不总是需要为 SST 创建合并迭代器。如果 SST 属于一个排序运行，你可以创建一个连接迭代器，简单地按顺序迭代每个 SST 中的键，因为一个排序运行中的 SST 不包含重叠的键范围，并且它们按第一个键排序。我们不想提前创建所有 SST 迭代器（因为这将导致一个块读取），因此我们只在此迭代器中存储 SST 对象。

## 任务 3：与读取路径集成

在此任务中，你需要修改：

```
src/lsm_iterator.rs
src/lsm_storage.rs
src/compact.rs
```

现在你的 LSM 树有了两级结构，你可以更改读取路径以使用新的连接迭代器来优化读取路径。

你需要更改 `LsmStorageIterator` 的内部迭代器类型。之后，你可以构造一个双合并迭代器来合并内存表和 L0 SST，以及另一个合并迭代器来合并该迭代器与 L1 连接迭代器。

你也可以更改压缩实现以利用连接迭代器。

你需要为连接迭代器实现 `num_active_iterators`，以便测试用例可以测试你的实现是否使用了连接迭代器，它应该始终为 1。

要交互式地测试你的实现：

```shell
cargo run --bin mini-lsm-cli-ref -- --compaction none # 参考解决方案
cargo run --bin mini-lsm-cli -- --compaction none # 你的解决方案
```

然后：

```
fill 1000 3000
flush
fill 1000 3000
flush
full_compaction
fill 1000 3000
flush
full_compaction
get 2333
scan 2000 2333
```

## 测试你的理解

* 读取/写入/空间放大的定义是什么？（这在概述章节中已涵盖）
* 准确计算读取/写入/空间放大的方法有哪些，估计它们的方法有哪些？
* 即使用户请求删除一个键，该键仍会占用一些存储空间，这是正确的吗？
* 鉴于压缩占用大量写入带宽和读取带宽，并可能干扰前台操作，当有大量写入流时推迟压缩是一个好主意。在这种情况下甚至停止/暂停现有的压缩任务是有益的。你对此有何看法？（阅读 [SILK: Preventing Latency Spikes in Log-Structured Merge Key-Value Stores](https://www.usenix.org/conference/atc19/presentation/balmau) 论文！）
* 为压缩使用/填充块缓存是一个好主意吗？还是在压缩时完全绕过块缓存更好？
* 在系统中拥有 `struct ConcatIterator<I: StorageIterator>` 有意义吗？
* 一些研究人员/工程师提议将压缩卸载到远程服务器或无服务器 lambda 函数。远程压缩的好处是什么，以及远程压缩可能带来的潜在挑战和性能影响是什么？（思考压缩完成时以及下一个读取请求时块缓存会发生什么...）

我们不提供问题的参考答案，欢迎在 Discord 社区中讨论它们。

{{#include copyright.md}}