// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_sheen`: Principled BSDF `Sheen Tint` and `Sheen Roughness`, enabled by
//! `Sheen Weight` → `sheenColorFactor`/`sheenColorTexture` (RGB) and `sheenRoughnessFactor`/
//! `sheenRoughnessTexture` (A) (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_sheen` (`io_scene_gltf2/blender/exp/material/extensions/sheen.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0): no extension when `Sheen Weight` is unlinked and 0
//! (its value is otherwise ignored); an unlinked tint or roughness gives its value; a linked
//! one gives its factor (`get_factor`, 1 when there is none) and the image behind it, if any
//! (the tint image is copied whole; the roughness is packed into A, from A unless a Separate
//! Color output names the channel).
//!
//! Raising ports `sheen()` (`imp/pbrMetallicRoughness.py`): `Sheen Weight` = 1,
//! `color_factor_and_texture` ("Sheen Color") and `scalar_factor_and_texture` ("Sheen
//! Roughness", Alpha).

use frap_nodes::SocketValue;

use super::transmission::ext_common::{
    TexKind, factor_f32, factor_rgb, scalar_factor_and_texture, texture_behind,
};
use crate::graph::Src;
use crate::{Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, Sheen};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let (Some(ti), Some(ri), Some(wi)) = (
        p.input_index("Sheen Tint"),
        p.input_index("Sheen Roughness"),
        p.input_index("Sheen Weight"),
    ) else {
        return;
    };
    if let Src::Const(SocketValue::Float(w)) = cx.src_at(p, wi)
        && *w == 0.0
    {
        return;
    }
    let color = match cx.src_at(p, ti) {
        Src::Const(SocketValue::Color(c)) => Channel {
            factor: [c[0], c[1], c[2]],
            texture: None,
        },
        src => Channel {
            factor: factor_rgb(cx, src).unwrap_or([1.0; 3]),
            texture: texture_behind(
                cx,
                p,
                ti,
                TexKind::Full(ChannelSel::Rgb),
                &mut out.diagnostics,
            ),
        },
    };
    let roughness = match cx.src_at(p, ri) {
        Src::Const(SocketValue::Float(f)) => Channel {
            factor: *f,
            texture: None,
        },
        src => Channel {
            factor: factor_f32(cx, src).unwrap_or(1.0),
            texture: texture_behind(
                cx,
                p,
                ri,
                TexKind::Packed(ChannelSel::A),
                &mut out.diagnostics,
            ),
        },
    };
    out.ext.sheen = Some(Sheen { color, roughness });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(s), Some(bsdf)) = (&lm.ext.sheen, cx.bsdf) else {
        return;
    };
    cx.set_input(bsdf, "Sheen Weight", SocketValue::Float(1.0));
    let socket = cx.input(bsdf, "Sheen Tint");
    let loc = cx.location("sheenColorTexture");
    cx.color_factor_and_texture(
        loc,
        "Sheen Color",
        socket,
        s.color.factor,
        s.color.texture.as_ref(),
    );
    let socket = cx.input(bsdf, "Sheen Roughness");
    let loc = cx.location("sheenRoughnessTexture");
    scalar_factor_and_texture(
        cx,
        loc,
        "Sheen Roughness",
        socket,
        s.roughness.factor,
        s.roughness.texture.as_ref(),
        4,
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "sheen",
    order: 360,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT_ROUGHNESS: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 80,
    key: "sheenRoughnessTexture",
    takes_space: |lm| {
        lm.ext
            .sheen
            .as_ref()
            .is_some_and(|s| s.roughness.texture.is_some())
    },
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT_COLOR: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 90,
    key: "sheenColorTexture",
    takes_space: |lm| {
        lm.ext
            .sheen
            .as_ref()
            .is_some_and(|s| s.color.texture.is_some())
    },
};
