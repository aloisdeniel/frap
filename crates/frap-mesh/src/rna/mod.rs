// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of `Mesh` and its element structs (ARCHITECTURE §6.1). M0: `name` only;
//! WP 1.27 owns this directory (`MeshVertex`, `MeshEdge`, `MeshPolygon`, `MeshLoop`, … as SoA
//! proxies, `foreach_get`/`foreach_set` bulk overrides). The Python-only RNA functions
//! `Mesh.from_pydata`/`shade_smooth`/`shade_flat` are registered by WP 1.22 in
//! `frap-ops-data/src/funcs/` (they need `frap_scene::RNA_FUNCS`, which L4 cannot reach;
//! decision log W0-22); this crate provides the Rust `Mesh::from_pydata` (WP 1.4).

use frap_rna::*;
use frap_spec_rna::{MESH, id};

use crate::mesh::Mesh;

fn no_attr(prop: &PropDef) -> RnaError {
    frap_rna::traits::no_attribute(&MESH, prop.identifier)
}

impl Rna for Mesh {
    fn srna(&self) -> &'static StructDef {
        &MESH
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if std::ptr::eq(prop, id::NAME) {
            return Ok(Field::Value(Value::Str(self.header.name.clone())));
        }
        Err(no_attr(prop))
    }
}

impl RnaMut for Mesh {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        if std::ptr::eq(prop, id::NAME) {
            return Ok(SetOutcome::NeedsHook); // unique_name + re-sort (§9.5)
        }
        Err(no_attr(prop))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(no_attr(prop))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(no_attr(prop))
    }
}

impl IdType for Mesh {
    type Kind = MeshId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        for m in self.materials.iter().flatten() {
            f(m.any());
        }
        if let Some(k) = self.shape_keys {
            f(k.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        for m in self.materials.iter_mut() {
            if m.map(|r| r.any()) == Some(from) {
                *m = to.and_then(IdRef::from_any);
            }
        }
        if self.shape_keys.map(|r| r.any()) == Some(from) {
            self.shape_keys = to.and_then(IdRef::from_any);
        }
    }
}
