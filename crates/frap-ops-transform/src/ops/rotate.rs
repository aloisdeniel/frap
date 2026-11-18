// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.rotate(value=…, orient_axis=…, orient_type=…, …)`, tier T0
//! (`tests/golden/cases/object/transform/*.py`). Dispatches to the `TRANSFORM_MODE_HANDLERS`
//! entry of the context mode ([`crate::core::dispatch`]). Port of `TRANSFORM_OT_rotate`'s exec
//! path (`editors/transform/transform_ops.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: without `constraint_axis` the rotation is about the **negated**
//! `orient_axis` of the orientation (`value=0.5, orient_axis='Z'` gives
//! `rotation_euler.z == -0.5`); with a one-axis constraint it is about that axis (`+0.5`).
//! Registered by hand: the mode handlers take the converted `OpArgs`.

use frap_op::prelude::*;
use frap_op::TransformOp;
use frap_spec_ops_transform::transform;

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    crate::core::dispatch(cx, TransformOp::Rotate, args)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static ROTATE: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_ROTATE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::Partial(
        "Object Mode and mesh Edit Mode (pose mode M6); the Affect Only Origins/Parents tool \
         settings are not implemented; snapping options change nothing (as in a non-interactive \
         Blender call)",
    ),
    notes: &[
        "Without constraint_axis Blender rotates about the negated orient_axis: \
         rotate(value=0.5, orient_axis='Z') turns by -0.5 about Z; with \
         constraint_axis=(False, False, True) it turns by +0.5.",
    ],
};

#[cfg(test)]
mod tests {
    use frap_rna::OpProps;
    use frap_spec_ops_transform::transform;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(super::ROTATE.spec, transform::Rotate::spec()));
    }
}
