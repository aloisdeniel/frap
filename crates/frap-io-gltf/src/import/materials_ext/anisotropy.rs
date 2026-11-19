// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_anisotropy` import (WP 3.6), the glTF-reading half of `anisotropy()`
//! (`io_scene_gltf2/blender/imp/KHR_materials_anisotropy.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): strength and rotation (default 0) and the texture (Non-Color).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_anisotropy::KhrMaterialsAnisotropy;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Anisotropy, ChannelSel, LoweredMaterial};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsAnisotropy>(host, m) else {
        return Ok(());
    };
    lm.ext.anisotropy = Some(Anisotropy {
        strength: e.anisotropy_strength_or_default() as f32,
        rotation: e.anisotropy_rotation_or_default() as f32,
        texture: import_texture(host, e.anisotropy_texture.as_ref(), ChannelSel::Rgb, true)?,
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsAnisotropy::NAME,
    order: 60,
    run,
};
