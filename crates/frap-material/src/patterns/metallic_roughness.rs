// SPDX-License-Identifier: GPL-3.0-or-later
//! Metallic and roughness: Principled BSDF `Metallic` / `Roughness` → `metallicFactor`,
//! `roughnessFactor` and `metallicRoughnessTexture` (ARCHITECTURE §16.2).
//!
//! Lowering ports `__gather_metallic_factor` / `__gather_roughness_factor` and the channel
//! rules of `image.py` (`io_scene_gltf2/blender/exp/material/`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): a constant (unlinked socket or Value node) is the factor; a Math MULTIPLY by
//! a constant gives the factor of the texture behind it; the texture is read through a
//! Separate Color output (Red/Green/Blue) or, plugged in directly, with the glTF convention
//! (metallic = B, roughness = G). Other setups keep factor 1 and get a WARNING.
//!
//! Raising ports `metallic_roughness()` (`imp/pbrMetallicRoughness.py`): constants, or one
//! Image Texture labelled "METALLIC ROUGHNESS" → Separate Color → optional "Metallic Factor"
//! / "Roughness Factor" Math MULTIPLY nodes. Textures that differ in anything but the channel
//! get one chain each.

use frap_nodes::{SocketAddr, SocketValue};

use super::base_color::describe;
use crate::graph::{Src, unrecognised};
use crate::{Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, TexRef};

fn lower_channel(cx: &LowerCx, out: &mut LoweredMaterial, socket: &str, direct: ChannelSel) -> Channel<f32> {
    let Some(p) = cx.principled() else {
        return Channel {
            factor: 1.0,
            texture: None,
        };
    };
    let src = cx.src(p, socket);
    if let Some(c) = cx.const_f32(src) {
        return Channel {
            factor: c,
            texture: None,
        };
    }
    let (factor, rest) = cx.value_times_const(src).unwrap_or((1.0, src));
    let texture = match cx.image_behind(rest) {
        Some((tex, ch @ (ChannelSel::R | ChannelSel::G | ChannelSel::B | ChannelSel::A))) => {
            cx.tex_ref(tex, ch, &mut out.diagnostics)
        }
        Some((tex, _)) => cx.tex_ref(tex, direct, &mut out.diagnostics),
        None => {
            if let Src::Linked(..) = rest {
                unrecognised(
                    &mut out.diagnostics,
                    format!("{socket}: unsupported node setup at {}; exported without a texture", describe(rest)),
                );
            }
            None
        }
    };
    Channel { factor, texture }
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if cx.principled().is_none() {
        return;
    }
    out.metallic = lower_channel(cx, out, "Metallic", ChannelSel::B);
    out.roughness = lower_channel(cx, out, "Roughness", ChannelSel::G);
}

/// Same image, UVs, transform, sampler and colour space: one Image Texture serves both.
fn same_texture(a: &TexRef, b: &TexRef) -> bool {
    a.image == b.image
        && a.uv == b.uv
        && a.transform == b.transform
        && a.sampler == b.sampler
        && a.colorspace_is_data == b.colorspace_is_data
}

fn channel_output(ch: ChannelSel) -> Option<&'static str> {
    match ch {
        ChannelSel::R => Some("Red"),
        ChannelSel::G => Some("Green"),
        ChannelSel::B => Some("Blue"),
        _ => None,
    }
}

/// One importer chain: factor Math nodes, a Separate Color for R/G/B channels, the texture.
fn chain(cx: &mut RaiseCx, location: [f32; 2], ends: &[(SocketAddr, &Channel<f32>, &str, f32)], tex: &TexRef) {
    let [mut x, y] = location;
    let mut sockets: Vec<(SocketAddr, ChannelSel)> = Vec::new();
    let mut any_factor = false;
    for (socket, ch, label, dy) in ends {
        let mut socket = *socket;
        if ch.factor != 1.0 {
            let m = cx.add_node("ShaderNodeMath", [x - 140.0, y - dy]);
            cx.set_label(m, &format!("{label} Factor"));
            cx.set_enum(m, "operation", "MULTIPLY");
            cx.link(socket, SocketAddr::output(m, 0));
            socket = SocketAddr::input(m, 0);
            cx.set_input_at(m, 1, SocketValue::Float(ch.factor));
            any_factor = true;
        }
        let channel = ch.texture.as_ref().map_or(ChannelSel::B, |t| t.channel);
        sockets.push((socket, channel));
    }
    if any_factor {
        x -= 200.0;
    }
    let mut color_to = None;
    let mut alpha_to = None;
    if sockets.iter().any(|(_, c)| channel_output(*c).is_some()) {
        let sep = cx.add_node("ShaderNodeSeparateColor", [x - 150.0, y - 75.0]);
        for (socket, c) in &sockets {
            if let Some(name) = channel_output(*c) {
                let o = cx.output(sep, name);
                cx.link(*socket, o);
            }
        }
        color_to = Some(cx.input(sep, "Color"));
        x -= 200.0;
    }
    for (socket, c) in &sockets {
        match c {
            ChannelSel::A => alpha_to = Some(*socket),
            ChannelSel::Rgb | ChannelSel::Rgba => color_to = color_to.or(Some(*socket)),
            _ => {}
        }
    }
    cx.texture(tex, [x, y], "METALLIC ROUGHNESS", color_to, alpha_to);
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(bsdf) = cx.bsdf else { return };
    let metallic = cx.input(bsdf, "Metallic");
    let roughness = cx.input(bsdf, "Roughness");
    let loc = cx.location("metallic_roughness");
    match (&lm.metallic.texture, &lm.roughness.texture) {
        (Some(mt), Some(rt)) if same_texture(mt, rt) => chain(
            cx,
            loc,
            &[
                (metallic, &lm.metallic, "Metallic", 0.0),
                (roughness, &lm.roughness, "Roughness", 200.0),
            ],
            mt,
        ),
        (mt, rt) => {
            match mt {
                Some(t) => chain(cx, loc, &[(metallic, &lm.metallic, "Metallic", 0.0)], t),
                None => cx.set_input_at(bsdf, metallic.index, SocketValue::Float(lm.metallic.factor)),
            }
            match rt {
                Some(t) => {
                    let below = [loc[0], loc[1] - 460.0 * f32::from(u8::from(mt.is_some()))];
                    chain(cx, below, &[(roughness, &lm.roughness, "Roughness", 200.0)], t)
                }
                None => cx.set_input_at(bsdf, roughness.index, SocketValue::Float(lm.roughness.factor)),
            }
        }
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "metallic_roughness",
    order: 40,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 20,
    key: "metallic_roughness",
    takes_space: |lm| lm.metallic.texture.is_some() || lm.roughness.texture.is_some(),
};
