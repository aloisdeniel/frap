// SPDX-License-Identifier: GPL-3.0-or-later
//! Node creation from the generated node-type table (`nodes.new`) and the default graphs of
//! `bpy.data.materials.new()` / `bpy.data.worlds.new()` (ARCHITECTURE §16.1, WP 1.11).
//!
//! Verified on Blender 5.2.4:
//! - `nodes.new(type=…)`: name = the type's default name made unique with `.001`
//!   (`BLI_uniquename`), `label` empty, `location` (0, 0), `width` from the type.
//! - An unknown or removed type (`ShaderNodeSeparateRGB`) raises
//!   `RuntimeError: Error: Node type ShaderNodeSeparateRGB undefined`; a registered type of
//!   another tree type raises the poll error of `poll.rs`.
//! - `materials.new()`: nodes `Principled BSDF` at (−200, 100), then `Material Output` at
//!   (200, 100), linked BSDF → Surface; the tree is named "Shader Nodetree".
//! - `worlds.new()`: `Background` (Color 0.05 grey, Strength 1) at (−200, 100), then
//!   `World Output` at (200, 100), linked Background → Surface; the tree is named
//!   "World Nodetree" (the factory-startup World's tree is "Shader Nodetree").

use frap_rna::naming::unique_subname;
use frap_rna::{PropKind, PyExc, RnaError, RnaErrorKind, StructDef, Value};
use smol_str::SmolStr;

use crate::generated::{NodePropDefault, NodeTypeDef, SocketDef, SocketDefault, node_type};
use crate::{Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketType, SocketValue};

/// Byte limit of node names (`MAX_NAME` sub-data: 63 bytes).
pub const MAX_NODE_NAME: usize = 63;
/// Name of the embedded tree of a material created by `bpy.data.materials.new()`.
pub const MATERIAL_TREE_NAME: &str = "Shader Nodetree";
/// Name of the embedded tree of a world created by `bpy.data.worlds.new()` (verified).
pub const WORLD_TREE_NAME: &str = "World Nodetree";

/// `RuntimeError: Error: Node type <bl_idname> undefined` (verified wording of `nodes.new`).
pub fn node_type_undefined(bl_idname: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Error: Node type {bl_idname} undefined"),
    )
}

/// The `SocketValue` of a fresh socket of type `ty` with generated default `d`.
pub fn socket_value(ty: SocketType, d: &SocketDefault) -> SocketValue {
    match *d {
        SocketDefault::None => match ty {
            SocketType::Object
            | SocketType::Image
            | SocketType::Collection
            | SocketType::Material
            | SocketType::Texture => SocketValue::Id(None),
            _ => SocketValue::None,
        },
        SocketDefault::Float(f) => SocketValue::Float(f),
        SocketDefault::Int(i) => SocketValue::Int(i as i32),
        SocketDefault::Bool(b) => SocketValue::Bool(b),
        SocketDefault::Vector(v) if ty == SocketType::Rotation => {
            let mut r = [0.0; 3];
            for (o, x) in r.iter_mut().zip(v) {
                *o = *x;
            }
            SocketValue::Rotation(r)
        }
        SocketDefault::Vector(v) => {
            let mut r = [0.0; 4];
            for (o, x) in r.iter_mut().zip(v) {
                *o = *x;
            }
            SocketValue::Vector(r)
        }
        SocketDefault::Color(c) => SocketValue::Color(c),
        SocketDefault::Str(s) => SocketValue::String(SmolStr::new_static(s)),
        SocketDefault::Menu(s) => SocketValue::Menu(SmolStr::new_static(s)),
    }
}

/// A fresh socket from its generated definition.
pub fn socket_from_def(d: &SocketDef) -> Socket {
    Socket {
        name: SmolStr::new_static(d.name),
        identifier: SmolStr::new_static(d.identifier),
        ty: d.ty,
        enabled: d.enabled,
        hide_value: d.hide_value,
        default_value: socket_value(d.ty, &d.default),
    }
}

/// The RNA value of a fresh node property (`NodeTypeDef::props`), typed by the node's
/// `PropDef` (enum vs string, ID vs struct pointer).
pub fn prop_value(srna: &'static StructDef, prop: &str, d: &NodePropDefault) -> Value {
    let kind = srna.find_prop(prop).map(|p| &p.kind);
    match *d {
        NodePropDefault::None => match kind {
            Some(PropKind::Pointer { is_id: true, .. }) => Value::Id(None),
            _ => Value::None,
        },
        NodePropDefault::Bool(b) => Value::Bool(b),
        NodePropDefault::Int(i) => Value::Int(i),
        NodePropDefault::Float(f) => Value::Float(f),
        NodePropDefault::Floats(v) => match srna.find_prop(prop) {
            Some(p) => Value::FloatArray {
                data: v.iter().copied().collect(),
                shape: p.array,
                tag: frap_rna::MathTag::for_subtype(p.subtype),
            },
            None => Value::FloatArray {
                data: v.iter().copied().collect(),
                shape: frap_rna::ArrayShape::Fixed(v.len() as u16),
                tag: frap_rna::MathTag::None,
            },
        },
        NodePropDefault::Str(s) => match kind {
            Some(PropKind::Enum { .. }) => Value::Enum(SmolStr::new_static(s)),
            _ => Value::Str(SmolStr::new_static(s)),
        },
        NodePropDefault::Flags(f) => {
            Value::EnumSet(f.iter().map(|s| SmolStr::new_static(s)).collect())
        }
    }
}

impl Node {
    /// A fresh node of type `def` (sockets, defaults, width and properties of the generated
    /// table) with the given uid and name, at (0, 0).
    pub fn from_type(def: &'static NodeTypeDef, uid: u32, name: SmolStr) -> Node {
        let mut node = Node {
            uid,
            name,
            label: String::new(),
            bl_idname: def.bl_idname,
            location: [0.0, 0.0],
            width: def.width,
            parent: None,
            mute: false,
            hide: false,
            inputs: def.inputs.iter().map(socket_from_def).collect(),
            outputs: def.outputs.iter().map(socket_from_def).collect(),
            props: PropBag {
                values: def
                    .props
                    .iter()
                    .map(|(k, v)| (*k, prop_value(def.srna, k, v)))
                    .collect(),
            },
        };
        node.update_availability();
        node
    }

    /// The generated type of this node (`None` for a `bl_idname` that is not in the table).
    pub fn type_def(&self) -> Option<&'static NodeTypeDef> {
        node_type(self.bl_idname)
    }
}

impl NodeTree {
    /// A node name that is unique in this tree (`BLI_uniquename`, 63 bytes, `.001` suffixes);
    /// node `except` (an index) is ignored, for renames. An empty request gives "Node".
    pub fn unique_node_name(&self, requested: &str, except: Option<usize>) -> SmolStr {
        let taken = |n: &str| {
            self.nodes
                .iter()
                .enumerate()
                .any(|(i, x)| Some(i) != except && x.name == n)
        };
        unique_subname(&taken, requested, MAX_NODE_NAME, "Node")
    }

    /// Appends a fresh node of type `def` named after the type (made unique) and returns its
    /// index in `nodes`.
    pub fn add_node(&mut self, def: &'static NodeTypeDef) -> usize {
        self.add_node_with(def, &|_| false)
    }

    /// [`NodeTree::add_node`] where names for which `reserved` is true also count as taken
    /// (`frap-material`'s `NameAlloc` while raising a graph).
    pub fn add_node_with(
        &mut self,
        def: &'static NodeTypeDef,
        reserved: &dyn Fn(&str) -> bool,
    ) -> usize {
        let taken = |n: &str| reserved(n) || self.nodes.iter().any(|x| x.name == n);
        let name = unique_subname(&taken, def.name, MAX_NODE_NAME, "Node");
        let uid = self.next_node_uid;
        self.next_node_uid += 1;
        self.nodes.push(Node::from_type(def, uid, name));
        self.nodes.len() - 1
    }

    /// `node_tree.nodes.new(type=bl_idname)`: index of the new node, or Blender's
    /// `Node type … undefined` error for unknown and removed types, or its `Cannot add node
    /// of type … to node tree …` poll error for a type of another tree type (`poll.rs`).
    pub fn new_node(&mut self, bl_idname: &str) -> Result<usize, RnaError> {
        if let Some(p) = super::poll::poll_failure(self.ty, bl_idname) {
            return Err(super::poll::poll_error(self, bl_idname, p));
        }
        let def = node_type(bl_idname).ok_or_else(|| node_type_undefined(bl_idname))?;
        Ok(self.add_node(def))
    }

    /// Adds the graph of `bpy.data.materials.new()`: `Principled BSDF` at (−200, 100) and
    /// `Material Output` at (200, 100), linked BSDF → Surface.
    pub fn add_default_material_graph(&mut self) {
        self.add_default_graph(
            "ShaderNodeBsdfPrincipled",
            "BSDF",
            "ShaderNodeOutputMaterial",
            "Surface",
        );
    }

    /// Adds the graph of `bpy.data.worlds.new()`: `Background` at (−200, 100) and
    /// `World Output` at (200, 100), linked Background → Surface. `worlds.new()` sets the
    /// Background colour to 0.05 grey (a fresh `nodes.new` Background is 0.8 grey); Strength
    /// stays 1 (verified).
    pub fn add_default_world_graph(&mut self) {
        let first = self.nodes.len();
        self.add_default_graph(
            "ShaderNodeBackground",
            "Background",
            "ShaderNodeOutputWorld",
            "Surface",
        );
        if let Some(c) = self.nodes[first].input_mut("Color") {
            c.default_value = SocketValue::Color([0.05, 0.05, 0.05, 1.0]);
        }
    }

    fn add_default_graph(&mut self, shader: &str, out_socket: &str, output: &str, in_socket: &str) {
        let s = self
            .new_node(shader)
            .expect("invariant: generated node type");
        self.nodes[s].location = [-200.0, 100.0];
        let o = self
            .new_node(output)
            .expect("invariant: generated node type");
        self.nodes[o].location = [200.0, 100.0];
        let (from_node, to_node) = (self.nodes[s].uid, self.nodes[o].uid);
        self.links.push(Link {
            from_node,
            from_socket: out_socket.into(),
            to_node,
            to_socket: in_socket.into(),
        });
    }

    /// The embedded tree of a material created by `bpy.data.materials.new()`.
    pub fn default_material() -> NodeTree {
        let mut t = NodeTree::new(MATERIAL_TREE_NAME, NodeTreeType::Shader, true);
        t.add_default_material_graph();
        t
    }

    /// The embedded tree of a world created by `bpy.data.worlds.new()`.
    pub fn default_world() -> NodeTree {
        let mut t = NodeTree::new(WORLD_TREE_NAME, NodeTreeType::Shader, true);
        t.add_default_world_graph();
        t
    }
}
