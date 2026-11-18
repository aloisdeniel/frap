// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers of the mesh layer hooks (`UVLoopLayers.active*`, `AttributeGroupMesh.active_color*`,
//! `default_color_name`, `render_color_index`). Not a concept: the leading underscore keeps it
//! out of the §10.5 file checks.

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_rna::{IdRef, MeshId, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg, Value};
use frap_scene::{Host, HostRead};
use smol_str::SmolStr;

use super::_common::internal;

/// The mesh at the root of a `mesh.uv_layers` / `mesh.attributes` API struct.
pub(crate) fn mesh_id(this: &RnaPtr) -> Result<IdRef<MeshId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed().ok_or_else(|| internal("not a mesh")),
        _ => Err(internal(format!(
            "{} is not rooted at a mesh",
            this.srna.identifier
        ))),
    }
}

/// The mesh read through the host: the edit-mode view while it is in Edit Mode (§6.3).
pub(crate) fn mesh(h: &dyn HostRead, this: &RnaPtr) -> Result<std::sync::Arc<Mesh>, RnaError> {
    let me = mesh_id(this)?;
    if h.main().meshes.get(me).is_none() {
        return Err(internal("the mesh was removed"));
    }
    Ok(h.mesh_view(me))
}

/// Runs `f` on the original mesh (a non-geometry write: allowed in Edit Mode, §6.3).
pub(crate) fn with_mesh_mut<R>(
    h: &mut dyn Host,
    this: &RnaPtr,
    f: impl FnOnce(&mut Mesh) -> R,
) -> Result<R, RnaError> {
    let me = mesh_id(this)?;
    let (main, log) = h.main_mut();
    let m = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("the mesh was removed"))?;
    Ok(f(m))
}

/// UV map names in collection order (`mesh.uv_layers`).
pub(crate) fn uv_names(m: &Mesh) -> Vec<SmolStr> {
    m.uv_maps().into_iter().map(|(n, _)| SmolStr::new(n)).collect()
}

/// Whether `a` is listed by `mesh.color_attributes` (FLOAT_COLOR / BYTE_COLOR on POINT or
/// CORNER).
pub(crate) fn is_color(a: &Attribute) -> bool {
    matches!(a.data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
        && matches!(a.domain, Domain::Point | Domain::Corner)
}

/// Colour attribute names in collection order.
pub(crate) fn color_names(m: &Mesh) -> Vec<SmolStr> {
    m.attributes()
        .iter()
        .filter(|a| is_color(a))
        .map(|a| a.name.clone())
        .collect()
}

/// Index of `name` in `names`, `-1` when absent (Blender's `active_index` of no layer).
pub(crate) fn index_of(names: &[SmolStr], name: Option<&str>) -> i32 {
    name.and_then(|n| names.iter().position(|x| x == n))
        .map_or(-1, |i| i as i32)
}

/// The collection item pointer `this[name]` (`mesh.uv_layers["UVMap"]`).
pub(crate) fn item_ptr(this: &RnaPtr, name: &str, srna: &'static frap_rna::StructDef) -> Value {
    Value::Ptr(Box::new(this.child(Seg::Key(name.into()), srna)))
}

/// The layer name a pointer assignment names (`= mesh.uv_layers['B']`), `None` for `= None`.
pub(crate) fn assigned_name(v: &Value, what: &str) -> Result<Option<SmolStr>, RnaError> {
    match v {
        Value::None => Ok(None),
        Value::Ptr(p) => match p.path.last() {
            Some(Seg::Key(k)) => Ok(Some(k.clone())),
            _ => Err(RnaError::new(
                RnaErrorKind::Type,
                PyExc::TypeError,
                format!("{what}: expected a layer of this mesh"),
            )),
        },
        _ => Err(RnaError::new(
            RnaErrorKind::Type,
            PyExc::TypeError,
            format!("{what}: expected a layer of this mesh"),
        )),
    }
}
