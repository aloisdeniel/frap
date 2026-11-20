// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy_struct.keyframe_delete(data_path, *, index=-1, frame=None, group="") -> bool`
//! (`pyrna_struct_keyframe_delete`, blender/source/blender/python/intern/bpy_rna_anim.cc,
//! Blender 5.2.4): the path is validated as for `keyframe_insert`, then
//! `animrig::delete_keyframe` removes the key at `frame` (the context scene's frame for
//! `None`) from the F-curves of the property in the ID's assigned slot (every index for `-1`);
//! F-curves left empty are removed. A missing `AnimData` or action raises Blender's
//! `RuntimeError` report; no key at that frame returns `False`. `group` is accepted and unused,
//! as in Blender.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::bpy_struct_keyframe_insert::frame_arg;
use crate::keying::{HostKeying, delete_keyframe, report_error};

#[rna_func(srna = "bpy_struct", func = "keyframe_delete")]
fn bpy_struct_keyframe_delete(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let prefix = "bpy_struct.keyframe_delete()";
    let data_path = args
        .get("data_path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let index = match args.get("index") {
        Some(Value::Int(i)) => i32::try_from(*i).unwrap_or(i32::MAX),
        _ => -1,
    };
    let parsed = crate::keying::parse_struct_args(&*h, this, prefix, &data_path, index)?;
    let frame = frame_arg(h, args);
    if this.srna.is_a("NlaStrip") {
        // The strip's own F-curves are not modelled (`fcu` is always found in Blender, which
        // only lets these properties be keyed when animated).
        return Ok(Value::Bool(false));
    }
    let index = (parsed.index >= 0).then_some(parsed.index);
    match delete_keyframe(&mut HostKeying(h), parsed.root, &parsed.path, index, frame) {
        Ok(n) => Ok(Value::Bool(n != 0)),
        Err(msg) => Err(report_error(msg)),
    }
}
