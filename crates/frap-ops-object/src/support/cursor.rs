// SPDX-License-Identifier: GPL-3.0-or-later
//! The 3D cursor and the view grid of the snap operators. Ports of
//! `BKE_scene_cursor_rot_to_mat3`, `BKE_scene_cursor_mat3_to_rot`, `BKE_scene_cursor_to_mat4`
//! (`blenkernel/intern/scene.cc`) and `ED_view3d_grid_view_scale` / `ED_scene_grid_scale`
//! (`editors/space_view3d/view3d_draw.cc`), Blender 5.2.4, GPL-2.0-or-later.

use frap_math::EulerOrder;
use frap_math::euler::{eul_o_to_mat3, mat3_to_compatible_eul_o, mat3_to_eul_o};
use frap_math::matrix::RotationMode;
use frap_math::quat::{
    axis_angle_to_mat3, mat3_normalized_to_quat, quat_to_axis_angle, quat_to_mat3,
};
use frap_scene::generated::plain::{UnitSettings, View3DCursor};
use glam::Mat3;

fn mode(cursor: &View3DCursor) -> RotationMode {
    RotationMode::from_blender(&cursor.rotation_mode)
        .unwrap_or(RotationMode::Euler(EulerOrder::XYZ))
}

/// `BKE_scene_cursor_rot_to_mat3` (the quaternion is used as stored).
pub fn rot_mat3(cursor: &View3DCursor) -> Mat3 {
    match mode(cursor) {
        RotationMode::AxisAngle => {
            let a = cursor.rotation_axis_angle;
            axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
        }
        RotationMode::Quaternion => quat_to_mat3(cursor.rotation_quaternion),
        RotationMode::Euler(order) => eul_o_to_mat3(cursor.rotation_euler, order),
    }
}

/// `BKE_scene_cursor_mat3_to_rot`: writes a unit rotation matrix into the channel of the
/// cursor's rotation mode (the other channels are kept).
pub fn set_rot_mat3(cursor: &mut View3DCursor, m: &Mat3, use_compat: bool) {
    match mode(cursor) {
        RotationMode::Quaternion => {
            // `use_compat` (`quat_to_compatible_quat`) is only used by interactive tools.
            cursor.rotation_quaternion = mat3_normalized_to_quat(m);
        }
        RotationMode::AxisAngle => {
            let (axis, angle) = quat_to_axis_angle(frap_math::quat::mat3_to_quat(m));
            cursor.rotation_axis_angle = [angle, axis[0], axis[1], axis[2]];
        }
        RotationMode::Euler(order) => {
            cursor.rotation_euler = if use_compat {
                mat3_to_compatible_eul_o(m, cursor.rotation_euler, order)
            } else {
                mat3_to_eul_o(m, order)
            };
        }
    }
    sync_matrix(cursor);
}

/// Stores `View3DCursor.matrix` (rows) from the location and rotation, the value Blender's
/// getter computes (`BKE_scene_cursor_to_mat4`).
pub fn sync_matrix(cursor: &mut View3DCursor) {
    let r = rot_mat3(cursor);
    let l = cursor.location;
    for (row, out) in cursor.matrix.iter_mut().enumerate().take(3) {
        *out = [r.col(0)[row], r.col(1)[row], r.col(2)[row], l[row]];
    }
    cursor.matrix[3] = [0.0, 0.0, 0.0, 1.0];
}

/// `ED_view3d_grid_view_scale` for frap's virtual Layout view (perspective, so the zoom does
/// not subdivide the grid): `View3D.overlay.grid_scale` (1) times the length of the scene's base
/// unit divided by `scale_length` (`NONE`: 1). Verified on 5.2.4: metric `(0.4, 0.6, -1.5)` →
/// `(0, 1, -1)`; `scale_length = 0.5` → steps of 2; imperial → steps of one foot (0.3048).
pub fn grid_scale(units: &UnitSettings) -> f32 {
    let base = match units.system.as_str() {
        "METRIC" => 1.0,
        "IMPERIAL" => 0.3048,
        _ => return 1.0,
    };
    base / units.scale_length
}

/// `gridf * floorf(0.5 + v / gridf)` per component.
pub fn snap_to_grid(v: [f32; 3], gridf: f32) -> [f32; 3] {
    v.map(|x| gridf * (0.5 + x / gridf).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid() {
        let mut u = UnitSettings::default();
        assert_eq!(
            snap_to_grid([0.4, 0.6, -1.5], grid_scale(&u)),
            [0.0, 1.0, -1.0]
        );
        u.scale_length = 0.5;
        assert_eq!(
            snap_to_grid([0.4, 0.6, -1.3], grid_scale(&u)),
            [0.0, 0.0, -2.0]
        );
        u.system = "NONE".into();
        assert_eq!(grid_scale(&u), 1.0);
        u.system = "IMPERIAL".into();
        u.scale_length = 1.0;
        let s = snap_to_grid([0.4, 0.6, -1.3], grid_scale(&u));
        assert!(
            (s[1] - 0.6096).abs() < 1e-6 && (s[2] + 1.2192).abs() < 1e-6,
            "{s:?}"
        );
    }

    #[test]
    fn reset_rotation_per_mode() {
        let mut c = View3DCursor {
            rotation_mode: "AXIS_ANGLE".into(),
            rotation_axis_angle: [0.5, 1.0, 0.0, 0.0],
            rotation_euler: [0.1, 0.2, 0.3],
            ..View3DCursor::default()
        };
        set_rot_mat3(&mut c, &Mat3::IDENTITY, false);
        assert_eq!(c.rotation_axis_angle, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(c.rotation_euler, [0.1, 0.2, 0.3]);
        c.rotation_mode = "QUATERNION".into();
        c.rotation_quaternion = [0.5; 4];
        set_rot_mat3(&mut c, &Mat3::IDENTITY, false);
        assert_eq!(c.rotation_quaternion, [1.0, 0.0, 0.0, 0.0]);
    }
}
