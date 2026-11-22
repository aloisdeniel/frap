// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.extrude_forked(ARMATURE_OT_extrude={…}, TRANSFORM_OT_translate={…})`, a
//! macro, tier T0 (WP 8.3.10; `ED_operatormacros_armature`,
//! blender/source/blender/editors/armature/armature_ops.cc, Blender 5.2.4,
//! GPL-2.0-or-later): `armature.extrude(forked=True)` then `transform.translate`. The macro
//! definition sets the step's `forked` (`RNA_boolean_set(otmacro->ptr, "forked", true)`), so
//! `ARMATURE_OT_extrude={'forked': False}` still forks (verified on 5.2.4); frap applies it
//! through `frap_op::pipeline::MACRO_STEP_FORCED`. With `Armature.use_mirror_x` a bone
//! without an X mirror gets a `_L`/`_R` pair of children. This exec is the macro's post-step
//! hook and changes nothing.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    kind = Macro,
    spec = armature::OPSPEC_ARMATURE_OT_EXTRUDE_FORKED
)]
fn exec(_cx: &mut OpCtx, _p: armature::ExtrudeForked) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    #[test]
    fn extrude_step_is_forced_forked() {
        let forced = frap_op::pipeline::macro_step_forced(
            "ARMATURE_OT_extrude_forked",
            "ARMATURE_OT_extrude",
        );
        assert_eq!(forced, &[("forked", true)]);
    }
}
