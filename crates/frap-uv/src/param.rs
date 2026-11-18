// SPDX-License-Identifier: GPL-3.0-or-later
//! [`ParamMesh`]: the port of Blender's `ParamHandle` and its chart construction
//! (`uv_parametrizer_*`, `blender/source/blender/geometry/intern/uv_parametrizer.cc`).
//!
//! Life cycle, as in Blender: [`ParamMesh::new`] → [`ParamMesh::face_add`] /
//! [`ParamMesh::edge_set_seam`] → [`ParamMesh::construct_end`] (splits into charts along the
//! seams) → a solver ([`ParamMesh::lscm_begin`] / [`ParamMesh::lscm_solve`] /
//! [`ParamMesh::lscm_end`], [`ParamMesh::average`], [`ParamMesh::pack`], …) →
//! [`ParamMesh::flush`] writes every chart's UVs into the caller's corner UV array at the
//! indices given to `face_add`. Stub bodies until WP 3.3.

use crate::pack::PackParams;

/// Key identifying a vertex or a face (`ParamKey`: Blender passes BMesh element indices).
pub type ParamKey = u64;

/// `PHandleState`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PhState {
    /// `PHANDLE_STATE_ALLOCATED`: faces and seams may be added.
    #[default]
    Allocated,
    /// `PHANDLE_STATE_CONSTRUCTED`: charts built.
    Constructed,
    /// `PHANDLE_STATE_LSCM`: between `lscm_begin` and `lscm_end`.
    Lscm,
    /// `PHANDLE_STATE_STRETCH`: between `stretch_begin` and `stretch_end`.
    Stretch,
}

/// `uv_parametrizer_construct_end` options.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConstructOpts {
    /// `fill_holes` (unwrap's `fill_holes`).
    pub fill_holes: bool,
    /// `topology_from_uvs` (charts from existing UV islands instead of seams).
    pub topology_from_uvs: bool,
}

/// Changed / failed chart counts of a solve (`count_changed`, `count_failed`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SolveCount {
    /// Charts solved.
    pub changed: u32,
    /// Charts whose solve failed (Blender keeps their UVs).
    pub failed: u32,
}

/// One face handed to [`ParamMesh::face_add`] (`uv_parametrizer_face_add`'s arguments).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ParamFace<'a> {
    /// Face key.
    pub key: ParamKey,
    /// Vertex keys, in corner order.
    pub vkeys: &'a [ParamKey],
    /// Vertex coordinates per corner.
    pub co: &'a [[f32; 3]],
    /// Index into the caller's corner UV array per corner (where `flush` writes).
    pub uv: &'a [u32],
    /// Initial UV per corner (read for pins and `topology_from_uvs`).
    pub uv_init: &'a [[f32; 2]],
    /// Optional per-corner weight (SLIM `weight_influence`).
    pub weight: Option<&'a [f32]>,
    /// Pinned per corner.
    pub pin: &'a [bool],
    /// Selected per corner.
    pub select: &'a [bool],
}

/// Blender's `ParamHandle`.
#[derive(Debug, Default)]
pub struct ParamMesh {
    /// Construction state.
    pub state: PhState,
    /// `aspect_y` (`uv_parametrizer_aspect_ratio`).
    pub aspect_y: f32,
}

impl ParamMesh {
    /// `ParamHandle::ParamHandle()`.
    pub fn new() -> ParamMesh {
        ParamMesh {
            state: PhState::Allocated,
            aspect_y: 1.0,
        }
    }

    /// `uv_parametrizer_aspect_ratio`.
    pub fn set_aspect_ratio(&mut self, aspect_y: f32) {
        self.aspect_y = aspect_y;
    }

    /// `uv_parametrizer_face_add` (n-gons are triangulated with Blender's polyfill + beautify).
    pub fn face_add(&mut self, face: &ParamFace<'_>) {
        let _ = face;
        todo!("WP 3.3: uv_parametrizer_face_add")
    }

    /// `uv_parametrizer_edge_set_seam`.
    pub fn edge_set_seam(&mut self, vkeys: [ParamKey; 2]) {
        let _ = vkeys;
        todo!("WP 3.3: uv_parametrizer_edge_set_seam")
    }

    /// `uv_parametrizer_construct_end`: returns the number of charts that failed (`count_fail`).
    pub fn construct_end(&mut self, opts: ConstructOpts) -> u32 {
        let _ = opts;
        todo!("WP 3.3: uv_parametrizer_construct_end")
    }

    /// Number of charts after construction.
    pub fn chart_count(&self) -> usize {
        todo!("WP 3.3: chart count")
    }

    /// `uv_parametrizer_lscm_begin(live, abf, use_original_bounds)`.
    pub fn lscm_begin(&mut self, live: bool, abf: bool, use_original_bounds: bool) {
        let _ = (live, abf, use_original_bounds);
        todo!("WP 3.3: uv_parametrizer_lscm_begin")
    }

    /// `uv_parametrizer_lscm_solve`.
    pub fn lscm_solve(&mut self) -> SolveCount {
        todo!("WP 3.3: uv_parametrizer_lscm_solve")
    }

    /// `uv_parametrizer_lscm_end`.
    pub fn lscm_end(&mut self) {
        todo!("WP 3.3: uv_parametrizer_lscm_end")
    }

    /// `uv_parametrizer_average(ignore_pinned, scale_uv, shear)`.
    pub fn average(&mut self, ignore_pinned: bool, scale_uv: bool, shear: bool) {
        let _ = (ignore_pinned, scale_uv, shear);
        todo!("WP 3.3: uv_parametrizer_average")
    }

    /// `uv_parametrizer_pack`.
    pub fn pack(&mut self, params: &PackParams) {
        let _ = params;
        todo!("WP 3.3: uv_parametrizer_pack")
    }

    /// `uv_parametrizer_original_bounds`.
    pub fn original_bounds(&mut self) {
        todo!("WP 3.3: uv_parametrizer_original_bounds")
    }

    /// `uv_parametrizer_flush`: writes every chart's UVs into `uvs` at the indices given to
    /// [`ParamMesh::face_add`].
    pub fn flush(&self, uvs: &mut [[f32; 2]]) {
        let _ = uvs;
        todo!("WP 3.3: uv_parametrizer_flush")
    }

    /// `uv_parametrizer_flush_restore`: writes back the UVs read at construction.
    pub fn flush_restore(&self, uvs: &mut [[f32; 2]]) {
        let _ = uvs;
        todo!("WP 3.3: uv_parametrizer_flush_restore")
    }
}
