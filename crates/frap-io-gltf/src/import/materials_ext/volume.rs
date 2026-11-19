// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_volume` import (WP 3.6), the glTF-reading half of `volume()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): thickness factor (default 0) and texture (G, Non-Color), attenuation distance
//! (default ∞) and colour (default white). The raise builds nodes only for a non-zero
//! thickness, as the add-on does.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_volume::KhrMaterialsVolume;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Channel, ChannelSel, LoweredMaterial, Volume};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsVolume>(host, m) else {
        return Ok(());
    };
    lm.ext.volume = Some(Volume {
        thickness: Channel {
            factor: e.thickness_factor_or_default() as f32,
            texture: import_texture(host, e.thickness_texture.as_ref(), ChannelSel::G, true)?,
        },
        attenuation_distance: e.attenuation_distance_or_default() as f32,
        attenuation_color: e.attenuation_color_or_default().map(|c| c as f32),
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsVolume::NAME,
    order: 30,
    run,
};
