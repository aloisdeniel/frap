// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.evaluated_get(depsgraph)` (`rna_ID_evaluated_get` → `DEG_get_evaluated_id`): an
//! evaluation boundary (§7.3: pending animation is applied and the depsgraph-lite updated),
//! then the ID itself.
//!
//! Blender returns a separate evaluated copy (`is_evaluated == True`, repr
//! `<bpy_struct, Object("Cube") at 0x…, evaluated>`); frap has no evaluated IDs of its own and
//! returns the original, whose derived reads (`matrix_world`, `dimensions`, `to_mesh()`) are
//! always fresh (D-04). An ID outside the depsgraph (an object in no scene) is the original in
//! Blender too.

use frap_rna::{RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::internal;

#[rna_func(srna = "ID", func = "evaluated_get")]
fn i_d_evaluated_get(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    h.apply_pending_animation()?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    match this.root {
        Root::Id(a) if this.path.is_empty() => Ok(Value::Id(Some(a))),
        Root::Embedded { .. } => Ok(Value::Ptr(Box::new(this.clone()))),
        _ => Err(internal("ID.evaluated_get(): not a datablock")),
    }
}
