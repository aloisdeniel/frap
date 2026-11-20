// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.shape_key_clear()` (`rna_Object_shape_key_clear` → `BKE_object_shapekey_free`,
//! WP 6.3): removes the object's key; the mesh keeps its positions.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Key;
use frap_scene::{FuncArgs, Host};

use super::vertex_groups_new::object_of;

#[rna_func(srna = "Object", func = "shape_key_clear")]
fn object_shape_key_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "shape_key_clear")?;
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    Key::object_free(main, log, ob);
    Ok(Value::None)
}
