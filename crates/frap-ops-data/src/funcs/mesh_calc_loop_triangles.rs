// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.calc_loop_triangles()` (`rna_Mesh_calc_loop_triangles`): Blender fills the
//! `loop_triangles` runtime array. frap computes `loop_triangles` on demand (a cached
//! `Mesh::looptris`), so the call has nothing to do and changes nothing.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

#[rna_func(srna = "Mesh", func = "calc_loop_triangles")]
fn mesh_calc_loop_triangles(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let me = mesh_of(this)?;
    h.main()
        .meshes
        .get(me)
        .ok_or_else(|| internal("calc_loop_triangles(): the mesh was removed"))?;
    Ok(Value::None)
}
