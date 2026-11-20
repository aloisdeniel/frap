// SPDX-License-Identifier: GPL-3.0-or-later
//! `VertexGroup.weight(index)` (`rna_VertexGroup_weight` → `vgroup_vert_weight`, WP 6.3): the
//! weight of vertex `index` in the group. Verified on 5.2.4: an unassigned vertex of a mesh
//! with weights, or any vertex of a mesh without weights, raises `RuntimeError: Error: Vertex
//! not in group`; an index past the last vertex of a mesh with weights answers `0.0`. In Edit
//! Mode the edit BMesh is read (`0.0` past the last vertex or without a weight layer). A
//! negative index (out-of-bounds read in Blender) raises `Vertex not in group`.

use frap_bmesh::shape::deform;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_int, report_error};
use super::vertex_groups_new::{group_of, mesh_of_object};

/// `get_vert_def_nr`: `Some(weight)`, `None` for "not in group".
fn vert_weight(h: &dyn Host, this: &RnaPtr, index: i64) -> Option<f32> {
    let main = h.main();
    let (ob, g) = group_of(main, this)?;
    let me = mesh_of_object(main, ob)?;
    let index = usize::try_from(index).ok()?;
    if let Some(em) = h.edit().meshes.get(&me.key()) {
        let bm = &em.bm;
        let verts = bm.vert_ids();
        let Some(d) = bm.vdata.deform.as_ref() else {
            return Some(0.0);
        };
        let Some(v) = verts.get(index) else {
            return Some(0.0);
        };
        let dv = d.get(v.slot())?;
        return deform::find(dv, g as u32).map(|i| dv.weights[i].1);
    }
    let m = main.meshes.get(me)?;
    let dv = m.deform_verts()?;
    if index >= m.verts_num() {
        return Some(0.0);
    }
    let d = &dv[index];
    deform::find(d, g as u32).map(|i| d.weights[i].1)
}

#[rna_func(srna = "VertexGroup", func = "weight")]
fn vertex_group_weight(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let index = arg_int(args, "index", 0);
    match vert_weight(h, this, index) {
        Some(w) => Ok(Value::Float(w)),
        None => Err(report_error("Vertex not in group")),
    }
}
