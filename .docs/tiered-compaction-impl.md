# Tiered Compaction 实现文档
## Tiered 概念
一组Key互不重叠的SST称为一个Sorted Run (一次合并/一次压缩的产物)
Tiered Compaction的组织方式:
将SST划分为由低到高的Tier,每一个Tier内部含有一个或多个Sorted Run,Tier内部是局部有序的
由于不强制维护每一个Tier内部的有序性,采用Tier压缩的LSM-Tree写放大会减小,读放大会增大

## 刷新策略

初始状态: levels = []  (空)  
第一次 flush:  
→ levels = [(tier_id=1, [sst_1])]   // 新 tier 插入前面  
第二次 flush:  
→ levels = [(tier_id=2, [sst_2]), (tier_id=1, [sst_1])]  // 新 tier 插入前面  
第三次 flush:  
→ levels = [(3, [sst_3]), (2, [sst_2]), (1, [sst_1])]  
// levels[0] = 最新 tier，levels[last] = 最老 tier（底层）  
当 levels.len() >= num_tiers (例如 8):  
→ 触发 compaction，合并多个 tiers  
→ 新 tier 放到 levels 末尾（底层位置）  
关键点:
1. 每次 flush：新 sorted run 插入 levels.insert(0, ...)（最新位置）
2. levels 索引：levels[0] = 最新，levels[last] = 最老（底层）
3. 压缩触发条件：levels.len() >= num_tiers
4. 压缩后新 tier 位置：放到被移除 tiers 的位置之后（往底层方向）

这与 Simple Leveled Compaction 不同——tiered 没有 L0，所有 sorted run 都在 level

## 压缩策略
    当 tier 数量 >= num_tiers才开始触发
### 空间放大触发
    (非底层总大小/底层大小) >= max_size_amplification_percent * 1%
触发全量压缩: 所有的tier合并为一个新的tier
### 大小比率触发
    (当前tier/之前所有tier) > (100+size_ration) * 1% 并且 待合并的tier数量 >= min_merge_width
压缩之前的tier
### 减少sorted run
    前两者都不触发,合并前max_merge_tiers个tier

## 代码集成
`lsm_storage.rs`:`Flush 时检查 flush_to_l0()，false 则直接 flush 到新 tier`
`compact/tiered.rs`: 实现策略
