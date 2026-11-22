// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.parent_inverse_apply()`, tier T1
//! (`tests/golden/cases/object/transform_m8/parent_inverse_apply*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/object/object_transform.cc
//! (`object_parent_inverse_apply_exec`) and blenkernel/intern/object.cc
//! (`BKE_object_apply_parent_inverse`), Blender 5.2.4, GPL-2.0-or-later. Poll
//! `ED_operator_objectmode`.
//!
//! Every selected editable object with a parent gets the parent's orthonormalized world
//! transform as its origin (`matrix_parent_inverse = parent⁻¹ · orthonormal(parent)`) and its
//! channels recomputed (no compatible Euler) so that its world matrix stays; shear of a
//! non-uniformly scaled parent is lost. The world matrices are those evaluated before the
//! operator (a selected parent and child both use the old ones). Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::xform::{Worlds, apply_parent_inverse};

#[operator(poll = poll::OBJECTMODE, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::ParentInverseApply) -> OpResult {
    let (objects, worlds) = {
        let c = cx.ctx();
        (c.selected_editable_objects(), Worlds::of(c.main))
    };
    for ob in objects {
        let parent = {
            let main = cx.main();
            let Some(o) = main.objects.get(ob) else {
                continue;
            };
            worlds.parent_matrix(main, o)
        };
        let Some(pw) = parent else { continue };
        let world = worlds.get(ob);
        apply_parent_inverse(cx.object_mut(ob)?, &pw, &world);
    }
    Ok(OpStatus::Finished)
}
