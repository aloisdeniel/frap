// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.calc_tangents(uvmap)` (`rna_Mesh_calc_tangents`, makesrna/intern/rna_mesh_api.cc, with
//! `bke::mesh::calc_uv_tangent_tris_quads`, blenkernel/intern/mesh_tangent.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Fills the temporary corner tangent layer (`CD_MLOOPTANGENT`) that `MeshLoop.tangent`,
//! `bitangent_sign` and `bitangent` read: MikkTSpace over the mesh faces with the corner
//! normals and the UV map named `uvmap` (the active UV map when the argument is omitted; an
//! explicit `""` names no map, verified on 5.2.4). The UV map is read the way
//! `attributes.lookup<float2>(uvmap, Corner)` reads it: any attribute of that name, its values
//! interpolated to the corners and converted to `float2` (a POINT FLOAT2 attribute, or
//! `uvmap='position'`, works; verified on 5.2.4). Errors:
//! - no such attribute: `RuntimeError: Error: Tangent space computation needs a UV Map,
//!   "<name>" not found, aborting`;
//! - a face with more than four corners: `RuntimeError: Error: Tangent space can only be
//!   computed for tris/quads, aborting`.
//!
//! The layer lives as long as Blender's temporary layer: the stale values survive geometry
//! edits and attribute removals in Object Mode, but an ID copy (`mesh.copy()`) and every
//! BMesh → Mesh conversion (leaving Edit Mode) read zero tangents (`frap_mesh::Mesh`'s
//! `tangents` field). Deviation: Blender zeroes the layer before it fails, so a failing call
//! leaves zero tangents behind; frap rolls the failing statement back, so the tangents of
//! the previous successful call stay.

use frap_mesh::Domain;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, mesh_of, report_error};

#[rna_func(srna = "Mesh", func = "calc_tangents")]
fn mesh_calc_tangents(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "calc_tangents")?;
    let me = mesh_of(this)?;
    let given = args
        .values
        .get_index_of("uvmap")
        .is_some_and(|i| args.is_set.contains(i));
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("calc_tangents(): the mesh was removed"))?;
    let uvmap: String = if given {
        arg_str(args, "uvmap").to_owned()
    } else {
        mesh.uv_active.as_deref().unwrap_or_default().to_owned()
    };
    mesh.set_corner_tangents(Some(vec![[0.0; 4]; mesh.corners_num()]));
    let uv = mesh
        .lookup_float2(&uvmap, Domain::Corner)
        .map(|v| v.into_owned());
    let Some(uv) = uv.filter(|v| !v.is_empty()) else {
        return Err(report_error(format!(
            "Tangent space computation needs a UV Map, \"{uvmap}\" not found, aborting"
        )));
    };
    let normals = mesh.corner_normals();
    let tangents = frap_geom::mikktspace::calc_uv_tangent_tris_quads(
        mesh.positions(),
        mesh.face_offsets(),
        mesh.corner_verts(),
        &normals,
        &uv,
    )
    .map_err(report_error)?;
    mesh.set_corner_tangents(Some(tangents));
    Ok(Value::None)
}
