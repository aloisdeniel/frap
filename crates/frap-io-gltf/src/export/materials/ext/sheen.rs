// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_sheen` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/sheen.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): `sheenColorFactor` when not black, `sheenRoughnessFactor` when not 0, and both
//! textures. An empty extension is not written.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_sheen::KhrMaterialsSheen;
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
    let Some(s) = &lm.ext.sheen else {
        return Ok(());
    };
    let mut e = KhrMaterialsSheen::default();
    if s.color.factor != [0.0; 3] {
        e.sheen_color_factor = Some(s.color.factor.map(num));
    }
    if let Some(t) = &s.color.texture {
        e.sheen_color_texture = cx.texture_info(t, "sheenColorTexture")?;
    }
    if s.roughness.factor != 0.0 {
        e.sheen_roughness_factor = Some(num(s.roughness.factor));
    }
    if let Some(t) = &s.roughness.texture {
        e.sheen_roughness_texture = cx.texture_info(t, "sheenRoughnessTexture")?;
    }
    if e != KhrMaterialsSheen::default() {
        m.extensions.set(Some(e));
        cx.use_extension(KhrMaterialsSheen::NAME, false);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsSheen::NAME,
    order: 50,
    run,
};
