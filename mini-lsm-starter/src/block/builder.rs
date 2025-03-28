#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use bytes::BufMut;

use crate::key::{KeySlice, KeyVec};

use super::Block;

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

    /// Adds a key-value pair to the block. Returns false when the block is full.
    #[must_use]
    pub fn add(&mut self, key: KeySlice, value: &[u8]) -> bool {
        let overlap_len = self.key_overlap_len(key);
        let rest_len = key.len() - overlap_len;
        // 编码后的长度必须小于等于block_size
        let target_size = self.data.len() /* kv */
            + rest_len /* key */
            + value.len() /* value */
            + size_of::<u16>() * 3 /* overlap_key_len + rest_key_len + value_len */
            + self.offsets.len() * 2 /* offsets */
            + size_of::<u16>() /* new_offset */
            + size_of::<u16>(); /* num_entries */
        if target_size > self.block_size && !self.first_key.is_empty() {
            return false;
        }
        if self.first_key.is_empty() {
            self.first_key.append(key.raw_ref());
        }
        self.offsets.push(self.data.len() as u16);
        self.data.put_u16(overlap_len as u16);
        self.data.put_u16(rest_len as u16);
        self.data.extend(&(key.raw_ref()[overlap_len..]));
        self.data.put_u16(value.len() as u16);
        self.data.extend(value);
        true
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

    fn key_overlap_len(&self, key: KeySlice) -> usize {
        self.first_key
            .as_key_slice()
            .raw_ref()
            .iter()
            .zip(key.raw_ref().iter())
            .take_while(|(a, b)| a == b)
            .count()
    }
}
