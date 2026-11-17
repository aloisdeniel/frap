// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.remove(attribute)` (also through `color_attributes`;
//! `rna_AttributeGroup_remove` → `BKE_attribute_remove`) over
//! `frap_mesh::Mesh::remove_attribute`: a required attribute raises `RuntimeError: Error:
//! Attribute is required and cannot be removed` (verified on 5.2.4). Blender removes by name
//! on the called mesh.

use frap_mesh::Domain;
use frap_rna::{MeshId, RnaError, RnaPtr, Root, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, mesh_of, report_error};

#[rna_func(srna = "AttributeGroupMesh", func = "remove")]
fn attribute_group_mesh_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let me = mesh_of(this)?;
    let target = arg_ptr(args, "attribute")
        .cloned()
        .ok_or_else(|| internal("attributes.remove(): no attribute pointer"))?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("attributes.remove(): the mesh was removed"))?;
    // The attribute's name from its pointer: `…attributes["name"]` or `…[i]` of
    // `attributes` / `color_attributes` (the source mesh's list).
    let source = match target.root {
        Root::Id(id) => id.typed::<MeshId>().and_then(|m| main.meshes.get(m)),
        _ => None,
    }
    .unwrap_or(mesh);
    let name = match target.path.as_slice() {
        [.., Seg::Prop(coll), Seg::Index(i)] => {
            let colors = *coll == "color_attributes";
            source
                .attributes()
                .iter()
                .filter(|a| {
                    !colors
                        || (matches!(
                            a.data,
                            frap_mesh::AttrData::FloatColor(_) | frap_mesh::AttrData::ByteColor(_)
                        ) && matches!(a.domain, Domain::Point | Domain::Corner))
                })
                .nth(usize::try_from(*i).unwrap_or(usize::MAX))
                .map(|a| a.name.clone())
        }
        [.., Seg::Key(k)] => Some(k.clone()),
        _ => None,
    }
    .ok_or_else(|| internal("attributes.remove(): cannot name the attribute"))?;
    main.meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("attributes.remove(): the mesh was removed"))?
        .remove_attribute(&name)
        .map_err(|e| report_error(e.message()))?;
    Ok(Value::None)
}
