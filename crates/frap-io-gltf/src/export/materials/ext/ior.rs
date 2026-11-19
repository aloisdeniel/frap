// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_ior` export and the final per-material checks (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/ior.py` and `export.py`
//! (glTF-Blender-IO 5.2.41, Apache-2.0). Runs after every other material extension, in the
//! add-on's order:
//! 1. `export_ior` + `__check_ior`: `ior` when not 1.5 and the material has
//!    `KHR_materials_transmission`, `KHR_materials_volume` or `KHR_materials_specular`
//!    [GLTF §4.5] (the volume counts even when step 2 drops it);
//! 2. `__check_volume`: the volume is dropped without `KHR_materials_transmission`;
//! 3. `__check_dispersion`: the dispersion is dropped without the volume.
//!
//! The volume and the dispersion that survive are declared in `extensionsUsed` here.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_dispersion::KhrMaterialsDispersion;
use frap_gltf::ext::khr_materials_ior::KhrMaterialsIor;
use frap_gltf::ext::khr_materials_specular::KhrMaterialsSpecular;
use frap_gltf::ext::khr_materials_transmission::KhrMaterialsTransmission;
use frap_gltf::ext::khr_materials_volume::KhrMaterialsVolume;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;
use frap_material::patterns::ior::GLTF_IOR;

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
    let has = |m: &GMaterial, name: &str| m.extensions.contains(name);
    if let Some(ior) = lm.ext.ior.filter(|i| *i != GLTF_IOR)
        && [
            KhrMaterialsTransmission::NAME,
            KhrMaterialsVolume::NAME,
            KhrMaterialsSpecular::NAME,
        ]
        .iter()
        .any(|n| has(m, n))
    {
        m.extensions.set(Some(KhrMaterialsIor {
            ior: Some(num(ior)),
            ..Default::default()
        }));
        cx.use_extension(KhrMaterialsIor::NAME, false);
    }
    if has(m, KhrMaterialsVolume::NAME) {
        if has(m, KhrMaterialsTransmission::NAME) {
            cx.use_extension(KhrMaterialsVolume::NAME, false);
        } else {
            m.extensions.remove(KhrMaterialsVolume::NAME);
        }
    }
    if has(m, KhrMaterialsDispersion::NAME) {
        if has(m, KhrMaterialsVolume::NAME) {
            cx.use_extension(KhrMaterialsDispersion::NAME, false);
        } else {
            m.extensions.remove(KhrMaterialsDispersion::NAME);
        }
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsIor::NAME,
    order: 900,
    run,
};
