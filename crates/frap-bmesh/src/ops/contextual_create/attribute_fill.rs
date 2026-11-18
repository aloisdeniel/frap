// SPDX-License-Identifier: GPL-3.0-or-later
//! `face_attribute_fill` (`operators/bmo_fill_attribute.cc`) and the face-normal recalculation
//! it falls back to (`operators/bmo_normals.cc`, `bmo_recalc_face_normals_exec`), Blender 5.2.4.
//!
//! The recalculation is a private copy used by the fill ops of WP 2.8 (`edgenet_fill`,
//! `holes_fill`) so they can be tested before WP 2.9's `ops/recalc_face_normals.rs` lands; the
//! two are the same port of the same Blender function.

use rustc_hash::FxHashSet;

use super::bli::{self, V3};
use super::construct;
use crate::{BMesh, ElemFlag, FaceId, LoopId};

/// `bm_loop_is_all_radial_tag`.
fn loop_is_all_radial_tag(bm: &BMesh, l: LoopId) -> bool {
    let mut it = bm.lp(l).radial_next;
    loop {
        if !bm.face(bm.lp(it).f).flag.contains(ElemFlag::TAG) {
            return false;
        }
        it = bm.lp(it).radial_next;
        if it == l {
            return true;
        }
    }
}

/// `bm_face_copy_shared_all`.
fn face_copy_shared_all(bm: &mut BMesh, l: LoopId, use_normals: bool, use_data: bool) {
    let f = bm.lp(l).f;
    let mut l_other = bm.lp(l).radial_next;
    while bm.face(bm.lp(l_other).f).flag.contains(ElemFlag::TAG) {
        l_other = bm.lp(l_other).radial_next;
    }
    let f_other = bm.lp(l_other).f;
    if use_data {
        bm.face_attrs_copy(f_other, f);
        let untag = |bm: &BMesh, ls: LoopId| !bm.face(bm.lp(ls).f).flag.contains(ElemFlag::TAG);
        construct::face_copy_shared(bm, f, Some(&untag));
    }
    if use_normals && bm.lp(l).v == bm.lp(l_other).v {
        bm.face_normal_flip(f);
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_fill_attribute.cc
// (bmesh_face_attribute_fill, bmo_face_attribute_fill_exec).
/// `bmesh.ops.face_attribute_fill`: flood-fills face attributes and winding from the faces
/// around `faces`; returns `faces_fail.out` (the input faces with no untagged neighbour).
pub fn face_attribute_fill(
    bm: &mut BMesh,
    faces: &[FaceId],
    use_normals: bool,
    use_data: bool,
) -> Vec<FaceId> {
    construct::faces_flag_disable_all(bm, ElemFlag::TAG);
    for &f in faces {
        bm.face_mut(f).flag.insert(ElemFlag::TAG);
    }
    let mut prev: Vec<LoopId> = Vec::new();
    let mut next: Vec<LoopId> = Vec::new();
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::TAG) {
            for l in bm.face_loops(f) {
                if !loop_is_all_radial_tag(bm, l) {
                    prev.push(l);
                }
            }
        }
    }
    let mut face_tot = 0usize;
    while !prev.is_empty() {
        while let Some(l) = prev.pop() {
            let f = bm.lp(l).f;
            if !bm.face(f).flag.contains(ElemFlag::TAG) {
                continue;
            }
            bm.face_mut(f).flag.remove(ElemFlag::TAG);
            let mut l_iter = bm.lp(l).next;
            while l_iter != l {
                let first = bm.lp(l_iter).radial_next;
                if first != l_iter {
                    let mut r = first;
                    loop {
                        if bm.face(bm.lp(r).f).flag.contains(ElemFlag::TAG) {
                            next.push(r);
                        }
                        r = bm.lp(r).radial_next;
                        if r == l_iter {
                            break;
                        }
                    }
                }
                l_iter = bm.lp(l_iter).next;
            }
            face_copy_shared_all(bm, l, use_normals, use_data);
            face_tot += 1;
        }
        std::mem::swap(&mut prev, &mut next);
    }
    if face_tot != faces.len() {
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
            .collect()
    } else {
        Vec::new()
    }
}

/// `bmo_recalc_normal_loop_filter_cb`.
fn manifold_loop(bm: &BMesh, l: LoopId) -> bool {
    bm.edge_is_manifold(bm.lp(l).e)
}

// Ported from blender/source/blender/bmesh/operators/bmo_normals.cc
// (recalc_face_normals_find_index).
fn find_index(bm: &BMesh, faces: &[FaceId]) -> (usize, bool) {
    let eps = f32::EPSILON;
    let mut cent_area_accum = 0.0f32;
    let mut cent = [0.0f32; 3];
    let cent_fac = 1.0 / faces.len() as f32;
    for &f in faces {
        let f_area = construct::face_calc_area(bm, f);
        let f_cent = construct::face_calc_center_median_weighted(bm, f);
        bli::madd_v3_v3fl(&mut cent, f_cent, cent_fac * f_area);
        cent_area_accum += f_area;
    }
    if cent_area_accum != 0.0 {
        cent = bli::scale(cent, 1.0 / cent_area_accum);
    }
    #[derive(Copy, Clone)]
    struct Best {
        dist_sq: f32,
        edge_dot: f32,
        loop_dot: f32,
    }
    let mut best = Best {
        dist_sq: eps,
        edge_dot: -f32::MAX,
        loop_dot: -f32::MAX,
    };
    let mut f_start_index = 0usize;
    let mut is_flip = false;
    for (i, &f) in faces.iter().enumerate() {
        for l in bm.face_loops(f) {
            let ll = bm.lp(l);
            let co = bm.vert(ll.v).co;
            let mut dir: V3 = bli::sub(co, cent);
            let mut test = Best {
                dist_sq: bli::len_squared_v3(dir),
                edge_dot: 0.0,
                loop_dot: 0.0,
            };
            let is_best_dist_sq = test.dist_sq > best.dist_sq;
            if is_best_dist_sq || test.dist_sq == best.dist_sq {
                dir = bli::scale(dir, 1.0 / test.dist_sq.sqrt());
                let (e0, l0) = bli::normalize_v3(bli::sub(bm.vert(bm.lp(ll.next).v).co, co));
                let (e1, l1) = bli::normalize_v3(bli::sub(bm.vert(bm.lp(ll.prev).v).co, co));
                if l0 > eps && l1 > eps {
                    test.edge_dot = bli::dot(dir, e0).max(bli::dot(dir, e1));
                    let is_best_edge_dot = test.edge_dot > best.edge_dot;
                    if is_best_dist_sq || is_best_edge_dot || test.edge_dot == best.edge_dot {
                        let (mut loop_dir, ll_len) = bli::normalize_v3(bli::cross(e0, e1));
                        if ll_len > eps {
                            if bli::dot(loop_dir, bm.face(f).no) < 0.0 {
                                loop_dir = bli::negate(loop_dir);
                            }
                            let loop_dir_dot = bli::dot(dir, loop_dir);
                            test.loop_dot = loop_dir_dot.abs();
                            if is_best_dist_sq || is_best_edge_dot || test.loop_dot > best.loop_dot
                            {
                                best = test;
                                f_start_index = i;
                                is_flip = loop_dir_dot < 0.0;
                            }
                        }
                    }
                }
            }
        }
    }
    (f_start_index, is_flip)
}

// Ported from blender/source/blender/bmesh/operators/bmo_normals.cc
// (bmo_recalc_face_normals_array).
fn recalc_array(bm: &mut BMesh, faces: &[FaceId], flagged: &FxHashSet<FaceId>) {
    let (start, is_flip) = find_index(bm, faces);
    let mut flip: FxHashSet<FaceId> = FxHashSet::default();
    let mut temp: FxHashSet<FaceId> = FxHashSet::default();
    if is_flip {
        flip.insert(faces[start]);
    }
    let mut stack = vec![faces[start]];
    temp.insert(faces[start]);
    while let Some(f) = stack.pop() {
        let flip_state = flip.contains(&f);
        for l in bm.face_loops(f) {
            let l_other = bm.lp(l).radial_next;
            if l_other != l && manifold_loop(bm, l) {
                let fo = bm.lp(l_other).f;
                if temp.insert(fo) {
                    if (bm.lp(l_other).v == bm.lp(l).v) != flip_state {
                        flip.insert(fo);
                    } else {
                        flip.remove(&fo);
                    }
                    stack.push(fo);
                }
            }
        }
    }
    for &f in faces {
        if flagged.contains(&f) && flip.contains(&f) {
            bm.face_normal_flip(f);
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_normals.cc
// (bmo_recalc_face_normals_exec).
/// `bmesh.ops.recalc_face_normals(faces=…)` (see the module docs).
pub fn recalc_face_normals(bm: &mut BMesh, faces: &[FaceId]) {
    let groups = construct::mesh_calc_face_groups(bm, &manifold_loop);
    let flagged: FxHashSet<FaceId> = faces.iter().copied().collect();
    for g in groups {
        if g.iter().any(|f| flagged.contains(f)) {
            recalc_array(bm, &g, &flagged);
        }
    }
}
