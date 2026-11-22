// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.copy_mirrored_faces(mesh_axis='POS_X', uv_axis='X', precision=3)`, tier T1
//! (`tests/golden/cases/uv/edit_m8/copy_mirrored_faces.py`). Port of
//! `uv_copy_mirrored_faces_exec` and `uv_copy_mirrored_faces`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_editmesh`. Vertices are paired with their mirror across the `mesh_axis`
//! plane after rounding the coordinates to `precision` decimals; a face whose vertices all have
//! a mirror gets the mirrored UVs (`u → 1 - u` on `uv_axis`) of the face made of those mirror
//! vertices, when both faces are visible and UV-selected and the face's median centre lies on
//! the `mesh_axis` side. Vertices rounding to the same position are counted and reported
//! (`WARNING` `<n> duplicates found in <m> mesh(es), mirror may be incomplete`). Faces are
//! visited in mesh order (Blender walks a hash map; the order only matters for a face that is
//! both a source and a destination). Always finishes.

use frap_bmesh::{FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use rustc_hash::FxHashMap;

use super::weld::common;
use crate::select::{UvSel, op};

/// A rounded position as a hash key (`float3` equality: `-0 == 0`).
fn key(p: [f32; 3]) -> [u32; 3] {
    p.map(|c| if c == 0.0 { 0 } else { c.to_bits() })
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::CopyMirroredFaces) -> OpResult {
    let precision = p.precision.value;
    let mesh_axis = match p.mesh_axis.value.identifier() {
        "POS_Y" => 1,
        "POS_Z" => 2,
        "NEG_X" => 3,
        "NEG_Y" => 4,
        "NEG_Z" => 5,
        _ => 0,
    };
    let uv_axis = usize::from(p.uv_axis.value.identifier() == "Y");
    let ts = op::ts(cx);
    let mut total_duplicates = 0;
    let mut meshes_with_duplicates = 0;
    for (_, me) in common::objects(cx) {
        let r = common::edit(cx, me, ts, |s| {
            let (changed, warn) = copy_mirrored(s, mesh_axis, uv_axis, precision);
            (warn, changed)
        })?;
        if let Some(warn) = r
            && warn != 0
        {
            total_duplicates += warn;
            meshes_with_duplicates += 1;
        }
    }
    if total_duplicates != 0 {
        cx.report(
            ReportType::Warning,
            format!(
                "{total_duplicates} duplicates found in {meshes_with_duplicates} mesh(es), \
                 mirror may be incomplete"
            ),
        );
    }
    Ok(OpStatus::Finished)
}

/// `uv_copy_mirrored_faces`: `(changed, double_warn)`.
fn copy_mirrored(s: &mut UvSel, mesh_axis: usize, uv_axis: usize, precision: i32) -> (bool, i32) {
    let mut double_warn = 0;
    let precision_scale = libm::powf(10.0, precision as f32);
    let axis = mesh_axis % 3;
    let mut mirror_gt: FxHashMap<[u32; 3], ([f32; 3], VertId)> = FxHashMap::default();
    let mut mirror_lt: FxHashMap<[u32; 3], ([f32; 3], VertId)> = FxHashMap::default();
    let mut gt_order: Vec<[u32; 3]> = Vec::new();
    let mut lt_order: Vec<[u32; 3]> = Vec::new();
    let verts = s.bm.vert_ids();
    for &v in &verts {
        let co = s.bm.vert(v).co;
        let pos = co.map(|c| (c * precision_scale).round());
        let k = key(pos);
        if pos[axis] >= 0.0 {
            if mirror_gt.insert(k, (pos, v)).is_some() {
                double_warn += 1;
            } else {
                gt_order.push(k);
            }
        }
        if pos[axis] <= 0.0 {
            if mirror_lt.insert(k, (pos, v)).is_some() {
                double_warn += 1;
            } else {
                lt_order.push(k);
            }
        }
    }
    // `vmap`: vertex slot → mirror vertex (`Map::add`: the first pairing wins).
    let mut vmap: Vec<Option<VertId>> = vec![None; s.bm.verts.capacity()];
    for (order, from, to) in [
        (&gt_order, &mirror_gt, &mirror_lt),
        (&lt_order, &mirror_lt, &mirror_gt),
    ] {
        for k in order {
            let (pos, v) = from[k];
            let mut mirror_pos = pos;
            mirror_pos[axis] = -mirror_pos[axis];
            if let Some(&(_, v_mirror)) = to.get(&key(mirror_pos))
                && vmap[v.slot()].is_none()
            {
                vmap[v.slot()] = Some(v_mirror);
            }
        }
    }

    // Faces by their sorted vertex set (`Map::add`: the first face wins).
    let faces = s.faces();
    let mut sorted_verts_to_face: FxHashMap<Vec<usize>, FaceId> = FxHashMap::default();
    let mut face_order: Vec<(Vec<usize>, FaceId)> = Vec::new();
    for &f in &faces {
        let mut sorted = Vec::new();
        let mut valid = true;
        for l in s.face_loops(f) {
            let v = s.lv(l);
            if vmap[v.slot()].is_none() {
                valid = false;
                break;
            }
            sorted.push(v.slot());
        }
        if valid {
            sorted.sort_unstable();
            if !sorted_verts_to_face.contains_key(&sorted) {
                sorted_verts_to_face.insert(sorted.clone(), f);
                face_order.push((sorted, f));
            }
        }
    }
    let mut face_map: Vec<(FaceId, FaceId)> = Vec::new();
    for (sorted, f_dst) in &face_order {
        let mut mirror: Vec<usize> = sorted
            .iter()
            .map(|&vs| vmap[vs].map_or(usize::MAX, |m| m.slot()))
            .collect();
        mirror.sort_unstable();
        if let Some(&f_src) = sorted_verts_to_face.get(&mirror)
            && f_src != *f_dst
        {
            face_map.push((*f_dst, f_src));
        }
    }

    let mut changed = false;
    let other_uv_axis = 1 - uv_axis;
    let is_negative = mesh_axis >= 3;
    for (f_dst, f_src) in face_map {
        // Skip unless both faces have all their UVs selected.
        if !s.face_visible(f_dst) || !s.face_select_test(f_dst) {
            continue;
        }
        if !s.face_visible(f_src) || !s.face_select_test(f_src) {
            continue;
        }
        let center = s.bm.face_calc_center_median(f_dst);
        if if is_negative {
            center[axis] > 0.0
        } else {
            center[axis] < 0.0
        } {
            continue;
        }
        for l_dst in s.face_loops(f_dst) {
            let Some(v_src) = vmap[s.lv(l_dst).slot()] else {
                continue;
            };
            let Some(l_src) = s.bm.face_vert_share_loop(f_src, v_src) else {
                continue;
            };
            let uv_src = s.uv(l_src);
            let uv_dst = &mut s.uv[l_dst.slot()];
            uv_dst[uv_axis] = -(uv_src[uv_axis] - 0.5) + 0.5;
            uv_dst[other_uv_axis] = uv_src[other_uv_axis];
            changed = true;
        }
    }
    (changed, double_warn)
}
