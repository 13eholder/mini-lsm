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

use std::sync::Arc;

use anyhow::Result;

use super::StorageIterator;
use crate::{
    key::KeySlice,
    table::{SsTable, SsTableIterator},
};

/// Concat multiple iterators ordered in key order and their key ranges do not overlap. We do not want to create the
/// iterators when initializing this iterator to reduce the overhead of seeking.
pub struct SstConcatIterator {
    current: Option<SsTableIterator>,
    next_sst_idx: usize,
    sstables: Vec<Arc<SsTable>>,
}

impl SstConcatIterator {
    pub fn create_and_seek_to_first(sstables: Vec<Arc<SsTable>>) -> Result<Self> {
        let (current, next_sst_idx) = if sstables.is_empty() {
            (None, 0)
        } else {
            (
                Some(SsTableIterator::create_and_seek_to_first(
                    sstables[0].clone(),
                )?),
                1,
            )
        };
        Ok(Self {
            current,
            next_sst_idx,
            sstables,
        })
    }

    pub fn create_and_seek_to_key(sstables: Vec<Arc<SsTable>>, key: KeySlice) -> Result<Self> {
        if sstables.is_empty() {
            return Ok(Self {
                current: None,
                next_sst_idx: 0,
                sstables,
            });
        }

        let idx = sstables
            .partition_point(|sst| sst.first_key().as_key_slice() <= key)
            .saturating_sub(1);

        let (current, next_sst_idx) =
            match SsTableIterator::create_and_seek_to_key(sstables[idx].clone(), key) {
                Ok(iter) => (Some(iter), idx + 1),
                Err(_) => {
                    let next_idx = idx + 1;
                    if next_idx < sstables.len() {
                        (
                            Some(SsTableIterator::create_and_seek_to_first(
                                sstables[next_idx].clone(),
                            )?),
                            next_idx + 1,
                        )
                    } else {
                        (None, sstables.len())
                    }
                }
            };

        Ok(Self {
            current,
            next_sst_idx,
            sstables,
        })
    }
}

impl StorageIterator for SstConcatIterator {
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice<'_> {
        self.current.as_ref().unwrap().key()
    }

    fn value(&self) -> &[u8] {
        self.current.as_ref().unwrap().value()
    }

    fn is_valid(&self) -> bool {
        self.current.is_some()
    }

    fn next(&mut self) -> Result<()> {
        if let Some(ref mut iter) = self.current {
            iter.next()?;
            if !iter.is_valid() {
                if self.next_sst_idx == self.sstables.len() {
                    self.current = None;
                } else {
                    self.current = Some(SsTableIterator::create_and_seek_to_first(
                        self.sstables[self.next_sst_idx].clone(),
                    )?);
                    self.next_sst_idx += 1;
                }
            }
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        if self.is_valid() { 1 } else { 0 }
    }
}
