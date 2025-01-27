use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bytes::BufMut;

use super::{BlockMeta, FileObject, SsTable};
use crate::{block::BlockBuilder, key::KeySlice, lsm_storage::BlockCache};

/// Builds an SSTable from key-value pairs.
pub struct SsTableBuilder {
    builder: BlockBuilder,
    first_key: Vec<u8>,
    last_key: Vec<u8>,
    data: Vec<u8>,
    pub(crate) meta: Vec<BlockMeta>,
    block_size: usize,
}

impl SsTableBuilder {
    /// Create a builder based on target block size.
    pub fn new(block_size: usize) -> Self {
        // unimplemented!()
        Self {
            builder: BlockBuilder::new(block_size),
            first_key: Vec::new(),
            last_key: Vec::new(),
            data: Vec::new(),
            meta: Vec::new(),
            block_size,
        }
    }

    /// Adds a key-value pair to SSTable.
    ///
    /// Note: You should split a new block when the current block is full.(`std::mem::replace` may
    /// be helpful here)
    pub fn add(&mut self, key: KeySlice, value: &[u8]) {
        // unimplemented!()
        if !self.builder.add(key, value) {
            let builder = std::mem::replace(&mut self.builder, BlockBuilder::new(self.block_size));
            let block = builder.build();
            let (first_key, last_key) = block.key_range();
            self.meta.push(BlockMeta {
                offset: self.data.len(),
                first_key,
                last_key,
            });
            self.data.extend(block.encode());
            assert!(self.builder.add(key, value));
        }
    }

    /// Get the estimated size of the SSTable.
    ///
    /// Since the data blocks contain much more data than meta blocks, just return the size of data
    /// blocks here.
    pub fn estimated_size(&self) -> usize {
        // unimplemented!()
        self.data.len()
    }

    /// Builds the SSTable and writes it to the given path. Use the `FileObject` structure to manipulate the disk objects.
    pub fn build(
        self,
        id: usize,
        block_cache: Option<Arc<BlockCache>>,
        path: impl AsRef<Path>,
    ) -> Result<SsTable> {
        let mut data = self.data;
        let mut meta = self.meta;
        let builder = self.builder;
        // data
        if !builder.is_empty() {
            let block = builder.build();
            let (first_key, last_key) = block.key_range();
            meta.push(BlockMeta {
                offset: data.len(),
                first_key,
                last_key,
            });
            data.extend(block.encode());
        }
        // meta
        let meta_size: usize = meta.iter().map(|meta| meta.encode_size()).sum();
        let mut meta_data = Vec::with_capacity(meta_size);
        BlockMeta::encode_block_meta(&meta, &mut meta_data);
        let block_meta_offset = data.len();
        data.extend(meta_data);
        data.put_u32(block_meta_offset as u32);

        let file = FileObject::create(path.as_ref(), data)?;
        SsTable::open(id, block_cache, file)
    }

    #[cfg(test)]
    pub(crate) fn build_for_test(self, path: impl AsRef<Path>) -> Result<SsTable> {
        self.build(0, None, path)
    }
}
