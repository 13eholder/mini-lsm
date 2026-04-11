# Leveled Compaction 实现文档
## 概念
LSM-Tree通用的压缩算法: L0 无序, L1-LN 有序; 压缩时优先压缩到最后一层来减少中间层级
从最后一层开始逐层计算target_size; 压缩到 target_size > 0 且 current_size < level_size