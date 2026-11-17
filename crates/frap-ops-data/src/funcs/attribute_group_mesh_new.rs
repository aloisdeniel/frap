// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.new(name, type, domain)` and `mesh.color_attributes.new(…)` (one
//! `AttributeGroupMesh` function, `rna_AttributeGroup_new`) over
//! `frap_mesh::Mesh::add_attribute`, which documents the verified behaviour (unique name,
//! default values, colour defaults, refused domains). Returns
//! `bpy.data.meshes['…'].attributes["name"]` (also when called through `color_attributes`).

use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, mesh_of, report_error};

#[rna_func(srna = "AttributeGroupMesh", func = "new")]
fn attribute_group_mesh_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let me = mesh_of(this)?;
    let (name, ty, domain) = (
        arg_str(args, "name").to_owned(),
        arg_str(args, "type").to_owned(),
        arg_str(args, "domain").to_owned(),
    );
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("attributes.new(): the mesh was removed"))?;
    let name = mesh
        .add_attribute(&name, &ty, &domain)
        .map_err(|e| report_error(e.message()))?;
    let srna = mesh
        .attributes()
        .get(&name)
        .map(|a| frap_mesh::attribute_srna(&a.data))
        .ok_or_else(|| internal("attributes.new(): the attribute was not created"))?;
    Ok(Value::Ptr(Box::new(
        RnaPtr::id(me.any(), &frap_spec_rna::MESH)
            .child(Seg::Prop("attributes"), srna)
            .child(Seg::Key(name), srna),
    )))
}
