#![allow(dead_code)] // REMOVE THIS LINE after fully implementing this functionality

mod leveled;
mod simple_leveled;
mod tiered;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
pub use leveled::{LeveledCompactionController, LeveledCompactionOptions, LeveledCompactionTask};
use serde::{Deserialize, Serialize};
pub use simple_leveled::{
    SimpleLeveledCompactionController, SimpleLeveledCompactionOptions, SimpleLeveledCompactionTask,
};
pub use tiered::{TieredCompactionController, TieredCompactionOptions, TieredCompactionTask};

use crate::iterators::concat_iterator::SstConcatIterator;
use crate::iterators::merge_iterator::MergeIterator;
use crate::iterators::two_merge_iterator::TwoMergeIterator;
use crate::iterators::StorageIterator;
use crate::key::KeySlice;
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
    fn do_compact<I>(&self, iter: &mut I, flush_to_bottom: bool) -> Result<Vec<Arc<SsTable>>>
    where
        I: for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>,
    {
        let mut ssts = Vec::new();
        let mut sst_builder = None;

        while iter.is_valid() {
            if sst_builder.is_none() {
                sst_builder = Some(SsTableBuilder::new(self.options.block_size));
            }
            let builder = sst_builder.as_mut().unwrap();

            if flush_to_bottom {
                if !iter.value().is_empty() {
                    builder.add(iter.key(), iter.value());
                }
            } else {
                builder.add(iter.key(), iter.value());
            }

            iter.next()?;

            if builder.estimated_size() >= self.options.target_sst_size {
                let sst_id = self.next_sst_id();
                let builder = sst_builder.take().unwrap();
                let sst = Arc::new(builder.build(
                    sst_id,
                    Some(self.block_cache.clone()),
                    self.path_of_sst(sst_id),
                )?);
                ssts.push(sst);
            }
        }

        if let Some(builder) = sst_builder {
            let sst_id = self.next_sst_id();
            let sst = Arc::new(builder.build(
                sst_id,
                Some(self.block_cache.clone()),
                self.path_of_sst(sst_id),
            )?);
            ssts.push(sst);
        }
        Ok(ssts)
    }

    fn compact(&self, task: &CompactionTask) -> Result<Vec<Arc<SsTable>>> {
        match task {
            CompactionTask::ForceFullCompaction {
                l0_sstables,
                l1_sstables,
            } => {
                let (mut l0_ssts, mut l1_ssts) = (Vec::new(), Vec::new());
                {
                    let state = self.state.read();
                    for sst_id in l0_sstables {
                        l0_ssts.push(state.sstables[sst_id].clone());
                    }

                    for sst_id in l1_sstables {
                        l1_ssts.push(state.sstables[sst_id].clone());
                    }
                }
                let mut l0_iters = Vec::new();
                for sst in l0_ssts {
                    l0_iters.push(Box::new(SsTableIterator::create_and_seek_to_first(sst)?));
                }

                let mut merge_iter = TwoMergeIterator::create(
                    MergeIterator::create(l0_iters),
                    SstConcatIterator::create_and_seek_to_first(l1_ssts)?,
                )?;

                self.do_compact(&mut merge_iter, true)
            }
            CompactionTask::Simple(task) => {
                let (mut upper_ssts, mut lower_ssts) = (Vec::new(), Vec::new());
                {
                    let state = self.state.read();
                    for sst_id in task.upper_level_sst_ids.iter() {
                        upper_ssts.push(state.sstables[sst_id].clone());
                    }
                    for sst_id in task.lower_level_sst_ids.iter() {
                        lower_ssts.push(state.sstables[sst_id].clone());
                    }
                }
                if task.upper_level.is_none() {
                    let mut l0_iters = Vec::new();
                    for sst in upper_ssts {
                        l0_iters.push(Box::new(SsTableIterator::create_and_seek_to_first(sst)?));
                    }
                    let mut merge_iter = TwoMergeIterator::create(
                        MergeIterator::create(l0_iters),
                        SstConcatIterator::create_and_seek_to_first(lower_ssts)?,
                    )?;
                    self.do_compact(&mut merge_iter, task.is_lower_level_bottom_level)
                } else {
                    let upper_iter = SstConcatIterator::create_and_seek_to_first(upper_ssts)?;
                    let lower_iter = SstConcatIterator::create_and_seek_to_first(lower_ssts)?;
                    let mut two_merge_iter = TwoMergeIterator::create(upper_iter, lower_iter)?;
                    self.do_compact(&mut two_merge_iter, task.is_lower_level_bottom_level)
                }
            }
            CompactionTask::Tiered(task) => {
                // 没有 l0_sstables
                let snapshot = self.state.read().clone();
                let mut iters = Vec::new();
                for (_, sst_ids) in &task.tiers {
                    let ssts = sst_ids
                        .iter()
                        .map(|sst_id| snapshot.sstables[sst_id].clone())
                        .collect::<Vec<_>>();
                    iters.push(Box::new(SstConcatIterator::create_and_seek_to_first(ssts)?));
                }
                let mut merge_iter = MergeIterator::create(iters);
                self.do_compact(&mut merge_iter, task.bottom_tier_included)
            }
            _ => unimplemented!(),
        }
    }

    pub fn force_full_compaction(&self) -> Result<()> {
        let (l0_sstables, l1_sstables) = {
            let state = self.state.read();
            (state.l0_sstables.clone(), state.levels[0].1.clone())
        };
        let task = CompactionTask::ForceFullCompaction {
            l0_sstables: l0_sstables.clone(),
            l1_sstables: l1_sstables.clone(),
        };
        let sstables = self.compact(&task)?;
        {
            let _state_lock = self.state_lock.lock();
            let mut state = self.state.read().as_ref().clone();
            // 删除已经压缩的l0_ssts,重新设置l1_ssts
            state
                .l0_sstables
                .retain(|sst_id| !l0_sstables.contains(sst_id));
            state.levels[0].1 = sstables.iter().map(|s| s.sst_id()).collect();

            for sst_id in l0_sstables.iter().chain(l1_sstables.iter()) {
                state.sstables.remove(sst_id);
            }
            for sst in sstables {
                state.sstables.insert(sst.sst_id(), sst);
            }

            *self.state.write() = Arc::new(state); // update state;
        }
        for sst_id in l0_sstables.into_iter().chain(l1_sstables.into_iter()) {
            std::fs::remove_file(self.path_of_sst(sst_id))?;
        }

        Ok(())
    }

    fn trigger_compaction(&self) -> Result<()> {
        let snapshot = { self.state.read().as_ref().clone() };
        if let Some(task) = self
            .compaction_controller
            .generate_compaction_task(&snapshot)
        {
            let sstables = self.compact(&task)?;
            let output = sstables.iter().map(|s| s.sst_id()).collect::<Vec<_>>();

            let _state_lock = self.state_lock.lock();
            // 必须重新获取snapshot,后台线程压缩期间不会阻塞 imm_memtables的创建以及memtable的更改
            let snapshot = self.state.read().as_ref().clone();
            let (mut snapshot, files_to_remove) = self
                .compaction_controller
                .apply_compaction_result(&snapshot, &task, &output, false);
            // 修改 sstables
            for sst_id in files_to_remove.iter() {
                snapshot.sstables.remove(sst_id);
            }
            for sst in sstables {
                snapshot.sstables.insert(sst.sst_id(), sst);
            }

            *self.state.write() = Arc::new(snapshot);

            drop(_state_lock);

            for sst_id in files_to_remove {
                std::fs::remove_file(self.path_of_sst(sst_id))?;
            }
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
        if self.state.read().imm_memtables.len() >= self.options.num_memtable_limit {
            return self.force_flush_next_imm_memtable();
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
