// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_specular` import (WP 3.6), the glTF-reading half of `specular()`
//! (`io_scene_gltf2/blender/imp/pbrMetallicRoughness.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): factor (default 1) and texture (A, sRGB), colour factor (default white) and
//! texture (sRGB). The raise halves the factor into `Specular IOR Level`.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_specular::KhrMaterialsSpecular;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Channel, ChannelSel, LoweredMaterial, Specular};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsSpecular>(host, m) else {
        return Ok(());
    };
    lm.ext.specular = Some(Specular {
        factor: Channel {
            factor: e.specular_factor_or_default() as f32,
            texture: import_texture(host, e.specular_texture.as_ref(), ChannelSel::A, false)?,
        },
        color: Channel {
            factor: e.specular_color_factor_or_default().map(|c| c as f32),
            texture: import_texture(
                host,
                e.specular_color_texture.as_ref(),
                ChannelSel::Rgb,
                false,
            )?,
        },
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsSpecular::NAME,
    order: 40,
    run,
};
