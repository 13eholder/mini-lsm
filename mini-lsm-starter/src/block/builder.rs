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

use crate::key::{KeySlice, KeyVec};

use super::Block;

use bytes::BufMut;

/// Builds a block.
pub struct BlockBuilder {
    /// Offsets of each key-value entries.
    offsets: Vec<u16>,
    /// All serialized key-value pairs in the block.
    data: Vec<u8>,
    /// The expected block size.
    block_size: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockBuilder {
    /// Creates a new block builder.
    pub fn new(block_size: usize) -> Self {
        Self {
            offsets: Vec::new(),
            data: Vec::new(),
            block_size,
            first_key: KeyVec::new(),
        }
    }

    pub fn estimated_size(&self) -> usize {
        self.data.len()
            + self.offsets.len() * std::mem::size_of::<u16>()
            + std::mem::size_of::<u16>() // for the number of entries
    }

    /// Adds a key-value pair to the block. Returns false when the block is full.
    /// You may find the `bytes::BufMut` trait useful for manipulating binary data.
    #[must_use]
    pub fn add(&mut self, key: KeySlice, value: &[u8]) -> bool {
        // key_overlap len + rest_key len + value len + offset + len of (rest key) + len of value
        // an entry also need an offset (u16)
        let key_overlap_len = self.key_overlap_len(key);
        let rest_key_len = key.len() - key_overlap_len;
        let target_size = std::mem::size_of::<u16>() * 4 + rest_key_len + value.len();
        let rest_key = KeySlice::from_slice(&key.raw_ref()[key_overlap_len..]);

        if self.is_empty() {
            self.first_key = key.to_key_vec();
            self._add(key_overlap_len, key, value);
            return true;
        }

        if self.estimated_size() + target_size > self.block_size {
            return false;
        }

        self._add(key_overlap_len, rest_key, value);

        true
    }

    fn key_overlap_len(&self, key: KeySlice) -> usize {
        self.first_key
            .raw_ref()
            .iter()
            .zip(key.raw_ref())
            .take_while(|(a, b)| a == b)
            .count()
    }

    fn _add(&mut self, key_overlap_len: usize, rest_key: KeySlice, value: &[u8]) {
        self.offsets.push(self.data.len() as u16);

        self.data.put_u16(key_overlap_len as u16);
        self.data.put_u16(rest_key.len() as u16);
        self.data.extend_from_slice(rest_key.raw_ref());
        self.data.put_u16(value.len() as u16);
        self.data.extend_from_slice(value);
    }

    /// Check if there is no key-value pair in the block.
    pub fn is_empty(&self) -> bool {
        self.first_key.is_empty()
    }

    /// Finalize the block.
    pub fn build(self) -> Block {
        Block {
            data: self.data,
            offsets: self.offsets,
        }
    }
}
