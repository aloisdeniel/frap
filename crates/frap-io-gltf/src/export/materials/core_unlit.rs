// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_unlit` materials.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (`__export_unlit`) and unlit.py (`gather_base_color_factor`, `gather_base_color_texture`),
//! glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors: the extension (an empty
//! object, not required), `pbrMetallicRoughness` with the base colour factor (not clamped,
//! omitted when `[1, 1, 1, 1]`), the base colour texture (copied whole: an unlit graph has no
//! socket named Alpha, so the image keeps its format), `metallicFactor` 0 and
//! `roughnessFactor` 0.9 always written; no emission, normal, occlusion or other extension.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_unlit::KhrMaterialsUnlit;
use frap_gltf::json::{Material, PbrMetallicRoughness};
use frap_material::LoweredMaterial;
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{Role, TexInput, TextureSlot, gather_texture_info_from};

/// Writes the unlit members (`extensions` before `pbrMetallicRoughness`, the traversal
/// order).
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    m.extensions.set(Some(KhrMaterialsUnlit::default()));
    g.use_extension(KhrMaterialsUnlit::NAME, false);
    let f = lm.base_color.factor.map(num);
    let mut pbr = PbrMetallicRoughness {
        base_color_factor: (f != [1.0; 4]).then_some(f),
        metallic_factor: Some(0.0),
        roughness_factor: Some(0.9),
        ..PbrMetallicRoughness::default()
    };
    if let Some(t) = &lm.base_color.texture {
        let inputs = [TexInput {
            tex: t,
            role: Role::Full,
        }];
        pbr.base_color_texture =
            gather_texture_info_from(g, t, &inputs, (r, TextureSlot::BaseColor), mesh)?;
    }
    m.pbr_metallic_roughness = Some(pbr);
    Ok(())
}
