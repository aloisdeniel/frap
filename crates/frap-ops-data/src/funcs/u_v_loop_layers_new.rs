// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.uv_layers.new(name="UVMap", do_init=True)` (`rna_Mesh_uv_layers_new` →
//! `ED_mesh_uv_add`) over `frap_mesh::Mesh::add_uv_map`, which documents the verified
//! behaviour. Past 8 UV maps Blender returns `None` with the WARNING `Cannot add more than 8
//! UV maps` (verified on 5.2.4).

use frap_rna::{Report, ReportType, RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, arg_str, internal, mesh_of};

#[rna_func(srna = "UVLoopLayers", func = "new")]
fn u_v_loop_layers_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let me = mesh_of(this)?;
    let name = arg_str(args, "name").to_owned();
    let do_init = arg_bool(args, "do_init", true);
    let (main, log) = h.main_mut();
    let added = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("uv_layers.new(): the mesh was removed"))?
        .add_uv_map(&name, do_init);
    let Some(name) = added else {
        h.report(Report::new(
            ReportType::Warning,
            format!("Cannot add more than {} UV maps", frap_mesh::MAX_UV_MAPS),
        ));
        return Ok(Value::None);
    };
    let srna = &frap_spec_rna::MESH_UV_LOOP_LAYER;
    Ok(Value::Ptr(Box::new(
        RnaPtr::id(me.any(), &frap_spec_rna::MESH)
            .child(Seg::Prop("uv_layers"), srna)
            .child(Seg::Key(name), srna),
    )))
}
