// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_dispersion` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/dispersion.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0): only for a material whose volume was gathered, `dispersion` when not 0. Put on
//! the material here, declared by the final checks of `ior.rs` (`__check_dispersion` drops it
//! when the volume itself was dropped).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_dispersion::KhrMaterialsDispersion;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;

use crate::export::ExportError;
use crate::export::materials::num;
use crate::material_ext::{ExtExportCx, MaterialExtExporter};

fn run(
    _cx: &mut ExtExportCx<'_, '_>,
    lm: &LoweredMaterial,
    m: &mut GMaterial,
) -> Result<(), ExportError> {
    // `__export_unlit` returns before `__gather_extensions`.
    if lm.unlit {
        return Ok(());
    }
    if lm.ext.volume.is_none() {
        return Ok(());
    }
    if let Some(d) = lm.ext.dispersion.filter(|d| *d != 0.0) {
        m.extensions.set(Some(KhrMaterialsDispersion {
            dispersion: Some(num(d)),
            ..Default::default()
        }));
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsDispersion::NAME,
    order: 80,
    run,
};
