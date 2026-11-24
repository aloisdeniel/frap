// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.validate(verbose=False, clean_customdata=True)` → `bool` (`BKE_mesh_validate`):
//! `True` when the mesh had to be changed. frap's mesh builders (`from_pydata`, primitives,
//! operators) never produce an invalid mesh, so a valid mesh answers `False` like Blender
//! (verified on 5.2.4: the factory cube, `verbose=True` too). Repaired as in Blender
//! (M8S10-3, verified on 5.2.4): non-finite vertex positions (zeroed), duplicate edges and
//! duplicate faces (removed, the first kept; see `Mesh::validate_repair`). A mesh made invalid
//! otherwise through low-level writes (`foreach_set` of out-of-range indices, `polygons.add` /
//! `loops.add` without the matching corners or edges: a face of fewer than 3 corners or using
//! a vertex twice, a corner whose edge does not join its vertex to the next one) is not
//! repaired yet: frap raises `NotImplementedError` (`RNA_NOT_IMPLEMENTED`) naming the first
//! problem rather than answering `False` for a broken mesh (deviations.md §2, M9; Blender
//! answers `True`).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of, not_implemented};

#[rna_func(srna = "Mesh", func = "validate")]
fn mesh_validate(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("validate(): the mesh was removed"))?;
    if let Some(problem) = mesh.validate_report() {
        return Err(not_implemented(format!(
            "Mesh.validate() cannot repair this mesh yet ({problem})"
        )));
    }
    // Write (and log the mesh modified) only when there is something to repair.
    let mut probe = mesh.clone();
    if !probe.validate_repair().map_err(internal)? {
        return Ok(Value::Bool(false));
    }
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("validate(): the mesh was removed"))?;
    *mesh = probe;
    Ok(Value::Bool(true))
}
