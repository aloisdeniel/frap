// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the `KHR_materials_*` patterns of WP 3.6 (`clearcoat`, `sheen`,
//! `transmission`, `volume`, `ior`, `specular`, `iridescence`, `anisotropy`, `dispersion`). They
//! live here, as a submodule of a pattern file, because the shared helper files
//! (`lowered/{build,graph}.rs`) belong to WP 3.5 (decision log M34W0-3).
//!
//! Lowering side, ported from `io_scene_gltf2/blender/exp/material/search_node_tree.py`
//! (glTF-Blender-IO 5.2.41, Apache-2.0):
//! - `NodeNav.get_factor` ([`factor_f32`], [`factor_rgb`]): a constant (unlinked socket,
//!   Value or RGB node, or a link the inliner constant-folds, `LowerCx::folded_output`),
//!   else the one constant input of a Math MULTIPLY (values) or of a Mix RGBA MULTIPLY
//!   whatever its Factor (colours);
//! - `get_texture_node_from_socket` ([`image_from_socket`]): the first Image Texture found by
//!   `from_socket`'s depth-first walk through every input of every node behind the socket
//!   (`None` when that node has no image), with the channel `image.py`'s
//!   `__get_image_data_mapping` reads along the path (a Separate Color output, an `Alpha`
//!   output; the one closest to the image wins);
//! - `get_socket_from_gltf_material_node` ([`settings_input`]): the first input with that name
//!   on a non-muted group node (the add-on's name test is always true, see
//!   `frap_nodes::builtin_groups::gltf_material_output::is_gltf_material_output`);
//! - `check_if_is_linked_to_active_output` ([`linked_to_active_output`]).
//!
//! Raising side, ported from `io_scene_gltf2/blender/imp/{material_utils,pbrMetallicRoughness}.py`:
//! [`scalar_factor_and_texture`], [`normal_map`], [`settings_node`] (`make_settings_node`) and
//! [`volume_node`].
//!
//! **Texture channels.** A [`TexRef::channel`] of an extension texture names the channel of the
//! Blender image that a glTF reader ends up reading: for the sockets the add-on packs into a
//! new image (`Coat Weight` → R, `Coat Roughness` → G, `Thickness` → G, `Specular IOR Level`
//! → A, `Sheen Roughness` → A; [`TexKind::Packed`]) it is the source channel (`image.py`
//! fills the glTF channel from it), for the others the add-on copies the whole image
//! ([`TexKind::Full`]) and it is the channel the glTF member reads.

use std::collections::BTreeSet;

use frap_nodes::builtin_groups::gltf_material_output;
use frap_nodes::{Node, SocketAddr, SocketType, SocketValue};
use frap_rna::Report;

use crate::graph::{Src, unrecognised};
use crate::{ChannelSel, LowerCx, NormalMap, RaiseCx, TexRef, UvSource};

/// The constant the add-on reads from an input of the inlined tree: the unlinked socket's
/// value, or the value the inliner constant-folds the link into
/// ([`LowerCx::folded_output`]). `None` when the input stays linked.
pub(crate) fn konst_f32<'a>(cx: &LowerCx<'a>, src: Src<'a>) -> Option<f32> {
    match src {
        Src::Const(SocketValue::Float(f)) => Some(*f),
        Src::Linked(n, o) => cx.folded_output(n, o, SocketType::Value).map(|v| v.f()),
        _ => None,
    }
}

/// [`konst_f32`] for colour sockets (RGB).
pub(crate) fn konst_rgb<'a>(cx: &LowerCx<'a>, src: Src<'a>) -> Option<[f32; 3]> {
    match src {
        Src::Const(SocketValue::Color(c)) => Some([c[0], c[1], c[2]]),
        Src::Linked(n, o) => cx.folded_output(n, o, SocketType::Rgba).map(|v| v.v()),
        _ => None,
    }
}

/// `NodeNav.get_factor` for a VALUE socket.
pub(crate) fn factor_f32<'a>(cx: &LowerCx<'a>, src: Src<'a>) -> Option<f32> {
    if let Some(c) = cx.const_f32(src) {
        return Some(c);
    }
    // The add-on reads the inlined tree, where a constant-folded link is a plain value.
    if let Src::Linked(n, o) = src
        && let Some(v) = cx.folded_output(n, o, SocketType::Value)
    {
        return Some(v.f());
    }
    let (a, b) = cx.value_multiply(src)?;
    match (cx.const_f32(a), cx.const_f32(b)) {
        (Some(x), None) | (None, Some(x)) => Some(x),
        _ => None,
    }
}

/// `NodeNav.get_factor` for an RGBA socket (`# TODO: check factor is 1?` in the add-on: the
/// Mix node's Factor is not checked).
pub(crate) fn factor_rgb<'a>(cx: &LowerCx<'a>, src: Src<'a>) -> Option<[f32; 3]> {
    if let Some(c) = cx.const_rgb(src) {
        return Some(c);
    }
    let Src::Linked(n, o) = src else { return None };
    if let Some(v) = cx.folded_output(n, o, SocketType::Rgba) {
        return Some(v.v());
    }
    if !(n.bl_idname == "ShaderNodeMix"
        && n.prop_str("data_type") == "RGBA"
        && n.prop_str("blend_type") == "MULTIPLY")
    {
        return None;
    }
    let at = |id: &str| n.inputs.iter().position(|s| s.identifier == id);
    let (ia, ib) = (at("A_Color")?, at("B_Color")?);
    match (
        cx.const_rgb(cx.src_at(n, ia)),
        cx.const_rgb(cx.src_at(n, ib)),
    ) {
        (Some(x), None) | (None, Some(x)) => Some(x),
        _ => None,
    }
}

/// An Image Texture found behind a socket and the channel read on the way.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Found<'a> {
    /// The Image Texture node (its image is set).
    pub node: &'a Node,
    /// The channel the path selects (Separate Color output, `Alpha` output), if any.
    pub channel: Option<ChannelSel>,
}

/// `get_texture_node_from_socket(node.inputs[input])`.
pub(crate) fn image_from_socket<'a>(
    cx: &LowerCx<'a>,
    node: &'a Node,
    input: usize,
) -> Option<Found<'a>> {
    let mut on_path = BTreeSet::new();
    let found = search(cx, node, input, None, &mut on_path)?;
    cx.image_of(found.node).map(|_| found)
}

fn search<'a>(
    cx: &LowerCx<'a>,
    node: &'a Node,
    input: usize,
    channel: Option<ChannelSel>,
    on_path: &mut BTreeSet<u32>,
) -> Option<Found<'a>> {
    let s = node.inputs.get(input)?;
    let l = cx.tree.link_into(node.uid, &s.identifier)?;
    let from = cx.tree.node(l.from_node)?;
    let out = from
        .output_by_identifier(&l.from_socket)
        .and_then(|i| from.outputs.get(i))?;
    let channel = match (from.bl_idname, out.name.as_str()) {
        ("ShaderNodeSeparateColor", "Red") => Some(ChannelSel::R),
        ("ShaderNodeSeparateColor", "Green") => Some(ChannelSel::G),
        ("ShaderNodeSeparateColor", "Blue") => Some(ChannelSel::B),
        _ if out.identifier == "Alpha" => Some(ChannelSel::A),
        _ => channel,
    };
    if from.bl_idname == "ShaderNodeTexImage" {
        return Some(Found {
            node: from,
            channel,
        });
    }
    // A link cycle cannot be built through the API; the guard keeps a corrupt file finite.
    if !on_path.insert(from.uid) {
        return None;
    }
    let r = (0..from.inputs.len()).find_map(|i| search(cx, from, i, channel, on_path));
    on_path.remove(&from.uid);
    r
}

/// How the add-on exports an extension texture (see the module docs).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TexKind {
    /// Packed into one channel of a new image; the default source channel when the path names
    /// none.
    Packed(ChannelSel),
    /// The whole image copied; the channel the glTF member reads.
    Full(ChannelSel),
}

/// The texture behind input `input` of `node` (`has_image_node_from_socket` +
/// `gather_texture_info`), `None` without an Image Texture with an image.
pub(crate) fn texture_behind<'a>(
    cx: &LowerCx<'a>,
    node: &'a Node,
    input: usize,
    kind: TexKind,
    diags: &mut Vec<Report>,
) -> Option<TexRef> {
    let found = image_from_socket(cx, node, input)?;
    let channel = match kind {
        TexKind::Packed(default) => found.channel.unwrap_or(default),
        TexKind::Full(c) => c,
    };
    cx.tex_ref(found.node, channel, diags)
}

/// A scalar member as the add-on writes it from an unlinked socket or a texture: the factor
/// (`None`: no member) and the texture.
pub(crate) struct Scalar {
    /// Socket value, or the texture's factor (1 without a constant multiplier).
    pub factor: Option<f32>,
    /// The texture.
    pub texture: Option<TexRef>,
}

/// The common shape of `clearcoat.py`, `transmission.py` and `volume.py`: an unlinked socket
/// gives its value; a socket with an image behind it gives the texture and its factor
/// (`get_factor`, 1 when there is none); anything else gives nothing (with a WARNING naming
/// `what`).
pub(crate) fn scalar_member<'a>(
    cx: &LowerCx<'a>,
    node: &'a Node,
    input: usize,
    kind: TexKind,
    what: &str,
    diags: &mut Vec<Report>,
) -> Scalar {
    let src = cx.src_at(node, input);
    if let Src::Const(v) = src {
        let factor = match v {
            SocketValue::Float(f) => Some(*f),
            _ => None,
        };
        return Scalar {
            factor,
            texture: None,
        };
    }
    if let Some(f) = konst_f32(cx, src) {
        return Scalar {
            factor: Some(f),
            texture: None,
        };
    }
    if let Some(texture) = texture_behind(cx, node, input, kind, diags) {
        return Scalar {
            factor: Some(factor_f32(cx, src).unwrap_or(1.0)),
            texture: Some(texture),
        };
    }
    if let Src::Linked(n, _) = src {
        unrecognised(
            diags,
            format!(
                "{what}: unsupported node setup at \"{}\" ({}); not exported",
                n.name, n.bl_idname
            ),
        );
    }
    Scalar {
        factor: None,
        texture: None,
    }
}

/// `get_socket_from_gltf_material_node(name)`: the first input called `name` of a non-muted
/// group node (one whose `node_tree` is set, or the unbound settings node a raise created
/// before the importer binds it).
pub(crate) fn settings_input<'a>(cx: &LowerCx<'a>, name: &str) -> Option<(&'a Node, usize)> {
    cx.tree
        .nodes
        .iter()
        .filter(|n| n.bl_idname == "ShaderNodeGroup" && !n.mute)
        .find_map(|n| n.input_index(name).map(|i| (n, i)))
}

/// `check_if_is_linked_to_active_output(node.outputs[0])`: whether the first output of `node`
/// reaches the active Material Output, through the first output of every node on the way.
pub(crate) fn linked_to_active_output(cx: &LowerCx, node: &Node) -> bool {
    let Some(active) = cx.material_output().map(|n| n.uid) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.uid) {
            continue;
        }
        let Some(out) = n.outputs.first() else {
            continue;
        };
        for l in &cx.tree.links {
            if l.from_node != n.uid || l.from_socket != out.identifier {
                continue;
            }
            if l.to_node == active {
                return true;
            }
            if let Some(to) = cx.tree.node(l.to_node)
                && to.bl_idname != "ShaderNodeOutputMaterial"
            {
                stack.push(to);
            }
        }
    }
    false
}

/// `get_node_socket(tree, type, …)`'s node: the first non-muted node of type `bl_idname`
/// linked to the active output.
pub(crate) fn node_linked_to_output<'a>(cx: &LowerCx<'a>, bl_idname: &str) -> Option<&'a Node> {
    cx.tree
        .nodes
        .iter()
        .find(|n| n.bl_idname == bl_idname && !n.mute && linked_to_active_output(cx, n))
}

/// The value of an unlinked or linked socket (`get_const_from_default_value_socket`: the
/// socket's own `default_value`, whatever is linked).
pub(crate) fn default_f32(node: &Node, input: usize) -> Option<f32> {
    match node.inputs.get(input)?.default_value {
        SocketValue::Float(f) => Some(f),
        _ => None,
    }
}

/// [`default_f32`] for colour sockets (RGB).
pub(crate) fn default_rgb(node: &Node, input: usize) -> Option<[f32; 3]> {
    match node.inputs.get(input)?.default_value {
        SocketValue::Color(c) => Some([c[0], c[1], c[2]]),
        _ => None,
    }
}

// ------------------------------------------------------------------------------- raise side

/// `scalar_factor_and_texture`: a constant, or texture → Separate Color (`channel` 0–2) or
/// the texture's Alpha (`channel` 4) → "<label> Factor" Math MULTIPLY (when the factor is not
/// 1) → `socket`.
pub(crate) fn scalar_factor_and_texture(
    cx: &mut RaiseCx,
    location: [f32; 2],
    label: &str,
    socket: SocketAddr,
    factor: f32,
    tex: Option<&TexRef>,
    channel: usize,
) {
    let [mut x, y] = location;
    let Some(tex) = tex else {
        cx.set_input_at(socket.node, socket.index, SocketValue::Float(factor));
        return;
    };
    let mut socket = socket;
    if factor != 1.0 {
        let m = cx.add_node("ShaderNodeMath", [x - 140.0, y]);
        cx.set_label(m, &format!("{label} Factor"));
        cx.set_enum(m, "operation", "MULTIPLY");
        cx.link(socket, SocketAddr::output(m, 0));
        socket = SocketAddr::input(m, 0);
        cx.set_input_at(m, 1, SocketValue::Float(factor));
        x -= 200.0;
    }
    let (color_to, alpha_to) = if channel != 4 {
        let sep = cx.add_node("ShaderNodeSeparateColor", [x - 150.0, y - 75.0]);
        cx.link(socket, SocketAddr::output(sep, channel));
        x -= 200.0;
        (Some(SocketAddr::input(sep, 0)), None)
    } else {
        (None, Some(socket))
    };
    cx.texture(tex, [x, y], &label.to_uppercase(), color_to, alpha_to);
}

/// The importer's UV map name for `TEXCOORD_n` (`'UVMap'`, `'UVMap.%03d'`).
pub(crate) fn uv_map_name(uv: &UvSource) -> String {
    match uv {
        UvSource::Named(n) => n.to_string(),
        UvSource::Index(i) if *i > 0 => format!("UVMap.{i:03}"),
        _ => "UVMap".to_owned(),
    }
}

/// `normal_map`: a Normal Map node (UV map, Strength = scale) → `socket`, its Color from the
/// texture labelled `label` in capitals.
pub(crate) fn normal_map(
    cx: &mut RaiseCx,
    location: [f32; 2],
    label: &str,
    socket: SocketAddr,
    nm: &NormalMap,
) {
    let [x, y] = location;
    let n = cx.add_node("ShaderNodeNormalMap", [x - 150.0, y - 40.0]);
    cx.set_prop(
        n,
        "uv_map",
        frap_rna::Value::Str(uv_map_name(&nm.texture.uv).into()),
    );
    cx.set_input(n, "Strength", SocketValue::Float(nm.scale));
    let o = cx.output(n, "Normal");
    cx.link(socket, o);
    let color = cx.input(n, "Color");
    cx.texture(
        &nm.texture,
        [x - 200.0, y],
        &label.to_uppercase(),
        Some(color),
        None,
    );
}

/// The interface of the add-on's "glTF Material Output" group (`create_settings_group`).
fn settings_interface() -> Vec<frap_nodes::InterfaceSocket> {
    gltf_material_output::new_group().interface
}

/// The settings node already in the graph being raised (a group node with the add-on's
/// inputs), if any.
pub(crate) fn find_settings_node(cx: &RaiseCx) -> Option<u32> {
    cx.into
        .node_tree
        .nodes
        .iter()
        .find(|n| {
            n.bl_idname == "ShaderNodeGroup"
                && gltf_material_output::INPUTS.iter().all(|(name, _)| {
                    n.inputs
                        .iter()
                        .any(|s| s.name == *name && s.ty == SocketType::Value)
                })
        })
        .map(|n| n.uid)
}

/// `need_settings_node` of `pbr_metallic_roughness()`: an occlusion texture, a non-zero volume
/// thickness, a non-zero dispersion or a non-zero iridescence factor.
pub(crate) fn needs_settings_node(lm: &crate::LoweredMaterial) -> bool {
    let e = &lm.ext;
    lm.occlusion.is_some()
        || e.volume.as_ref().is_some_and(|v| v.thickness.factor != 0.0)
        || e.dispersion.is_some_and(|d| d != 0.0)
        || e.iridescence
            .as_ref()
            .is_some_and(|i| i.factor.factor != 0.0)
}

/// `make_settings_node`: the material's "glTF Material Output" group node (`Group`, at
/// (40, −370), width 180), unless the graph has one. Its sockets are those of the add-on's
/// group; `node_tree` is bound to the `bpy.data.node_groups` datablock by the importer
/// (`frap_io_gltf::material_ext::bind_settings_nodes`), which `raise` cannot reach.
pub(crate) fn settings_node(cx: &mut RaiseCx) -> u32 {
    if let Some(uid) = find_settings_node(cx) {
        return uid;
    }
    let uid = cx.add_node("ShaderNodeGroup", [40.0, -370.0]);
    let node = cx.node_mut(uid);
    node.width = 180.0;
    node.sync_group_sockets(Some(&settings_interface()));
    uid
}

/// The Volume Absorption node of `pbr_metallic_roughness` (at (40, −520) below the settings
/// node, else (40, −370)), linked into the Material Output's `Volume`.
pub(crate) fn volume_node(cx: &mut RaiseCx, below_settings: bool) -> u32 {
    let y = if below_settings { -520.0 } else { -370.0 };
    let va = cx.add_node("ShaderNodeVolumeAbsorption", [40.0, y]);
    if let Some(out) = cx.output {
        let v = cx.input(out, "Volume");
        let o = SocketAddr::output(va, 0);
        cx.link(v, o);
    }
    va
}
