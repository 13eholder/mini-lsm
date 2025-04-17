use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::lsm_storage::LsmStorageState;

#[derive(Debug, Serialize, Deserialize)]
pub struct TieredCompactionTask {
    pub tiers: Vec<(usize, Vec<usize>)>,
    pub bottom_tier_included: bool,
}

#[derive(Debug, Clone)]
pub struct TieredCompactionOptions {
    pub num_tiers: usize,
    pub max_size_amplification_percent: usize,
    pub size_ratio: usize,
    pub min_merge_width: usize,
    pub max_merge_width: Option<usize>,
}

pub struct TieredCompactionController {
    options: TieredCompactionOptions,
}

impl TieredCompactionController {
    pub fn new(options: TieredCompactionOptions) -> Self {
        Self { options }
    }

    pub fn generate_compaction_task(
        &self,
        snapshot: &LsmStorageState,
    ) -> Option<TieredCompactionTask> {
        if snapshot.levels.len() < self.options.num_tiers {
            return None;
        }
        // space amp
        let mut upper_level_size = 0;
        for i in 0..(snapshot.levels.len() - 1) {
            upper_level_size += snapshot.levels[i].1.len();
        }
        let lower_level_size = snapshot.levels.last().unwrap().1.len();
        let space_amp_ratio = (upper_level_size as f64) / (lower_level_size as f64) * 100.0;
        if space_amp_ratio >= self.options.max_size_amplification_percent as f64 {
            return Some(TieredCompactionTask {
                tiers: snapshot.levels.clone(),
                bottom_tier_included: true,
            });
        }
        // size ratio
        upper_level_size = 0;
        for i in 0..(snapshot.levels.len() - 1) {
            upper_level_size += snapshot.levels[i].1.len();
            let lower_level_size = snapshot.levels[i + 1].1.len();
            let size_ratio = (upper_level_size as f64) / (lower_level_size as f64) * 100.0;
            if size_ratio > self.options.size_ratio as f64 && i + 1 >= self.options.min_merge_width
            {
                return Some(TieredCompactionTask {
                    tiers: snapshot
                        .levels
                        .iter()
                        .take(i + 1)
                        .cloned()
                        .collect::<Vec<_>>(),
                    bottom_tier_included: i + 1 >= snapshot.levels.len(),
                });
            }
        }
        // 全部压缩减少读放大

        let num_tiers_to_take = snapshot
            .levels
            .len()
            .min(self.options.max_merge_width.unwrap_or(usize::MAX));
        Some(TieredCompactionTask {
            tiers: snapshot
                .levels
                .iter()
                .take(num_tiers_to_take)
                .cloned()
                .collect::<Vec<_>>(),
            bottom_tier_included: snapshot.levels.len() >= num_tiers_to_take,
        })
    }

    pub fn apply_compaction_result(
        &self,
        snapshot: &LsmStorageState,
        task: &TieredCompactionTask,
        output: &[usize],
    ) -> (LsmStorageState, Vec<usize>) {
        let mut snapshot = snapshot.clone();
        let mut tier_to_remove = task.tiers.iter().map(|(x, _)| *x).collect::<HashSet<_>>();
        let mut levels = Vec::new();
        let mut files_to_remove = Vec::new();
        let mut add_new_tier = false;
        for (tier_id, files) in &snapshot.levels {
            if tier_to_remove.remove(tier_id) {
                files_to_remove.extend(files.iter().copied());
            } else {
                levels.push((*tier_id, files.clone()));
            }
            // 注意层级,size_ratio引起的压缩会在中间插入新tier
            if tier_to_remove.is_empty() && !add_new_tier {
                add_new_tier = true;
                levels.push((output[0], output.to_vec()));
            }
        }
        snapshot.levels = levels;
        (snapshot, files_to_remove)
    }
}
