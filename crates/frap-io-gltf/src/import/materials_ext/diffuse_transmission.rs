// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_diffuse_transmission` import (WP 7.4): the inverse of the
//! `frap_diffuse_transmission` export. io_scene_gltf2 5.2.41 ignores the extension (it is not
//! in `extensions_managed`; a file requiring it is refused, by frap too); frap reads it into
//! `LoweredMaterial::ext.diffuse_transmission`, which `frap_material::raise` turns into the
//! Principled BSDF's `Thin Wall` and `Subsurface Weight` (deviations §2). Factor (default 0)
//! and texture (A, linear); colour (default white) and texture (RGB, sRGB). Blender's
//! thin-walled subsurface takes the Base Color, so a transmission colour other than the base
//! colour is not kept (WARNING). An extension with a 0 factor and no texture changes nothing.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_diffuse_transmission::KhrMaterialsDiffuseTransmission;
use frap_gltf::json::Material as GMaterial;
use frap_material::{Channel, ChannelSel, DiffuseTransmission, LoweredMaterial};
use frap_rna::ReportType;

use crate::import::ImportError;
use crate::material_ext::{ExtImportHost, MaterialExtImporter, ext_of, import_texture};

fn run(
    host: &mut dyn ExtImportHost,
    m: &GMaterial,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    let Some(e) = ext_of::<KhrMaterialsDiffuseTransmission>(host, m) else {
        return Ok(());
    };
    let factor = e.diffuse_transmission_factor.unwrap_or(0.0) as f32;
    let factor_tex = import_texture(
        host,
        e.diffuse_transmission_texture.as_ref(),
        ChannelSel::A,
        true,
    )?;
    if factor == 0.0 && factor_tex.is_none() {
        return Ok(());
    }
    let color = e
        .diffuse_transmission_color_factor
        .unwrap_or([1.0; 3])
        .map(|c| c as f32);
    let color_tex = import_texture(
        host,
        e.diffuse_transmission_color_texture.as_ref(),
        ChannelSel::Rgb,
        false,
    )?;
    let base = &lm.base_color;
    let base_rgb = [base.factor[0], base.factor[1], base.factor[2]];
    let same_texture = match (&color_tex, &base.texture) {
        (None, None) => true,
        (Some(a), Some(b)) => a.image == b.image,
        _ => false,
    };
    // A white colour with no texture is the extension's default: nothing is lost.
    let default_color = color == [1.0; 3] && color_tex.is_none();
    if !default_color && (color != base_rgb || !same_texture) {
        host.report(
            ReportType::Warning,
            format!(
                "{}: diffuseTransmissionColor is not imported (Blender's thin-walled subsurface uses the Base Color)",
                KhrMaterialsDiffuseTransmission::NAME
            ),
        );
    }
    lm.ext.diffuse_transmission = Some(DiffuseTransmission {
        factor: Channel {
            factor,
            texture: factor_tex,
        },
        color: Channel {
            factor: color,
            texture: color_tex,
        },
    });
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_IMPORTERS)]
static IMPORTER: MaterialExtImporter = MaterialExtImporter {
    name: KhrMaterialsDiffuseTransmission::NAME,
    order: 85,
    run,
};
