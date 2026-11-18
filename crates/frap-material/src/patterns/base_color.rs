// SPDX-License-Identifier: GPL-3.0-or-later
//! Base colour: Principled BSDF `Base Color` → `baseColorFactor` (RGB) and
//! `baseColorTexture` (ARCHITECTURE §16.2).
//!
//! Lowering ports `__gather_base_color_factor` / `gather_color_info`
//! (`io_scene_gltf2/blender/exp/material/pbr_metallic_roughness.py`,
//! `search_node_tree.py`, glTF-Blender-IO 5.2.41, Apache-2.0):
//! - unlinked socket, RGB node: the factor, clamped to [0, 1];
//! - Mix (RGBA, MULTIPLY, Factor 1) of a constant and an Image Texture: factor × texture;
//! - Image Texture `Color`: the texture, factor white;
//! - when what is left of the Alpha socket (after `alpha.rs`'s clip and factor) is the
//!   `Alpha` output of the same image, the texture is read as RGBA.
//!
//! A material whose surface is not a Principled BSDF keeps the glTF default material and
//! gets a WARNING (unlit/emission shaders arrive with M3).
//!
//! Raising ports the colour half of `base_color()` (`imp/pbrMetallicRoughness.py`): constant
//! factor (and `Material.diffuse_color`, `set_viewport_color`), or, after the alpha clip
//! frame that `alpha.rs` built, a "Color Factor" Mix when the factor is not white, an
//! "Alpha Factor" Math MULTIPLY when the texture alpha is used and the alpha factor is not
//! 1, and the Image Texture labelled "BASE COLOR", in the importer's creation order.

use frap_nodes::{Node, SocketAddr, SocketValue};

use super::alpha::{alpha_info, texture_alpha_plan};
use crate::graph::{Src, unrecognised};
use crate::{ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// `gather_color_info` without colour attributes (M3 `vertex_color.rs`): the constant factor
/// and the source left after peeling a multiply-by-constant.
pub(crate) fn color_info<'a>(
    cx: &LowerCx<'a>,
    src: Src<'a>,
) -> (Option<[f32; 3]>, Option<Src<'a>>) {
    if let Some(c) = cx.const_rgb(src) {
        return (Some(c), None);
    }
    if let Some((c, rest)) = cx.color_times_const(src) {
        return (Some(c), Some(rest));
    }
    match src {
        Src::Linked(..) => (None, Some(src)),
        _ => (None, None),
    }
}

/// Short description of the node behind a source, for diagnostics.
pub(crate) fn describe(src: Src<'_>) -> String {
    match src {
        Src::Linked(n, _) => node_desc(n),
        _ => "a constant".into(),
    }
}

fn node_desc(n: &Node) -> String {
    format!("\"{}\" ({})", n.name, n.bl_idname)
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else {
        let what = match cx.surface_shader() {
            Some(n) => format!(
                "the surface shader {} is not a Principled BSDF",
                node_desc(n)
            ),
            None => "no shader is linked to the Material Output's Surface".into(),
        };
        unrecognised(
            &mut out.diagnostics,
            format!(
                "Material \"{}\": {what}; lowered as the glTF default material",
                cx.mat.header.name
            ),
        );
        return;
    };
    let (factor, rest) = color_info(cx, cx.src(p, "Base Color"));
    let rgb = factor.unwrap_or([1.0; 3]).map(|c| c.clamp(0.0, 1.0));
    out.base_color.factor[..3].copy_from_slice(&rgb);
    out.base_color.texture = None;
    if let Some(rest) = rest {
        match cx.image_behind(rest) {
            Some((tex, ChannelSel::Rgb)) => {
                out.base_color.texture = cx.tex_ref(tex, ChannelSel::Rgb, &mut out.diagnostics);
                if out.base_color.texture.is_none() {
                    unrecognised(
                        &mut out.diagnostics,
                        format!("Base Color: Image Texture \"{}\" has no image", tex.name),
                    );
                }
            }
            _ => unrecognised(
                &mut out.diagnostics,
                format!(
                    "Base Color: unsupported node setup at {}; exported without a texture",
                    describe(rest)
                ),
            ),
        }
    }
    // The alpha left behind the clip/factor nodes: this texture's alpha channel, or
    // something the exporter would pack into a new image (M3).
    let Some(info) = alpha_info(cx) else { return };
    if let Src::Linked(..) = info.rest {
        let image = out.base_color.texture.as_ref().map(|t| t.image);
        match cx.image_behind(info.rest) {
            Some((tex, ChannelSel::A)) if image.is_some() && cx.image_of(tex) == image => {
                if let Some(t) = out.base_color.texture.as_mut() {
                    t.channel = ChannelSel::Rgba;
                }
            }
            _ => unrecognised(
                &mut out.diagnostics,
                format!(
                    "Alpha: unsupported node setup at {}; only the base colour image's alpha is exported",
                    describe(info.rest)
                ),
            ),
        }
    }
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(bsdf) = cx.bsdf else { return };
    let f = lm.base_color.factor;
    let rgb = [f[0], f[1], f[2]];
    let socket = cx.input(bsdf, "Base Color");
    let Some(tex) = &lm.base_color.texture else {
        cx.color_factor_and_texture([0.0, 0.0], "Color", socket, rgb, None);
        cx.into.diffuse_color = f;
        return;
    };
    let [mut x, y] = cx.location("base_color");
    let (tex_alpha, clip, needs_alpha_factor) = texture_alpha_plan(lm);
    if clip.is_some() {
        x -= 400.0;
    }
    let needs_color_factor = rgb != [1.0, 1.0, 1.0];
    let mut color_socket = socket;
    if needs_color_factor {
        let mix = cx.add_node("ShaderNodeMix", [x - 140.0, y]);
        cx.set_label(mix, "Color Factor");
        cx.set_enum(mix, "data_type", "RGBA");
        cx.set_enum(mix, "blend_type", "MULTIPLY");
        let o = cx.output(mix, "Result");
        cx.link(color_socket, o);
        cx.set_input(mix, "Factor", SocketValue::Float(1.0));
        color_socket = cx.input(mix, "A");
        cx.set_input(mix, "B", SocketValue::Color([rgb[0], rgb[1], rgb[2], 1.0]));
    }
    let mut alpha_to = if tex_alpha {
        cx.base_alpha_socket
    } else {
        None
    };
    if needs_alpha_factor && let Some(socket) = alpha_to {
        let m = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
        cx.set_label(m, "Alpha Factor");
        cx.link(socket, SocketAddr::output(m, 0));
        cx.set_enum(m, "operation", "MULTIPLY");
        alpha_to = Some(SocketAddr::input(m, 0));
        cx.set_input_at(m, 1, SocketValue::Float(f[3]));
    }
    if needs_color_factor || needs_alpha_factor {
        x -= 200.0;
    }
    cx.texture(tex, [x, y], "BASE COLOR", Some(color_socket), alpha_to);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "base_color",
    order: 20,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 10,
    key: "base_color",
    takes_space: |lm| lm.base_color.texture.is_some() || lm.vertex_color.is_some(),
};
