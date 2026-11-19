// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_transmission` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/transmission.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): `transmissionFactor` with a texture, or when not 0, and `transmissionTexture`.
//! An empty extension is not written.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_transmission::KhrMaterialsTransmission;
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
    let Some(t) = &lm.ext.transmission else {
        return Ok(());
    };
    let mut e = KhrMaterialsTransmission::default();
    if t.factor.texture.is_some() || t.factor.factor != 0.0 {
        e.transmission_factor = Some(num(t.factor.factor));
    }
    if let Some(tex) = &t.factor.texture {
        e.transmission_texture = cx.texture_info(tex, "transmissionTexture")?;
    }
    if e != KhrMaterialsTransmission::default() {
        m.extensions.set(Some(e));
        cx.use_extension(KhrMaterialsTransmission::NAME, false);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsTransmission::NAME,
    order: 20,
    run,
};
