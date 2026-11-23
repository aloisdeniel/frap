// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_anisotropy`: Principled BSDF `Anisotropic`, `Anisotropic Rotation` (× 2π)
//! and the importer's tangent-space texture setup → `anisotropyStrength`,
//! `anisotropyRotation` and `anisotropyTexture` (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_anisotropy` and `detect_anisotropy_nodes`
//! (`io_scene_gltf2/blender/exp/material/extensions/anisotropy.py`, `search_node_tree.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0):
//! - both sockets unlinked: strength = `Anisotropic`, rotation = `Anisotropic Rotation` × 2π;
//! - the importer's setup (Tangent node in UV_MAP mode on `Tangent`; `Anisotropic` ← Math
//!   MULTIPLY ← Separate XYZ `Z`; Separate XYZ `X`/`Y` → Math ARCTAN2 → Math ADD → Math DIVIDE
//!   by 2π → `Anisotropic Rotation`; Separate XYZ ← Vector Math MULTIPLY_ADD (×(2, 2, 1),
//!   +(−1, −1, 0)) ← Image Texture): strength = the MULTIPLY's constant, rotation = the ADD's
//!   constant (already in radians), the image copied whole;
//! - otherwise, with an image behind either socket, the add-on bakes an image from their
//!   grey values (`export_anisotropy_from_grayscale_textures`, `MaterialExts::
//!   anisotropy_grayscale`, WP 8.12); without any image there is no extension.
//!
//! Raising ports `anisotropy()` (`imp/KHR_materials_anisotropy.py`): the two socket values, or
//! the setup above (Tangent, "Anisotropy strength", "Rotation conversion", "Anisotropy
//! rotation", "ArcTan2", Separate XYZ, Vector Math, Image Texture "ANISOTROPY").

use std::f64::consts::TAU;

use frap_nodes::{Node, SocketAddr, SocketValue};
use frap_rna::Value;

use super::transmission::ext_common::{factor_f32, image_from_socket, konst_f32, uv_map_name};
use crate::graph::Src;
use crate::{
    Anisotropy, ChannelSel, GrayscaleAnisotropy, LowerCx, LoweredMaterial, MaterialPattern,
    RaiseCx, TexRef,
};

/// The 2π that `detect_anisotropy_nodes` compares the DIVIDE node's input with (a literal in
/// the add-on, within 1e-4).
#[allow(clippy::approx_constant)]
const ADDON_TAU: f64 = 6.283185;

/// `socket.links[0].from_node` (and the output identifier), no reroute skipping.
fn from<'a>(cx: &LowerCx<'a>, node: &Node, i: usize) -> Option<(&'a Node, &'a str)> {
    let s = node.inputs.get(i)?;
    let l = cx.tree.link_into(node.uid, &s.identifier)?;
    Some((cx.tree.node(l.from_node)?, l.from_socket.as_str()))
}

/// `output.links[0]`: the first link leaving output `i` of `node` (`tree.links` order).
fn first_link_from<'a>(cx: &LowerCx<'a>, node: &Node, i: usize) -> Option<(&'a Node, &'a str)> {
    let id = &node.outputs.get(i)?.identifier;
    let l = cx
        .tree
        .links
        .iter()
        .find(|l| l.from_node == node.uid && &l.from_socket == id)?;
    Some((cx.tree.node(l.to_node)?, l.to_socket.as_str()))
}

fn is_math(n: &Node, op: &str) -> bool {
    n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == op
}

fn vec3(n: &Node, i: usize) -> Option<[f32; 3]> {
    match n.inputs.get(i)?.default_value {
        SocketValue::Vector(v) => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

/// `previous_node(socket)`: the node behind input `i`, reroutes skipped.
fn previous<'a>(cx: &LowerCx<'a>, node: &'a Node, i: usize) -> Option<&'a Node> {
    match cx.src_at(node, i) {
        Src::Linked(n, _) => Some(n),
        _ => None,
    }
}

/// `detect_anisotropy_nodes`: `(strength, rotation, texture node)`.
fn detect<'a>(
    cx: &LowerCx<'a>,
    p: &'a Node,
    (ai, ri, ti): (usize, usize, usize),
) -> Option<(Option<f32>, Option<f32>, &'a Node)> {
    let tangent = previous(cx, p, ti)?;
    if tangent.bl_idname != "ShaderNodeTangent" || tangent.prop_str("direction_type") != "UV_MAP" {
        return None;
    }
    from(cx, p, ri)?;
    let (mul, _) = from(cx, p, ai)?;
    if !is_math(mul, "MULTIPLY") {
        return None;
    }
    let (sep, z) = from(cx, mul, 0)?;
    if sep.bl_idname != "ShaderNodeSeparateXYZ" || z != "Z" {
        return None;
    }
    let (atan, _) = first_link_from(cx, sep, 0)?;
    if !is_math(atan, "ARCTAN2") || from(cx, atan, 0)?.1 != "Y" || from(cx, atan, 1)?.1 != "X" {
        return None;
    }
    let (add, _) = first_link_from(cx, atan, 0)?;
    if !is_math(add, "ADD") {
        return None;
    }
    let (div, _) = first_link_from(cx, add, 0)?;
    if !is_math(div, "DIVIDE") {
        return None;
    }
    match div.inputs.get(1).map(|s| &s.default_value) {
        Some(SocketValue::Float(f)) if (f64::from(*f) - ADDON_TAU).abs() <= 0.0001 => {}
        _ => return None,
    }
    let (to, to_socket) = first_link_from(cx, div, 0)?;
    if to_socket != "Anisotropic Rotation" || to.bl_idname != "ShaderNodeBsdfPrincipled" {
        return None;
    }
    let (vm, _) = from(cx, sep, 0)?;
    if vm.bl_idname != "ShaderNodeVectorMath"
        || vm.prop_str("operation") != "MULTIPLY_ADD"
        || vec3(vm, 1)? != [2.0, 2.0, 1.0]
        || vec3(vm, 2)? != [-1.0, -1.0, 0.0]
    {
        return None;
    }
    let (tex, _) = from(cx, vm, 0)?;
    if tex.bl_idname != "ShaderNodeTexImage" {
        return None;
    }
    let found = image_from_socket(cx, vm, 0)?;
    let strength = cx.const_f32(cx.src_at(mul, 1));
    let rotation = cx.const_f32(cx.src_at(add, 1));
    Some((strength, rotation, found.node))
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let (Some(ai), Some(ri), Some(ti)) = (
        p.input_index("Anisotropic"),
        p.input_index("Anisotropic Rotation"),
        p.input_index("Tangent"),
    ) else {
        return;
    };
    if let (Some(s), Some(r)) = (
        konst_f32(cx, cx.src_at(p, ai)),
        konst_f32(cx, cx.src_at(p, ri)),
    ) {
        out.ext.anisotropy = Some(Anisotropy {
            strength: s,
            rotation: (f64::from(r) * TAU) as f32,
            texture: None,
        });
        return;
    }
    if let Some((strength, rotation, tex)) = detect(cx, p, (ai, ri, ti)) {
        let texture = cx.tex_ref(tex, ChannelSel::Rgb, &mut out.diagnostics);
        out.ext.anisotropy = Some(Anisotropy {
            strength: strength.unwrap_or(0.0),
            rotation: rotation.unwrap_or(0.0),
            texture,
        });
        return;
    }
    // `export_anisotropy_from_grayscale_textures` (WP 8.12): the primary socket is
    // `Anisotropic` when it has an image, else `Anisotropic Rotation`; the strength is the
    // factor of `Anisotropic` (1 without one), the rotation the factor of `Anisotropic Rotation`
    // × 2π (0 without one).
    let (sh, rh) = (image_from_socket(cx, p, ai), image_from_socket(cx, p, ri));
    if sh.is_none() && rh.is_none() {
        return;
    }
    let strength_tex = sh.and_then(|h| cx.tex_ref(h.node, ChannelSel::Rgb, &mut out.diagnostics));
    let rotation_tex = rh.and_then(|h| cx.tex_ref(h.node, ChannelSel::Rgb, &mut out.diagnostics));
    let primary = strength_tex.clone().or_else(|| rotation_tex.clone());
    let strength = factor_f32(cx, cx.src_at(p, ai)).unwrap_or(1.0);
    let rotation_f64 = factor_f32(cx, cx.src_at(p, ri)).map_or(0.0, |f| f64::from(f) * TAU);
    let rotation = rotation_f64 as f32;
    out.ext.anisotropy = Some(Anisotropy {
        strength,
        rotation,
        texture: primary,
    });
    out.ext.anisotropy_grayscale = Some(GrayscaleAnisotropy {
        strength: strength_tex,
        rotation: rotation_tex,
        rotation_f64,
    });
}

fn raise_texture(cx: &mut RaiseCx, bsdf: u32, a: &Anisotropy, tex: &TexRef) {
    let [x, y] = cx.location("anisotropy");
    let tangent_socket = cx.input(bsdf, "Tangent");
    let strength_socket = cx.input(bsdf, "Anisotropic");
    let rotation_socket = cx.input(bsdf, "Anisotropic Rotation");
    let tangent = cx.add_node("ShaderNodeTangent", [x - 180.0, y - 200.0]);
    cx.set_enum(tangent, "direction_type", "UV_MAP");
    cx.set_prop(tangent, "uv_map", Value::Str(uv_map_name(&tex.uv).into()));
    let o = cx.output(tangent, "Tangent");
    cx.link(tangent_socket, o);
    let math = |cx: &mut RaiseCx, label: &str, op: &str, at: [f32; 2]| {
        let m = cx.add_node("ShaderNodeMath", at);
        cx.set_label(m, label);
        cx.set_enum(m, "operation", op);
        m
    };
    let mul = math(
        cx,
        "Anisotropy strength",
        "MULTIPLY",
        [x - 180.0, y + 200.0],
    );
    cx.link(strength_socket, SocketAddr::output(mul, 0));
    cx.set_input_at(mul, 1, SocketValue::Float(a.strength));
    let div = math(cx, "Rotation conversion", "DIVIDE", [x - 180.0, y]);
    cx.link(rotation_socket, SocketAddr::output(div, 0));
    cx.set_input_at(div, 1, SocketValue::Float(TAU as f32));
    let add = math(cx, "Anisotropy rotation", "ADD", [x - 360.0, y]);
    cx.link(SocketAddr::input(div, 0), SocketAddr::output(add, 0));
    cx.set_input_at(add, 1, SocketValue::Float(a.rotation));
    let atan = math(cx, "ArcTan2", "ARCTAN2", [x - 540.0, y]);
    cx.link(SocketAddr::input(add, 0), SocketAddr::output(atan, 0));
    let sep = cx.add_node("ShaderNodeSeparateXYZ", [x - 720.0, y]);
    cx.link(SocketAddr::input(atan, 0), SocketAddr::output(sep, 1));
    cx.link(SocketAddr::input(atan, 1), SocketAddr::output(sep, 0));
    cx.link(SocketAddr::input(mul, 0), SocketAddr::output(sep, 2));
    let vm = cx.add_node("ShaderNodeVectorMath", [x - 900.0, y]);
    cx.set_enum(vm, "operation", "MULTIPLY_ADD");
    cx.set_input_at(vm, 1, SocketValue::Vector([2.0, 2.0, 1.0, 0.0]));
    cx.set_input_at(vm, 2, SocketValue::Vector([-1.0, -1.0, 0.0, 0.0]));
    cx.link(SocketAddr::input(sep, 0), SocketAddr::output(vm, 0));
    cx.texture(
        tex,
        [x - 1080.0, y],
        "ANISOTROPY",
        Some(SocketAddr::input(vm, 0)),
        None,
    );
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(a), Some(bsdf)) = (&lm.ext.anisotropy, cx.bsdf) else {
        return;
    };
    match &a.texture {
        None => {
            cx.set_input(bsdf, "Anisotropic", SocketValue::Float(a.strength));
            cx.set_input(
                bsdf,
                "Anisotropic Rotation",
                SocketValue::Float((f64::from(a.rotation) / TAU) as f32),
            );
        }
        Some(t) => raise_texture(cx, bsdf, a, t),
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "anisotropy",
    order: 350,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 70,
    key: "anisotropy",
    takes_space: |lm| {
        lm.ext
            .anisotropy
            .as_ref()
            .is_some_and(|a| a.texture.is_some())
    },
};


#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;

    /// WP 8.12: textures outside the importer's set-up are baked from their grey values: the
    /// strength texture is the primary one, the strength 1 without a factor, the rotation the
    /// unlinked socket's value × 2π.
    #[test]
    fn grayscale_textures_are_baked() {
        let mut images = Images::default();
        let img = images.add("strength", true);
        let mut m = mat("Gray");
        let t = image_node(&mut m, img);
        link(&mut m, (t, "Color"), (BSDF, "Anisotropic"));
        set_in(&mut m, BSDF, "Anisotropic Rotation", float(0.25));
        let lm = lower_m(&m, &images);
        let a = lm.ext.anisotropy.as_ref().unwrap();
        assert_eq!(a.strength, 1.0);
        assert!((f64::from(a.rotation) - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(a.texture.as_ref().unwrap().image, img);
        let g = lm.ext.anisotropy_grayscale.as_ref().unwrap();
        assert_eq!(g.strength.as_ref().unwrap().image, img);
        assert!(g.rotation.is_none());
        // Both sockets unlinked: no bake.
        let m = mat("Plain");
        assert!(lower_m(&m, &images).ext.anisotropy_grayscale.is_none());
    }
}
