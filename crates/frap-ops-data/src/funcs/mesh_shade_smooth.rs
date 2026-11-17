// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.shade_smooth()` (Python-only, `_bpy_types.py`): removes the `sharp_face` attribute
//! (verified on 5.2.4), over `frap_mesh::Mesh::shade_smooth` (WP 1.4; moved here from WP 1.27,
//! decision log W0-22).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

#[rna_func(srna = "Mesh", func = "shade_smooth")]
fn mesh_shade_smooth(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    main.meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("shade_smooth(): the mesh was removed"))?
        .shade_smooth();
    Ok(Value::None)
}
