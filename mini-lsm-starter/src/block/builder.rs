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
    #[must_use]
    pub fn add(&mut self, key: KeySlice, value: &[u8]) -> bool {
        let key_overlap_len = self.key_overlap_len(key);
        let rest_key_len = key.key_len() - key_overlap_len;
        // overlap_len(u16) + rest_key_len(u16) + rest_key + ts(u64) + value_len(u16) + value + offset(u16)
        let target_size = std::mem::size_of::<u16>() * 3
            + std::mem::size_of::<u64>()
            + rest_key_len
            + value.len();

        if !self.is_empty() && self.estimated_size() + target_size > self.block_size {
            return false;
        }

        let rest_key_slice = &key.key_ref()[key_overlap_len..];
        self.offsets.push(self.data.len() as u16);

        self.data.put_u16(key_overlap_len as u16);
        self.data.put_u16(rest_key_len as u16);
        self.data.extend_from_slice(rest_key_slice);
        self.data.put_u64(key.ts());
        self.data.put_u16(value.len() as u16);
        self.data.extend_from_slice(value);

        if self.first_key.is_empty() {
            self.first_key = key.to_key_vec();
        }

        true
    }

    fn key_overlap_len(&self, key: KeySlice) -> usize {
        self.first_key
            .key_ref()
            .iter()
            .zip(key.key_ref())
            .take_while(|(a, b)| a == b)
            .count()
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
