// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_variants` import (WP 7.5): the add-on's `BlenderGlTF.manage_material_variants`
//! (the scene's variant list, the preference `KHR_materials_variants_ui` turned on) and
//! `blender/imp/mesh.py`'s per-slot mappings (`Mesh.gltf2_variant_mesh_data`,
//! `gltf2_variant_default_materials`, written by `mesh.rs::assign_materials` through
//! [`slot_mappings`]).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/blender_gltf.py`
//! (`manage_material_variants`), `blender/imp/mesh.py` (`do_primitives`, "Manage variants")
//! and `blender/imp/material.py` (`gltf.variant_mapping`), Apache-2.0, The glTF-Blender-IO
//! authors:
//!
//! - a file whose root has the extension replaces the variant list of `bpy.data.scenes[0]`
//!   (not the context scene): `variant_idx` is the index, then the name goes through the
//!   add-on's name update (unique among the earlier variants, `Name.001`);
//! - a primitive with `mappings` gets its own material slot (slots are never merged for it),
//!   a default-material entry for that slot (the primitive's material, or none) and one
//!   variant-primitive entry per mapping: the mapping's material (created like the
//!   primitive's, with the primitive's `COLOR_0`; none without `material`) and one pointer per
//!   variant index, in file order.

use frap_gltf::ext::khr_materials_variants::{KhrMaterialsVariants, KhrMaterialsVariantsPrimitive};
use frap_mesh::gltf_variants::{
    GltfVariantDefaultMaterial, GltfVariantPointer, GltfVariantPrimitive, MeshGltfVariants,
};
use frap_rna::{IdRef, MaterialId};
use frap_scene::id::GltfVariant;
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::{Gltf, ImportError, materials};

/// Whether variants are imported (gates the M1 WARNING of `Gltf::warn_unsupported`).
pub(crate) const IMPLEMENTED: bool = true;

const NAME: &str = "KHR_materials_variants";

// Ported from io_scene_gltf2 5.2.41 `blender/com/gltf2_blender_utils.py` (`find_unused_name`).
/// A name not in `used`, at most 63 UTF-8 bytes: `desired`, `desired.001`, …
pub(crate) fn find_unused_name(used: &[SmolStr], desired: &str) -> SmolStr {
    let mut stem: Vec<char> = desired.chars().take(63).collect();
    let mut suffix = String::new();
    let mut cntr = 1u64;
    loop {
        let name: String = stem.iter().collect::<String>() + &suffix;
        if name.len() > 63 {
            stem.pop();
            continue;
        }
        if !used.iter().any(|u| u == name.as_str()) {
            return SmolStr::new(name);
        }
        suffix = format!(".{cntr:03}");
        cntr += 1;
    }
}

/// `manage_material_variants`: creates the variant list of `bpy.data.scenes[0]`; returns
/// whether the file has variants (the operator then turns `KHR_materials_variants_ui` on).
pub(super) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<bool, ImportError> {
    if !cx.doc.extensions.contains(NAME) {
        return Ok(false);
    }
    let root = cx
        .doc
        .extensions
        .get::<KhrMaterialsVariants>()
        .map_err(|e| ImportError::Data(e.to_string()))?
        .unwrap_or_default();
    let Some(first) = main.scenes.iter().next().map(|(r, _)| r) else {
        return Ok(true);
    };
    let Some(scene) = main.scenes.get_mut(first, log) else {
        return Ok(true);
    };
    let list = &mut scene.gltf2_variants.variants;
    list.clear();
    for (idx, v) in root.variants.iter().enumerate() {
        // `var.variant_idx = idx_variant; var.name = variant['name']`: the name update skips
        // the variant at position `variant_idx` (this one), so it is unique among the earlier.
        let used: Vec<SmolStr> = list.iter().map(|g| g.name.clone()).collect();
        list.push(GltfVariant {
            name: find_unused_name(&used, &v.name),
            variant_idx: idx as i32,
        });
    }
    Ok(true)
}

/// The primitive's mappings (`'KHR_materials_variants' in prim.extensions and 'mappings' in
/// …`), or `None`.
pub(crate) fn mappings(
    prim: &frap_gltf::json::Primitive,
) -> Result<Option<KhrMaterialsVariantsPrimitive>, ImportError> {
    let Some(v) = prim.extensions.0.get(NAME) else {
        return Ok(None);
    };
    if v.get("mappings").is_none() {
        return Ok(None);
    }
    prim.extensions
        .get::<KhrMaterialsVariantsPrimitive>()
        .map_err(|e| ImportError::Data(e.to_string()))
}

/// Whether a primitive has the extension at all (`has_materials` counts it).
pub(crate) fn has_extension(prim: &frap_gltf::json::Primitive) -> bool {
    prim.extensions.contains(NAME)
}

/// "Manage variants" for one primitive in material slot `slot` (its material `material`):
/// appends the default material and the variant primitives to `out`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn slot_mappings(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    ext: &KhrMaterialsVariantsPrimitive,
    slot: usize,
    material: Option<IdRef<MaterialId>>,
    vertex_color: bool,
    out: &mut MeshGltfVariants,
) -> Result<(), ImportError> {
    out.default_materials.push(GltfVariantDefaultMaterial {
        material_slot_index: slot as i32,
        default_material: material,
        ..GltfVariantDefaultMaterial::default()
    });
    for m in &ext.mappings {
        let material = match m.material {
            None => None,
            Some(i) => Some(materials::create(
                cx,
                main,
                log,
                Some(i as usize),
                vertex_color,
            )?),
        };
        out.mesh_data.push(GltfVariantPrimitive {
            material_slot_index: slot as i32,
            material,
            variants: m
                .variants
                .iter()
                .map(|&v| GltfVariantPointer {
                    variant_idx: v as i32,
                    ..GltfVariantPointer::default()
                })
                .collect(),
            ..GltfVariantPrimitive::default()
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_made_unique_like_the_addon() {
        let used: Vec<SmolStr> = vec!["Red".into()];
        assert_eq!(find_unused_name(&used, "Red"), "Red.001");
        assert_eq!(find_unused_name(&used, "Blue"), "Blue");
        assert_eq!(find_unused_name(&[], &"z".repeat(64)).len(), 63);
    }
}
