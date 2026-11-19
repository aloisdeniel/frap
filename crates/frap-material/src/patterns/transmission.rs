// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_transmission`: Principled BSDF `Transmission Weight` → `transmissionFactor`
//! and `transmissionTexture` (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_transmission` (`io_scene_gltf2/blender/exp/material/extensions/
//! transmission.py`, glTF-Blender-IO 5.2.41, Apache-2.0): an unlinked socket gives its value;
//! a socket with an image behind it gives the texture (the whole image is copied, glTF reads
//! R) and its factor (`get_factor`, 1 without a constant multiplier); anything else gives no
//! member and a WARNING. `Some` whenever there is a Principled BSDF (the add-on always returns
//! the extension; an empty one is dropped when written).
//!
//! Raising ports `transmission()` (`imp/pbrMetallicRoughness.py`): `scalar_factor_and_texture`
//! (R through a Separate Color, "Transmission Factor" Math MULTIPLY, Image Texture labelled
//! "TRANSMISSION"). The add-on also turns on `Material.use_raytrace_refraction` for a factor
//! above 0, an EEVEE setting frap does not model yet (no effect on the graph or the export).
//!
//! The submodule `ext_common` holds the helpers every `KHR_materials_*` pattern shares.

pub(crate) mod ext_common;
#[cfg(test)]
mod ext_tests;

use ext_common::{TexKind, scalar_factor_and_texture, scalar_member};

use crate::{
    Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, Transmission,
};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let Some(i) = p.input_index("Transmission Weight") else {
        return;
    };
    let s = scalar_member(
        cx,
        p,
        i,
        TexKind::Full(ChannelSel::R),
        "Transmission Weight",
        &mut out.diagnostics,
    );
    out.ext.transmission = Some(Transmission {
        factor: Channel {
            factor: s.factor.unwrap_or(0.0),
            texture: s.texture,
        },
    });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(t), Some(bsdf)) = (&lm.ext.transmission, cx.bsdf) else {
        return;
    };
    let socket = cx.input(bsdf, "Transmission Weight");
    let loc = cx.location("transmission");
    scalar_factor_and_texture(
        cx,
        loc,
        "Transmission",
        socket,
        t.factor.factor,
        t.factor.texture.as_ref(),
        0,
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "transmission",
    order: 310,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 30,
    key: "transmission",
    takes_space: |lm| {
        lm.ext
            .transmission
            .as_ref()
            .is_some_and(|t| t.factor.texture.is_some())
    },
};
