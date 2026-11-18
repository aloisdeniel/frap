// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.solidify` (Blender 5.2.4 `bmo_solidify_face_region_exec`,
//! `operators/bmo_extrude.cc`), implemented by WP 2.10. Uses `reverse_faces` (WP 2.9) and
//! `extrude_face_region` (WP 2.5).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.solidify(bmesh, geom=[], thickness=0.0)`.
//!
//! **Create/kill order.** The input faces are reversed first (`reverse_faces`, so the shell
//! is extruded inward), then the region is extruded keeping the originals
//! (`extrude_face_region use_keep_orig=True`, in that op's create order). The extruded faces
//! are marked, high-quality vertex normals are computed over the marked region
//! (`calc_solidify_normals`: angle-weighted edge normals, standard normals at non-manifold
//! vertices) and each marked vertex moves along its normal by `thickness` times the
//! angle-weighted mean of `1 / cos` between its normal and its faces' normals
//! (`solidify_add_thickness`). Nothing is killed. Default cube, all faces: V16 F12, the new
//! inner shell faces are the output ([MESH §4.12]).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use super::triangulate::math;
use crate::ops::extrude_face_region::{ExtrudeFaceRegionIn, extrude_face_region};
use crate::ops::reverse_faces::{ReverseFacesIn, reverse_faces};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, ElemFlag, ElemTypes, FaceId};

/// Input slots of `solidify` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidifyIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `thickness`.
    pub thickness: f32,
}

/// Output slots of `solidify`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SolidifyOut {
    /// `geom.out`: the new shell faces, edges and vertices (`extrude_face_region`'s
    /// `geom.out`).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_solidify_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const EDGE_NONMAN: u16 = 1;
const EDGE_MARK: u16 = 2;
const FACE_MARK: u16 = 4;
const VERT_MARK: u16 = 8;
const VERT_NONMAN: u16 = 16;

// Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc (calc_solidify_normals).
fn calc_solidify_normals(bm: &mut BMesh) {
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.insert(ElemFlag::TAG);
    }
    bm.elem_index_ensure(ElemTypes::EDGE);
    let mut edge_face_count = vec![0i32; bm.edges_num()];
    for f in bm.face_ids() {
        if !bm.oflag_test(f, FACE_MARK) {
            continue;
        }
        for e in bm.face_edges(f) {
            bm.oflag_enable(e, EDGE_MARK);
            let [a, b] = bm.edge(e).v;
            bm.oflag_enable(a, VERT_MARK);
            bm.oflag_enable(b, VERT_MARK);
            edge_face_count[bm.index_of(BMElem::Edge(e))] += 1;
        }
    }
    for e in bm.edge_ids() {
        if !bm.oflag_test(e, EDGE_MARK) {
            continue;
        }
        let i = edge_face_count[bm.index_of(BMElem::Edge(e))];
        if i == 0 || i > 2 {
            bm.oflag_enable(e, EDGE_NONMAN);
            let [a, b] = bm.edge(e).v;
            bm.oflag_enable(a, VERT_NONMAN);
            bm.oflag_enable(b, VERT_NONMAN);
        }
    }
    for v in bm.vert_ids() {
        if !bm.vert_is_manifold(v) {
            bm.oflag_enable(v, VERT_NONMAN);
            continue;
        }
        if bm.oflag_test(v, VERT_MARK) {
            bm.vert_mut(v).no = [0.0; 3];
        }
    }
    for e in bm.edge_ids() {
        if !bm.oflag_test(e, EDGE_MARK) || bm.oflag_test(e, EDGE_NONMAN) {
            continue;
        }
        let mut f1: Option<FaceId> = None;
        let mut f2: Option<FaceId> = None;
        for f in bm.edge_faces(e) {
            if bm.oflag_test(f, FACE_MARK) {
                if f1.is_none() {
                    f1 = Some(f);
                } else {
                    f2 = Some(f);
                }
            }
        }
        let Some(f1) = f1 else {
            continue;
        };
        let edge_normal = if let Some(f2) = f2 {
            let (n1, n2) = (bm.face(f1).no, bm.face(f2).no);
            let angle = math::angle_normalized_v3v3(n1, n2);
            if angle > 0.0 {
                // Two faces: the edge normal is weighted by the angle between them.
                frap_geom::normals::normalize_v3_length(math::add(n1, n2), angle).0
            } else {
                // Can't do anything useful here.
                let [a, b] = bm.edge(e).v;
                bm.vert_mut(a).flag.remove(ElemFlag::TAG);
                bm.vert_mut(b).flag.remove(ElemFlag::TAG);
                continue;
            }
        } else {
            // Only one face: 90 degrees seems good enough.
            math::scale(bm.face(f1).no, std::f32::consts::FRAC_PI_2)
        };
        let [a, b] = bm.edge(e).v;
        let na = bm.vert(a).no;
        bm.vert_mut(a).no = math::add(na, edge_normal);
        let nb = bm.vert(b).no;
        bm.vert_mut(b).no = math::add(nb, edge_normal);
    }
    for v in bm.vert_ids() {
        if !bm.oflag_test(v, VERT_MARK) {
            continue;
        }
        if bm.oflag_test(v, VERT_NONMAN) {
            bm.vert_normal_update(v);
        } else {
            let (n, len) = math::normalize_v3(bm.vert(v).no);
            bm.vert_mut(v).no = n;
            if len == 0.0 && !bm.vert(v).flag.contains(ElemFlag::TAG) {
                // Totally flat: use the normal of any marked face around the vertex.
                if let Some(f) = bm
                    .vert_faces(v)
                    .into_iter()
                    .find(|&f| bm.oflag_test(f, FACE_MARK))
                {
                    bm.vert_mut(v).no = bm.face(f).no;
                }
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc (solidify_add_thickness).
fn solidify_add_thickness(bm: &mut BMesh, dist: f32) {
    bm.elem_index_ensure(ElemTypes::VERT);
    let n = bm.verts_num();
    let mut vert_angles = vec![0.0f32; n];
    let mut vert_accum = vec![0.0f32; n];
    for f in bm.face_ids() {
        if !bm.oflag_test(f, FACE_MARK) {
            continue;
        }
        let verts = bm.face_verts(f);
        let cos: Vec<[f32; 3]> = verts.iter().map(|&v| bm.vert(v).co).collect();
        let face_angles = math::angle_poly_v3(&cos);
        let fno = bm.face(f).no;
        for (k, &v) in verts.iter().enumerate() {
            let index = bm.index_of(BMElem::Vert(v));
            vert_accum[index] += face_angles[k];
            vert_angles[index] +=
                math::shell_v3v3_normalized_to_dist(bm.vert(v).no, fno) * face_angles[k];
        }
    }
    for v in bm.vert_ids() {
        let index = bm.index_of(BMElem::Vert(v));
        if vert_accum[index] != 0.0 {
            let (co, no) = (bm.vert(v).co, bm.vert(v).no);
            bm.vert_mut(v).co = math::madd(co, no, dist * (vert_angles[index] / vert_accum[index]));
        }
    }
}

/// Runs `solidify` on `bm`.
pub fn solidify(bm: &mut BMesh, i: &SolidifyIn) -> Result<SolidifyOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "solidify: dead element {dead:?} in geom"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        // Flip the original faces (so the shell is extruded inward).
        let faces: Vec<FaceId> = i
            .geom
            .iter()
            .filter_map(|g| match g {
                BMElem::Face(f) => Some(*f),
                _ => None,
            })
            .collect();
        reverse_faces(
            bm,
            &ReverseFacesIn {
                faces,
                flip_multires: true,
            },
        )?;
        // Extrude the region.
        let ext = extrude_face_region(
            bm,
            &ExtrudeFaceRegionIn {
                geom: i.geom.clone(),
                use_keep_orig: true,
                ..ExtrudeFaceRegionIn::default()
            },
        )?;
        // Push the vertices of the extruded faces inward to create thickness.
        for g in &ext.geom {
            if let BMElem::Face(f) = g {
                bm.oflag_enable(*f, FACE_MARK);
            }
        }
        calc_solidify_normals(bm);
        solidify_add_thickness(bm, i.thickness);
        Ok(SolidifyOut { geom: ext.geom })
    })
}
