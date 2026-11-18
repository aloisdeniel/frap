// SPDX-License-Identifier: GPL-3.0-or-later
//! CustomData: typed columns by element slot, and interpolation (`blenkernel/intern/
//! customdata.cc`, `bmesh/intern/bmesh_interp.cc`; ARCHITECTURE §6.2). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1.
//!
//! A layer is a column with one value per **pool slot** of its domain (free slots hold
//! garbage), stored as a `frap_mesh::AttrData` so conversion to and from `Mesh` attributes is a
//! gather/scatter. Layers keep the order `BM_mesh_bm_from_me` creates them in.

use frap_mesh::AttrData;
use smol_str::SmolStr;

use crate::types::{BMesh, FaceId, LoopId, VertId};

/// One layer.
#[derive(Clone, Debug)]
pub struct CdLayer {
    /// Attribute name (`UVMap`, `crease_edge`, `.select_vert`, …).
    pub name: SmolStr,
    /// Values by element slot.
    pub data: AttrData,
}

/// The layers of one domain.
#[derive(Clone, Debug, Default)]
pub struct CustomData {
    /// Layers in creation order.
    pub layers: Vec<CdLayer>,
}

impl CustomData {
    /// Position of the layer named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }
    /// Grows every column to `slots` values (new slots get the type's default).
    pub fn ensure_slots(&mut self, slots: usize) {
        let _ = slots;
        todo!("WP 2.1: CustomData::ensure_slots")
    }
    /// `CustomData_bmesh_copy_data`: copies every layer's value from slot `src` to slot `dst`.
    pub fn copy_elem(&mut self, src: u32, dst: u32) {
        let _ = (src, dst);
        todo!("WP 2.1: CustomData_bmesh_copy_data")
    }
    /// `CustomData_bmesh_set_default`: the type defaults in slot `i`.
    pub fn set_default(&mut self, i: u32) {
        let _ = i;
        todo!("WP 2.1: CustomData_bmesh_set_default")
    }
    /// `CustomData_bmesh_interp`: slot `dst` = Σ `weights[k]` · slot `src[k]` (types that do
    /// not interpolate take the first source).
    pub fn interp(&mut self, src: &[u32], weights: &[f32], dst: u32) {
        let _ = (src, weights, dst);
        todo!("WP 2.1: CustomData_bmesh_interp")
    }
}

impl BMesh {
    /// `BM_loop_interp_from_face`: the loop data of `l_dst` interpolated from `f_src`'s corners
    /// at `l_dst`'s position (mean-value coordinates, `do_vertex` also interpolates the vertex
    /// data).
    pub fn loop_interp_from_face(&mut self, l_dst: LoopId, f_src: FaceId, do_vertex: bool) {
        let _ = (l_dst, f_src, do_vertex);
        todo!("WP 2.1: BM_loop_interp_from_face")
    }
    /// `BM_data_interp_from_verts`: `v_dst` data = mix of `v_src1`/`v_src2` by `fac`.
    pub fn data_interp_from_verts(
        &mut self,
        v_src1: VertId,
        v_src2: VertId,
        v_dst: VertId,
        fac: f32,
    ) {
        let _ = (v_src1, v_src2, v_dst, fac);
        todo!("WP 2.1: BM_data_interp_from_verts")
    }
}
