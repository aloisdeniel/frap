// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.planar_faces` (Blender 5.2.4 `bmo_planar_faces_exec`,
//! `operators/bmo_planar_faces.cc`), implemented by WP 8.3.3.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.planar_faces(bmesh, faces=[], iterations=0, factor=0.0)` → `{'geom': []}`.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_planar_faces.cc (Blender 5.2.4,
//! GPL-2.0-or-later). Iteratively flattens the faces with four or more corners: each pass
//! projects every corner of a tagged face onto the plane through the face's length-weighted
//! centre (computed once, before the first pass) with the face's **original** normal
//! (normals are not updated between passes, "else we 'move' the face"), averages the
//! projections per vertex and moves each vertex by `factor` towards its average when it moved
//! more than 1e-5; the faces around every vertex are tagged again. Stops after `iterations`
//! passes or a pass that moves nothing. Triangles are skipped.
//!
//! **Create/kill order**: nothing is created or killed. Operator type flags:
//! `SELECT_FLUSH | SELECT_VALIDATE` (the caller updates the normals).

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, FaceId, VertId};

/// Input slots of `planar_faces`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanarFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `iterations`.
    pub iterations: i32,
    /// `factor`.
    pub factor: f32,
}

/// Output slots of `planar_faces`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PlanarFacesOut {
    /// `geom.out`: Blender's operator never fills it.
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_planar_faces_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::SELECT_FLUSH.union(OpTypeFlag::SELECT_VALIDATE);

type V3 = [f32; 3];

fn len_v3v3(a: V3, b: V3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// `BM_face_calc_center_median_weighted`: the corners weighted by their two edge lengths.
pub(crate) fn face_calc_center_median_weighted(bm: &BMesh, f: FaceId) -> V3 {
    let mut cent = [0.0f32; 3];
    let mut totw = 0.0f32;
    let loops = bm.face_loops(f);
    let edge_len = |l: crate::LoopId| {
        let [a, b] = bm.edge(bm.lp(l).e).v;
        len_v3v3(bm.vert(a).co, bm.vert(b).co)
    };
    let mut w_prev = edge_len(bm.lp(loops[0]).prev);
    for &l in &loops {
        let w_curr = edge_len(l);
        let w = w_curr + w_prev;
        let co = bm.vert(bm.lp(l).v).co;
        for k in 0..3 {
            cent[k] += co[k] * w;
        }
        totw += w;
        w_prev = w_curr;
    }
    if totw != 0.0 {
        let f = 1.0 / totw;
        for c in &mut cent {
            *c *= f;
        }
    }
    cent
}

/// Runs `planar_faces` on `bm`.
pub fn planar_faces(bm: &mut BMesh, i: &PlanarFacesIn) -> Result<PlanarFacesOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "planar_faces: dead face {dead:?} in faces"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        run(bm, i);
        Ok(PlanarFacesOut::default())
    })
}

fn run(bm: &mut BMesh, i: &PlanarFacesIn) {
    let fac = i.factor;
    let eps_sq = 0.00001f32 * 0.00001f32;
    let nf = i.faces.len();
    let mut faces_center = vec![[0.0f32; 3]; nf];
    // `ELE_FACE_ADJUST`, by face slot.
    let mut face_adjust = vec![false; bm.faces.capacity()];
    for (k, &f) in i.faces.iter().enumerate() {
        if bm.face(f).len == 3 {
            continue;
        }
        faces_center[k] = face_calc_center_median_weighted(bm, f);
        face_adjust[f.slot()] = true;
    }
    // `vaccum_map`: the accumulation per vertex, in first-insertion order (the order does not
    // change the result: every vertex moves by its own average).
    let mut order: Vec<VertId> = Vec::new();
    let mut accum: Vec<Option<([f32; 3], i32)>> = vec![None; bm.verts.capacity()];
    for _ in 0..i.iterations.max(0) {
        frap_math::budget::check();
        let mut changed = false;
        for (k, &f) in i.faces.iter().enumerate() {
            if !face_adjust[f.slot()] {
                continue;
            }
            face_adjust[f.slot()] = false;
            // `plane_from_point_normal_v3(plane, faces_center[i], f->no)`.
            let no = bm.face(f).no;
            let c = faces_center[k];
            let d = -(no[0] * c[0] + no[1] * c[1] + no[2] * c[2]);
            for l in bm.face_loops(f) {
                let v = bm.lp(l).v;
                let pt = bm.vert(v).co;
                // `closest_to_plane_normalized_v3`.
                let side = pt[0] * no[0] + pt[1] * no[1] + pt[2] * no[2] + d;
                let co = [
                    pt[0] + no[0] * -side,
                    pt[1] + no[1] * -side,
                    pt[2] + no[2] * -side,
                ];
                let slot = &mut accum[v.slot()];
                if slot.is_none() {
                    order.push(v);
                    *slot = Some(([0.0; 3], 0));
                }
                let va = slot.as_mut().expect("just inserted");
                va.1 += 1;
                // `interp_v3_v3v3(va.co, va.co, co, 1.0f / float(va.co_tot))`.
                let t = 1.0 / va.1 as f32;
                let s = 1.0 - t;
                for (a, c) in va.0.iter_mut().zip(co) {
                    *a = s * *a + t * c;
                }
            }
        }
        for &v in &order {
            let (va_co, _) = accum[v.slot()].expect("in order");
            let vco = bm.vert(v).co;
            let d = [vco[0] - va_co[0], vco[1] - va_co[1], vco[2] - va_co[2]];
            if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > eps_sq {
                let s = 1.0 - fac;
                bm.vert_mut(v).co = [
                    s * vco[0] + fac * va_co[0],
                    s * vco[1] + fac * va_co[1],
                    s * vco[2] + fac * va_co[2],
                ];
                changed = true;
            }
            // Tag for re-calculation.
            for f in bm.vert_faces(v) {
                if bm.face(f).len != 3 {
                    face_adjust[f.slot()] = true;
                }
            }
        }
        // If nothing changed, break out early.
        if !changed {
            break;
        }
        for v in order.drain(..) {
            accum[v.slot()] = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_centre_of_a_square_is_its_middle() {
        let mut bm = BMesh::new();
        let vs: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, crate::CreateFlag::empty()))
        .collect();
        let f = bm
            .face_create_verts(&vs, None, crate::CreateFlag::empty(), true)
            .expect("face");
        assert_eq!(face_calc_center_median_weighted(&bm, f), [1.0, 1.0, 0.0]);
    }
}
