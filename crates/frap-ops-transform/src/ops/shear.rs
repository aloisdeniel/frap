// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.shear(angle=…, orient_axis=…, orient_axis_ortho=…, orient_type=…, …)`,
//! tier T1 (`tests/golden/cases/{object,edit}/transform_m8/shear_*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/transform/transform_mode_shear.cc (`initShear`,
//! `apply_shear`, `apply_shear_value`, `transdata_elem_shear`) and the `TRANSFORM_OT_shear`
//! exec path of `transform_ops.cc`, Blender 5.2.4, GPL-2.0-or-later.
//!
//! The shear is `x += tan(angle) · y` in the frame whose X axis is the orientation's
//! `orient_axis_ortho` axis and whose Z axis is its `orient_axis` axis (unset axes equal to
//! the other one become Z and Y, as `initShear` resets them; two equal axes both given are
//! degenerate and move every element onto the pivot, as in Blender); it is applied about the
//! pivot
//! (`tool_settings.transform_pivot_point`, each element's own origin for
//! `INDIVIDUAL_ORIGINS`), scaled by the proportional factor.
//! - Edit Mode (meshes): the selected vertices of every mesh in Edit Mode, through the
//!   edit-mode engine (`crate::edit::trans`, world space, NORMAL orientation from the
//!   selection, islands for `INDIVIDUAL_ORIGINS`, proportional editing).
//! - Object Mode: only the objects' **locations** change, and Blender shears the object's
//!   `location` channel about the world-space pivot (`td->iloc - center` with `iloc` the
//!   parent-space location and `center` the world pivot): an unparented object without delta
//!   location moves as its origin would, a parented one by the same formula in its parent's
//!   space (verified on 5.2.4).
//!
//! `orient_type` takes the custom orientation names too; unset, it is the scene orientation
//! (`transform.select_orientation`). Snapping options change nothing (as in a
//! non-interactive Blender call). Other modes answer `NotImplementedError`.

use frap_op::prelude::*;
use frap_rna::OpProps;
use frap_scene::ContextMode;
use frap_spec_ops_transform::transform;
use glam::{Mat3, Vec3};

use crate::core::orient::Orientation;
use crate::core::pivot::{self, PivotPoint};
use crate::core::proportional::{Falloff, Proportional};
use crate::edit::{Mode, Request, matrix, proportional, set};
use crate::object::data::{self, ObjectTd};

fn axis_index(a: transform::shear::OrientAxis) -> usize {
    match a {
        transform::shear::OrientAxis::X => 0,
        transform::shear::OrientAxis::Y => 1,
        transform::shear::OrientAxis::Z => 2,
    }
}

fn ortho_index(a: transform::shear::OrientAxisOrtho) -> usize {
    match a {
        transform::shear::OrientAxisOrtho::X => 0,
        transform::shear::OrientAxisOrtho::Y => 1,
        transform::shear::OrientAxisOrtho::Z => 2,
    }
}

/// `initTransInfo` reads both axes, `initShear` resets equal axes to Z and Y, then
/// `initTransform` reads the axes that were **set** again (verified: `orient_axis='X',
/// orient_axis_ortho='X'` stays degenerate and collapses everything onto the pivot, while
/// `orient_axis='X'` alone shears along Y).
fn axes(p: &transform::Shear) -> (usize, usize) {
    let (mut axis, mut ortho) = (
        axis_index(p.orient_axis.value),
        ortho_index(p.orient_axis_ortho.value),
    );
    if axis == ortho {
        (axis, ortho) = (2, 1);
    }
    if p.orient_axis.is_set {
        axis = axis_index(p.orient_axis.value);
    }
    if p.orient_axis_ortho.is_set {
        ortho = ortho_index(p.orient_axis_ortho.value);
    }
    (axis, ortho)
}

/// `initShear` stores `tanf(angle)`, `apply_shear` takes it back through `atanf` and `tanf`
/// (no snapping or numeric input in a non-interactive call).
pub fn shear_value(angle: f32) -> f32 {
    angle.tan().atan().tan()
}

/// The `orient_type`, `orient_matrix` and `orient_matrix_type` as `initTransInfo` combines
/// them (see `edit::trans::orient_props`).
fn orient_props(p: &transform::Shear) -> (Option<String>, Option<Mat3>, String) {
    let ot = set(&p.orient_type).map(|s| s.to_string());
    let mt = set(&p.orient_matrix_type).map(|s| s.to_string());
    match (matrix(&p.orient_matrix), mt) {
        (None, _) => (ot, None, "GLOBAL".into()),
        (Some(m), Some(mt)) => (Some(ot.unwrap_or_else(|| mt.clone())), Some(m), mt),
        (Some(m), None) if ot.is_none() => (Some("GLOBAL".into()), Some(m), "GLOBAL".into()),
        (Some(_), None) => (ot, None, "GLOBAL".into()),
    }
}

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    let p = transform::Shear::from_args(args);
    let (axis, ortho) = axes(&p);
    let value = shear_value(p.angle.value);
    match cx.ctx().mode() {
        ContextMode::EditMesh => {
            let mut r = Request::new(Mode::Shear { ortho });
            r.values[0] = value;
            r.orient_axis = axis;
            r.orient_type = set(&p.orient_type).map(|s| s.to_string());
            r.orient_matrix = matrix(&p.orient_matrix);
            r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
            r.proportional = proportional(
                &p.use_proportional_edit,
                p.proportional_edit_falloff.value.identifier(),
                p.proportional_size.value,
                p.use_proportional_connected.value,
                p.use_proportional_projected.value,
            );
            crate::edit::trans::run(cx, r)
        }
        ContextMode::Object => object_shear(cx, &p, axis, ortho, value),
        mode => Err(OpError::NotImplemented(format!(
            "frap: transform.shear is not implemented in {} mode yet (Object Mode and mesh \
             Edit Mode only)",
            mode.as_str()
        ))),
    }
}

/// Object Mode: `transdata_elem_shear` on every object of `createTransObject`.
fn object_shear(
    cx: &mut OpCtx,
    p: &transform::Shear,
    axis: usize,
    ortho: usize,
    value: f32,
) -> OpResult {
    let results: Vec<(ObjectTd, Vec3)> = {
        let c = cx.ctx();
        let ts = c.tool_settings();
        if ts.use_transform_data_origin || ts.use_transform_skip_children {
            return Err(OpError::NotImplemented(
                "frap: object-mode transforms with tool_settings.use_transform_data_origin or \
                 use_transform_skip_children are not implemented yet"
                    .into(),
            ));
        }
        let prop = (p.use_proportional_edit.is_set && p.use_proportional_edit.value).then(|| {
            let size = p.proportional_size.value;
            Proportional::new(
                Falloff::from_identifier(p.proportional_edit_falloff.value.identifier()),
                if size <= 0.00001 { 1.0 } else { size },
            )
        });
        let mut tds = data::create(&c, false, prop.is_some());
        if !tds.iter().any(|t| t.selected) {
            cx.report(
                ReportType::Warning,
                "frap: nothing to transform: no visible object is selected",
            );
            return Ok(OpStatus::Cancelled);
        }
        if let Some(mut prop) = prop {
            // `set_prop_dist` (centers through `td->mtx`, see the object handler) and
            // `calculatePropRatio`.
            let point = |t: &ObjectTd| t.mtx * t.center;
            let sel: Vec<Vec3> = tds.iter().filter(|t| t.selected).map(point).collect();
            for td in tds.iter_mut().filter(|t| !t.selected) {
                let q = point(td);
                let rdist = sel
                    .iter()
                    .map(|s| (*s - q).length())
                    .fold(f32::MAX, f32::min);
                td.factor = prop.factor(rdist);
            }
        }
        let (orient_type, orient_matrix, orient_matrix_type) = orient_props(p);
        let src = crate::object::orient_source(&c);
        let orient = Orientation::from_props(
            orient_type.as_deref(),
            orient_matrix,
            &orient_matrix_type,
            &src,
        );
        let around = PivotPoint::from_identifier(&ts.transform_pivot_point);
        let centers: Vec<Vec3> = tds
            .iter()
            .filter(|t| t.selected && !t.no_center)
            .map(|t| t.center)
            .collect();
        let active_center = c
            .active_object()
            .filter(|a| c.main.objects.contains(*a))
            .map(|a| frap_scene::id::transform::world_matrix_in(c.main, a).w_axis.truncate());
        let center = pivot::center(
            around,
            &centers,
            Vec3::from_array(c.cursor().location),
            active_center,
            None,
        );
        let mat_final = crate::edit::trans::shear_matrix(&orient.matrix, axis, ortho, value);
        tds.into_iter()
            .filter(|t| !t.skip)
            .map(|td| {
                let iloc = td.init.location;
                let center = if around.is_local() { td.center } else { center };
                let mut vec = mat_final * (iloc - center);
                vec += center;
                vec -= iloc;
                vec *= td.factor;
                let loc = iloc + vec;
                (td, loc)
            })
            .collect()
    };
    for (td, loc) in results {
        if loc != td.init.location {
            cx.object_mut(td.ob)?.location = loc.to_array();
        }
    }
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static SHEAR: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_SHEAR,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: crate::core::orient::ORIENT_DYN_ENUMS,
    tier: Tier::T1,
    support: Support::Partial(
        "Object Mode and mesh Edit Mode; mirror (mesh symmetry) and snapping options change \
         nothing; the Affect Only Origins/Parents tool settings are not implemented",
    ),
    notes: &[
        "In Object Mode only locations change: Blender shears each object's location channel \
         about the world-space pivot.",
        "orient_axis_ortho is the direction that moves, by tan(angle) times the coordinate \
         along orient_axis_ortho × orient_axis; passing both axes equal is degenerate and \
         moves everything onto the pivot, as in Blender.",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(SHEAR.spec, transform::Shear::spec()));
    }

    #[test]
    fn shear_matrix_moves_ortho_by_tan_times_the_third_axis() {
        // orient_axis Z, ortho X: x += value · (X × Z) = value · -Y.
        let m = crate::edit::trans::shear_matrix(&Mat3::IDENTITY, 2, 0, 0.5);
        let v = m * Vec3::new(0.0, 1.0, 0.0);
        assert!(v.abs_diff_eq(Vec3::new(-0.5, 1.0, 0.0), 1e-6), "{v:?}");
        assert!((m * Vec3::Z).abs_diff_eq(Vec3::Z, 1e-6));
        assert!((shear_value(0.5) - 0.5f32.tan()).abs() < 1e-6);
    }
}
