// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bevel` (Blender 5.2.4 `bmo_bevel`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.11 (Bevel).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bevel(bmesh, geom=[], offset=0.0, offset_type='OFFSET', profile_type='SUPERELLIPSE', segments=0, profile=0.0, affect='VERTICES', clamp_overlap=False, material=0, loop_slide=False, mark_seam=False, mark_sharp=False, harden_normals=False, face_strength_mode='NONE', miter_outer='SHARP', miter_inner='SHARP', spread=0.0, custom_profile=None, vmesh_method='ADJ')`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `bevel` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct BevelIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `offset`.
    pub offset: f32,
    /// `offset_type` (enum identifier).
    pub offset_type: &'static str,
    /// `profile_type` (enum identifier).
    pub profile_type: &'static str,
    /// `segments`.
    pub segments: i32,
    /// `profile`.
    pub profile: f32,
    /// `affect` (enum identifier).
    pub affect: &'static str,
    /// `clamp_overlap`.
    pub clamp_overlap: bool,
    /// `material`.
    pub material: i32,
    /// `loop_slide`.
    pub loop_slide: bool,
    /// `mark_seam`.
    pub mark_seam: bool,
    /// `mark_sharp`.
    pub mark_sharp: bool,
    /// `harden_normals`.
    pub harden_normals: bool,
    /// `face_strength_mode` (enum identifier).
    pub face_strength_mode: &'static str,
    /// `miter_outer` (enum identifier).
    pub miter_outer: &'static str,
    /// `miter_inner` (enum identifier).
    pub miter_inner: &'static str,
    /// `spread`.
    pub spread: f32,
    /// `custom_profile` (custom profile curve (unsupported)).
    pub custom_profile: Option<()>,
    /// `vmesh_method` (enum identifier).
    pub vmesh_method: &'static str,
}

impl Default for BevelIn {
    fn default() -> Self {
        BevelIn {
            geom: Vec::new(),
            offset: 0.0,
            offset_type: "OFFSET",
            profile_type: "SUPERELLIPSE",
            segments: 0,
            profile: 0.0,
            affect: "VERTICES",
            clamp_overlap: false,
            material: 0,
            loop_slide: false,
            mark_seam: false,
            mark_sharp: false,
            harden_normals: false,
            face_strength_mode: "NONE",
            miter_outer: "SHARP",
            miter_inner: "SHARP",
            spread: 0.0,
            custom_profile: None,
            vmesh_method: "ADJ",
        }
    }
}

/// Output slots of `bevel` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BevelOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `bevel` on `bm`.
pub fn bevel(bm: &mut BMesh, i: &BevelIn) -> Result<BevelOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.11: bmesh op bevel")
}
