// SPDX-License-Identifier: GPL-3.0-or-later
//! Corner-data cycling inside faces, shared by `mesh.uvs_rotate`/`uvs_reverse` and
//! `mesh.colors_rotate`/`colors_reverse`.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_utils.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `bmo_rotate_uvs_exec`, `bm_face_reverse_uvs` / `bmo_reverse_uvs_exec`,
//! `bmo_rotate_colors_exec`, `bm_face_reverse_colors` / `bmo_reverse_colors_exec`. The UV and
//! colour variants move whole values, so one generic column routine serves both.

use std::sync::Arc;

use frap_bmesh::{BMesh, CdLayer, ElemFlag};
use frap_mesh::AttrData;

/// The loop slots of the faces with `SELECT`, in mesh order (`faces=%hf` with
/// `BM_ELEM_SELECT`), each face's from its first loop.
fn selected_faces_slots(bm: &BMesh) -> Vec<Vec<usize>> {
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
        .map(|f| bm.face_loops(f).into_iter().map(|l| l.slot()).collect())
        .collect()
}

/// Rotates the values of each face's corners: clockwise (`use_ccw=False`) every corner takes
/// the value of the corner before it and the first that of the last; counter-clockwise every
/// corner takes the value of the corner after it and the last that of the first.
fn rotate_col<T: Clone + Default>(col: &mut Vec<T>, faces: &[Vec<usize>], use_ccw: bool) {
    for slots in faces {
        let Some(&max) = slots.iter().max() else {
            continue;
        };
        if col.len() <= max {
            col.resize(max + 1, T::default());
        }
        let vals: Vec<T> = slots.iter().map(|&s| col[s].clone()).collect();
        let n = slots.len();
        for (i, &s) in slots.iter().enumerate() {
            let src = if use_ccw { (i + 1) % n } else { (i + n - 1) % n };
            col[s] = vals[src].clone();
        }
    }
}

/// Reverses the order of each face's corner values (`bm_face_reverse_uvs`): corner `i` takes the
/// value of corner `len - i - 1`.
fn reverse_col<T: Clone + Default>(col: &mut Vec<T>, faces: &[Vec<usize>]) {
    for slots in faces {
        let Some(&max) = slots.iter().max() else {
            continue;
        };
        if col.len() <= max {
            col.resize(max + 1, T::default());
        }
        let vals: Vec<T> = slots.iter().map(|&s| col[s].clone()).collect();
        let n = slots.len();
        for (i, &s) in slots.iter().enumerate() {
            col[s] = vals[n - i - 1].clone();
        }
    }
}

/// What to do with each selected face's corners.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Cycle {
    /// `rotate_uvs` / `rotate_colors` with `use_ccw`.
    Rotate {
        /// Counter-clockwise.
        use_ccw: bool,
    },
    /// `reverse_uvs` / `reverse_colors`.
    Reverse,
}

/// Applies `op` to the selected faces' values of corner layer `layer` (a `FLOAT2`, `FLOAT_COLOR`
/// or `BYTE_COLOR` column; other types are left alone).
pub(crate) fn apply(bm: &mut BMesh, layer: usize, op: Cycle) {
    let faces = selected_faces_slots(bm);
    let Some(CdLayer { data, .. }) = bm.ldata.layers.get_mut(layer) else {
        return;
    };
    macro_rules! go {
        ($c:expr) => {{
            let col = Arc::make_mut($c);
            match op {
                Cycle::Rotate { use_ccw } => rotate_col(col, &faces, use_ccw),
                Cycle::Reverse => reverse_col(col, &faces),
            }
        }};
    }
    match data {
        AttrData::Float2(c) => go!(c),
        AttrData::FloatColor(c) => go!(c),
        AttrData::ByteColor(c) => go!(c),
        _ => {}
    }
}

/// `CustomData_get_offset(&bm->ldata, CD_PROP_FLOAT2)`: the position of the active UV map (the
/// BMesh's `uv_active` layer, else its first non-internal `FLOAT2` corner layer).
pub(crate) fn active_uv(bm: &BMesh) -> Option<usize> {
    let float2 = |l: &CdLayer| matches!(l.data, AttrData::Float2(_)) && !l.name.starts_with('.');
    if let Some(a) = &bm.ldata.uv_active
        && let Some(i) = bm.ldata.find(a)
        && float2(&bm.ldata.layers[i])
    {
        return Some(i);
    }
    bm.ldata.layers.iter().position(float2)
}

/// `BKE_attribute_to_index(mesh, active_color_attribute, ATTR_DOMAIN_MASK_CORNER,
/// CD_MASK_COLOR_ALL)` then `bmo_get_loop_color_ref`: the position of the corner colour layer
/// named `name` (both walk the edit BMesh's corner layers in order, so the index names the same
/// layer); `None` when the active colour is unset, on another domain or anonymous.
pub(crate) fn active_corner_color(bm: &BMesh, name: Option<&str>) -> Option<usize> {
    let name = name?;
    if name.starts_with(".a_") {
        return None;
    }
    bm.ldata.layers.iter().position(|l| {
        l.name == name && matches!(l.data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_and_reverse_like_bmo_utils() {
        let faces = vec![vec![0, 1, 2, 3], vec![5, 4, 6]];
        let mut c = vec![0, 1, 2, 3, 4, 5, 6];
        rotate_col(&mut c, &faces, false);
        assert_eq!(c, [3, 0, 1, 2, 5, 6, 4], "clockwise: the previous corner's value");
        let mut c = vec![0, 1, 2, 3, 4, 5, 6];
        rotate_col(&mut c, &faces, true);
        assert_eq!(c, [1, 2, 3, 0, 6, 4, 5], "counter-clockwise: the next corner's value");
        let mut c = vec![0, 1, 2, 3, 4, 5, 6];
        reverse_col(&mut c, &faces);
        assert_eq!(c, [3, 2, 1, 0, 4, 6, 5]);
    }
}
