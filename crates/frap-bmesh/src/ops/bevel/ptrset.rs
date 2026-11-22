// SPDX-License-Identifier: GPL-3.0-or-later
//! Iteration order of Blender's `blender::Set<BMFace *>` (`BLI_set.hh`), used by
//! `BM_mesh_bevel` to kill the rebuilt original faces (`rebuilt_orig_faces`). WP 8.2.
//!
//! Ported from blender/source/blender/blenlib/BLI_set.hh, BLI_hash_tables.hh,
//! BLI_probing_strategies.hh and BLI_hash.hh (Blender 5.2.4, GPL-2.0-or-later).
//!
//! The kill order decides which freed face slots the next operator of the same Edit Mode
//! session reuses (the mempool's free list is LIFO), so a second bevel (or any operator that
//! adds faces after a bevel) orders its faces after it. The set hashes a pointer as
//! `address >> 4` (`DefaultHash<T *>`), keeps up to 4 keys in an 8-slot inline buffer, grows at
//! a load factor of 1/2 (`realloc_and_reinsert` re-adds the keys in old-slot order) and probes
//! with `PythonProbingStrategy<1, false>`. Iteration is in slot order.
//!
//! **Address model.** An edit-mode face (`BMFace_OFlag`, 56 bytes) lives at
//! `chunk + 16 + 56 * index` in its `BLI_mempool` chunk (`CHUNK_OVERHEAD` = 16 bytes:
//! `MEM_mallocN`'s header and the chunk link). Measured on Blender 5.2.4 (macOS arm64,
//! `repr(BMFace)` in Edit Mode, several runs): the face chunk is a 32 KiB allocation aligned
//! to 16 KiB whose higher address bits change from run to run. The emulation takes the first
//! chunk at address 0 and the following chunks back to back: the hash bits below bit 10 are
//! then Blender's, which decides every first probe of tables up to 1024 slots and every second
//! probe of tables up to 32 slots (`perturb >> 5`). It reproduces the kill order Blender
//! 5.2.4 shows on grids of 25 and 36 faces (64 and 128 slots) and, given the measured chunk
//! address, on 81 faces (256 slots), where three keys need a probe that reads address bits
//! above bit 14: Blender's order there changes from run to run, and frap gives the order of a
//! chunk whose high bits are zero.

/// `sizeof(BMFace_OFlag)` (a face of an edit-mode BMesh, `use_toolflags`).
pub(crate) const FACE_OFLAG_SIZE: u64 = 56;

/// `CHUNK_OVERHEAD` of `BLI_mempool.cc` on 64-bit platforms.
const CHUNK_OVERHEAD: u64 = 16;

/// The modelled address of the element in pool slot `slot` (chunks of `pchunk` elements of
/// `esize` bytes, each chunk a power-of-two allocation laid out back to back from `chunk0`).
fn address(slot: u32, esize: u64, pchunk: u32, chunk0: u64) -> u64 {
    let slot = u64::from(slot);
    if pchunk == 0 {
        return chunk0 + CHUNK_OVERHEAD + esize * slot;
    }
    let pchunk = u64::from(pchunk);
    let chunk_bytes = (pchunk * esize + CHUNK_OVERHEAD).next_power_of_two();
    chunk0 + (slot / pchunk) * chunk_bytes + CHUNK_OVERHEAD + esize * (slot % pchunk)
}

/// `DefaultHash<T *>`.
fn ptr_hash(addr: u64) -> u64 {
    addr >> 4
}

/// The slot `PythonProbingStrategy` finds for `hash` in `slots` (the first empty one).
fn probe(slots: &[Option<u32>], hash: u64) -> usize {
    let mask = slots.len() as u64 - 1;
    let mut h = hash;
    let mut perturb = hash;
    loop {
        let i = (h & mask) as usize;
        if slots[i].is_none() {
            return i;
        }
        perturb >>= 5;
        h = h.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb);
    }
}

/// `power_of_2_max(ceil(min_usable / (1/2)))`, at least the 8 inline slots
/// (`compute_total_and_usable_slots` with `InlineBufferCapacity` 4).
fn total_slots(min_usable: usize) -> usize {
    (min_usable * 2).next_power_of_two().max(8)
}

/// The iteration order of a `Set<T *>` into which the elements of pool slots `keys` were
/// added in this order (duplicates ignored, as `Set::add`), each `esize` bytes in chunks of
/// `pchunk` elements.
pub(crate) fn set_order(keys: &[u32], esize: u64, pchunk: u32) -> Vec<u32> {
    set_order_at(keys, esize, pchunk, 0)
}

/// [`set_order`] with the first chunk at address `chunk0`.
fn set_order_at(keys: &[u32], esize: u64, pchunk: u32, chunk0: u64) -> Vec<u32> {
    let mut slots: Vec<Option<u32>> = vec![None];
    let mut usable = 0usize;
    let mut occupied = 0usize;
    let mut seen = rustc_hash::FxHashSet::default();
    let hash = |k: u32| ptr_hash(address(k, esize, pchunk, chunk0));
    for &k in keys {
        let h = hash(k);
        // `add__impl`: `ensure_can_add` runs before the probe that finds a duplicate.
        if occupied >= usable {
            // `realloc_and_reinsert(size + 1)`.
            let min_usable = (occupied + 1).max(occupied * 2);
            let total = total_slots(min_usable);
            let mut grown = vec![None; total];
            for old in slots.iter().flatten() {
                let i = probe(&grown, hash(*old));
                grown[i] = Some(*old);
            }
            slots = grown;
            usable = total / 2;
        }
        if !seen.insert(k) {
            continue;
        }
        let i = probe(&slots, h);
        slots[i] = Some(k);
        occupied += 1;
    }
    slots.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_match_blender_layout() {
        // `repr(BMFace)` of the factory cube in Edit Mode: 0x…4010, 0x…4048, 0x…4080, …
        assert_eq!(address(0, 56, 584, 0), 0x10);
        assert_eq!(address(1, 56, 584, 0), 0x48);
        assert_eq!(address(5, 56, 584, 0x8c76d4000), 0x8c76d4128);
        // Second chunk: 584 * 56 + 16 = 32720 rounds up to 32768.
        assert_eq!(address(584, 56, 584, 0), 32768 + 16);
    }

    #[test]
    fn small_sets_iterate_by_hash_slot() {
        // Hashes 1, 4, 8, 11 (addresses 0x10, 0x48, 0x80, 0xb8) in 8 slots: 1, 4, 0 (8 & 7),
        // 3 (11 & 7).
        assert_eq!(set_order(&[0, 1, 2, 3], 56, 584), vec![2, 0, 3, 1]);
        // Duplicates are ignored.
        assert_eq!(set_order(&[0, 0, 1], 56, 584), vec![0, 1]);
        assert!(set_order(&[], 56, 584).is_empty());
    }

    #[test]
    fn deep_probes_read_the_chunk_address() {
        // Blender 5.2.4, every edge of `primitive_grid_add(x_subdivisions=9, y_subdivisions=9)`
        // beveled: its 81 faces (slots 0..80, added in slot order) die in this order with the
        // face chunk at 0xc29228000 (measured with `repr()`).
        let keys: Vec<u32> = (0..81).collect();
        let mut blender: Vec<u32> = vec![73, 0, 1, 75, 2, 3, 77, 4, 5, 79, 6, 7];
        blender.extend(8..=26);
        blender.push(74);
        blender.extend(27..=36);
        blender.push(76);
        blender.extend(37..=46);
        blender.push(78);
        blender.extend(47..=56);
        blender.push(80);
        blender.extend(57..=72);
        assert_eq!(set_order_at(&keys, 56, 584, 0xc29228000), blender);
        // With the chunk at 0, keys 74, 76, 78 and 80 probe into other slots.
        let zero = set_order(&keys, 56, 584);
        assert_ne!(zero, blender);
        assert_eq!(zero[..12], blender[..12]);
    }

    #[test]
    fn growth_keeps_every_key() {
        let keys: Vec<u32> = (0..100).rev().collect();
        let mut got = set_order(&keys, 56, 584);
        assert_eq!(got.len(), 100);
        got.sort_unstable();
        assert_eq!(got, (0..100).collect::<Vec<_>>());
    }
}
