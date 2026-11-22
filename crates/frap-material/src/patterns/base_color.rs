// SPDX-License-Identifier: GPL-3.0-or-later
//! Base colour: Principled BSDF `Base Color` (and `Alpha`) → `baseColorFactor` and
//! `baseColorTexture` (ARCHITECTURE §16.2).
//!
//! Lowering ports `__gather_base_color_factor` / `__gather_base_color_texture`
//! (`io_scene_gltf2/blender/exp/material/pbr_metallic_roughness.py`) with `gather_color_info`
//! and the texture search of `search_node_tree.py` and the channel rules of `image.py`
//! (glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors):
//! - the factor peels a multiplication by a constant (and one by a colour attribute,
//!   `vertex_color.rs`) off the socket; RGB clamped to [0, 1], alpha from `alpha.rs`;
//! - the texture is the first Image Texture found behind `Base Color`, else behind `Alpha`
//!   (the add-on's depth-first search, through any node); an `Alpha` fed by the same image's
//!   `Alpha` output makes the texture RGBA ([`ChannelSel::Rgba`]); a texture behind `Alpha`
//!   only gives a white image carrying that channel as alpha (channel `A`, or `R`/`G`/`B`
//!   through a Separate Color);
//! - without a Principled BSDF, an Emission shader with an emissive factor gives the add-on's
//!   `get_default_pbr_for_emissive_node` (`baseColorFactor` [0, 0, 0, 1]); any other surface
//!   keeps the glTF default material and gets a WARNING.
//!
//! An alpha taken from another image (or another channel of it, or from an image's colour
//! output, RGB → BW) is packed into the base colour image's alpha as the add-on does
//! (`MaterialExts::base_alpha`, WP 8.12); a grey alpha alone gives a white image carrying it.
//!
//! Raising ports `base_color()` (`imp/pbrMetallicRoughness.py`): constant factor (and
//! `Material.diffuse_color`, `set_viewport_color`), or the "Alpha Clip" frame
//! (`1 − (alpha < cutoff)`) for MASK, a "Color Factor" Mix and an "Alpha Factor" Math
//! MULTIPLY when needed, the "Mix Vertex Color" / "Mix Vertex Alpha" nodes and a Color
//! Attribute node when the primitive has `COLOR_0`, and the Image Texture labelled
//! "BASE COLOR", in the importer's creation order. `unlit.rs` calls [`raise_base_color`] with
//! its Emission node.

use frap_nodes::{Node, SocketAddr, SocketValue};

use crate::graph::{In, Src, SrcChannel, unrecognised};
use crate::{
    AlphaMode, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, PackedAlpha, RaiseCx, TexRef,
};

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

/// The base-colour texture of a colour socket and an alpha socket (`__gather_base_color_texture`
/// with the channel rules of `image.py`): see the module documentation.
pub(crate) fn base_color_texture<'a>(
    cx: &LowerCx<'a>,
    color: Option<In<'a>>,
    alpha: Option<In<'a>>,
    diags: &mut Vec<frap_rna::Report>,
) -> (Option<TexRef>, Option<PackedAlpha>) {
    let color_hit = color.and_then(|s| cx.texture_from(s));
    let alpha_hit = alpha.and_then(|s| cx.texture_from(s).map(|h| (s, h)));
    match (color_hit, alpha_hit) {
        (Some(c), alpha_hit) => {
            let Some(mut t) = cx.tex_ref(c.node, ChannelSel::Rgb, diags) else {
                return (None, None);
            };
            let mut packed = None;
            if let Some((a, ah)) = alpha_hit {
                let same = cx.image_of(ah.node) == Some(t.image);
                match cx.source_channel(a, &ah) {
                    SrcChannel::One(ChannelSel::A) if same => t.channel = ChannelSel::Rgba,
                    // Another image, another channel, or a colour converted to grey: packed
                    // into the base colour image's alpha (WP 8.12).
                    ch => {
                        let (channel, rgb_to_bw) = match ch {
                            SrcChannel::One(ch) => (ch, false),
                            SrcChannel::Rgb2Bw => (ChannelSel::A, true),
                        };
                        packed = cx.tex_ref(ah.node, channel, diags).map(|texture| PackedAlpha {
                            texture,
                            rgb_to_bw,
                        });
                    }
                }
            }
            (Some(t), packed)
        }
        (None, Some((a, ah))) => match cx.source_channel(a, &ah) {
            SrcChannel::One(ch) => (cx.tex_ref(ah.node, ch, diags), None),
            SrcChannel::Rgb2Bw => {
                // A grey alpha alone: a white image carrying it (WP 8.12).
                let t = cx.tex_ref(ah.node, ChannelSel::A, diags);
                let packed = t.clone().map(|texture| PackedAlpha {
                    texture,
                    rgb_to_bw: true,
                });
                (t, packed)
            }
        },
        (None, None) => (None, None),
    }
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit {
        return;
    }
    let base_socket = cx
        .get_socket("Base Color")
        .or_else(|| cx.get_socket("BaseColor"));
    let Some(p) = cx.principled() else {
        let factor_socket = base_socket.or_else(|| cx.gltf_output_input("BaseColorFactor"));
        if out.emissive.factor != [0.0; 3] && factor_socket.is_none() {
            // `get_default_pbr_for_emissive_node`
            out.base_color.factor = [0.0, 0.0, 0.0, 1.0];
            return;
        }
        let what = match cx.surface_shader() {
            Some(n) => format!(
                "the surface shader {} is not a Principled BSDF",
                node_desc(n)
            ),
            None if cx.inline_fails() => {
                "its node tree has a link cycle, which Blender's exporter cannot inline".into()
            }
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
    let factor_socket = base_socket.or_else(|| cx.gltf_output_input("BaseColorFactor"));
    let rgb = factor_socket
        .and_then(|s| cx.color_info(s).factor)
        .unwrap_or([1.0; 3])
        .map(|c| c.clamp(0.0, 1.0));
    out.base_color.factor[..3].copy_from_slice(&rgb);
    let tex_socket = base_socket.or_else(|| cx.gltf_output_input("BaseColor"));
    let alpha_socket = In::named(p, "Alpha");
    let (texture, packed) = base_color_texture(cx, tex_socket, alpha_socket, &mut out.diagnostics);
    out.base_color.texture = texture;
    out.ext.base_alpha = packed;
    // A Base Color linked to something that is neither a texture, a factor nor a colour
    // attribute: the add-on exports the factor only.
    if let Some(s) = base_socket
        && cx.texture_from(s).is_none()
        && cx.get_constant(s).is_none()
    {
        let info = cx.color_info(s);
        if info.factor.is_none() && info.attrib.is_none() {
            unrecognised(
                &mut out.diagnostics,
                format!(
                    "Base Color: unsupported node setup at {}; exported without a texture",
                    describe(cx.src_in(s))
                ),
            );
        }
    }
}

/// Whether the texture's colour, the texture's alpha (or one channel of it, for an
/// alpha-only texture) and a vertex colour feed the raised graph.
fn texture_uses(lm: &LoweredMaterial) -> (bool, bool) {
    match lm.base_color.texture.as_ref().map(|t| t.channel) {
        None => (false, false),
        Some(ChannelSel::Rgb) => (true, false),
        Some(ChannelSel::Rgba) => (true, true),
        Some(_) => (false, true),
    }
}

/// `base_color()` of the importer: see the module documentation. `alpha_socket` is `None`
/// when the shader has no alpha input (an opaque unlit material); `label` is the Image
/// Texture's (`"BASE COLOR"`, or `"DIFFUSE"` for `is_diffuse`, the spec-gloss diffuse).
///
/// Lowered materials that the importer cannot produce are raised so that lowering the
/// graph gives them back (`lower(raise(lower(x))) == lower(x)`): a texture whose alpha is not
/// used leaves the alpha a constant (MASK: the clip frame fed by the factor), and an
/// alpha-only texture feeds only the alpha.
pub(crate) fn raise_base_color(
    lm: &LoweredMaterial,
    cx: &mut RaiseCx,
    location: [f32; 2],
    color_socket: SocketAddr,
    alpha_socket: Option<SocketAddr>,
    label: &str,
) {
    let [mut x, y] = location;
    let f = lm.base_color.factor;
    let color_factor = [f[0], f[1], f[2]];
    let alpha_factor = f[3];
    let texture = lm.base_color.texture.as_ref();
    let vertex_color = lm.vertex_color.is_some();
    // Which halves the Color Attribute node feeds.
    let vc_color = lm.vertex_color.as_ref().is_some_and(|v| v.color);
    let vc_alpha = lm.vertex_color.as_ref().is_some_and(|v| v.alpha);
    let rgba = |c: [f32; 3]| SocketValue::Color([c[0], c[1], c[2], 1.0]);
    // Only factor.
    if texture.is_none() && !vertex_color {
        cx.set_input_at(color_socket.node, color_socket.index, rgba(color_factor));
        if let Some(a) = alpha_socket {
            let v = match lm.alpha_mode {
                AlphaMode::Opaque => 1.0,
                AlphaMode::Mask { cutoff } if alpha_factor >= cutoff => 1.0,
                AlphaMode::Mask { .. } => 0.0,
                AlphaMode::Blend => alpha_factor,
            };
            cx.set_input_at(a.node, a.index, SocketValue::Float(v));
        }
        return;
    }
    let (tex_color, tex_alpha) = texture_uses(lm);
    // Something feeds the alpha besides the factor (the importer assumes it always does).
    let alpha_linked = tex_alpha || vc_alpha;
    let mut alpha_socket = alpha_socket;
    if let Some(a) = alpha_socket {
        match lm.alpha_mode {
            AlphaMode::Opaque => {
                cx.set_input_at(a.node, a.index, SocketValue::Float(1.0));
                alpha_socket = None;
            }
            AlphaMode::Mask { cutoff } if cutoff == 0.0 || cutoff > 1.0 => {
                let v = if cutoff == 0.0 { 1.0 } else { 0.0 };
                cx.set_input_at(a.node, a.index, SocketValue::Float(v));
                alpha_socket = None;
            }
            AlphaMode::Mask { cutoff } => {
                // 1 - (alpha < cutoff), since there is no >= node.
                let frame = cx.add_node("NodeFrame", [0.0, 0.0]);
                cx.set_label(frame, "Alpha Clip");
                let sub = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
                cx.node_mut(sub).parent = Some(frame);
                cx.link(a, SocketAddr::output(sub, 0));
                cx.set_enum(sub, "operation", "SUBTRACT");
                cx.set_input_at(sub, 0, SocketValue::Float(1.0));
                let next = SocketAddr::input(sub, 1);
                x -= 200.0;
                let lt = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
                cx.node_mut(lt).parent = Some(frame);
                cx.link(next, SocketAddr::output(lt, 0));
                cx.set_enum(lt, "operation", "LESS_THAN");
                cx.set_input_at(lt, 1, SocketValue::Float(cutoff));
                x -= 200.0;
                if alpha_linked {
                    alpha_socket = Some(SocketAddr::input(lt, 0));
                } else {
                    cx.set_input_at(lt, 0, SocketValue::Float(alpha_factor));
                    alpha_socket = None;
                }
            }
            AlphaMode::Blend if !alpha_linked => {
                cx.set_input_at(a.node, a.index, SocketValue::Float(alpha_factor));
                alpha_socket = None;
            }
            AlphaMode::Blend => {}
        }
    }
    // Mix in the base colour factor.
    let mut color_socket = color_socket;
    let color_linked = tex_color || vc_color;
    if !color_linked {
        cx.set_input_at(color_socket.node, color_socket.index, rgba(color_factor));
    }
    let needs_color_factor = color_linked && color_factor != [1.0, 1.0, 1.0];
    let needs_alpha_factor = alpha_factor != 1.0 && alpha_socket.is_some();
    if needs_color_factor || needs_alpha_factor {
        if needs_color_factor {
            let mix = cx.add_node("ShaderNodeMix", [x - 140.0, y]);
            cx.set_label(mix, "Color Factor");
            cx.set_enum(mix, "data_type", "RGBA");
            cx.set_enum(mix, "blend_type", "MULTIPLY");
            let o = cx.output(mix, "Result");
            cx.link(color_socket, o);
            cx.set_input(mix, "Factor", SocketValue::Float(1.0));
            color_socket = cx.input(mix, "A");
            cx.set_input(mix, "B", rgba(color_factor));
        }
        if let Some(a) = alpha_socket.filter(|_| needs_alpha_factor) {
            let m = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
            cx.set_label(m, "Alpha Factor");
            cx.link(a, SocketAddr::output(m, 0));
            cx.set_enum(m, "operation", "MULTIPLY");
            alpha_socket = Some(SocketAddr::input(m, 0));
            cx.set_input_at(m, 1, SocketValue::Float(alpha_factor));
        }
        x -= 200.0;
    }
    // Where the texture and the vertex colour put their outputs.
    let (mut tex_color_to, mut tex_alpha_to) = (color_socket, alpha_socket);
    let mut vc_color_to = vc_color.then_some(color_socket);
    let mut vc_alpha_to = alpha_socket.filter(|_| vc_alpha);
    if texture.is_some() && vertex_color {
        if vc_color {
            let mix = cx.add_node("ShaderNodeMix", [x - 140.0, y]);
            cx.set_label(mix, "Mix Vertex Color");
            cx.set_enum(mix, "data_type", "RGBA");
            cx.set_enum(mix, "blend_type", "MULTIPLY");
            let o = cx.output(mix, "Result");
            cx.link(color_socket, o);
            cx.set_input(mix, "Factor", SocketValue::Float(1.0));
            tex_color_to = cx.input(mix, "A");
            vc_color_to = Some(cx.input(mix, "B"));
        }
        if let Some(a) = alpha_socket.filter(|_| vc_alpha && tex_alpha) {
            let m = cx.add_node("ShaderNodeMath", [x - 140.0, y - 230.0]);
            cx.set_label(m, "Mix Vertex Alpha");
            cx.set_enum(m, "operation", "MULTIPLY");
            cx.link(a, SocketAddr::output(m, 0));
            tex_alpha_to = Some(SocketAddr::input(m, 0));
            vc_alpha_to = Some(SocketAddr::input(m, 1));
        }
        x -= 200.0;
    }
    if let Some(v) = &lm.vertex_color {
        // A Color Attribute node naming the attribute (empty: the active one, as the
        // importer makes it); a second one when Alpha reads another attribute.
        let vc_node = |cx: &mut RaiseCx, name: &Option<smol_str::SmolStr>, dy: f32| {
            let vc = cx.add_node("ShaderNodeVertexColor", [x - 250.0, y - 240.0 - dy]);
            if let Some(n) = name {
                cx.set_prop(vc, "layer_name", frap_rna::Value::Str(n.clone()));
            }
            vc
        };
        let vc = vc_node(
            cx,
            if v.color {
                &v.attribute
            } else {
                &v.alpha_attribute
            },
            0.0,
        );
        if let Some(c) = vc_color_to {
            let o = cx.output(vc, "Color");
            cx.link(c, o);
        }
        if let Some(a) = vc_alpha_to {
            let from = if v.color && v.alpha_attribute != v.attribute {
                vc_node(cx, &v.alpha_attribute, 120.0)
            } else {
                vc
            };
            let o = cx.output(from, "Alpha");
            cx.link(a, o);
        }
        x -= 280.0;
    }
    let Some(t) = texture else { return };
    let alpha_to = if tex_alpha { tex_alpha_to } else { None };
    match t.channel {
        ChannelSel::R | ChannelSel::G | ChannelSel::B => {
            // An alpha-only texture read through one colour channel.
            let Some(a) = alpha_to else { return };
            let sep = cx.add_node("ShaderNodeSeparateColor", [x - 150.0, y - 75.0]);
            let out = match t.channel {
                ChannelSel::R => 0,
                ChannelSel::G => 1,
                _ => 2,
            };
            cx.link(a, SocketAddr::output(sep, out));
            let c = cx.input(sep, "Color");
            cx.texture(t, [x - 200.0, y], label, Some(c), None);
        }
        _ => {
            let color_to = tex_color.then_some(tex_color_to);
            cx.texture(t, [x, y], label, color_to, alpha_to);
        }
    }
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(bsdf) = cx.bsdf else { return };
    let color = cx.input(bsdf, "Base Color");
    let alpha = cx.input(bsdf, "Alpha");
    let loc = cx.location("base_color");
    // `set_viewport_color`
    if lm.base_color.texture.is_none() && lm.vertex_color.is_none() {
        cx.into.diffuse_color = lm.base_color.factor;
    }
    raise_base_color(lm, cx, loc, color, Some(alpha), "BASE COLOR");
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

#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;
    use crate::{AlphaMode, ChannelSel, VertexColorUse};

    #[test]
    fn texture_found_through_any_node() {
        let mut images = Images::default();
        let img = images.add("albedo", false);
        // Texture × constant through a Mix MULTIPLY (factor 1): factor and texture.
        let mut m = mat("Tinted");
        let t = image_node(&mut m, img);
        let mix = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mix, "data_type", "RGBA");
        set_enum(&mut m, mix, "blend_type", "MULTIPLY");
        set_in(&mut m, mix, "Factor", float(1.0));
        set_in(&mut m, mix, "B", color(0.5, 0.25, 1.0));
        link(&mut m, (t, "Color"), (mix, "A"));
        link(&mut m, (mix, "Result"), (BSDF, "Base Color"));
        let lm = lower_m(&m, &images);
        assert_eq!(lm.base_color.factor, [0.5, 0.25, 1.0, 1.0]);
        assert_eq!(lm.base_color.texture.as_ref().unwrap().image, img);
        assert!(lm.diagnostics.is_empty(), "{:?}", lm.diagnostics);
        assert_law(&lm, &images);
        // Through an unrelated node (Hue/Saturation): still the texture, factor white.
        let mut m = mat("Hue");
        let t = image_node(&mut m, img);
        let hue = add(&mut m, "ShaderNodeHueSaturation");
        link(&mut m, (t, "Color"), (hue, "Color"));
        link(&mut m, (hue, "Color"), (BSDF, "Base Color"));
        let lm = lower_m(&m, &images);
        assert_eq!(lm.base_color.texture.as_ref().unwrap().image, img);
        assert_eq!(lm.base_color.factor, [1.0; 4]);
    }

    #[test]
    fn alpha_textures() {
        let mut images = Images::default();
        let (a, b) = (images.add("a", false), images.add("b", false));
        // An alpha-only texture: channel A, the colour constant.
        let mut m = mat("AlphaOnly");
        set_in(&mut m, BSDF, "Base Color", color(0.2, 0.6, 0.4));
        let t = image_node(&mut m, a);
        link(&mut m, (t, "Alpha"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        let tex = lm.base_color.texture.as_ref().unwrap();
        assert_eq!((tex.image, tex.channel), (a, ChannelSel::A));
        assert_eq!(lm.base_color.factor, [0.2, 0.6, 0.4, 1.0]);
        assert_eq!(lm.alpha_mode, AlphaMode::Blend);
        assert_law(&lm, &images);
        // Alpha from another image than the colour: not packed, WARNING.
        let mut m = mat("TwoImages");
        let ta = image_node(&mut m, a);
        let tb = image_node(&mut m, b);
        link(&mut m, (ta, "Color"), (BSDF, "Base Color"));
        link(&mut m, (tb, "Alpha"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert_eq!(
            lm.base_color.texture.as_ref().unwrap().channel,
            ChannelSel::Rgb
        );
        assert_eq!(lm.diagnostics.len(), 1);
        // A colour output into Alpha (RGB → BW): WARNING, no texture.
        let mut m = mat("Gray");
        let ta = image_node(&mut m, a);
        link(&mut m, (ta, "Color"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert!(lm.base_color.texture.is_none());
        assert!(lm.diagnostics[0].message.contains("grey"));
    }

    #[test]
    fn vertex_colour_and_texture_raise_like_the_importer() {
        let mut images = Images::default();
        let a = images.add("a", false);
        let mut m = mat("Painted");
        let t = image_node(&mut m, a);
        link(&mut m, (t, "Color"), (BSDF, "Base Color"));
        link(&mut m, (t, "Alpha"), (BSDF, "Alpha"));
        let mut lm = lower_m(&m, &images);
        lm.vertex_color = Some(VertexColorUse::color_and_alpha(None, true));
        let (raised, _) = raise_lower(&lm, &images);
        let labels: Vec<&str> = raised
            .node_tree
            .nodes
            .iter()
            .map(|n| n.label.as_str())
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(
            labels,
            ["Mix Vertex Color", "Mix Vertex Alpha", "BASE COLOR"]
        );
        assert!(
            raised
                .node_tree
                .first_of_type("ShaderNodeVertexColor")
                .is_some()
        );
    }
}
