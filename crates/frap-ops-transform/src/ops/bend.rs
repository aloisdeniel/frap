// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.bend(value=(…,), …)`, tier T0
//! (`tests/golden/cases/{object,edit}/transform_m8/bend_*.py`, WP 8.3.5).
//!
//! Blender 5.2.4 registers `TRANSFORM_OT_bend` without an exec callback
//! (`transform_ops.cc`: `// ot->exec = transform_exec; /* Unsupported. */`): the bend is defined
//! between the 3D cursor and the **mouse** (`initBend` reads `t->mval` for its end point and
//! radius), so only the interactive modal tool bends. Called from Python (also with
//! `'INVOKE_DEFAULT'`, which `-b` runs as exec) it logs `Invalid operator call
//! 'TRANSFORM_OT_bend'`, changes nothing and returns `{'PASS_THROUGH'}`, in Object and Edit
//! Mode, with every pivot (verified on 5.2.4). frap answers the same no-op as `FINISHED` (its
//! answer for `PASS_THROUGH`), with a WARNING that names the alternatives.

use frap_op::prelude::*;
use frap_rna::OpProps;
use frap_spec_ops_transform::transform;

/// The WARNING of every call.
pub const WARNING: &str = "frap: transform.bend did nothing: Blender 5.2 has no exec for it \
     (the bend runs only interactively, between the 3D cursor and the mouse; a script call \
     logs \"Invalid operator call 'TRANSFORM_OT_bend'\" and returns PASS_THROUGH)";

fn exec(cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    // The arguments are converted (and checked) like Blender's; nothing reads them.
    let _ = transform::Bend::from_args(args);
    cx.report(ReportType::Warning, WARNING);
    cx.hint(
        "BEND_NO_EXEC",
        "bend with a modifier instead: bpy.ops.object.modifier_add(type='SIMPLE_DEFORM') then \
         set its deform_method = 'BEND', angle and deform_axis (and apply it with \
         bpy.ops.object.modifier_apply), or rotate the selection in steps with \
         transform.rotate and proportional editing",
    );
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static BEND: OperatorDef = OperatorDef {
    spec: &transform::OPSPEC_TRANSFORM_OT_BEND,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::Full,
    notes: &[
        "Blender 5.2 has no exec for transform.bend: a script call changes nothing \
         (PASS_THROUGH, answered FINISHED with a WARNING). Bend with a SIMPLE_DEFORM modifier \
         (deform_method='BEND') instead.",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_with_the_generated_spec() {
        assert!(std::ptr::eq(BEND.spec, transform::Bend::spec()));
    }
}
