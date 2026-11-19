// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_clearcoat`: Principled BSDF `Coat Weight`, `Coat Roughness` and
//! `Coat Normal` → `clearcoatFactor`/`clearcoatTexture` (R), `clearcoatRoughnessFactor`/
//! `clearcoatRoughnessTexture` (G) and `clearcoatNormalTexture` (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_clearcoat` (`io_scene_gltf2/blender/exp/material/extensions/
//! clearcoat.py`, glTF-Blender-IO 5.2.41, Apache-2.0): each socket gives its unlinked value,
//! or the texture behind it (packed: R for the weight, G for the roughness, unless a
//! Separate Color output names the source channel) and its factor (1 without a constant
//! multiplier). A socket linked to anything else gives the glTF default (weight 0) or Blender's
//! (`BLENDER_COAT_ROUGHNESS` 0.03, which the exporter does not write). The normal texture is
//! the image behind `Coat Normal` (copied whole) with the first Normal Map's unlinked
//! `Strength` as scale. `Some` whenever there is a Principled BSDF.
//!
//! Raising ports `clearcoat()` (`imp/pbrMetallicRoughness.py`): `scalar_factor_and_texture`
//! for the weight ("Clearcoat", R) and the roughness ("Clearcoat Roughness", G), then
//! `normal_map` ("Clearcoat Normal").

use super::transmission::ext_common::{
    TexKind, image_from_socket, normal_map, scalar_factor_and_texture, scalar_member,
};
use crate::{
    Channel, ChannelSel, Clearcoat, LowerCx, LoweredMaterial, MaterialPattern, NormalMap, RaiseCx,
};

/// `BLENDER_COAT_ROUGHNESS` (`io/com/constants.py`): the Principled BSDF's default coat
/// roughness, which the exporter treats as "not set".
pub const BLENDER_COAT_ROUGHNESS: f32 = 0.03;

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let (Some(w), Some(r)) = (
        p.input_index("Coat Weight"),
        p.input_index("Coat Roughness"),
    ) else {
        return;
    };
    let weight = scalar_member(
        cx,
        p,
        w,
        TexKind::Packed(ChannelSel::R),
        "Coat Weight",
        &mut out.diagnostics,
    );
    let rough = scalar_member(
        cx,
        p,
        r,
        TexKind::Packed(ChannelSel::G),
        "Coat Roughness",
        &mut out.diagnostics,
    );
    let normal = p.input_index("Coat Normal").and_then(|n| {
        let found = image_from_socket(cx, p, n)?;
        let texture = cx.tex_ref(found.node, ChannelSel::Rgb, &mut out.diagnostics)?;
        Some(NormalMap {
            texture,
            scale: normal_scale(cx, p, n),
        })
    });
    out.ext.clearcoat = Some(Clearcoat {
        factor: Channel {
            factor: weight.factor.unwrap_or(0.0),
            texture: weight.texture,
        },
        roughness: Channel {
            factor: rough.factor.unwrap_or(BLENDER_COAT_ROUGHNESS),
            texture: rough.texture,
        },
        normal,
    });
}

/// `__gather_normal_scale`: the unlinked `Strength` of the first Normal Map node found behind
/// the socket (depth first), 1 otherwise.
fn normal_scale(cx: &LowerCx, p: &frap_nodes::Node, input: usize) -> f32 {
    fn find<'a>(
        cx: &LowerCx<'a>,
        node: &'a frap_nodes::Node,
        depth: usize,
    ) -> Option<&'a frap_nodes::Node> {
        if depth > cx.tree.nodes.len() {
            return None;
        }
        node.inputs.iter().find_map(|s| {
            let l = cx.tree.link_into(node.uid, &s.identifier)?;
            let from = cx.tree.node(l.from_node)?;
            if from.bl_idname == "ShaderNodeNormalMap" {
                return Some(from);
            }
            find(cx, from, depth + 1)
        })
    }
    let s = &p.inputs[input];
    let first = cx
        .tree
        .link_into(p.uid, &s.identifier)
        .and_then(|l| cx.tree.node(l.from_node));
    let nm = match first {
        Some(n) if n.bl_idname == "ShaderNodeNormalMap" => Some(n),
        Some(n) => find(cx, n, 0),
        None => None,
    };
    let Some(nm) = nm else { return 1.0 };
    match nm.input_index("Strength").map(|i| cx.src_at(nm, i)) {
        Some(crate::graph::Src::Const(frap_nodes::SocketValue::Float(f))) => *f,
        _ => 1.0,
    }
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(c), Some(bsdf)) = (&lm.ext.clearcoat, cx.bsdf) else {
        return;
    };
    let socket = cx.input(bsdf, "Coat Weight");
    let loc = cx.location("clearcoat");
    scalar_factor_and_texture(
        cx,
        loc,
        "Clearcoat",
        socket,
        c.factor.factor,
        c.factor.texture.as_ref(),
        0,
    );
    let socket = cx.input(bsdf, "Coat Roughness");
    let loc = cx.location("clearcoat_roughness");
    scalar_factor_and_texture(
        cx,
        loc,
        "Clearcoat Roughness",
        socket,
        c.roughness.factor,
        c.roughness.texture.as_ref(),
        1,
    );
    if let Some(n) = &c.normal {
        let socket = cx.input(bsdf, "Coat Normal");
        let loc = cx.location("clearcoat_normal");
        normal_map(cx, loc, "Clearcoat Normal", socket, n);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "clearcoat",
    order: 300,
    lower,
    raise: Some(raise),
};

fn clearcoat(lm: &LoweredMaterial) -> Option<&Clearcoat> {
    lm.ext.clearcoat.as_ref()
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 100,
    key: "clearcoat",
    takes_space: |lm| clearcoat(lm).is_some_and(|c| c.factor.texture.is_some()),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT_ROUGHNESS: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 110,
    key: "clearcoat_roughness",
    takes_space: |lm| clearcoat(lm).is_some_and(|c| c.roughness.texture.is_some()),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT_NORMAL: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 120,
    key: "clearcoat_normal",
    takes_space: |lm| clearcoat(lm).is_some_and(|c| c.normal.is_some()),
};
