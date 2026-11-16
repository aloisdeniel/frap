// SPDX-License-Identifier: GPL-3.0-or-later
//! Graph building shared by the pattern raisers (`raise` side). Port of the parts of
//! `io_scene_gltf2/blender/imp/{pbrMetallicRoughness,material_utils,texture}.py`
//! (glTF-Blender-IO 5.2.41, Apache-2.0) that M1 needs: the base Principled BSDF → Material
//! Output graph, `calc_locations`, `color_factor_and_texture` and `texture` (Image Texture,
//! sampler, mixed-wrap UV chain, Mapping node for `KHR_texture_transform`, UV Map node).

use std::collections::BTreeMap;

use frap_nodes::{Node, SocketAddr, SocketValue};
use frap_rna::Value;
use smol_str::SmolStr;

use crate::graph::gltf_to_blender_transform;
use crate::{Filter, LoweredMaterial, RaiseCx, TexRef, UvSource, Wrap};

/// Height of one block of the importer's layout (`calc_locations`).
const BLOCK_HEIGHT: f32 = 460.0;

/// `calc_locations`: the upper-right corner of each block of the imported graph, laid out
/// top to bottom (a block with a texture takes 460 units) and centred vertically.
pub fn calc_locations(lm: &LoweredMaterial) -> BTreeMap<&'static str, [f32; 2]> {
    let x = -200.0;
    let mut y = 0.0;
    let mut locs = BTreeMap::new();
    let blocks: [(&'static str, bool); 5] = [
        (
            "base_color",
            lm.base_color.texture.is_some() || lm.vertex_color.is_some(),
        ),
        (
            "metallic_roughness",
            lm.metallic.texture.is_some() || lm.roughness.texture.is_some(),
        ),
        ("normal", lm.normal.is_some()),
        ("emission", lm.emissive.texture.is_some()),
        ("occlusion", lm.occlusion.is_some()),
    ];
    for (key, takes_space) in blocks {
        locs.insert(key, [x, y]);
        if takes_space {
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
        self.set_prop(uid, prop, Value::Enum(SmolStr::new(item)));
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
        let mut uv_socket = self.input(img, "Vector");
        x -= 340.0;

        let (ws, wt) = (tex.sampler.wrap_s, tex.sampler.wrap_t);
        let mut needs_uv_map = false;
        if ws == wt {
            let ext = match ws {
                Wrap::Clamp => "EXTEND",
                Wrap::Mirror => "MIRROR",
                _ => "REPEAT",
            };
            self.set_enum(img, "extension", ext);
        } else {
            // Mixed wrap modes: per-axis WRAP / PINGPONG math on the UVs.
            self.set_enum(img, "extension", "EXTEND");
            let com = self.add_node("ShaderNodeCombineXYZ", [x - 140.0, y - 100.0]);
            let o = self.output(com, "Vector");
            self.link(uv_socket, o);
            let mut axis = [self.input(com, "X"), self.input(com, "Y")];
            x -= 200.0;
            for (i, wrap) in [ws, wt].into_iter().enumerate() {
                let op = match wrap {
                    Wrap::Repeat => Some("WRAP"),
                    Wrap::Mirror => Some("PINGPONG"),
                    _ => None,
                };
                if let Some(op) = op {
                    let m =
                        self.add_node("ShaderNodeMath", [x - 140.0, y + 30.0 - i as f32 * 200.0]);
                    self.set_enum(m, "operation", op);
                    if op == "WRAP" {
                        self.set_input_at(m, 1, SocketValue::Float(0.0));
                        self.set_input_at(m, 2, SocketValue::Float(1.0));
                    } else {
                        self.set_input_at(m, 1, SocketValue::Float(1.0));
                    }
                    self.link(axis[i], SocketAddr::output(m, 0));
                    axis[i] = SocketAddr::input(m, 0);
                }
            }
            x -= 200.0;
            let sep = self.add_node("ShaderNodeSeparateXYZ", [x - 140.0, y - 100.0]);
            let (ox, oy) = (self.output(sep, "X"), self.output(sep, "Y"));
            self.link(axis[0], ox);
            self.link(axis[1], oy);
            uv_socket = self.input(sep, "Vector");
            x -= 200.0;
            needs_uv_map = true;
        }

        if let Some(t) = &tex.transform {
            let mapping = self.add_node("ShaderNodeMapping", [x - 160.0, y + 30.0]);
            self.set_enum(mapping, "vector_type", "POINT");
            let o = self.output(mapping, "Vector");
            self.link(uv_socket, o);
            uv_socket = self.input(mapping, "Vector");
            let (loc, rot, scale) = gltf_to_blender_transform(t);
            self.set_input(
                mapping,
                "Location",
                SocketValue::Vector([loc[0], loc[1], 0.0, 0.0]),
            );
            self.set_input(
                mapping,
                "Rotation",
                SocketValue::Vector([0.0, 0.0, rot, 0.0]),
            );
            self.set_input(
                mapping,
                "Scale",
                SocketValue::Vector([scale[0], scale[1], 1.0, 0.0]),
            );
            x -= 260.0;
            needs_uv_map = true;
        }

        let uv_name: Option<SmolStr> = match &tex.uv {
            UvSource::Named(n) => Some(n.clone()),
            UvSource::Index(0) => needs_uv_map.then(|| "UVMap".into()),
            UvSource::Index(i) => Some(format!("UVMap.{i:03}").into()),
            _ => needs_uv_map.then(SmolStr::default),
        };
        if let Some(name) = uv_name {
            let uv = self.add_node("ShaderNodeUVMap", [x - 160.0, y - 70.0]);
            self.set_prop(uv, "uv_map", Value::Str(name));
            let o = self.output(uv, "UV");
            self.link(uv_socket, o);
        }
        img
    }
}
