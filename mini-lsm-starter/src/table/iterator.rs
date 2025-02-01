use std::sync::Arc;

use anyhow::{bail, Result};
use bytes::Bytes;

use super::SsTable;
use crate::{
    block::BlockIterator,
    iterators::StorageIterator,
    key::{KeyBytes, KeySlice},
};

/// An iterator over the contents of an SSTable.
pub struct SsTableIterator {
    table: Arc<SsTable>,
    blk_iter: BlockIterator,
    blk_idx: usize,
}

impl SsTableIterator {
    /// Create a new iterator and seek to the first key-value pair in the first data block.
    pub fn create_and_seek_to_first(table: Arc<SsTable>) -> Result<Self> {
        // unimplemented!()
        let blk_idx = 0;
        let blk_iter = BlockIterator::create_and_seek_to_first(table.read_block_cached(blk_idx)?);
        Ok(Self {
            table,
            blk_iter,
            blk_idx,
        })
    }

    /// Seek to the first key-value pair in the first data block.
    pub fn seek_to_first(&mut self) -> Result<()> {
        // unimplemented!()
        if self.blk_idx == 0 {
            self.blk_iter.seek_to_first();
        } else {
            self.blk_idx = 0;
            self.blk_iter = BlockIterator::create_and_seek_to_first(
                self.table.read_block_cached(self.blk_idx)?,
            );
        }
        Ok(())
    }

    /// Create a new iterator and seek to the first key-value pair which >= `key`.
    pub fn create_and_seek_to_key(table: Arc<SsTable>, key: KeySlice) -> Result<Self> {
        // unimplemented!()
        let blk_idx = table.find_block_idx(key);
        if blk_idx == table.block_meta.len() {
            bail!("No such key in sst");
        }
        let blk_iter =
            BlockIterator::create_and_seek_to_key(table.read_block_cached(blk_idx)?, key);
        Ok(Self {
            table,
            blk_iter,
            blk_idx,
        })
    }

    /// Seek to the first key-value pair which >= `key`.
    /// Note: You probably want to review the handout for detailed explanation when implementing
    /// this function.
    pub fn seek_to_key(&mut self, key: KeySlice) -> Result<()> {
        // unimplemented!()
        let blk_idx = self.table.find_block_idx(key);
        if blk_idx == self.table.num_of_blocks() {
            bail!(
                "No such key in sst : {:#?}",
                KeyBytes::from_bytes(Bytes::copy_from_slice(key.raw_ref()))
            );
        }
        if self.blk_idx == blk_idx {
            self.blk_iter.seek_to_key(key);
        } else {
            self.blk_idx = blk_idx;
            self.blk_iter =
                BlockIterator::create_and_seek_to_key(self.table.read_block_cached(blk_idx)?, key);
        }

        Ok(())
    }
}

impl StorageIterator for SsTableIterator {
    type KeyType<'a> = KeySlice<'a>;

    /// Return the `key` that's held by the underlying block iterator.
    fn key(&self) -> KeySlice {
        // unimplemented!()
        self.blk_iter.key()
    }

    /// Return the `value` that's held by the underlying block iterator.
    fn value(&self) -> &[u8] {
        // unimplemented!()
        self.blk_iter.value()
    }

    /// Return whether the current block iterator is valid or not.
    fn is_valid(&self) -> bool {
        // unimplemented!()
        self.blk_iter.is_valid()
    }

    /// Move to the next `key` in the block.
    /// Note: You may want to check if the current block iterator is valid after the move.
    fn next(&mut self) -> Result<()> {
        // unimplemented!()
        self.blk_iter.next();
        if !self.blk_iter.is_valid() && self.blk_idx + 1 < self.table.num_of_blocks() {
            self.blk_idx += 1;
            self.blk_iter = BlockIterator::create_and_seek_to_first(
                self.table.read_block_cached(self.blk_idx)?,
            );
        }
        Ok(())
    }
}
