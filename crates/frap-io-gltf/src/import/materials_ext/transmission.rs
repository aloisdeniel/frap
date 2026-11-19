// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_transmission` import (WP 3.6), the glTF-reading half of `transmission()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): factor (default 0) and texture (R, Non-Color).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_transmission::KhrMaterialsTransmission;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Channel, ChannelSel, LoweredMaterial, Transmission};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsTransmission>(host, m) else {
        return Ok(());
    };
    lm.ext.transmission = Some(Transmission {
        factor: Channel {
            factor: e.transmission_factor_or_default() as f32,
            texture: import_texture(host, e.transmission_texture.as_ref(), ChannelSel::R, true)?,
        },
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsTransmission::NAME,
    order: 20,
    run,
};
