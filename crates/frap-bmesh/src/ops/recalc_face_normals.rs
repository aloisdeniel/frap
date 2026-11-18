// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.recalc_face_normals` (Blender 5.2.4 `bmo_recalc_face_normals_exec`,
//! `operators/bmo_normals.cc`), implemented by WP 2.9 (Normals, marks, attributes).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.recalc_face_normals(bmesh, faces=[])`.
//!
//! **Create/kill order**: nothing is created or killed; faces are flipped in place with
//! `BM_face_normal_flip` (`l_first` kept).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_normals.cc and
//! blender/source/blender/bmesh/intern/bmesh_query.cc (`BM_mesh_calc_face_groups`):
//! 1. The faces of the mesh are split into groups connected across **manifold** edges
//!    (exactly two faces), walked depth first from the first untagged face in slot order
//!    (the face header `TAG` is used, and left set, as Blender does).
//! 2. A group with at least one input face is oriented: its area-weighted centre is computed
//!    (`BM_face_calc_area` × `BM_face_calc_center_median_weighted` / face count), the
//!    outer-most loop is searched (largest distance from the centre, then best edge alignment,
//!    then best loop-normal alignment) and decides whether its face points inwards; from that
//!    face, winding is propagated across manifold edges (a neighbour whose shared edge runs
//!    the same way is inconsistent).
//! 3. Only input faces of the group that end up flagged as flipped are flipped.
//!
//! Face normals are recomputed first (Blender asserts they are valid; recomputing gives the
//! same values then). Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC`.

use crate::query::geom::{self, V3};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, ElemFlag, FaceId};

/// Input slots of `recalc_face_normals` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecalcFaceNormalsIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
}

/// Output slots of `recalc_face_normals` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RecalcFaceNormalsOut {
    /// Always empty (`bmesh.ops.recalc_face_normals` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_recalc_face_normals_def` (`UNTAN_MULTIRES` has no frap
/// equivalent).
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

const FACE_FLAG: u16 = 1 << 0;
const FACE_FLIP: u16 = 1 << 1;
const FACE_TEMP: u16 = 1 << 2;

/// Runs `recalc_face_normals` on `bm`.
pub fn recalc_face_normals(
    bm: &mut BMesh,
    i: &RecalcFaceNormalsIn,
) -> Result<RecalcFaceNormalsOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "recalc_face_normals: dead face {dead:?} in faces"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, &i.faces));
    Ok(RecalcFaceNormalsOut::default())
}

/// `bmo_recalc_normal_loop_filter_cb`: walk only across manifold edges.
fn loop_filter(bm: &BMesh, l: crate::LoopId) -> bool {
    bm.edge_is_manifold(bm.lp(l).e)
}

fn exec(bm: &mut BMesh, input: &[FaceId]) {
    for f in bm.face_ids() {
        bm.face_normal_update(f);
    }
    let groups = calc_face_groups(bm);
    for &f in input {
        bm.oflag_enable(f, FACE_FLAG);
    }
    for group in groups {
        frap_math::budget::check();
        if group.iter().any(|&f| bm.oflag_test(f, FACE_FLAG)) {
            recalc_face_normals_array(bm, &group, FACE_FLAG);
        }
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_mesh_calc_face_groups,
// with filter_fn = manifold edges, no pair filter, hflag_test = 0, htype_step = BM_EDGE).
/// The face groups, each in the order Blender's stack walk adds its faces.
fn calc_face_groups(bm: &mut BMesh) -> Vec<Vec<FaceId>> {
    let faces = bm.face_ids();
    for &f in &faces {
        bm.face_mut(f).flag.remove(ElemFlag::TAG);
    }
    let mut groups = Vec::new();
    let mut next = 0usize;
    let mut touched = 0usize;
    while touched != faces.len() {
        let mut stack = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !bm.face(f).flag.contains(ElemFlag::TAG) {
                bm.face_mut(f).flag.insert(ElemFlag::TAG);
                stack.push(f);
                break;
            }
        }
        debug_assert!(!stack.is_empty(), "face groups: untouched face expected");
        if stack.is_empty() {
            break;
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            frap_math::budget::check();
            group.push(f);
            touched += 1;
            for l_iter in bm.face_loops(f) {
                let mut l_radial = bm.lp(l_iter).radial_next;
                if l_radial == l_iter || !loop_filter(bm, l_iter) {
                    continue;
                }
                loop {
                    let f_other = bm.lp(l_radial).f;
                    if !bm.face(f_other).flag.contains(ElemFlag::TAG) {
                        bm.face_mut(f_other).flag.insert(ElemFlag::TAG);
                        stack.push(f_other);
                    }
                    l_radial = bm.lp(l_radial).radial_next;
                    if l_radial == l_iter {
                        break;
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// `BM_edge_calc_length`.
fn edge_length(bm: &BMesh, e: crate::EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    let d = geom::sub(bm.vert(a).co, bm.vert(b).co);
    geom::dot(d, d).sqrt()
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (BM_face_calc_area).
/// `BM_face_calc_area`: half the length of the Newell cross sum.
fn face_area(bm: &BMesh, f: FaceId) -> f32 {
    let mut n = [0.0f32; 3];
    for l in bm.face_loops(f) {
        let a = bm.vert(bm.lp(l).v).co;
        let b = bm.vert(bm.lp(bm.lp(l).next).v).co;
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    geom::dot(n, n).sqrt() * 0.5
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc
// (BM_face_calc_center_median_weighted).
/// The centre of `f` weighted by the lengths of the two edges at each corner.
fn face_center_median_weighted(bm: &BMesh, f: FaceId) -> V3 {
    let loops = bm.face_loops(f);
    let mut cent = [0.0f32; 3];
    let mut totw = 0.0f32;
    let mut w_prev = edge_length(bm, bm.lp(bm.lp(loops[0]).prev).e);
    for &l in &loops {
        let w_curr = edge_length(bm, bm.lp(l).e);
        let w = w_curr + w_prev;
        geom::madd_v3_v3fl(&mut cent, bm.vert(bm.lp(l).v).co, w);
        totw += w;
        w_prev = w_curr;
    }
    if totw != 0.0 {
        cent = geom::scale(cent, 1.0 / totw);
    }
    cent
}

#[derive(Copy, Clone)]
struct Best {
    dist_sq: f32,
    edge_dot: f32,
    loop_dot: f32,
}

// Ported from blender/source/blender/bmesh/operators/bmo_normals.cc
// (recalc_face_normals_find_index).
/// The index in `faces` of the face of the outer-most loop, and whether that face points
/// towards the centre.
fn find_index(bm: &BMesh, faces: &[FaceId]) -> (usize, bool) {
    let eps = f32::EPSILON;
    let mut cent_area_accum = 0.0f32;
    let mut cent = [0.0f32; 3];
    let cent_fac = 1.0f32 / faces.len() as f32;
    for &f in faces {
        let f_area = face_area(bm, f);
        let f_cent = face_center_median_weighted(bm, f);
        geom::madd_v3_v3fl(&mut cent, f_cent, cent_fac * f_area);
        cent_area_accum += f_area;
    }
    if cent_area_accum != 0.0 {
        cent = geom::scale(cent, 1.0 / cent_area_accum);
    }
    let mut best = Best {
        dist_sq: eps,
        edge_dot: -f32::MAX,
        loop_dot: -f32::MAX,
    };
    let mut is_flip = false;
    let mut f_start_index = 0usize;
    for (i, &f) in faces.iter().enumerate() {
        for l in bm.face_loops(f) {
            let ll = bm.lp(l);
            let co = bm.vert(ll.v).co;
            let mut dir = geom::sub(co, cent);
            let mut test = Best {
                dist_sq: geom::len_squared_v3(dir),
                edge_dot: 0.0,
                loop_dot: 0.0,
            };
            let is_best_dist_sq = test.dist_sq > best.dist_sq;
            if !(is_best_dist_sq || test.dist_sq == best.dist_sq) {
                continue;
            }
            dir = geom::scale(dir, 1.0 / test.dist_sq.sqrt());
            let (e0, l0) = geom::normalize_v3(geom::sub(bm.vert(bm.lp(ll.next).v).co, co));
            let (e1, l1) = geom::normalize_v3(geom::sub(bm.vert(bm.lp(ll.prev).v).co, co));
            if !(l0 > eps && l1 > eps) {
                continue;
            }
            test.edge_dot = geom::dot(dir, e0).max(geom::dot(dir, e1));
            let is_best_edge_dot = test.edge_dot > best.edge_dot;
            if !(is_best_dist_sq || is_best_edge_dot || test.edge_dot == best.edge_dot) {
                continue;
            }
            let (mut loop_dir, len) = geom::normalize_v3(geom::cross(e0, e1));
            if len <= eps {
                continue;
            }
            if geom::dot(loop_dir, bm.face(ll.f).no) < 0.0 {
                loop_dir = geom::scale(loop_dir, -1.0);
            }
            let loop_dir_dot = geom::dot(dir, loop_dir);
            test.loop_dot = loop_dir_dot.abs();
            if is_best_dist_sq || is_best_edge_dot || test.loop_dot > best.loop_dot {
                best = test;
                f_start_index = i;
                is_flip = loop_dir_dot < 0.0;
            }
        }
    }
    (f_start_index, is_flip)
}

// Ported from blender/source/blender/bmesh/operators/bmo_normals.cc
// (bmo_recalc_face_normals_array).
/// Makes the winding of the connected `faces` consistent and outward, flipping only those
/// carrying `oflag`.
fn recalc_face_normals_array(bm: &mut BMesh, faces: &[FaceId], oflag: u16) {
    let oflag_flip = oflag | FACE_FLIP;
    let (f_start_index, is_flip) = find_index(bm, faces);
    let start = faces[f_start_index];
    if is_flip {
        bm.oflag_enable(start, FACE_FLIP);
    }
    let mut stack = vec![start];
    bm.oflag_enable(start, FACE_TEMP);
    while let Some(f) = stack.pop() {
        frap_math::budget::check();
        let flip_state = bm.oflag_test(f, FACE_FLIP);
        for l_iter in bm.face_loops(f) {
            let l_other = bm.lp(l_iter).radial_next;
            if l_other == l_iter || !loop_filter(bm, l_iter) {
                continue;
            }
            let f_other = bm.lp(l_other).f;
            if !bm.oflag_test(f_other, FACE_TEMP) {
                bm.oflag_enable(f_other, FACE_TEMP);
                let same_dir = bm.lp(l_other).v == bm.lp(l_iter).v;
                bm.oflag_set(f_other, FACE_FLIP, same_dir != flip_state);
                stack.push(f_other);
            }
        }
    }
    for &f in faces {
        if bm.oflags.get(BMElem::Face(f)) & oflag_flip == oflag_flip {
            bm.face_normal_flip(f);
        }
        bm.oflag_disable(f, FACE_TEMP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::reverse_faces::{ReverseFacesIn, reverse_faces};
    use crate::types::CreateFlag;

    /// The cube of `mesh.primitive_cube_add()` ([MESH §1.4]).
    fn cube() -> BMesh {
        let mut bm = BMesh::new();
        let co = [
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
        ];
        let v: Vec<_> = co
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        for f in [
            [0, 1, 3, 2],
            [2, 3, 7, 6],
            [6, 7, 5, 4],
            [4, 5, 1, 0],
            [2, 6, 4, 0],
            [7, 3, 1, 5],
        ] {
            bm.face_create_verts(&f.map(|i| v[i]), None, CreateFlag::empty(), true)
                .expect("cube face");
        }
        bm.normals_update();
        bm
    }

    fn verts_of(bm: &BMesh) -> Vec<Vec<u32>> {
        bm.face_ids()
            .into_iter()
            .map(|f| bm.face_verts(f).into_iter().map(|v| v.0).collect())
            .collect()
    }

    #[test]
    fn restores_flipped_faces_outward() {
        let mut bm = cube();
        let original = verts_of(&bm);
        let faces = bm.face_ids();
        reverse_faces(
            &mut bm,
            &ReverseFacesIn {
                faces: vec![faces[1], faces[5]],
                ..Default::default()
            },
        )
        .expect("flip");
        assert_ne!(verts_of(&bm), original);
        recalc_face_normals(&mut bm, &RecalcFaceNormalsIn { faces }).expect("recalc");
        assert_eq!(verts_of(&bm), original);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn all_inward_cube_is_turned_outward() {
        let mut bm = cube();
        let original = verts_of(&bm);
        let faces = bm.face_ids();
        reverse_faces(
            &mut bm,
            &ReverseFacesIn {
                faces: faces.clone(),
                ..Default::default()
            },
        )
        .expect("flip");
        recalc_face_normals(&mut bm, &RecalcFaceNormalsIn { faces }).expect("recalc");
        assert_eq!(verts_of(&bm), original);
        for f in bm.face_ids() {
            let c = bm.face_calc_center_median(f);
            assert!(geom::dot(c, bm.face(f).no) > 0.0, "face {f:?} points inwards");
        }
    }

    #[test]
    fn only_input_faces_are_flipped() {
        let mut bm = cube();
        let faces = bm.face_ids();
        reverse_faces(
            &mut bm,
            &ReverseFacesIn {
                faces: vec![faces[0], faces[1]],
                ..Default::default()
            },
        )
        .expect("flip");
        let flipped = verts_of(&bm);
        recalc_face_normals(
            &mut bm,
            &RecalcFaceNormalsIn {
                faces: vec![faces[1]],
            },
        )
        .expect("recalc");
        let after = verts_of(&bm);
        assert_eq!(after[0], flipped[0], "face 0 is not an input face");
        assert_ne!(after[1], flipped[1], "face 1 is turned outward");
    }
}
