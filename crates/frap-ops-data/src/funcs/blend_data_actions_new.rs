// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.actions.new(name)` (`rna_Main_actions_new` → `animrig::action_add`, Blender
//! 5.2.4): an empty layered action (no slots, no layers, `is_empty`), no users; the name made
//! unique (`Action`, `Action.001`).

use frap_anim::Action;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataActions", func = "new")]
fn blend_data_actions_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(main.actions.insert(&name, Action::new(name.as_str()), log).into())
}
