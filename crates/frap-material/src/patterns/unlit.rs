// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_unlit` (ARCHITECTURE §16.2): a "shadeless" material, whose surface is a
//! colour rather than a lit shader.
//!
//! Lowering ports `detect_shadeless_material` and `__export_unlit`
//! (`io_scene_gltf2/blender/exp/material/unlit.py`, `materials.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors). Recognised:
//! - a Background shader linked to the active output (its `Color`);
//! - the active output's `Surface` fed by a colour (an RGB node, a texture, …), optionally
//!   through the "lightpath trick" (a Mix Shader whose factor is Light Path `Is Camera Ray`
//!   and whose second shader is an Emission, whose `Color` is then read), and optionally
//!   through a Mix Shader with a Transparent BSDF first, whose factor is the alpha.
//!
//! An Emission shader alone on the output is **not** unlit (the add-on exports it as an
//! emissive material, `emission.rs`). An unlit material lowers to `unlit`, the base colour
//! (factor not clamped; the texture only when every socket read has one, the image being the
//! last one's, as the add-on's ALL filter and channel copies do), the alpha mode of the alpha
//! socket, metallic 0 and roughness 0.9; every other pattern leaves it alone.
//!
//! Raising ports `unlit()` (`imp/KHR_materials_unlit.py`): Emission, Light Path, Transparent
//! BSDF and Mix Shader ("lightpath trick"), a second Mix Shader with the Transparent BSDF
//! for non-opaque materials, the Material Output, then `base_color()` into the Emission's
//! `Color` at (−200, 380).

use frap_nodes::SocketAddr;

use super::alpha::gather_alpha_info;
use super::vertex_color::vertex_color_of;
use crate::graph::In;
use crate::{AlphaMode, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// What `detect_shadeless_material` finds.
struct Shadeless<'a> {
    /// `rgb_socket`
    rgb: In<'a>,
    /// `alpha_socket`
    alpha: Option<In<'a>>,
}

/// `__detect_lightpath_trick(socket)`: the socket the colour comes from.
fn lightpath_trick<'a>(cx: &LowerCx<'a>, socket: In<'a>) -> Option<In<'a>> {
    let (mix, _) = cx.peek_back(socket)?;
    if mix.bl_idname != "ShaderNodeMixShader" {
        return None;
    }
    let (lp, o) = cx.peek_back(In {
        node: mix,
        index: 0,
    })?;
    if lp.bl_idname != "ShaderNodeLightPath" || lp.outputs[o].identifier != "Is Camera Ray" {
        return None;
    }
    let mut next = In {
        node: mix,
        index: 2,
    };
    if let Some((em, _)) = cx.peek_back(next)
        && em.bl_idname == "ShaderNodeEmission"
    {
        next = In { node: em, index: 0 };
    }
    Some(next)
}

/// `__detect_mix_alpha(socket)`: the alpha socket and the socket the colour comes from.
fn mix_alpha<'a>(cx: &LowerCx<'a>, socket: In<'a>) -> Option<(In<'a>, In<'a>)> {
    let (mix, _) = cx.peek_back(socket)?;
    if mix.bl_idname != "ShaderNodeMixShader" {
        return None;
    }
    let (tr, _) = cx.peek_back(In {
        node: mix,
        index: 1,
    })?;
    (tr.bl_idname == "ShaderNodeBsdfTransparent").then_some((
        In {
            node: mix,
            index: 0,
        },
        In {
            node: mix,
            index: 2,
        },
    ))
}

/// `detect_shadeless_material`.
fn detect<'a>(cx: &LowerCx<'a>) -> Option<Shadeless<'a>> {
    if let Some(bg) = cx.get_socket("Background") {
        return Some(Shadeless {
            rgb: bg,
            alpha: None,
        });
    }
    let out = cx.material_output()?;
    let mut socket = In {
        node: out,
        index: 0,
    };
    let mut alpha = None;
    if let Some(next) = lightpath_trick(cx, socket) {
        socket = next;
    } else {
        if let Some((a, next)) = mix_alpha(cx, socket) {
            alpha = Some(a);
            socket = next;
        }
        if let Some(next) = lightpath_trick(cx, socket) {
            socket = next;
        }
    }
    let is_rgba = |s: &frap_nodes::Socket| s.ty == frap_nodes::SocketType::Rgba;
    if !is_rgba(socket.socket()) {
        let (n, o) = cx.peek_back(socket)?;
        if !is_rgba(&n.outputs[o]) {
            return None;
        }
    }
    Some(Shadeless { rgb: socket, alpha })
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    out.unlit = false;
    let Some(s) = detect(cx) else { return };
    out.unlit = true;
    let rgb = cx.color_info(s.rgb).factor.unwrap_or([1.0; 3]);
    let (alpha, mode) = match s.alpha {
        Some(a) => {
            let info = gather_alpha_info(cx, a);
            (info.factor.unwrap_or(1.0), info.mode)
        }
        None => (1.0, AlphaMode::Opaque),
    };
    out.base_color.factor = [rgb[0], rgb[1], rgb[2], alpha];
    out.alpha_mode = mode;
    out.metallic.factor = 0.0;
    out.roughness.factor = 0.9;
    // `gather_base_color_texture`: every socket must lead to a texture (filter ALL); the
    // image is copied from each in turn, so the last one's wins.
    let sockets: Vec<In> = std::iter::once(s.rgb).chain(s.alpha).collect();
    let hits: Option<Vec<_>> = sockets.iter().map(|&i| cx.texture_from(i)).collect();
    out.base_color.texture = hits.and_then(|hits| {
        let mut t = cx.tex_ref(hits[0].node, ChannelSel::Rgb, &mut out.diagnostics)?;
        if let Some(last) = hits.get(1) {
            t.image = cx.image_of(last.node)?;
            t.channel = ChannelSel::Rgba;
        }
        Some(t)
    });
    out.vertex_color = vertex_color_of(cx, Some(s.rgb), s.alpha);
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    if !lm.unlit {
        return;
    }
    // `unlit()` replaces `pbr_metallic_roughness()`: start from an empty tree.
    {
        let t = &mut cx.into.node_tree;
        t.nodes.clear();
        t.links.clear();
    }
    cx.names.taken.clear();
    cx.bsdf = None;
    let emission = cx.add_node("ShaderNodeEmission", [10.0, 126.0]);
    let lightpath = cx.add_node("ShaderNodeLightPath", [10.0, 600.0]);
    let transparent = cx.add_node("ShaderNodeBsdfTransparent", [10.0, 240.0]);
    let mix = cx.add_node("ShaderNodeMixShader", [260.0, 320.0]);
    let ray = cx.output(lightpath, "Is Camera Ray");
    cx.link(SocketAddr::input(mix, 0), ray);
    cx.link(
        SocketAddr::input(mix, 1),
        SocketAddr::output(transparent, 0),
    );
    cx.link(SocketAddr::input(mix, 2), SocketAddr::output(emission, 0));
    let mut alpha_socket = None;
    let out = if lm.alpha_mode == AlphaMode::Opaque {
        let out = cx.add_node("ShaderNodeOutputMaterial", [490.0, 290.0]);
        cx.link(SocketAddr::input(out, 0), SocketAddr::output(mix, 0));
        out
    } else {
        let mix2 = cx.add_node("ShaderNodeMixShader", [490.0, -50.0]);
        alpha_socket = Some(SocketAddr::input(mix2, 0));
        let out = cx.add_node("ShaderNodeOutputMaterial", [700.0, -70.0]);
        cx.link(
            SocketAddr::input(mix2, 1),
            SocketAddr::output(transparent, 0),
        );
        cx.link(SocketAddr::input(mix2, 2), SocketAddr::output(mix, 0));
        cx.link(SocketAddr::input(out, 0), SocketAddr::output(mix2, 0));
        out
    };
    cx.output = Some(out);
    let color = cx.input(emission, "Color");
    super::base_color::raise_base_color(lm, cx, [-200.0, 380.0], color, alpha_socket);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "unlit",
    order: 1,
    lower,
    raise: Some(raise),
};

#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;
    use crate::{AlphaMode, ChannelSel, LoweredMaterial};

    fn plain_unlit(lm: &LoweredMaterial) -> (bool, [f32; 4], f32, f32, AlphaMode) {
        (
            lm.unlit,
            lm.base_color.factor,
            lm.metallic.factor,
            lm.roughness.factor,
            lm.alpha_mode,
        )
    }

    #[test]
    fn shadeless_setups() {
        let images = Images::default();
        // A Background shader.
        let mut m = mat("Bg");
        let bg = add(&mut m, "ShaderNodeBackground");
        set_in(&mut m, bg, "Color", color(2.0, 0.4, 0.6));
        link_at(&mut m, (bg, 0), (OUT, 0));
        let lm = lower_m(&m, &images);
        assert_eq!(
            plain_unlit(&lm),
            (true, [2.0, 0.4, 0.6, 1.0], 0.0, 0.9, AlphaMode::Opaque),
            "not clamped"
        );
        assert!(lm.diagnostics.is_empty());
        assert_law(&lm, &images);
        // An RGB node on Surface.
        let mut m = mat("Rgb");
        let rgb = add(&mut m, "ShaderNodeRGB");
        m.node_tree.node_mut(rgb).unwrap().outputs[0].default_value = color(1.0, 0.5, 0.0);
        link_at(&mut m, (rgb, 0), (OUT, 0));
        assert_eq!(lower_m(&m, &images).base_color.factor, [1.0, 0.5, 0.0, 1.0]);
        // An Emission shader alone is emissive, not unlit.
        let mut m = mat("Glow");
        let em = add(&mut m, "ShaderNodeEmission");
        link_at(&mut m, (em, 0), (OUT, 0));
        assert!(!lower_m(&m, &images).unlit);
    }

    #[test]
    fn lightpath_trick_with_alpha_and_texture() {
        let mut images = Images::default();
        let img = images.add("tex", false);
        // The importer's graph for BLEND 0.5 with a texture.
        let mut lm = LoweredMaterial::gltf_default();
        lm.unlit = true;
        lm.alpha_mode = AlphaMode::Blend;
        lm.base_color.factor = [0.5, 0.25, 1.0, 0.5];
        lm.metallic.factor = 0.0;
        lm.roughness.factor = 0.9;
        let (m, again) = raise_lower(&lm, &images);
        let names: Vec<&str> = m.node_tree.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Emission",
                "Light Path",
                "Transparent BSDF",
                "Mix Shader",
                "Mix Shader.001",
                "Material Output"
            ]
        );
        assert_eq!(plain_unlit(&again), plain_unlit(&lm));
        // With an RGBA texture: Color into the Emission, Alpha into the alpha Mix Shader.
        lm.base_color.texture = Some(crate::TexRef {
            image: img,
            uv: crate::UvSource::ActiveRender,
            channel: ChannelSel::Rgba,
            transform: None,
            sampler: crate::Sampler {
                filter: crate::Filter::Linear,
                wrap_s: crate::Wrap::Repeat,
                wrap_t: crate::Wrap::Repeat,
            },
            colorspace_is_data: false,
        });
        let (_, again) = raise_lower(&lm, &images);
        assert_eq!(again.base_color.texture, lm.base_color.texture);
        assert_eq!(plain_unlit(&again), plain_unlit(&lm));
        // Opaque: no alpha Mix Shader.
        lm.alpha_mode = AlphaMode::Opaque;
        lm.base_color.factor[3] = 1.0;
        lm.base_color.texture = None;
        let (m, again) = raise_lower(&lm, &images);
        assert_eq!(m.node_tree.nodes.len(), 5);
        assert_eq!(plain_unlit(&again), plain_unlit(&lm));
    }
}
