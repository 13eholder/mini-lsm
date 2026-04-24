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

use std::ops::Bound;

use anyhow::{Result, anyhow};
use bytes::Bytes;

use crate::{
    iterators::{
        StorageIterator, concat_iterator::SstConcatIterator, merge_iterator::MergeIterator,
        two_merge_iterator::TwoMergeIterator,
    },
    mem_table::MemTableIterator,
    table::SsTableIterator,
};
type MtableIter = MergeIterator<MemTableIterator>;
type L0SstIter = MergeIterator<SsTableIterator>;
type SstIter = MergeIterator<SstConcatIterator>;
type LsmIteratorInner = TwoMergeIterator<TwoMergeIterator<MtableIter, L0SstIter>, SstIter>;

pub struct LsmIterator {
    inner: LsmIteratorInner,
    end_bound: Bound<Bytes>,
    prev_key: Vec<u8>,
    is_valid: bool,
    read_ts: u64,
}

impl LsmIterator {
    pub(crate) fn new(
        iter: LsmIteratorInner,
        end_bound: Bound<Bytes>,
        read_ts: u64,
    ) -> Result<Self> {
        let mut iter = Self {
            is_valid: iter.is_valid(),
            inner: iter,
            end_bound,
            prev_key: Vec::new(),
            read_ts,
        };
        iter.move_to_valid()?;
        Ok(iter)
    }

    fn next_inner(&mut self) -> Result<()> {
        self.inner.next()?;
        if !self.inner.is_valid() {
            self.is_valid = false;
            return Ok(());
        }
        match self.end_bound.as_ref() {
            Bound::Unbounded => {}
            Bound::Included(key) => self.is_valid = self.inner.key().key_ref() <= key.as_ref(),
            Bound::Excluded(key) => self.is_valid = self.inner.key().key_ref() < key.as_ref(),
        }
        Ok(())
    }

    fn move_to_valid(&mut self) -> Result<()> {
        loop {
            match self.end_bound.as_ref() {
                Bound::Unbounded => {}
                Bound::Included(key)
                    if self.inner.is_valid() && self.inner.key().key_ref() > key.as_ref() =>
                {
                    self.is_valid = false;
                    return Ok(());
                }
                Bound::Excluded(key)
                    if self.inner.is_valid() && self.inner.key().key_ref() >= key.as_ref() =>
                {
                    self.is_valid = false;
                    return Ok(());
                }
                _ => {}
            }
            while self.inner.is_valid() && self.inner.key().key_ref() == self.prev_key {
                self.next_inner()?;
            }
            if !self.is_valid {
                return Ok(());
            }
            if !self.inner.is_valid() {
                self.is_valid = false;
                return Ok(());
            }
            self.prev_key.clear();
            self.prev_key.extend(self.inner.key().key_ref());

            while self.inner.is_valid()
                && self.inner.key().key_ref() == self.prev_key
                && self.inner.key().ts() > self.read_ts
            {
                self.next_inner()?;
            }
            if !self.is_valid {
                return Ok(());
            }
            if !self.inner.is_valid() {
                self.is_valid = false;
                return Ok(());
            }
            if self.inner.key().key_ref() != self.prev_key {
                continue;
            }
            if !self.inner.value().is_empty() {
                self.is_valid = true;
                return Ok(());
            }
        }
    }
}

impl StorageIterator for LsmIterator {
    type KeyType<'a> = &'a [u8];

    fn is_valid(&self) -> bool {
        self.is_valid
    }

    fn key(&self) -> &[u8] {
        self.inner.key().key_ref()
    }

    fn value(&self) -> &[u8] {
        self.inner.value()
    }

    fn next(&mut self) -> Result<()> {
        self.next_inner()?;
        self.move_to_valid()?;
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        self.inner.num_active_iterators()
    }
}

/// A wrapper around existing iterator, will prevent users from calling `next` when the iterator is
/// invalid. If an iterator is already invalid, `next` does not do anything. If `next` returns an error,
/// `is_valid` should return false, and `next` should always return an error.
pub struct FusedIterator<I: StorageIterator> {
    iter: I,
    has_errored: bool,
}

impl<I: StorageIterator> FusedIterator<I> {
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            has_errored: false,
        }
    }
}

impl<I: StorageIterator> StorageIterator for FusedIterator<I> {
    type KeyType<'a>
        = I::KeyType<'a>
    where
        Self: 'a;

    fn is_valid(&self) -> bool {
        !self.has_errored && self.iter.is_valid()
    }

    fn key(&self) -> Self::KeyType<'_> {
        self.iter.key()
    }

    fn value(&self) -> &[u8] {
        self.iter.value()
    }

    fn next(&mut self) -> Result<()> {
        if self.has_errored {
            return Err(anyhow!("Inner StorageIter Has Error"));
        }
        if !self.iter.is_valid() {
            return Ok(());
        }

        if let Err(e) = self.iter.next() {
            self.has_errored = true;
            return Err(e);
        }

        if self.iter.is_valid() && self.iter.value().is_empty() {
            return self.next();
        }

        Ok(())
    }
    fn num_active_iterators(&self) -> usize {
        self.iter.num_active_iterators()
    }
}
