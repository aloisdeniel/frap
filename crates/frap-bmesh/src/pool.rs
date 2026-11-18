// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_mempool` emulation (ARCHITECTURE §6.2, CONTRACT frozen by M2 W0; implemented by
//! WP 2.1): `alloc` pops the most recently freed slot (LIFO), else appends; iteration is in
//! ascending slot order, skipping free slots ([MESH §2.4, App. B.6]: after freeing slots 2 then
//! 4, three allocations land in 4, 2, then a new slot 6).
//!
//! Ported from blender/source/blender/blenlib/intern/BLI_mempool.cc (Blender 5.2.4). A real
//! mempool is a list of fixed-size chunks whose elements are threaded on one free list: a new
//! chunk is only allocated when the free list is empty and is appended (so iteration order is
//! slot order), and freeing pushes onto the head of the free list (LIFO). One more rule is
//! observable through element order: **when the last element is freed and the pool owns more
//! than one chunk, every chunk but the first is released and the free list is rebuilt in slot
//! order** (`BLI_mempool_free`, "Nothing is in use; free all the chunks except the first").
//! The emulation therefore tracks the chunk size (`pchunk`, after `USE_CHUNK_POW2` rounding) and
//! the chunk count, see [`Pool::with_chunks`] and [`ChunkSizes`].

/// A pool of elements addressed by slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Pool<T> {
    /// `Some` for live slots.
    pub slots: Vec<Option<T>>,
    /// Freed slots, most recent last.
    pub free: Vec<u32>,
    /// Live element count.
    pub live: u32,
    /// Elements per chunk (`pool->pchunk`); 0 disables chunk tracking (one unbounded chunk,
    /// so the reset-on-empty rule never applies).
    pub pchunk: u32,
    /// Chunks currently owned by the pool (preallocated ones included).
    pub chunks: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            pchunk: 0,
            chunks: 0,
        }
    }
}

/// `pchunk` of the four BMesh pools after `BLI_mempool_create`'s power-of-two rounding
/// (`(power_of_2_max(pchunk * esize) - CHUNK_OVERHEAD) / esize`, `CHUNK_OVERHEAD` = 16 on
/// 64-bit), for an edit-mode BMesh (`use_toolflags`: `BMVert_OFlag` 56 bytes, `BMEdge_OFlag`
/// 80, `BMLoop` 72, `BMFace_OFlag` 56) and the requested chunk sizes of
/// `bm_mesh_chunksize_default` = {512, 1024, 2048, 512}.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChunkSizes;

impl ChunkSizes {
    /// Vertex pool chunk size.
    pub const VERT: u32 = 584;
    /// Edge pool chunk size.
    pub const EDGE: u32 = 1638;
    /// Loop pool chunk size.
    pub const LOOP: u32 = 3640;
    /// Face pool chunk size.
    pub const FACE: u32 = 584;
    /// `bm_mesh_chunksize_default`: the requested (unrounded) chunk sizes, used by
    /// `mempool_maxchunks` to count the preallocated chunks.
    pub const REQUESTED: [u32; 4] = [512, 1024, 2048, 512];
}

impl<T> Pool<T> {
    /// `BLI_mempool_create(esize, elem_num, pchunk_requested, …)`: a pool whose chunks hold
    /// `pchunk` elements, with `mempool_maxchunks(elem_num, pchunk_requested)` chunks
    /// preallocated (none when `elem_num` is 0).
    pub fn with_chunks(pchunk: u32, elem_num: u32, pchunk_requested: u32) -> Pool<T> {
        let chunks = if elem_num == 0 {
            0
        } else if elem_num <= pchunk_requested {
            1
        } else {
            elem_num / pchunk_requested + 1
        };
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            pchunk,
            chunks,
        }
    }

    /// Stores `v` in the most recently freed slot, else a new one; returns the slot.
    pub fn alloc(&mut self, v: T) -> u32 {
        self.live += 1;
        if let Some(i) = self.free.pop() {
            debug_assert!(self.slots[i as usize].is_none());
            self.slots[i as usize] = Some(v);
            return i;
        }
        if self.pchunk != 0 && self.slots.len() as u64 >= self.chunks as u64 * self.pchunk as u64 {
            // `mempool_chunk_add`: the free list is empty.
            self.chunks += 1;
        }
        let i = u32::try_from(self.slots.len()).expect("BMesh pool exceeds u32 slots");
        self.slots.push(Some(v));
        i
    }

    /// Frees slot `i` (no-op on a free slot).
    pub fn free(&mut self, i: u32) {
        let Some(slot) = self.slots.get_mut(i as usize) else {
            return;
        };
        if slot.take().is_none() {
            return;
        }
        self.free.push(i);
        self.live -= 1;
        if self.live == 0 && self.pchunk != 0 && self.chunks > 1 {
            // "Nothing is in use; free all the chunks except the first": the free list becomes
            // the first chunk in slot order.
            self.slots.clear();
            self.free.clear();
            self.chunks = 1;
        }
    }

    /// The slot `alloc` would return next.
    pub fn next_slot(&self) -> u32 {
        match self.free.last() {
            Some(&i) => i,
            None => self.slots.len() as u32,
        }
    }

    /// Live elements in ascending slot order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|v| (i as u32, v)))
    }
    /// Live slots in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.iter().map(|(i, _)| i)
    }
    /// The element in slot `i`, if live.
    pub fn get(&self, i: u32) -> Option<&T> {
        self.slots.get(i as usize).and_then(Option::as_ref)
    }
    /// The element in slot `i`, if live.
    pub fn get_mut(&mut self, i: u32) -> Option<&mut T> {
        self.slots.get_mut(i as usize).and_then(Option::as_mut)
    }
    /// Whether slot `i` is live.
    pub fn contains(&self, i: u32) -> bool {
        self.get(i).is_some()
    }
    /// Live element count.
    pub fn live(&self) -> usize {
        self.live as usize
    }
    /// Slot count (live and free): the length of per-slot columns.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifo_reuse_matches_mesh_app_b6() {
        // [MESH App. B.6]: delete v2 then v4 of 6 verts, add 100, 200, 300 →
        // [0, 1, 200, 3, 100, 5, 300].
        let mut p: Pool<f32> = Pool::with_chunks(584, 6, 512);
        for i in 0..6 {
            p.alloc(i as f32);
        }
        p.free(2);
        p.free(4);
        for v in [100.0, 200.0, 300.0] {
            p.alloc(v);
        }
        let order: Vec<f32> = p.iter().map(|(_, v)| *v).collect();
        assert_eq!(order, vec![0.0, 1.0, 200.0, 3.0, 100.0, 5.0, 300.0]);
    }

    #[test]
    fn empty_pool_with_one_chunk_keeps_lifo() {
        let mut p: Pool<u32> = Pool::with_chunks(584, 4, 512);
        for i in 0..4 {
            p.alloc(i);
        }
        for i in 0..4 {
            p.free(i);
        }
        assert_eq!(p.alloc(10), 3);
        assert_eq!(p.alloc(11), 2);
    }

    #[test]
    fn empty_pool_with_two_chunks_resets() {
        let mut p: Pool<u32> = Pool::with_chunks(584, 600, 512);
        assert_eq!(p.chunks, 2);
        for i in 0..600 {
            p.alloc(i);
        }
        for i in 0..600 {
            p.free(i);
        }
        assert_eq!(p.chunks, 1);
        assert_eq!(p.alloc(10), 0);
        assert_eq!(p.alloc(11), 1);
    }

    #[test]
    fn growing_past_a_chunk_adds_one() {
        let mut p: Pool<u32> = Pool::with_chunks(584, 500, 512);
        assert_eq!(p.chunks, 1);
        for i in 0..584 {
            p.alloc(i);
        }
        assert_eq!(p.chunks, 1);
        p.alloc(584);
        assert_eq!(p.chunks, 2);
    }
}
