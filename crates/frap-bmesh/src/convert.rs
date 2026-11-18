// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh` ⇄ `BMesh` (`BM_mesh_bm_from_me` / `BM_mesh_bm_to_me`, `bmesh/intern/
//! bmesh_mesh_convert.cc`; ARCHITECTURE §6.2, [MESH §2.5]). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1.
//!
//! `from_mesh` keeps element order (slot = index), reads selection from `.select_*`, hiding from
//! `.hide_*`, flags from `sharp_face`/`sharp_edge`/`uv_seam`, and every other attribute into a
//! CustomData layer. `to_mesh` writes in iteration order and is the **only** caller of
//! `Mesh::remove_all_false_flags` (all-false `sharp_face`/`sharp_edge`/`uv_seam` are removed);
//! check the attribute order after a round trip (Blender 5.2.4 reorders: `my_flag`, `UVMap`,
//! `position`). Shape keys: the basis propagates offsets to the other keys.

use frap_mesh::Mesh;

use crate::types::BMesh;

/// `BMeshFromMeshParams`.
#[derive(Clone, Debug, PartialEq)]
pub struct FromMeshParams {
    /// `calc_face_normal`.
    pub calc_face_normal: bool,
    /// `calc_vert_normal`.
    pub calc_vert_normal: bool,
    /// `use_shapekey`: load the key blocks as shape layers.
    pub use_shapekey: bool,
    /// `active_shapekey` (1-based, 0 = basis).
    pub active_shapekey: i32,
}

impl Default for FromMeshParams {
    /// Edit-mode entry (`EDBM_mesh_make`): normals computed, shape keys loaded.
    fn default() -> Self {
        FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: true,
            active_shapekey: 0,
        }
    }
}

/// `BMeshToMeshParams`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToMeshParams {
    /// `calc_object_remap` (unused by frap: no hooks/parent vertices).
    pub calc_object_remap: bool,
    /// `update_shapekey_indices`.
    pub update_shapekey_indices: bool,
}

impl BMesh {
    /// `BM_mesh_bm_from_me`.
    pub fn from_mesh(me: &Mesh, params: &FromMeshParams) -> BMesh {
        let _ = (me, params);
        todo!("WP 2.1: BM_mesh_bm_from_me")
    }
    /// `BM_mesh_bm_to_me`: the mesh of this BMesh; non-geometry data (name, materials, active
    /// layers, texture space, shape-key pointer) comes from `template`.
    pub fn to_mesh(&self, template: &Mesh, params: &ToMeshParams) -> Mesh {
        let _ = (template, params);
        todo!("WP 2.1: BM_mesh_bm_to_me")
    }
}
