// SPDX-License-Identifier: GPL-3.0-or-later
//! Transform orientations (`orient_type`, `orient_matrix`, `orient_matrix_type`): the space
//! matrix `spacemtx` whose columns are the X/Y/Z axes values and constraints are expressed in.
//! Ports of `ED_transform_calc_orientation_from_type_ex`, `gimbal_axis_object`,
//! `eulO_to_gimbal_axis` (`editors/transform/transform_orientations.cc`,
//! `blenlib/intern/math_rotation_c.cc`) and the orientation part of `initTransInfo`
//! (`transform_generics.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (object mode): LOCAL and NORMAL use the active object's normalized world
//! axes; PARENT the active object's parent's axes; GIMBAL the active object's gimbal axes
//! (Euler: `eulO_to_gimbal_axis`; axis-angle: `axis_angle_to_gimbal_axis`); VIEW the inverse of
//! the 3D View's view matrix (frap's virtual view stores Blender's `-b` value, so the axes
//! match); CURSOR the 3D cursor's rotation. Where these have nothing to use (no active object,
//! an active object without a parent, a quaternion active object for GIMBAL) the matrix is the
//! identity but the **requested type is kept**: a LOCAL or GIMBAL transform of several objects
//! then still constrains each object along its own axes (`translate(value=(1, 0, 0),
//! orient_type='LOCAL')` with no active object moves a cube turned (0.3, 0.4, 0.5) by
//! (0.8083, 0.4416, -0.3894)). An `orient_matrix` replaces the computed matrix when
//! `orient_type` (or the scene orientation) equals `orient_matrix_type`.
//!
//! Custom orientations (WP 8.3.5, `transform.create_orientation`): `orient_type` and
//! `orient_matrix_type` also take the names of the scene's custom orientations
//! (`rna_TransformOrientation_itemf`: the seven built-in items, then one item per custom
//! orientation, value `V3D_ORIENT_CUSTOM + index`), and an unset `orient_type` is the scene's
//! orientation (`scene.transform_orientation_slots[0]`, set by `transform.select_orientation`
//! and `create_orientation(use=True)`), as `initTransInfo` reads it in a 3D View.

use frap_math::EulerOrder;
use frap_math::euler::eul_o_to_mat3;
use frap_math::matrix::RotationMode;
use frap_op::EnumItemOwned;
use frap_scene::Context;
use frap_scene::id::Scene;
use glam::{Mat3, Vec3};
use smol_str::SmolStr;

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
    /// `V3D_ORIENT_CUSTOM + i`: the scene's custom orientation `i`.
    Custom(usize),
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
    /// From an `orient_type` identifier, custom orientation names included (the first custom
    /// orientation of that name; an unknown name reads as GLOBAL).
    pub fn resolve(s: &str, scene: &SceneOrientations) -> OrientType {
        match OrientType::from_identifier(s) {
            OrientType::Global if s != "GLOBAL" => scene
                .customs
                .iter()
                .position(|(n, _)| n == s)
                .map_or(OrientType::Global, OrientType::Custom),
            t => t,
        }
    }
    /// From a `V3D_ORIENT_*` value (`Scene::orientation_slot_index`); a custom index past the
    /// scene's orientations reads as GLOBAL (`initTransInfo`).
    pub fn from_value(v: i32, custom_count: usize) -> OrientType {
        match v {
            1 => OrientType::Local,
            2 => OrientType::Normal,
            3 => OrientType::View,
            4 => OrientType::Gimbal,
            5 => OrientType::Cursor,
            6 => OrientType::Parent,
            v if v >= Scene::ORIENT_CUSTOM
                && ((v - Scene::ORIENT_CUSTOM) as usize) < custom_count =>
            {
                OrientType::Custom((v - Scene::ORIENT_CUSTOM) as usize)
            }
            _ => OrientType::Global,
        }
    }
    /// The `V3D_ORIENT_*` value (`CustomMatrix` has none and gives `V3D_ORIENT_CUSTOM - 1`).
    pub fn value(self) -> i32 {
        match self {
            OrientType::Global => 0,
            OrientType::Local => 1,
            OrientType::Normal => 2,
            OrientType::View => 3,
            OrientType::Gimbal => 4,
            OrientType::Cursor => 5,
            OrientType::Parent => 6,
            OrientType::CustomMatrix => Scene::ORIENT_CUSTOM - 1,
            OrientType::Custom(i) => Scene::ORIENT_CUSTOM + i as i32,
        }
    }
}

/// The scene's orientation state a transform reads: the default slot's orientation and the
/// custom orientations (name, axes as columns).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneOrientations {
    /// `scene.transform_orientation_slots[0]` resolved (GLOBAL without a 3D View).
    pub default: Option<OrientType>,
    /// `scene->transform_spaces`.
    pub customs: Vec<(SmolStr, Mat3)>,
}

impl SceneOrientations {
    /// The orientation state of `scene`.
    pub fn of(scene: &Scene) -> SceneOrientations {
        let customs: Vec<(SmolStr, Mat3)> = scene
            .transform_orientations
            .iter()
            .map(|t| (t.name.clone(), t.matrix))
            .collect();
        let default = OrientType::from_value(
            scene.orientation_slot_index(Scene::ORIENT_SLOT_DEFAULT),
            customs.len(),
        );
        SceneOrientations {
            default: Some(default),
            customs,
        }
    }
    /// The scene orientation (`orient_type_scene`).
    pub fn scene_type(&self) -> OrientType {
        self.default.unwrap_or(OrientType::Global)
    }
}

/// The dynamic enums of an operator with `orient_type` and `orient_matrix_type`.
pub const ORIENT_DYN_ENUMS: &[(&str, frap_op::DynEnumFn)] = &[
    ("orient_type", orient_type_items),
    ("orient_matrix_type", orient_type_items),
];

/// The items of every `orient_type` / `orient_matrix_type` / `orientation` property
/// (`rna_TransformOrientation_itemf`): the seven built-in orientations, then the context
/// scene's custom orientations by name.
pub fn orient_type_items(c: &Context) -> Vec<EnumItemOwned> {
    const BUILTIN: [(&str, &str, &str); 7] = [
        (
            "GLOBAL",
            "Global",
            "Align the transformation axes to world space",
        ),
        (
            "LOCAL",
            "Local",
            "Align the transformation axes to the selected objects' local space",
        ),
        (
            "NORMAL",
            "Normal",
            "Align the transformation axes to average normal of selected elements (bone Y axis \
             for pose mode)",
        ),
        (
            "GIMBAL",
            "Gimbal",
            "Align each axis to the Euler rotation axis as used for input",
        ),
        (
            "VIEW",
            "View",
            "Align the transformation axes to the window",
        ),
        (
            "CURSOR",
            "Cursor",
            "Align the transformation axes to the 3D cursor",
        ),
        (
            "PARENT",
            "Parent",
            "Align the transformation axes to the object's parent space",
        ),
    ];
    let mut out: Vec<EnumItemOwned> = BUILTIN
        .iter()
        .map(|(i, n, d)| EnumItemOwned {
            identifier: (*i).into(),
            name: (*n).into(),
            description: (*d).into(),
        })
        .collect();
    out.extend(
        c.scene()
            .transform_orientations
            .iter()
            .map(|t| EnumItemOwned {
                identifier: t.name.clone(),
                name: t.name.clone(),
                description: SmolStr::default(),
            }),
    );
    out
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
    /// The scene's orientation slot and custom orientations (WP 8.3.5).
    pub scene: SceneOrientations,
}

/// A resolved orientation: the requested type (kept even when its matrix falls back to the
/// identity, as `t->orient[…].type` is) and its axes as matrix columns.
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
    /// `ED_transform_calc_orientation_from_type_ex` (object mode). The fallbacks give the
    /// identity matrix and keep `kind` (verified: with Cube and Light selected and no active
    /// object, LOCAL moves each along its own X; with a quaternion active object, GIMBAL still
    /// turns an Euler object about its own gimbal axis).
    pub fn calc(kind: OrientType, src: &OrientSource, custom: Mat3) -> Orientation {
        let matrix = match kind {
            OrientType::Global => Mat3::IDENTITY,
            // Quaternion rotation (no gimbal axes) and no active object: the identity
            // (verified: a single quaternion object moves along world X).
            OrientType::Gimbal => src.active_gimbal.unwrap_or(Mat3::IDENTITY),
            OrientType::Parent => src
                .active_parent_world
                .map_or(Mat3::IDENTITY, |m| axes_of(&m)),
            // In Object Mode NORMAL is LOCAL (but keeps its type: its constraint is not
            // per-object).
            OrientType::Local | OrientType::Normal => {
                src.active_world.map_or(Mat3::IDENTITY, |m| axes_of(&m))
            }
            OrientType::View => axes_of(&invert_m3_or_identity(&src.view)),
            OrientType::Cursor => src.cursor,
            OrientType::CustomMatrix => custom,
            // `applyTransformOrientation`; an index past the list cannot be resolved.
            OrientType::Custom(i) => src.scene.customs.get(i).map_or(Mat3::IDENTITY, |c| c.1),
        };
        Orientation { kind, matrix }
    }
    /// The orientation of an operator call (`initTransInfo`): `orient_type` when set, else the
    /// scene's (`transform_orientation_slots[0]`, GLOBAL by default); a set `orient_matrix` is
    /// used as is when `orient_matrix_type` (default GLOBAL) equals that orientation (how
    /// Blender redoes a transform).
    pub fn from_props(
        orient_type: Option<&str>,
        orient_matrix: Option<Mat3>,
        orient_matrix_type: &str,
        src: &OrientSource,
    ) -> Orientation {
        let kind = match orient_type {
            Some(s) => OrientType::resolve(s, &src.scene),
            None => src.scene.scene_type(),
        };
        if let Some(m) = orient_matrix
            && kind == OrientType::resolve(orient_matrix_type, &src.scene)
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

/// `axis_angle_to_gimbal_axis`: Z is the axis; X is the axis crossed with its cyclic
/// permutation, turned by the angle about the axis; Y completes the frame. Reproduced from
/// Blender 5.2.4's results (translating along each GIMBAL axis of axis-angle objects): X is
/// `normalize((ay, az, ax) × a)` rotated by `angle` (zero when the axis is along (1, 1, 1), as
/// in Blender).
pub fn axis_angle_gimbal_axes(axis: [f32; 3], angle: f32) -> Mat3 {
    let z = Vec3::from_array(axis).normalize_or_zero();
    let x0 = Vec3::new(z.y, z.z, z.x).cross(z).normalize_or_zero();
    let x = crate::core::math::axis_angle_normalized_to_mat3(z, angle) * x0;
    let y = z.cross(x);
    Mat3::from_cols(x, y, z)
}

/// `gimbal_axis_object`: the gimbal axes of an object rotating with Euler angles or an axis
/// and angle, in world space (rotated by its parent's normalized axes); `None` for a
/// quaternion. `axis_angle` is `rotation_axis_angle` (angle first).
pub fn gimbal_axis_object(
    mode: RotationMode,
    euler: [f32; 3],
    axis_angle: [f32; 4],
    parent_world: Option<&Mat3>,
) -> Option<Mat3> {
    let g = match mode {
        RotationMode::Euler(order) => euler_gimbal_axes(euler, order),
        RotationMode::AxisAngle => {
            axis_angle_gimbal_axes([axis_angle[1], axis_angle[2], axis_angle[3]], axis_angle[0])
        }
        RotationMode::Quaternion => return None,
    };
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
    fn fallbacks_keep_the_type_with_identity_axes() {
        for kind in [
            OrientType::Local,
            OrientType::Normal,
            OrientType::Parent,
            OrientType::Gimbal,
        ] {
            let o = Orientation::calc(kind, &src(), Mat3::IDENTITY);
            assert_eq!(o.kind, kind);
            assert_eq!(o.matrix, Mat3::IDENTITY);
        }
    }

    #[test]
    fn axis_angle_gimbal_matches_blender() {
        // translate along each GIMBAL axis of a cube with rotation_axis_angle (0.7, 0.3, 0.2,
        // 0.9), and X for other axes and angles (verified on 5.2.4).
        let g = axis_angle_gimbal_axes([0.3, 0.2, 0.9], 0.7);
        assert!(
            g.x_axis
                .abs_diff_eq(Vec3::new(0.7559, 0.5389, -0.3717), 1e-4)
        );
        assert!(
            g.y_axis
                .abs_diff_eq(Vec3::new(-0.5769, 0.8167, 0.0108), 1e-4)
        );
        assert!(
            g.z_axis
                .abs_diff_eq(Vec3::new(0.3094, 0.2063, 0.9283), 1e-4)
        );
        let x = |a: [f32; 3], angle: f32| axis_angle_gimbal_axes(a, angle).x_axis;
        assert!(x([0.3, 0.2, 0.9], 1.5).abs_diff_eq(Vec3::new(0.1128, 0.9613, -0.2512), 1e-4));
        assert!(x([0.0, 1.0, 0.0], 0.5).abs_diff_eq(Vec3::new(0.4794, 0.0, 0.8776), 1e-4));
        assert!(x([1.0, 1.0, 0.0], 0.0).abs_diff_eq(Vec3::new(-1.0, 1.0, 1.0) / 3f32.sqrt(), 1e-4));
        assert!(x([0.3, 0.2, -0.9], 0.0).abs_diff_eq(Vec3::new(0.8769, 0.3157, 0.3625), 1e-4));
        assert!(x([-1.0, 0.0, 0.0], 0.0).abs_diff_eq(Vec3::Y, 1e-6));
        assert_eq!(x([1.0, 1.0, 1.0], 0.3), Vec3::ZERO);
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
        let g = gimbal_axis_object(
            RotationMode::Euler(EulerOrder::XYZ),
            [0.3, 0.4, 0.5],
            [0.0; 4],
            None,
        )
        .expect("euler");
        let d = g * Vec3::ONE;
        assert!(
            d.abs_diff_eq(Vec3::new(0.3289, 1.3192, 0.6106), 1e-4),
            "{d:?}"
        );
        assert!(gimbal_axis_object(RotationMode::Quaternion, [0.0; 3], [0.0; 4], None).is_none());
        assert_eq!(order_axes(EulerOrder::ZXY), [2, 0, 1]);
    }

    #[test]
    fn custom_matrix_replaces_matching_type() {
        let m = Mat3::from_rotation_z(1.0);
        let o = Orientation::from_props(None, Some(m), "GLOBAL", &src());
        assert_eq!(o.kind, OrientType::CustomMatrix);
        assert_eq!(o.matrix, m);
        let o = Orientation::from_props(Some("LOCAL"), Some(m), "GLOBAL", &src());
        assert_eq!(o.kind, OrientType::Local);
        assert_eq!(o.matrix, Mat3::IDENTITY);
        assert_eq!(
            OrientType::from_identifier("MyOrientation"),
            OrientType::Global
        );
    }

    #[test]
    fn custom_orientations_by_name_and_as_the_scene_default() {
        let m = Mat3::from_rotation_x(0.7);
        let mut s = src();
        s.scene.customs = vec![("Face".into(), Mat3::IDENTITY), ("Edge".into(), m)];
        // By name.
        let o = Orientation::from_props(Some("Edge"), None, "GLOBAL", &s);
        assert_eq!(o.kind, OrientType::Custom(1));
        assert_eq!(o.matrix, m);
        assert_eq!(OrientType::resolve("Nope", &s.scene), OrientType::Global);
        assert_eq!(OrientType::resolve("VIEW", &s.scene), OrientType::View);
        // Unset `orient_type`: the scene orientation.
        s.scene.default = Some(OrientType::Custom(1));
        let o = Orientation::from_props(None, None, "GLOBAL", &s);
        assert_eq!(o.kind, OrientType::Custom(1));
        assert_eq!(o.matrix, m);
        // Values round-trip; a custom index past the list reads as GLOBAL.
        for t in [OrientType::Local, OrientType::Parent, OrientType::Custom(1)] {
            assert_eq!(OrientType::from_value(t.value(), 2), t);
        }
        assert_eq!(
            OrientType::from_value(OrientType::Custom(5).value(), 2),
            OrientType::Global
        );
    }
}
