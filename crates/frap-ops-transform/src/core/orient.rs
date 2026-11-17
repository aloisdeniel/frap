// SPDX-License-Identifier: GPL-3.0-or-later
//! Transform orientations (`orient_type`, `orient_matrix`, `orient_matrix_type`): the space
//! matrix `spacemtx` whose columns are the X/Y/Z axes values and constraints are expressed in.
//! Ports of `ED_transform_calc_orientation_from_type_ex`, `gimbal_axis_object`,
//! `eulO_to_gimbal_axis` (`editors/transform/transform_orientations.cc`,
//! `blenlib/intern/math_rotation_c.cc`) and the orientation part of `initTransInfo`
//! (`transform_generics.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (object mode): LOCAL and NORMAL use the active object's normalized world
//! axes; PARENT the active object's parent's axes and GLOBAL without a parent; GIMBAL the
//! active object's Euler gimbal axes and GLOBAL for a quaternion or axis-angle object; VIEW the
//! inverse of the 3D View's view matrix (frap's virtual view stores Blender's `-b` value, so
//! the axes match); CURSOR the 3D cursor's rotation. An `orient_matrix` replaces the computed
//! matrix when `orient_type` (or the scene default GLOBAL) equals `orient_matrix_type`.

use frap_math::EulerOrder;
use frap_math::euler::eul_o_to_mat3;
use frap_math::matrix::RotationMode;
use glam::{Mat3, Vec3};

use crate::core::math::{axes_of, invert_m3_or_identity};

/// `orient_type` identifiers (`rna_enum_transform_orientation_items` + the custom matrix).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OrientType {
    /// `'GLOBAL'`
    Global,
    /// `'LOCAL'`
    Local,
    /// `'NORMAL'`
    Normal,
    /// `'GIMBAL'`
    Gimbal,
    /// `'VIEW'`
    View,
    /// `'CURSOR'`
    Cursor,
    /// `'PARENT'`
    Parent,
    /// `V3D_ORIENT_CUSTOM_MATRIX`: the operator's `orient_matrix`.
    CustomMatrix,
}

impl OrientType {
    /// From an `orient_type` identifier (unknown or custom orientation names read as GLOBAL,
    /// as Blender does for an index past the custom orientations).
    pub fn from_identifier(s: &str) -> OrientType {
        match s {
            "LOCAL" => OrientType::Local,
            "NORMAL" => OrientType::Normal,
            "GIMBAL" => OrientType::Gimbal,
            "VIEW" => OrientType::View,
            "CURSOR" => OrientType::Cursor,
            "PARENT" => OrientType::Parent,
            _ => OrientType::Global,
        }
    }
}

/// What the orientation of a transform depends on, gathered by the mode handler.
#[derive(Clone, Debug, Default)]
pub struct OrientSource {
    /// The active object's world matrix (3×3 part), if there is an active object.
    pub active_world: Option<Mat3>,
    /// The active object's gimbal axes (`gimbal_axis_object`), when it rotates with Euler
    /// angles.
    pub active_gimbal: Option<Mat3>,
    /// The world matrix (3×3 part) of the active object's parent, if it has one.
    pub active_parent_world: Option<Mat3>,
    /// The 3D cursor's rotation matrix.
    pub cursor: Mat3,
    /// The 3D View's view matrix (3×3 part); its inverse gives the VIEW axes.
    pub view: Mat3,
}

/// A resolved orientation: the type actually used (falling back to GLOBAL where Blender does)
/// and its axes as matrix columns.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Orientation {
    /// The orientation type in effect (`t->orient[…].type`).
    pub kind: OrientType,
    /// `t->spacemtx`: columns are the X/Y/Z axes.
    pub matrix: Mat3,
}

impl Orientation {
    /// GLOBAL.
    pub const GLOBAL: Orientation = Orientation {
        kind: OrientType::Global,
        matrix: Mat3::IDENTITY,
    };
    /// `t->spacemtx_inv`.
    pub fn inverse(&self) -> Mat3 {
        invert_m3_or_identity(&self.matrix)
    }
    /// `ED_transform_calc_orientation_from_type_ex` (object mode).
    pub fn calc(kind: OrientType, src: &OrientSource, custom: Mat3) -> Orientation {
        let global = Orientation::GLOBAL;
        let with = |matrix: Mat3| Orientation { kind, matrix };
        match kind {
            OrientType::Global => global,
            // Gimbal without Euler rotation and parent without a parent fall through to global
            // (verified: a quaternion object or an unparented one moves along world X).
            OrientType::Gimbal => src.active_gimbal.map_or(global, with),
            OrientType::Parent => src
                .active_parent_world
                .map(|m| axes_of(&m))
                .map_or(global, with),
            // In Object Mode NORMAL is LOCAL (but keeps its type: its constraint is not
            // per-object).
            OrientType::Local | OrientType::Normal => {
                src.active_world.map(|m| axes_of(&m)).map_or(global, with)
            }
            OrientType::View => with(axes_of(&invert_m3_or_identity(&src.view))),
            OrientType::Cursor => with(src.cursor),
            OrientType::CustomMatrix => with(custom),
        }
    }
    /// The orientation of an operator call (`initTransInfo`): `orient_type` when set, else the
    /// scene's (GLOBAL); a set `orient_matrix` is used as is when `orient_matrix_type` (default
    /// GLOBAL) equals that orientation (how Blender redoes a transform).
    pub fn from_props(
        orient_type: Option<&str>,
        orient_matrix: Option<Mat3>,
        orient_matrix_type: &str,
        src: &OrientSource,
    ) -> Orientation {
        let kind = OrientType::from_identifier(orient_type.unwrap_or("GLOBAL"));
        if let Some(m) = orient_matrix
            && kind == OrientType::from_identifier(orient_matrix_type)
        {
            return Orientation::calc(OrientType::CustomMatrix, src, m);
        }
        Orientation::calc(kind, src, Mat3::IDENTITY)
    }
}

/// `eulO_to_gimbal_axis`: the first rotation axis is local, the second is local without the
/// first rotation, the last is global.
pub fn euler_gimbal_axes(eul: [f32; 3], order: EulerOrder) -> Mat3 {
    let axes = order_axes(order);
    let mut cols = [Vec3::ZERO; 3];
    let m = eul_o_to_mat3(eul, order);
    cols[axes[0]] = m.col(axes[0]);
    let mut teul = eul;
    teul[axes[0]] = 0.0;
    let m = eul_o_to_mat3(teul, order);
    cols[axes[1]] = m.col(axes[1]);
    cols[axes[2]][axes[2]] = 1.0;
    Mat3::from_cols(cols[0], cols[1], cols[2])
}

/// The axis indices of an Euler order, first applied first (`rotOrders[order].axis`).
pub fn order_axes(order: EulerOrder) -> [usize; 3] {
    let s = order.as_str().as_bytes();
    let idx = |c: u8| (c - b'X') as usize;
    [idx(s[0]), idx(s[1]), idx(s[2])]
}

/// `gimbal_axis_object`: the gimbal axes of an object rotating with Euler angles, in world
/// space (rotated by its parent's normalized axes); `None` for quaternion and axis-angle.
pub fn gimbal_axis_object(
    mode: RotationMode,
    euler: [f32; 3],
    parent_world: Option<&Mat3>,
) -> Option<Mat3> {
    let RotationMode::Euler(order) = mode else {
        return None;
    };
    let g = euler_gimbal_axes(euler, order);
    Some(match parent_world {
        Some(p) => crate::core::math::normalize_m3(p) * g,
        None => g,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> OrientSource {
        OrientSource {
            cursor: Mat3::IDENTITY,
            view: Mat3::IDENTITY,
            ..OrientSource::default()
        }
    }

    #[test]
    fn local_without_active_is_global() {
        let o = Orientation::calc(OrientType::Local, &src(), Mat3::IDENTITY);
        assert_eq!(o, Orientation::GLOBAL);
        let o = Orientation::calc(OrientType::Parent, &src(), Mat3::IDENTITY);
        assert_eq!(o.kind, OrientType::Global);
        let o = Orientation::calc(OrientType::Gimbal, &src(), Mat3::IDENTITY);
        assert_eq!(o.kind, OrientType::Global);
    }

    #[test]
    fn local_normalizes_the_active_axes() {
        let s = OrientSource {
            active_world: Some(
                Mat3::from_rotation_z(0.5) * Mat3::from_diagonal(Vec3::new(2.0, 3.0, 4.0)),
            ),
            ..src()
        };
        let o = Orientation::calc(OrientType::Normal, &s, Mat3::IDENTITY);
        assert_eq!(o.kind, OrientType::Normal);
        assert!(o.matrix.abs_diff_eq(Mat3::from_rotation_z(0.5), 1e-6));
    }

    #[test]
    fn gimbal_axes_of_xyz() {
        // Translate (1, 1, 1) GIMBAL on a cube with rotation (0.3, 0.4, 0.5) XYZ moves it to
        // (0.3289, 1.3192, 0.6106) (verified on 5.2.4).
        let g = gimbal_axis_object(RotationMode::Euler(EulerOrder::XYZ), [0.3, 0.4, 0.5], None)
            .expect("euler");
        let d = g * Vec3::ONE;
        assert!(
            d.abs_diff_eq(Vec3::new(0.3289, 1.3192, 0.6106), 1e-4),
            "{d:?}"
        );
        assert!(gimbal_axis_object(RotationMode::Quaternion, [0.0; 3], None).is_none());
        assert_eq!(order_axes(EulerOrder::ZXY), [2, 0, 1]);
    }

    #[test]
    fn custom_matrix_replaces_matching_type() {
        let m = Mat3::from_rotation_z(1.0);
        let o = Orientation::from_props(None, Some(m), "GLOBAL", &src());
        assert_eq!(o.kind, OrientType::CustomMatrix);
        assert_eq!(o.matrix, m);
        let o = Orientation::from_props(Some("LOCAL"), Some(m), "GLOBAL", &src());
        assert_eq!(o.kind, OrientType::Global);
        assert_eq!(
            OrientType::from_identifier("MyOrientation"),
            OrientType::Global
        );
    }
}
