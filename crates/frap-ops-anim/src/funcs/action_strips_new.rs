// SPDX-License-Identifier: GPL-3.0-or-later
//! `layer.strips.new(type='KEYFRAME')` (`rna_ActionStrips_new` → `Layer::strip_add`, Blender
//! 5.2.4): an infinite keyframe strip without channelbags. Blender 5.2 allows one strip per
//! layer: a second raises `RuntimeError: Error: A layer may not have more than one strip`
//! (verified).

use frap_anim::rna::action::Loc;
use frap_anim::{ActionStrip, KeyframeStrip};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, internal, loc_ptr, report_error, with_action};

#[rna_func(srna = "ActionStrips", func = "new")]
fn action_strips_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Layer(l) = loc else {
            return Err(internal("strips.new() outside a layer"));
        };
        let layer = &mut act.layers[l];
        if !layer.strips.is_empty() {
            return Err(report_error("A layer may not have more than one strip"));
        }
        layer
            .strips
            .push(ActionStrip::Keyframe(KeyframeStrip::default()));
        let s = layer.strips.len() - 1;
        Ok(loc_ptr(a, act, Loc::Strip(l, s)))
    })
}
