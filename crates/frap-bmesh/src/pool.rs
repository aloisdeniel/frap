// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_mempool` emulation (ARCHITECTURE §6.2, CONTRACT frozen by M2 W0; implemented by
//! WP 2.1): `alloc` pops the most recently freed slot (LIFO), else appends; iteration is in
//! ascending slot order, skipping free slots ([MESH §2.4, App. B.6]: after freeing slots 2 then
//! 4, three allocations land in 4, 2, then a new slot 6).

/// A pool of elements addressed by slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Pool<T> {
    /// `Some` for live slots.
    pub slots: Vec<Option<T>>,
    /// Freed slots, most recent last.
    pub free: Vec<u32>,
    /// Live element count.
    pub live: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }
}

impl<T> Pool<T> {
    /// Stores `v` in the most recently freed slot, else a new one; returns the slot.
    pub fn alloc(&mut self, v: T) -> u32 {
        let _ = v;
        todo!("WP 2.1: Pool::alloc")
    }
    /// Frees slot `i` (no-op on a free slot).
    pub fn free(&mut self, i: u32) {
        let _ = i;
        todo!("WP 2.1: Pool::free")
    }
    /// Live elements in ascending slot order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|v| (i as u32, v)))
    }
    /// The element in slot `i`, if live.
    pub fn get(&self, i: u32) -> Option<&T> {
        self.slots.get(i as usize).and_then(Option::as_ref)
    }
    /// The element in slot `i`, if live.
    pub fn get_mut(&mut self, i: u32) -> Option<&mut T> {
        self.slots.get_mut(i as usize).and_then(Option::as_mut)
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
