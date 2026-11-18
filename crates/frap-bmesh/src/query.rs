// SPDX-License-Identifier: GPL-3.0-or-later
//! Queries (`bmesh/intern/bmesh_query.cc`, `bmesh_iterators.cc`; ARCHITECTURE §6.2). CONTRACT
//! frozen by M2 W0, implemented by WP 2.1. Iterators follow Blender's cycle order (disk cycle
//! from `v->e`, radial cycle from `e->l`, face loops from `l_first`), so ports keep
//! Blender's visiting order.

use crate::types::{BMesh, EdgeId, FaceId, LoopId, VertId};

impl BMesh {
    /// `BM_ITER_ELEM(e, v, BM_EDGES_OF_VERT)`: the disk cycle of `v`.
    pub fn vert_edges(&self, v: VertId) -> Vec<EdgeId> {
        let _ = v;
        todo!("WP 2.1: BM_EDGES_OF_VERT")
    }
    /// `BM_FACES_OF_VERT`.
    pub fn vert_faces(&self, v: VertId) -> Vec<FaceId> {
        let _ = v;
        todo!("WP 2.1: BM_FACES_OF_VERT")
    }
    /// `BM_LOOPS_OF_VERT`.
    pub fn vert_loops(&self, v: VertId) -> Vec<LoopId> {
        let _ = v;
        todo!("WP 2.1: BM_LOOPS_OF_VERT")
    }
    /// `BM_LOOPS_OF_EDGE`: the radial cycle of `e`.
    pub fn edge_loops(&self, e: EdgeId) -> Vec<LoopId> {
        let _ = e;
        todo!("WP 2.1: BM_LOOPS_OF_EDGE")
    }
    /// `BM_FACES_OF_EDGE`.
    pub fn edge_faces(&self, e: EdgeId) -> Vec<FaceId> {
        let _ = e;
        todo!("WP 2.1: BM_FACES_OF_EDGE")
    }
    /// `BM_LOOPS_OF_FACE`, from `l_first`.
    pub fn face_loops(&self, f: FaceId) -> Vec<LoopId> {
        let _ = f;
        todo!("WP 2.1: BM_LOOPS_OF_FACE")
    }
    /// `BM_VERTS_OF_FACE`, from `l_first`.
    pub fn face_verts(&self, f: FaceId) -> Vec<VertId> {
        let _ = f;
        todo!("WP 2.1: BM_VERTS_OF_FACE")
    }
    /// `BM_EDGES_OF_FACE`, from `l_first`.
    pub fn face_edges(&self, f: FaceId) -> Vec<EdgeId> {
        let _ = f;
        todo!("WP 2.1: BM_EDGES_OF_FACE")
    }
    /// `BM_edge_exists`.
    pub fn edge_exists(&self, a: VertId, b: VertId) -> Option<EdgeId> {
        let _ = (a, b);
        todo!("WP 2.1: BM_edge_exists")
    }
    /// `BM_face_exists`: a face using exactly `verts` (any rotation, same winding).
    pub fn face_exists(&self, verts: &[VertId]) -> Option<FaceId> {
        let _ = verts;
        todo!("WP 2.1: BM_face_exists")
    }
    /// `BM_edge_other_vert`.
    pub fn edge_other_vert(&self, e: EdgeId, v: VertId) -> VertId {
        let _ = (e, v);
        todo!("WP 2.1: BM_edge_other_vert")
    }
    /// `BM_edge_is_manifold`: exactly two faces.
    pub fn edge_is_manifold(&self, e: EdgeId) -> bool {
        let _ = e;
        todo!("WP 2.1: BM_edge_is_manifold")
    }
    /// `BM_edge_is_boundary`: exactly one face.
    pub fn edge_is_boundary(&self, e: EdgeId) -> bool {
        let _ = e;
        todo!("WP 2.1: BM_edge_is_boundary")
    }
    /// `BM_edge_is_wire`: no face.
    pub fn edge_is_wire(&self, e: EdgeId) -> bool {
        let _ = e;
        todo!("WP 2.1: BM_edge_is_wire")
    }
    /// `BM_vert_is_manifold`.
    pub fn vert_is_manifold(&self, v: VertId) -> bool {
        let _ = v;
        todo!("WP 2.1: BM_vert_is_manifold")
    }
    /// `BM_vert_edge_count`.
    pub fn vert_edge_count(&self, v: VertId) -> usize {
        let _ = v;
        todo!("WP 2.1: BM_vert_edge_count")
    }
    /// `BM_face_calc_normal` (Newell, triangle/quad special cases: `frap_geom::normals`).
    pub fn face_calc_normal(&self, f: FaceId) -> [f32; 3] {
        let _ = f;
        todo!("WP 2.1: BM_face_calc_normal")
    }
    /// `BM_face_calc_center_median`.
    pub fn face_calc_center_median(&self, f: FaceId) -> [f32; 3] {
        let _ = f;
        todo!("WP 2.1: BM_face_calc_center_median")
    }
    /// `BM_mesh_normals_update`: face normals, then angle-weighted vertex normals.
    pub fn normals_update(&mut self) {
        todo!("WP 2.1: BM_mesh_normals_update")
    }
}
