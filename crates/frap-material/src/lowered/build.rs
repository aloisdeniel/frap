// SPDX-License-Identifier: GPL-3.0-or-later
//! Graph building shared by the pattern raisers (`raise` side). Port of the parts of
//! `io_scene_gltf2/blender/imp/{pbrMetallicRoughness,material_utils,texture}.py`
//! (glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors): the base Principled
//! BSDF → Material Output graph, `calc_locations`, `color_factor_and_texture`,
//! `scalar_factor_and_texture`, `normal_map`, `make_settings_node` and `texture` (Image
//! Texture; the sampler, Mapping and UV Map parts live in the per-texture patterns
//! `sampler.rs`, `texture_transform.rs` and `uv_source.rs`).

use std::collections::BTreeMap;

use frap_nodes::{Node, SocketAddr, SocketValue};
use frap_rna::Value;

use crate::{ChannelSel, Filter, LoweredMaterial, NormalMap, RaiseCx, TexRef, UvSource};

/// Height of one block of the importer's layout (`calc_locations`).
const BLOCK_HEIGHT: f32 = 460.0;

/// One block of the importer's layout (`calc_locations`), registered by the pattern file that
/// raises the block's nodes (one `LAYOUT_BLOCKS` entry next to its `MATERIAL_PATTERNS` entry),
/// so later patterns add their block without editing [`calc_locations`] (§16.2, §22.1).
#[derive(Debug)]
pub struct LayoutBlock {
    /// Position in the importer's top-to-bottom list. The importer's keys, in order:
    /// `base_color` 10, `metallic_roughness` 20, `transmission` 30, `normal` 40,
    /// `specularTexture` 50, `specularColorTexture` 60, `anisotropy` 70,
    /// `sheenRoughnessTexture` 80, `sheenColorTexture` 90, `clearcoat` 100,
    /// `clearcoat_roughness` 110, `clearcoat_normal` 120, `emission` 130, `iridescence` 140,
    /// `occlusion` 150, `volume_thickness` 160.
    pub order: u16,
    /// The block's key ([`RaiseCx::location`]).
    pub key: &'static str,
    /// Whether the block holds a texture and so takes `BLOCK_HEIGHT` (460) units.
    pub takes_space: fn(&LoweredMaterial) -> bool,
}

/// Every layout block, one per pattern that lays out nodes.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static LAYOUT_BLOCKS: [LayoutBlock];

/// `calc_locations`: the upper-right corner of each registered [`LAYOUT_BLOCKS`] block of the
/// imported graph, laid out top to bottom in `order` (a block with a texture takes 460 units)
/// and centred vertically.
pub fn calc_locations(lm: &LoweredMaterial) -> BTreeMap<&'static str, [f32; 2]> {
    let x = -200.0;
    let mut y = 0.0;
    let mut locs = BTreeMap::new();
    let mut blocks: Vec<&LayoutBlock> = LAYOUT_BLOCKS.iter().collect();
    blocks.sort_by(|a, b| a.order.cmp(&b.order).then(a.key.cmp(b.key)));
    for b in blocks {
        locs.insert(b.key, [x, y]);
        if (b.takes_space)(lm) {
            y -= BLOCK_HEIGHT;
        }
    }
    let y_offset = -y / 2.0 - 20.0;
    for v in locs.values_mut() {
        v[1] += y_offset;
    }
    locs
}

impl RaiseCx<'_> {
    /// The node with this uid.
    pub fn node_mut(&mut self, uid: u32) -> &mut Node {
        self.into
            .node_tree
            .node_mut(uid)
            .expect("invariant: raise only addresses nodes it created")
    }

    /// Adds a node of type `bl_idname` at `location` (name made unique against the tree and
    /// `names`, which records it) and returns its uid.
    pub fn add_node(&mut self, bl_idname: &str, location: [f32; 2]) -> u32 {
        let def = frap_nodes::generated::node_type(bl_idname)
            .expect("invariant: raise only creates generated node types");
        let taken = &self.names.taken;
        let i = self
            .into
            .node_tree
            .add_node_with(def, &|n| taken.contains(n));
        let n = &mut self.into.node_tree.nodes[i];
        n.location = location;
        self.names.taken.insert(n.name.clone());
        n.uid
    }

    /// Sets a node's label.
    pub fn set_label(&mut self, uid: u32, label: &str) {
        self.node_mut(uid).label = label.to_owned();
    }

    /// Writes a node property (`operation`, `data_type`, …) and updates socket availability.
    pub fn set_prop(&mut self, uid: u32, prop: &str, v: Value) {
        let ok = self.node_mut(uid).set_prop(prop, v);
        debug_assert!(ok, "node has no property {prop}");
    }

    /// Writes an enum node property.
    pub fn set_enum(&mut self, uid: u32, prop: &str, item: &str) {
        self.set_prop(uid, prop, Value::Enum(smol_str::SmolStr::new(item)));
    }

    /// Writes `node.inputs[index].default_value`.
    pub fn set_input_at(&mut self, uid: u32, index: usize, v: SocketValue) {
        self.node_mut(uid).inputs[index].default_value = v;
    }

    /// Writes `node.inputs['name'].default_value` (first enabled socket).
    pub fn set_input(&mut self, uid: u32, name: &str, v: SocketValue) {
        let n = self.node_mut(uid);
        let i = n
            .input_index(name)
            .unwrap_or_else(|| panic!("invariant: {} has an input {name}", n.bl_idname));
        n.inputs[i].default_value = v;
    }

    /// Input address of `node.inputs['name']`.
    pub fn input(&mut self, uid: u32, name: &str) -> SocketAddr {
        let n = self.node_mut(uid);
        let i = n
            .input_index(name)
            .unwrap_or_else(|| panic!("invariant: {} has an input {name}", n.bl_idname));
        SocketAddr::input(uid, i)
    }

    /// Output address of `node.outputs['name']`.
    pub fn output(&mut self, uid: u32, name: &str) -> SocketAddr {
        let n = self.node_mut(uid);
        let i = n
            .output_index(name)
            .unwrap_or_else(|| panic!("invariant: {} has an output {name}", n.bl_idname));
        SocketAddr::output(uid, i)
    }

    /// `links.new(a, b)` (either order).
    pub fn link(&mut self, a: SocketAddr, b: SocketAddr) {
        self.into
            .node_tree
            .link_new(a, b, true)
            .expect("invariant: raise links an output to an input");
    }

    /// The base graph of `pbr_metallic_roughness`: clears the tree, adds `Principled BSDF`
    /// at (10, 300) and `Material Output` at (300, 300), links BSDF → Surface and records
    /// both in [`RaiseCx::bsdf`] / [`RaiseCx::output`].
    pub fn reset_graph(&mut self) {
        let t = &mut self.into.node_tree;
        t.nodes.clear();
        t.links.clear();
        self.names.taken.clear();
        self.base_alpha_socket = None;
        let bsdf = self.add_node("ShaderNodeBsdfPrincipled", [10.0, 300.0]);
        let out = self.add_node("ShaderNodeOutputMaterial", [300.0, 300.0]);
        let (a, b) = (self.output(bsdf, "BSDF"), self.input(out, "Surface"));
        self.link(a, b);
        self.bsdf = Some(bsdf);
        self.output = Some(out);
    }

    /// The location of a layout block (`calc_locations`), (−200, 0) if unknown.
    pub fn location(&self, block: &str) -> [f32; 2] {
        self.locations.get(block).copied().unwrap_or([-200.0, 0.0])
    }

    /// `color_factor_and_texture`: a constant colour, or `texture × factor` through a Mix
    /// (RGBA, MULTIPLY) labelled `"<label> Factor"` when the factor is not white.
    pub fn color_factor_and_texture(
        &mut self,
        location: [f32; 2],
        label: &str,
        socket: SocketAddr,
        factor: [f32; 3],
        tex: Option<&TexRef>,
    ) -> Option<u32> {
        let [mut x, y] = location;
        let Some(tex) = tex else {
            self.set_input_at(
                socket.node,
                socket.index,
                SocketValue::Color([factor[0], factor[1], factor[2], 1.0]),
            );
            return None;
        };
        let mut socket = socket;
        if factor != [1.0, 1.0, 1.0] {
            let mix = self.add_node("ShaderNodeMix", [x - 140.0, y]);
            self.set_enum(mix, "data_type", "RGBA");
            self.set_label(mix, &format!("{label} Factor"));
            self.set_enum(mix, "blend_type", "MULTIPLY");
            let out = self.output(mix, "Result");
            self.link(socket, out);
            self.set_input(mix, "Factor", SocketValue::Float(1.0));
            socket = self.input(mix, "A");
            self.set_input(
                mix,
                "B",
                SocketValue::Color([factor[0], factor[1], factor[2], 1.0]),
            );
            x -= 200.0;
        }
        Some(self.texture(tex, [x, y], &label.to_uppercase(), Some(socket), None))
    }

    /// `scalar_factor_and_texture`: a constant, or `texture channel × factor` through a Math
    /// MULTIPLY labelled `"<label> Factor"` when the factor is not 1, and a Separate Color
    /// for an R/G/B channel (channel `A` reads the texture's `Alpha` output). Returns the
    /// Image Texture uid.
    pub fn scalar_factor_and_texture(
        &mut self,
        location: [f32; 2],
        label: &str,
        socket: SocketAddr,
        factor: f32,
        tex: Option<&TexRef>,
    ) -> Option<u32> {
        let [mut x, y] = location;
        let Some(tex) = tex else {
            self.set_input_at(socket.node, socket.index, SocketValue::Float(factor));
            return None;
        };
        let mut socket = socket;
        if factor != 1.0 {
            let m = self.add_node("ShaderNodeMath", [x - 140.0, y]);
            self.set_label(m, &format!("{label} Factor"));
            self.set_enum(m, "operation", "MULTIPLY");
            self.link(socket, SocketAddr::output(m, 0));
            socket = SocketAddr::input(m, 0);
            self.set_input_at(m, 1, SocketValue::Float(factor));
            x -= 200.0;
        }
        let channel_out = match tex.channel {
            ChannelSel::R => Some(0),
            ChannelSel::G => Some(1),
            ChannelSel::B => Some(2),
            _ => None,
        };
        if let Some(c) = channel_out {
            let sep = self.add_node("ShaderNodeSeparateColor", [x - 150.0, y - 75.0]);
            self.link(socket, SocketAddr::output(sep, c));
            socket = self.input(sep, "Color");
            x -= 200.0;
        }
        let (color_to, alpha_to) = if tex.channel == ChannelSel::A {
            (None, Some(socket))
        } else {
            (Some(socket), None)
        };
        Some(self.texture(tex, [x, y], &label.to_uppercase(), color_to, alpha_to))
    }

    /// `normal_map`: a Normal Map node (`uv_map` named after the texture's `TEXCOORD_n`,
    /// `Strength` = scale) at `(x − 150, y − 40)` linked into `socket`, and the texture
    /// labelled `label` upper-cased. Returns the Normal Map uid.
    pub fn normal_map(
        &mut self,
        location: [f32; 2],
        label: &str,
        socket: SocketAddr,
        n: &NormalMap,
    ) -> u32 {
        let [x, y] = location;
        let node = self.add_node("ShaderNodeNormalMap", [x - 150.0, y - 40.0]);
        let uv = match &n.texture.uv {
            UvSource::Index(i) if *i > 0 => format!("UVMap.{i:03}"),
            UvSource::Named(name) => name.to_string(),
            _ => "UVMap".to_owned(),
        };
        self.set_prop(node, "uv_map", Value::Str(uv.into()));
        self.set_input(node, "Strength", SocketValue::Float(n.scale));
        let o = self.output(node, "Normal");
        self.link(socket, o);
        let color = self.input(node, "Color");
        self.texture(
            &n.texture,
            [x - 200.0, y],
            &label.to_uppercase(),
            Some(color),
            None,
        );
        node
    }

    /// `make_settings_node`: the "glTF Material Output" group node (named `Group`, at
    /// (40, −370), width 180) that occlusion, volume thickness, dispersion and iridescence
    /// link into, created on first use. It gets the group's five inputs; the importer sets
    /// its `node_tree` to the `bpy.data.node_groups` entry afterwards (`raise` cannot reach
    /// `bpy.data`). Returns its uid.
    pub fn settings_node(&mut self) -> u32 {
        if let Some(n) = self
            .into
            .node_tree
            .nodes
            .iter()
            .find(|n| frap_nodes::is_group_node(n.bl_idname) && n.input("Occlusion").is_some())
        {
            return n.uid;
        }
        let group = frap_nodes::builtin_groups::gltf_material_output::new_group();
        let uid = self.add_node("ShaderNodeGroup", [40.0, -370.0]);
        let n = self.node_mut(uid);
        n.width = 180.0;
        n.sync_group_sockets(Some(&group.interface));
        uid
    }

    /// `texture`: an Image Texture node (label, image, interpolation, extension) at
    /// `location` linked into `color_to` / `alpha_to`, plus the UV chain the texture needs
    /// (mixed wrap modes, Mapping node, UV Map node). Returns the Image Texture uid.
    pub fn texture(
        &mut self,
        tex: &TexRef,
        location: [f32; 2],
        label: &str,
        color_to: Option<SocketAddr>,
        alpha_to: Option<SocketAddr>,
    ) -> u32 {
        let [mut x, y] = location;
        let img = self.add_node("ShaderNodeTexImage", [x - 240.0, y]);
        self.set_label(img, label);
        self.set_prop(img, "image", Value::Id(Some(tex.image.any())));
        let interpolation = match tex.sampler.filter {
            Filter::Closest => "Closest",
            Filter::Cubic => "Cubic",
            _ => "Linear",
        };
        self.set_enum(img, "interpolation", interpolation);
        if let Some(to) = color_to {
            let o = self.output(img, "Color");
            self.link(to, o);
        }
        if let Some(to) = alpha_to {
            let o = self.output(img, "Alpha");
            self.link(to, o);
        }
        let uv_socket = self.input(img, "Vector");
        x -= 340.0;
        let (uv_socket, x, wrap_needs_uv) =
            crate::patterns::sampler::raise_wrapping(self, tex, img, uv_socket, x, y);
        let (uv_socket, x, needs_uv_map) = match &tex.transform {
            Some(t) => {
                let (s, x) =
                    crate::patterns::texture_transform::raise_mapping(self, t, uv_socket, x, y);
                (s, x, true)
            }
            None => (uv_socket, x, wrap_needs_uv),
        };
        crate::patterns::uv_source::raise_uv_map(self, &tex.uv, needs_uv_map, uv_socket, x, y);
        img
    }
}
