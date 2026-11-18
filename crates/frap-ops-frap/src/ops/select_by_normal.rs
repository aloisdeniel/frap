// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.select_by_normal(direction=(0, 0, 1), threshold=0.523599,
//! orientation='GLOBAL', extend=False)`, tier T1 (`tests/golden/cases/edit/select/
//! by_normal.py`): frap's selection by face orientation for agents, which cannot click
//! (ARCHITECTURE §22.4, OPERATORS.md; spec `spec/frap/ops/frap.select_by_normal.json`).
//! Blender has no such operator: goldens run `tools/blender/emulations.py::select_by_normal`
//! on the Blender side, which this operator follows step by step:
//! 1. `direction` is normalised in double precision; a NaN or infinite component or the zero
//!    vector raises `ValueError`;
//! 2. for every mesh in Edit Mode (each mesh once, active object first): without `extend`
//!    every element is deselected and the selection history cleared;
//! 3. each visible face whose normal `n` satisfies `dot(n, direction) >= cos(threshold)` is
//!    selected (`BMFace.select_set(True)`: its edges and vertices too). With `GLOBAL`, `n` is
//!    the face normal in world space (the inverse transpose of the object's 3x3 world matrix
//!    applied to the local normal, then normalised; an object with a singular matrix selects
//!    nothing); with `LOCAL` it is the local normal (each object's own axes);
//! 4. the selection is flushed for the select mode (`bm.select_flush_mode()`).
//!
//! `bpy.ops.frap.select_by_normal(direction=(0, 0, 1))` selects the faces pointing up.

use frap_bmesh::{BMesh, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{PyExc, RnaError, RnaErrorKind};
use frap_spec_ops_frap::frap::{self, select_by_normal::Orientation};
use glam::{Mat3, Mat4, Vec3};

/// Why a `direction` cannot be normalised.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BadDirection {
    /// A NaN or infinite component.
    NotFinite,
    /// All components zero.
    Zero,
}

impl BadDirection {
    fn message(self) -> &'static str {
        match self {
            BadDirection::NotFinite => "frap.select_by_normal: direction must be finite",
            BadDirection::Zero => "frap.select_by_normal: direction must not be a zero vector",
        }
    }
}

/// Normalises `v` in `f64` (so huge or tiny finite components neither overflow nor underflow
/// the length), rejecting non-finite components and the exact zero vector.
fn normalized(v: [f32; 3]) -> Result<Vec3, BadDirection> {
    if !v.iter().all(|c| c.is_finite()) {
        return Err(BadDirection::NotFinite);
    }
    let d = v.map(f64::from);
    let len = d.iter().map(|c| c * c).sum::<f64>().sqrt();
    if len == 0.0 {
        return Err(BadDirection::Zero);
    }
    Ok(Vec3::from(d.map(|c| (c / len) as f32)))
}

/// The matrix taking local face normals to world space (`None` when the object's 3x3 world
/// matrix is singular).
pub(crate) fn normal_matrix(world: &Mat4) -> Option<Mat3> {
    let m = Mat3::from_mat4(*world);
    let det = m.determinant();
    (det != 0.0 && det.is_finite()).then(|| m.inverse().transpose())
}

/// Selects the visible faces of `bm` facing `dir` (unit) within `limit` = `cos(threshold)`;
/// `normal_to_space` maps local normals (`None`: no face matches). Deselects everything first
/// unless `extend`. Returns the number of faces matched.
pub(crate) fn select_facing(
    bm: &mut BMesh,
    dir: Vec3,
    limit: f32,
    normal_to_space: Option<Mat3>,
    extend: bool,
) -> usize {
    if !extend {
        bm.deselect_all();
    }
    let mut n_matched = 0;
    if let Some(m) = normal_to_space {
        for f in bm.face_ids() {
            if bm.face(f).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            let n = m * Vec3::from(bm.face(f).no);
            let len = n.length();
            if len == 0.0 {
                continue;
            }
            if (n / len).dot(dir) >= limit {
                bm.face_select_set(f, true);
                n_matched += 1;
            }
        }
    }
    bm.select_flush_mode();
    n_matched
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "frap extension: selects the visible faces whose normal is within threshold of direction (GLOBAL: world space; LOCAL: each object's local space)",
        "every mesh in Edit Mode takes part; without extend the selection is replaced",
        "goldens run tools/blender/emulations.py::select_by_normal on the Blender side",
    ]
)]
fn exec(cx: &mut OpCtx, p: frap::SelectByNormal) -> OpResult {
    let dir = normalized(p.direction.value).map_err(|bad| {
        OpError::Rna(RnaError::new(
            RnaErrorKind::Other,
            PyExc::ValueError,
            bad.message(),
        ))
    })?;
    let limit = p.threshold.value.cos();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let space = match p.orientation.value {
            Orientation::Local => Some(Mat3::IDENTITY),
            Orientation::Global => normal_matrix(&cx.evaluated(ob)?.matrix_world),
        };
        select_facing(cx.bmesh_mut(me)?, dir, limit, space, p.extend.value);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> BMesh {
        let mesh = frap_mesh::primitives::cube::cube_mesh(&Default::default());
        BMesh::from_mesh(&mesh, &Default::default())
    }

    #[test]
    fn selects_faces_within_the_angle() {
        let mut bm = cube();
        let up = Vec3::Z;
        let n = select_facing(&mut bm, up, (0.5f32).cos(), Some(Mat3::IDENTITY), false);
        assert_eq!(n, 1);
        assert_eq!(bm.selected_counts(), (4, 4, 1));
        // 90 degrees and a bit: the four side faces join (dot = 0 >= cos(1.6)).
        let n = select_facing(&mut bm, up, (1.6f32).cos(), Some(Mat3::IDENTITY), false);
        assert_eq!(n, 5);
    }

    #[test]
    fn world_space_uses_the_inverse_transpose() {
        let mut bm = cube();
        // Rotated 180 degrees about X: the local -Z face points up.
        let world = Mat4::from_rotation_x(std::f32::consts::PI);
        let m = normal_matrix(&world).unwrap();
        let n = select_facing(&mut bm, Vec3::Z, 0.9, Some(m), false);
        assert_eq!(n, 1);
        let f = bm
            .face_ids()
            .into_iter()
            .find(|&f| bm.is_selected(f.into()))
            .unwrap();
        assert!(bm.face(f).no[2] < -0.99);
        // A singular matrix selects nothing (and still replaces the selection).
        assert!(normal_matrix(&Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0))).is_none());
        assert_eq!(select_facing(&mut bm, Vec3::Z, 0.9, None, false), 0);
        assert_eq!(bm.selected_counts(), (0, 0, 0));
    }

    #[test]
    fn zero_direction_is_rejected() {
        assert_eq!(normalized([0.0; 3]), Err(BadDirection::Zero));
        assert_eq!(normalized([0.0, 0.0, 2.0]), Ok(Vec3::Z));
    }

    #[test]
    fn non_finite_and_extreme_directions() {
        assert_eq!(
            normalized([f32::NAN, 0.0, 1.0]),
            Err(BadDirection::NotFinite)
        );
        assert_eq!(
            normalized([0.0, f32::INFINITY, 0.0]),
            Err(BadDirection::NotFinite)
        );
        // f32 would overflow (1e20^2) or underflow (1e-30^2) the squared length.
        assert_eq!(normalized([0.0, 0.0, 1e20]), Ok(Vec3::Z));
        assert_eq!(normalized([0.0, 0.0, -f32::MAX]), Ok(Vec3::NEG_Z));
        assert_eq!(normalized([1e-30, 0.0, 0.0]), Ok(Vec3::X));
        assert_eq!(normalized([f32::from_bits(1), 0.0, 0.0]), Ok(Vec3::X));
    }
}
