// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_clearcoat` import (WP 3.6), the glTF-reading half of `clearcoat()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): factor (default 0) and texture (R, Non-Color), roughness factor (default
//! Blender's 0.03 without a roughness texture, else 0) and texture (G, Non-Color), normal
//! texture (Non-Color) with its scale.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_clearcoat::KhrMaterialsClearcoat;
use frap_gltf::json::Material as GMaterial;
use frap_material::patterns::clearcoat::BLENDER_COAT_ROUGHNESS;
use frap_material::{Channel, ChannelSel, Clearcoat, LoweredMaterial};

use crate::import::ImportError;
use crate::material_ext::{
    ExtImportHost, MaterialExtImporter, ext_of, import_normal, import_texture,
};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsClearcoat>(host, m) else {
        return Ok(());
    };
    let roughness_default = if e.clearcoat_roughness_texture.is_none() {
        BLENDER_COAT_ROUGHNESS
    } else {
        0.0
    };
    lm.ext.clearcoat = Some(Clearcoat {
        factor: Channel {
            factor: e.clearcoat_factor_or_default() as f32,
            texture: import_texture(host, e.clearcoat_texture.as_ref(), ChannelSel::R, true)?,
        },
        roughness: Channel {
            factor: e
                .clearcoat_roughness_factor
                .map_or(roughness_default, |f| f as f32),
            texture: import_texture(
                host,
                e.clearcoat_roughness_texture.as_ref(),
                ChannelSel::G,
                true,
            )?,
        },
        normal: import_normal(host, e.clearcoat_normal_texture.as_ref())?,
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsClearcoat::NAME,
    order: 10,
    run,
};
