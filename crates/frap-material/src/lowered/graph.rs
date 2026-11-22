// SPDX-License-Identifier: GPL-3.0-or-later
//! Graph navigation shared by the pattern recognisers (`lower` side). Port of
//! `io_scene_gltf2/blender/exp/material/search_node_tree.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors) on one material node tree:
//!
//! - the active Material Output and `get_node_socket` / `get_socket` (the first non-muted
//!   node of a type linked to it: Principled BSDF, Emission, Background, Volume Absorption);
//! - `get_socket_from_gltf_material_node` (an input of the "glTF Material Output" group node);
//! - `NodeNav`: `move_back` through reroutes, `get_constant` (unlinked sockets, RGB, Value and
//!   unlinked Ambient Occlusion nodes), `get_factor`, `get_multiply_factors`,
//!   `detect_multiply_by_constant`, `get_color_attrib`, `detect_multiply_by_color_attrib`,
//!   `gather_color_info`;
//! - `from_socket(…, FilterByType(ShaderNodeTexImage))` / `get_texture_node_from_socket`: the
//!   depth-first search for the Image Texture behind a socket, with the links walked, which
//!   decide the image channel used (`image.py`, `__get_image_data_mapping`);
//! - the texture reference of an Image Texture: image, colour space, sampler (`sampler.rs`),
//!   texture transform (`texture_transform.rs`) and UV source (`uv_source.rs`).
//!
//! Node groups: frap's lowering sees one tree (`LowerCx` has no access to
//! `bpy.data.node_groups`), so it does not descend into group nodes as the add-on does; a
//! group node only provides the inputs of the "glTF Material Output" group (see
//! [`LowerCx::gltf_output_input`]). Recorded in docs/deviations.md §2.

use frap_nodes::{Node, Socket, SocketType, SocketValue};
use frap_rna::{IdRef, ImageId, Report, ReportType, Value};
use smol_str::SmolStr;

use crate::{ChannelSel, LowerCx, Sampler, TexRef, TexTransform, UvSource};

mod fold;
pub use fold::FoldVal;

/// Where an input socket gets its value from.
#[derive(Copy, Clone, Debug)]
pub enum Src<'t> {
    /// Unlinked: the socket's own default value.
    Const(&'t SocketValue),
    /// Linked from output `.1` (index in `node.outputs`) of node `.0` (reroutes skipped).
    Linked(&'t Node, usize),
    /// The socket does not exist.
    Missing,
}

/// One input socket of a node: the add-on's `NodeSocket` / a `NodeNav` with `in_socket`
/// selected.
#[derive(Copy, Clone, Debug)]
pub struct In<'t> {
    /// The node.
    pub node: &'t Node,
    /// Index in `node.inputs`.
    pub index: usize,
}

impl<'t> In<'t> {
    /// `node.inputs[index]`.
    pub fn socket(&self) -> &'t Socket {
        &self.node.inputs[self.index]
    }

    /// `node.inputs['name']` (first enabled input with that name), if any.
    pub fn named(node: &'t Node, name: &str) -> Option<In<'t>> {
        node.input_index(name).map(|index| In { node, index })
    }

    /// The input with this identifier (`NodeNav.select_input_socket('#A_Color')`).
    pub fn by_identifier(node: &'t Node, identifier: &str) -> Option<In<'t>> {
        node.input_by_identifier(identifier)
            .map(|index| In { node, index })
    }
}

/// A constant read by `get_constant`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Konst {
    /// An RGB colour (alpha dropped).
    Rgb([f32; 3]),
    /// A float.
    Value(f32),
    /// A vector (unlinked VECTOR sockets).
    Vector([f32; 3]),
}

impl Konst {
    /// The float of a VALUE constant.
    pub fn value(self) -> Option<f32> {
        match self {
            Konst::Value(v) => Some(v),
            _ => None,
        }
    }

    /// The colour of an RGB constant.
    pub fn rgb(self) -> Option<[f32; 3]> {
        match self {
            Konst::Rgb(c) => Some(c),
            _ => None,
        }
    }
}

/// `gather_color_info`: `color = factor * color attribute * texture`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorInfo {
    /// `colorFactor` (`None`: none found).
    pub factor: Option<[f32; 3]>,
    /// `colorAttrib` (`Some("")`: the active render colour attribute).
    pub attrib: Option<SmolStr>,
}

/// One link walked by the texture search: the output it starts from.
#[derive(Copy, Clone, Debug)]
pub struct PathLink<'t> {
    /// `link.from_node`
    pub from_node: &'t Node,
    /// `link.from_socket` (index in `from_node.outputs`).
    pub from_output: usize,
}

impl<'t> PathLink<'t> {
    /// `link.from_socket`
    pub fn from_socket(&self) -> &'t Socket {
        &self.from_node.outputs[self.from_output]
    }
}

/// `NodeTreeSearchResult` of the Image Texture search: the node and the links walked from
/// the start socket to it (start side first, the link leaving the Image Texture last).
#[derive(Clone, Debug)]
pub struct TexHit<'t> {
    /// The Image Texture node.
    pub node: &'t Node,
    /// `result.path`
    pub path: Vec<PathLink<'t>>,
}

/// The image channel a texture search reads, as `image.py` (`__get_image_data_mapping`)
/// decides it: `Channel.R/G/B/A`, or `RGB2BW` (luminance of an RGB output plugged into
/// Alpha).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SrcChannel {
    /// A single channel (R, G, B or A).
    One(ChannelSel),
    /// `Channel.RGB2BW`
    Rgb2Bw,
}

/// The state of one depth-first node search ([`LowerCx::texture_from`]).
struct SearchState<'t> {
    /// `is_valid` of each link of the tree, in `links` order.
    valid: Vec<bool>,
    /// Nodes whose whole upstream was searched without a match.
    done: std::collections::BTreeSet<u32>,
    /// Nodes on the current path (the start node included).
    on_path: std::collections::BTreeSet<u32>,
    /// The links walked so far.
    path: Vec<PathLink<'t>>,
}

/// A WARNING diagnostic for a node setup the recognisers do not understand.
pub fn unrecognised(out: &mut Vec<Report>, what: impl Into<String>) {
    let r = Report::new(ReportType::Warning, what.into());
    if !out.contains(&r) {
        out.push(r);
    }
}

impl<'a> LowerCx<'a> {
    /// The active Material Output (`is_active_output`): the first `ShaderNodeOutputMaterial`
    /// in `tree.nodes`, whatever its `target` (verified on 5.2.4: Blender flags the first
    /// output node of the tree, and the next one when it is removed; frap does not store the
    /// flag, so a script cannot move it yet).
    ///
    /// `None` when [`Self::inline_fails`]: the inlined tree the exporter reads is empty.
    pub fn material_output(&self) -> Option<&'a Node> {
        if self.inline_fails() {
            return None;
        }
        self.tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeOutputMaterial")
    }

    /// The node feeding the active output's `Surface` (reroutes skipped), for diagnostics.
    pub fn surface_shader(&self) -> Option<&'a Node> {
        let out = self.material_output()?;
        match self.src(out, "Surface") {
            Src::Linked(n, _) => Some(n),
            _ => None,
        }
    }

    /// `get_node_socket(tree, type, …)`: the first non-muted node of type `bl_idname` in
    /// `tree.nodes` whose first output reaches the active Material Output
    /// (`check_if_is_linked_to_active_output`: directly, or through the first output of every
    /// node on the way, such as a Mix Shader or a reroute).
    pub fn linked_node_of_type(&self, bl_idname: &str) -> Option<&'a Node> {
        let active = self.material_output()?.uid;
        self.tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == bl_idname && !n.mute && self.reaches_output(n, active))
    }

    /// The Principled BSDF every core pattern reads (`get_socket(tree, name)`).
    pub fn principled(&self) -> Option<&'a Node> {
        self.linked_node_of_type("ShaderNodeBsdfPrincipled")
    }

    /// `get_socket(tree, name)`: input `name` of the Principled BSDF; `"Emissive"` is the
    /// `Color` of a dedicated Emission node first, else the Principled `Emission Color`;
    /// `"Background"` the `Color` of a Background node.
    pub fn get_socket(&self, name: &str) -> Option<In<'a>> {
        match name {
            "Emissive" => self
                .linked_node_of_type("ShaderNodeEmission")
                .and_then(|n| In::named(n, "Color"))
                .or_else(|| In::named(self.principled()?, "Emission Color")),
            "Background" => In::named(self.linked_node_of_type("ShaderNodeBackground")?, "Color"),
            _ => In::named(self.principled()?, name),
        }
    }

    /// `get_socket_from_gltf_material_node(tree, name)`: the first input named `name` of a
    /// group node. The add-on means the "glTF Material Output" group (or the legacy
    /// "glTF Settings"), but its name test is always true (see
    /// `frap_nodes::builtin_groups::gltf_material_output::is_gltf_material_output`), so any
    /// group node with a node tree qualifies; a group node without a tree has no inputs.
    /// Muted group nodes are skipped (`get_material_nodes`).
    pub fn gltf_output_input(&self, name: &str) -> Option<In<'a>> {
        if self.inline_fails() {
            return None;
        }
        self.gltf_output_input_raw(name)
    }

    /// [`Self::gltf_output_input`] in the original tree.
    fn gltf_output_input_raw(&self, name: &str) -> Option<In<'a>> {
        self.tree
            .nodes
            .iter()
            .filter(|n| frap_nodes::is_group_node(n.bl_idname) && !n.mute)
            .find_map(|n| {
                n.inputs
                    .iter()
                    .position(|s| s.name == name)
                    .map(|index| In { node: n, index })
            })
    }

    /// `check_if_is_linked_to_active_output(node.outputs[0])`, iteratively, each node visited
    /// once (a cycle cannot reach the output any other way).
    fn reaches_output(&self, from: &'a Node, active: u32) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![from];
        while let Some(n) = stack.pop() {
            if !seen.insert(n.uid) {
                continue;
            }
            let Some(out) = n.outputs.first() else {
                continue;
            };
            for l in &self.tree.links {
                if l.from_node != n.uid || l.from_socket != out.identifier {
                    continue;
                }
                if l.to_node == active {
                    return true;
                }
                if let Some(to) = self.tree.node(l.to_node)
                    && to.bl_idname != "ShaderNodeOutputMaterial"
                {
                    stack.push(to);
                }
            }
        }
        false
    }

    /// The source of input `name` (first enabled socket with that name) of `node`.
    pub fn src(&self, node: &'a Node, name: &str) -> Src<'a> {
        match node.input_index(name) {
            Some(i) => self.src_at(node, i),
            None => Src::Missing,
        }
    }

    /// The source of `node.inputs[i]` (`NodeNav.move_back`: the first link, reroutes skipped;
    /// an unlinked reroute gives its input's value).
    pub fn src_at(&self, node: &'a Node, i: usize) -> Src<'a> {
        let Some(s) = node.inputs.get(i) else {
            return Src::Missing;
        };
        let mut link = self.tree.link_into(node.uid, &s.identifier);
        let mut guard = 0;
        while let Some(l) = link {
            let Some(from) = self.tree.node(l.from_node) else {
                break;
            };
            if from.bl_idname == "NodeReroute" && guard <= self.tree.nodes.len() {
                guard += 1;
                match from.inputs.first() {
                    Some(inp) => match self.tree.link_into(from.uid, &inp.identifier) {
                        Some(next) => {
                            link = Some(next);
                            continue;
                        }
                        None => return Src::Const(&inp.default_value),
                    },
                    None => return Src::Missing,
                }
            }
            return match from.output_by_identifier(&l.from_socket) {
                Some(o) => Src::Linked(from, o),
                None => Src::Missing,
            };
        }
        Src::Const(&s.default_value)
    }

    /// The source of an input socket.
    pub fn src_in(&self, i: In<'a>) -> Src<'a> {
        self.src_at(i.node, i.index)
    }

    /// `NodeNav.peek_back()`: the node and output an input is linked from (reroutes
    /// skipped).
    pub fn peek_back(&self, i: In<'a>) -> Option<(&'a Node, usize)> {
        if self.folded_value(i).is_some() {
            // The inlined tree has a constant there instead of the link.
            return None;
        }
        match self.src_in(i) {
            Src::Linked(n, o) => Some((n, o)),
            _ => None,
        }
    }

    /// Whether the exporter reads the material through `InlineShaderNodes.from_material`
    /// (`BlenderMaterialIndentifier.__can_use_inline`): yes, unless a "glTF Material Output"
    /// group node has an `Occlusion` input (then the original tree is read), or the exporter
    /// asked for the original tree ([`LowerCx::original`], WP 7.7).
    pub fn uses_inline_tree(&self) -> bool {
        !self.original && self.gltf_output_input_raw("Occlusion").is_none()
    }

    /// `InlineShaderNodes.from_material` gives up (`ShaderNodesInliner::do_inline` returns
    /// false on `has_available_link_cycle`), so the exporter reads an empty tree: the
    /// material exports as the glTF default (verified on 5.2.4: a Math cycle on Base Color
    /// exports only `doubleSided`).
    pub fn inline_fails(&self) -> bool {
        self.uses_inline_tree() && self.has_available_link_cycle()
    }

    /// `bNodeTree::has_available_link_cycle`: a cycle through links between enabled sockets.
    pub fn has_available_link_cycle(&self) -> bool {
        let tree = self.tree;
        let n = tree.nodes.len();
        let index: std::collections::HashMap<u32, usize> = tree
            .nodes
            .iter()
            .enumerate()
            .map(|(i, nd)| (nd.uid, i))
            .collect();
        let enabled_out =
            |nd: &Node, id: &str| nd.outputs.iter().any(|s| s.identifier == id && s.enabled);
        let enabled_in =
            |nd: &Node, id: &str| nd.inputs.iter().any(|s| s.identifier == id && s.enabled);
        // Edges from each node to the nodes it feeds.
        let mut next: Vec<Vec<usize>> = vec![Vec::new(); n];
        for l in &tree.links {
            let (Some(&f), Some(&t)) = (index.get(&l.from_node), index.get(&l.to_node)) else {
                continue;
            };
            if enabled_out(&tree.nodes[f], &l.from_socket)
                && enabled_in(&tree.nodes[t], &l.to_socket)
            {
                next[f].push(t);
            }
        }
        // Iterative three-colour depth-first search.
        let mut state = vec![0u8; n];
        for start in 0..n {
            if state[start] != 0 {
                continue;
            }
            let mut stack = vec![(start, 0usize)];
            state[start] = 1;
            while let Some(&mut (v, ref mut k)) = stack.last_mut() {
                if let Some(&w) = next[v].get(*k) {
                    *k += 1;
                    match state[w] {
                        0 => {
                            state[w] = 1;
                            stack.push((w, 0));
                        }
                        1 => return true,
                        _ => {}
                    }
                } else {
                    state[v] = 2;
                    stack.pop();
                }
            }
        }
        false
    }

    /// The value the inliner (`shader_nodes_inline.cc`, Blender 5.2.4, GPL-2.0-or-later)
    /// constant-folds into input `i` (any type: the implicit conversion into the socket's type
    /// applied): the input is linked (through reroutes and muted nodes) to a node with a
    /// multi-function whose available inputs are all constants (see `graph/fold.rs` for the
    /// nodes frap evaluates), or to a muted node or an unlinked reroute that leaves it
    /// dangling (then it is the socket's own value). `None` when the input is unlinked, not
    /// folded, or the original tree is read.
    pub fn folded_value(&self, i: In<'a>) -> Option<FoldVal> {
        if !self.uses_inline_tree() {
            return None;
        }
        let mut f = fold::Folder::new(self);
        let (from, o) = f.link_into(i.node, i.index)?;
        let s = i.socket();
        let ty = match s.ty {
            SocketType::Shader => SocketType::Rgba,
            t => t,
        };
        match f.output(from, o) {
            fold::Res::Val(v) => v.to(ty),
            fold::Res::Dangling => FoldVal::of_socket(s.ty, &s.default_value),
            fold::Res::No => None,
        }
    }

    /// The constant the inliner folds output `o` of node `n` into (what a link from it is
    /// replaced with), converted to `ty`; `None` when it stays linked, leaves the consumer
    /// dangling, or the original tree is read. For callers holding a resolved [`Src`] rather
    /// than the input it feeds (the `KHR_materials_*` patterns' `get_factor`).
    pub fn folded_output(&self, n: &'a Node, o: usize, ty: SocketType) -> Option<FoldVal> {
        if !self.uses_inline_tree() {
            return None;
        }
        match fold::Folder::new(self).output(n, o) {
            fold::Res::Val(v) => v.to(ty),
            fold::Res::Dangling | fold::Res::No => None,
        }
    }

    /// [`Self::folded_value`] of a VALUE input.
    pub fn folded(&self, i: In<'a>) -> Option<f32> {
        if i.socket().ty != SocketType::Value {
            return None;
        }
        self.folded_value(i).map(FoldVal::f)
    }

    /// The value the exporter reads from `inputs[…].default_value` of input `i`: its own value
    /// when unlinked, the folded one when the inliner folds the link; `None` when it stays
    /// linked (the add-on then reads `is_linked`, or the stale default).
    pub fn inline_value(&self, i: In<'a>) -> Option<FoldVal> {
        let s = i.socket();
        if self.tree.link_into(i.node.uid, &s.identifier).is_none() {
            return FoldVal::of_socket(s.ty, &s.default_value);
        }
        self.folded_value(i)
    }

    /// `NodeNav.get_constant()` (see the module documentation), by the socket's type.
    pub fn get_constant(&self, i: In<'a>) -> Option<Konst> {
        let s = i.socket();
        let ty = s.ty;
        if let Some(v) = self.folded_value(i) {
            return Some(match v {
                FoldVal::Float(f) => Konst::Value(f),
                FoldVal::Vector(v) => Konst::Vector(v),
                FoldVal::Color(c) => Konst::Rgb(rgb(&c)),
            });
        }
        if self.tree.link_into(i.node.uid, &s.identifier).is_none() {
            return match (ty, &s.default_value) {
                (SocketType::Rgba, SocketValue::Color(c)) => Some(Konst::Rgb(rgb(c))),
                (SocketType::Shader, _) => Some(Konst::Rgb([0.0; 3])),
                (SocketType::Vector, SocketValue::Vector(c)) => {
                    Some(Konst::Vector([c[0], c[1], c[2]]))
                }
                (SocketType::Value, SocketValue::Float(f)) => Some(Konst::Value(*f)),
                _ => None,
            };
        }
        let mut cur = i;
        // Each Ambient Occlusion hop moves back once; a tree has finitely many nodes.
        for _ in 0..=self.tree.nodes.len() {
            match self.src_in(cur) {
                // An unlinked reroute: the navigation stops on it, which is no constant.
                Src::Missing | Src::Const(_) => return None,
                Src::Linked(n, o) => match (ty, n.bl_idname) {
                    (SocketType::Rgba | SocketType::Shader, "ShaderNodeRGB") => {
                        return match &n.outputs[o].default_value {
                            SocketValue::Color(c) => Some(Konst::Rgb(rgb(c))),
                            _ => None,
                        };
                    }
                    (SocketType::Rgba | SocketType::Shader, "ShaderNodeAmbientOcclusion") => {
                        let color = In::named(n, "Color")?;
                        match self.src_in(color) {
                            Src::Const(SocketValue::Color(c)) => return Some(Konst::Rgb(rgb(c))),
                            Src::Linked(..) => cur = color,
                            _ => return None,
                        }
                    }
                    (SocketType::Value, "ShaderNodeValue") => {
                        return match n.outputs[o].default_value {
                            SocketValue::Float(f) => Some(Konst::Value(f)),
                            _ => None,
                        };
                    }
                    _ => return None,
                },
            }
        }
        None
    }

    /// `NodeNav.get_factor()`: a constant, or the single constant input of a Mix (RGBA,
    /// MULTIPLY; the factor is not checked) or Math MULTIPLY feeding the socket.
    pub fn get_factor(&self, i: In<'a>) -> Option<Konst> {
        if let Some(c) = self.get_constant(i) {
            return Some(c);
        }
        let (n, _) = self.peek_back(i)?;
        let (x1, x2) = match i.socket().ty {
            SocketType::Rgba
                if n.bl_idname == "ShaderNodeMix"
                    && n.prop_str("data_type") == "RGBA"
                    && n.prop_str("blend_type") == "MULTIPLY" =>
            {
                (
                    In::by_identifier(n, "A_Color").and_then(|s| self.get_constant(s)),
                    In::by_identifier(n, "B_Color").and_then(|s| self.get_constant(s)),
                )
            }
            SocketType::Value
                if n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY" =>
            {
                (
                    self.get_constant(In { node: n, index: 0 }),
                    self.get_constant(In { node: n, index: 1 }),
                )
            }
            _ => return None,
        };
        match (x1, x2) {
            (Some(c), None) | (None, Some(c)) => Some(c),
            _ => None,
        }
    }

    /// `get_multiply_factors(nav)`: the two factors of a Mix (RGBA, MULTIPLY, Factor 1) or a
    /// Math MULTIPLY feeding the socket.
    pub fn multiply_factors(&self, i: In<'a>) -> Option<(In<'a>, In<'a>)> {
        let (n, _) = self.peek_back(i)?;
        match i.socket().ty {
            SocketType::Rgba => {
                let is_mul = n.bl_idname == "ShaderNodeMix"
                    && n.prop_str("data_type") == "RGBA"
                    && n.prop_str("blend_type") == "MULTIPLY"
                    && In::named(n, "Factor")
                        .and_then(|f| self.get_constant(f))
                        .and_then(Konst::value)
                        == Some(1.0);
                is_mul.then_some(())?;
                Some((
                    In::by_identifier(n, "A_Color")?,
                    In::by_identifier(n, "B_Color")?,
                ))
            }
            SocketType::Value
                if n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY" =>
            {
                Some((In { node: n, index: 0 }, In { node: n, index: 1 }))
            }
            _ => None,
        }
    }

    /// `detect_multiply_by_constant(nav)`: the constant and the other factor.
    pub fn multiply_by_constant(&self, i: In<'a>) -> Option<(Konst, In<'a>)> {
        let (f1, f2) = self.multiply_factors(i)?;
        if let Some(c) = self.get_constant(f1) {
            return Some((c, f2));
        }
        self.get_constant(f2).map(|c| (c, f1))
    }

    /// `get_color_attrib(nav)`: a Color Attribute node (`layer_name`) or an Attribute node
    /// of type GEOMETRY (`attribute_name`) feeding the socket; `""` is the active render
    /// colour attribute.
    pub fn color_attrib(&self, i: In<'a>) -> Option<SmolStr> {
        let (n, _) = self.peek_back(i)?;
        match n.bl_idname {
            "ShaderNodeVertexColor" => Some(n.prop_str("layer_name").into()),
            "ShaderNodeAttribute" if n.prop_str("attribute_type") == "GEOMETRY" => {
                Some(n.prop_str("attribute_name").into())
            }
            _ => None,
        }
    }

    /// `detect_multiply_by_color_attrib(nav)`: the attribute and the other factor.
    pub fn multiply_by_color_attrib(&self, i: In<'a>) -> Option<(SmolStr, In<'a>)> {
        let (f1, f2) = self.multiply_factors(i)?;
        if let Some(a) = self.color_attrib(f1) {
            return Some((a, f2));
        }
        self.color_attrib(f2).map(|a| (a, f1))
    }

    /// `gather_color_info(nav)`: peels `[Multiply by Factor] -> [Multiply by Color Attrib]`
    /// (in either order) off a colour socket.
    pub fn color_info(&self, i: In<'a>) -> ColorInfo {
        let mut info = ColorInfo::default();
        let mut nav = i;
        for _ in 0..2 {
            if info.factor.is_none() {
                if let Some(a) = self.get_constant(nav) {
                    info.factor = Some(konst3(a));
                    break;
                }
                if let Some((a, rest)) = self.multiply_by_constant(nav) {
                    info.factor = Some(konst3(a));
                    nav = rest;
                    continue;
                }
            }
            if info.attrib.is_none() {
                if let Some(a) = self.color_attrib(nav) {
                    info.attrib = Some(a);
                    break;
                }
                if let Some((a, rest)) = self.multiply_by_color_attrib(nav) {
                    info.attrib = Some(a);
                    nav = rest;
                    continue;
                }
            }
            break;
        }
        info
    }

    // ------------------------------------------------------------------ M1 helpers

    /// `get_constant` for colour sockets: RGB of an unlinked socket or of an RGB node.
    pub fn const_rgb(&self, src: Src<'a>) -> Option<[f32; 3]> {
        match src {
            Src::Const(SocketValue::Color(c)) => Some(rgb(c)),
            Src::Linked(n, o) if n.bl_idname == "ShaderNodeRGB" => match n.outputs[o].default_value
            {
                SocketValue::Color(c) => Some(rgb(&c)),
                _ => None,
            },
            _ => None,
        }
    }

    /// `get_constant` for value sockets: an unlinked socket or a Value node.
    pub fn const_f32(&self, src: Src<'a>) -> Option<f32> {
        match src {
            Src::Const(SocketValue::Float(f)) => Some(*f),
            Src::Linked(n, o) if n.bl_idname == "ShaderNodeValue" => {
                match n.outputs[o].default_value {
                    SocketValue::Float(f) => Some(f),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `get_multiply_factors` for a colour socket: a Mix (RGBA, MULTIPLY, Factor 1) gives its
    /// `A` and `B` sources.
    pub fn color_multiply(&self, src: Src<'a>) -> Option<(Src<'a>, Src<'a>)> {
        let Src::Linked(n, _) = src else { return None };
        let is_mul = n.bl_idname == "ShaderNodeMix"
            && n.prop_str("data_type") == "RGBA"
            && n.prop_str("blend_type") == "MULTIPLY"
            && self.const_f32(self.src(n, "Factor")) == Some(1.0);
        is_mul.then(|| (self.src(n, "A"), self.src(n, "B")))
    }

    /// `get_multiply_factors` for a value socket: Math MULTIPLY gives its two inputs.
    pub fn value_multiply(&self, src: Src<'a>) -> Option<(Src<'a>, Src<'a>)> {
        let Src::Linked(n, _) = src else { return None };
        (n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY")
            .then(|| (self.src_at(n, 0), self.src_at(n, 1)))
    }

    /// `detect_multiply_by_constant` for colours: the constant and the other factor.
    pub fn color_times_const(&self, src: Src<'a>) -> Option<([f32; 3], Src<'a>)> {
        let (a, b) = self.color_multiply(src)?;
        if let Some(c) = self.const_rgb(a) {
            return Some((c, b));
        }
        self.const_rgb(b).map(|c| (c, a))
    }

    /// `detect_multiply_by_constant` for values: the constant and the other factor.
    pub fn value_times_const(&self, src: Src<'a>) -> Option<(f32, Src<'a>)> {
        let (a, b) = self.value_multiply(src)?;
        if let Some(c) = self.const_f32(a) {
            return Some((c, b));
        }
        self.const_f32(b).map(|c| (c, a))
    }

    /// The Image Texture node behind `src` and the channel used: its `Color` (RGB) or `Alpha`
    /// (A) output, or a Separate Color (RGB mode) output fed by its `Color` (R, G or B).
    pub fn image_behind(&self, src: Src<'a>) -> Option<(&'a Node, ChannelSel)> {
        self.image_behind_at(src, 0)
    }

    /// [`Self::image_behind`], `depth` Separate Color nodes deep (a cycle of Separate Color
    /// nodes ends once every node was crossed).
    fn image_behind_at(&self, src: Src<'a>, depth: usize) -> Option<(&'a Node, ChannelSel)> {
        let Src::Linked(n, o) = src else { return None };
        match n.bl_idname {
            "ShaderNodeTexImage" => match n.outputs[o].identifier.as_str() {
                "Color" => Some((n, ChannelSel::Rgb)),
                "Alpha" => Some((n, ChannelSel::A)),
                _ => None,
            },
            "ShaderNodeSeparateColor" if n.prop_str("mode") == "RGB" => {
                let channel = match n.outputs[o].identifier.as_str() {
                    "Red" => ChannelSel::R,
                    "Green" => ChannelSel::G,
                    "Blue" => ChannelSel::B,
                    _ => return None,
                };
                if depth >= self.tree.nodes.len() {
                    return None;
                }
                match self.image_behind_at(self.src(n, "Color"), depth + 1)? {
                    (tex, ChannelSel::Rgb) => Some((tex, channel)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------------ texture search

    /// `get_texture_node_from_socket(socket)`: the first Image Texture found by the add-on's
    /// depth-first search back from an input socket (`from_socket` with
    /// `FilterByType(ShaderNodeTexImage)`: the node feeding each link, then its inputs in
    /// order, recursively), `None` when there is none or when the first one found has no
    /// image. Group nodes are not entered (module documentation).
    pub fn texture_from(&self, i: In<'a>) -> Option<TexHit<'a>> {
        if i.node.bl_idname == "ShaderNodeTexImage" {
            return self.image_of(i.node).map(|_| TexHit {
                node: i.node,
                path: Vec::new(),
            });
        }
        let hit = self.search_from(i, "ShaderNodeTexImage")?;
        self.image_of(hit.node).map(|_| hit)
    }

    /// `from_socket(socket, FilterByType(bl_idname))[0]`: the first node of that type found by
    /// the depth-first search back from an input socket (`__gather_normal_scale` looks for
    /// the Normal Map node this way).
    pub fn node_from(&self, i: In<'a>, bl_idname: &str) -> Option<&'a Node> {
        if i.node.bl_idname == bl_idname {
            return Some(i.node);
        }
        self.search_from(i, bl_idname).map(|h| h.node)
    }

    /// The depth-first search of [`Self::texture_from`] / [`Self::node_from`] from input `i`.
    fn search_from(&self, i: In<'a>, ty: &str) -> Option<TexHit<'a>> {
        let mut st = SearchState {
            valid: self.tree.links_valid(),
            done: std::collections::BTreeSet::new(),
            on_path: std::collections::BTreeSet::from([i.node.uid]),
            path: Vec::new(),
        };
        self.search(i.node, i.index, ty, &mut st)
    }

    /// `__search_from_socket`, stopping at the first match. `st.done` holds nodes whose whole
    /// upstream was searched without a match (searching them again finds nothing either).
    ///
    /// Links that Blender flags invalid (`NodeLink.is_valid`: the back edge of a cycle, a
    /// shader output into a non-shader input, an unavailable socket) are not followed: they
    /// carry nothing in the tree the exporter reads (verified on 5.2.4: an Image Texture whose
    /// link closes a Math cycle is not exported). Edges back to a node on the current path
    /// are skipped as well, so a cycle always ends the walk.
    fn search(
        &self,
        node: &'a Node,
        input: usize,
        ty: &str,
        st: &mut SearchState<'a>,
    ) -> Option<TexHit<'a>> {
        let s = node.inputs.get(input)?;
        for (li, l) in self.tree.links.iter().enumerate() {
            if l.to_node != node.uid || l.to_socket != s.identifier || !st.valid[li] {
                continue;
            }
            let Some(from) = self.tree.node(l.from_node) else {
                continue;
            };
            let Some(o) = from.output_by_identifier(&l.from_socket) else {
                continue;
            };
            if frap_nodes::is_group_node(from.bl_idname)
                || st.done.contains(&from.uid)
                || st.on_path.contains(&from.uid)
            {
                continue;
            }
            st.path.push(PathLink {
                from_node: from,
                from_output: o,
            });
            if from.bl_idname == ty {
                return Some(TexHit {
                    node: from,
                    path: st.path.clone(),
                });
            }
            st.on_path.insert(from.uid);
            for k in 0..from.inputs.len() {
                if let Some(hit) = self.search(from, k, ty, st) {
                    return Some(hit);
                }
            }
            st.on_path.remove(&from.uid);
            st.path.pop();
            st.done.insert(from.uid);
        }
        None
    }

    /// `has_image_node_from_socket`.
    pub fn has_image(&self, i: Option<In<'a>>) -> bool {
        i.is_some_and(|i| self.texture_from(i).is_some())
    }

    /// The channel `image.py` reads for the socket `i` from the texture found behind it
    /// (`__get_image_data_mapping`): a Separate Color output on the way (`Red`, `Green`,
    /// `Blue`) or an `Alpha` output (the one nearest the texture wins); else the glTF
    /// convention of the socket (Metallic B, Roughness G, Occlusion R, Coat Weight R, Coat
    /// Roughness G, Thickness G; an Alpha socket fed by a colour output: RGB → BW, else A;
    /// Specular IOR Level and Sheen Roughness A), else R.
    pub fn source_channel(&self, i: In<'a>, hit: &TexHit<'a>) -> SrcChannel {
        let s = i.socket();
        let mut chan = None;
        for elem in &hit.path {
            let from = elem.from_socket();
            if elem.from_node.bl_idname == "ShaderNodeSeparateColor" {
                chan = match from.identifier.as_str() {
                    "Red" => Some(ChannelSel::R),
                    "Green" => Some(ChannelSel::G),
                    "Blue" => Some(ChannelSel::B),
                    _ => chan,
                };
            }
            if from.identifier == "Alpha" {
                chan = Some(ChannelSel::A);
            }
        }
        if let Some(c) = chan {
            return SrcChannel::One(c);
        }
        let id = s.identifier.as_str();
        let c = match id {
            "Metallic" => ChannelSel::B,
            "Roughness" => ChannelSel::G,
            _ if s.name == "Occlusion" => ChannelSel::R,
            "Alpha" => {
                let last_is_color = hit
                    .path
                    .last()
                    .is_some_and(|e| e.from_socket().ty == SocketType::Rgba);
                if last_is_color {
                    return SrcChannel::Rgb2Bw;
                }
                ChannelSel::A
            }
            "Coat Weight" => ChannelSel::R,
            "Coat Roughness" | "Thickness" => ChannelSel::G,
            "Specular IOR Level" | "Sheen Roughness" => ChannelSel::A,
            _ => ChannelSel::R,
        };
        SrcChannel::One(c)
    }

    /// The image of an Image Texture node (`None` when unset: the exporter skips it).
    pub fn image_of(&self, tex: &Node) -> Option<IdRef<ImageId>> {
        match tex.prop("image") {
            Some(Value::Id(Some(id))) => IdRef::from_any(*id),
            _ => None,
        }
    }

    /// The texture reference of Image Texture node `tex` read through `channel`: image,
    /// colour space, sampler (`sampler.rs`), UV source (`uv_source.rs`) and texture transform
    /// (`texture_transform.rs`), as `__gather_texture_transform_and_tex_coord` walks
    /// `[UV Map] => [Mapping] => [UV Wrapping] => [Texture]`. Unsupported setups add a
    /// WARNING.
    pub fn tex_ref(
        &self,
        tex: &'a Node,
        channel: ChannelSel,
        diags: &mut Vec<Report>,
    ) -> Option<TexRef> {
        let image = self.image_of(tex)?;
        {
            let mut used = self.used_nodes.borrow_mut();
            if !used.contains(&tex.name) {
                used.push(tex.name.clone());
            }
        }
        let colorspace_is_data = self.images.image(image).is_some_and(|im| {
            im.colorspace_settings.is_data || im.colorspace_settings.name == "Non-Color"
        });
        let wrapping = crate::patterns::sampler::detect_manual_uv_wrapping(self, tex);
        let sampler: Sampler = crate::patterns::sampler::sampler_of(tex, wrapping.as_ref());
        let mut src = match &wrapping {
            Some(w) => self.src_in(w.next),
            None => self.src(tex, "Vector"),
        };
        let mut transform: Option<TexTransform> = None;
        if let Src::Linked(n, _) = src
            && n.bl_idname == "ShaderNodeMapping"
        {
            transform = crate::patterns::texture_transform::from_mapping(self, n, diags);
            src = self.src(n, "Vector");
        }
        let uv: UvSource = crate::patterns::uv_source::uv_of(tex, src, diags);
        Some(TexRef {
            image,
            uv,
            channel,
            transform,
            sampler,
            colorspace_is_data,
        })
    }
}

/// One Math node operation on constants, as Blender's multi-functions compute it (the
/// `safe_*` helpers of `BLI_math_base_safe.h`, `BLI_math_base.h`; Blender 5.2.4,
/// GPL-2.0-or-later). `None` for an unknown operation.
pub fn math_op(op: &str, [a, b, c]: [f32; 3]) -> Option<f32> {
    let safe_divide = |a: f32, b: f32| if b == 0.0 { 0.0 } else { a / b };
    let smoothmin = |a: f32, b: f32, c: f32| {
        if c != 0.0 {
            let h = (c - (a - b).abs()).max(0.0) / c;
            a.min(b) - h * h * h * c * (1.0 / 6.0)
        } else {
            a.min(b)
        }
    };
    let fract = |a: f32| a - a.floor();
    Some(match op {
        "ADD" => a + b,
        "SUBTRACT" => a - b,
        "MULTIPLY" => a * b,
        "DIVIDE" => safe_divide(a, b),
        "MULTIPLY_ADD" => a * b + c,
        "POWER" => {
            if a < 0.0 && b != (b as i32) as f32 {
                0.0
            } else {
                a.powf(b)
            }
        }
        "LOGARITHM" => {
            if a <= 0.0 || b <= 0.0 {
                0.0
            } else {
                safe_divide(a.ln(), b.ln())
            }
        }
        "SQRT" => {
            if a < 0.0 {
                0.0
            } else {
                a.sqrt()
            }
        }
        "INVERSE_SQRT" => {
            if a > 0.0 {
                1.0 / a.sqrt()
            } else {
                0.0
            }
        }
        "ABSOLUTE" => a.abs(),
        "EXPONENT" => a.exp(),
        "MINIMUM" => a.min(b),
        "MAXIMUM" => a.max(b),
        "LESS_THAN" => f32::from(u8::from(a < b)),
        "GREATER_THAN" => f32::from(u8::from(a > b)),
        "SIGN" => {
            if a > 0.0 {
                1.0
            } else if a < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        "COMPARE" => f32::from(u8::from((a - b).abs() <= c.max(f32::EPSILON))),
        "SMOOTH_MIN" => smoothmin(a, b, c),
        "SMOOTH_MAX" => -smoothmin(-a, -b, c),
        "ROUND" => (a + 0.5).floor(),
        "FLOOR" => a.floor(),
        "CEIL" => a.ceil(),
        "TRUNC" => a.trunc(),
        "FRACT" => fract(a),
        "MODULO" => {
            if b != 0.0 {
                a % b
            } else {
                0.0
            }
        }
        "FLOORED_MODULO" => {
            if b != 0.0 {
                a - (a / b).floor() * b
            } else {
                0.0
            }
        }
        "WRAP" => {
            let range = b - c;
            if range != 0.0 {
                a - range * ((a - c) / range).floor()
            } else {
                c
            }
        }
        "SNAP" => safe_divide(a, b).floor() * b,
        "PINGPONG" => {
            if b != 0.0 {
                (fract((a - b) / (b * 2.0)) * b * 2.0 - b).abs()
            } else {
                0.0
            }
        }
        "SINE" => a.sin(),
        "COSINE" => a.cos(),
        "TANGENT" => a.tan(),
        "SINH" => a.sinh(),
        "COSH" => a.cosh(),
        "TANH" => a.tanh(),
        "ARCSINE" => {
            if a <= -1.0 {
                -std::f32::consts::FRAC_PI_2
            } else if a >= 1.0 {
                std::f32::consts::FRAC_PI_2
            } else {
                a.asin()
            }
        }
        "ARCCOSINE" => {
            if a <= -1.0 {
                std::f32::consts::PI
            } else if a >= 1.0 {
                0.0
            } else {
                a.acos()
            }
        }
        "ARCTANGENT" => a.atan(),
        "ARCTAN2" => a.atan2(b),
        "RADIANS" => a * (std::f32::consts::PI / 180.0),
        "DEGREES" => a * (180.0 / std::f32::consts::PI),
        _ => return None,
    })
}

fn rgb(c: &[f32; 4]) -> [f32; 3] {
    [c[0], c[1], c[2]]
}

/// `a[:3]` of a constant (a value counts for every channel, as Python's list slicing
/// would fail on it: such a factor never occurs on a colour socket).
fn konst3(k: Konst) -> [f32; 3] {
    match k {
        Konst::Rgb(c) | Konst::Vector(c) => c,
        Konst::Value(v) => [v; 3],
    }
}

pub use crate::patterns::texture_transform::{
    blender_to_gltf_transform, gltf_to_blender_transform, is_identity,
};

/// Helpers for the pattern tests (WP 3.5): materials built node by node as a script would.
#[cfg(test)]
pub(crate) mod testkit {
    use std::collections::HashMap;

    use frap_nodes::{NodeTree, SocketAddr, SocketValue};
    use frap_rna::{IdKey, IdRef, ImageId, Value};
    use frap_scene::id::{Image, Material};
    use smol_str::SmolStr;

    use crate::{ImageLookup, LoweredMaterial, NameAlloc};

    /// Images by reference.
    #[derive(Default)]
    pub struct Images(pub HashMap<IdRef<ImageId>, Image>);

    impl ImageLookup for Images {
        fn image(&self, im: IdRef<ImageId>) -> Option<&Image> {
            self.0.get(&im)
        }
    }

    impl Images {
        /// A 4×4 image (Non-Color when `data`).
        pub fn add(&mut self, name: &str, data: bool) -> IdRef<ImageId> {
            let r = IdRef::from_key(IdKey::next());
            let mut im = Image::new(name, 4, 4);
            if data {
                im.colorspace_settings.name = "Non-Color".into();
            }
            self.0.insert(r, im);
            r
        }
    }

    /// `bpy.data.materials.new(name)`.
    pub fn mat(name: &str) -> Material {
        let mut m = Material::new(name);
        m.node_tree = NodeTree::default_material();
        m
    }

    /// The default Principled BSDF.
    pub const BSDF: u32 = 0;
    /// The default Material Output.
    pub const OUT: u32 = 1;

    /// `nodes.new(ty)`.
    pub fn add(m: &mut Material, ty: &str) -> u32 {
        let i = m.node_tree.new_node(ty).unwrap();
        m.node_tree.nodes[i].uid
    }

    /// `node.inputs[name].default_value = v`.
    pub fn set_in(m: &mut Material, uid: u32, name: &str, v: SocketValue) {
        m.node_tree
            .node_mut(uid)
            .unwrap()
            .input_mut(name)
            .unwrap()
            .default_value = v;
    }

    /// `node.inputs[i].default_value = v`.
    pub fn set_in_at(m: &mut Material, uid: u32, i: usize, v: SocketValue) {
        m.node_tree.node_mut(uid).unwrap().inputs[i].default_value = v;
    }

    /// `node.<prop> = 'ITEM'`.
    pub fn set_enum(m: &mut Material, uid: u32, prop: &str, v: &str) {
        assert!(
            m.node_tree
                .node_mut(uid)
                .unwrap()
                .set_prop(prop, Value::Enum(v.into()))
        );
    }

    /// `links.new(from.outputs[name], to.inputs[name])`.
    pub fn link(m: &mut Material, from: (u32, &str), to: (u32, &str)) {
        m.node_tree.link_by_name(from, to).unwrap();
    }

    /// `links.new(from.outputs[i], to.inputs[j])`.
    pub fn link_at(m: &mut Material, from: (u32, usize), to: (u32, usize)) {
        m.node_tree
            .link_new(
                SocketAddr::output(from.0, from.1),
                SocketAddr::input(to.0, to.1),
                true,
            )
            .unwrap();
    }

    /// An Image Texture node showing `image`.
    pub fn image_node(m: &mut Material, image: IdRef<ImageId>) -> u32 {
        let t = add(m, "ShaderNodeTexImage");
        assert!(
            m.node_tree
                .node_mut(t)
                .unwrap()
                .set_prop("image", Value::Id(Some(image.any())))
        );
        t
    }

    /// A group node with the "glTF Material Output" inputs (no node tree, as `raise` makes
    /// it).
    pub fn settings_node(m: &mut Material) -> u32 {
        let g = frap_nodes::builtin_groups::gltf_material_output::new_group();
        let uid = add(m, "ShaderNodeGroup");
        m.node_tree
            .node_mut(uid)
            .unwrap()
            .sync_group_sockets(Some(&g.interface));
        uid
    }

    /// RGBA colour value.
    pub fn color(r: f32, g: f32, b: f32) -> SocketValue {
        SocketValue::Color([r, g, b, 1.0])
    }

    /// Float value.
    pub fn float(f: f32) -> SocketValue {
        SocketValue::Float(f)
    }

    /// `lower(material)`.
    pub fn lower_m(m: &Material, images: &Images) -> LoweredMaterial {
        crate::lower(&m.node_tree, m, images)
    }

    /// `raise(lm)` into a fresh material, then lowered again; checks that `NameAlloc` holds
    /// every node name.
    pub fn raise_lower(lm: &LoweredMaterial, images: &Images) -> (Material, LoweredMaterial) {
        let mut m = Material::new("raised");
        let mut names = NameAlloc::default();
        crate::raise(lm, &mut m, &mut names);
        let in_tree: std::collections::BTreeSet<SmolStr> =
            m.node_tree.nodes.iter().map(|n| n.name.clone()).collect();
        assert_eq!(names.taken, in_tree, "NameAlloc holds every node name");
        let again = lower_m(&m, images);
        (m, again)
    }

    /// The law `lower(raise(lower(x))) == lower(x)` (diagnostics aside).
    pub fn assert_law(lm: &LoweredMaterial, images: &Images) {
        let (m, again) = raise_lower(lm, images);
        let strip = |x: &LoweredMaterial| LoweredMaterial {
            diagnostics: Vec::new(),
            ..x.clone()
        };
        assert_eq!(
            strip(&again),
            strip(lm),
            "lower(raise(lower(x))) != lower(x); raised nodes: {:#?}",
            m.node_tree
                .nodes
                .iter()
                .map(|n| (n.name.as_str(), n.bl_idname))
                .collect::<Vec<_>>()
        );
    }
}
