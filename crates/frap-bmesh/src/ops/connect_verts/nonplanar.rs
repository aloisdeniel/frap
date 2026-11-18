// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_verts_nonplanar` (Blender 5.2.4 `bmo_connect_verts_nonplanar_exec`,
//! `operators/bmo_connect_nonplanar.cc`), implemented by WP 2.8.
//!
//! Python signature: `bmesh.ops.connect_verts_nonplanar(bmesh, angle_limit=0.0, faces=[])`.
//!
//! **Create order**: faces with more than three corners go on a stack (input order, popped
//! last first); each is split once (`BM_face_split`) along the legal diagonal leaving the two
//! flattest halves, when the angle between the halves' normals exceeds `angle_limit`; both
//! halves go back on the stack (the new face last, so it is processed first).
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH`.

use super::{face_calc_normal_subset, face_splits_check_legal, loop_is_adjacent};
use crate::ops::contextual_create::bli;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, LoopId};

/// Input slots of `connect_verts_nonplanar`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertsNonplanarIn {
    /// `angle_limit` (radians).
    pub angle_limit: f32,
    /// `faces`.
    pub faces: Vec<FaceId>,
}

/// Output slots of `connect_verts_nonplanar`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertsNonplanarOut {
    /// `edges.out` then `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the new edges (mesh order).
    pub edges: Vec<EdgeId>,
    /// `faces.out`: the split faces (mesh order).
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_connect_verts_nonplanar_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const EDGE_OUT: u16 = 1;
const FACE_OUT: u16 = 2;

/// `bm_face_subset_calc_planar`.
fn subset_calc_planar(bm: &BMesh, l_first: LoopId, l_last: LoopId, no: [f32; 3]) -> f32 {
    let l_term = bm.lp(l_last).next;
    let mut z_prev = bli::dot(no, bm.vert(bm.lp(l_last).v).co);
    let mut delta_z = 0.0f32;
    let mut l = l_first;
    loop {
        let z_curr = bli::dot(no, bm.vert(bm.lp(l).v).co);
        delta_z += (z_curr - z_prev).abs();
        z_prev = z_curr;
        l = bm.lp(l).next;
        if l == l_term {
            break;
        }
    }
    delta_z
}

/// `bm_face_split_find`.
fn face_split_find(bm: &BMesh, f: FaceId) -> Option<([LoopId; 2], f32)> {
    let l_arr = bm.face_loops(f);
    let n = l_arr.len();
    let mut err_best = f32::MAX;
    let mut angle_best_cos = -f32::MAX;
    let mut found: Option<[LoopId; 2]> = None;
    for i_a in 0..n {
        let l_a = l_arr[i_a];
        for &l_b in l_arr.iter().skip(i_a + 2) {
            if loop_is_adjacent(bm, l_a, l_b) {
                continue;
            }
            let (no_a, la) = face_calc_normal_subset(bm, l_a, l_b);
            if la == 0.0 {
                continue;
            }
            let (no_b, lb) = face_calc_normal_subset(bm, l_b, l_a);
            if lb == 0.0 {
                continue;
            }
            let err_test = subset_calc_planar(bm, l_a, l_b, no_a) + subset_calc_planar(bm, l_b, l_a, no_b);
            if err_test < err_best {
                let mut split = [Some([l_a, l_b])];
                face_splits_check_legal(bm, f, &mut split);
                if split[0].is_some() {
                    err_best = err_test;
                    found = Some([l_a, l_b]);
                    angle_best_cos = bli::dot(no_a, no_b);
                }
            }
        }
    }
    found.map(|p| (p, angle_best_cos))
}

/// Runs `connect_verts_nonplanar` on `bm`.
pub fn connect_verts_nonplanar(
    bm: &mut BMesh,
    i: &ConnectVertsNonplanarIn,
) -> Result<ConnectVertsNonplanarOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|&&f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "connect_verts_nonplanar: dead face {dead:?}"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect_nonplanar.cc
// (bmo_connect_verts_nonplanar_exec, bm_face_split_by_angle).
fn exec(bm: &mut BMesh, i: &ConnectVertsNonplanarIn) -> ConnectVertsNonplanarOut {
    let angle_limit_cos = libm::cosf(i.angle_limit);
    let mut stack: Vec<FaceId> = i
        .faces
        .iter()
        .copied()
        .filter(|&f| bm.face(f).len > 3)
        .collect();
    let mut changed = false;
    while let Some(f) = stack.pop() {
        frap_math::budget::check();
        let Some((pair, angle_cos)) = face_split_find(bm, f) else {
            continue;
        };
        if angle_cos >= angle_limit_cos {
            continue;
        }
        let Some((f_new, l_new)) = bm.face_split(f, pair[0], pair[1], None, false) else {
            continue;
        };
        bm.oflag_enable(f, FACE_OUT);
        bm.oflag_enable(f_new, FACE_OUT);
        let e = bm.lp(l_new).e;
        bm.oflag_enable(e, EDGE_OUT);
        for fp in [f, f_new] {
            bm.face_normal_update(fp);
            if bm.face(fp).len > 3 {
                stack.push(fp);
            }
        }
        changed = true;
    }
    let mut out = ConnectVertsNonplanarOut::default();
    if changed {
        out.edges = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| bm.oflag_test(e, EDGE_OUT))
            .collect();
        out.faces = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, FACE_OUT))
            .collect();
        out.geom = out.edges.iter().map(|&e| BMElem::Edge(e)).collect();
        out.geom.extend(out.faces.iter().map(|&f| BMElem::Face(f)));
    }
    out
}
