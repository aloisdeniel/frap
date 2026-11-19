// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.calc_tangents(uvmap)` (`rna_Mesh_calc_tangents`, makesrna/intern/rna_mesh_api.cc, with
//! `bke::mesh::calc_uv_tangent_tris_quads`, blenkernel/intern/mesh_tangent.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Fills the temporary corner tangent layer (`CD_MLOOPTANGENT`) that `MeshLoop.tangent`,
//! `bitangent_sign` and `bitangent` read: MikkTSpace over the mesh faces with the corner
//! normals and the UV map named `uvmap` (the active UV map when the argument is omitted; an
//! explicit `""` names no map, verified on 5.2.4). The layer is (re)created zeroed first, so a
//! failing call leaves zero tangents behind, as Blender's does:
//! - no such UV map: `RuntimeError: Error: Tangent space computation needs a UV Map, "<name>"
//!   not found, aborting`;
//! - a face with more than four corners: `RuntimeError: Error: Tangent space can only be
//!   computed for tris/quads, aborting`.

use frap_mesh::{AttrData, Domain};
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
        .attributes()
        .get(&uvmap)
        .and_then(|a| match (&a.domain, &a.data) {
            (Domain::Corner, AttrData::Float2(v)) => Some(v.clone()),
            _ => None,
        });
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
