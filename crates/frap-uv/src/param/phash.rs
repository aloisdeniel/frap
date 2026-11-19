// SPDX-License-Identifier: GPL-3.0-or-later
//! `PHash`: the parametrizer's special purpose hash. It keeps every element in a single
//! linked list (`nextlink`, the chart's element list during construction) whose order depends
//! on the hash: an element whose bucket is empty goes to the front of the list, otherwise it is
//! inserted right after the bucket's first element; growing re-inserts the whole list. That
//! order is what Blender iterates vertices, edges and faces in, so it is ported exactly.
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).

use super::{NIL, ParamKey};

/// `PHashSizes`.
const PHASH_SIZES: [u64; 29] = [
    1,
    3,
    5,
    11,
    17,
    37,
    67,
    131,
    257,
    521,
    1031,
    2053,
    4099,
    8209,
    16411,
    32771,
    65537,
    131_101,
    262_147,
    524_309,
    1_048_583,
    2_097_169,
    4_194_319,
    8_388_617,
    16_777_259,
    33_554_467,
    67_108_879,
    134_217_757,
    268_435_459,
];

/// An element linked by `PHash` (`PHashLink`: `next` is the element's `nextlink`).
pub(crate) trait HashLink {
    fn next(&self) -> u32;
    fn set_next(&mut self, n: u32);
    fn key(&self) -> ParamKey;
}

/// `PHash` (the list head lives in the caller, `ph->list`).
#[derive(Clone, Debug)]
pub(crate) struct PHash {
    buckets: Vec<u32>,
    size: usize,
    cursize: u64,
    cursize_id: usize,
}

impl PHash {
    /// `phash_new(list, sizehint)`.
    pub(crate) fn new(sizehint: u64) -> PHash {
        let mut cursize_id = 0;
        while PHASH_SIZES[cursize_id] < sizehint {
            cursize_id += 1;
        }
        let cursize = PHASH_SIZES[cursize_id];
        PHash {
            buckets: vec![NIL; cursize as usize],
            size: 0,
            cursize,
            cursize_id,
        }
    }

    /// `PHASH_hash`.
    #[inline]
    fn hash(&self, key: ParamKey) -> usize {
        (key % self.cursize) as usize
    }

    /// `phash_insert`.
    pub(crate) fn insert<T: HashLink>(&mut self, nodes: &mut [T], list: &mut u32, link: u32) {
        let size = self.cursize as usize;
        let hash = self.hash(nodes[link as usize].key());
        let lookup = self.buckets[hash];
        if lookup == NIL {
            // Insert in front of the list.
            self.buckets[hash] = link;
            nodes[link as usize].set_next(*list);
            *list = link;
        } else {
            // Insert after the existing element.
            let n = nodes[lookup as usize].next();
            nodes[link as usize].set_next(n);
            nodes[lookup as usize].set_next(link);
        }
        self.size += 1;

        if self.size > size * 3 {
            let first = *list;
            self.cursize_id += 1;
            self.cursize = PHASH_SIZES[self.cursize_id];
            self.buckets = vec![NIL; self.cursize as usize];
            self.size = 0;
            *list = NIL;
            let mut l = first;
            while l != NIL {
                let next = nodes[l as usize].next();
                self.insert(nodes, list, l);
                l = next;
            }
        }
    }

    /// `phash_lookup`.
    pub(crate) fn lookup<T: HashLink>(&self, nodes: &[T], key: ParamKey) -> u32 {
        let hash = self.hash(key);
        let mut link = self.buckets[hash];
        while link != NIL {
            let k = nodes[link as usize].key();
            if k == key {
                return link;
            }
            if self.hash(k) != hash {
                return NIL;
            }
            link = nodes[link as usize].next();
        }
        NIL
    }

    /// `phash_next`.
    pub(crate) fn next<T: HashLink>(&self, nodes: &[T], key: ParamKey, link: u32) -> u32 {
        let hash = self.hash(key);
        let mut link = nodes[link as usize].next();
        while link != NIL {
            let k = nodes[link as usize].key();
            if k == key {
                return link;
            }
            if self.hash(k) != hash {
                return NIL;
            }
            link = nodes[link as usize].next();
        }
        NIL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct N {
        next: u32,
        key: u64,
    }
    impl HashLink for N {
        fn next(&self) -> u32 {
            self.next
        }
        fn set_next(&mut self, n: u32) {
            self.next = n;
        }
        fn key(&self) -> u64 {
            self.key
        }
    }

    #[test]
    fn insert_lookup_and_order() {
        let mut nodes: Vec<N> = (0..40)
            .map(|k| N {
                next: NIL,
                key: k * 7 % 23,
            })
            .collect();
        let mut ph = PHash::new(1);
        let mut list = NIL;
        for i in 0..nodes.len() as u32 {
            ph.insert(&mut nodes, &mut list, i);
        }
        // Every element is in the list exactly once.
        let mut seen = vec![false; nodes.len()];
        let mut l = list;
        while l != NIL {
            assert!(!seen[l as usize]);
            seen[l as usize] = true;
            l = nodes[l as usize].next;
        }
        assert!(seen.iter().all(|&s| s));
        // Lookup finds an element with the key, and `next` walks the duplicates.
        for key in 0..23u64 {
            let mut found = 0;
            let mut e = ph.lookup(&nodes, key);
            while e != NIL {
                assert_eq!(nodes[e as usize].key, key);
                found += 1;
                e = ph.next(&nodes, key, e);
            }
            assert_eq!(found, nodes.iter().filter(|n| n.key == key).count());
        }
    }
}
