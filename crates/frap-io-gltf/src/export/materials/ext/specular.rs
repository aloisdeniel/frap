// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_specular` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/specular.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): `specularFactor` when below 1 (the lowered factor is already capped, the excess
//! moved into the colour), `specularColorFactor` when not white, and both textures. An empty
//! extension is not written.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_specular::KhrMaterialsSpecular;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;

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
    let Some(s) = &lm.ext.specular else {
        return Ok(());
    };
    let mut e = KhrMaterialsSpecular::default();
    if s.factor.factor < 1.0 {
        e.specular_factor = Some(num(s.factor.factor));
    }
    if let Some(t) = &s.factor.texture {
        e.specular_texture = cx.texture_info(t, "specularTexture")?;
    }
    // A linked tint's found factor is written even when white (WP 8.12).
    if s.color.factor != [1.0; 3] || lm.ext.specular_color_set {
        e.specular_color_factor = Some(s.color.factor.map(num));
    }
    if let Some(t) = &s.color.texture {
        e.specular_color_texture = cx.texture_info(t, "specularColorTexture")?;
    }
    if e != KhrMaterialsSpecular::default() {
        m.extensions.set(Some(e));
        cx.use_extension(KhrMaterialsSpecular::NAME, false);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsSpecular::NAME,
    order: 40,
    run,
};
