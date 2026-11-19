// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_anisotropy` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/anisotropy.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): `anisotropyStrength` and `anisotropyRotation` when not 0, and
//! `anisotropyTexture`. An empty extension is not written.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_anisotropy::KhrMaterialsAnisotropy;
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
    let Some(a) = &lm.ext.anisotropy else {
        return Ok(());
    };
    let mut e = KhrMaterialsAnisotropy::default();
    if a.strength != 0.0 {
        e.anisotropy_strength = Some(num(a.strength));
    }
    if a.rotation != 0.0 {
        e.anisotropy_rotation = Some(num(a.rotation));
    }
    if let Some(t) = &a.texture {
        e.anisotropy_texture = cx.texture_info(t, "anisotropyTexture")?;
    }
    if e != KhrMaterialsAnisotropy::default() {
        m.extensions.set(Some(e));
        cx.use_extension(KhrMaterialsAnisotropy::NAME, false);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsAnisotropy::NAME,
    order: 60,
    run,
};
