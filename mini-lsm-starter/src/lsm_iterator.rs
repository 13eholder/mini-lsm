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
use nom::AsBytes;

use crate::{
    iterators::{
        StorageIterator, concat_iterator::SstConcatIterator, merge_iterator::MergeIterator,
        two_merge_iterator::TwoMergeIterator,
    },
    key::KeySlice,
    mem_table::MemTableIterator,
    table::SsTableIterator,
};
type MtableIter = MergeIterator<MemTableIterator>;
type L0SstIter = MergeIterator<SsTableIterator>;
type SstIter = SstConcatIterator;
/// Represents the internal type for an LSM iterator. This type will be changed across the course for multiple times.
type LsmIteratorInner = TwoMergeIterator<TwoMergeIterator<MtableIter, L0SstIter>, SstIter>;

pub struct LsmIterator {
    inner: LsmIteratorInner,
    end_bound: Bound<Bytes>,
}

impl LsmIterator {
    pub(crate) fn new(iter: LsmIteratorInner, end_bound: Bound<Bytes>) -> Result<Self> {
        Ok(Self {
            inner: iter,
            end_bound,
        })
    }
}

impl StorageIterator for LsmIterator {
    type KeyType<'a> = &'a [u8];

    fn is_valid(&self) -> bool {
        match &self.end_bound {
            Bound::Included(end) => {
                self.inner.is_valid() && self.inner.key() <= KeySlice::from_slice(end.as_bytes())
            }
            Bound::Excluded(end) => {
                self.inner.is_valid() && self.inner.key() < KeySlice::from_slice(end.as_bytes())
            }
            Bound::Unbounded => self.inner.is_valid(),
        }
    }

    fn key(&self) -> &[u8] {
        self.inner.key().raw_ref()
    }

    fn value(&self) -> &[u8] {
        self.inner.value()
    }

    fn next(&mut self) -> Result<()> {
        self.inner.next()
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
        let mut iter = Self {
            iter,
            has_errored: false,
        };
        if iter.is_valid() && iter.value().is_empty() {
            let _ = iter.next();
        }
        iter
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
