// SPDX-License-Identifier: GPL-3.0-or-later
//! Writes to the glTF add-on's variant PropertyGroups (WP 7.5; `hooks/gltf2_variants.rs`):
//! `gltf2_KHR_materials_variants_variant.{name, variant_idx}` (a scene's variant, or the
//! `variant` of a pointer; `name` runs the add-on's `on_variant_name_update`),
//! `gltf2_KHR_materials_variants_primitive.{name, material_slot_index, material,
//! active_variant_idx}` and its `variants` collection's methods,
//! `gltf2_KHR_materials_variant_pointer.name`,
//! `gltf2_KHR_materials_variants_default_material.{name, material_slot_index,
//! default_material}`. The read proxies serve reads; values arrive coerced (`INT` range,
//! `Material` pointer type), so these hooks only store them.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::gltf2_variants::{
    IdPropOp, Place, int, material, mesh_variants_mut, place_of, scene_variants_mut,
    set_variant_name, string,
};
use crate::Host;
use crate::id::paths::internal;

fn gone() -> RnaError {
    internal("the glTF variant item was removed")
}

/// The set hook of every writable property of the variant groups.
fn set_field(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let place = place_of(h.main(), this)?;
    match (place, prop.identifier) {
        (Place::SceneVariant(..) | Place::NestedVariant(..), "name") => {
            set_variant_name(h, place, string(&v)?)
        }
        (Place::SceneVariant(sc, i), "variant_idx") => {
            let n = int(&v)?;
            let s = scene_variants_mut(h, sc)?;
            s.variants.get_mut(i).ok_or_else(gone)?.variant_idx = n;
            Ok(())
        }
        (Place::NestedVariant(me, i, j), "variant_idx") => {
            let n = int(&v)?;
            let m = mesh_variants_mut(h, me)?;
            m.mesh_data
                .get_mut(i)
                .and_then(|p| p.variants.get_mut(j))
                .ok_or_else(gone)?
                .variant_idx = n;
            Ok(())
        }
        (Place::Primitive(me, i), p) => {
            let m = mesh_variants_mut(h, me)?;
            let prim = m.mesh_data.get_mut(i).ok_or_else(gone)?;
            match p {
                "name" => prim.name = string(&v)?,
                "material_slot_index" => prim.material_slot_index = int(&v)?,
                "material" => prim.material = material(&v)?,
                "active_variant_idx" => prim.active_variant_idx = int(&v)?,
                "variants" => IdPropOp::decode(&v)?.apply(&mut prim.variants),
                _ => return Err(internal("not a writable property")),
            }
            Ok(())
        }
        (Place::Pointer(me, i, j), "name") => {
            let s = string(&v)?;
            let m = mesh_variants_mut(h, me)?;
            m.mesh_data
                .get_mut(i)
                .and_then(|p| p.variants.get_mut(j))
                .ok_or_else(gone)?
                .name = s;
            Ok(())
        }
        (Place::Default(me, i), p) => {
            let m = mesh_variants_mut(h, me)?;
            let d = m.default_materials.get_mut(i).ok_or_else(gone)?;
            match p {
                "name" => d.name = string(&v)?,
                "material_slot_index" => d.material_slot_index = int(&v)?,
                "default_material" => d.default_material = material(&v)?,
                _ => return Err(internal("not a writable property")),
            }
            Ok(())
        }
        _ => Err(internal("not a writable glTF variant property")),
    }
}

macro_rules! set_hooks {
    ($(($m:ident, $srna:literal, $prop:literal)),* $(,)?) => {$(
        #[rna_hook(srna = $srna, prop = $prop, set = set_field, krate = crate)]
        mod $m {}

        const _: () = {
            #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
            #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = frap_rna::linkme)]
            static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                srna: $srna,
                prop: $prop,
                coll: false,
            };
        };
    )*};
}

set_hooks! {
    (variant_name, "gltf2_KHR_materials_variants_variant", "name"),
    (variant_idx, "gltf2_KHR_materials_variants_variant", "variant_idx"),
    (primitive_name, "gltf2_KHR_materials_variants_primitive", "name"),
    (primitive_slot, "gltf2_KHR_materials_variants_primitive", "material_slot_index"),
    (primitive_material, "gltf2_KHR_materials_variants_primitive", "material"),
    (primitive_active, "gltf2_KHR_materials_variants_primitive", "active_variant_idx"),
    (primitive_variants, "gltf2_KHR_materials_variants_primitive", "variants"),
    (pointer_name, "gltf2_KHR_materials_variant_pointer", "name"),
    (default_name, "gltf2_KHR_materials_variants_default_material", "name"),
    (default_slot, "gltf2_KHR_materials_variants_default_material", "material_slot_index"),
    (default_material, "gltf2_KHR_materials_variants_default_material", "default_material"),
}
