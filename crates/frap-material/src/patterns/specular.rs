// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_specular`: Principled BSDF `Specular IOR Level` (× 2) and `Specular Tint` →
//! `specularFactor`/`specularTexture` (A) and `specularColorFactor`/`specularColorTexture`
//! (RGB) (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_specular` (`io_scene_gltf2/blender/exp/material/extensions/
//! specular.py`, glTF-Blender-IO 5.2.41, Apache-2.0). The lowered values are the glTF ones:
//! - `fac` is twice an unlinked `Specular IOR Level`; for a linked one, the constant factor
//!   (`get_factor`), doubled unless it is exactly 1 (the add-on's test), or none;
//! - `specularFactor` = `fac` when below 1, else 1 (glTF caps it; the add-on moves the excess
//!   into the colour);
//! - `specularColorFactor` = the tint (unlinked value, or the constant factor of a linked one,
//!   white when there is none) multiplied by `fac` when `fac` exceeds 1;
//! - textures: the image behind `Specular IOR Level` (packed into A, from A unless a Separate
//!   Color output names the channel) and behind `Specular Tint` (copied whole).
//!
//! The add-on also writes `specularColorFactor: [1, 1, 1]` for a linked tint whose constant
//! factor is exactly white (a list compared with a tuple); frap omits it like any default.
//!
//! Raising ports `specular()` (`imp/pbrMetallicRoughness.py`): `Specular IOR Level` = half the
//! factor through `scalar_factor_and_texture` ("Specular", Alpha), then
//! `color_factor_and_texture` ("Specular Color").

use frap_nodes::SocketValue;

use super::transmission::ext_common::{
    TexKind, factor_f32, factor_rgb, scalar_factor_and_texture, texture_behind,
};
use crate::graph::Src;
use crate::{Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, Specular};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let (Some(si), Some(ti)) = (
        p.input_index("Specular IOR Level"),
        p.input_index("Specular Tint"),
    ) else {
        return;
    };
    // `fac` in f64, as Python computes it.
    let (fac, texture) = match cx.src_at(p, si) {
        Src::Const(SocketValue::Float(f)) => (Some(f64::from(*f) * 2.0), None),
        src => {
            let fac = factor_f32(cx, src).map(|c| if c != 1.0 { f64::from(c) * 2.0 } else { 1.0 });
            let tex = texture_behind(
                cx,
                p,
                si,
                TexKind::Packed(ChannelSel::A),
                &mut out.diagnostics,
            );
            (fac, tex)
        }
    };
    let over = fac.filter(|f| *f > 1.0);
    let factor = Channel {
        factor: fac.filter(|f| *f < 1.0).map_or(1.0, |f| f as f32),
        texture,
    };
    let scale = |c: [f32; 3]| match over {
        Some(f) => c.map(|x| (f64::from(x) * f) as f32),
        None => c,
    };
    let color = match cx.src_at(p, ti) {
        Src::Const(SocketValue::Color(c)) => Channel {
            factor: scale([c[0], c[1], c[2]]),
            texture: None,
        },
        src => Channel {
            factor: match (factor_rgb(cx, src), over) {
                (Some(c), _) => scale(c),
                (None, Some(f)) => [f as f32; 3],
                (None, None) => [1.0; 3],
            },
            texture: texture_behind(
                cx,
                p,
                ti,
                TexKind::Full(ChannelSel::Rgb),
                &mut out.diagnostics,
            ),
        },
    };
    out.ext.specular = Some(Specular { factor, color });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(s), Some(bsdf)) = (&lm.ext.specular, cx.bsdf) else {
        return;
    };
    let socket = cx.input(bsdf, "Specular IOR Level");
    let loc = cx.location("specularTexture");
    scalar_factor_and_texture(
        cx,
        loc,
        "Specular",
        socket,
        (0.5 * f64::from(s.factor.factor)) as f32,
        s.factor.texture.as_ref(),
        4,
    );
    let socket = cx.input(bsdf, "Specular Tint");
    let loc = cx.location("specularColorTexture");
    cx.color_factor_and_texture(
        loc,
        "Specular Color",
        socket,
        s.color.factor,
        s.color.texture.as_ref(),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "specular",
    order: 340,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 50,
    key: "specularTexture",
    takes_space: |lm| {
        lm.ext
            .specular
            .as_ref()
            .is_some_and(|s| s.factor.texture.is_some())
    },
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT_COLOR: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 60,
    key: "specularColorTexture",
    takes_space: |lm| {
        lm.ext
            .specular
            .as_ref()
            .is_some_and(|s| s.color.texture.is_some())
    },
};
