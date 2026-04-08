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

use std::cmp::{self};
use std::collections::BinaryHeap;
use std::collections::binary_heap::PeekMut;

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
// 由调用者维护iters顺序
impl<I: StorageIterator> MergeIterator<I> {
    pub fn create(iters: Vec<Box<I>>) -> Self {
        let mut heap = BinaryHeap::new();
        for (i, iter) in iters.into_iter().filter(|iter| iter.is_valid()).enumerate() {
            heap.push(HeapWrapper(i, iter));
        }
        let current = heap.pop();
        Self {
            iters: heap,
            current,
        }
    }
}

impl<I: 'static + for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>> StorageIterator
    for MergeIterator<I>
{
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice<'_> {
        self.current.as_ref().unwrap().1.key()
    }

    fn value(&self) -> &[u8] {
        self.current.as_ref().unwrap().1.value()
    }

    fn is_valid(&self) -> bool {
        self.current.is_some()
    }
    // 如果next报错,认为整个迭代器失效,处理完二叉堆逻辑问题后,立刻返回错误
    fn next(&mut self) -> Result<()> {
        if let Some(mut current) = self.current.take() {
            while let Some(mut other) = self.iters.peek_mut() {
                if other.1.key() != current.1.key() {
                    break;
                }

                match other.1.next() {
                    Err(e) => {
                        PeekMut::pop(other);
                        return Err(e);
                    }
                    Ok(_) if !other.1.is_valid() => {
                        PeekMut::pop(other);
                        continue;
                    }
                    _ => continue,
                }
            }

            match current.1.next() {
                Err(e) => return Err(e),
                Ok(_) if current.1.is_valid() => {
                    self.iters.push(current);
                    self.current = self.iters.pop();
                    return Ok(());
                }
                _ => {
                    self.current = self.iters.pop();
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        if self.is_valid() {
            self.iters
                .iter()
                .map(|it| it.1.num_active_iterators())
                .sum::<usize>()
                + 1
        } else {
            0
        }
    }
}
