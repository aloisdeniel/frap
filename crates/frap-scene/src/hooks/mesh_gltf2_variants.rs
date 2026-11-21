// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.gltf2_variant_mesh_data`, `Mesh.gltf2_variant_default_materials` and
//! `Mesh.gltf2_variant_pointer` (WP 7.5): the glTF add-on's per-slot variant data
//! (`hooks/gltf2_variants.rs`). `Mesh`'s binding (frap-mesh) answers `NeedsHook` for them;
//! with the add-on preference off they are Blender's `AttributeError`.

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::gltf2_variants::{
    Defaults, IdPropOp, Primitives, mesh_root, mesh_variants_mut, require_ui, string,
};
use crate::{Host, HostRead};

macro_rules! decls {
    ($(($srna:literal, $prop:literal, $coll:literal)),* $(,)?) => {$(
        const _: () = {
            #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
            #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = frap_rna::linkme)]
            static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                srna: $srna,
                prop: $prop,
                coll: $coll,
            };
        };
    )*};
}

decls! {
    ("Mesh", "gltf2_variant_mesh_data", true),
    ("Mesh", "gltf2_variant_default_materials", true),
    ("Mesh", "gltf2_variant_pointer", false),
}

fn mesh<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<&'h frap_mesh::Mesh, RnaError> {
    let me = mesh_root(this)?;
    h.main()
        .meshes
        .get(me)
        .ok_or_else(|| crate::id::paths::internal("the mesh was removed"))
}

#[rna_hook(srna = "Mesh", prop = "gltf2_variant_mesh_data", krate = crate)]
mod mesh_data {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        require_ui(h, this, prop.identifier)?;
        let m = mesh(h, this)?;
        Ok(Field::Coll(CollRef::Owned(Box::new(Primitives(
            &m.gltf2_variants.mesh_data,
        )))))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        require_ui(&*h, this, prop.identifier)?;
        let op = IdPropOp::decode(&v)?;
        let me = mesh_root(this)?;
        op.apply(&mut mesh_variants_mut(h, me)?.mesh_data);
        Ok(())
    }
}

#[rna_hook(srna = "Mesh", prop = "gltf2_variant_default_materials", krate = crate)]
mod default_materials {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        require_ui(h, this, prop.identifier)?;
        let m = mesh(h, this)?;
        Ok(Field::Coll(CollRef::Owned(Box::new(Defaults(
            &m.gltf2_variants.default_materials,
        )))))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        require_ui(&*h, this, prop.identifier)?;
        let op = IdPropOp::decode(&v)?;
        let me = mesh_root(this)?;
        op.apply(&mut mesh_variants_mut(h, me)?.default_materials);
        Ok(())
    }
}

#[rna_hook(srna = "Mesh", prop = "gltf2_variant_pointer", krate = crate)]
mod pointer {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        require_ui(h, this, prop.identifier)?;
        let m = mesh(h, this)?;
        Ok(Field::Value(Value::Str(m.gltf2_variants.pointer.clone())))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        require_ui(&*h, this, prop.identifier)?;
        let me = mesh_root(this)?;
        let s = string(&v)?;
        mesh_variants_mut(h, me)?.pointer = s;
        Ok(())
    }
}
