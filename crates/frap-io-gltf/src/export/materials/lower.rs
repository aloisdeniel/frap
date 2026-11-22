// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the exporter gets a [`LoweredMaterial`] (ARCHITECTURE §16.2: "the exporter consumes
//! `LoweredMaterial`").
//!
//! [`lowered`] calls `frap_material::lower` as soon as any `MATERIAL_PATTERNS` entry is linked
//! in (WP 1.11). Until then (WP 1.16 landed in the same wave, decision log M1 W2) it uses
//! [`bootstrap`], a minimal recogniser for the M1 vocabulary: the unlinked inputs of the
//! "Principled BSDF" feeding the material output (Base Color, Metallic, Roughness, Alpha,
//! Emission Color × Emission Strength) and an Image Texture whose Color (and optionally Alpha)
//! output feeds Base Color directly, or whose Color feeds Emission Color. Delete `bootstrap` once `frap-material` patterns are
//! merged; nothing else in the exporter depends on it.

use frap_material::{
    AlphaMode, Channel, ChannelSel, Filter, LoweredMaterial, MATERIAL_PATTERNS, MaterialExts,
    Sampler, TexRef, UvSource, Wrap,
};
use frap_nodes::{Node, NodeTree, SocketValue};
use frap_rna::{ImageId, Value};
use frap_scene::Main;
use frap_scene::id::Material;
use frap_spec_ops_io::export_scene::gltf::ExportAnimationMode;

use crate::export::ExportSettings;

/// The lowered material: `frap_material::lower` when patterns are registered, else
/// [`bootstrap`].
pub fn lowered(main: &Main, mat: &Material) -> LoweredMaterial {
    if MATERIAL_PATTERNS.is_empty() {
        bootstrap(main, mat)
    } else {
        frap_material::lower(&mat.node_tree, mat, main)
    }
}

/// `BlenderMaterialIndentifier.__can_use_inline` is false for the exporter's settings
/// (`bmat.used == "ORIGINAL"`, WP 7.7): pointer animation is exported
/// (`gltf_export_anim_pointer`: `export_animations`, `export_pointer_animation` and a SCENE
/// or NLA_TRACKS `export_animation_mode`) and the material's node tree has animation data.
/// The add-on then reads the original tree: no constant folding and no node-group inlining
/// (`frap_material::lower_original`), so a factor fed by a node it cannot read keeps the glTF
/// default (a Math node computing 0.3 x 2 into Roughness exports `roughnessFactor` 1). Its
/// other reasons are the glTF Material Output's `Occlusion` (handled by the patterns,
/// `LowerCx::uses_inline_tree`) and `export_unused_textures` with `export_materials='EXPORT'`
/// (WP 8.12).
pub fn reads_original(settings: &ExportSettings, mat: &Material) -> bool {
    let p = &settings.props;
    let pointer = p.export_animations.value
        && p.export_pointer_animation.value
        && matches!(
            p.export_animation_mode.value,
            ExportAnimationMode::Scene | ExportAnimationMode::NlaTracks
        );
    (pointer && mat.node_tree.animation_data.is_some())
        || crate::export::unused::unused_textures(settings)
}

/// The lowered material the exporter writes for `mat` with `settings` (the original tree
/// when [`reads_original`]).
pub fn lowered_for(main: &Main, mat: &Material, settings: &ExportSettings) -> LoweredMaterial {
    if !MATERIAL_PATTERNS.is_empty() && reads_original(settings, mat) {
        frap_material::lower_original(&mat.node_tree, mat, main)
    } else {
        lowered(main, mat)
    }
}

/// A lowered material with glTF's defaults (white, metallic 1, roughness 1, opaque).
pub fn defaults(mat: &Material) -> LoweredMaterial {
    LoweredMaterial {
        base_color: Channel {
            factor: [1.0; 4],
            texture: None,
        },
        metallic: Channel {
            factor: 1.0,
            texture: None,
        },
        roughness: Channel {
            factor: 1.0,
            texture: None,
        },
        normal: None,
        occlusion: None,
        occlusion_strength: 1.0,
        emissive: Channel {
            factor: [0.0; 3],
            texture: None,
        },
        emissive_strength: 1.0,
        alpha_mode: AlphaMode::Opaque,
        double_sided: !mat.use_backface_culling,
        unlit: false,
        ext: MaterialExts::default(),
        vertex_color: None,
        diagnostics: Vec::new(),
    }
}

fn node(tree: &NodeTree, uid: u32) -> Option<&Node> {
    tree.nodes.iter().find(|n| n.uid == uid)
}

/// The node and output socket identifier linked into `input` of node `to`.
fn link_into<'t>(tree: &'t NodeTree, to: &Node, input: &str) -> Option<(&'t Node, &'t str)> {
    let sock = to.inputs.iter().find(|s| s.enabled && s.name == input)?;
    let l = tree
        .links
        .iter()
        .find(|l| l.to_node == to.uid && l.to_socket == sock.identifier)?;
    Some((node(tree, l.from_node)?, l.from_socket.as_str()))
}

fn input<'n>(n: &'n Node, name: &str) -> Option<&'n SocketValue> {
    n.inputs
        .iter()
        .find(|s| s.enabled && s.name == name)
        .map(|s| &s.default_value)
}

fn float(n: &Node, name: &str, default: f32) -> f32 {
    match input(n, name) {
        Some(SocketValue::Float(f)) => *f,
        _ => default,
    }
}

fn color(n: &Node, name: &str, default: [f32; 4]) -> [f32; 4] {
    match input(n, name) {
        Some(SocketValue::Color(c)) => *c,
        _ => default,
    }
}

fn prop_str<'n>(n: &'n Node, name: &str) -> Option<&'n str> {
    match n.props.values.get(name) {
        Some(Value::Enum(s) | Value::Str(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// An Image Texture node as a texture reference (active-render UV map, its sampler).
fn tex_ref(main: &Main, n: &Node, channel: ChannelSel) -> Option<TexRef> {
    let image = match n.props.values.get("image") {
        Some(Value::Id(Some(id))) => id.typed::<ImageId>()?,
        _ => return None,
    };
    let filter = match prop_str(n, "interpolation") {
        Some("Closest") => Filter::Closest,
        Some("Cubic") => Filter::Cubic,
        _ => Filter::Linear,
    };
    let wrap = match prop_str(n, "extension") {
        Some("EXTEND" | "CLIP") => Wrap::Clamp,
        Some("MIRROR") => Wrap::Mirror,
        _ => Wrap::Repeat,
    };
    let colorspace_is_data = main.images.get(image).is_some_and(|im| {
        im.colorspace_settings.is_data || im.colorspace_settings.name == "Non-Color"
    });
    Some(TexRef {
        image,
        uv: UvSource::ActiveRender,
        channel,
        transform: None,
        sampler: Sampler {
            filter,
            wrap_s: wrap,
            wrap_t: wrap,
        },
        colorspace_is_data,
    })
}

/// The M1 bootstrap recogniser (see the module docs).
pub fn bootstrap(main: &Main, mat: &Material) -> LoweredMaterial {
    let mut lm = defaults(mat);
    let tree = &mat.node_tree;
    // The Principled BSDF feeding the (first) Material Output's Surface, else the first one.
    let out = tree
        .nodes
        .iter()
        .find(|n| n.bl_idname == "ShaderNodeOutputMaterial");
    let bsdf = out
        .and_then(|o| link_into(tree, o, "Surface"))
        .map(|(n, _)| n)
        .filter(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
        .or_else(|| {
            tree.nodes
                .iter()
                .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
        });
    let Some(bsdf) = bsdf else {
        return lm;
    };
    let mut base = color(bsdf, "Base Color", [0.8, 0.8, 0.8, 1.0]);
    let alpha = float(bsdf, "Alpha", 1.0);
    let base_tex = link_into(tree, bsdf, "Base Color")
        .filter(|(n, s)| n.bl_idname == "ShaderNodeTexImage" && *s == "Color")
        .map(|(n, _)| n);
    let alpha_tex = link_into(tree, bsdf, "Alpha")
        .filter(|(n, s)| n.bl_idname == "ShaderNodeTexImage" && *s == "Alpha")
        .map(|(n, _)| n);
    if let Some(t) = base_tex {
        let with_alpha = alpha_tex.is_some_and(|a| a.uid == t.uid);
        lm.base_color.texture = tex_ref(
            main,
            t,
            if with_alpha {
                ChannelSel::Rgba
            } else {
                ChannelSel::Rgb
            },
        );
        base = [1.0, 1.0, 1.0, base[3]];
        if with_alpha {
            lm.alpha_mode = AlphaMode::Blend;
        }
    }
    lm.base_color.factor = [base[0], base[1], base[2], alpha];
    if base_tex.is_none() || alpha_tex.is_none() {
        lm.alpha_mode = if alpha >= 1.0 {
            AlphaMode::Opaque
        } else if alpha == 0.0 {
            AlphaMode::Mask { cutoff: 0.5 }
        } else {
            AlphaMode::Blend
        };
    }
    lm.metallic.factor = float(bsdf, "Metallic", 0.0);
    lm.roughness.factor = float(bsdf, "Roughness", 0.5);
    let mut e = color(bsdf, "Emission Color", [1.0; 4]);
    // An Image Texture's Color into Emission Color: `emissiveTexture`, factor white
    // (emission.py `export_emission_factor`: a linked socket's value is not read).
    if let Some((t, _)) = link_into(tree, bsdf, "Emission Color")
        .filter(|(n, s)| n.bl_idname == "ShaderNodeTexImage" && *s == "Color")
    {
        lm.emissive.texture = tex_ref(main, t, ChannelSel::Rgb);
        e = [1.0; 4];
    }
    lm.emissive.factor = [e[0], e[1], e[2]];
    lm.emissive_strength = float(bsdf, "Emission Strength", 0.0);
    lm
}
