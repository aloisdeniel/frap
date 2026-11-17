// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.camera_add(enter_editmode=False, align='WORLD', location=…, rotation=…,
//! scale=…)`, tier T0 (`tests/golden/cases/object/add/camera*.py`). Port of
//! `object_camera_add_exec` (`editors/object/object_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) with frap's virtual 3D View (§8, D-10). Verified on 5.2.4 in a VIEW_3D
//! context:
//! - `align` is forced to `VIEW`: an unset `rotation` is the view rotation
//!   (`(1.1088, 0, 1.1549)` with the factory viewport) whatever `align` says; a set `rotation`
//!   wins; unset `location` is the 3D cursor; `scale` and `enter_editmode` are ignored;
//! - object and camera data are both requested as `Camera` and uniquified independently
//!   (the factory scene gives object `Camera.001` with data `Camera.001`; after renaming the
//!   data they differ), with the values of `bpy.data.cameras.new` (`display_size` 1, the grid
//!   scale of the factory viewport);
//! - the new object is linked to `context.collection`, selected and active, others deselected;
//! - `scene.camera` is set to it when the scene has no camera (the 3D View's `scenelock`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Prop;
use frap_scene::id::Camera;
use frap_spec_ops_object::object;

/// Placement of the non-mesh add operators (WP 1.20).
pub(crate) mod common {
    use std::sync::OnceLock;

    use frap_math::EulerOrder;
    use frap_math::euler::quat_to_eul_o;
    use frap_math::quat::conjugate_qt;
    use frap_op::prelude::*;
    use frap_rna::Prop;

    /// `rv3d->viewquat` of the factory Layout viewport: the inverse of the dumped
    /// `region_3d.view_rotation`. The factory `view_matrix` that
    /// `frap_op::helpers::view_rotation` reads is stale in `-b` and gives `(1.10871, 0.01326,
    /// 1.14827)`; the primitives (WP 1.19) take the rotation from this quaternion too.
    fn viewquat() -> [f32; 4] {
        static QUAT: OnceLock<[f32; 4]> = OnceLock::new();
        *QUAT.get_or_init(|| {
            let q = frap_scene::startup::factory_dump()
                .get("view3d")
                .and_then(|v| v.get("region_3d"))
                .and_then(|v| v.get("view_rotation"))
                .and_then(|v| v.as_array())
                .filter(|a| a.len() == 4)
                .map(|a| {
                    std::array::from_fn::<f32, 4, _>(|i| a[i].as_f64().unwrap_or(0.0) as f32)
                })
                .unwrap_or([1.0, 0.0, 0.0, 0.0]);
            conjugate_qt(q)
        })
    }

    /// `ED_object_rotation_from_view(C, rot, 'Z')` with the virtual 3D View (verified on
    /// 5.2.4: `(1.10875, 1.2e-7, 1.15488)`).
    pub(crate) fn view_rotation() -> [f32; 3] {
        let mut q = viewquat();
        q[0] = -q[0];
        quat_to_eul_o(q, EulerOrder::XYZ)
    }

    /// `ED_object_add_generic_get_opts` with `align_axis = 'Z'`: `helpers::placement`, with
    /// the view rotation of [`view_rotation`] for `VIEW`.
    pub(crate) fn placement(
        cx: &OpCtx,
        location: &Prop<[f32; 3]>,
        rotation: &Prop<[f32; 3]>,
        align: helpers::Align,
        scale: &Prop<[f32; 3]>,
    ) -> Result<helpers::Placement, OpError> {
        if align == helpers::Align::View && !rotation.is_set {
            let from_view = Prop {
                value: view_rotation(),
                is_set: true,
            };
            return helpers::placement(cx, location, &from_view, helpers::Align::World, scale);
        }
        helpers::placement(cx, location, rotation, align, scale)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn view_rotation_is_blenders() {
            let r = super::view_rotation();
            assert!((r[0] - 1.10875).abs() < 1e-4, "{r:?}");
            assert!(r[1].abs() < 1e-4, "{r:?}");
            assert!((r[2] - 1.15488).abs() < 1e-4, "{r:?}");
        }
    }
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = [
        "align is forced to VIEW (Blender's camera_add): an unset rotation is the view rotation",
        "sets scene.camera when the scene has none (frap's virtual 3D View, D-10)",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::CameraAdd) -> OpResult {
    // `RNA_enum_set(op->ptr, "align", ALIGN_VIEW)`: the rotation comes from the view.
    let at = common::placement(
        cx,
        &p.location,
        &p.rotation,
        helpers::Align::View,
        &Prop {
            value: [0.0; 3],
            is_set: false,
        },
    )?;
    let data = {
        let (main, log) = cx.main_mut();
        main.cameras.insert("Camera", Camera::new("Camera"), log)
    };
    // `ED_object_add_type(C, OB_CAMERA, nullptr, loc, rot, false, …)`: never enters Edit Mode.
    let ob = helpers::add_object_with_data(cx, "Camera", Some(data.any()), at, false)?;
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if main.scenes.get(scene).is_some_and(|s| s.camera.is_none())
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.camera = Some(ob);
    }
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}
