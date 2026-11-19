// SPDX-License-Identifier: GPL-3.0-or-later
//! Metallic and roughness: Principled BSDF `Metallic` / `Roughness` → `metallicFactor`,
//! `roughnessFactor` and `metallicRoughnessTexture` (ARCHITECTURE §16.2). The exporter packs
//! the textures (metallic → B, roughness → G, with the occlusion of `occlusion.rs` → R when
//! the images have the same size: the ORM image).
//!
//! Lowering ports `__gather_metallic_factor` / `__gather_roughness_factor` /
//! `__gather_metallic_roughness_texture` (`io_scene_gltf2/blender/exp/material/
//! pbr_metallic_roughness.py`) and the channel rules of `image.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors): the factor is `get_factor` of the socket (a
//! constant, or the single constant input of a Math MULTIPLY; 1 otherwise); the texture is
//! the first Image Texture found behind the socket, read through the Separate Color output
//! on the way, or, plugged in directly, with the glTF convention (metallic = B,
//! roughness = G).
//!
//! Raising ports `metallic_roughness()` (`imp/pbrMetallicRoughness.py`): constants, or one
//! Image Texture labelled "METALLIC ROUGHNESS" → Separate Color → optional "Metallic Factor"
//! / "Roughness Factor" Math MULTIPLY nodes. Textures that differ in anything but the channel
//! get one chain each.

use frap_nodes::{SocketAddr, SocketValue};

use super::base_color::describe;
use crate::graph::{Konst, SrcChannel, unrecognised};
use crate::{Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, TexRef};

/// The texture of a scalar socket read through the channel `image.py` picks (`None` without
/// a texture; RGB → BW only happens on Alpha sockets).
pub(crate) fn scalar_texture(
    cx: &LowerCx,
    socket: Option<crate::graph::In>,
    diags: &mut Vec<frap_rna::Report>,
) -> Option<TexRef> {
    let s = socket?;
    let hit = cx.texture_from(s)?;
    let ch = match cx.source_channel(s, &hit) {
        SrcChannel::One(c) => c,
        SrcChannel::Rgb2Bw => ChannelSel::R,
    };
    cx.tex_ref(hit.node, ch, diags)
}

fn lower_channel(
    cx: &LowerCx,
    out: &mut LoweredMaterial,
    socket: &str,
    gltf_factor: &str,
) -> Channel<f32> {
    let factor_socket = cx
        .get_socket(socket)
        .or_else(|| cx.gltf_output_input(gltf_factor));
    let factor = match factor_socket.and_then(|s| cx.get_factor(s)) {
        Some(Konst::Value(v)) => v,
        _ => 1.0,
    };
    let s = cx.get_socket(socket);
    let texture = scalar_texture(cx, s, &mut out.diagnostics);
    if texture.is_none()
        && let Some(s) = s
        && cx.peek_back(s).is_some()
        && factor_socket.and_then(|f| cx.get_factor(f)).is_none()
    {
        unrecognised(
            &mut out.diagnostics,
            format!(
                "{socket}: unsupported node setup at {}; exported without a texture",
                describe(cx.src_in(s))
            ),
        );
    }
    Channel { factor, texture }
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit || cx.principled().is_none() {
        return;
    }
    out.metallic = lower_channel(cx, out, "Metallic", "MetallicFactor");
    out.roughness = lower_channel(cx, out, "Roughness", "RoughnessFactor");
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
fn chain(
    cx: &mut RaiseCx,
    location: [f32; 2],
    ends: &[(SocketAddr, &Channel<f32>, &str, f32)],
    tex: &TexRef,
) {
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
                None => {
                    cx.set_input_at(bsdf, metallic.index, SocketValue::Float(lm.metallic.factor))
                }
            }
            match rt {
                Some(t) => {
                    let below = [loc[0], loc[1] - 460.0 * f32::from(u8::from(mt.is_some()))];
                    chain(
                        cx,
                        below,
                        &[(roughness, &lm.roughness, "Roughness", 200.0)],
                        t,
                    )
                }
                None => cx.set_input_at(
                    bsdf,
                    roughness.index,
                    SocketValue::Float(lm.roughness.factor),
                ),
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
