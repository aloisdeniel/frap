// SPDX-License-Identifier: GPL-3.0-or-later
//! Walkers (`bmesh/intern/bmesh_walkers_impl.cc`; ARCHITECTURE §6.2): edge loop, edge ring,
//! face island and connected shell, visiting elements in Blender's order. CONTRACT frozen by
//! M2 W0, implemented by WP 2.1.

use crate::types::{BMesh, EdgeId, FaceId, VertId};

/// Which elements a walker may cross (`BMW_MASK_NOP` and the delimit flags of
/// `mesh.select_linked`: `SEAM`, `SHARP`, `MATERIAL`, `NORMAL`, `UV`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct WalkDelimit {
    /// Stop at seams.
    pub seam: bool,
    /// Stop at sharp edges.
    pub sharp: bool,
    /// Stop between faces of different materials.
    pub material: bool,
    /// Stop between faces whose normals differ.
    pub normal: bool,
    /// Stop at UV boundaries.
    pub uv: bool,
}

impl BMesh {
    /// `BMW_EDGELOOP` from `e`.
    pub fn walk_edge_loop(&self, e: EdgeId) -> Vec<EdgeId> {
        let _ = e;
        todo!("WP 2.1: BMW_EDGELOOP")
    }
    /// `BMW_EDGERING` from `e`.
    pub fn walk_edge_ring(&self, e: EdgeId) -> Vec<EdgeId> {
        let _ = e;
        todo!("WP 2.1: BMW_EDGERING")
    }
    /// `BMW_ISLAND` (faces connected over non-delimited edges) from `f`.
    pub fn walk_face_island(&self, f: FaceId, delimit: WalkDelimit) -> Vec<FaceId> {
        let _ = (f, delimit);
        todo!("WP 2.1: BMW_ISLAND")
    }
    /// `BMW_VERT_SHELL` (vertices connected by edges) from `v`.
    pub fn walk_vert_shell(&self, v: VertId) -> Vec<VertId> {
        let _ = v;
        todo!("WP 2.1: BMW_VERT_SHELL")
    }
}
