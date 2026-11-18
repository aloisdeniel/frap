// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.attribute_add(name="", domain='POINT', data_type='FLOAT')`, tier T1
//! (`tests/golden/cases/edit/normals/attribute_*.py`). Port of `geometry_attribute_add_exec`
//! and `set_active_default_status_on_add` (`editors/geometry/geometry_attributes.cc`) with
//! `BKE_attribute_new` (`blenkernel/intern/attribute.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! - The name is made unique (`Attribute` when empty, `.001`… on a clash, vertex-group names
//!   included); `CURVE`, `INSTANCE` and `LAYER` are refused (`Attribute domain not supported
//!   by this geometry type`).
//! - Object Mode appends the attribute at the end of the attribute list; Edit Mode adds a
//!   BMesh layer (inserted by type among the layers of its domain) and refuses a name stored
//!   as BMesh topology or flags (`Unable to create attribute in edit mode`) or a built-in name
//!   with another domain or type.
//! - Values are the CustomData defaults (0 / False, white colours, identity quaternions and
//!   matrices).
//! - The new attribute becomes the **active attribute**; a colour attribute becomes the
//!   active and default colour when those name no colour attribute; a UV map (FLOAT2 on
//!   CORNER) becomes the active and render UV map when those name no UV map.
//!
//! Shared modules: [`attrs`] (the attribute API of both modes) and [`common`] (context, polls,
//! [`common::with_store`]).

use frap_mesh::Domain;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::geometry;

pub(crate) mod attrs;
pub(crate) mod common;
pub(crate) mod convert;

use attrs::{Names, Store};

/// The mesh domain of an `AttrDomain` identifier; `None` for CURVE / INSTANCE / LAYER.
pub(crate) fn mesh_domain(id: &str) -> Option<Domain> {
    match id {
        "POINT" => Some(Domain::Point),
        "EDGE" => Some(Domain::Edge),
        "FACE" => Some(Domain::Face),
        "CORNER" => Some(Domain::Corner),
        _ => None,
    }
}

/// `geometry_attribute_domain_itemf` (`rna_enum_attribute_domain_itemf` for a mesh): POINT,
/// EDGE, FACE and CORNER, as the dynamic items of the `domain` enum (other domains raise
/// Blender's `enum "CURVE" not found in ('POINT', 'EDGE', 'FACE', 'CORNER')`).
pub(crate) fn mesh_domain_items(_c: &frap_scene::Context) -> Vec<frap_op::EnumItemOwned> {
    geometry::enums::E0
        .iter()
        .filter(|i| mesh_domain(i.identifier).is_some())
        .map(|i| frap_op::EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// The `TypeError` Blender raises while converting the arguments when `domain` is not one of
/// [`mesh_domain_items`] (the session converts without a context, so `exec` repeats it).
pub(crate) fn domain_enum_error(requested: &str) -> OpError {
    let items = ["POINT", "EDGE", "FACE", "CORNER"];
    let list = items
        .iter()
        .map(|i| format!("'{i}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut e = frap_rna::RnaError::new(
        frap_rna::RnaErrorKind::Enum,
        frap_rna::PyExc::TypeError,
        format!(
            "Converting py args to operator properties: enum \"{requested}\" not found in ({list})"
        ),
    );
    e.detail.insert("valid".into(), items.to_vec().into());
    OpError::Rna(e)
}

/// Built-in mesh attributes with a fixed domain and type
/// (`AttributeAccessor::get_builtin_domain_and_type`).
fn builtin_domain_and_type(name: &str) -> Option<(Domain, &'static str)> {
    Some(match name {
        "position" => (Domain::Point, "FLOAT_VECTOR"),
        ".edge_verts" => (Domain::Edge, "INT32_2D"),
        ".corner_vert" | ".corner_edge" => (Domain::Corner, "INT"),
        "material_index" => (Domain::Face, "INT"),
        "sharp_face" => (Domain::Face, "BOOLEAN"),
        "sharp_edge" => (Domain::Edge, "BOOLEAN"),
        "id" => (Domain::Point, "INT"),
        _ => return None,
    })
}

/// `set_active_default_status_on_add`.
pub(crate) fn set_status_on_add(
    store: &dyn Store,
    names: &mut Names,
    name: &str,
    domain: Domain,
    data_type: &str,
) {
    let list = store.layers();
    if attrs::is_color(domain, data_type) {
        let found = |n: &Option<smol_str::SmolStr>| {
            n.as_deref().is_some_and(|n| {
                list.iter()
                    .any(|l| l.name == n && attrs::is_color(l.domain, l.data_type))
            })
        };
        if !found(&names.color_active) {
            names.color_active = Some(name.into());
        }
        if !found(&names.color_default) {
            names.color_default = Some(name.into());
        }
    } else if attrs::is_uv(name, domain, data_type) {
        let found = |n: &Option<smol_str::SmolStr>| {
            n.as_deref().is_some_and(|n| {
                list.iter()
                    .any(|l| l.name == n && attrs::is_uv(&l.name, l.domain, l.data_type))
            })
        };
        if !found(&names.uv_active) {
            names.uv_active = Some(name.into());
        }
        if !found(&names.uv_render) {
            names.uv_render = Some(name.into());
        }
    }
}

/// `BKE_attribute_new` (Edit Mode) / `AttributeStorage::add` (Object Mode) with the checks of
/// both paths: the new attribute's unique name, or Blender's error report.
pub(crate) fn add(
    store: &mut dyn Store,
    vgroups: &[smol_str::SmolStr],
    requested: &str,
    domain: Option<Domain>,
    data_type: &str,
) -> Result<smol_str::SmolStr, String> {
    let Some(domain) = domain else {
        return Err("Attribute domain not supported by this geometry type".into());
    };
    let name = attrs::unique_name(store, vgroups, requested);
    if store.is_edit() {
        let check = if requested.is_empty() {
            "Attribute"
        } else {
            requested
        };
        if frap_bmesh::convert::stored_in_bmesh_builtin(check) {
            return Err("Unable to create attribute in edit mode".into());
        }
        if let Some((d, t)) = builtin_domain_and_type(check) {
            if d != domain {
                return Err(format!("Domain unsupported for \"{check}\" attribute"));
            }
            if t != data_type {
                return Err(format!("Type unsupported for \"{check}\" attribute"));
            }
        }
    }
    attrs::add_default(store, name.clone(), domain, data_type)
        .ok_or_else(|| "Attribute type not supported".to_string())?;
    Ok(name)
}

#[operator(
    poll = common::ATTRIBUTES_POLL,
    tier = T1,
    dyn_enums = [("domain", mesh_domain_items)]
)]
fn exec(cx: &mut OpCtx, p: geometry::AttributeAdd) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let domain = mesh_domain(p.domain.value.identifier());
    if domain.is_none() {
        return Err(domain_enum_error(p.domain.value.identifier()));
    }
    let data_type = p.data_type.value.identifier();
    let requested = p.name.value.clone();
    let r = common::with_store(cx, me, |store, names, vgroups| {
        let name = add(store, vgroups, &requested, domain, data_type)?;
        names.active = Some(name.clone());
        let d = domain.expect("checked by add");
        set_status_on_add(store, names, &name, d, data_type);
        Ok::<_, String>(name)
    })?;
    match r {
        Ok(_) => Ok(OpStatus::Finished),
        Err(m) => Err(OpError::Runtime(m)),
    }
}
