// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_torus_add`, tier T1 (`tests/golden/cases/primitives/torus_*.py`).
//!
//! A Python operator in Blender (`scripts/startup/bl_operators/add_mesh_torus.py`,
//! `AddTorus.execute`, and `scripts/modules/bpy_extras/object_utils.py`, `object_data_add` /
//! `add_object_align_init`, Blender 5.2.4, GPL-2.0-or-later), so its placement rules differ
//! from the C primitives (verified on 5.2.4):
//! - the mesh is created first as `Torus` (uniquified in `bpy.data.meshes`) and the object is
//!   named after the mesh's final name (`bpy.data.objects.new(obdata.name, obdata)`);
//! - an unset `rotation` makes the operator use the `object_align` preference (`WORLD` in the
//!   factory settings) whatever `align` says, so `align='VIEW'` alone adds an unrotated torus;
//! - a set `rotation` is **replaced** by the view rotation for `align='VIEW'` (the inverse of
//!   `region_3d.view_matrix`, the stale factory matrix in `-b`: `(1.1087, 0.0133, 1.1483)`)
//!   and by the cursor rotation for `align='CURSOR'`;
//! - the object is placed by assigning `matrix_world = Translation(location) @ rotation`, so
//!   rotation and scale come from Blender's matrix decomposition (`rotation=(4, 5, 6)` reads
//!   back as `(-2.283, -1.283, -0.283)`, the view rotation leaves a scale of `0.9999998`);
//! - the mesh never sees the placement (no creation matrix, no `scale` property).

use frap_math::EulerOrder;
use frap_math::euler::eul_o_to_mat3;
use frap_math::glam::{Mat3, Mat4, Vec3};
use frap_math::matrix::RotationMode;
use frap_math::quat::{axis_angle_to_mat3, normalize_qt, quat_to_mat3};
use frap_mesh::primitives::torus::{Torus, TorusMode, torus};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdAny, Prop};
use frap_scene::generated::plain::View3DCursor;

use super::primitive_cube_add::common;
use frap_spec_ops_mesh::mesh::{self, primitive_torus_add};

/// `scene.cursor.matrix` without its translation (`BKE_scene_cursor_rot_to_mat3`).
fn cursor_matrix(cursor: &View3DCursor) -> Mat3 {
    match RotationMode::from_blender(&cursor.rotation_mode) {
        Some(RotationMode::Euler(order)) => eul_o_to_mat3(cursor.rotation_euler, order),
        Some(RotationMode::AxisAngle) => {
            let a = cursor.rotation_axis_angle;
            axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
        }
        Some(RotationMode::Quaternion) => quat_to_mat3(normalize_qt(cursor.rotation_quaternion).0),
        None => eul_o_to_mat3(cursor.rotation_euler, EulerOrder::XYZ),
    }
}

/// `add_object_align_init(context, operator)`: the matrix assigned to `matrix_world`.
fn align_matrix(
    cx: &OpCtx,
    location: &Prop<[f32; 3]>,
    rotation: &Prop<[f32; 3]>,
    align: primitive_torus_add::Align,
) -> Mat4 {
    let c = cx.ctx();
    let location = if location.is_set {
        location.value
    } else {
        c.cursor().location
    };
    let rot = if !rotation.is_set {
        // `properties.align = preferences.edit.object_align` ('WORLD'), rotation stays zero.
        Mat3::IDENTITY
    } else {
        match align {
            primitive_torus_add::Align::World => eul_o_to_mat3(rotation.value, EulerOrder::XYZ),
            primitive_torus_add::Align::View => Mat3::from_mat4(c.win.view3d.view_matrix).inverse(),
            primitive_torus_add::Align::Cursor => cursor_matrix(c.cursor()),
        }
    };
    Mat4::from_cols(
        rot.x_axis.extend(0.0),
        rot.y_axis.extend(0.0),
        rot.z_axis.extend(0.0),
        Vec3::from(location).extend(1.0),
    )
}

/// Moves the object's entry before its mesh's in the change log (object first, as every add
/// operator reports them).
fn object_first(cx: &mut OpCtx, ob: IdAny, data: IdAny) {
    let (_, log) = cx.main_mut();
    if let (Some(a), Some(b)) = (
        log.added.iter().position(|x| *x == data),
        log.added.iter().position(|x| *x == ob),
    ) && a < b
    {
        log.added.swap(a, b);
    }
}

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial("adding into a mesh in Edit Mode (joined into it) answers OP_NOT_IMPLEMENTED"),
    notes = [
        "Python operator in Blender: the object is named after the mesh ('Torus', uniquified in bpy.data.meshes)",
        "unset location => 3D cursor location",
        "unset rotation => no rotation, whatever align says (object_align preference WORLD); a set rotation is replaced by the view rotation for align='VIEW' and by the cursor rotation for align='CURSOR'",
        "the object is placed through matrix_world, so rotation and scale come from Blender's matrix decomposition",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveTorusAdd) -> OpResult {
    if let Some((_, ob)) = poll::edit_object(&cx.ctx())
        && ob.object_type() == "MESH"
    {
        return Err(OpError::NotImplemented(
            "frap: adding a torus to a mesh in Edit Mode (joined into the edit mesh) is not implemented yet"
                .into(),
        ));
    }
    let params = Torus {
        major_segments: p.major_segments.value,
        minor_segments: p.minor_segments.value,
        mode: match p.mode.value {
            primitive_torus_add::Mode::MajorMinor => TorusMode::MajorMinor,
            primitive_torus_add::Mode::ExtInt => TorusMode::ExtInt,
        },
        major_radius: p.major_radius.value,
        minor_radius: p.minor_radius.value,
        abso_major_rad: p.abso_major_rad.value,
        abso_minor_rad: p.abso_minor_rad.value,
        generate_uvs: p.generate_uvs.value,
    };
    common::reserve(params.estimate())?;
    let mut mesh = torus(&params);
    let matrix = align_matrix(cx, &p.location, &p.rotation, p.align.value);
    let (main, log) = cx.main_mut();
    mesh.header.name = "Torus".into();
    let me = main.meshes.insert("Torus", mesh, log);
    let name = main.meshes.name_of(me).unwrap_or("Torus").to_owned();
    let ob = cx.add_object(
        &name,
        Some(me.any()),
        Placement {
            location: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        },
    );
    object_first(cx, ob.any(), me.any());
    cx.object_mut(ob)?.set_matrix_world(&matrix, None);
    cx.result("object", ob);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_matrix_follows_rotation_mode() {
        let mut c = View3DCursor {
            rotation_euler: [0.2, 0.3, 0.4],
            ..Default::default()
        };
        let e = cursor_matrix(&c);
        assert!(e.abs_diff_eq(eul_o_to_mat3([0.2, 0.3, 0.4], EulerOrder::XYZ), 0.0));
        c.rotation_mode = "QUATERNION".into();
        c.rotation_quaternion = [2.0, 0.0, 0.0, 0.0];
        assert!(cursor_matrix(&c).abs_diff_eq(Mat3::IDENTITY, 1e-7));
    }
}
