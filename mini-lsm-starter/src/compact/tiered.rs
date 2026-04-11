// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;

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
        // check num tier
        if snapshot.levels.len() < self.options.num_tiers {
            return None;
        }
        // 空间放大策略
        let tiers = snapshot.levels.clone();
        let last_tier = tiers.len() - 1;
        let bottom_tier_size = tiers[last_tier].1.len();
        let upper_tier_size = tiers
            .iter()
            .take(last_tier)
            .map(|(_, sst_ids)| sst_ids.len())
            .sum::<usize>();
        if upper_tier_size as f64 / bottom_tier_size as f64
            >= self.options.max_size_amplification_percent as f64 / 100.0
        {
            return Some(TieredCompactionTask {
                tiers,
                bottom_tier_included: true,
            });
        }
        // 大小比率触发
        let mut upper_tier_size = 0;
        for tier in 0..last_tier {
            upper_tier_size += tiers[tier].1.len();
            let lower_tier_size = tiers[tier + 1].1.len();
            if lower_tier_size as f64 / upper_tier_size as f64
                > 1.0 + self.options.size_ratio as f64 / 100.0
                && tier + 1 >= self.options.min_merge_width
            {
                return Some(TieredCompactionTask {
                    tiers: tiers.into_iter().take(tier + 1).collect(),
                    bottom_tier_included: false,
                });
            }
        }
        // 减少sorted run
        let max_merge_width = self.options.max_merge_width.unwrap_or(usize::MAX);
        Some(TieredCompactionTask {
            tiers: tiers.into_iter().take(max_merge_width).collect(),
            bottom_tier_included: last_tier < max_merge_width,
        })
    }

    pub fn apply_compaction_result(
        &self,
        snapshot: &LsmStorageState,
        task: &TieredCompactionTask,
        output: &[usize],
    ) -> (LsmStorageState, Vec<usize>) {
        let mut new_snapshot = snapshot.clone();

        let mut tier_to_remove = task
            .tiers
            .iter()
            .map(|(x, y)| (*x, y))
            .collect::<HashMap<_, _>>();

        let mut levels = Vec::new();
        let mut new_tier_added = false;
        let mut files_to_remove = Vec::new();
        //flush thread可能会刷新新的层级
        for (tier_id, files) in &new_snapshot.levels {
            if let Some(ffiles) = tier_to_remove.remove(tier_id) {
                files_to_remove.extend(ffiles.iter().copied());
            } else {
                levels.push((*tier_id, files.clone()));
            }

            if tier_to_remove.is_empty() && !new_tier_added {
                new_tier_added = true;
                levels.push((output[0], output.to_vec()));
            }
        }

        new_snapshot.levels = levels;
        (new_snapshot, files_to_remove)
    }
}
