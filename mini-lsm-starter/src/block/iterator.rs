#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use std::sync::Arc;

use bytes::Buf;

use crate::key::{KeySlice, KeyVec};

use super::{Block, U16_SIZE};

/// Iterates on a block.
pub struct BlockIterator {
    /// The internal `Block`, wrapped by an `Arc`
    block: Arc<Block>,
    /// The current key, empty represents the iterator is invalid
    key: KeyVec,
    /// the current value range in the block.data, corresponds to the current key
    value_range: (usize, usize),
    /// Current index of the key-value pair, should be in range of [0, num_of_elements)
    idx: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockIterator {
    fn new(block: Arc<Block>) -> Self {
        Self {
            block,
            key: KeyVec::new(),
            value_range: (0, 0),
            idx: 0,
            first_key: KeyVec::new(),
        }
    }

    /// Creates a block iterator and seek to the first entry.
    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        let mut iter = Self::new(block);
        iter.seek_to_first();
        iter
    }

    /// Creates a block iterator and seek to the first key that >= `key`.
    pub fn create_and_seek_to_key(block: Arc<Block>, key: KeySlice) -> Self {
        // unimplemented!()
        let mut iter = Self::new(block);
        iter.seek_to_key(key);
        iter
    }

    /// Returns the key of the current entry.
    pub fn key(&self) -> KeySlice {
        self.key.as_key_slice()
    }

    /// Returns the value of the current entry.
    pub fn value(&self) -> &[u8] {
        &self.block.data[self.value_range.0..self.value_range.1]
    }

    /// Returns true if the iterator is valid.
    /// Note: You may want to make use of `key`
    pub fn is_valid(&self) -> bool {
        !self.first_key.is_empty() && self.idx < self.block.num_entries()
    }

    /// Seeks to the first key in the block.
    pub fn seek_to_first(&mut self) {
        self.idx = 0;
        (self.key, self.value_range) = self.decode(self.idx);
        if self.first_key.is_empty() {
            self.first_key = self.key.clone();
        }
    }

    /// Move to the next key in the block.
    pub fn next(&mut self) {
        self.idx += 1;
        if self.is_valid() {
            (self.key, self.value_range) = self.decode(self.idx);
        }
    }

    fn decode(&self, idx: usize) -> (KeyVec, (usize, usize)) {
        let offset = self.block.offsets[idx] as usize;
        let mut data = &self.block.data[offset..];
        let key_len = data.get_u16() as usize;
        let mut key = KeyVec::new();
        key.append(&data[..key_len]);
        data.advance(key_len);
        let value_len = data.get_u16() as usize;
        /* data_begin + key_len(u16) + len(key) + value_len(u16) */
        let value_begin = offset + U16_SIZE + key_len + U16_SIZE;
        let value_range = (value_begin, value_begin + value_len);
        (key, value_range)
    }

    /// Seek to the first key that >= `key`.
    /// Note: You should assume the key-value pairs in the block are sorted when being added by
    /// callers.
    pub fn seek_to_key(&mut self, key: KeySlice) {
        // unimplemented!()
        // let mut left=0, right= self.block.num_entries();
        let (mut left, mut right) = (0, self.block.num_entries());

        while left < right {
            let mid = (left + right) / 2;
            let (cur_key, _) = self.decode(mid);
            if cur_key.as_key_slice() < key {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        if right < self.block.num_entries() {
            if self.first_key.is_empty() {
                self.seek_to_first();
            }
            (self.key, self.value_range) = self.decode(right);
        }
        self.idx = right;
    }
}
