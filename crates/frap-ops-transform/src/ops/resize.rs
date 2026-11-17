// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.resize(value=…, orient_type=…, …)`, tier T0
//! (`tests/golden/cases/object/transform/*.py`). Dispatches to the `TRANSFORM_MODE_HANDLERS`
//! entry of the context mode ([`crate::core::dispatch`]). Port of `TRANSFORM_OT_resize`'s exec
//! path (`editors/transform/transform_ops.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: objects keep their rotation and take the new size measured along their
//! own axes, so a non-uniform resize of a rotated object loses the shear (scale
//! `(1.8195, 1.2998, 1.0)` for `value=(2, 1, 1)` on a cube turned 0.5 rad about Z); `resize()`
//! without a value scales to zero. Registered by hand: the mode handlers take the converted
//! `OpArgs`.

use frap_op::prelude::*;
use frap_op::TransformOp;
use frap_spec_ops_transform::transform;

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    crate::core::dispatch(cx, TransformOp::Resize, args)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static RESIZE: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_RESIZE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::Partial(
        "object mode only (edit mode M2, pose mode M6); texture_space and the Affect Only \
         Origins/Parents tool settings are not implemented; snapping options change nothing (as \
         in a non-interactive Blender call)",
    ),
    notes: &[
        "Resize keeps each object's rotation: a non-uniform resize of a rotated object loses \
         the shear, exactly as in Blender.",
        "resize() without value scales to zero (Blender's unset value is (0, 0, 0)).",
    ],
};

#[cfg(test)]
mod tests {
    use frap_rna::OpProps;
    use frap_spec_ops_transform::transform;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(super::RESIZE.spec, transform::Resize::spec()));
    }
}
