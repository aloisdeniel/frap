// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.duplicate_move_linked(OBJECT_OT_duplicate={…},
//! TRANSFORM_OT_translate={…})`, a macro (`OBJECT_OT_duplicate_move_linked`,
//! `editors/object/object_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `object.duplicate` with
//! `linked=True` then `transform.translate`. The macro definition sets the step's `linked`
//! (`RNA_boolean_set(otmacro->ptr, "linked", true)`), so the copies always share the object
//! data, even with `OBJECT_OT_duplicate={'linked': False}` (verified on 5.2.4); frap applies
//! it through `frap_op::pipeline::MACRO_STEP_FORCED`. This exec is the post-step hook.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(poll = poll::OBJECTMODE, tier = T0, kind = Macro)]
fn exec(_cx: &mut OpCtx, _p: object::DuplicateMoveLinked) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicate_step_is_forced_linked() {
        let forced = frap_op::pipeline::macro_step_forced(
            "OBJECT_OT_duplicate_move_linked",
            "OBJECT_OT_duplicate",
        );
        assert_eq!(forced, &[("linked", true)]);
        assert!(
            frap_op::pipeline::macro_step_forced("OBJECT_OT_duplicate_move", "OBJECT_OT_duplicate")
                .is_empty()
        );
    }
}
