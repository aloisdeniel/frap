// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.groups.new(name)` (`rna_Channelbag_group_new` → `channel_group_create`, Blender
//! 5.2.4): an empty group, selected, its name made unique (`G`, `G.001`; `Group` when empty,
//! verified on 5.2.4).

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_str, internal, loc_ptr, with_action};

#[rna_func(srna = "ActionChannelbagGroups", func = "new")]
fn action_channelbag_groups_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("groups.new() outside a channelbag"));
        };
        let bag = bag_mut(act, l, s, b).ok_or_else(|| internal("no channelbag"))?;
        let g = bag.group_create(&name);
        Ok(loc_ptr(a, act, Loc::Group(l, s, b, g)))
    })
}
