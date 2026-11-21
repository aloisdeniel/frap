// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.gltf2_KHR_materials_variants_variants` and `Scene.gltf2_active_variant` (WP 7.5):
//! the glTF add-on's variant list (`hooks/gltf2_variants.rs`). `Scene`'s binding answers
//! `NoAttribute` for them, so these hooks serve them; with the add-on preference off they are
//! Blender's `AttributeError`. The list's `add()` / `remove()` / `clear()` / `move()` reach the
//! set hook of the collection ([`IdPropOp`]).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::gltf2_variants::{
    IdPropOp, SceneVariants, int, require_ui, scene_root, scene_variants_mut,
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
    ("Scene", "gltf2_KHR_materials_variants_variants", true),
    ("Scene", "gltf2_active_variant", false),
}

#[rna_hook(srna = "Scene", prop = "gltf2_KHR_materials_variants_variants", krate = crate)]
mod variants {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        require_ui(h, this, prop.identifier)?;
        let sc = scene_root(this)?;
        let s = h
            .main()
            .scenes
            .get(sc)
            .ok_or_else(|| crate::id::paths::internal("the scene was removed"))?;
        Ok(Field::Coll(CollRef::Owned(Box::new(SceneVariants(
            &s.gltf2_variants.variants,
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
        let sc = scene_root(this)?;
        op.apply(&mut scene_variants_mut(h, sc)?.variants);
        Ok(())
    }
}

#[rna_hook(srna = "Scene", prop = "gltf2_active_variant", krate = crate)]
mod active_variant {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        require_ui(h, this, prop.identifier)?;
        let sc = scene_root(this)?;
        let s = h
            .main()
            .scenes
            .get(sc)
            .ok_or_else(|| crate::id::paths::internal("the scene was removed"))?;
        Ok(Field::Value(Value::Int(i64::from(
            s.gltf2_variants.active_variant,
        ))))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        require_ui(&*h, this, prop.identifier)?;
        let sc = scene_root(this)?;
        let i = int(&v)?;
        scene_variants_mut(h, sc)?.active_variant = i;
        Ok(())
    }
}
