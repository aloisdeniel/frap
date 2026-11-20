// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.groups.remove(action_group)` (`rna_Channelbag_group_remove` →
//! `channel_group_remove`, Blender 5.2.4): the group; its F-curves stay, ungrouped, at the start
//! of the ungrouped ones. A group of another channelbag raises `RuntimeError: Error: Could not
//! remove the F-Curve Group from the collection because it does not exist in the collection`.

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, internal, loc_arg, report_error, with_action};

#[rna_func(srna = "ActionChannelbagGroups", func = "remove")]
fn action_channelbag_groups_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let target = arg_ptr(args, "action_group").and_then(|p| loc_arg(h.main(), a, p));
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("groups.remove() outside a channelbag"));
        };
        match target {
            Some(Loc::Group(gl, gs, gb, g)) if (gl, gs, gb) == (l, s, b) => {
                if let Some(bag) = bag_mut(act, l, s, b) {
                    bag.group_remove(g);
                }
                Ok(Value::None)
            }
            _ => Err(report_error(
                "Could not remove the F-Curve Group from the collection because it does not \
                 exist in the collection",
            )),
        }
    })
}
