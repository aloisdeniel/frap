// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_iridescence`: Principled BSDF `Thin Film Thickness` / `Thin Film IOR` and the
//! "glTF Material Output" group's `Iridescence Factor` / `Iridescence Thickness Minimum` →
//! `iridescenceFactor`/`iridescenceTexture` (R), `iridescenceIor`,
//! `iridescenceThicknessMinimum`/`Maximum` and `iridescenceThicknessTexture` (G)
//! (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_iridescence` and `detect_iridescence_thickness_texure`
//! (`io_scene_gltf2/blender/exp/material/extensions/iridescence.py`, `search_node_tree.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0):
//! - no extension without a Principled BSDF or without a group input `Iridescence Factor`;
//! - `iridescenceIor`: `Thin Film IOR`'s own value; `iridescenceFactor`: `get_factor` of the
//!   group input (0 when there is none, which makes the exporter drop the extension) and the
//!   image behind it (copied whole);
//! - thickness: an unlinked `Thin Film Thickness` is the maximum (the minimum is the group's
//!   `Iridescence Thickness Minimum`, which the exporter ignores without a texture); the
//!   importer's texture setup (Mix FLOAT ← Separate Color ← Image Texture on the Factor, Value
//!   nodes on A and B, A's Value also feeding the group's minimum) gives the texture (copied
//!   whole, glTF reads G) and both Value nodes; anything else gives `get_factor` of the socket
//!   (400 when there is none).
//!
//! Raising ports `iridescence()` (`imp/KHR_materials_iridescence.py`): nothing without the
//! settings node (created for a non-zero factor); without textures the four sockets get their
//! values; with them, `scalar_factor_and_texture` on the factor ("Iridescence", R) and the
//! thickness setup above, and — as the add-on does — `Thin Film IOR` and, without a thickness
//! texture, `Thin Film Thickness` keep their Blender defaults.

use frap_nodes::{Node, SocketAddr, SocketValue};

use super::transmission::ext_common::{
    TexKind, default_f32, factor_f32, find_settings_node, konst_f32, scalar_factor_and_texture,
    settings_input, texture_behind,
};
use crate::{
    Channel, ChannelSel, Iridescence, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, TexRef,
};

/// `GLTF_IRIDESCENCE_IOR` (`io/com/constants.py`).
pub const GLTF_IRIDESCENCE_IOR: f32 = 1.3;
/// The glTF default minimum thickness (nm).
pub const THICKNESS_MINIMUM: f32 = 100.0;
/// The glTF default maximum thickness (nm).
pub const THICKNESS_MAXIMUM: f32 = 400.0;

/// The node linked into `node.inputs[i]` (first link, no reroute skipping: the add-on reads
/// `socket.links[0].from_node`).
fn from_node<'a>(cx: &LowerCx<'a>, node: &Node, i: usize) -> Option<&'a Node> {
    let s = node.inputs.get(i)?;
    let l = cx.tree.link_into(node.uid, &s.identifier)?;
    cx.tree.node(l.from_node)
}

fn value_of(n: &Node) -> Option<f32> {
    match n.outputs.first()?.default_value {
        SocketValue::Float(f) => Some(f),
        _ => None,
    }
}

/// `detect_iridescence_thickness_texure`: `(minimum, maximum, texture)`.
fn thickness_texture<'a>(
    cx: &LowerCx<'a>,
    p: &'a Node,
    thickness: usize,
    minimum: Option<(&'a Node, usize)>,
    out: &mut LoweredMaterial,
) -> Option<(f32, f32, TexRef)> {
    let mix = from_node(cx, p, thickness)?;
    if mix.bl_idname != "ShaderNodeMix" || mix.prop_str("data_type") != "FLOAT" {
        return None;
    }
    let sep = from_node(cx, mix, mix.input_index("Factor")?)?;
    if sep.bl_idname != "ShaderNodeSeparateColor" {
        return None;
    }
    let tex = from_node(cx, sep, 0)?;
    if tex.bl_idname != "ShaderNodeTexImage" {
        return None;
    }
    let min_node = from_node(cx, mix, mix.input_index("A")?)?;
    if min_node.bl_idname != "ShaderNodeValue" {
        return None;
    }
    let out0 = &min_node.outputs.first()?.identifier;
    let uses = cx
        .tree
        .links
        .iter()
        .filter(|l| l.from_node == min_node.uid && &l.from_socket == out0)
        .count();
    if uses != 2 {
        return None;
    }
    let (g, mi) = minimum?;
    if from_node(cx, g, mi).map(|n| n.uid) != Some(min_node.uid) {
        return None;
    }
    let max_node = from_node(cx, mix, mix.input_index("B")?)?;
    if max_node.bl_idname != "ShaderNodeValue" {
        return None;
    }
    let texture = texture_behind(
        cx,
        sep,
        0,
        TexKind::Full(ChannelSel::G),
        &mut out.diagnostics,
    )?;
    Some((value_of(min_node)?, value_of(max_node)?, texture))
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let Some(ti) = p.input_index("Thin Film Thickness") else {
        return;
    };
    let Some((g, fi)) = settings_input(cx, "Iridescence Factor") else {
        return;
    };
    let minimum = settings_input(cx, "Iridescence Thickness Minimum");
    let ior = p
        .input_index("Thin Film IOR")
        .and_then(|i| default_f32(p, i))
        .unwrap_or(GLTF_IRIDESCENCE_IOR);
    let factor = Channel {
        factor: factor_f32(cx, cx.src_at(g, fi)).unwrap_or(0.0),
        texture: texture_behind(
            cx,
            g,
            fi,
            TexKind::Full(ChannelSel::R),
            &mut out.diagnostics,
        ),
    };
    let group_min = minimum
        .and_then(|(n, i)| factor_f32(cx, cx.src_at(n, i)))
        .unwrap_or(THICKNESS_MINIMUM);
    let (thickness_min, thickness_max) = match cx.src_at(p, ti) {
        src if konst_f32(cx, src).is_some() => (
            group_min,
            Channel {
                factor: konst_f32(cx, src).unwrap_or(THICKNESS_MAXIMUM),
                texture: None,
            },
        ),
        src => match thickness_texture(cx, p, ti, minimum, out) {
            Some((min, max, texture)) => (
                min,
                Channel {
                    factor: max,
                    texture: Some(texture),
                },
            ),
            None => (
                group_min,
                Channel {
                    factor: factor_f32(cx, src).unwrap_or(THICKNESS_MAXIMUM),
                    texture: None,
                },
            ),
        },
    };
    out.ext.iridescence = Some(Iridescence {
        factor,
        ior,
        thickness_min,
        thickness_max,
    });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(ir), Some(bsdf)) = (&lm.ext.iridescence, cx.bsdf) else {
        return;
    };
    // The settings node exists when anything needed it (the `volume` pattern's prelude), a
    // non-zero factor included; without it the importer does nothing.
    let Some(settings) = find_settings_node(cx) else {
        return;
    };
    let factor_socket = cx.input(settings, "Iridescence Factor");
    let min_socket = cx.input(settings, "Iridescence Thickness Minimum");
    let max_socket = cx.input(bsdf, "Thin Film Thickness");
    if ir.factor.texture.is_none() && ir.thickness_max.texture.is_none() {
        cx.set_input_at(
            settings,
            factor_socket.index,
            SocketValue::Float(ir.factor.factor),
        );
        cx.set_input_at(
            bsdf,
            max_socket.index,
            SocketValue::Float(ir.thickness_max.factor),
        );
        cx.set_input_at(
            settings,
            min_socket.index,
            SocketValue::Float(ir.thickness_min),
        );
        cx.set_input(bsdf, "Thin Film IOR", SocketValue::Float(ir.ior));
        return;
    }
    let [x, y] = cx.location("iridescence");
    if let Some(t) = &ir.factor.texture {
        scalar_factor_and_texture(
            cx,
            [x, y],
            "Iridescence",
            factor_socket,
            ir.factor.factor,
            Some(t),
            0,
        );
    }
    if let Some(t) = &ir.thickness_max.texture {
        let mix = cx.add_node("ShaderNodeMix", [x - 180.0, y - 300.0]);
        cx.set_label(mix, "Iridescence Thickness");
        cx.link(max_socket, SocketAddr::output(mix, 0));
        let sep = cx.add_node("ShaderNodeSeparateColor", [x - 360.0, y - 300.0]);
        cx.link(SocketAddr::output(sep, 1), SocketAddr::input(mix, 0));
        cx.texture(
            t,
            [x - 540.0, y - 600.0],
            "IRIDESCENCE THICKNESS",
            Some(SocketAddr::input(sep, 0)),
            None,
        );
        let vmin = cx.add_node("ShaderNodeValue", [x - 360.0, y - 500.0]);
        cx.set_label(vmin, "Iridescence Thickness Minimum");
        cx.node_mut(vmin).outputs[0].default_value = SocketValue::Float(ir.thickness_min);
        cx.link(SocketAddr::input(mix, 2), SocketAddr::output(vmin, 0));
        cx.link(min_socket, SocketAddr::output(vmin, 0));
        let vmax = cx.add_node("ShaderNodeValue", [x - 360.0, y - 600.0]);
        cx.set_label(vmax, "Iridescence Thickness Maximum");
        cx.node_mut(vmax).outputs[0].default_value = SocketValue::Float(ir.thickness_max.factor);
        cx.link(SocketAddr::input(mix, 3), SocketAddr::output(vmax, 0));
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "iridescence",
    order: 370,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 140,
    key: "iridescence",
    takes_space: |lm| {
        lm.ext
            .iridescence
            .as_ref()
            .is_some_and(|i| i.factor.texture.is_some())
    },
};
