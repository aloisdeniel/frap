// SPDX-License-Identifier: GPL-3.0-or-later
//! Alpha: Principled BSDF `Alpha` → `baseColorFactor[3]`, `alphaMode` and `alphaCutoff`
//! (ARCHITECTURE §16.2). The alpha mode comes from the node graph, never from
//! `blend_method` [RNA §0.9]. Whether the base-colour texture's alpha channel is used (RGBA)
//! is decided by `base_color.rs`, which owns that texture.
//!
//! Lowering ports `gather_alpha_info` / `detect_alpha_clip`
//! (`io_scene_gltf2/blender/exp/material/search_node_tree.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0): constant 1 → OPAQUE; clip setups (Math ROUND → cutoff 0.5;
//! `1 − (x < c)`; `1 − (c > x)`; legacy `x > c`, `c < x`) → MASK; then the factor (constant,
//! or Math MULTIPLY by a constant); without a clip, a factor 0 → MASK 0.5, 1 → OPAQUE,
//! anything else (another constant, a texture) → BLEND.
//!
//! Raising ports the alpha half of `base_color()` (`imp/pbrMetallicRoughness.py`) and
//! `set_eevee_surface_render_method` (`imp/material.py`): without a base-colour texture
//! alpha, a constant (OPAQUE → 1, MASK → `factor >= cutoff`, BLEND → factor); with one, the
//! "Alpha Clip" frame (`1 − (alpha < cutoff)`) for MASK, whose free input is left in
//! [`RaiseCx::base_alpha_socket`]. The pattern runs before `base_color`, which then creates
//! the "Color Factor" Mix, the "Alpha Factor" Math MULTIPLY and the texture in that order,
//! as the importer's `base_color()` does (node names and `tree.nodes`/`tree.links` order
//! match). BLEND gives `surface_render_method = 'BLENDED'` (`blend_method` mirror `'BLEND'`),
//! else `'DITHERED'` (`'HASHED'`), verified on 5.2.4.

use frap_nodes::{SocketAddr, SocketValue};

use crate::graph::Src;
use crate::{AlphaMode, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// What `gather_alpha_info` reads from the Alpha socket.
pub(crate) struct AlphaInfo<'a> {
    /// The alpha mode.
    pub mode: AlphaMode,
    /// The constant factor (`None`: no constant found, i.e. 1).
    pub factor: Option<f32>,
    /// What is left behind the clip and factor nodes (a texture's `Alpha` output, …).
    pub rest: Src<'a>,
}

/// `detect_alpha_clip`: the cutoff and the source behind the clip nodes.
fn detect_clip<'a>(cx: &LowerCx<'a>, src: Src<'a>) -> Option<(f32, Src<'a>)> {
    let Src::Linked(n, _) = src else { return None };
    if n.bl_idname != "ShaderNodeMath" {
        return None;
    }
    let (in0, in1) = (cx.src_at(n, 0), cx.src_at(n, 1));
    match n.prop_str("operation") {
        "ROUND" => return Some((0.5, in0)),
        "SUBTRACT" if cx.const_f32(in0) == Some(1.0) => {
            if let Src::Linked(n2, _) = in1
                && n2.bl_idname == "ShaderNodeMath"
            {
                let (a, b) = (cx.src_at(n2, 0), cx.src_at(n2, 1));
                match (n2.prop_str("operation"), cx.const_f32(a), cx.const_f32(b)) {
                    ("LESS_THAN", None, Some(c)) => return Some((c, a)),
                    ("GREATER_THAN", Some(c), None) => return Some((c, b)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    match (
        n.prop_str("operation"),
        cx.const_f32(in0),
        cx.const_f32(in1),
    ) {
        ("GREATER_THAN", _, Some(c)) => Some((c, in0)),
        ("LESS_THAN", Some(c), _) => Some((c, in1)),
        _ => None,
    }
}

/// `gather_alpha_info` without colour attributes (M3): `None` without a Principled BSDF.
pub(crate) fn alpha_info<'a>(cx: &LowerCx<'a>) -> Option<AlphaInfo<'a>> {
    let p = cx.principled()?;
    let src = cx.src(p, "Alpha");
    if cx.const_f32(src) == Some(1.0) {
        return Some(AlphaInfo {
            mode: AlphaMode::Opaque,
            factor: Some(1.0),
            rest: Src::Missing,
        });
    }
    let (cutoff, mut rest) = match detect_clip(cx, src) {
        Some((c, rest)) => (Some(c), rest),
        None => (None, src),
    };
    let mut factor = None;
    if let Some(c) = cx.const_f32(rest) {
        factor = Some(c);
        rest = Src::Missing;
    } else if let Some((c, other)) = cx.value_times_const(rest) {
        factor = Some(c);
        rest = other;
    }
    let mode = match (cutoff, factor) {
        (Some(c), _) => AlphaMode::Mask { cutoff: c },
        (None, Some(0.0)) => AlphaMode::Mask { cutoff: 0.5 },
        (None, Some(1.0)) => AlphaMode::Opaque,
        _ => AlphaMode::Blend,
    };
    Some(AlphaInfo { mode, factor, rest })
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(info) = alpha_info(cx) else { return };
    out.alpha_mode = info.mode;
    out.base_color.factor[3] = info.factor.unwrap_or(1.0);
}

/// Whether the raised graph links the base-colour texture's alpha, and whether it needs the
/// clip nodes and the factor node: `(texture alpha used, clip cutoff, factor node)`.
pub(crate) fn texture_alpha_plan(lm: &LoweredMaterial) -> (bool, Option<f32>, bool) {
    let a = lm.base_color.factor[3];
    let tex_alpha = lm
        .base_color
        .texture
        .as_ref()
        .is_some_and(|t| t.channel == ChannelSel::Rgba);
    if !tex_alpha {
        return (false, None, false);
    }
    match lm.alpha_mode {
        AlphaMode::Opaque => (false, None, false),
        AlphaMode::Mask { cutoff } if cutoff == 0.0 || cutoff > 1.0 => (false, None, false),
        AlphaMode::Mask { cutoff } => (true, Some(cutoff), a != 1.0),
        AlphaMode::Blend => (true, None, a != 1.0),
    }
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    cx.into.surface_render_method = match lm.alpha_mode {
        AlphaMode::Blend => "BLENDED",
        _ => "DITHERED",
    }
    .into();
    cx.into.blend_method = match lm.alpha_mode {
        AlphaMode::Blend => "BLEND",
        _ => "HASHED",
    }
    .into();
    let Some(bsdf) = cx.bsdf else { return };
    let a = lm.base_color.factor[3];
    let alpha = cx.input(bsdf, "Alpha");
    let (tex_alpha, clip, _) = texture_alpha_plan(lm);
    if !tex_alpha {
        let v = match lm.alpha_mode {
            AlphaMode::Opaque => 1.0,
            AlphaMode::Mask { cutoff } if cutoff > 1.0 => 0.0,
            AlphaMode::Mask { cutoff } if a >= cutoff => 1.0,
            AlphaMode::Mask { .. } => 0.0,
            AlphaMode::Blend => a,
        };
        cx.set_input_at(bsdf, alpha.index, SocketValue::Float(v));
        return;
    }
    let [mut x, y] = cx.location("base_color");
    // `base_color()` up to its factor nodes: the clip frame; `base_color.rs` goes on from
    // the free input left in `base_alpha_socket`.
    let mut socket: SocketAddr = alpha;
    if let Some(cutoff) = clip {
        // 1 - (alpha < cutoff), since there is no >= node.
        let frame = cx.add_node("NodeFrame", [0.0, 0.0]);
        cx.set_label(frame, "Alpha Clip");
        let sub = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
        cx.node_mut(sub).parent = Some(frame);
        cx.link(socket, SocketAddr::output(sub, 0));
        cx.set_enum(sub, "operation", "SUBTRACT");
        cx.set_input_at(sub, 0, SocketValue::Float(1.0));
        socket = SocketAddr::input(sub, 1);
        x -= 200.0;
        let lt = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
        cx.node_mut(lt).parent = Some(frame);
        cx.link(socket, SocketAddr::output(lt, 0));
        cx.set_enum(lt, "operation", "LESS_THAN");
        socket = SocketAddr::input(lt, 0);
        cx.set_input_at(lt, 1, SocketValue::Float(cutoff));
    }
    cx.base_alpha_socket = Some(socket);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "alpha",
    order: 15,
    lower,
    raise: Some(raise),
};
