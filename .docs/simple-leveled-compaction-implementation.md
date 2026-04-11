# Simple Leveled Compaction 实现计划

## 概述

本文档记录 mini-lsm week2-02 (Simple Leveled Compaction) 的完整实现计划，涵盖三个 Task 的实现细节。

---

## Task 1: Simple Leveled Compaction 核心逻辑

**文件**: `mini-lsm-starter/src/compact/simple_leveled.rs`

### 1.1 `generate_compaction_task` 实现

**职责**: 根据当前 LSM 状态生成压缩任务，返回 `None` 表示无需压缩。

**实现逻辑**:

1. **边界检查**: 若 `max_levels == 0`，直接返回 `None`

2. **收集各层大小**:
   ```rust
   let mut level_sizes = Vec::new();
   level_sizes.push(snapshot.l0_sstables.len());
   for (_, files) in &snapshot.levels {
       level_sizes.push(files.len());
   }
   ```

3. **L0 触发检查**:
   - 条件: `snapshot.l0_sstables.len() >= level0_file_num_compaction_trigger`
   - 返回任务: `upper_level = None`, `lower_level = 1`
   - `is_lower_level_bottom_level = (max_levels == 1)`

4. **Size Ratio 触发检查** (L1 ~ L_max-1):
   - 遍历 `i` 从 1 到 `max_levels - 1`
   - 计算: `size_ratio = level_sizes[i+1] / level_sizes[i]`
   - 条件: `size_ratio < size_ratio_percent / 100.0`
   - 注意: 若上层为空 (`upper_size == 0`) 则跳过
   - 返回任务: `upper_level = Some(i)`, `lower_level = i + 1`

**关键注意**:
- `levels` 向量是 0-based: `levels[0]` 对应 L1, `levels[1]` 对应 L2
- 任务中的 `level` 是 1-based

### 1.2 `apply_compaction_result` 实现

**职责**: 将压缩结果应用到 LSM 状态，返回新状态和需要删除的文件列表。

**实现逻辑**:

1. 克隆 snapshot
2. **处理上层文件移除**:
   - 若 `upper_level = Some(level)`: 清空 `levels[level-1]` 的文件列表
   - 若 `upper_level = None` (L0): 从 `l0_sstables` 中过滤掉已压缩的 SST
3. **处理下层文件替换**: 将 `levels[lower_level-1]` 替换为 output
4. **Sanity Checks**:
   - 验证 `upper_level_sst_ids` 与当前状态一致
   - 验证 `lower_level_sst_ids` 与当前状态一致
   - 确保所有预期的 L0 SST 都被找到（处理并发 flush 场景）
5. 返回 `(new_state, files_to_remove)`

---

## Task 2: Compaction Thread

**文件**: `mini-lsm-starter/src/compact.rs`

### 2.1 `compact` 方法 - Simple 分支

**实现逻辑**:

根据 `upper_level` 是否为 `None` 区分两种情况:

**情况 A: L1+ 压缩 (`upper_level = Some(_)`)**
- 上层 SST → `SstConcatIterator` (key range 不重叠)
- 下层 SST → `SstConcatIterator` (key range 不重叠)
- 用 `TwoMergeIterator` 合并两层

**情况 B: L0 压缩 (`upper_level = None`)**
- L0 SST → `MergeIterator` (key range 可能重叠)
- 下层 SST → `SstConcatIterator` (key range 不重叠)
- 用 `TwoMergeIterator` 合并

**迭代器选择原则**: 尽可能减少活跃迭代器数量，使用 concat iterator 替代 merge iterator。

### 2.2 `trigger_compaction` 实现

**完整流程**:

1. 获取当前 state snapshot
2. 调用 `generate_compaction_task`，若无任务则返回 `Ok(())`
3. 调用 `compact` 执行压缩，得到新 SST 列表
4. 获取 `state_lock` 锁:
   - 克隆 state
   - 将新 SST 插入 `sstables` HashMap
   - 调用 `apply_compaction_result` 获取新状态和待删除文件列表
   - 从 `sstables` 中移除待删除文件的引用
   - 写入新 state
5. 释放锁后，删除磁盘上旧的 SST 文件

**并发安全**:
- 在持有 `state_lock` 期间做状态更新
- 在释放锁后做文件删除（避免长时间持锁）
- `apply_compaction_result` 内部做 sanity check 处理并发 flush 场景

---

## Task 3: 读取路径集成

**文件**: `mini-lsm-starter/src/lsm_storage.rs`

### 3.1 `scan` 方法优化

**原有实现问题**: 将所有 levels 的 SST 混在一起用 `MergeIterator` 处理，效率低。

**优化方案**:

1. **Memtables**: `MergeIterator` 合并 memtable + imm_memtables
2. **L0 SSTs**: `MergeIterator` (key range 可能重叠)
3. **L1+ SSTs**: `SstConcatIterator` (key range 不重叠，更高效)
4. 用 `TwoMergeIterator` 逐层合并:
   - memtable_iter + l0_iter → `TwoMergeIterator`
   - 上述结果 + level_iter → `TwoMergeIterator`
   - 最后包装 `LsmIterator` 和 `FusedIterator`

### 3.2 `get` 方法

**无需修改**: 现有实现已经正确遍历所有 levels（`l0_sstables.chain(levels.flat_map(...))`），对每个 SST 做 bloom filter 和 key range 检查。

---

## 参数说明

| 参数                                 | 含义                              | 示例值      |
| ------------------------------------ | --------------------------------- | ----------- |
| `size_ratio_percent`                 | 下层文件数/上层文件数的百分比阈值 | 200 (即 2x) |
| `level0_file_num_compaction_trigger` | L0 触发压缩的文件数阈值           | 2           |
| `max_levels`                         | LSM 树的层数（不含 L0）           | 3           |

## 压缩示例

```
初始: L0(2): [1,2], L1(0): [], L2(0): [], L3(0): []
↓ L0 触发
L0(0): [], L1(2): [3,4], L2(0): [], L3(0): []
↓ L1/L2 ratio 触发 (0/2 < 200%)
L0(0): [], L1(0): [], L2(2): [5,6], L3(0): []
↓ L2/L3 ratio 触发 (0/2 < 200%)
L0(0): [], L1(0): [], L2(0): [], L3(2): [7,8]
```

---

## 测试验证

运行命令:
```bash
cargo x copy-test --week 2 --day 2
cargo x scheck
```

预期结果: 47 tests passed, 0 failed

关键测试: `week2_day2::test_integration` - 完整的 simple leveled compaction 集成测试