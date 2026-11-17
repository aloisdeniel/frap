// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.update(calc_edges=False, calc_edges_loose=False)` (`rna_Mesh_update`): derives the
//! edges of a mesh that has faces but no edges (always with `calc_edges=True`) and drops the
//! runtime caches. frap evaluates derived data fresh on every read (D-04), so the call is
//! otherwise a no-op; `calc_edges_loose` only tags a cache in Blender. Verified on 5.2.4: a
//! triangle built with `vertices.add/loops.add/polygons.add` gets its 3 edges, appended as
//! `.edge_verts` and `.select_edge`; the factory cube is unchanged.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, internal, mesh_of};

#[rna_func(srna = "Mesh", func = "update")]
fn mesh_update(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "update")?;
    let me = mesh_of(this)?;
    let calc_edges = arg_bool(args, "calc_edges", false);
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("update(): the mesh was removed"))?;
    // Only a real change bumps the mesh's generation.
    if calc_edges || (mesh.faces_num() > 0 && mesh.edges_num() == 0) {
        main.meshes
            .get_mut(me, log)
            .ok_or_else(|| internal("update(): the mesh was removed"))?
            .update(calc_edges);
    }
    Ok(Value::None)
}
