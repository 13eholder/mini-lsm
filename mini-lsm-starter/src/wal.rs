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

use anyhow::{Result, bail};
use bytes::{Buf, BufMut, Bytes};
use crossbeam_skiplist::SkipMap;
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;

use crate::key::{KeyBytes, KeySlice};

pub struct Wal {
    file: Arc<Mutex<BufWriter<File>>>,
}

impl Wal {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Wal {
            file: Arc::new(Mutex::new(BufWriter::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(path)?,
            ))),
        })
    }

    pub fn recover(path: impl AsRef<Path>, skiplist: &SkipMap<KeyBytes, Bytes>) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).append(true).open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let mut buf = &buf[..];

        while !buf.is_empty() {
            let batch_size = buf.get_u32() as usize;
            let mut batch_buf = &buf[..batch_size];
            let checksum = crc32fast::hash(batch_buf);
            let expected_checksum = (&buf[batch_size..]).get_u32();
            if checksum != expected_checksum {
                bail!("checksum mismatch");
            }
            while !batch_buf.is_empty() {
                let key_len = batch_buf.get_u16() as usize;
                let key = batch_buf.copy_to_bytes(key_len);
                let ts = batch_buf.get_u64();
                let value_len = batch_buf.get_u16() as usize;
                let value = batch_buf.copy_to_bytes(value_len);
                skiplist.insert(KeyBytes::from_bytes_with_ts(key, ts), value);
            }
            buf.advance(batch_size + std::mem::size_of::<u32>());
        }

        Ok(Wal {
            file: Arc::new(Mutex::new(BufWriter::new(file))),
        })
    }

    pub fn put(&self, key: KeySlice, value: &[u8]) -> Result<()> {
        self.put_batch(&[(key, value)])
    }

    pub fn put_batch(&self, data: &[(KeySlice, &[u8])]) -> Result<()> {
        let mut file = self.file.lock();
        let mut buf = Vec::new();
        for (key, value) in data {
            buf.put_u16(key.key_len() as u16);
            buf.put_slice(key.key_ref());
            buf.put_u64(key.ts());
            buf.put_u16(value.len() as u16);
            buf.put_slice(value);
        }
        file.write_all(&(buf.len() as u32).to_be_bytes())?;
        file.write_all(&buf)?;
        file.write_all(&crc32fast::hash(&buf).to_be_bytes())?;
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        let mut file = self.file.lock();
        file.flush()?;
        file.get_ref().sync_all()?;
        Ok(())
    }
}
