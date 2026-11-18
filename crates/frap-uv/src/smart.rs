// SPDX-License-Identifier: GPL-3.0-or-later
//! Smart UV Project (`smart_project_exec` of `uvedit_unwrap_ops.cc`): groups faces by their
//! projection normals (`smart_uv_project_calculate_project_normals`, `angle_limit`,
//! `area_weight`), projects each group onto its normal's plane, then the caller packs the
//! islands ([`crate::pack`]). Layout equal to Blender's headless output [MESH App. B.5].
//! Stub bodies until WP 3.3.

/// One projection group: the normal and the faces projected along it (face indices in the
/// caller's order).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SmartGroup {
    /// Projection normal.
    pub normal: [f32; 3],
    /// Faces of the group.
    pub faces: Vec<u32>,
}

/// Groups `face_normals` (weighted by `face_areas`) into projection groups.
pub fn project_groups(
    face_normals: &[[f32; 3]],
    face_areas: &[f32],
    angle_limit: f32,
    area_weight: f32,
) -> Vec<SmartGroup> {
    let _ = (face_normals, face_areas, angle_limit, area_weight);
    todo!("WP 3.3: smart project groups")
}
