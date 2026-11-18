// SPDX-License-Identifier: GPL-3.0-or-later
//! Steps shared by the C `mesh.primitive_*_add` operators (WP 1.19): the object-mode half of
//! `make_prim_init` / `make_prim_finish` (`editors/mesh/editmesh_add.cc`) and the creation
//! matrix of `ED_object_new_primitive_matrix` (`editors/object/object_add.cc`), Blender 5.2.4,
//! GPL-2.0-or-later. Declared under `primitive_cube_add.rs` because `lib.rs` belongs to the
//! integrator (ARCHITECTURE §22.1); the torus is a Python operator with its own path.

use std::sync::OnceLock;

use frap_math::EulerOrder;
use frap_math::euler::quat_to_eul_o;
use frap_math::quat::{axis_angle_to_quat, conjugate_qt, mul_qt_qtqt, quat_to_mat3};
use frap_mesh::Mesh;
use frap_mesh::primitives::{Matrix4, creation_matrix};
use frap_op::helpers::{self, Align};
use frap_op::prelude::*;
use frap_rna::Prop;
use frap_scene::id::Object;

/// `frap_note`s of every C primitive operator (`--describe`).
pub(crate) const NOTES: [&str; 4] = [
    "unset location => 3D cursor location",
    "unset rotation => from align (WORLD: none, VIEW: the virtual 3D View, CURSOR: the 3D cursor); a set rotation wins over align",
    "unset scale => (1, 1, 1); the scale is baked into the mesh, object scale stays 1",
    "enter_editmode=True enters Edit Mode; adding while a mesh is in Edit Mode builds the primitive into the edit mesh, with only the new elements selected",
];

/// `support` text of every C primitive operator.
pub(crate) const PARTIAL: &str = "in Edit Mode, UV and ico spheres do not reuse the edit mesh's freed element slots in Blender's order (element order may differ after deletions)";

/// Where the new object goes and the matrix its geometry is created with.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Prim {
    /// Object location and rotation (`Placement::scale` is the operator scale).
    pub at: Placement,
    /// `ED_object_new_primitive_matrix`: the inverse of the new object's rotation applied to
    /// the operator rotation (identity up to float noise), the operator `scale` folded in when
    /// the operator passes one (cube, cylinder, cone, UV sphere, ico sphere).
    pub matrix: Matrix4,
}

/// `BKE_object_to_mat4` of an object freshly added at `at` (`ED_object_base_init_transform_on_add`:
/// location and Euler XYZ rotation set, scale 1), as Blender's `float[4][4]` (`m[col][row]`).
pub(crate) fn object_matrix(at: &Placement) -> Matrix4 {
    let mut ob = Object::new("", None);
    ob.location = at.location;
    ob.rotation_euler = at.rotation;
    ob.matrix_basis().to_cols_array_2d()
}

/// The axis the new object's local Z (or Y) is aligned with for `align='VIEW'`
/// (`ED_object_add_generic_get_opts(C, op, align_axis, …)`: `'Y'` for Suzanne, `'Z'` for the
/// other primitives).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ViewAxis {
    /// `'Z'`: the object's Z axis points at the viewer.
    Z,
    /// `'Y'`: the object's Y axis points away from the viewer (Suzanne faces the view).
    Y,
}

/// `rv3d->viewquat` of the factory Layout viewport. `region_3d.view_rotation` (dumped in
/// `startup.json`) is its inverse (`rna_RegionView3D_view_rotation_get`).
///
/// `WindowState::view3d.view_matrix` holds the factory `region_3d.view_matrix`, which is
/// stale in `-b` (never redrawn since the file was saved) and is what RNA and the Python torus
/// operator read; `helpers::view_rotation` derives the rotation from it and gets the stale
/// `(1.10871, 0.01326, 1.14827)` instead of Blender's `(1.10875, 1.2e-7, 1.15488)`. Until the
/// virtual view carries its own `viewquat`, the C primitives take it from the factory dump.
fn viewquat() -> [f32; 4] {
    static QUAT: OnceLock<[f32; 4]> = OnceLock::new();
    *QUAT.get_or_init(|| {
        let q = frap_scene::startup::factory_dump()
            .get("view3d")
            .and_then(|v| v.get("region_3d"))
            .and_then(|v| v.get("view_rotation"))
            .and_then(|v| v.as_array())
            .filter(|a| a.len() == 4)
            .map(|a| std::array::from_fn::<f32, 4, _>(|i| a[i].as_f64().unwrap_or(0.0) as f32))
            .unwrap_or([1.0, 0.0, 0.0, 0.0]);
        conjugate_qt(q)
    })
}

/// `ED_object_rotation_from_view(C, rot, align_axis)` for the virtual 3D View (verified on
/// 5.2.4 with the factory Layout viewport: `'Z'` gives `(1.10875, 1.2e-7, 1.15488)`, `'Y'`
/// `(-0.46204, 1.2e-7, 1.15488)`). `'Y'` first turns the view by 90° about `rv3d->viewinv[0]`,
/// the view's X axis in world space, taken here from `viewquat` (the result matches Blender's
/// only with an axis consistent with `viewquat`, not with the stale `view_matrix`).
pub(crate) fn view_rotation(axis: ViewAxis) -> [f32; 3] {
    let mut quat = match axis {
        ViewAxis::Z => viewquat(),
        ViewAxis::Y => {
            let x = quat_to_mat3(viewquat()).transpose().x_axis;
            let quat_y = axis_angle_to_quat([x.x, x.y, x.z], std::f32::consts::FRAC_PI_2);
            mul_qt_qtqt(viewquat(), quat_y)
        }
    };
    quat[0] = -quat[0];
    quat_to_eul_o(quat, EulerOrder::XYZ)
}

/// `ED_object_add_generic_get_opts` (is-set semantics, `helpers::placement`) then the creation
/// matrix. `align` is the operator's `align` identifier; `uses_scale` is false for the
/// operators that pass no scale to their `create_*` call (plane, circle, grid, monkey: their
/// `scale` property is ignored, verified on 5.2.4).
pub(crate) fn prim(
    cx: &OpCtx,
    location: &Prop<[f32; 3]>,
    rotation: &Prop<[f32; 3]>,
    align: &str,
    scale: &Prop<[f32; 3]>,
    uses_scale: bool,
) -> Result<Prim, OpError> {
    prim_aligned(
        cx,
        location,
        rotation,
        align,
        scale,
        uses_scale,
        ViewAxis::Z,
    )
}

/// [`prim`] with the `align='VIEW'` axis of the operator.
pub(crate) fn prim_aligned(
    cx: &OpCtx,
    location: &Prop<[f32; 3]>,
    rotation: &Prop<[f32; 3]>,
    align: &str,
    scale: &Prop<[f32; 3]>,
    uses_scale: bool,
    view_axis: ViewAxis,
) -> Result<Prim, OpError> {
    let align = Align::from_identifier(align).unwrap_or(Align::World);
    let at = if align == Align::View && !rotation.is_set {
        let from_view = Prop {
            value: view_rotation(view_axis),
            is_set: true,
        };
        helpers::placement(cx, location, &from_view, Align::World, scale)?
    } else {
        helpers::placement(cx, location, rotation, align, scale)?
    };
    let matrix = creation_matrix(
        &object_matrix(&at),
        at.location,
        at.rotation,
        uses_scale.then_some(at.scale),
    );
    Ok(Prim { at, matrix })
}

/// `--max-elements` pre-check (ARCHITECTURE §12): rejects a request whose estimated counts
/// exceed the statement budget or Blender's `int` range before anything is built, so absurd
/// values (`x_subdivisions=10**7`) fail in microseconds with `LIMIT_EXCEEDED`.
pub(crate) fn reserve(estimate: Option<frap_mesh::primitives::Estimate>) -> Result<(), OpError> {
    frap_math::budget::reserve_corners(
        estimate.map(|e| e.corners),
        estimate.map(|e| e.max_domain()),
    )
    .map_err(|e| OpError::Limit(e.to_string()))
}

/// `make_prim_finish` in Object Mode: adds the object and its mesh (both requested as `name`,
/// uniquified independently), selects and activates it, and answers `result.object`.
pub(crate) fn finish(
    cx: &mut OpCtx,
    name: &str,
    mesh: Mesh,
    at: Placement,
    enter_editmode: bool,
) -> OpResult {
    let ob = helpers::add_primitive_or_merge(cx, name, mesh, at, enter_editmode)?;
    cx.result("object", ob);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(location: [f32; 3], rotation: [f32; 3]) -> Placement {
        Placement {
            location,
            rotation,
            scale: [1.0; 3],
        }
    }

    #[test]
    fn view_rotation_is_blenders_viewquat_rotation() {
        // primitive_cube_add(align='VIEW') in the factory Layout viewport (verified on 5.2.4):
        // rotation_euler (1.1087520122528076, 1.2411510397214442e-07, 1.154877781867981).
        let r = view_rotation(ViewAxis::Z);
        let want = [1.108_752_f32, 1.241_151e-7, 1.154_877_8];
        for (a, b) in r.iter().zip(want) {
            assert!((a - b).abs() < 2e-6, "{r:?}");
        }
    }

    #[test]
    fn view_rotation_y_turns_the_view_about_its_x_axis() {
        // primitive_monkey_add(align='VIEW') in the factory Layout viewport (verified on
        // 5.2.4): rotation_euler (-0.46204436, 1.241151e-07, 1.1548778).
        let r = view_rotation(ViewAxis::Y);
        let want = [-0.462_044_36_f32, 1.241_151e-7, 1.154_877_8];
        for (a, b) in r.iter().zip(want) {
            assert!((a - b).abs() < 2e-6, "{r:?}");
        }
    }

    #[test]
    fn object_matrix_is_translation_without_rotation() {
        let m = object_matrix(&at([1.0, 2.0, 3.0], [0.0; 3]));
        assert_eq!(m[3], [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(m[0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(m[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(m[2], [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn creation_matrix_of_a_rotated_add_is_nearly_the_scale() {
        let a = at([1.0, -2.0, 0.5], [0.3, 1.2, -2.0]);
        let m = creation_matrix(
            &object_matrix(&a),
            a.location,
            a.rotation,
            Some([1.0, 2.0, 3.0]),
        );
        let want = frap_mesh::primitives::scale_matrix([1.0, 2.0, 3.0]);
        for (c, w) in m.iter().zip(want.iter()) {
            for (x, y) in c.iter().zip(w.iter()) {
                assert!((x - y).abs() < 1e-5, "{m:?}");
            }
        }
    }
}
