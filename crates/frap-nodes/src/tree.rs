// SPDX-License-Identifier: GPL-3.0-or-later
//! Node trees (ARCHITECTURE §16.1, CONTRACT frozen at M1 W0; socket and link names are
//! `SmolStr` per RFC 0001 F-NODE-1, because group sockets take them from user data).
//!
//! `header.is_embedded_data` is true for Material/World trees (`Root::Embedded`, §9.3) and false
//! for `bpy.data.node_groups`. Image Texture / Environment nodes hold their image as a
//! `Value::Id` in their `PropBag` (marker kind `ImageId`, §5.1).

use frap_anim::AnimData;
use frap_rna::*;
use indexmap::IndexMap;
use smol_str::SmolStr;

// WP 1.11: node creation and default graphs, editing (sockets, links), socket availability.
mod avail;
mod build;
mod edit;
mod poll;
mod toposort;

pub use avail::*;
pub use build::*;
pub use edit::*;

/// `NodeTree.bl_idname` family.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NodeTreeType {
    /// `ShaderNodeTree`
    Shader,
    /// `GeometryNodeTree`
    Geometry,
}

/// A node tree (embedded in a Material/World, or a `bpy.data.node_groups` ID).
#[derive(Clone, Debug)]
pub struct NodeTree {
    /// ID header.
    pub header: IdHeader,
    /// Tree type.
    pub ty: NodeTreeType,
    /// Nodes, in `tree.nodes` order.
    pub nodes: Vec<Node>,
    /// Links, in `tree.links` order.
    pub links: Vec<Link>,
    /// Group interface (`tree.interface.items_tree`).
    pub interface: Vec<InterfaceSocket>,
    /// Next session-unique node uid (§21.4: deterministic per session).
    pub next_node_uid: u32,
    /// Number of the next `Socket_<n>` interface item identifier (Blender's
    /// `bNodeTreeInterface.next_uid`): only increases, so identifiers are never reused after
    /// `interface.remove()`/`clear()` (WP 3.1).
    pub next_interface_uid: u32,
    /// Animation (`NTShader Nodetree` action slot, §18).
    pub animation_data: Option<AnimData>,
}

impl NodeTree {
    /// An empty tree (no nodes, links or interface). `embedded` gives the header of a
    /// Material/World tree (`is_embedded_data`, name "Shader Nodetree" by convention of the
    /// caller); otherwise a `bpy.data.node_groups` ID. The default graphs (`materials.new`:
    /// Principled BSDF + Material Output; worlds: Background + World Output) are WP 1.11's.
    pub fn new(name: &str, ty: NodeTreeType, embedded: bool) -> NodeTree {
        NodeTree {
            header: if embedded {
                IdHeader::embedded(name)
            } else {
                IdHeader::new(name)
            },
            ty,
            nodes: Vec::new(),
            links: Vec::new(),
            interface: Vec::new(),
            next_node_uid: 0,
            next_interface_uid: 0,
            animation_data: None,
        }
    }
}

/// One node.
#[derive(Clone, Debug)]
pub struct Node {
    /// Stable uid inside the tree (links refer to it).
    pub uid: u32,
    /// `node.name` (≤63 bytes, unique per tree).
    pub name: SmolStr,
    /// `node.label`
    pub label: String,
    /// `node.bl_idname` ("ShaderNodeBsdfPrincipled").
    pub bl_idname: &'static str,
    /// `node.location`
    pub location: [f32; 2],
    /// `node.width`
    pub width: f32,
    /// Parent frame uid.
    pub parent: Option<u32>,
    /// `node.mute`
    pub mute: bool,
    /// `node.hide`
    pub hide: bool,
    /// Input sockets.
    pub inputs: Vec<Socket>,
    /// Output sockets.
    pub outputs: Vec<Socket>,
    /// Node properties (`data_type`, `operation`, `image`, …), keyed by RNA identifier and
    /// typed by the generated `NodeTypeDef`.
    pub props: PropBag,
}

/// Node properties by RNA identifier (values already coerced against the node type's
/// `PropDef`s).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropBag {
    /// Values in `NodeTypeDef` order.
    pub values: IndexMap<&'static str, Value>,
}

/// Socket data type (`NodeSocket.type`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SocketType {
    /// VALUE
    Value,
    /// INT
    Int,
    /// BOOLEAN
    Boolean,
    /// VECTOR
    Vector,
    /// RGBA
    Rgba,
    /// SHADER
    Shader,
    /// STRING
    String,
    /// ROTATION
    Rotation,
    /// MATRIX
    Matrix,
    /// MENU
    Menu,
    /// GEOMETRY
    Geometry,
    /// OBJECT
    Object,
    /// IMAGE
    Image,
    /// COLLECTION
    Collection,
    /// MATERIAL
    Material,
    /// TEXTURE
    Texture,
    /// CUSTOM
    Custom,
}

/// A socket's `default_value`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SocketValue {
    /// No default value (SHADER, GEOMETRY).
    None,
    /// VALUE
    Float(f32),
    /// INT
    Int(i32),
    /// BOOLEAN
    Bool(bool),
    /// VECTOR (2D/3D/4D vectors store their used components first).
    Vector([f32; 4]),
    /// RGBA (linear)
    Color([f32; 4]),
    /// ROTATION (Euler XYZ)
    Rotation([f32; 3]),
    /// STRING
    String(SmolStr),
    /// MENU
    Menu(SmolStr),
    /// OBJECT / IMAGE / COLLECTION / MATERIAL / TEXTURE
    Id(Option<IdAny>),
}

/// One socket. Lookup by name returns the **first enabled** socket; identifiers are not keys.
#[derive(Clone, Debug)]
pub struct Socket {
    /// `socket.name`
    pub name: SmolStr,
    /// `socket.identifier`
    pub identifier: SmolStr,
    /// Data type.
    pub ty: SocketType,
    /// `socket.enabled` (recomputed by `availability/<node>.rs`).
    pub enabled: bool,
    /// `socket.hide_value`
    pub hide_value: bool,
    /// `socket.default_value`
    pub default_value: SocketValue,
}

/// A link between two sockets (by node uid and socket identifier).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// Source node uid.
    pub from_node: u32,
    /// Source socket identifier.
    pub from_socket: SmolStr,
    /// Target node uid.
    pub to_node: u32,
    /// Target socket identifier.
    pub to_socket: SmolStr,
}

/// Direction of an interface socket.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum InOut {
    /// INPUT
    Input,
    /// OUTPUT
    Output,
}

/// One item of a group interface (`NodeTreeInterfaceSocket`).
#[derive(Clone, Debug)]
pub struct InterfaceSocket {
    /// `name`
    pub name: SmolStr,
    /// `identifier` ("Input_1", "Socket_1")
    pub identifier: SmolStr,
    /// `in_out`
    pub in_out: InOut,
    /// `socket_type`
    pub socket_type: SocketType,
    /// `default_value`
    pub default_value: SocketValue,
}

impl IdType for NodeTree {
    type Kind = NodeTreeId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        for n in &self.nodes {
            for v in n.props.values.values() {
                if let Value::Id(Some(id)) = v {
                    f(*id);
                }
            }
            for s in n.inputs.iter().chain(&n.outputs) {
                if let SocketValue::Id(Some(id)) = s.default_value {
                    f(id);
                }
            }
        }
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        for n in &mut self.nodes {
            for v in n.props.values.values_mut() {
                if *v == Value::Id(Some(from)) {
                    *v = Value::Id(to);
                }
            }
            for s in n.inputs.iter_mut().chain(&mut n.outputs) {
                if s.default_value == SocketValue::Id(Some(from)) {
                    s.default_value = SocketValue::Id(to);
                }
            }
        }
        if let Some(a) = self.animation_data.as_mut()
            && a.action.map(|r| r.any()) == Some(from)
        {
            a.action = to.and_then(IdRef::from_any);
        }
    }
}
