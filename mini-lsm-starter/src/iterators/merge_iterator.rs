#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use std::cmp::{self};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use anyhow::Result;

use crate::key::KeySlice;

use super::StorageIterator;

struct HeapWrapper<I: StorageIterator>(pub usize, pub Box<I>);

impl<I: StorageIterator> PartialEq for HeapWrapper<I> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

impl<I: StorageIterator> Eq for HeapWrapper<I> {}

impl<I: StorageIterator> PartialOrd for HeapWrapper<I> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<I: StorageIterator> Ord for HeapWrapper<I> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.1
            .key()
            .cmp(&other.1.key())
            .then(self.0.cmp(&other.0))
            .reverse()
    }
}

/// Merge multiple iterators of the same type. If the same key occurs multiple times in some
/// iterators, prefer the one with smaller index.
pub struct MergeIterator<I: StorageIterator> {
    iters: BinaryHeap<HeapWrapper<I>>,
    current: Option<HeapWrapper<I>>,
}

impl<I: StorageIterator> MergeIterator<I> {
    pub fn create(iters: Vec<Box<I>>) -> Self {
        let mut iters = BinaryHeap::from_iter(
            iters
                .into_iter()
                .filter(|iter| iter.is_valid())
                .enumerate()
                .map(|(idx, item)| HeapWrapper(idx, item)),
        );
        let current = iters.pop();
        Self { iters, current }
    }
}

impl<I: 'static + for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>> StorageIterator
    for MergeIterator<I>
{
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice {
        self.current.as_ref().unwrap().1.key()
    }

    fn value(&self) -> &[u8] {
        self.current.as_ref().unwrap().1.value()
    }

    fn is_valid(&self) -> bool {
        if self.current.is_none() {
            return false;
        }
        self.current.as_ref().unwrap().1.is_valid()
    }

    fn next(&mut self) -> Result<()> {
        let current = self.current.as_mut().unwrap();
        while let Some(mut iter) = self.iters.peek_mut() {
            if iter.1.key() != current.1.key() {
                break;
            }
            if let err @ Err(_) = iter.1.next() {
                PeekMut::pop(iter);
                return err;
            }
            if !iter.1.is_valid() {
                PeekMut::pop(iter);
            }
        }
        current.1.next()?;

        if !current.1.is_valid() {
            if let Some(iter) = self.iters.pop() {
                *current = iter;
            }
            return Ok(());
        }

        if let Some(mut iter) = self.iters.peek_mut() {
            if *current < *iter {
                std::mem::swap(current, &mut *iter);
            }
        }

        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        let mut num = 0;
        if let Some(ref cur) = self.current {
            num += cur.1.num_active_iterators();
        }
        self.iters
            .iter()
            .for_each(|iter| num += iter.1.num_active_iterators());
        num
    }
}
