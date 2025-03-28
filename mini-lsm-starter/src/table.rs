#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

pub(crate) mod bloom;
mod builder;
mod iterator;

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
pub use builder::SsTableBuilder;
use bytes::{Buf, BufMut, Bytes};
pub use iterator::SsTableIterator;
use nom::AsBytes;

use crate::block::Block;
use crate::key::{KeyBytes, KeySlice};
use crate::lsm_storage::BlockCache;

use self::bloom::Bloom;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMeta {
    /// Offset of this data block.
    pub offset: usize,
    /// The first key of the data block.
    pub first_key: KeyBytes,
    /// The last key of the data block.
    pub last_key: KeyBytes,
}

impl BlockMeta {
    /// Encode block meta to a buffer.
    /// You may add extra fields to the buffer,
    /// in order to help keep track of `first_key` when decoding from the same buffer in the future.
    pub fn encode_block_meta(block_meta: &[BlockMeta], buf: &mut Vec<u8>) {
        // unimplemented!()
        for meta in block_meta {
            buf.put_u64(meta.offset as u64);
            buf.put_u16(meta.first_key.len() as u16);
            buf.put_slice(meta.first_key.raw_ref());
            buf.put_u16(meta.last_key.len() as u16);
            buf.put_slice(meta.last_key.raw_ref());
        }
        // buf.put_u64(block_meta.len() as u64);
    }

    /// Decode block meta from a buffer.
    pub fn decode_block_meta(buf: impl Buf) -> Vec<BlockMeta> {
        // unimplemented!()
        let mut metas = Vec::new();
        let mut reader = buf.reader();
        let buf_reader = reader.get_mut();
        while buf_reader.has_remaining() {
            let offset = buf_reader.get_u64() as usize;
            let first_key_len = buf_reader.get_u16() as usize;
            let first_key = KeyBytes::from_bytes(buf_reader.copy_to_bytes(first_key_len));
            let last_key_len = buf_reader.get_u16() as usize;
            let last_key = KeyBytes::from_bytes(buf_reader.copy_to_bytes(last_key_len));
            metas.push(BlockMeta {
                offset,
                first_key,
                last_key,
            });
        }
        metas
    }
    pub fn encode_size(&self) -> usize {
        size_of::<u64>() + size_of::<u16>() * 2 + self.first_key.len() + self.last_key.len()
    }
}

/// A file object.
pub struct FileObject(Option<File>, u64);

impl FileObject {
    pub fn read(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        use std::os::unix::fs::FileExt;
        let mut data = vec![0; len as usize];
        self.0
            .as_ref()
            .unwrap()
            .read_exact_at(&mut data[..], offset)?;
        Ok(data)
    }

    pub fn size(&self) -> u64 {
        self.1
    }

    /// Create a new file object (day 2) and write the file to the disk (day 4).
    pub fn create(path: &Path, data: Vec<u8>) -> Result<Self> {
        std::fs::write(path, &data)?;
        File::open(path)?.sync_all()?;
        Ok(FileObject(
            Some(File::options().read(true).write(false).open(path)?),
            data.len() as u64,
        ))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let file = File::options().read(true).write(false).open(path)?;
        let size = file.metadata()?.len();
        Ok(FileObject(Some(file), size))
    }
}

/// An SSTable.
pub struct SsTable {
    /// The actual storage unit of SsTable, the format is as above.
    pub(crate) file: FileObject,
    /// The meta blocks that hold info for data blocks.
    pub(crate) block_meta: Vec<BlockMeta>,
    /// The offset that indicates the start point of meta blocks in `file`.
    pub(crate) block_meta_offset: usize,
    id: usize,
    block_cache: Option<Arc<BlockCache>>,
    first_key: KeyBytes,
    last_key: KeyBytes,
    pub(crate) bloom: Option<Bloom>,
    /// The maximum timestamp stored in this SST, implemented in week 3.
    max_ts: u64,
}

impl SsTable {
    #[cfg(test)]
    pub(crate) fn open_for_test(file: FileObject) -> Result<Self> {
        Self::open(0, None, file)
    }

    /// Open SSTable from a file.
    pub fn open(id: usize, block_cache: Option<Arc<BlockCache>>, file: FileObject) -> Result<Self> {
        let file_len = file.1;
        let u32_size = size_of::<u32>() as u64;
        // bloom filter
        let bloom_filter_end = file_len - u32_size;
        let bloom_filter_offset = (&file.read(bloom_filter_end, u32_size)?[..]).get_u32() as u64;
        let bloom_filter = Bloom::decode(
            file.read(bloom_filter_offset, bloom_filter_end - bloom_filter_offset)?
                .as_bytes(),
        )?;
        // block meta
        let block_meta_end = bloom_filter_offset - u32_size;
        let block_meta_offset = (&file.read(block_meta_end, u32_size)?[..]).get_u32() as u64;
        let block_meta = BlockMeta::decode_block_meta(
            file.read(block_meta_offset, block_meta_end - block_meta_offset)?
                .as_bytes(),
        );
        let mut sst = Self {
            file,
            block_meta,
            block_meta_offset: block_meta_offset as usize,
            id,
            block_cache,
            first_key: KeyBytes::from_bytes(Bytes::new()),
            last_key: KeyBytes::from_bytes(Bytes::new()),
            bloom: Some(bloom_filter),
            max_ts: 0,
        };
        if let Some(first) = sst.block_meta.first() {
            sst.first_key = first.first_key.clone();
        }
        if let Some(last) = sst.block_meta.last() {
            sst.last_key = last.last_key.clone();
        }
        Ok(sst)
    }

    /// Create a mock SST with only first key + last key metadata
    pub fn create_meta_only(
        id: usize,
        file_size: u64,
        first_key: KeyBytes,
        last_key: KeyBytes,
    ) -> Self {
        Self {
            file: FileObject(None, file_size),
            block_meta: vec![],
            block_meta_offset: 0,
            id,
            block_cache: None,
            first_key,
            last_key,
            bloom: None,
            max_ts: 0,
        }
    }

    /// Read a block from the disk.
    pub fn read_block(&self, block_idx: usize) -> Result<Arc<Block>> {
        if block_idx >= self.block_meta.len() {
            bail!(
                "block idx {} out of range [0,{})",
                block_idx,
                self.block_meta.len()
            );
        }
        let block_begin = self.block_meta[block_idx].offset;
        let block_end = if block_idx + 1 < self.block_meta.len() {
            self.block_meta[block_idx + 1].offset
        } else {
            self.block_meta_offset
        };
        let data = self
            .file
            .read(block_begin as u64, (block_end - block_begin) as u64)?;
        Ok(Arc::new(Block::decode(data.as_slice())))
    }

    /// Read a block from disk, with block cache. (Day 4)
    pub fn read_block_cached(&self, block_idx: usize) -> Result<Arc<Block>> {
        if let Some(ref cache) = self.block_cache {
            return cache
                .try_get_with((self.sst_id(), block_idx), || self.read_block(block_idx))
                .map_err(|err| anyhow!("{}", err.to_string()));
        }
        self.read_block(block_idx)
    }

    /// Find the block that may contain `key`.
    /// Note: You may want to make use of the `first_key` stored in `BlockMeta`.
    /// You may also assume the key-value pairs stored in each consecutive block are sorted.
    pub fn find_block_idx(&self, key: KeySlice) -> usize {
        // unimplemented!()
        let (mut left, mut right) = (0usize, self.block_meta.len());
        while left < right {
            let mid = (left + right) / 2;
            if self.block_meta[mid].last_key.as_key_slice() < key {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        if right == self.num_of_blocks() {
            right -= 1;
        }
        right
    }

    /// Get number of data blocks.
    pub fn num_of_blocks(&self) -> usize {
        self.block_meta.len()
    }

    pub fn first_key(&self) -> &KeyBytes {
        &self.first_key
    }

    pub fn last_key(&self) -> &KeyBytes {
        &self.last_key
    }

    pub fn table_size(&self) -> u64 {
        self.file.1
    }

    pub fn sst_id(&self) -> usize {
        self.id
    }

    pub fn max_ts(&self) -> u64 {
        self.max_ts
    }
}
