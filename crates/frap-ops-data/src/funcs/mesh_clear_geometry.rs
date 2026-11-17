// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.clear_geometry()` (`BKE_mesh_clear_geometry_and_metadata`): removes every element,
//! attribute, UV map and vertex group; keeps the material slots. Verified on 5.2.4: the factory
//! cube keeps `position`, `.edge_verts`, `.corner_vert`, `.corner_edge` (empty) and its one
//! material slot.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

#[rna_func(srna = "Mesh", func = "clear_geometry")]
fn mesh_clear_geometry(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "clear_geometry")?;
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    main.meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("clear_geometry(): the mesh was removed"))?
        .clear_geometry();
    Ok(Value::None)
}
