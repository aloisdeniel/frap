// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_sheen` import (WP 3.6), the glTF-reading half of `sheen()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): colour factor (default black) and texture (sRGB), roughness factor (default 0)
//! and texture (A, sRGB). The raise sets `Sheen Weight` to 1.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_sheen::KhrMaterialsSheen;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Channel, ChannelSel, LoweredMaterial, Sheen};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsSheen>(host, m) else {
        return Ok(());
    };
    lm.ext.sheen = Some(Sheen {
        color: Channel {
            factor: e.sheen_color_factor_or_default().map(|c| c as f32),
            texture: import_texture(host, e.sheen_color_texture.as_ref(), ChannelSel::Rgb, false)?,
        },
        roughness: Channel {
            factor: e.sheen_roughness_factor_or_default() as f32,
            texture: import_texture(
                host,
                e.sheen_roughness_texture.as_ref(),
                ChannelSel::A,
                false,
            )?,
        },
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsSheen::NAME,
    order: 50,
    run,
};
