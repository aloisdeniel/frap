// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.shape_key_add(name='Key', from_mix=True)` (`rna_Object_shape_key_add` →
//! `BKE_object_shapekey_insert`, WP 6.3): adds a key block and returns it. The first block of
//! a new key copies the mesh positions; later ones copy the mesh (`from_mix=False`) or the
//! current mix of the key (`from_mix=True`, `BKE_key_evaluate_object`: values, vertex groups,
//! `show_only_shape_key`). The new block's value is 1 (clamped to its slider range), the name
//! is unique in the key (`Key`, `Key.001`, …). A non-mesh object raises `RuntimeError: Error:
//! Object 'Camera' does not support shapes`.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_object_api.cc and
//! blender/source/blender/blenkernel/intern/object.cc (Blender 5.2.4, GPL-2.0-or-later).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Key;
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, arg_str, report_error};
use super::vertex_groups_new::object_of;

#[rna_func(srna = "Object", func = "shape_key_add")]
fn object_shape_key_add(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "shape_key_add")?;
    let ob = object_of(this)?;
    let name = arg_str(args, "name").to_owned();
    let from_mix = arg_bool(args, "from_mix", true);
    let (main, log) = h.main_mut();
    let Some((k, i)) = Key::object_insert(main, log, ob, Some(&name), from_mix) else {
        let n = main.objects.name_of(ob).unwrap_or_default().to_owned();
        return Err(report_error(format!(
            "Object '{n}' does not support shapes"
        )));
    };
    let key = main
        .shape_keys
        .get_mut(k, log)
        .ok_or_else(|| super::_common::internal("shape_key_add(): the key was removed"))?;
    let kb = &mut key.key_blocks[i];
    kb.set_value(1.0);
    Ok(Value::Ptr(Box::new(Key::block_ptr(k, &kb.name.clone()))))
}
