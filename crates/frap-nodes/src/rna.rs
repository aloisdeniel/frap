// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of `NodeTree`, `Node`, `NodeSocket` and `NodeLink` (WP 1.11, ARCHITECTURE
//! §9.4, §16.1).
//!
//! The bindings are hand-written because the struct type is per instance: a node's `srna` is
//! its type's (`ShaderNodeMix`, from the generated `NodeTypeDef`) and a socket's is its socket
//! class (`NodeSocketColor`, `NodeSocketFloatFactor`), which decides `default_value`'s shape.
//! Nodes, sockets and links are served through proxies that keep a reference to the whole
//! tree, because their properties need siblings: `node.name` is unique per tree,
//! `socket.is_linked` and `socket.node` read the links and nodes, `link.from_node` is a node.
//!
//! Verified on 5.2.4 (RFC 0001 F-TR-5, Appendix A): nodes repr by name
//! (`nodes["Principled BSDF"]`), sockets by index (`inputs[0]` even when looked up as
//! `inputs['Base Color']`), socket name lookup returns the first **enabled** socket, links
//! have no name (`links[0]`). Writing `node.name` to a taken name gives the next free `.NNN`
//! (`Mix` renamed to the taken `Mix.001` becomes `Mix.002`); an empty name becomes "Node".

use frap_rna::traits::{index_out_of_range, read_only, wrong_access};
use frap_rna::*;
use smol_str::SmolStr;

use crate::generated::node_type;
use crate::{
    INPUT_LINK_LIMIT, Link, MAX_NODE_NAME, Node, NodeTree, NodeTreeType, OUTPUT_LINK_LIMIT,
    SocketAddr, SocketType, SocketValue,
};

/// A property that exists in Blender's RNA but has no frap binding yet.
pub fn unbound(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::NotImplementedError,
        format!("frap does not implement {}.{prop} yet", srna.identifier),
    )
}

fn val(v: Value) -> Result<Field<'static>, RnaError> {
    Ok(Field::Value(v))
}

fn str_val(s: impl Into<SmolStr>) -> Result<Field<'static>, RnaError> {
    val(Value::Str(s.into()))
}

/// Math wrapper of a float array property: Blender wraps 3-component colours in
/// `mathutils.Color` but leaves 4-component ones (`default_value` of colour sockets) as
/// `bpy_prop_array`.
fn tag_for(prop: &PropDef) -> MathTag {
    match MathTag::for_subtype(prop.subtype) {
        MathTag::Color if prop.array_len() != 3 => MathTag::None,
        t => t,
    }
}

fn floats(prop: &PropDef, data: &[f32]) -> Value {
    Value::FloatArray {
        data: data.iter().copied().collect(),
        shape: prop.array,
        tag: tag_for(prop),
    }
}

fn internal(prop: &PropDef, v: &Value) -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        format!(
            "frap internal: {} cannot take the coerced value {v:?}",
            prop.identifier
        ),
    )
}

fn as_f32(prop: &PropDef, v: &Value) -> Result<f32, RnaError> {
    match *v {
        Value::Float(f) => Ok(f),
        Value::Double(d) => Ok(d as f32),
        Value::Int(i) => Ok(i as f32),
        _ => Err(internal(prop, v)),
    }
}

fn as_bool(prop: &PropDef, v: &Value) -> Result<bool, RnaError> {
    match *v {
        Value::Bool(b) => Ok(b),
        Value::Int(i @ (0 | 1)) => Ok(i == 1),
        _ => Err(internal(prop, v)),
    }
}

fn as_str<'v>(prop: &PropDef, v: &'v Value) -> Result<&'v SmolStr, RnaError> {
    match v {
        Value::Str(s) | Value::Enum(s) => Ok(s),
        _ => Err(internal(prop, v)),
    }
}

fn as_floats<'v>(prop: &PropDef, v: &'v Value) -> Result<&'v [f32], RnaError> {
    match v {
        Value::FloatArray { data, .. } => Ok(data),
        _ => Err(internal(prop, v)),
    }
}

/// Truncates a sub-data name to `max` bytes on a UTF-8 boundary.
fn clip(s: &str, max: usize) -> &str {
    frap_rna::naming::truncate_utf8(s, max)
}

// ------------------------------------------------------------------------------- NodeTree

fn tree_srna(ty: NodeTreeType) -> &'static StructDef {
    match ty {
        NodeTreeType::Shader => &frap_spec_rna::SHADER_NODE_TREE,
        NodeTreeType::Geometry => &frap_spec_rna::GEOMETRY_NODE_TREE,
    }
}

fn tree_type_str(ty: NodeTreeType) -> (&'static str, &'static str) {
    match ty {
        NodeTreeType::Shader => ("SHADER", "ShaderNodeTree"),
        NodeTreeType::Geometry => ("GEOMETRY", "GeometryNodeTree"),
    }
}

impl Rna for NodeTree {
    fn srna(&self) -> &'static StructDef {
        tree_srna(self.ty)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "name" => str_val(self.header.name.clone()),
            "name_full" => str_val(self.header.name.clone()),
            "is_embedded_data" => val(Value::Bool(self.header.is_embedded_data)),
            "use_fake_user" => val(Value::Bool(self.header.use_fake_user)),
            "session_uid" => val(Value::Int(i64::from(self.header.session_uid))),
            "library" => Ok(Field::Id(self.header.library.map(|l| l.any()))),
            "nodes" => Ok(Field::Coll(CollRef::Owned(Box::new(NodesColl {
                tree: self,
            })))),
            "links" => Ok(Field::Coll(CollRef::Owned(Box::new(LinksColl {
                tree: self,
            })))),
            "type" => val(Value::Enum(tree_type_str(self.ty).0.into())),
            "bl_idname" => str_val(tree_type_str(self.ty).1),
            "animation_data" if self.animation_data.is_none() => val(Value::None),
            id => Err(unbound(self.srna(), id)),
        }
    }
}

impl RnaMut for NodeTree {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            // ID renames re-sort `bpy.data` and need `Main` (global hook, WP 1.10).
            "name" => Ok(SetOutcome::NeedsHook),
            "use_fake_user" => {
                self.header.use_fake_user = as_bool(prop, &v)?;
                Ok(SetOutcome::Done)
            }
            id if prop.is_readonly() => Err(read_only(self.srna(), id)),
            id => Err(unbound(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(match prop.identifier {
            "animation_data" => unbound(self.srna(), prop.identifier),
            id => wrong_access(self.srna(), id, "a struct"),
        })
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "nodes" => Ok(Box::new(NodesCollMut { tree: self })),
            "links" => Ok(Box::new(LinksCollMut { tree: self })),
            id => Err(wrong_access(self.srna(), id, "a collection")),
        }
    }
}

// ---------------------------------------------------------------------------------- nodes

/// `node_tree.nodes` (keyed by node name).
pub struct NodesColl<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
}

/// Mutable `node_tree.nodes`.
pub struct NodesCollMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
}

fn nodes_key(t: &NodeTree, i: usize) -> Option<SmolStr> {
    t.nodes.get(i).map(|n| n.name.clone())
}

fn nodes_find(t: &NodeTree, key: &str) -> Option<usize> {
    t.nodes.iter().position(|n| n.name == key)
}

fn nodes_item(t: &NodeTree, i: usize) -> Result<Item<'_>, RnaError> {
    if i >= t.nodes.len() {
        return Err(index_out_of_range(i, t.nodes.len()));
    }
    Ok(Item::Struct(StructRef::Owned(Box::new(NodeRef {
        tree: t,
        i,
    }))))
}

impl RnaColl for NodesColl<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::NODE
    }
    fn len(&self) -> usize {
        self.tree.nodes.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        nodes_key(self.tree, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        nodes_find(self.tree, key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        nodes_item(self.tree, i)
    }
}

impl RnaColl for NodesCollMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::NODE
    }
    fn len(&self) -> usize {
        self.tree.nodes.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        nodes_key(self.tree, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        nodes_find(self.tree, key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        nodes_item(self.tree, i)
    }
}

impl RnaCollMut for NodesCollMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.tree.nodes.len();
        if i >= len {
            return Err(index_out_of_range(i, len));
        }
        Ok(Box::new(NodeMut {
            tree: &mut *self.tree,
            i,
        }))
    }
}

/// A node of a tree (read proxy).
pub struct NodeRef<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
    /// Index in `tree.nodes`.
    pub i: usize,
}

/// A node of a tree (write proxy).
pub struct NodeMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
    /// Index in `tree.nodes`.
    pub i: usize,
}

/// The RNA struct of a node: its type's (`ShaderNodeMix`), or `Node` for unknown types.
pub fn node_srna(n: &Node) -> &'static StructDef {
    node_type(n.bl_idname).map_or(&frap_spec_rna::NODE, |d| d.srna)
}

fn node_get<'a>(t: &'a NodeTree, i: usize, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    let n = &t.nodes[i];
    let srna = node_srna(n);
    if let Some(v) = n.props.values.get(prop.identifier) {
        return Ok(match v {
            Value::Id(id) => Field::Id(*id),
            v => Field::Value(v.clone()),
        });
    }
    match prop.identifier {
        "name" => str_val(n.name.clone()),
        "label" => str_val(n.label.as_str()),
        "bl_idname" => str_val(n.bl_idname),
        "location" => val(floats(prop, &n.location)),
        "location_absolute" => val(floats(prop, &t.location_absolute(i))),
        "width" => val(Value::Float(n.width)),
        "hide" => val(Value::Bool(n.hide)),
        "mute" => val(Value::Bool(n.mute)),
        "parent" => match n.parent.and_then(|u| t.node_index(u)) {
            Some(p) => Ok(Field::Struct(StructRef::Owned(Box::new(NodeRef {
                tree: t,
                i: p,
            })))),
            None => val(Value::None),
        },
        "inputs" | "outputs" => Ok(Field::Coll(CollRef::Owned(Box::new(SocketsColl {
            tree: t,
            node: i,
            is_output: prop.identifier == "outputs",
        })))),
        id => Err(unbound(srna, id)),
    }
}

impl Rna for NodeRef<'_> {
    fn srna(&self) -> &'static StructDef {
        node_srna(&self.tree.nodes[self.i])
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        node_get(self.tree, self.i, prop)
    }
}

impl Rna for NodeMut<'_> {
    fn srna(&self) -> &'static StructDef {
        node_srna(&self.tree.nodes[self.i])
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        node_get(self.tree, self.i, prop)
    }
}

impl RnaMut for NodeMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        let i = self.i;
        let n = &mut self.tree.nodes[i];
        if n.props.values.contains_key(prop.identifier) {
            n.set_prop(prop.identifier, v);
            return Ok(SetOutcome::Done);
        }
        match prop.identifier {
            "name" => {
                let requested = as_str(prop, &v)?.clone();
                let name = self.tree.unique_node_name(&requested, Some(i));
                self.tree.nodes[i].name = name;
            }
            "label" => n.label = clip(as_str(prop, &v)?, MAX_NODE_NAME).to_owned(),
            "location" => {
                let f = as_floats(prop, &v)?;
                if f.len() != 2 {
                    return Err(internal(prop, &v));
                }
                n.location = [f[0], f[1]];
            }
            "width" => n.width = as_f32(prop, &v)?,
            "hide" => n.hide = as_bool(prop, &v)?,
            "mute" => n.mute = as_bool(prop, &v)?,
            id if prop.is_readonly() => return Err(read_only(srna, id)),
            id => return Err(unbound(srna, id)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        match prop.identifier {
            "parent" => {
                let p = self.tree.nodes[self.i]
                    .parent
                    .and_then(|u| self.tree.node_index(u));
                match p {
                    Some(p) => Ok(Box::new(NodeMut {
                        tree: &mut *self.tree,
                        i: p,
                    })),
                    None => Err(unbound(srna, "parent")),
                }
            }
            id => Err(wrong_access(srna, id, "a struct")),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "inputs" | "outputs" => Ok(Box::new(SocketsCollMut {
                tree: &mut *self.tree,
                node: self.i,
                is_output: prop.identifier == "outputs",
            })),
            id => Err(wrong_access(self.srna(), id, "a collection")),
        }
    }
}

// -------------------------------------------------------------------------------- sockets

/// `node.inputs` / `node.outputs`: keyed by name (first **enabled** match), repr by index.
pub struct SocketsColl<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
    /// Node index in `tree.nodes`.
    pub node: usize,
    /// `outputs` rather than `inputs`.
    pub is_output: bool,
}

/// Mutable `node.inputs` / `node.outputs`.
pub struct SocketsCollMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
    /// Node index in `tree.nodes`.
    pub node: usize,
    /// `outputs` rather than `inputs`.
    pub is_output: bool,
}

fn sockets_of(t: &NodeTree, node: usize, is_output: bool) -> &[crate::Socket] {
    let n = &t.nodes[node];
    if is_output { &n.outputs } else { &n.inputs }
}

fn sockets_item(
    t: &NodeTree,
    node: usize,
    is_output: bool,
    i: usize,
) -> Result<Item<'_>, RnaError> {
    let len = sockets_of(t, node, is_output).len();
    if i >= len {
        return Err(index_out_of_range(i, len));
    }
    Ok(Item::Struct(StructRef::Owned(Box::new(SocketRef {
        tree: t,
        node,
        is_output,
        i,
    }))))
}

macro_rules! sockets_coll {
    ($t:ty) => {
        impl RnaColl for $t {
            fn item_type(&self) -> &'static StructDef {
                &frap_spec_rna::NODE_SOCKET
            }
            fn len(&self) -> usize {
                sockets_of(&self.tree, self.node, self.is_output).len()
            }
            fn key_of(&self, i: usize) -> Option<SmolStr> {
                sockets_of(&self.tree, self.node, self.is_output)
                    .get(i)
                    .map(|s| s.name.clone())
            }
            fn find(&self, key: &str) -> Option<usize> {
                crate::find_socket(sockets_of(&self.tree, self.node, self.is_output), key)
            }
            fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
                sockets_item(&self.tree, self.node, self.is_output, i)
            }
            /// Verified: `inputs['Base Color']` reprs as `inputs[0]` (F-TR-5).
            fn item_seg(&self, i: usize) -> Seg {
                Seg::Index(i as i64)
            }
        }
    };
}

sockets_coll!(SocketsColl<'_>);
sockets_coll!(SocketsCollMut<'_>);

impl RnaCollMut for SocketsCollMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = sockets_of(self.tree, self.node, self.is_output).len();
        if i >= len {
            return Err(index_out_of_range(i, len));
        }
        Ok(Box::new(SocketMut {
            tree: &mut *self.tree,
            node: self.node,
            is_output: self.is_output,
            i,
        }))
    }
}

/// A socket (read proxy).
pub struct SocketRef<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
    /// Node index in `tree.nodes`.
    pub node: usize,
    /// In `outputs` rather than `inputs`.
    pub is_output: bool,
    /// Index in `inputs`/`outputs`.
    pub i: usize,
}

/// A socket (write proxy).
pub struct SocketMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
    /// Node index in `tree.nodes`.
    pub node: usize,
    /// In `outputs` rather than `inputs`.
    pub is_output: bool,
    /// Index in `inputs`/`outputs`.
    pub i: usize,
}

/// `NodeSocket.type` of a socket type.
pub fn socket_type_str(ty: SocketType) -> &'static str {
    match ty {
        SocketType::Value => "VALUE",
        SocketType::Int => "INT",
        SocketType::Boolean => "BOOLEAN",
        SocketType::Vector => "VECTOR",
        SocketType::Rgba => "RGBA",
        SocketType::Shader => "SHADER",
        SocketType::String => "STRING",
        SocketType::Rotation => "ROTATION",
        SocketType::Matrix => "MATRIX",
        SocketType::Menu => "MENU",
        SocketType::Geometry => "GEOMETRY",
        SocketType::Object => "OBJECT",
        SocketType::Image => "IMAGE",
        SocketType::Collection => "COLLECTION",
        SocketType::Material => "MATERIAL",
        SocketType::Texture => "TEXTURE",
        _ => "CUSTOM",
    }
}

/// Socket class of a socket type without a generated definition (group sockets).
fn fallback_socket_idname(ty: SocketType) -> &'static str {
    match ty {
        SocketType::Value => "NodeSocketFloat",
        SocketType::Int => "NodeSocketInt",
        SocketType::Boolean => "NodeSocketBool",
        SocketType::Vector => "NodeSocketVector",
        SocketType::Rgba => "NodeSocketColor",
        SocketType::Shader => "NodeSocketShader",
        SocketType::String => "NodeSocketString",
        SocketType::Rotation => "NodeSocketRotation",
        SocketType::Matrix => "NodeSocketMatrix",
        SocketType::Menu => "NodeSocketMenu",
        SocketType::Geometry => "NodeSocketGeometry",
        SocketType::Object => "NodeSocketObject",
        SocketType::Image => "NodeSocketImage",
        SocketType::Collection => "NodeSocketCollection",
        SocketType::Material => "NodeSocketMaterial",
        SocketType::Texture => "NodeSocketTexture",
        _ => "NodeSocketVirtual",
    }
}

/// `socket.bl_idname`: the class of the generated socket definition (`NodeSocketFloatFactor`),
/// else the plain class of its type.
pub fn socket_bl_idname(n: &Node, is_output: bool, i: usize) -> &'static str {
    let s = if is_output {
        &n.outputs[i]
    } else {
        &n.inputs[i]
    };
    node_type(n.bl_idname)
        .and_then(|d| {
            let defs = if is_output { d.outputs } else { d.inputs };
            defs.iter()
                .find(|x| x.identifier == s.identifier && x.ty == s.ty)
        })
        .map_or_else(|| fallback_socket_idname(s.ty), |d| d.bl_idname)
}

/// The RNA struct of a socket (`NodeSocketColor`, …; `NodeSocket` if the class is unknown).
pub fn socket_srna(n: &Node, is_output: bool, i: usize) -> &'static StructDef {
    frap_spec_rna::find_struct(socket_bl_idname(n, is_output, i))
        .unwrap_or(&frap_spec_rna::NODE_SOCKET)
}

fn socket_get<'a>(
    t: &'a NodeTree,
    node: usize,
    is_output: bool,
    i: usize,
    prop: &'static PropDef,
) -> Result<Field<'a>, RnaError> {
    let n = &t.nodes[node];
    let s = if is_output {
        &n.outputs[i]
    } else {
        &n.inputs[i]
    };
    let def = node_type(n.bl_idname).and_then(|d| {
        let defs = if is_output { d.outputs } else { d.inputs };
        defs.iter().find(|x| x.identifier == s.identifier)
    });
    match prop.identifier {
        "name" => str_val(s.name.clone()),
        "identifier" => str_val(s.identifier.clone()),
        "label" | "description" => str_val(""),
        "is_output" => val(Value::Bool(is_output)),
        "hide" => val(Value::Bool(def.is_some_and(|d| d.hide))),
        "enabled" => val(Value::Bool(s.enabled)),
        "is_unavailable" => val(Value::Bool(!s.enabled)),
        "is_multi_input" => val(Value::Bool(false)),
        "hide_value" => val(Value::Bool(s.hide_value)),
        "link_limit" => val(Value::Int(i64::from(if is_output {
            OUTPUT_LINK_LIMIT
        } else {
            INPUT_LINK_LIMIT
        }))),
        "is_linked" => {
            let a = SocketAddr {
                node: n.uid,
                is_output,
                index: i,
            };
            val(Value::Bool(t.is_linked(a)))
        }
        "type" => val(Value::Enum(socket_type_str(s.ty).into())),
        "bl_idname" => str_val(socket_bl_idname(n, is_output, i)),
        "node" => Ok(Field::Struct(StructRef::Owned(Box::new(NodeRef {
            tree: t,
            i: node,
        })))),
        "default_value" => Ok(match &s.default_value {
            SocketValue::None => Field::Value(Value::None),
            SocketValue::Float(f) => Field::Value(Value::Float(*f)),
            SocketValue::Int(x) => Field::Value(Value::Int(i64::from(*x))),
            SocketValue::Bool(b) => Field::Value(Value::Bool(*b)),
            SocketValue::Vector(v) => {
                let len = prop.array_len().clamp(1, 4);
                Field::Value(floats(prop, &v[..len]))
            }
            SocketValue::Color(c) => Field::Value(floats(prop, c)),
            SocketValue::Rotation(r) => Field::Value(floats(prop, r)),
            SocketValue::String(x) => Field::Value(Value::Str(x.clone())),
            SocketValue::Menu(x) => Field::Value(Value::Enum(x.clone())),
            SocketValue::Id(id) => Field::Id(*id),
        }),
        id => Err(unbound(socket_srna(n, is_output, i), id)),
    }
}

impl Rna for SocketRef<'_> {
    fn srna(&self) -> &'static StructDef {
        socket_srna(&self.tree.nodes[self.node], self.is_output, self.i)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        socket_get(self.tree, self.node, self.is_output, self.i, prop)
    }
}

impl Rna for SocketMut<'_> {
    fn srna(&self) -> &'static StructDef {
        socket_srna(&self.tree.nodes[self.node], self.is_output, self.i)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        socket_get(self.tree, self.node, self.is_output, self.i, prop)
    }
}

impl RnaMut for SocketMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        let n = &mut self.tree.nodes[self.node];
        let s = if self.is_output {
            &mut n.outputs[self.i]
        } else {
            &mut n.inputs[self.i]
        };
        match prop.identifier {
            "name" => s.name = clip(as_str(prop, &v)?, MAX_NODE_NAME).into(),
            "enabled" => s.enabled = as_bool(prop, &v)?,
            "hide_value" => s.hide_value = as_bool(prop, &v)?,
            "default_value" => match &mut s.default_value {
                SocketValue::Float(f) => *f = as_f32(prop, &v)?,
                SocketValue::Int(x) => match v {
                    Value::Int(i) => *x = i32::try_from(i).map_err(|_| internal(prop, &v))?,
                    _ => return Err(internal(prop, &v)),
                },
                SocketValue::Bool(b) => *b = as_bool(prop, &v)?,
                SocketValue::Vector(x) => {
                    let f = as_floats(prop, &v)?;
                    if f.len() > 4 {
                        return Err(internal(prop, &v));
                    }
                    x[..f.len()].copy_from_slice(f);
                }
                SocketValue::Color(x) => {
                    let f = as_floats(prop, &v)?;
                    if f.len() != 4 {
                        return Err(internal(prop, &v));
                    }
                    x.copy_from_slice(f);
                }
                SocketValue::Rotation(x) => {
                    let f = as_floats(prop, &v)?;
                    if f.len() != 3 {
                        return Err(internal(prop, &v));
                    }
                    x.copy_from_slice(f);
                }
                SocketValue::String(x) | SocketValue::Menu(x) => *x = as_str(prop, &v)?.clone(),
                SocketValue::Id(x) => match v {
                    Value::Id(id) => *x = id,
                    Value::None => *x = None,
                    _ => return Err(internal(prop, &v)),
                },
                SocketValue::None => return Err(no_default_value(srna)),
            },
            id if prop.is_readonly() => return Err(read_only(srna, id)),
            id => return Err(unbound(srna, id)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let i = self.node;
        match prop.identifier {
            "node" => Ok(Box::new(NodeMut {
                tree: &mut *self.tree,
                i,
            })),
            id => Err(wrong_access(self.srna(), id, "a struct")),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(wrong_access(self.srna(), prop.identifier, "a collection"))
    }
}

fn no_default_value(srna: &'static StructDef) -> RnaError {
    frap_rna::traits::no_attribute(srna, "default_value")
}

// ---------------------------------------------------------------------------------- links

/// `node_tree.links` (nameless: repr by index).
pub struct LinksColl<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
}

/// Mutable `node_tree.links` (links are read-only structs: writes go through the tree API).
pub struct LinksCollMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
}

fn links_item(t: &NodeTree, i: usize) -> Result<Item<'_>, RnaError> {
    if i >= t.links.len() {
        return Err(index_out_of_range(i, t.links.len()));
    }
    Ok(Item::Struct(StructRef::Owned(Box::new(LinkRef {
        tree: t,
        i,
    }))))
}

macro_rules! links_coll {
    ($t:ty) => {
        impl RnaColl for $t {
            fn item_type(&self) -> &'static StructDef {
                &frap_spec_rna::NODE_LINK
            }
            fn len(&self) -> usize {
                self.tree.links.len()
            }
            fn key_of(&self, _: usize) -> Option<SmolStr> {
                None
            }
            fn find(&self, _: &str) -> Option<usize> {
                None
            }
            fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
                links_item(&self.tree, i)
            }
        }
    };
}

links_coll!(LinksColl<'_>);
links_coll!(LinksCollMut<'_>);

impl RnaCollMut for LinksCollMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.tree.links.len();
        if i >= len {
            return Err(index_out_of_range(i, len));
        }
        Ok(Box::new(LinkMut {
            tree: &mut *self.tree,
            i,
        }))
    }
}

/// A link (read proxy).
pub struct LinkRef<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
    /// Index in `tree.links`.
    pub i: usize,
}

/// A link (write proxy: every stored link property is read-only or not modelled).
pub struct LinkMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
    /// Index in `tree.links`.
    pub i: usize,
}

fn link_end(t: &NodeTree, l: &Link, output: bool) -> Option<(usize, usize)> {
    let (uid, id) = if output {
        (l.from_node, &l.from_socket)
    } else {
        (l.to_node, &l.to_socket)
    };
    let ni = t.node_index(uid)?;
    let n = &t.nodes[ni];
    let si = if output {
        n.output_by_identifier(id)?
    } else {
        n.input_by_identifier(id)?
    };
    Some((ni, si))
}

fn link_get<'a>(t: &'a NodeTree, i: usize, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    let l = &t.links[i];
    let end = |output: bool| -> Result<Field<'a>, RnaError> {
        let (ni, si) = link_end(t, l, output)
            .ok_or_else(|| unbound(&frap_spec_rna::NODE_LINK, prop.identifier))?;
        Ok(match prop.identifier {
            "from_node" | "to_node" => {
                Field::Struct(StructRef::Owned(Box::new(NodeRef { tree: t, i: ni })))
            }
            _ => Field::Struct(StructRef::Owned(Box::new(SocketRef {
                tree: t,
                node: ni,
                is_output: output,
                i: si,
            }))),
        })
    };
    match prop.identifier {
        "from_node" | "from_socket" => end(true),
        "to_node" | "to_socket" => end(false),
        "is_valid" => val(Value::Bool(t.link_is_valid(l))),
        "is_muted" => val(Value::Bool(false)),
        "is_hidden" => {
            let enabled = |output: bool| {
                link_end(t, l, output)
                    .and_then(|(ni, si)| t.nodes[ni].socket(output, si))
                    .is_some_and(|s| s.enabled)
            };
            val(Value::Bool(!(enabled(true) && enabled(false))))
        }
        "multi_input_sort_id" => val(Value::Int(0)),
        id => Err(unbound(&frap_spec_rna::NODE_LINK, id)),
    }
}

impl Rna for LinkRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::NODE_LINK
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        link_get(self.tree, self.i, prop)
    }
}

impl Rna for LinkMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::NODE_LINK
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        link_get(self.tree, self.i, prop)
    }
}

impl RnaMut for LinkMut<'_> {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        if prop.is_readonly() {
            Err(read_only(&frap_spec_rna::NODE_LINK, prop.identifier))
        } else {
            Err(unbound(&frap_spec_rna::NODE_LINK, prop.identifier))
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let l = self.tree.links[self.i].clone();
        let output = matches!(prop.identifier, "from_node" | "from_socket");
        let Some((ni, si)) = link_end(self.tree, &l, output) else {
            return Err(unbound(&frap_spec_rna::NODE_LINK, prop.identifier));
        };
        match prop.identifier {
            "from_node" | "to_node" => Ok(Box::new(NodeMut {
                tree: &mut *self.tree,
                i: ni,
            })),
            "from_socket" | "to_socket" => Ok(Box::new(SocketMut {
                tree: &mut *self.tree,
                node: ni,
                is_output: output,
                i: si,
            })),
            id => Err(wrong_access(&frap_spec_rna::NODE_LINK, id, "a struct")),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(wrong_access(
            &frap_spec_rna::NODE_LINK,
            prop.identifier,
            "a collection",
        ))
    }
}

#[cfg(test)]
mod tests;
