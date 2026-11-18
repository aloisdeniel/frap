// SPDX-License-Identifier: GPL-3.0-or-later
//! Kernel Euler operators with Blender's names, so ports are 1:1 and creation order matches
//! (`bmesh/intern/bmesh_core.cc`, `bmesh_mods.cc`, `bmesh_delete.cc`; ARCHITECTURE §6.2).
//! CONTRACT frozen by M2 W0, implemented by WP 2.1. Every function keeps the disk and radial
//! cycles valid (`validate.rs`) and allocates and frees in Blender's order.

use crate::types::{BMElem, BMesh, CreateFlag, EdgeId, ElemTypes, FaceId, LoopId, VertId};

impl BMesh {
    /// `BM_vert_create`: a vertex at `co`, custom data copied from `example` (else defaults).
    pub fn vert_create(
        &mut self,
        co: [f32; 3],
        example: Option<VertId>,
        flag: CreateFlag,
    ) -> VertId {
        let _ = (co, example, flag);
        todo!("WP 2.1: BM_vert_create")
    }
    /// `BM_edge_create`: an edge `v1`–`v2` (the existing one with `NO_DOUBLE`).
    pub fn edge_create(
        &mut self,
        v1: VertId,
        v2: VertId,
        example: Option<EdgeId>,
        flag: CreateFlag,
    ) -> EdgeId {
        let _ = (v1, v2, example, flag);
        todo!("WP 2.1: BM_edge_create")
    }
    /// `BM_face_create`: a face over `verts` with the given `edges` (`edges[i]` joins
    /// `verts[i]` and `verts[i + 1]`); `None` when the input is degenerate.
    pub fn face_create(
        &mut self,
        verts: &[VertId],
        edges: &[EdgeId],
        example: Option<FaceId>,
        flag: CreateFlag,
    ) -> Option<FaceId> {
        let _ = (verts, edges, example, flag);
        todo!("WP 2.1: BM_face_create")
    }
    /// `BM_face_create_verts`: a face over `verts`, creating missing edges when
    /// `create_edges` (in `(v[n-1], v[0]), (v[0], v[1]), …` order).
    pub fn face_create_verts(
        &mut self,
        verts: &[VertId],
        example: Option<FaceId>,
        flag: CreateFlag,
        create_edges: bool,
    ) -> Option<FaceId> {
        let _ = (verts, example, flag, create_edges);
        todo!("WP 2.1: BM_face_create_verts")
    }
    /// `BM_vert_kill`: the vertex and every edge and face using it.
    pub fn vert_kill(&mut self, v: VertId) {
        let _ = v;
        todo!("WP 2.1: BM_vert_kill")
    }
    /// `BM_edge_kill`: the edge and every face using it.
    pub fn edge_kill(&mut self, e: EdgeId) {
        let _ = e;
        todo!("WP 2.1: BM_edge_kill")
    }
    /// `BM_face_kill`: the face and its loops (edges and vertices stay).
    pub fn face_kill(&mut self, f: FaceId) {
        let _ = f;
        todo!("WP 2.1: BM_face_kill")
    }
    /// `bmesh_kernel_split_edge_make_vert` (SEMV): splits `e` next to `tv`; returns the new
    /// vertex and the new edge.
    pub fn split_edge_make_vert(&mut self, tv: VertId, e: EdgeId) -> (VertId, EdgeId) {
        let _ = (tv, e);
        todo!("WP 2.1: bmesh_kernel_split_edge_make_vert")
    }
    /// `bmesh_kernel_join_edge_kill_vert` (JEKV): removes `v_kill` (valence 2) and `e_kill`,
    /// joining the other edge over them; returns the surviving edge.
    pub fn join_edge_kill_vert(
        &mut self,
        e_kill: EdgeId,
        v_kill: VertId,
        do_del: bool,
        check_edge_exists: bool,
        kill_degenerate_faces: bool,
        kill_duplicate_faces: bool,
    ) -> Option<EdgeId> {
        let _ = (e_kill, v_kill, do_del, check_edge_exists);
        let _ = (kill_degenerate_faces, kill_duplicate_faces);
        todo!("WP 2.1: bmesh_kernel_join_edge_kill_vert")
    }
    /// `bmesh_kernel_split_face_make_edge` (SFME): splits `f` between the corners `l_v1` and
    /// `l_v2`; returns the new face and its loop on the new edge.
    pub fn split_face_make_edge(
        &mut self,
        f: FaceId,
        l_v1: LoopId,
        l_v2: LoopId,
        example: Option<EdgeId>,
        no_double: bool,
    ) -> Option<(FaceId, LoopId)> {
        let _ = (f, l_v1, l_v2, example, no_double);
        todo!("WP 2.1: bmesh_kernel_split_face_make_edge")
    }
    /// `bmesh_kernel_join_face_kill_edge` (JFKE): joins `f1` and `f2` over `e`; returns the
    /// surviving face.
    pub fn join_face_kill_edge(&mut self, f1: FaceId, f2: FaceId, e: EdgeId) -> Option<FaceId> {
        let _ = (f1, f2, e);
        todo!("WP 2.1: bmesh_kernel_join_face_kill_edge")
    }
    /// `BM_vert_splice`: merges `v_src` into `v_dst` (no shared edge allowed).
    pub fn vert_splice(&mut self, v_dst: VertId, v_src: VertId) -> bool {
        let _ = (v_dst, v_src);
        todo!("WP 2.1: BM_vert_splice")
    }
    /// `BM_edge_splice`: merges `e_src` into `e_dst` (same vertices).
    pub fn edge_splice(&mut self, e_dst: EdgeId, e_src: EdgeId) -> bool {
        let _ = (e_dst, e_src);
        todo!("WP 2.1: BM_edge_splice")
    }
    /// `bmesh_kernel_loop_reverse` / `BM_face_normal_flip`: reverses the winding, keeping
    /// `l_first` ([MESH §4.9]).
    pub fn face_normal_flip(&mut self, f: FaceId) {
        let _ = f;
        todo!("WP 2.1: BM_face_normal_flip")
    }
    /// `BM_vert_separate`: splits `v` into one vertex per edge fan (restricted to `e_in` when
    /// not empty); returns the vertices (the original first).
    pub fn vert_separate(&mut self, v: VertId, e_in: &[EdgeId], copy_select: bool) -> Vec<VertId> {
        let _ = (v, e_in, copy_select);
        todo!("WP 2.1: BM_vert_separate")
    }
    /// `BM_mesh_elem_index_ensure`: dense indices (iteration order) of the given element
    /// types, read with [`BMesh::index_of`].
    pub fn elem_index_ensure(&mut self, which: ElemTypes) {
        let _ = which;
        todo!("WP 2.1: BM_mesh_elem_index_ensure")
    }
    /// The dense index (`BM_elem_index_get`) of an element after `elem_index_ensure`.
    pub fn index_of(&self, e: BMElem) -> usize {
        let _ = e;
        todo!("WP 2.1: BM_elem_index_get")
    }
    /// `BM_mesh_delete_hflag_context` core (`bmesh_delete.cc`): deletes `geom` with a
    /// `bmesh.ops.delete` context (`VERTS`, `EDGES`, `FACES_ONLY`, `EDGES_FACES`, `FACES`,
    /// `FACES_KEEP_BOUNDARY`, `TAGGED_ONLY`) in Blender's kill order.
    pub fn delete_context(&mut self, geom: &[BMElem], context: &str) {
        let _ = (geom, context);
        todo!("WP 2.1: BM_mesh_delete_hflag_context")
    }
}
