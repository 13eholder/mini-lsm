mod builder;
mod iterator;

pub use builder::BlockBuilder;
use bytes::{Buf, BufMut, Bytes};
pub use iterator::BlockIterator;

use crate::key::KeyBytes;

pub(crate) const U16_SIZE: usize = size_of::<u16>();

/// A block is the smallest unit of read and caching in LSM tree. It is a collection of sorted key-value pairs.
pub struct Block {
    pub(crate) data: Vec<u8>,
    pub(crate) offsets: Vec<u16>,
}

impl Block {
    /// Encode the internal data to the data layout illustrated in the tutorial
    /// Note: You may want to recheck if any of the expected field is missing from your output
    pub fn encode(&self) -> Bytes {
        // unimplemented!()
        let num_entries = self.offsets.len();
        let mut buffer = self.data.clone();
        for offset in &self.offsets {
            buffer.put_u16(*offset);
        }
        buffer.put_u16(num_entries as u16);
        buffer.into()
    }

    /// Decode from the data layout, transform the input `data` to a single `Block`
    pub fn decode(data: &[u8]) -> Self {
        let num_entries = (&data[data.len() - U16_SIZE..]).get_u16() as usize;
        let data_end = data.len() - U16_SIZE - U16_SIZE * num_entries;
        let offsets = &data[data_end..data.len() - U16_SIZE];
        let data = &data[..data_end];
        Self {
            data: Vec::from(data),
            offsets: offsets.chunks(U16_SIZE).map(|mut x| x.get_u16()).collect(),
        }
    }
    /// num_entries
    pub(crate) fn num_entries(&self) -> usize {
        self.offsets.len()
    }

    pub(crate) fn key_range(&self) -> (KeyBytes, KeyBytes) {
        let get_key = |idx| {
            if let Some(off) = self.offsets.get(idx) {
                let mut key_begin = &self.data[*off as usize..];
                let key_len = key_begin.get_u16() as usize;
                return KeyBytes::from_bytes(Bytes::copy_from_slice(&key_begin[..key_len]));
            }
            KeyBytes::from_bytes(Bytes::new())
        };
        (get_key(0), get_key(self.offsets.len() - 1))
    }
}
