// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_clearcoat` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/clearcoat.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): `clearcoatFactor` with a texture, or when not 0; `clearcoatRoughnessFactor`
//! with a texture, or when it differs from Blender's default coat roughness 0.03 by more than
//! 1e-5 (so 0.03 is not written and reads back as glTF's 0); the three textures (the weight and
//! roughness textures are one glTF texture when both exist, verified on 5.2.4); the normal
//! texture's `scale` when not 1. An empty extension is not written (`__fix_json`).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_clearcoat::KhrMaterialsClearcoat;
use frap_gltf::json::{Material as GMaterial, NormalTextureInfo};
use frap_material::LoweredMaterial;
use frap_material::patterns::clearcoat::BLENDER_COAT_ROUGHNESS;

use crate::export::ExportError;
use crate::export::materials::num;
use crate::material_ext::{ExtExportCx, MaterialExtExporter};

fn run(
    cx: &mut ExtExportCx<'_, '_>,
    lm: &LoweredMaterial,
    m: &mut GMaterial,
) -> Result<(), ExportError> {
    // `__export_unlit` returns before `__gather_extensions`.
    if lm.unlit {
        return Ok(());
    }
    let Some(c) = &lm.ext.clearcoat else {
        return Ok(());
    };
    let mut e = KhrMaterialsClearcoat::default();
    if c.factor.texture.is_some() || c.factor.factor != 0.0 {
        e.clearcoat_factor = Some(num(c.factor.factor));
    }
    // With both textures the add-on packs them into one image (R: weight, G: roughness),
    // gathered for the socket pair, so both members name the same glTF texture.
    let both = c.factor.texture.is_some() && c.roughness.texture.is_some();
    if let Some(t) = &c.factor.texture {
        e.clearcoat_texture = cx.texture_info(t, "clearcoatTexture")?;
    }
    let r = c.roughness.factor;
    if c.roughness.texture.is_some()
        || (f64::from(r) - f64::from(BLENDER_COAT_ROUGHNESS)).abs() > 1e-5
    {
        e.clearcoat_roughness_factor = Some(num(r));
    }
    if let Some(t) = &c.roughness.texture {
        let slot = if both {
            "clearcoatTexture"
        } else {
            "clearcoatRoughnessTexture"
        };
        e.clearcoat_roughness_texture = cx.texture_info(t, slot)?;
    }
    if let Some(n) = &c.normal
        && let Some(info) = cx.texture_info(&n.texture, "clearcoatNormalTexture")?
    {
        e.clearcoat_normal_texture = Some(NormalTextureInfo {
            index: info.index,
            scale: (n.scale != 1.0).then(|| num(n.scale)),
            tex_coord: info.tex_coord,
            extensions: info.extensions,
            extras: None,
        });
    }
    if e != KhrMaterialsClearcoat::default() {
        m.extensions.set(Some(e));
        cx.use_extension(KhrMaterialsClearcoat::NAME, false);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsClearcoat::NAME,
    order: 10,
    run,
};
