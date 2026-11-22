// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_mirror(extend=False)`, tier T0
//! (`tests/golden/cases/object/select_m8/mirror*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc (`object_select_mirror_exec`) with
//! `BLI_string_flip_side_name` (`frap_scene::edit_armature::names`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `objects_selectable_poll`; for every selected visible base (the list taken before
//!   any change, base order) whose name flips (`Hand.L` ↔ `Hand.R`, `L_` ↔ `R_`,
//!   `Left` ↔ `Right`, a trailing number stripped), the object of the flipped name is selected
//!   when it has a selectable base (visible or not); without `extend` the original is then
//!   deselected (so with both sides selected, the side processed last stays selected);
//! - always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::names::flip_side_name;
use frap_spec_ops_object::object;

use super::select_grouped::SELECTABLE;
use super::select_grouped::bases::Bases;

#[operator(poll = SELECTABLE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectMirror) -> OpResult {
    let mut b = Bases::load(cx);
    for prim in b.selected() {
        let ob = b.list[prim].ob;
        let flipped = {
            let main = cx.main();
            let name = main.objects.name_of(ob).unwrap_or_default();
            let flip = flip_side_name(name, true);
            if flip != name {
                main.objects.find(&flip)
            } else {
                None
            }
        };
        if let Some(sec) = flipped.and_then(|o| b.find(o)) {
            b.select(sec, true);
        }
        if !p.extend.value {
            b.select(prim, false);
        }
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}
