#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

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
        if sstables.is_empty() {
            return Ok(SstConcatIterator {
                current: None,
                next_sst_idx: 0,
                sstables,
            });
        }
        Ok(Self {
            current: Some(SsTableIterator::create_and_seek_to_first(
                sstables[0].clone(),
            )?),
            next_sst_idx: 1,
            sstables,
        })
    }

    pub fn create_and_seek_to_key(sstables: Vec<Arc<SsTable>>, key: KeySlice) -> Result<Self> {
        let mut idx = 0;
        while idx < sstables.len() {
            let iter = SsTableIterator::create_and_seek_to_key(sstables[idx].clone(), key)?;
            if iter.is_valid() {
                break;
            }
            idx += 1;
        }
        if idx < sstables.len() {
            Ok(Self {
                current: Some(SsTableIterator::create_and_seek_to_key(
                    sstables[idx].clone(),
                    key,
                )?),
                next_sst_idx: idx + 1,
                sstables,
            })
        } else {
            Ok(Self {
                current: None,
                next_sst_idx: idx + 1,
                sstables,
            })
        }
    }
}

impl StorageIterator for SstConcatIterator {
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice {
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
                self.current.take();
            }
        }
        if self.current.is_none() && self.next_sst_idx < self.sstables.len() {
            self.current = Some(SsTableIterator::create_and_seek_to_first(
                self.sstables[self.next_sst_idx].clone(),
            )?);
            self.next_sst_idx += 1;
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        1
    }
}
