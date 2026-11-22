// SPDX-License-Identifier: GPL-3.0-or-later
//! The `GHash` `uv.stitch` collects its UV edges in: `stitch_init` copies the edges out in the
//! hash's iteration order, which decides the edge order of every later pass (normals, rotation
//! sums, the selection stack). Only what that needs: insertion (head of the bucket chain,
//! growth through the prime bucket sizes), lookup and iteration order.
//!
//! Ported from blender/source/blender/blenlib/intern/BLI_ghash.cc (`hashsizes`,
//! `ghash_buckets_expand`, `ghash_buckets_resize`, `ghash_insert_ex`, `GHASH_ITER`) and
//! blender/source/blender/blenlib/intern/BLI_ghash_utils.cc (`BLI_ghashutil_uinthash`),
//! Blender 5.2.4, GPL-2.0-or-later.

/// `hashsizes` (`GHASH_USE_MODULO_BUCKETS`).
const HASHSIZES: [u32; 27] = [
    5, 11, 17, 37, 67, 131, 257, 521, 1031, 2053, 4099, 8209, 16411, 32771, 65537, 131101,
    262147, 524309, 1048583, 2097169, 4194319, 8388617, 16777259, 33554467, 67108879, 134217757,
    268435459,
];

/// `GHASH_LIMIT_GROW`.
fn limit_grow(nbuckets: u32) -> u32 {
    nbuckets * 3 / 4
}

/// `BLI_ghashutil_uinthash`.
pub(crate) fn uinthash(mut key: u32) -> u32 {
    key = key.wrapping_add(!(key << 16));
    key ^= key >> 5;
    key = key.wrapping_add(key << 3);
    key ^= key >> 13;
    key = key.wrapping_add(!(key << 9));
    key ^= key >> 17;
    key
}

/// `uv_edge_hash`.
fn edge_hash(key: (u32, u32)) -> u32 {
    uinthash(key.1).wrapping_add(uinthash(key.0))
}

/// A `GHash` of UV edges `(uv1, uv2)` (`uv1 < uv2`) to an edge index.
#[derive(Clone, Debug)]
pub(crate) struct EdgeHash {
    /// `(key, value, next entry in the bucket chain)`.
    entries: Vec<((u32, u32), u32, Option<u32>)>,
    buckets: Vec<Option<u32>>,
    cursize: usize,
    limit_grow: u32,
}

impl EdgeHash {
    /// `BLI_ghash_new` (no reserve).
    pub(crate) fn new() -> EdgeHash {
        EdgeHash {
            entries: Vec::new(),
            buckets: vec![None; HASHSIZES[0] as usize],
            cursize: 0,
            limit_grow: limit_grow(HASHSIZES[0]),
        }
    }

    fn bucket(&self, key: (u32, u32)) -> usize {
        (edge_hash(key) % self.buckets.len() as u32) as usize
    }

    /// `BLI_ghash_lookup`.
    pub(crate) fn get(&self, key: (u32, u32)) -> Option<u32> {
        let mut e = self.buckets[self.bucket(key)];
        while let Some(i) = e {
            let (k, v, next) = self.entries[i as usize];
            if k == key {
                return Some(v);
            }
            e = next;
        }
        None
    }

    /// `BLI_ghash_insert` (the key must be absent).
    pub(crate) fn insert(&mut self, key: (u32, u32), value: u32) {
        let b = self.bucket(key);
        let idx = self.entries.len() as u32;
        self.entries.push((key, value, self.buckets[b]));
        self.buckets[b] = Some(idx);
        self.expand(self.entries.len() as u32);
    }

    /// `BLI_ghash_len`.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// `ghash_buckets_expand`.
    fn expand(&mut self, nentries: u32) {
        if nentries < self.limit_grow {
            return;
        }
        let mut new_nbuckets = self.buckets.len() as u32;
        while nentries > self.limit_grow && self.cursize < HASHSIZES.len() - 1 {
            self.cursize += 1;
            new_nbuckets = HASHSIZES[self.cursize];
            self.limit_grow = limit_grow(new_nbuckets);
        }
        if new_nbuckets as usize == self.buckets.len() {
            return;
        }
        self.limit_grow = limit_grow(new_nbuckets);
        // `ghash_buckets_resize` (growing): old buckets in order, each chain from its head,
        // pushed to the head of its new bucket.
        let old = std::mem::replace(&mut self.buckets, vec![None; new_nbuckets as usize]);
        for head in old {
            let mut e = head;
            while let Some(i) = e {
                let (key, _, next) = self.entries[i as usize];
                let b = self.bucket(key);
                self.entries[i as usize].2 = self.buckets[b];
                self.buckets[b] = Some(i);
                e = next;
            }
        }
    }

    /// The values in `GHASH_ITER` order (buckets in order, each chain from its head).
    pub(crate) fn values(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.entries.len());
        for head in &self.buckets {
            let mut e = *head;
            while let Some(i) = e {
                let (_, v, next) = self.entries[i as usize];
                out.push(v);
                e = next;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uinthash_matches_blender() {
        // Values of the C function with 32-bit unsigned arithmetic.
        let expect = [2894457277u32, 2635899082, 1185230479, 1799067091, 4002961159];
        for (k, e) in [0u32, 1, 7, 1000, u32::MAX].into_iter().zip(expect) {
            assert_eq!(uinthash(k), e);
        }
    }

    #[test]
    fn grows_and_keeps_every_entry() {
        let mut h = EdgeHash::new();
        for i in 0..200u32 {
            h.insert((i, i + 1), i);
        }
        assert_eq!(h.len(), 200);
        for i in 0..200u32 {
            assert_eq!(h.get((i, i + 1)), Some(i));
        }
        assert_eq!(h.get((5, 7)), None);
        let mut v = h.values();
        v.sort_unstable();
        assert_eq!(v, (0..200).collect::<Vec<_>>());
        assert_eq!(h.buckets.len(), 521);
    }
}
