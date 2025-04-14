use serde::{Deserialize, Serialize};

use crate::lsm_storage::LsmStorageState;

#[derive(Debug, Clone)]
pub struct SimpleLeveledCompactionOptions {
    pub size_ratio_percent: usize,
    pub level0_file_num_compaction_trigger: usize,
    pub max_levels: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleLeveledCompactionTask {
    // if upper_level is `None`, then it is L0 compaction
    pub upper_level: Option<usize>,
    pub upper_level_sst_ids: Vec<usize>,
    pub lower_level: usize,
    pub lower_level_sst_ids: Vec<usize>,
    /// 如果压缩到最低层,不需要考虑删除标记；中间层需要考虑
    pub is_lower_level_bottom_level: bool,
}

pub struct SimpleLeveledCompactionController {
    options: SimpleLeveledCompactionOptions,
}

impl SimpleLeveledCompactionController {
    pub fn new(options: SimpleLeveledCompactionOptions) -> Self {
        Self { options }
    }

    /// Generates a compaction task.
    ///
    /// Returns `None` if no compaction needs to be scheduled. The order of SSTs in the compaction task id vector matters.
    pub fn generate_compaction_task(
        &self,
        snapshot: &LsmStorageState,
    ) -> Option<SimpleLeveledCompactionTask> {
        if snapshot.l0_sstables.len() >= self.options.level0_file_num_compaction_trigger {
            return Some(SimpleLeveledCompactionTask {
                upper_level: None,
                upper_level_sst_ids: snapshot.l0_sstables.clone(),
                lower_level: 1,
                lower_level_sst_ids: snapshot.levels[0].1.clone(),
                is_lower_level_bottom_level: 1 == self.options.max_levels,
            });
        }
        if self.options.max_levels == 1 {
            return None;
        }
        let (mut lower_level_idx, mut upper_level_idx) = (None, None);
        for idx in 0..self.options.max_levels - 1 {
            let upper_num = snapshot.levels[idx].1.len();
            let lower_num = snapshot.levels[idx + 1].1.len();
            if upper_num == 0 {
                continue;
            }
            if (lower_num / upper_num) * 100 < self.options.size_ratio_percent {
                upper_level_idx = Some(idx);
                lower_level_idx = Some(idx + 1);
                break;
            }
        }

        if lower_level_idx.is_none() {
            return None;
        }

        let upper_level = upper_level_idx.unwrap() + 1;
        let lower_level = lower_level_idx.unwrap() + 1;
        Some(SimpleLeveledCompactionTask {
            upper_level: Some(upper_level),
            upper_level_sst_ids: snapshot.levels[upper_level - 1].1.clone(),
            lower_level,
            lower_level_sst_ids: snapshot.levels[lower_level - 1].1.clone(),
            is_lower_level_bottom_level: lower_level == self.options.max_levels,
        })
    }

    /// Apply the compaction result.
    ///
    /// The compactor will call this function with the compaction task and the list of SST ids generated. This function applies the
    /// result and generates a new LSM state. The functions should only change `l0_sstables` and `levels` without changing memtables
    /// and `sstables` hash map. Though there should only be one thread running compaction jobs, you should think about the case
    /// where an L0 SST gets flushed while the compactor generates new SSTs, and with that in mind, you should do some sanity checks
    /// in your implementation.
    pub fn apply_compaction_result(
        &self,
        snapshot: &LsmStorageState,
        task: &SimpleLeveledCompactionTask,
        output: &[usize],
    ) -> (LsmStorageState, Vec<usize>) {
        // println!(
        //     "apply task {task:?} output {output:?} to state.levels {:?}",
        //     snapshot.levels
        // );
        let mut state = snapshot.clone();
        let mut files_to_remove = Vec::new();

        if task.upper_level.is_none() {
            // L0 -> L1
            state
                .l0_sstables
                .retain(|sst_id| !task.upper_level_sst_ids.contains(sst_id));
        } else {
            // L -> L+1
            let upper_level = task.upper_level.unwrap();
            state.levels[upper_level - 1].1.clear();
        }
        state.levels[task.lower_level - 1].1 = output.to_vec();
        // println!("after apply new snapshoy.levels {:?}", state.levels);
        files_to_remove.extend(
            task.upper_level_sst_ids
                .iter()
                .chain(task.lower_level_sst_ids.iter()),
        );

        return (state, files_to_remove);
    }
}
