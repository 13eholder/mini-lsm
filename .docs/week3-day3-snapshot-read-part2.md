# Week 3 Day 3 Snapshot Read Part 2

## 任务目标

对齐 `mini-lsm-book/src/week3-01-ts-key-refactor.md`、
`mini-lsm-book/src/week3-02-snapshot-read-part-1.md`、
`mini-lsm-book/src/week3-03-snapshot-read-part-2.md`，
检查并修正 `mini-lsm-starter/src` 的 week3 snapshot read 实现。

## 发现的问题

1. `Watermark`、`LsmMvccInner::new_txn`、`Transaction::drop` 没接通。
   snapshot 创建后没有登记 read ts，释放时也没有移除 reader。
2. WAL 恢复只统计了 SST 和 immutable memtable 的最大 ts，没有统计当前 memtable WAL。
   数据恢复后 `latest_commit_ts` 可能偏小，导致新 snapshot 看不到刚恢复的数据。
3. flush 到 SST 后的范围扫描边界有 bug。
   `Bound::Excluded` 的上界场景会把越界 key 继续暴露给用户。
4. 读路径职责有些混杂。
   tombstone 过滤一部分在 `LsmIterator`，一部分在 `FusedIterator`，边界处理分散，不利于维护。

## 实际修改

### MVCC 生命周期

- 在 `src/mvcc/watermark.rs` 实现 reader 引用计数和最小 read ts 计算。
- 在 `src/mvcc.rs` 的 `new_txn` 中登记 snapshot 的 `read_ts`。
- 在 `src/mvcc/txn.rs` 的 `Drop` 中移除 reader。

### Snapshot Read 路径

- 在 `src/lsm_iterator.rs` 中把 `read_ts` 过滤、同 user key 旧版本跳过、tombstone 跳过统一收敛到 `LsmIterator`。
- 去掉不再需要的 `LsmRangeIterator`，降低一层状态机复杂度。
- 在 `src/mvcc/txn.rs` 中让 `TxnIterator` 主动跳过 delete，保证用户视图不返回 tombstone。

### Range 边界和 memtable 映射

- 在 `src/mem_table.rs` 增加 `map_key_bound_plus_ts`，统一把用户 range 转成 MVCC key range。
- 在 `src/lsm_storage.rs` 的 `scan_with_ts` 中使用这套边界映射。
- 修复 `Excluded` 边界下 SST seek 后只跳过一个版本的问题，改成跳过同 user key 的全部版本。

### 恢复路径

- 在 `src/lsm_storage.rs` 的 `open` 中，把当前 memtable WAL 的 `max_ts` 一并纳入恢复计算。
- `mvcc` 初始化改为使用恢复得到的 `recovered_max_ts`。

### 回归测试

- `src/lsm_storage.rs`
  - `test_recover_commit_ts_from_current_wal`
  - `test_flushed_snapshot_scan_bounds`
- `src/tests/week3_day3.rs`
  - 挂载 week3 day3 测试模块并跑通 snapshot read 回归。

## 验证结果

执行通过：

- `cargo test -p mini-lsm-starter week3_day3 -- --nocapture`
- `cargo test -p mini-lsm-starter week3_day2 -- --nocapture`
- `cargo test -p mini-lsm-starter week2_day6 -- --nocapture`
- `cargo x scheck`

`cargo x scheck` 仍有 warning，但当前剩余 warning 主要来自 week5/6 之前尚未使用的 OCC/serializable 结构体字段，不影响 week3 day3 的正确性。

## 剩余可优化项

1. `src/lsm_storage.rs` 里内部 `get(self: &Arc<Self>, ...)` 目前没有被直接调用，可以在后续统一 API 时收敛。
2. `src/mvcc.rs` / `src/mvcc/txn.rs` 中 `commit_lock`、`committed_txns`、`key_hashes` 等字段是为后续事务冲突检测预留的，现在会产生 dead code warning。
3. `MemTable::get` 里的 `unsafe transmute` 是任务书建议的做法，已经补上类型注解；后续如果要继续优化，可以考虑把 probe 辅助逻辑再封装一下，减少局部 unsafe 暴露。
