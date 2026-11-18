// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.transform(mode=…, value=…, …)`, tier T1
//! (`tests/golden/cases/edit/transform/transform_generic.py`): the generic transform whose
//! `mode` picks the transform (`TRANSFORM_OT_transform`, `transform_ops.cc`, Blender 5.2.4,
//! GPL-2.0-or-later; decision log M2W0-9 assigns it to WP 2.12).
//!
//! `value` is a 4-float array: setting it constrains to every axis, as Blender does for array
//! values (`t_values_set_is_array`), so `mode='ROTATION'` with a set value scales positions by
//! `cos(angle)` like `rotate(constraint_axis=(True, True, True))`.
//! - `TRANSLATION`, `ROTATION`, `RESIZE` run in every mode with a transform handler (Object
//!   Mode through the object handler, with the matching `translate`/`rotate`/`resize`
//!   arguments);
//! - `TOSPHERE`, `SHRINKFATTEN`, `PUSHPULL`, `CREASE`, `VERTEX_CREASE`, `BWEIGHT`, `MIRROR` and
//!   `EDGESLIDE` run in Edit Mode on meshes;
//! - the other modes (`SHEAR`, `BEND`, `TILT`, `TRACKBALL`, `ALIGN`, `SKIN_RESIZE`, bone,
//!   curve, mask, time and sequencer modes, `INIT`, `DUMMY`) answer `NotImplementedError`.

use frap_op::prelude::*;
use frap_op::{TransformOp, convert_args};
use frap_rna::{OpProps, Value};
use frap_scene::ContextMode;
use frap_spec_ops_transform::transform::{self, Transform, transform::Mode as TMode};
use glam::Vec3;

use crate::edit::{Mode, Request, axis_index, matrix, proportional, set};

/// The `translate`/`rotate`/`resize` arguments equivalent to a `transform.transform` call: every
/// set property of the same name is copied, `value` is cut to the target's shape, and a set
/// `value` constrains to every axis.
fn sub_args(
    op: TransformOp,
    args: &OpArgs,
) -> Result<OpArgs, OpError> {
    let spec = match op {
        TransformOp::Rotate => &transform::OPSPEC_TRANSFORM_OT_ROTATE,
        TransformOp::Resize => &transform::OPSPEC_TRANSFORM_OT_RESIZE,
        _ => &transform::OPSPEC_TRANSFORM_OT_TRANSLATE,
    };
    let (mut out, _, _) =
        convert_args(spec, Vec::new(), Vec::new()).map_err(|e| OpError::Runtime(e.message))?;
    let src_value = match args.get("value") {
        Value::FloatArray { data, .. } => data.to_vec(),
        _ => vec![0.0; 4],
    };
    let names: Vec<&'static str> = out.values.keys().copied().collect();
    for (i, name) in names.into_iter().enumerate() {
        if !args.was_set(name) {
            continue;
        }
        let v = if name == "value" {
            match out.values.get(name) {
                Some(Value::FloatArray { shape, tag, data }) => Value::FloatArray {
                    data: src_value.iter().take(data.len()).copied().collect(),
                    shape: *shape,
                    tag: tag.clone(),
                },
                _ => Value::Float(src_value.first().copied().unwrap_or(0.0)),
            }
        } else {
            args.get(name).clone()
        };
        out.values.insert(name, v);
        out.is_set.insert(i);
    }
    if args.was_set("value")
        && let Some(i) = out.values.get_index_of("constraint_axis")
    {
        out.values.insert(
            "constraint_axis",
            Value::BoolArray(smallvec::smallvec![true, true, true]),
        );
        out.is_set.insert(i);
    }
    Ok(out)
}

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    let p = Transform::from_args(args);
    let tmode = p.mode.value;
    let generic = match tmode {
        TMode::Translation => Some(TransformOp::Translate),
        TMode::Rotation => Some(TransformOp::Rotate),
        TMode::Resize => Some(TransformOp::Resize),
        _ => None,
    };
    let in_edit_mesh = cx.ctx().mode() == ContextMode::EditMesh;
    if let Some(op) = generic
        && !in_edit_mesh
    {
        let sub = sub_args(op, args)?;
        return crate::core::dispatch(cx, op, &sub);
    }
    let mode = match tmode {
        TMode::Translation => Mode::Translate,
        TMode::Rotation => Mode::Rotate,
        TMode::Resize => Mode::Resize,
        TMode::Tosphere => Mode::ToSphere,
        TMode::Shrinkfatten => Mode::ShrinkFatten { even: false },
        TMode::Pushpull => Mode::PushPull,
        TMode::Crease => Mode::EdgeCrease,
        TMode::VertexCrease => Mode::VertCrease,
        TMode::Bweight => Mode::BevelWeight,
        TMode::Mirror => Mode::Mirror,
        TMode::Edgeslide => Mode::EdgeSlide,
        other => {
            return Err(OpError::NotImplemented(format!(
                "frap: transform.transform(mode='{}') is not implemented yet",
                other.identifier()
            )));
        }
    };
    crate::edit::require_edit_mesh(cx)?;
    if p.texture_space.value {
        return Err(OpError::NotImplemented(
            "frap: edit-mode transforms with texture_space=True are not implemented yet".into(),
        ));
    }
    if p.use_automerge_and_split.is_set && p.use_automerge_and_split.value {
        return Err(OpError::NotImplemented(
            "frap: edit-mode transforms with use_automerge_and_split=True are not implemented yet"
                .into(),
        ));
    }
    let mut r = Request::new(mode);
    if let Some(v) = set(&p.value) {
        r.values = v;
        r.values_is_array = true;
    }
    r.orient_axis = axis_index(p.orient_axis.value.identifier());
    r.orient_type = set(&p.orient_type).map(|s| s.to_string());
    r.orient_matrix = matrix(&p.orient_matrix);
    r.orient_matrix_type = p.orient_matrix_type.value.to_string();
    r.constraint_axis = set(&p.constraint_axis);
    r.has_constraint_prop = true;
    r.center_override = set(&p.center_override).map(Vec3::from_array);
    if !matches!(
        mode,
        Mode::EdgeCrease | Mode::VertCrease | Mode::BevelWeight | Mode::Mirror | Mode::EdgeSlide
    ) {
        r.proportional = proportional(
            &p.use_proportional_edit,
            p.proportional_edit_falloff.value.identifier(),
            p.proportional_size.value,
            p.use_proportional_connected.value,
            p.use_proportional_projected.value,
        );
    }
    crate::edit::trans::run(cx, r)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static TRANSFORM: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_TRANSFORM,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Partial(
        "modes TRANSLATION, ROTATION, RESIZE (every mode with a transform handler) and \
         TOSPHERE, SHRINKFATTEN, PUSHPULL, CREASE, VERTEX_CREASE, BWEIGHT, MIRROR, EDGESLIDE \
         (Edit Mode on meshes); the other modes are not implemented; snapping options change \
         nothing",
    ),
    notes: &[
        "value is an array: setting it constrains to every axis (mode='ROTATION' with a value \
         scales positions by cos(angle), as in Blender).",
    ],
};

#[cfg(test)]
mod tests {
    use frap_rna::OpProps;
    use frap_spec_ops_transform::transform;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(
            super::TRANSFORM.spec,
            transform::Transform::spec()
        ));
    }
}
