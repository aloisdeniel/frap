// SPDX-License-Identifier: GPL-3.0-or-later
//! Occlusion: the `Occlusion` input of the "glTF Material Output" group node →
//! `occlusionTexture` with `strength` (ARCHITECTURE §16.2). The Principled BSDF has no
//! occlusion socket, so the add-on's group node carries it; the exporter packs the occlusion
//! (R) with metallic (B) and roughness (G) into one ORM image when the three images have the
//! same size.
//!
//! Lowering ports `__gather_occlusion_texture` (`io_scene_gltf2/blender/exp/material/
//! materials.py`) and `__gather_occlusion_strength` (`texture_info.py`, glTF-Blender-IO
//! 5.2.41, Apache-2.0, The glTF-Blender-IO authors): the texture is the first Image Texture
//! found behind the socket, read through a Separate Color output on the way (R by
//! default); the strength is the factor of a Mix (blend MIX) of white and the texture in
//! front of the socket (`1 − factor` when white is the second colour).
//!
//! Raising ports `occlusion()` and `make_settings_node()` (`imp/pbrMetallicRoughness.py`):
//! the group node (`RaiseCx::settings_node`; the importer links its `node_tree` to
//! `bpy.data.node_groups['glTF Material Output']`), an "Occlusion Strength" Mix with white
//! when the strength is not 1, a Separate Color (`Red`) and the Image Texture labelled
//! "OCCLUSION".

use frap_nodes::{SocketAddr, SocketValue};

use super::metallic_roughness::scalar_texture;
use crate::graph::{In, Konst};
use crate::{ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// The occlusion socket: a Principled `Occlusion` input (none in 5.2), else the glTF Material
/// Output's.
pub(crate) fn occlusion_socket<'a>(cx: &LowerCx<'a>) -> Option<In<'a>> {
    cx.get_socket("Occlusion")
        .or_else(|| cx.gltf_output_input("Occlusion"))
}

/// `__gather_occlusion_strength(primary_socket)`.
fn strength(cx: &LowerCx, socket: In) -> Option<f32> {
    let (n, _) = cx.peek_back(socket)?;
    if n.bl_idname != "ShaderNodeMix" || n.prop_str("blend_type") != "MIX" {
        return None;
    }
    let fac = In::named(n, "Factor")
        .and_then(|f| cx.get_constant(f))
        .and_then(Konst::value)?;
    let col = |id| In::by_identifier(n, id).and_then(|s| cx.get_constant(s));
    let white = Some(Konst::Rgb([1.0; 3]));
    match (col("A_Color"), col("B_Color")) {
        (a, None) if a == white => Some(fac),
        (None, b) if b == white => Some((1.0 - f64::from(fac)) as f32),
        _ => None,
    }
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    out.occlusion = None;
    out.occlusion_strength = 1.0;
    if out.unlit {
        return;
    }
    let Some(socket) = occlusion_socket(cx) else {
        return;
    };
    out.occlusion = scalar_texture(cx, Some(socket), &mut out.diagnostics);
    if out.occlusion.is_some() {
        out.occlusion_strength = strength(cx, socket).unwrap_or(1.0);
    }
}

pub(crate) fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(_), Some(tex)) = (cx.bsdf, &lm.occlusion) else {
        return;
    };
    let settings = cx.settings_node();
    let mut socket = cx.input(settings, "Occlusion");
    let [mut x, y] = cx.location("occlusion");
    let strength = lm.occlusion_strength;
    if strength != 1.0 {
        // Mix with white.
        let mix = cx.add_node("ShaderNodeMix", [x - 140.0, y]);
        cx.set_label(mix, "Occlusion Strength");
        cx.set_enum(mix, "data_type", "RGBA");
        cx.set_enum(mix, "blend_type", "MIX");
        let o = cx.output(mix, "Result");
        cx.link(socket, o);
        cx.set_input(mix, "Factor", SocketValue::Float(strength));
        cx.set_input(mix, "A", SocketValue::Color([1.0; 4]));
        socket = cx.input(mix, "B");
        x -= 200.0;
    }
    let out = match tex.channel {
        ChannelSel::G => Some(1),
        ChannelSel::B => Some(2),
        ChannelSel::A => None,
        _ => Some(0),
    };
    let (color_to, alpha_to) = match out {
        Some(o) => {
            let sep = cx.add_node("ShaderNodeSeparateColor", [x - 150.0, y - 75.0]);
            cx.link(socket, SocketAddr::output(sep, o));
            x -= 200.0;
            (Some(cx.input(sep, "Color")), None)
        }
        None => (None, Some(socket)),
    };
    cx.texture(tex, [x, y], "OCCLUSION", color_to, alpha_to);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "occlusion",
    order: 44,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 150,
    key: "occlusion",
    takes_space: |lm| lm.occlusion.is_some(),
};

#[cfg(test)]
mod tests {
    use frap_nodes::SocketValue;

    use crate::ChannelSel;
    use crate::graph::testkit::*;

    #[test]
    fn occlusion_from_the_gltf_output_group() {
        let mut images = Images::default();
        let img = images.add("ao", true);
        let mut m = mat("AO");
        let t = image_node(&mut m, img);
        let g = settings_node(&mut m);
        let mix = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mix, "data_type", "RGBA");
        set_in(&mut m, mix, "Factor", float(0.6));
        set_in(&mut m, mix, "A", SocketValue::Color([1.0; 4]));
        link(&mut m, (t, "Color"), (mix, "B"));
        link(&mut m, (mix, "Result"), (g, "Occlusion"));
        let lm = lower_m(&m, &images);
        let o = lm.occlusion.as_ref().unwrap();
        assert_eq!((o.image, o.channel), (img, ChannelSel::R));
        assert_eq!(lm.occlusion_strength, 0.6);
        assert_law(&lm, &images);
        // Raised: the settings node "Group" (its five inputs), an "Occlusion Strength" Mix.
        let (raised, _) = raise_lower(&lm, &images);
        let group = raised.node_tree.node_by_name("Group").unwrap();
        assert_eq!(group.inputs.len(), 5);
        assert_eq!((group.location, group.width), ([40.0, -370.0], 180.0));
        assert!(
            raised
                .node_tree
                .nodes
                .iter()
                .any(|n| n.label == "Occlusion Strength")
        );
        // White as the second colour: strength 1 − factor.
        let mut m = mat("Reversed");
        let t = image_node(&mut m, img);
        let g = settings_node(&mut m);
        let mix = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mix, "data_type", "RGBA");
        set_in(&mut m, mix, "Factor", float(0.25));
        set_in(&mut m, mix, "B", SocketValue::Color([1.0; 4]));
        link(&mut m, (t, "Color"), (mix, "A"));
        link(&mut m, (mix, "Result"), (g, "Occlusion"));
        assert_eq!(lower_m(&m, &images).occlusion_strength, 0.75);
        // Through a Separate Color: that channel.
        let mut m = mat("Green");
        let t = image_node(&mut m, img);
        let g = settings_node(&mut m);
        let sep = add(&mut m, "ShaderNodeSeparateColor");
        link(&mut m, (t, "Color"), (sep, "Color"));
        link(&mut m, (sep, "Green"), (g, "Occlusion"));
        let lm = lower_m(&m, &images);
        assert_eq!(lm.occlusion.as_ref().unwrap().channel, ChannelSel::G);
        assert_eq!(lm.occlusion_strength, 1.0);
        assert_law(&lm, &images);
    }
}
