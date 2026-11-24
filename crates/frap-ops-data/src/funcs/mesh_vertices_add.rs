// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.vertices.add(count)` (`rna_Mesh_vertices_add` → `ED_mesh_verts_add`,
//! `editors/mesh/mesh_data.cc`), Blender 5.2.4, GPL-2.0-or-later: appends `count` default
//! elements ([`frap_mesh::Mesh::add_elements`], verified on 5.2.4); `count <= 0` adds
//! nothing. The low-level way to build a mesh with `foreach_set` (`vertices.add`,
//! `loops.add`, `polygons.add`, then `update(calc_edges=True)`). Refused in Edit Mode
//! (`RNA_EDITMODE_WRITE`, D-06).

use frap_mesh::Domain;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_int, internal, mesh_of};

#[rna_func(srna = "MeshVertices", func = "add")]
fn mesh_vertices_add(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "add")?;
    let me = mesh_of(this)?;
    let count = arg_int(args, "count", 0);
    if count <= 0 {
        return Ok(Value::None);
    }
    let (main, log) = h.main_mut();
    main.meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("vertices.add(): the mesh was removed"))?
        .add_elements(Domain::Point, count);
    Ok(Value::None)
}
