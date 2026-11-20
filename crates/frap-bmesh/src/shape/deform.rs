// SPDX-License-Identifier: GPL-3.0-or-later
//! `MDeformVert` helpers (`BKE_defvert_*`, `blenkernel/intern/deform.cc`) and the per-vertex
//! parts of the vertex-group editors (`vgroup_nr_vert_add`, `vgroup_limit_total_subset`,
//! `vgroup_parray_remove_zero`, `editors/object/object_vgroup.cc`).
//!
//! Ported from blender/source/blender/blenkernel/intern/deform.cc and
//! blender/source/blender/editors/object/object_vgroup.cc (Blender 5.2.4,
//! GPL-2.0-or-later). The weight list keeps Blender's order: entries are appended and a
//! removal moves the last entry into the hole (`BKE_defvert_remove_group`), which is what
//! `MeshVertex.groups` and the glTF exporter's influence order see.

use frap_mesh::{DeformVert, Domain, Mesh, SELECT_VERT};
use smol_str::SmolStr;

use crate::types::{BMesh, ElemFlag};

/// `MAX_VGROUP_NAME - 1`.
pub const GROUP_NAME_MAX: usize = 63;

/// `BKE_object_defgroup_unique_name`: `requested` made unique among `names` other than
/// `skip` (`BLI_uniquename_cb`, default `Group`).
pub fn unique_group_name(names: &[SmolStr], skip: Option<usize>, requested: &str) -> SmolStr {
    frap_rna::naming::unique_subname(
        &|n| {
            names
                .iter()
                .enumerate()
                .any(|(i, x)| Some(i) != skip && x == n)
        },
        requested,
        GROUP_NAME_MAX,
        "Group",
    )
}

/// The mesh's `.select_vert` (all false without the attribute).
pub fn mesh_vert_selection(me: &Mesh) -> Vec<bool> {
    me.bool_attribute(SELECT_VERT, Domain::Point)
        .map(|s| s.to_vec())
        .unwrap_or_else(|| vec![false; me.verts_num()])
}

/// `object_defgroup_remove_object_mode` on a mesh: group `g`'s weights go and every higher
/// group index moves down (the name list is the caller's).
pub fn remove_group_mesh(me: &mut Mesh, g: u32) {
    if me.deform_verts().is_none() {
        return;
    }
    for dv in me.deform_verts_mut().iter_mut() {
        remove_and_shift(dv, g);
    }
}

/// `object_defgroup_remove_edit_mode` on a BMesh: group `g` is cleared from every vertex and,
/// when any vertex held it, the higher group indices move down.
pub fn remove_group_bmesh(bm: &mut BMesh, g: u32) {
    if !clear_group_bmesh(bm, g, false) {
        return;
    }
    let ids = bm.vert_ids();
    if let Some(d) = bm.vdata.deform.as_mut() {
        for v in ids {
            if let Some(dv) = d.get_mut(v.slot()) {
                shift_down_above(dv, g);
            }
        }
    }
}

/// `BKE_object_defgroup_clear` on a mesh in Object Mode: removes group `g` from every vertex
/// that has weights (only the selected ones with `use_selection`). True when a vertex with
/// weights was visited.
pub fn clear_group_mesh(me: &mut Mesh, g: u32, use_selection: bool) -> bool {
    if me.deform_verts().is_none() {
        return false;
    }
    let sel = mesh_vert_selection(me);
    let mut changed = false;
    for (i, dv) in me.deform_verts_mut().iter_mut().enumerate() {
        if !dv.weights.is_empty() && (!use_selection || sel.get(i).copied().unwrap_or(false)) {
            remove_group(dv, g);
            changed = true;
        }
    }
    changed
}

/// `BKE_object_defgroup_clear` on an edit BMesh.
pub fn clear_group_bmesh(bm: &mut BMesh, g: u32, use_selection: bool) -> bool {
    let ids = bm.vert_ids();
    let sel: Vec<bool> = ids
        .iter()
        .map(|&v| bm.vert(v).flag.contains(ElemFlag::SELECT))
        .collect();
    let Some(d) = bm.vdata.deform.as_mut() else {
        return false;
    };
    let mut changed = false;
    for (k, v) in ids.into_iter().enumerate() {
        if let Some(dv) = d.get_mut(v.slot())
            && !dv.weights.is_empty()
            && (!use_selection || sel[k])
        {
            remove_group(dv, g);
            changed = true;
        }
    }
    changed
}

/// The `CD_MDEFORMVERT` column of a BMesh, added when missing (`BM_data_layer_add`).
pub fn bmesh_deform_mut(bm: &mut BMesh) -> &mut Vec<DeformVert> {
    let slots = bm.verts.capacity().max(bm.verts.next_slot() as usize);
    let d = bm.vdata.deform.get_or_insert_with(Vec::new);
    if d.len() < slots {
        d.resize(slots, DeformVert::default());
    }
    d
}

/// `VertexGroup.add(type=…)` assign mode (`WEIGHT_REPLACE`, `WEIGHT_ADD`, `WEIGHT_SUBTRACT`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssignMode {
    /// `REPLACE`
    Replace,
    /// `ADD`
    Add,
    /// `SUBTRACT`
    Subtract,
}

impl AssignMode {
    /// From the RNA identifier.
    pub fn from_identifier(s: &str) -> Option<AssignMode> {
        Some(match s {
            "REPLACE" => AssignMode::Replace,
            "ADD" => AssignMode::Add,
            "SUBTRACT" => AssignMode::Subtract,
            _ => return None,
        })
    }
}

/// `BKE_defvert_find_index`: the position of group `g` in the weight list.
pub fn find(dv: &DeformVert, g: u32) -> Option<usize> {
    dv.weights.iter().position(|&(x, _)| x == g)
}

/// `BKE_defvert_find_weight`: the weight of group `g` (0 when unassigned).
pub fn weight(dv: &DeformVert, g: u32) -> f32 {
    find(dv, g).map_or(0.0, |i| dv.weights[i].1)
}

/// `BKE_defvert_ensure_index`: the weight of group `g`, appended with 0 when missing.
pub fn ensure(dv: &mut DeformVert, g: u32) -> &mut f32 {
    let i = match find(dv, g) {
        Some(i) => i,
        None => {
            dv.weights.push((g, 0.0));
            dv.weights.len() - 1
        }
    };
    &mut dv.weights[i].1
}

/// `BKE_defvert_add_index_notest`: appends `(g, w)` without looking for `g`.
pub fn add_notest(dv: &mut DeformVert, g: u32, w: f32) {
    dv.weights.push((g, w));
}

/// `BKE_defvert_remove_group`: removes entry `i`, moving the last entry into its place.
pub fn remove_at(dv: &mut DeformVert, i: usize) {
    if i < dv.weights.len() {
        dv.weights.swap_remove(i);
    }
}

/// Removes group `g` from the vertex (`BKE_defvert_find_index` + `BKE_defvert_remove_group`).
pub fn remove_group(dv: &mut DeformVert, g: u32) -> bool {
    match find(dv, g) {
        Some(i) => {
            remove_at(dv, i);
            true
        }
        None => false,
    }
}

/// `vgroup_nr_vert_add`: assigns `weight` to group `g` with `mode` (the weight is already
/// clamped to 0..1 by RNA). `ADD` clamps the sum at 1; `SUBTRACT` removes the vertex from the
/// group when the result is 0 or less and does nothing for an unassigned vertex.
pub fn vert_add(dv: &mut DeformVert, g: u32, weight: f32, mode: AssignMode) {
    match find(dv, g) {
        Some(i) => match mode {
            AssignMode::Replace => dv.weights[i].1 = weight,
            AssignMode::Add => {
                let w = dv.weights[i].1 + weight;
                dv.weights[i].1 = w.min(1.0);
            }
            AssignMode::Subtract => {
                dv.weights[i].1 -= weight;
                if dv.weights[i].1 <= 0.0 {
                    remove_at(dv, i);
                }
            }
        },
        None => match mode {
            AssignMode::Subtract => {}
            AssignMode::Replace | AssignMode::Add => add_notest(dv, g, weight),
        },
    }
}

/// Removing group `g` from the object in Object Mode (`object_defgroup_remove_object_mode`):
/// its weight goes and every higher group index moves down by one.
pub fn remove_and_shift(dv: &mut DeformVert, g: u32) {
    remove_group(dv, g);
    for w in dv.weights.iter_mut() {
        if w.0 > g {
            w.0 -= 1;
        }
    }
}

/// Lowers every group index above `g` by one (the Edit Mode removal's second pass).
pub fn shift_down_above(dv: &mut DeformVert, g: u32) {
    for w in dv.weights.iter_mut() {
        if w.0 > g {
            w.0 -= 1;
        }
    }
}

fn flag(flags: &[bool], g: u32) -> bool {
    flags.get(g as usize).copied().unwrap_or(false)
}

/// `BKE_defvert_normalize_ex`: scales the weights of the groups in `subset` (every group when
/// empty) so they sum to 1, keeping `locks` (hard) and preferring to keep `soft_locks`.
pub fn normalize_ex(dv: &mut DeformVert, subset: &[bool], locks: &[bool], soft_locks: &[bool]) {
    let use_subset = !subset.is_empty();
    let use_locks = !locks.is_empty();
    let use_soft = !soft_locks.is_empty();
    match dv.weights.len() {
        0 => return,
        1 => {
            let g = dv.weights[0].0;
            if use_subset && !flag(subset, g) {
                return;
            }
            if !use_locks || !flag(locks, g) {
                dv.weights[0].1 = 1.0;
            }
            return;
        }
        _ => {}
    }
    let (mut locked, mut soft, mut regular) = (0.0f32, 0.0f32, 0.0f32);
    let mut soft_count = 0;
    for &(g, w) in &dv.weights {
        if use_subset && !flag(subset, g) {
            continue;
        }
        if use_locks && flag(locks, g) {
            locked += w;
        } else if use_soft && flag(soft_locks, g) {
            soft += w;
            soft_count += 1;
        } else {
            regular += w;
        }
    }
    let available = (1.0 - locked).max(0.0);
    if regular <= 0.0 && soft <= 0.0 {
        if soft_count == 0 {
            return;
        }
        let w = available / soft_count as f32;
        for e in dv.weights.iter_mut() {
            if use_subset && !flag(subset, e.0) {
                continue;
            }
            if use_locks && flag(locks, e.0) {
                continue;
            }
            if use_soft && flag(soft_locks, e.0) {
                e.1 = w;
            }
        }
        return;
    }
    let (soft_scale, regular_scale) = if soft >= available || regular <= 0.0 {
        (available / soft, 0.0)
    } else {
        (1.0, (available - soft) / regular)
    };
    for e in dv.weights.iter_mut() {
        if use_subset && !flag(subset, e.0) {
            continue;
        }
        if use_locks && flag(locks, e.0) {
            continue;
        }
        if use_soft && flag(soft_locks, e.0) {
            e.1 *= soft_scale;
        } else {
            e.1 *= regular_scale;
        }
        e.1 = e.1.clamp(0.0, 1.0);
    }
}

/// `inv_cmp_mdef_vert_weights`: descending weight; ties answer what Blender's comparator does
/// (it compares the addresses of its two local pointer variables, a constant per build:
/// `&dw1 > &dw2` on the 5.2.4 macOS arm64 build, verified by the `limit_total` goldens).
fn inv_cmp(a: &(u32, f32), b: &(u32, f32)) -> i32 {
    if a.1 < b.1 {
        1
    } else if a.1 > b.1 {
        -1
    } else {
        LIMIT_TOTAL_TIE
    }
}

/// What `inv_cmp_mdef_vert_weights` answers for two equal weights on Blender 5.2.4.
const LIMIT_TOTAL_TIE: i32 = -1;

/// One vertex of `vgroup_limit_total_subset`: when the vertex has more than `max` weights in
/// the groups of `validmap` (`subset_count` is the size of the subset), the lowest of them are
/// dropped. Returns the number removed. The list is re-packed as Blender does: other groups
/// first (in order), then the subset's weights sorted descending (`qsort`).
pub fn limit_total(
    dv: &mut DeformVert,
    validmap: &[bool],
    subset_count: usize,
    max: usize,
) -> usize {
    if subset_count <= max {
        return 0;
    }
    let n = dv.weights.len();
    let mut tmp: Vec<(u32, f32)> = vec![(0, 0.0); n];
    let (mut bone, mut non_bone) = (0usize, 0usize);
    for &w in &dv.weights {
        if flag(validmap, w.0) {
            tmp[n - 1 - bone] = w;
            bone += 1;
        } else {
            tmp[non_bone] = w;
            non_bone += 1;
        }
    }
    if bone <= max {
        return 0;
    }
    let drop = bone - max;
    crate::ops::contextual_create::bli::c_qsort(&mut tmp[non_bone..], &inv_cmp);
    tmp.truncate(n - drop);
    dv.weights = tmp.into_iter().collect();
    drop
}

/// One vertex of `vgroup_parray_remove_zero`: removes the weights `<= epsilon` of the groups
/// in `validmap`, last first, keeping one weight with `keep_single`.
pub fn remove_zero(dv: &mut DeformVert, validmap: &[bool], epsilon: f32, keep_single: bool) {
    let mut j = dv.weights.len();
    while j > 0 {
        j -= 1;
        if keep_single && dv.weights.len() == 1 {
            break;
        }
        let (g, w) = dv.weights[j];
        if flag(validmap, g) && w <= epsilon {
            remove_at(dv, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn dv(w: &[(u32, f32)]) -> DeformVert {
        DeformVert {
            weights: w.iter().copied().collect(),
        }
    }

    #[test]
    fn assign_modes_match_vgroup_nr_vert_add() {
        let mut d = DeformVert::default();
        vert_add(&mut d, 0, 0.5, AssignMode::Replace);
        vert_add(&mut d, 0, 0.7, AssignMode::Add);
        assert_eq!(d.weights.as_slice(), &[(0, 1.0)]);
        vert_add(&mut d, 1, 0.2, AssignMode::Subtract);
        assert_eq!(d.weights.len(), 1);
        vert_add(&mut d, 0, 0.3, AssignMode::Subtract);
        assert!((weight(&d, 0) - 0.7).abs() < 1e-6);
        vert_add(&mut d, 0, 0.7, AssignMode::Subtract);
        assert!(find(&d, 0).is_none());
    }

    #[test]
    fn removal_moves_the_last_entry() {
        let mut d = dv(&[(0, 0.1), (1, 0.2), (2, 0.3)]);
        remove_group(&mut d, 0);
        assert_eq!(d.weights.as_slice(), &[(2, 0.3), (1, 0.2)]);
        remove_and_shift(&mut d, 1);
        assert_eq!(d.weights.as_slice(), &[(1, 0.3)]);
    }

    #[test]
    fn normalize_scales_unlocked_weights() {
        let mut d = dv(&[(0, 0.2), (1, 0.6)]);
        normalize_ex(&mut d, &[], &[], &[]);
        assert!((d.weights[0].1 - 0.25).abs() < 1e-6);
        assert!((d.weights[1].1 - 0.75).abs() < 1e-6);
        let mut d = dv(&[(0, 0.2), (1, 0.6)]);
        normalize_ex(&mut d, &[], &[true, false], &[]);
        assert!((d.weights[0].1 - 0.2).abs() < 1e-6);
        assert!((d.weights[1].1 - 0.8).abs() < 1e-6);
        let mut d = dv(&[(3, 0.0)]);
        normalize_ex(&mut d, &[], &[], &[]);
        assert_eq!(d.weights[0].1, 1.0);
    }

    #[test]
    fn limit_total_drops_the_lowest() {
        let mut d = DeformVert {
            weights: smallvec![(0, 0.1), (1, 0.9), (2, 0.5)],
        };
        assert_eq!(limit_total(&mut d, &[true; 3], 3, 2), 1);
        assert_eq!(d.weights.as_slice(), &[(1, 0.9), (2, 0.5)]);
    }

    #[test]
    fn remove_zero_keeps_single() {
        let mut d = dv(&[(0, 0.0), (1, 0.0)]);
        remove_zero(&mut d, &[true, true], 0.0, true);
        assert_eq!(d.weights.len(), 1);
        let mut d = dv(&[(0, 0.0), (1, 0.4)]);
        remove_zero(&mut d, &[true, true], 0.0, false);
        assert_eq!(d.weights.as_slice(), &[(1, 0.4)]);
    }
}
