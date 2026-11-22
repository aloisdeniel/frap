// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BKE_defvert_*` helpers the M8 weight operators need beyond `frap_bmesh::shape::deform`:
//! copying one group or a subset of groups between vertices, the flip and remap of group
//! indices, and the mirrored copy of `vgroup_parray_mirror_sync`.
//!
//! Ported from blender/source/blender/blenkernel/intern/deform.cc (Blender 5.2.4,
//! GPL-2.0-or-later). Entries keep Blender's order: `BKE_defvert_ensure_index` appends,
//! `BKE_defvert_remove_group` moves the last entry into the hole.

use frap_bmesh::shape::deform;
use frap_mesh::DeformVert;

/// `BKE_defvert_copy_index(dst, group_dst, src, group_src)`: the weight of `group_src` in
/// `src` written to `group_dst` of `dst` (created when needed); a source without that group
/// sets an existing destination weight to 0 (it is not removed).
pub fn copy_index(dst: &mut DeformVert, group_dst: u32, src: &DeformVert, group_src: u32) {
    match deform::find(src, group_src) {
        Some(i) => {
            let w = src.weights[i].1;
            *deform::ensure(dst, group_dst) = w;
        }
        None => {
            if let Some(i) = deform::find(dst, group_dst) {
                dst.weights[i].1 = 0.0;
            }
        }
    }
}

/// [`copy_index`] within one vertex (`dvert_dst == dvert_src`).
pub fn copy_index_self(dv: &mut DeformVert, group_dst: u32, group_src: u32) {
    let src = dv.clone();
    copy_index(dv, group_dst, &src, group_src);
}

/// `BKE_defvert_copy_subset`: [`copy_index`] for every group of `subset` (in index order).
pub fn copy_subset(dst: &mut DeformVert, src: &DeformVert, subset: &[bool]) {
    for (g, _) in subset.iter().enumerate().filter(|(_, s)| **s) {
        copy_index(dst, g as u32, src, g as u32);
    }
}

/// `BKE_defvert_mirror_subset`: for every group `g` of `subset` (up to the flip map's length),
/// the weight of `g` in `src` written to `flip_map[g]` of `dst`. `same` tells that `dst` and
/// `src` are the same vertex, where groups that map to themselves are skipped.
pub fn mirror_subset(
    dst: &mut DeformVert,
    src: &DeformVert,
    subset: &[bool],
    flip_map: &[i32],
    same: bool,
) {
    let n = subset.len().min(flip_map.len());
    for g in 0..n {
        if subset[g] && (!same || flip_map[g] != g as i32) {
            let Ok(t) = u32::try_from(flip_map[g]) else {
                continue;
            };
            if same {
                copy_index_self(dst, t, g as u32);
            } else {
                copy_index(dst, t, src, g as u32);
            }
        }
    }
}

/// `BKE_defvert_copy`: `dst` takes `src`'s weights.
pub fn copy(dst: &mut DeformVert, src: &DeformVert) {
    dst.weights = src.weights.clone();
}

/// `BKE_defvert_sync_mapped(dst, src, flip_map, use_ensure)`: when both vertices have weights,
/// each weight of `src` is written to group `flip_map[g]` of `dst` (created with
/// `use_ensure`, else only when present).
pub fn sync_mapped(dst: &mut DeformVert, src: &DeformVert, flip_map: &[i32], use_ensure: bool) {
    if src.weights.is_empty() || dst.weights.is_empty() {
        return;
    }
    for &(g, w) in &src.weights {
        let Some(&t) = flip_map.get(g as usize) else {
            continue;
        };
        let Ok(t) = u32::try_from(t) else {
            continue;
        };
        if use_ensure {
            *deform::ensure(dst, t) = w;
        } else if let Some(i) = deform::find(dst, t) {
            dst.weights[i].1 = w;
        }
    }
}

/// `BKE_defvert_flip`: every group index with a flip partner (`flip_map[g] >= 0`) is replaced
/// by it.
pub fn flip(dv: &mut DeformVert, flip_map: &[i32]) {
    for e in dv.weights.iter_mut() {
        if let Some(&t) = flip_map.get(e.0 as usize)
            && let Ok(t) = u32::try_from(t)
        {
            e.0 = t;
        }
    }
}

/// `BKE_defvert_remap`: every group index below the map's length becomes `map[g]`.
pub fn remap(dv: &mut DeformVert, map: &[usize]) {
    for e in dv.weights.iter_mut() {
        if let Some(&t) = map.get(e.0 as usize) {
            e.0 = t as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dv(w: &[(u32, f32)]) -> DeformVert {
        DeformVert {
            weights: w.iter().copied().collect(),
        }
    }

    #[test]
    fn copy_index_creates_or_zeroes() {
        let src = dv(&[(1, 0.5)]);
        let mut d = dv(&[(0, 0.2)]);
        copy_index(&mut d, 2, &src, 1);
        assert_eq!(d.weights.as_slice(), &[(0, 0.2), (2, 0.5)]);
        copy_index(&mut d, 0, &src, 3);
        assert_eq!(d.weights.as_slice(), &[(0, 0.0), (2, 0.5)]);
        copy_index(&mut d, 4, &src, 3);
        assert_eq!(d.weights.len(), 2);
    }

    #[test]
    fn mirror_subset_on_one_vertex_skips_self_mapped_groups() {
        // Groups: 0 = A.L, 1 = A.R, 2 = B.
        let flip_map = [1, 0, 2];
        let mut d = dv(&[(0, 0.25), (2, 0.5)]);
        let src = d.clone();
        mirror_subset(&mut d, &src, &[true, true, true], &flip_map, true);
        // A.L -> A.R (created), then A.R (now 0.25) -> A.L, B is skipped.
        assert_eq!(d.weights.as_slice(), &[(0, 0.25), (2, 0.5), (1, 0.25)]);
    }

    #[test]
    fn sync_mapped_needs_weights_on_both_sides() {
        let src = dv(&[(0, 0.75)]);
        let mut empty = DeformVert::default();
        sync_mapped(&mut empty, &src, &[1, 0], true);
        assert!(empty.weights.is_empty());
        let mut d = dv(&[(3, 0.1)]);
        sync_mapped(&mut d, &src, &[1, 0], true);
        assert_eq!(d.weights.as_slice(), &[(3, 0.1), (1, 0.75)]);
        let mut d = dv(&[(3, 0.1)]);
        sync_mapped(&mut d, &src, &[1, 0], false);
        assert_eq!(d.weights.as_slice(), &[(3, 0.1)]);
    }

    #[test]
    fn flip_and_remap() {
        let mut d = dv(&[(0, 0.1), (1, 0.2), (2, 0.3)]);
        flip(&mut d, &[1, 0, -1]);
        assert_eq!(d.weights.as_slice(), &[(1, 0.1), (0, 0.2), (2, 0.3)]);
        remap(&mut d, &[2, 0, 1]);
        assert_eq!(d.weights.as_slice(), &[(0, 0.1), (2, 0.2), (1, 0.3)]);
    }
}
