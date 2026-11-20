// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.shape_key_remove(key)` (`rna_Object_shape_key_remove` →
//! `BKE_object_shapekey_remove`, WP 6.3): removes the block; blocks relative to it fall back to
//! the basis; removing the basis writes the next block into the mesh; removing the last block
//! frees the key. A block of another key raises `RuntimeError: Error: ShapeKey not found`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Key;
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, report_error};
use super::vertex_groups_new::{mesh_of_object, object_of};

#[rna_func(srna = "Object", func = "shape_key_remove")]
fn object_shape_key_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "shape_key_remove")?;
    let ob = object_of(this)?;
    let p = arg_ptr(args, "key").ok_or_else(|| internal("shape_key_remove(): no key"))?;
    let main = h.main();
    let own = mesh_of_object(main, ob)
        .and_then(|me| main.meshes.get(me))
        .and_then(|m| m.shape_keys);
    let i = match Key::block_of(main, p) {
        Some((k, i)) if Some(k) == own => i,
        _ => return Err(report_error("ShapeKey not found")),
    };
    let (main, log) = h.main_mut();
    if !Key::object_remove(main, log, ob, i) {
        return Err(report_error("Could not remove ShapeKey"));
    }
    Ok(Value::None)
}
