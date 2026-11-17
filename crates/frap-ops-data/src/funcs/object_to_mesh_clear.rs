// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.to_mesh_clear()` (`rna_Object_to_mesh_clear` → `BKE_object_to_mesh_clear`): frees
//! the temporary mesh `to_mesh()` gave the object. Verified on 5.2.4: it returns `None`, is a
//! no-op on an object without one (a second call, a camera), and the freed mesh then raises
//! `ReferenceError: StructRNA of type Mesh has been removed`.
//!
//! frap's `to_mesh()` result is a [`frap_rna::Detached`] view of the object's own mesh (see
//! `object_to_mesh.rs`), so there is nothing to free: the call is a no-op and never touches
//! the original data. Deviation: the old result stays readable (deviations.md).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, root_id};

#[rna_func(srna = "Object", func = "to_mesh_clear")]
fn object_to_mesh_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let ob = root_id(this)?
        .typed()
        .ok_or_else(|| internal("to_mesh_clear(): not an object"))?;
    if h.main().objects.get(ob).is_none() {
        return Err(internal("to_mesh_clear(): the object was removed"));
    }
    Ok(Value::None)
}
