// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.translate(value=…, orient_type=…, constraint_axis=…, …)`, tier T0
//! (`tests/golden/cases/object/transform/*.py`). The dispatcher (owned once, §10.3): it runs
//! the `TRANSFORM_MODE_HANDLERS` entry of the context mode (`object/` M1; edit mode M2, pose
//! mode M6 add handlers), see [`crate::core::dispatch`]. Port of `TRANSFORM_OT_translate`'s
//! exec path (`editors/transform/transform_ops.cc`, Blender 5.2.4, GPL-2.0-or-later); the poll
//! (`ED_operator_screenactive`) always passes in frap's virtual window.
//!
//! The operator is registered by hand rather than with `#[operator]` because the mode
//! handlers take the converted `OpArgs` (`TransformModeHandler::apply`).

use frap_op::TransformOp;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    crate::core::dispatch(cx, TransformOp::Translate, args)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static TRANSLATE: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_TRANSLATE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    // Custom orientations by name (WP 8.3.5).
    dyn_enums: crate::core::orient::ORIENT_DYN_ENUMS,
    tier: Tier::T0,
    support: Support::Partial(
        "Object Mode and mesh Edit Mode (pose mode M6); texture_space, translate_origin and the \
         Affect Only Origins/Parents tool settings are not implemented; snapping options change \
         nothing (as in a non-interactive Blender call)",
    ),
    notes: &[
        "An array value constrains to every axis: constraint_axis is ignored when value is set \
         (Blender behaves the same).",
        "orient_type='LOCAL' with several objects moves each object along its own axes.",
    ],
};

#[cfg(test)]
mod tests {
    use frap_rna::OpProps;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(
            super::TRANSLATE.spec,
            transform::Translate::spec()
        ));
        assert_eq!(super::TRANSLATE.spec.idname, "transform.translate");
    }

    use frap_spec_ops_transform::transform;
}
