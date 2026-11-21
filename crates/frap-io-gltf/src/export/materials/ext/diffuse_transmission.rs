// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_diffuse_transmission` export (WP 7.4), behind `frap_diffuse_transmission`
//! (io_scene_gltf2 5.2.41 does not write the extension): from the lowered material's thin-walled
//! subsurface (`frap_material::patterns::diffuse_transmission`), `diffuseTransmissionFactor`
//! when not 0 and its texture (A), `diffuseTransmissionColorFactor` when not white and its
//! texture (RGB). Not for unlit materials; an empty extension is not written. Optional
//! (`extensionsUsed` only): a viewer without it shows the opaque base material.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_diffuse_transmission::KhrMaterialsDiffuseTransmission;
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
    if !cx.g.settings.props.frap_diffuse_transmission.value || lm.unlit {
        return Ok(());
    }
    let Some(d) = &lm.ext.diffuse_transmission else {
        return Ok(());
    };
    let mut e = KhrMaterialsDiffuseTransmission::default();
    if d.factor.factor != 0.0 {
        e.diffuse_transmission_factor = Some(num(d.factor.factor));
    }
    if let Some(t) = &d.factor.texture {
        e.diffuse_transmission_texture = cx.texture_info(t, "diffuseTransmissionTexture")?;
    }
    if d.color.factor != [1.0; 3] {
        e.diffuse_transmission_color_factor = Some(d.color.factor.map(num));
    }
    if let Some(t) = &d.color.texture {
        e.diffuse_transmission_color_texture =
            cx.texture_info(t, "diffuseTransmissionColorTexture")?;
    }
    if e != KhrMaterialsDiffuseTransmission::default() {
        m.extensions.set(Some(e));
        cx.use_extension(
            KhrMaterialsDiffuseTransmission::NAME,
            KhrMaterialsDiffuseTransmission::REQUIRED_BY_DEFAULT,
        );
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsDiffuseTransmission::NAME,
    order: 85,
    run,
};
