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

#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

mod leveled;
mod simple_leveled;
mod tiered;

use core::panic;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
pub use leveled::{LeveledCompactionController, LeveledCompactionOptions, LeveledCompactionTask};
use serde::{Deserialize, Serialize};
pub use simple_leveled::{
    SimpleLeveledCompactionController, SimpleLeveledCompactionOptions, SimpleLeveledCompactionTask,
};
pub use tiered::{TieredCompactionController, TieredCompactionOptions, TieredCompactionTask};

use crate::iterators::StorageIterator;
use crate::iterators::concat_iterator::SstConcatIterator;
use crate::iterators::merge_iterator::MergeIterator;
use crate::iterators::two_merge_iterator::TwoMergeIterator;
use crate::key::KeySlice;
use crate::lsm_iterator::FusedIterator;
use crate::lsm_storage::{LsmStorageInner, LsmStorageState};
use crate::table::{SsTable, SsTableBuilder, SsTableIterator};

#[derive(Debug, Serialize, Deserialize)]
pub enum CompactionTask {
    Leveled(LeveledCompactionTask),
    Tiered(TieredCompactionTask),
    Simple(SimpleLeveledCompactionTask),
    ForceFullCompaction {
        l0_sstables: Vec<usize>,
        l1_sstables: Vec<usize>,
    },
}

impl CompactionTask {
    fn compact_to_bottom_level(&self) -> bool {
        match self {
            CompactionTask::ForceFullCompaction { .. } => true,
            CompactionTask::Leveled(task) => task.is_lower_level_bottom_level,
            CompactionTask::Simple(task) => task.is_lower_level_bottom_level,
            CompactionTask::Tiered(task) => task.bottom_tier_included,
        }
    }
}

pub(crate) enum CompactionController {
    Leveled(LeveledCompactionController),
    Tiered(TieredCompactionController),
    Simple(SimpleLeveledCompactionController),
    NoCompaction,
}

impl CompactionController {
    pub fn generate_compaction_task(&self, snapshot: &LsmStorageState) -> Option<CompactionTask> {
        match self {
            CompactionController::Leveled(ctrl) => ctrl
                .generate_compaction_task(snapshot)
                .map(CompactionTask::Leveled),
            CompactionController::Simple(ctrl) => ctrl
                .generate_compaction_task(snapshot)
                .map(CompactionTask::Simple),
            CompactionController::Tiered(ctrl) => ctrl
                .generate_compaction_task(snapshot)
                .map(CompactionTask::Tiered),
            CompactionController::NoCompaction => unreachable!(),
        }
    }

    pub fn apply_compaction_result(
        &self,
        snapshot: &LsmStorageState,
        task: &CompactionTask,
        output: &[usize],
        in_recovery: bool,
    ) -> (LsmStorageState, Vec<usize>) {
        match (self, task) {
            (CompactionController::Leveled(ctrl), CompactionTask::Leveled(task)) => {
                ctrl.apply_compaction_result(snapshot, task, output, in_recovery)
            }
            (CompactionController::Simple(ctrl), CompactionTask::Simple(task)) => {
                ctrl.apply_compaction_result(snapshot, task, output)
            }
            (CompactionController::Tiered(ctrl), CompactionTask::Tiered(task)) => {
                ctrl.apply_compaction_result(snapshot, task, output)
            }
            _ => unreachable!(),
        }
    }
}

impl CompactionController {
    pub fn flush_to_l0(&self) -> bool {
        matches!(
            self,
            Self::Leveled(_) | Self::Simple(_) | Self::NoCompaction
        )
    }
}

#[derive(Debug, Clone)]
pub enum CompactionOptions {
    /// Leveled compaction with partial compaction + dynamic level support (= RocksDB's Leveled
    /// Compaction)
    Leveled(LeveledCompactionOptions),
    /// Tiered compaction (= RocksDB's universal compaction)
    Tiered(TieredCompactionOptions),
    /// Simple leveled compaction
    Simple(SimpleLeveledCompactionOptions),
    /// In no compaction mode (week 1), always flush to L0
    NoCompaction,
}

impl LsmStorageInner {
    fn do_compact<I>(&self, iter: &mut I) -> Result<Vec<Arc<SsTable>>>
    where
        I: for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>,
    {
        let mut new_ssts = Vec::new();
        let mut builder = SsTableBuilder::new(self.options.block_size);

        while iter.is_valid() {
            builder.add(iter.key(), iter.value());
            iter.next()?;
            if builder.estimated_size() >= self.options.target_sst_size {
                let finished_builder =
                    std::mem::replace(&mut builder, SsTableBuilder::new(self.options.block_size));
                let sst_id = self.next_sst_id();
                let sst = finished_builder.build(
                    sst_id,
                    Some(self.block_cache.clone()),
                    self.path_of_sst(sst_id),
                )?;
                new_ssts.push(Arc::new(sst));
            }
        }

        if builder.estimated_size() > 0 {
            let sst_id = self.next_sst_id();
            let sst = builder.build(
                sst_id,
                Some(self.block_cache.clone()),
                self.path_of_sst(sst_id),
            )?;
            new_ssts.push(Arc::new(sst));
        }

        Ok(new_ssts)
    }
    fn compact(&self, task: &CompactionTask) -> Result<Vec<Arc<SsTable>>> {
        let compact_to_bottom_level = task.compact_to_bottom_level();
        let snapshot = self.state.read().clone();

        match task {
            CompactionTask::ForceFullCompaction {
                l0_sstables,
                l1_sstables,
            } => {
                let l0_ssts = l0_sstables
                    .iter()
                    .filter_map(|id| snapshot.sstables.get(id))
                    .cloned()
                    .collect::<Vec<_>>();
                let l1_ssts = l1_sstables
                    .iter()
                    .filter_map(|id| snapshot.sstables.get(id))
                    .cloned()
                    .collect::<Vec<_>>();

                let mut iters_to_merge = Vec::with_capacity(l0_ssts.len() + l1_ssts.len());
                for l0_sst in l0_ssts {
                    iters_to_merge
                        .push(Box::new(SsTableIterator::create_and_seek_to_first(l0_sst)?));
                }
                for l1_sst in l1_ssts {
                    iters_to_merge
                        .push(Box::new(SsTableIterator::create_and_seek_to_first(l1_sst)?));
                }

                if compact_to_bottom_level {
                    self.do_compact(&mut FusedIterator::new(MergeIterator::create(
                        iters_to_merge,
                    )))
                } else {
                    self.do_compact(&mut MergeIterator::create(iters_to_merge))
                }
            }

            CompactionTask::Leveled(task) => {
                unimplemented!()
            }
            CompactionTask::Simple(task) => {
                // 根据lower_level_sst_ids创建SstConcactIterator
                // 判断upper_level 是否是L0
                let lower_level_ssts = task
                    .lower_level_sst_ids
                    .iter()
                    .filter_map(|id| snapshot.sstables.get(id))
                    .cloned()
                    .collect::<Vec<_>>();
                let lower_level_iter =
                    SstConcatIterator::create_and_seek_to_first(lower_level_ssts)?;

                let upper_level_ssts = task
                    .upper_level_sst_ids
                    .iter()
                    .filter_map(|id| snapshot.sstables.get(id))
                    .cloned()
                    .collect::<Vec<_>>();

                if let Some(upper_level) = task.upper_level {
                    let upper_level_iter =
                        SstConcatIterator::create_and_seek_to_first(upper_level_ssts)?;
                    let mut iter = TwoMergeIterator::create(upper_level_iter, lower_level_iter)?;
                    if compact_to_bottom_level {
                        // 如果是压缩到最底层,使用FusedIterator,保证每个key只出现一次
                        self.do_compact(&mut FusedIterator::new(iter))
                    } else {
                        self.do_compact(&mut iter)
                    }
                } else {
                    // L0 SSTable会重叠,使用MergeIterator
                    let mut l0_sst_iters = Vec::with_capacity(upper_level_ssts.len());
                    for sst in upper_level_ssts {
                        l0_sst_iters
                            .push(Box::new(SsTableIterator::create_and_seek_to_first(sst)?));
                    }
                    let l0_sst_iter = MergeIterator::create(l0_sst_iters);
                    self.do_compact(&mut TwoMergeIterator::create(
                        l0_sst_iter,
                        lower_level_iter,
                    )?)
                }
            }
            CompactionTask::Tiered(task) => {
                // Implementation for tiered compaction
                unimplemented!()
            }
        }
    }

    pub fn force_full_compaction(&self) -> Result<()> {
        let snapshot = self.state.read().clone();
        let task = CompactionTask::ForceFullCompaction {
            l0_sstables: snapshot.l0_sstables.clone(),
            l1_sstables: snapshot.levels.first().cloned().unwrap_or_default().1,
        };
        drop(snapshot);
        let new_ssts = self.compact(&task)?;

        let CompactionTask::ForceFullCompaction {
            l0_sstables,
            l1_sstables,
        } = task
        else {
            panic!("unexpected compaction task type")
        };

        let ssts_to_remove;
        {
            let _state_lock = self.state_lock.lock();
            let mut state = self.state.write();
            let mut snapshot = state.as_ref().clone();

            ssts_to_remove = l0_sstables
                .iter()
                .chain(l1_sstables.iter())
                .cloned()
                .collect::<HashSet<_>>();

            for sst_id in &ssts_to_remove {
                snapshot.sstables.remove(sst_id);
            }

            snapshot.l0_sstables.retain(|s| !ssts_to_remove.contains(s));

            let mut target_sst_ids = snapshot.levels[0].1.clone();
            target_sst_ids.retain(|s| !ssts_to_remove.contains(s));

            for sst in new_ssts {
                let id = sst.sst_id();
                target_sst_ids.push(id);
                snapshot.sstables.insert(id, sst);
            }
            snapshot.levels[0].1 = target_sst_ids;

            *state = Arc::new(snapshot);
        }

        for sst_id in ssts_to_remove {
            std::fs::remove_file(self.path_of_sst(sst_id))?;
        }

        Ok(())
    }

    fn trigger_compaction(&self) -> Result<()> {
        let snapshot = self.state.read().clone();
        let task = self
            .compaction_controller
            .generate_compaction_task(&snapshot);
        drop(snapshot);

        if task.is_none() {
            return Ok(());
        }

        let task = task.unwrap();
        let new_ssts = self.compact(&task)?;
        let output = new_ssts.iter().map(|sst| sst.sst_id()).collect::<Vec<_>>();
        let ssts_to_remove = {
            let _state_lock = self.state_lock.lock();
            let mut state = self.state.write();
            let (mut new_state, ssts_to_remove) = self
                .compaction_controller
                .apply_compaction_result(&state, &task, &output, false);
            for sst in new_ssts {
                new_state.sstables.insert(sst.sst_id(), sst.clone());
            }
            *state = Arc::new(new_state);
            ssts_to_remove
        };

        for sst_id in ssts_to_remove {
            std::fs::remove_file(self.path_of_sst(sst_id))?;
        }

        Ok(())
    }

    pub(crate) fn spawn_compaction_thread(
        self: &Arc<Self>,
        rx: crossbeam_channel::Receiver<()>,
    ) -> Result<Option<std::thread::JoinHandle<()>>> {
        if let CompactionOptions::Leveled(_)
        | CompactionOptions::Simple(_)
        | CompactionOptions::Tiered(_) = self.options.compaction_options
        {
            let this = self.clone();
            let handle = std::thread::spawn(move || {
                let ticker = crossbeam_channel::tick(Duration::from_millis(50));
                loop {
                    crossbeam_channel::select! {
                        recv(ticker) -> _ => if let Err(e) = this.trigger_compaction() {
                            eprintln!("compaction failed: {}", e);
                        },
                        recv(rx) -> _ => return
                    }
                }
            });
            return Ok(Some(handle));
        }
        Ok(None)
    }

    fn trigger_flush(&self) -> Result<()> {
        let mtable_size = self.state.read().imm_memtables.len() + 1;
        if mtable_size > self.options.num_memtable_limit {
            self.force_flush_next_imm_memtable()?;
        }
        Ok(())
    }

    pub(crate) fn spawn_flush_thread(
        self: &Arc<Self>,
        rx: crossbeam_channel::Receiver<()>,
    ) -> Result<Option<std::thread::JoinHandle<()>>> {
        let this = self.clone();
        let handle = std::thread::spawn(move || {
            let ticker = crossbeam_channel::tick(Duration::from_millis(50));
            loop {
                crossbeam_channel::select! {
                    recv(ticker) -> _ => if let Err(e) = this.trigger_flush() {
                        eprintln!("flush failed: {}", e);
                    },
                    recv(rx) -> _ => return
                }
            }
        });
        Ok(Some(handle))
    }
}
