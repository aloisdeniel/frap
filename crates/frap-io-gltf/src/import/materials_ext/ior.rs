// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_ior` import (WP 3.6), the glTF-reading half of the end of
//! `pbr_metallic_roughness()` (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0): the IOR (default 1.5), raised into the Principled
//! BSDF's `IOR`.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_ior::KhrMaterialsIor;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    if let Some(e) = ext_of::<KhrMaterialsIor>(host, m) {
        lm.ext.ior = Some(e.ior_or_default() as f32);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsIor::NAME,
    order: 90,
    run,
};
