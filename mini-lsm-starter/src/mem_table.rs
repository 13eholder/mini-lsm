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

use std::ops::Bound;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use anyhow::Result;
use bytes::Bytes;
use crossbeam_skiplist::SkipMap;
use crossbeam_skiplist::map::Entry;
use ouroboros::self_referencing;

use crate::iterators::StorageIterator;
use crate::key::{KeyBytes, KeySlice, TS_DEFAULT, TS_RANGE_BEGIN, TS_RANGE_END};
use crate::table::SsTableBuilder;
use crate::wal::Wal;

/// A basic mem-table based on crossbeam-skiplist.
pub struct MemTable {
    map: Arc<SkipMap<KeyBytes, Bytes>>,
    wal: Option<Wal>,
    id: usize,
    approximate_size: Arc<AtomicUsize>,
}

/// Create a bound of `Bytes` from a bound of `&[u8]`.
pub(crate) fn map_bound(bound: Bound<&[u8]>) -> Bound<Bytes> {
    match bound {
        Bound::Included(x) => Bound::Included(Bytes::copy_from_slice(x)),
        Bound::Excluded(x) => Bound::Excluded(Bytes::copy_from_slice(x)),
        Bound::Unbounded => Bound::Unbounded,
    }
}

/// Create a bound of `KeyBytes` from a bound of `KeySlice`.
pub(crate) fn map_key_bound(bound: Bound<KeySlice>) -> Bound<KeyBytes> {
    match bound {
        Bound::Included(x) => Bound::Included(x.to_key_vec().into_key_bytes()),
        Bound::Excluded(x) => Bound::Excluded(x.to_key_vec().into_key_bytes()),
        Bound::Unbounded => Bound::Unbounded,
    }
}

/// Create range bounds over MVCC keys from user key bounds.
pub(crate) fn map_key_bound_plus_ts<'a>(
    lower: Bound<&'a [u8]>,
    upper: Bound<&'a [u8]>,
    read_ts: u64,
) -> (Bound<KeySlice<'a>>, Bound<KeySlice<'a>>) {
    (
        match lower {
            Bound::Included(key) => Bound::Included(KeySlice::from_slice(key, read_ts)),
            Bound::Excluded(key) => Bound::Excluded(KeySlice::from_slice(key, TS_RANGE_END)),
            Bound::Unbounded => Bound::Unbounded,
        },
        match upper {
            Bound::Included(key) => Bound::Included(KeySlice::from_slice(key, TS_RANGE_END)),
            Bound::Excluded(key) => Bound::Excluded(KeySlice::from_slice(key, TS_RANGE_BEGIN)),
            Bound::Unbounded => Bound::Unbounded,
        },
    )
}

impl MemTable {
    /// Create a new mem-table.
    pub fn create(id: usize) -> Self {
        Self {
            map: Arc::new(SkipMap::new()),
            wal: None,
            id,
            approximate_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a new mem-table with WAL
    pub fn create_with_wal(id: usize, path: impl AsRef<Path>) -> Result<Self> {
        let wal = Wal::create(&path)?;
        Ok(Self {
            map: Arc::new(SkipMap::new()),
            wal: Some(wal),
            id,
            approximate_size: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Create a memtable from WAL
    pub fn recover_from_wal(id: usize, path: impl AsRef<Path>) -> Result<Self> {
        let map = Arc::new(SkipMap::new());
        let wal = Wal::recover(&path, &map)?;
        Ok(Self {
            map,
            wal: Some(wal),
            id,
            approximate_size: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn for_testing_put_slice(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.put(KeySlice::from_slice(key, TS_DEFAULT), value)
    }

    pub fn for_testing_get_slice(&self, key: &[u8]) -> Option<Bytes> {
        self.get(key)
    }

    pub fn for_testing_scan_slice(
        &self,
        lower: Bound<&[u8]>,
        upper: Bound<&[u8]>,
    ) -> MemTableIterator {
        let lower = match lower {
            Bound::Included(key) => Bound::Included(KeyBytes::from_bytes_with_ts(
                Bytes::copy_from_slice(key),
                TS_DEFAULT,
            )),
            Bound::Excluded(key) => Bound::Excluded(KeyBytes::from_bytes_with_ts(
                Bytes::copy_from_slice(key),
                TS_DEFAULT,
            )),
            Bound::Unbounded => Bound::Unbounded,
        };
        let upper = match upper {
            Bound::Included(key) => Bound::Included(KeyBytes::from_bytes_with_ts(
                Bytes::copy_from_slice(key),
                TS_DEFAULT,
            )),
            Bound::Excluded(key) => Bound::Excluded(KeyBytes::from_bytes_with_ts(
                Bytes::copy_from_slice(key),
                TS_DEFAULT,
            )),
            Bound::Unbounded => Bound::Unbounded,
        };
        self.scan(lower, upper)
    }

    /// Get a value by key.
    pub fn get(&self, key: &[u8]) -> Option<Bytes> {
        self.map
            .get(&KeyBytes::from_bytes_with_ts(
                Bytes::from_static(unsafe { std::mem::transmute::<&[u8], &'static [u8]>(key) }),
                TS_DEFAULT,
            ))
            .map(|entry| entry.value().clone())
    }

    /// Get the latest version visible at `read_ts` for `key` in this memtable.
    /// Returns `Some(empty_bytes)` for a visible tombstone, and `None` if this memtable
    /// does not contain any visible version for the key.
    pub fn get_with_ts(&self, key: &[u8], read_ts: u64) -> Option<Bytes> {
        let lower = Bound::Included(KeyBytes::from_bytes_with_ts(
            Bytes::copy_from_slice(key),
            TS_RANGE_BEGIN,
        ));
        let upper = Bound::Included(KeyBytes::from_bytes_with_ts(
            Bytes::copy_from_slice(key),
            TS_RANGE_END,
        ));

        for entry in self.map.range((lower, upper)) {
            if entry.key().key_ref() != key {
                break;
            }
            if entry.key().ts() <= read_ts {
                return Some(entry.value().clone());
            }
        }
        None
    }

    /// Put a key-value pair into the mem-table.
    pub fn put(&self, key: KeySlice, value: &[u8]) -> Result<()> {
        self.put_batch(&[(key, value)])
    }

    pub fn put_batch(&self, data: &[(KeySlice, &[u8])]) -> Result<()> {
        let mut estimated_size = 0;
        for (key, value) in data {
            estimated_size += key.raw_len() + value.len();
            self.map.insert(
                key.to_key_vec().into_key_bytes(),
                Bytes::copy_from_slice(value),
            );
        }
        self.approximate_size
            .fetch_add(estimated_size, std::sync::atomic::Ordering::AcqRel);
        if let Some(ref wal) = self.wal {
            wal.put_batch(data)?;
        }
        Ok(())
    }

    pub fn sync_wal(&self) -> Result<()> {
        if let Some(ref wal) = self.wal {
            wal.sync()?;
        }
        Ok(())
    }

    /// Get an iterator over a range of keys.
    pub fn scan(&self, lower: Bound<KeyBytes>, upper: Bound<KeyBytes>) -> MemTableIterator {
        let mut iter = MemTableIteratorBuilder {
            map: self.map.clone(),
            iter_builder: |map| map.range((lower, upper)),
            item: (KeyBytes::new(), Bytes::new()),
        }
        .build();
        iter.next().unwrap();
        iter
    }

    /// Flush the mem-table to SSTable.
    pub fn flush(&self, builder: &mut SsTableBuilder) -> Result<()> {
        for entry in self.map.iter() {
            builder.add(entry.key().as_key_slice(), entry.value().as_ref());
        }
        Ok(())
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn approximate_size(&self) -> usize {
        self.approximate_size
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Only use this function when closing the database
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn max_ts(&self) -> u64 {
        self.map
            .iter()
            .map(|entry| entry.key().ts())
            .max()
            .unwrap_or(0)
    }
}

type SkipMapRangeIter<'a> = crossbeam_skiplist::map::Range<
    'a,
    KeyBytes,
    (Bound<KeyBytes>, Bound<KeyBytes>),
    KeyBytes,
    Bytes,
>;

/// An iterator over a range of `SkipMap`.
#[self_referencing]
pub struct MemTableIterator {
    /// Stores a reference to the skipmap.
    map: Arc<SkipMap<KeyBytes, Bytes>>,
    /// Stores a skipmap iterator that refers to the lifetime of `MemTableIterator` itself.
    #[borrows(map)]
    #[not_covariant]
    iter: SkipMapRangeIter<'this>,
    /// Stores the current key-value pair.
    item: (KeyBytes, Bytes),
}

impl MemTableIterator {
    fn entry_to_item(entry: Option<Entry<'_, KeyBytes, Bytes>>) -> (KeyBytes, Bytes) {
        entry
            .map(|x| (x.key().clone(), x.value().clone()))
            .unwrap_or_else(|| (KeyBytes::new(), Bytes::new()))
    }
}

impl StorageIterator for MemTableIterator {
    type KeyType<'a> = KeySlice<'a>;

    fn value(&self) -> &[u8] {
        self.borrow_item().1.as_ref()
    }

    fn key(&self) -> KeySlice<'_> {
        self.borrow_item().0.as_key_slice()
    }

    fn is_valid(&self) -> bool {
        !self.borrow_item().0.is_empty()
    }

    fn next(&mut self) -> Result<()> {
        let entry = self.with_iter_mut(|iter| MemTableIterator::entry_to_item(iter.next()));
        self.with_item_mut(|item| {
            *item = entry;
        });
        Ok(())
    }
}
