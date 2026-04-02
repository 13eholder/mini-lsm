<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 快照读取 - 内存表和时间戳

在本章中，你将：

* 重构你的内存表/WAL 以存储一个键的多个版本。
* 实现新的引擎写入路径为每个键分配时间戳。
* 使你的压缩过程感知多版本键。
* 实现新的引擎读取路径以返回键的最新版本。

在重构期间，你可能需要根据需要将某些函数的签名从 `&self` 更改为 `self: &Arc<Self>`。

要运行测试用例：

```
cargo x copy-test --week 3 --day 2
cargo x scheck
```

**注意：完成本章后，你还需要通过所有 <= 2.4 的测试。**

## 任务 1：内存表、预写日志和读取路径

在此任务中，你需要修改：

```
src/wal.rs
src/mem_table.rs
src/lsm_storage.rs
```

我们已经使引擎中的大多数键成为 `KeySlice`，它包含一个字节键和一个时间戳。然而，我们系统的某些部分仍未考虑时间戳。在我们的第一个任务中，你需要修改你的内存表和 WAL 实现以考虑时间戳。

你需要首先更改内存表中存储的 `SkipMap` 类型。

```rust,no_run
pub struct MemTable {
    // map: Arc<SkipMap<Bytes, Bytes>>,
    map: Arc<SkipMap<KeyBytes, Bytes>>, // Bytes -> KeyBytes
    // ...
}
```

之后，你可以继续修复所有编译器错误以完成此任务。

**MemTable::get**

我们保留 get 接口，以便测试用例仍然可以探测内存表中键的特定版本。完成此任务后，此接口不应在你的读取路径中使用。考虑到我们在跳表中存储 `KeyBytes`，即 `(Bytes, u64)`，而用户探测的是 `KeySlice`，即 `(&[u8], u64)`。我们必须找到一种方法将后者转换为前者的引用，以便我们可以在跳表中检索数据。

为此，你可以使用不安全代码将 `&[u8]` 强制转换为静态，并使用 `Bytes::from_static` 从静态切片创建字节对象。这是安全的，因为 `Bytes` 不会尝试释放切片的内存，因为它被假定为静态的。

<details>

<summary>剧透：将 u8 切片转换为 Bytes</summary>

```rust,no_run
Bytes::from_static(unsafe { std::mem::transmute(key.key_ref()) })
```

</details>

这不是问题，因为我们之前有的是 `Bytes` 和 `&[u8]`，其中 `Bytes` 实现了 `Borrow<[u8]>`。

**MemTable::put**

签名应更改为 `fn put(&self, key: KeySlice, value: &[u8])`，你需要在实现中将键切片转换为 `KeyBytes`。

**MemTable::scan**

签名应更改为 `fn scan(&self, lower: Bound<KeySlice>, upper: Bound<KeySlice>) -> MemTableIterator`。你需要将 `KeySlice` 转换为 `KeyBytes` 并将这些用作 `SkipMap::range` 参数。

**MemTable::flush**

现在，在将内存表刷新到 SST 时，你应该使用键的时间戳而不是默认时间戳。

**MemTableIterator**

它现在应该存储 `(KeyBytes, Bytes)`，返回的键类型应为 `KeySlice`。

**Wal::recover** 和 **Wal::put**

预写日志现在应该接受键切片而不是用户键切片。在序列化和反序列化 WAL 记录时，你应该将时间戳放入 WAL 文件，并对时间戳和你之前拥有的所有其他字段进行校验和。

WAL 格式如下：

```
| key_len (exclude ts len) (u16) | key | ts (u64) | value_len (u16) | value | checksum (u32) |
```

**LsmStorageInner::get**

之前，我们将 `get` 实现为首先探测内存表，然后扫描 SST。现在我们已经将内存表更改为使用新的 key-ts API，我们需要重新实现 `get` 接口。最简单的方法是创建一个合并迭代器，覆盖我们拥有的一切——内存表、不可变内存表、L0 SST 和其他级别 SST，与你之前在 `scan` 中所做的相同，只是我们在 SST 上进行了布隆过滤器过滤。

**LsmStorageInner::scan**

你需要合并新的内存表 API，并且应将扫描范围设置为 `(user_key_begin, TS_RANGE_BEGIN)` 和 `(user_key_end, TS_RANGE_END)`。请注意，当你处理排除边界时，你需要正确地将迭代器定位到下一个键（而不是相同时间戳的当前键）。

## 任务 2：写入路径

在此任务中，你需要修改：

```
src/lsm_storage.rs
```

我们在 `LsmStorageInner` 中有一个 `mvcc` 字段，其中包含本周多版本并发控制所需的所有数据结构。当你打开目录并初始化存储引擎时，你需要创建该结构。

在你的 `write_batch` 实现中，你需要为写入批次中的所有键获取一个提交时间戳。你可以通过使用 `self.mvcc().latest_commit_ts() + 1` 在逻辑开始时获取时间戳，并在逻辑结束时使用 `self.mvcc().update_commit_ts(ts)` 来递增下一个提交时间戳。为确保所有写入批次具有不同的时间戳并且新键放置在旧键之上，你需要在函数开始时持有写锁 `self.mvcc().write_lock.lock()`，以便一次只有一个线程可以写入存储引擎。

## 任务 3：MVCC 压缩

在此任务中，你需要修改：

```
src/compact.rs
```

我们在前面章节中所做的是只保留键的最新版本，并在压缩键到底层时移除键（如果键被删除）。使用 MVCC，我们现在有了与键关联的时间戳，我们不能对压缩使用相同的逻辑。

在本章中，你可以简单地移除删除键的逻辑。你可以暂时忽略 `compact_to_bottom_level`，并且你应该在压缩期间保留键的所有版本。

此外，你需要以这样的方式实现压缩算法：即使超过 SST 大小限制，具有不同时间戳的相同键也会放在同一个 SST 文件中。这确保了如果一个键在某个级别的 SST 文件中找到，它不会在该级别的其他 SST 文件中，从而简化了系统许多部分的实现。

## 任务 4：LSM 迭代器

在此任务中，你需要修改：

```
src/lsm_iterator.rs
```

在上一章中，我们实现了 LSM 迭代器，将具有不同时间戳的相同键视为不同的键。现在，我们需要重构 LSM 迭代器，以便如果从子迭代器检索到键的多个版本，只返回键的最新版本。

你需要在迭代器中记录 `prev_key`。如果我们已经将键的最新版本返回给用户，我们可以跳过所有旧版本并继续下一个键。

此时，你应该通过前面章节的所有测试，除了持久性测试（2.5 和 2.6）。

## 测试你的理解

* MVCC 引擎中的 `get` 与你第 2 周构建的引擎有什么区别？
* 在第 2 周，当 `get` 时，你在找到键的第一个内存表/级别处停止。在 MVCC 版本中，你能做同样的事情吗？
* 如何将 `KeySlice` 转换为 `&KeyBytes`？这是一个安全/合理的操作吗？
* 为什么我们需要在写入路径中获取写锁？

我们不提供问题的参考答案，欢迎在 Discord 社区中讨论它们。

## 额外任务

* **内存表获取的早期停止**。与其创建覆盖所有内存表和 SST 的合并迭代器，我们可以如下实现 `get`：如果我们在内存表中找到键的版本，我们可以停止搜索。对于 SST 也是如此。

{{#include copyright.md}}