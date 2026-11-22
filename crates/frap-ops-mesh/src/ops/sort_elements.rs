// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.sort_elements(type='VIEW_ZAXIS', elements={'VERT'}, reverse=False, seed=0)`,
//! tier T1 (`tests/golden/cases/edit/tools_m8/sort_elements*.py`). Port of
//! `edbm_sort_elements_exec` and `sort_bmelem_flag` (`editors/mesh/editmesh_tools.cc`) and
//! `BM_mesh_remap` (`bmesh/intern/bmesh_mesh.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with a selected element of an affected type, the selected
//! vertices, edges and/or faces (`elements`; unset: the select mode's types) get a sort key and
//! are sorted with the C library `qsort` (not stable: equal keys keep macOS libc's order,
//! `frap_uv::pack::qsort_libc`); the unselected ones keep their indices and the selected ones
//! fill the remaining indices in key order. Keys: `VIEW_ZAXIS`/`VIEW_XAXIS` the depth / X of
//! the element (vertex, edge midpoint, face median centre) in the virtual 3D View (D-10);
//! `CURSOR_DISTANCE` the squared distance to the 3D cursor; `MATERIAL` (faces only) the
//! material index (`reverse`: `32767 - index`), ties by index; `RANDOMIZE` a `BLI_rng` value
//! seeded with `seed` (re-seeded per type; meshes after the first add the hash of the object's
//! `id.name`); `REVERSE` the negated index. `reverse` negates the view and cursor keys.
//! `SELECTED` moves the selected elements first (last with `reverse`) keeping both relative
//! orders. Selection, custom data and the selection history follow the elements.

use frap_bmesh::{BMesh, ElemFlag, ElemTypes, SelectMode};
use frap_math::rng::BliRng;
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh::{self, sort_elements::Type};
use frap_uv::pack::qsort_libc;
use glam::Mat4;

use super::select_random::strhash_p;

pub(crate) mod remap;

/// `MAXMAT`.
const MAXMAT: i16 = 32767;

/// `BMElemSort`.
#[derive(Copy, Clone, Debug)]
struct ElemSort {
    srt: f32,
    org_idx: u32,
}

/// `bmelemsort_comp`: `(a > b) - (a < b)` (NaN compares equal).
fn bmelemsort_comp(a: &ElemSort, b: &ElemSort) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a.srt > b.srt {
        Ordering::Greater
    } else if a.srt < b.srt {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// `mul_m4_m4m4(R, A, B)` (`R = A · B`, the SSE path: `(B0·A0 + B1·A1) + (B2·A2 + B3·A3)`
/// per column), on Blender's `m[col][row]` arrays.
pub(crate) fn mul_m4_m4m4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for k in 0..4 {
            r[i][k] =
                (b[i][0] * a[0][k] + b[i][1] * a[1][k]) + (b[i][2] * a[2][k] + b[i][3] * a[3][k]);
        }
    }
    r
}

/// `mul_v3_m4v3(r, M, v)`.
pub(crate) fn mul_v3_m4v3(m: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2] + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2] + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2] + m[3][2],
    ]
}

fn len_squared_v3v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

fn mid_v3_v3v3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// The element positions the keys are taken from, per type (0 vertices, 1 edges, 2 faces),
/// with their selection, in index order.
struct Elems {
    co: [Vec<[f32; 3]>; 3],
    mat: Vec<i16>,
    sel: [Vec<bool>; 3],
}

fn gather(bm: &BMesh, types: ElemTypes) -> Elems {
    let flag = ElemFlag::SELECT;
    let mut co: [Vec<[f32; 3]>; 3] = Default::default();
    let mut sel: [Vec<bool>; 3] = Default::default();
    let mut mat = Vec::new();
    if types.contains(ElemTypes::VERT) {
        for (_, v) in bm.verts.iter() {
            co[0].push(v.co);
            sel[0].push(v.flag.contains(flag));
        }
    }
    if types.contains(ElemTypes::EDGE) {
        for (_, e) in bm.edges.iter() {
            co[1].push(mid_v3_v3v3(bm.vert(e.v[0]).co, bm.vert(e.v[1]).co));
            sel[1].push(e.flag.contains(flag));
        }
    }
    if types.contains(ElemTypes::FACE) {
        for (i, f) in bm.faces.iter() {
            co[2].push(bm.face_calc_center_median(frap_bmesh::FaceId(i)));
            sel[2].push(f.flag.contains(flag));
            mat.push(f.mat_nr);
        }
    }
    Elems { co, mat, sel }
}

/// `sort_bmelem_flag`: the old-to-new index maps (`None`: type untouched).
fn sort_maps(
    e: &Elems,
    action: Type,
    reverse: bool,
    seed: u32,
    viewmat: &[[f32; 4]; 4],
    obmat: &[[f32; 4]; 4],
    cursor_local: [f32; 3],
) -> [Option<Vec<u32>>; 3] {
    let tot = [e.co[0].len(), e.co[1].len(), e.co[2].len()];
    let mut map: [Option<Vec<u32>>; 3] = [None, None, None];
    // `pblock` (protected = unselected) and `sblock` (keys of the selected).
    let mut pblock: [Option<Vec<bool>>; 3] = [None, None, None];
    let mut sblock: [Vec<ElemSort>; 3] = Default::default();
    let fact = if reverse { -1.0f32 } else { 1.0 };

    let mut keyed = |j: usize, key: &mut dyn FnMut(usize) -> f32| {
        if tot[j] == 0 {
            return;
        }
        let mut pb = vec![false; tot[j]];
        for (i, p) in pb.iter_mut().enumerate() {
            if e.sel[j][i] {
                sblock[j].push(ElemSort {
                    srt: key(i),
                    org_idx: i as u32,
                });
            } else {
                *p = true;
            }
        }
        pblock[j] = Some(pb);
    };

    match action {
        Type::ViewZaxis | Type::ViewXaxis => {
            let coidx = if action == Type::ViewZaxis { 2 } else { 0 };
            // Apply the view matrix to the object matrix.
            let mat = mul_m4_m4m4(viewmat, obmat);
            for j in 0..3 {
                keyed(j, &mut |i| mul_v3_m4v3(&mat, e.co[j][i])[coidx] * fact);
            }
        }
        Type::CursorDistance => {
            for j in 0..3 {
                keyed(j, &mut |i| {
                    len_squared_v3v3(cursor_local, e.co[j][i]) * fact
                });
            }
        }
        Type::Material => {
            // Faces only. Multiplying with totface and adding i keeps the current order of the
            // faces of one material.
            let totf = tot[2] as f32;
            keyed(2, &mut |i| {
                let nr = e.mat[i];
                let srt = if reverse {
                    f32::from(MAXMAT - nr)
                } else {
                    f32::from(nr)
                };
                srt * totf + i as f32
            });
        }
        Type::Selected => {
            for j in (0..3).rev() {
                if tot[j] == 0 {
                    continue;
                }
                let mut mp: Vec<u32> = Vec::with_capacity(tot[j]);
                let mut tb: Vec<u32> = Vec::with_capacity(tot[j]);
                for i in 0..tot[j] {
                    if e.sel[j][i] {
                        mp.push(i as u32);
                    } else {
                        tb.push(i as u32);
                    }
                }
                let aff = mp.len();
                if aff == 0 || aff == tot[j] {
                    continue;
                }
                // The new order (new index -> old index), selected first unless `reverse`.
                let order: Vec<u32> = if reverse {
                    tb.iter().chain(mp.iter()).copied().collect()
                } else {
                    mp.iter().chain(tb.iter()).copied().collect()
                };
                // Reverse mapping, we want an org2new one.
                let mut m = vec![0u32; tot[j]];
                for (new, &old) in order.iter().enumerate() {
                    m[old as usize] = new as u32;
                }
                map[j] = Some(m);
            }
        }
        Type::Randomize => {
            for j in 0..3 {
                // Re-init the random generator for each element type, for a consistent result
                // when enabling/disabling an element type.
                let mut rng = BliRng::new_srandom(seed);
                keyed(j, &mut |_| rng.get_float());
            }
        }
        Type::Reverse => {
            for j in 0..3 {
                keyed(j, &mut |i| -(i as f32));
            }
        }
    }

    let affected = sblock.iter().map(Vec::len).sum::<usize>() + map.iter().flatten().count();
    if affected == 0 {
        return [None, None, None];
    }

    // Sort affected elements, and populate the mapping arrays.
    for j in (0..3).rev() {
        let (Some(pb), true) = (&pblock[j], map[j].is_none()) else {
            continue;
        };
        let sb = &mut sblock[j];
        qsort_libc(sb, &mut bmelemsort_comp);
        let mut mp = vec![0u32; tot[j]];
        let mut s = sb.len();
        for i in (0..tot[j]).rev() {
            if pb[i] {
                // Protected.
                mp[i] = i as u32;
            } else {
                s -= 1;
                mp[sb[s].org_idx as usize] = i as u32;
            }
        }
        map[j] = Some(mp);
    }
    map
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SortElements) -> OpResult {
    let action = p.r#type.value;
    let use_reverse = p.reverse.value;
    let seed = p.seed.value as u32;

    // If no element types are set, use the current selection mode.
    let elem_types = if p.elements.is_set {
        let mut t = ElemTypes::empty();
        for s in &p.elements.value {
            match s.as_str() {
                "VERT" => t |= ElemTypes::VERT,
                "EDGE" => t |= ElemTypes::EDGE,
                "FACE" => t |= ElemTypes::FACE,
                _ => {}
            }
        }
        t
    } else {
        let Some(me) =
            poll::edit_object(&cx.ctx()).and_then(|(_, o)| o.data?.typed::<frap_rna::MeshId>())
        else {
            return Ok(OpStatus::Cancelled);
        };
        let mode = super::split::edbm::read(cx, me, |bm| bm.select_mode).unwrap_or_default();
        let mut t = ElemTypes::empty();
        if mode.contains(SelectMode::VERT) {
            t |= ElemTypes::VERT;
        }
        if mode.contains(SelectMode::EDGE) {
            t |= ElemTypes::EDGE;
        }
        if mode.contains(SelectMode::FACE) {
            t |= ElemTypes::FACE;
        }
        t
    };

    let (viewmat, cursor) = {
        let c = cx.ctx();
        (c.win.view3d.view_matrix, c.scene().cursor.location)
    };
    let viewmat = viewmat.to_cols_array_2d();

    for (ob_index, (ob, me)) in helpers::iter_edit_meshes(cx).into_iter().enumerate() {
        let Some((v, e, f)) = super::split::edbm::read(cx, me, |bm| bm.selected_counts()) else {
            continue;
        };
        if !((elem_types.contains(ElemTypes::VERT) && v > 0)
            || (elem_types.contains(ElemTypes::EDGE) && e > 0)
            || (elem_types.contains(ElemTypes::FACE) && f > 0))
        {
            continue;
        }
        let mut seed_iter = seed;
        // This gives a consistent result regardless of object order.
        if ob_index != 0 {
            let name = cx
                .main()
                .objects
                .get(ob)
                .map(|o| format!("OB{}", o.header.name))
                .unwrap_or_default();
            seed_iter = seed_iter.wrapping_add(strhash_p(&name));
        }
        let obmat: Mat4 = world_matrix_in(cx.main(), ob);
        // `invert_m4_m4(mat, object_to_world)`, `mul_m4_v3(mat, cur)`.
        let cursor_local = mul_v3_m4v3(&obmat.inverse().to_cols_array_2d(), cursor);
        let obmat = obmat.to_cols_array_2d();
        let maps = {
            let elems =
                super::split::edbm::read(cx, me, |bm| gather(bm, elem_types)).expect("edit mesh");
            sort_maps(
                &elems,
                action,
                use_reverse,
                seed_iter,
                &viewmat,
                &obmat,
                cursor_local,
            )
        };
        if maps.iter().all(Option::is_none) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        remap::mesh_remap(
            bm,
            maps[0].as_deref(),
            maps[1].as_deref(),
            maps[2].as_deref(),
        );
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elems(n: usize, sel: &[usize]) -> Elems {
        let co: Vec<[f32; 3]> = (0..n).map(|i| [i as f32, 0.0, 0.0]).collect();
        let s: Vec<bool> = (0..n).map(|i| sel.contains(&i)).collect();
        Elems {
            co: [co, Vec::new(), Vec::new()],
            mat: Vec::new(),
            sel: [s, Vec::new(), Vec::new()],
        }
    }

    const I: [[f32; 4]; 4] = frap_bmesh::IDENTITY;

    #[test]
    fn reverse_reverses_the_selected_only() {
        let e = elems(5, &[1, 2, 4]);
        let m = sort_maps(&e, Type::Reverse, false, 0, &I, &I, [0.0; 3]);
        // Unselected 0 and 3 stay; 1, 2, 4 reversed into the slots 1, 2, 4.
        assert_eq!(m[0].as_deref(), Some(&[0u32, 4, 2, 3, 1][..]));
    }

    #[test]
    fn selected_moves_selection_first_or_last() {
        let e = elems(5, &[1, 3]);
        let m = sort_maps(&e, Type::Selected, false, 0, &I, &I, [0.0; 3]);
        assert_eq!(m[0].as_deref(), Some(&[2u32, 0, 3, 1, 4][..]));
        let m = sort_maps(&e, Type::Selected, true, 0, &I, &I, [0.0; 3]);
        assert_eq!(m[0].as_deref(), Some(&[0u32, 3, 1, 4, 2][..]));
        // Nothing to move when everything is selected.
        let e = elems(3, &[0, 1, 2]);
        let m = sort_maps(&e, Type::Selected, false, 0, &I, &I, [0.0; 3]);
        assert!(m[0].is_none());
    }

    #[test]
    fn cursor_distance_sorts_nearest_first() {
        let e = elems(4, &[0, 1, 2, 3]);
        let m = sort_maps(&e, Type::CursorDistance, false, 0, &I, &I, [3.0, 0.0, 0.0]);
        assert_eq!(m[0].as_deref(), Some(&[3u32, 2, 1, 0][..]));
        let m = sort_maps(&e, Type::CursorDistance, true, 0, &I, &I, [3.0, 0.0, 0.0]);
        assert_eq!(m[0].as_deref(), Some(&[0u32, 1, 2, 3][..]));
    }
}
