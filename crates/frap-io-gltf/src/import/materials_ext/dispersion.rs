// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_dispersion` import (WP 3.6), the glTF-reading half of `dispersion()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): the dispersion (default 0), raised into the settings node.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_dispersion::KhrMaterialsDispersion;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    if let Some(e) = ext_of::<KhrMaterialsDispersion>(host, m) {
        lm.ext.dispersion = Some(e.dispersion_or_default() as f32);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsDispersion::NAME,
    order: 80,
    run,
};
