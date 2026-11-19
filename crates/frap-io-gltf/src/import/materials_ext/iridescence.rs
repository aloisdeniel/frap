// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_iridescence` import (WP 3.6), the glTF-reading half of `iridescence()`
//! (`io_scene_gltf2/blender/imp/KHR_materials_iridescence.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): factor (default 0) and texture (R, Non-Color), IOR (default 1.3), minimum
//! thickness (default 100) and the thickness texture (G, Non-Color).
//!
//! The add-on reads the maximum thickness from the key `iridescenceThickness`, which the glTF
//! extension does not define (it is `iridescenceThicknessMaximum`), so an imported maximum is
//! always 400 unless a file carries that key; frap reads it the same way.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_iridescence::KhrMaterialsIridescence;
use frap_gltf::json::Material as GMaterial;
use frap_material::patterns::iridescence::THICKNESS_MAXIMUM;
use frap_material::{Channel, ChannelSel, Iridescence, LoweredMaterial};

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsIridescence>(host, m) else {
        return Ok(());
    };
    let thickness_max = m
        .extensions
        .get_raw(KhrMaterialsIridescence::NAME)
        .and_then(|v| v.get("iridescenceThickness"))
        .and_then(serde_json::Value::as_f64)
        .map_or(THICKNESS_MAXIMUM, |f| f as f32);
    lm.ext.iridescence = Some(Iridescence {
        factor: Channel {
            factor: e.iridescence_factor_or_default() as f32,
            texture: import_texture(host, e.iridescence_texture.as_ref(), ChannelSel::R, true)?,
        },
        ior: e.iridescence_ior_or_default() as f32,
        thickness_min: e.iridescence_thickness_minimum_or_default() as f32,
        thickness_max: Channel {
            factor: thickness_max,
            texture: import_texture(
                host,
                e.iridescence_thickness_texture.as_ref(),
                ChannelSel::G,
                true,
            )?,
        },
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsIridescence::NAME,
    order: 70,
    run,
};
