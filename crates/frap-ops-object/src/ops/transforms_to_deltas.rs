// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.transforms_to_deltas(mode='ALL', reset_values=True)`, tier T1
//! (`tests/golden/cases/object/transform_m8/transforms_to_deltas*.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object.py (`TransformsToDeltas`) and the
//! mathutils methods it calls (`Euler.rotate`, `Quaternion.rotate`:
//! blender/source/blender/python/mathutils/mathutils_Euler.cc, mathutils_Quaternion.cc,
//! `mathutils_any_to_rotmat` in mathutils.cc), Blender 5.2.4, GPL-2.0-or-later. Poll
//! `context.scene.is_editable`.
//!
//! For every selected editable object: `LOC` adds `location` to `delta_location`; `ROT` makes
//! the delta rotation the rotation turned by the old delta (Euler: `R_delta · R` back to
//! compatible Euler angles; quaternion: the same with matrices, keeping the quaternion's
//! length); axis-angle rotations are skipped, as the add-on says (`Unsupported`); `SCALE`
//! multiplies `delta_scale` by `scale`. With `reset_values` the channels are cleared after
//! the transfer; clearing Euler angles also sets `rotation_mode` to `XYZ` (a quirk of the
//! add-on's `rotation_euler.zero()`, see `transfer_rotation`). Nothing selected → WARNING `No selected editable objects to operate on` and
//! `CANCELLED`.

use frap_math::euler::{eul_o_to_mat3, mat3_to_compatible_eul_o};
use frap_math::matrix::RotationMode;
use frap_math::quat::{QUAT_IDENTITY, mat3_to_quat, mul_qt_fl, normalize_qt, quat_to_mat3};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::Object;
use frap_spec_ops_object::object;
use frap_spec_ops_object::object::transforms_to_deltas::Mode;

/// `Quaternion.rotate(other)` on `q`: `other`'s normalized rotation applied after `q`'s, the
/// result scaled back to `q`'s length.
pub fn quat_rotate(q: [f32; 4], other: [f32; 4]) -> [f32; 4] {
    let other_rmat = quat_to_mat3(normalize_qt(other).0);
    let (tquat, length) = normalize_qt(q);
    let rmat = other_rmat * quat_to_mat3(tquat);
    mul_qt_fl(mat3_to_quat(&rmat), length)
}

fn transfer_rotation(o: &mut Object, reset: bool) {
    match o.rotation_mode_enum() {
        RotationMode::Quaternion => {
            let delta = o.delta_rotation_quaternion;
            o.delta_rotation_quaternion = quat_rotate(o.rotation_quaternion, delta);
            if reset {
                o.rotation_quaternion = QUAT_IDENTITY;
            }
        }
        // "Unsupported".
        RotationMode::AxisAngle => {}
        RotationMode::Euler(order) => {
            // `Euler.rotate(delta)`: both Eulers in the object's rotation order.
            let other = eul_o_to_mat3(o.delta_rotation_euler, order);
            let eul = o.rotation_euler;
            let rmat = other * eul_o_to_mat3(eul, order);
            o.delta_rotation_euler = mat3_to_compatible_eul_o(&rmat, eul, order);
            if reset {
                // `obj.rotation_euler.zero()` writes through a fresh `Euler` wrapper that was
                // never read, so its order is still XYZ: the write-back sets `rotation_mode`
                // to XYZ (Euler to Euler converts no values), verified on 5.2.4.
                o.rotation_euler = [0.0; 3];
                o.rotation_mode = "XYZ".into();
            }
        }
    }
}

#[operator(poll = poll::SCENE_EDITABLE, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::TransformsToDeltas) -> OpResult {
    let objects = cx.ctx().selected_editable_objects();
    if objects.is_empty() {
        cx.report(
            ReportType::Warning,
            "No selected editable objects to operate on",
        );
        return Ok(OpStatus::Cancelled);
    }
    let mode = p.mode.value;
    let reset = p.reset_values.value;
    for ob in objects {
        let o = cx.object_mut(ob)?;
        if matches!(mode, Mode::All | Mode::Loc) {
            for i in 0..3 {
                o.delta_location[i] += o.location[i];
            }
            if reset {
                o.location = [0.0; 3];
            }
        }
        if matches!(mode, Mode::All | Mode::Rot) {
            transfer_rotation(o, reset);
        }
        if matches!(mode, Mode::All | Mode::Scale) {
            for i in 0..3 {
                o.delta_scale[i] *= o.scale[i];
            }
            if reset {
                o.scale = [1.0; 3];
            }
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternion_rotate_keeps_the_length() {
        let q = [2.0, 0.0, 0.0, 0.0];
        let r = quat_rotate(q, [0.0, 0.0, 0.0, 1.0]);
        // A half turn about Z, length 2.
        assert!((r[3].abs() - 2.0).abs() < 1e-5, "{r:?}");
    }
}
