// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.flatten(bmesh, geom=[], factor=0.0, method=0, view_normal=Vector(), lock_x=False,
//! lock_y=False, lock_z=False)` (new in 5.x), with the face and edge grouping it uses.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_flatten.cc and
//! blender/source/blender/bmesh/intern/bmesh_query.cc (`BM_mesh_calc_face_groups`,
//! `BM_mesh_calc_edge_groups`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The faces of `geom` are grouped by shared edges; each group's vertices (face order, first
//! occurrence) are projected onto one plane: the best-fit plane of the vertex cloud
//! (`BM_verts_calc_normal_from_cloud_ex`), the plane through the centroid with the
//! area-weighted mean face normal (`NORMAL`; best fit for loose edges), or the plane through
//! the centroid facing the view (`VIEW`). The edges of `geom` that belong to none of those
//! faces are grouped by shared vertices and flattened the same way. Locked axes keep their
//! coordinate; `factor` blends between the original and the projected position.

use frap_bmesh::ops::contextual_create::construct::{
    face_calc_area, verts_calc_normal_from_cloud_ex,
};
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, VertId};

/// `FlattenMethod`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FlattenMethod {
    /// `FLATTEN_BEST_FIT`.
    BestFit,
    /// `FLATTEN_NORMAL`.
    Normal,
    /// `FLATTEN_VIEW`.
    View,
}

/// The input slots of `flatten`.
pub(crate) struct FlattenIn {
    pub geom: Vec<BMElem>,
    pub factor: f32,
    pub method: FlattenMethod,
    pub view_normal: [f32; 3],
    pub lock: [bool; 3],
}

type V3 = [f32; 3];

/// `compute_centroid`.
fn compute_centroid(bm: &BMesh, verts: &[VertId]) -> V3 {
    let mut c = [0.0f32; 3];
    for &v in verts {
        let co = bm.vert(v).co;
        for k in 0..3 {
            c[k] += co[k];
        }
    }
    let n = verts.len() as f32;
    [c[0] / n, c[1] / n, c[2] / n]
}

/// `compute_average_face_normal`.
fn compute_average_face_normal(bm: &BMesh, faces: &[FaceId]) -> V3 {
    let mut n = [0.0f32; 3];
    for &f in faces {
        let no = bm.face(f).no;
        let a = face_calc_area(bm, f);
        for k in 0..3 {
            n[k] += no[k] * a;
        }
    }
    // `normalize_v3(normal) != 0.0f ? normal : float3(0, 0, 1)`.
    let d = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if d > 1.0e-35 {
        let f = 1.0 / d.sqrt();
        [n[0] * f, n[1] * f, n[2] * f]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// `flatten_verts_fn`.
fn flatten_verts(bm: &mut BMesh, i: &FlattenIn, verts: &[VertId], faces: &[FaceId]) {
    let mut method = i.method;
    // The normal method averages face normals: vertices without faces use the best fit.
    if method == FlattenMethod::Normal && faces.is_empty() {
        method = FlattenMethod::BestFit;
    }
    let (normal, center) = match method {
        FlattenMethod::BestFit => {
            let (n, c, _) = verts_calc_normal_from_cloud_ex(bm, verts);
            (n, c)
        }
        FlattenMethod::Normal => (
            compute_average_face_normal(bm, faces),
            compute_centroid(bm, verts),
        ),
        FlattenMethod::View => (i.view_normal, compute_centroid(bm, verts)),
    };
    for &v in verts {
        let co = bm.vert(v).co;
        let d = [co[0] - center[0], co[1] - center[1], co[2] - center[2]];
        let dot = d[0] * normal[0] + d[1] * normal[1] + d[2] * normal[2];
        // `co - dot(co - center, normal) * normal`.
        let mut projected = [
            co[0] - dot * normal[0],
            co[1] - dot * normal[1],
            co[2] - dot * normal[2],
        ];
        for k in 0..3 {
            if i.lock[k] {
                projected[k] = co[k];
            }
        }
        // `math::interpolate(co, projected, factor)`.
        let t = i.factor;
        bm.vert_mut(v).co = [
            co[0] * (1.0 - t) + projected[0] * t,
            co[1] * (1.0 - t) + projected[1] * t,
            co[2] * (1.0 - t) + projected[2] * t,
        ];
    }
}

/// `BM_mesh_calc_face_groups(bm, …, nullptr, nullptr, nullptr, hflag_test = BM_ELEM_TAG,
/// htype_step = BM_EDGE)`: the groups of tagged faces connected by edges, in Blender's
/// visiting order (afterwards every face carries `BM_ELEM_TAG`).
pub(crate) fn calc_face_groups_tagged(bm: &mut BMesh) -> Vec<Vec<FaceId>> {
    let faces = bm.face_ids();
    let mut tot_faces = 0usize;
    for &f in &faces {
        let fl = &mut bm.face_mut(f).flag;
        if fl.contains(ElemFlag::TAG) {
            tot_faces += 1;
            fl.remove(ElemFlag::TAG);
        } else {
            // Never walk over tagged.
            fl.insert(ElemFlag::TAG);
        }
    }
    let mut groups = Vec::new();
    let mut next = 0usize;
    let mut tot_touch = 0usize;
    while tot_touch != tot_faces {
        let mut stack: Vec<FaceId> = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !bm.face(f).flag.contains(ElemFlag::TAG) {
                bm.face_mut(f).flag.insert(ElemFlag::TAG);
                stack.push(f);
                break;
            }
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            group.push(f);
            tot_touch += 1;
            for l_iter in bm.face_loops(f) {
                let mut l_radial = bm.lp(l_iter).radial_next;
                if l_radial == l_iter {
                    continue;
                }
                loop {
                    let fo = bm.lp(l_radial).f;
                    if !bm.face(fo).flag.contains(ElemFlag::TAG) {
                        bm.face_mut(fo).flag.insert(ElemFlag::TAG);
                        stack.push(fo);
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

/// `BM_mesh_calc_edge_groups(bm, …, nullptr, nullptr, hflag_test = BM_ELEM_TAG)`: the groups
/// of tagged edges connected by vertices.
pub(crate) fn calc_edge_groups_tagged(bm: &mut BMesh) -> Vec<Vec<EdgeId>> {
    let edges = bm.edge_ids();
    let mut tot_edges = 0usize;
    for &e in &edges {
        let fl = &mut bm.edge_mut(e).flag;
        if fl.contains(ElemFlag::TAG) {
            tot_edges += 1;
            fl.remove(ElemFlag::TAG);
        } else {
            fl.insert(ElemFlag::TAG);
        }
    }
    let mut groups = Vec::new();
    let mut next = 0usize;
    let mut tot_touch = 0usize;
    while tot_touch != tot_edges {
        let mut stack: Vec<EdgeId> = Vec::new();
        while next < edges.len() {
            let e = edges[next];
            next += 1;
            if !bm.edge(e).flag.contains(ElemFlag::TAG) {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                stack.push(e);
                break;
            }
        }
        let mut group = Vec::new();
        while let Some(e) = stack.pop() {
            group.push(e);
            tot_touch += 1;
            for v in bm.edge(e).v {
                for eo in bm.vert_edges(v) {
                    if !bm.edge(eo).flag.contains(ElemFlag::TAG) {
                        bm.edge_mut(eo).flag.insert(ElemFlag::TAG);
                        stack.push(eo);
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// `BMO_slot_buffer_hflag_enable(bm, slots_in, "geom", htype, BM_ELEM_TAG, false)` after
/// `BM_mesh_elem_hflag_disable_all(bm, htype, BM_ELEM_TAG, false)`.
pub(crate) fn tag_geom(bm: &mut BMesh, geom: &[BMElem], verts: bool, edges: bool, faces: bool) {
    if verts {
        for v in bm.verts.slots.iter_mut().flatten() {
            v.flag.remove(ElemFlag::TAG);
        }
    }
    if edges {
        for e in bm.edges.slots.iter_mut().flatten() {
            e.flag.remove(ElemFlag::TAG);
        }
    }
    if faces {
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
    }
    for &g in geom {
        let on = match g {
            BMElem::Vert(_) => verts,
            BMElem::Edge(_) => edges,
            BMElem::Face(_) => faces,
        };
        if on && bm.elem_alive(g) {
            bm.elem_flag_mut(g).insert(ElemFlag::TAG);
        }
    }
}

/// `bmo_flatten_exec`.
pub(crate) fn flatten(bm: &mut BMesh, i: &FlattenIn) {
    tag_geom(bm, &i.geom, false, true, true);
    let face_groups = calc_face_groups_tagged(bm);
    for faces in &face_groups {
        // Untag the edges of the faces: the edges still tagged afterwards have no selected
        // face and are found by the second pass.
        for &f in faces {
            for e in bm.face_edges(f) {
                bm.edge_mut(e).flag.remove(ElemFlag::TAG);
            }
        }
        // `collect_verts_from_faces` (a `VectorSet`: first occurrence order).
        let mut verts: Vec<VertId> = Vec::new();
        let mut seen = vec![false; bm.verts.capacity()];
        for &f in faces {
            for v in bm.face_verts(f) {
                if !seen[v.slot()] {
                    seen[v.slot()] = true;
                    verts.push(v);
                }
            }
        }
        flatten_verts(bm, i, &verts, faces);
    }
    let edge_groups = calc_edge_groups_tagged(bm);
    for edges in &edge_groups {
        let mut verts: Vec<VertId> = Vec::new();
        let mut seen = vec![false; bm.verts.capacity()];
        for &e in edges {
            for v in bm.edge(e).v {
                if !seen[v.slot()] {
                    seen[v.slot()] = true;
                    verts.push(v);
                }
            }
        }
        flatten_verts(bm, i, &verts, &[]);
    }
}
