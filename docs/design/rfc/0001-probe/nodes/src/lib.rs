// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-nodes` (§16.1) and its RNA bindings.
//!
//! Contract deviation used here (RFC 0001 F-NODE-1): `Socket.name` / `Socket.identifier` /
//! `Link.from_socket` / `Link.to_socket` are `SmolStr`, not `&'static str`: group-node sockets
//! take their names from the group interface (user data: `interface.new_socket(name=…)`), and
//! `.frap` deserialisation cannot produce `&'static str` without leaking.
#![forbid(unsafe_code)]

use frap_probe_rna::*;
use frap_probe_spec as spec;
use smol_str::SmolStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeTreeType {
    Shader,
    Geometry,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SocketType {
    Float,
    Vector,
    Rgba,
    Shader,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SocketValue {
    None,
    Float(f32),
    Vector([f32; 3]),
    Rgba([f32; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    pub name: SmolStr,
    pub identifier: SmolStr,
    pub ty: SocketType,
    pub enabled: bool,
    pub hide_value: bool,
    pub default_value: SocketValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub uid: u32,
    pub name: SmolStr,
    pub label: String,
    pub bl_idname: &'static str,
    pub location: [f32; 2],
    pub width: f32,
    pub inputs: Vec<Socket>,
    pub outputs: Vec<Socket>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub from_node: u32,
    pub from_socket: SmolStr,
    pub to_node: u32,
    pub to_socket: SmolStr,
}

#[derive(Clone, Debug)]
pub struct NodeTree {
    pub header: IdHeader,
    pub ty: NodeTreeType,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub next_node_uid: u32,
}

fn sock(name: &str, identifier: &str, ty: SocketType, v: SocketValue) -> Socket {
    Socket {
        name: name.into(),
        identifier: identifier.into(),
        ty,
        enabled: true,
        hide_value: false,
        default_value: v,
    }
}

impl NodeTree {
    /// `bpy.data.materials.new()` graph [RNA §0.8]: Principled BSDF (−200,100) -> Material Output.
    /// Sockets: the first five Principled inputs in 5.2 order (probe subset).
    pub fn new_material_tree() -> NodeTree {
        let mut header = IdHeader::named("Shader Nodetree");
        header.is_embedded_data = true;
        let principled = Node {
            uid: 1,
            name: "Principled BSDF".into(),
            label: String::new(),
            bl_idname: "ShaderNodeBsdfPrincipled",
            location: [-200.0, 100.0],
            width: 240.0,
            inputs: vec![
                sock(
                    "Base Color",
                    "Base Color",
                    SocketType::Rgba,
                    SocketValue::Rgba([0.8, 0.8, 0.8, 1.0]),
                ),
                sock("Metallic", "Metallic", SocketType::Float, SocketValue::Float(0.0)),
                sock("Roughness", "Roughness", SocketType::Float, SocketValue::Float(0.5)),
                sock("IOR", "IOR", SocketType::Float, SocketValue::Float(1.5)),
                sock("Alpha", "Alpha", SocketType::Float, SocketValue::Float(1.0)),
            ],
            outputs: vec![sock("BSDF", "BSDF", SocketType::Shader, SocketValue::None)],
        };
        let output = Node {
            uid: 2,
            name: "Material Output".into(),
            label: String::new(),
            bl_idname: "ShaderNodeOutputMaterial",
            location: [200.0, 100.0],
            width: 140.0,
            inputs: vec![sock("Surface", "Surface", SocketType::Shader, SocketValue::None)],
            outputs: vec![],
        };
        NodeTree {
            header,
            ty: NodeTreeType::Shader,
            nodes: vec![principled, output],
            links: vec![Link {
                from_node: 1,
                from_socket: "BSDF".into(),
                to_node: 2,
                to_socket: "Surface".into(),
            }],
            next_node_uid: 3,
        }
    }

    /// A Mix node whose `A` exists three times with different identifiers (first-enabled lookup).
    pub fn add_mix(&mut self, data_type_rgba: bool) {
        let uid = self.next_node_uid;
        self.next_node_uid += 1;
        let mut a_f = sock("A", "A_Float", SocketType::Float, SocketValue::Float(0.5));
        let mut a_v = sock("A", "A_Vector", SocketType::Vector, SocketValue::Vector([0.5; 3]));
        let mut a_c = sock("A", "A_Color", SocketType::Rgba, SocketValue::Rgba([0.5, 0.5, 0.5, 1.0]));
        a_f.enabled = true;
        a_v.enabled = false;
        a_c.enabled = data_type_rgba;
        if data_type_rgba {
            a_f.enabled = false;
        }
        self.nodes.push(Node {
            uid,
            name: "Mix".into(),
            label: String::new(),
            bl_idname: "ShaderNodeMix",
            location: [0.0, 0.0],
            width: 140.0,
            inputs: vec![
                sock("Factor", "Factor_Float", SocketType::Float, SocketValue::Float(0.5)),
                a_f,
                a_v,
                a_c,
            ],
            outputs: vec![],
        });
    }
}

// ------------------------------------------------------------------------------ RNA

fn node_srna(bl_idname: &str) -> &'static StructDef {
    spec::generated::rna::ALL
        .iter()
        .copied()
        .find(|s| s.identifier == bl_idname)
        .unwrap_or(&spec::NODE)
}

fn socket_srna(ty: SocketType) -> &'static StructDef {
    match ty {
        SocketType::Rgba => &spec::NODE_SOCKET_COLOR,
        // Float/Vector/Shader socket structs are not in the WP 0.2 dump subset.
        _ => &spec::NODE_SOCKET,
    }
}

impl Rna for NodeTree {
    fn srna(&self) -> &'static StructDef {
        match self.ty {
            NodeTreeType::Shader => &spec::SHADER_NODE_TREE,
            NodeTreeType::Geometry => &spec::NODE_TREE,
        }
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "nodes" => Ok(Field::Coll(CollRef::Owned(Box::new(Nodes { nodes: &self.nodes })))),
            "name" => Ok(Field::Value(Value::Str(self.header.name.clone()))),
            "is_embedded_data" => Ok(Field::Value(Value::Bool(self.header.is_embedded_data))),
            id => Err(unbound(self.srna(), id)),
        }
    }
}
impl RnaMut for NodeTree {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        if prop.identifier == "name" {
            return Ok(SetOutcome::NeedsHook);
        }
        Err(unbound(self.srna(), prop.identifier))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "nodes" => Ok(Box::new(NodesMut { nodes: &mut self.nodes })),
            id => Err(unbound(self.srna(), id)),
        }
    }
}

pub struct Nodes<'a> {
    pub nodes: &'a [Node],
}
pub struct NodesMut<'a> {
    pub nodes: &'a mut Vec<Node>,
}

impl RnaColl for Nodes<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::NODE
    }
    fn len(&self) -> usize {
        self.nodes.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        Some(self.nodes[i].name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == key)
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Borrowed(&self.nodes[i]))
    }
}
impl RnaColl for NodesMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::NODE
    }
    fn len(&self) -> usize {
        self.nodes.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        Some(self.nodes[i].name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == key)
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Borrowed(&self.nodes[i]))
    }
}
impl RnaCollMut for NodesMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        // In-memory struct: the guard is a boxed reborrow (needs the blanket impl, F-TR-6).
        Ok(Box::new(&mut self.nodes[i]))
    }
}

impl Rna for Node {
    fn srna(&self) -> &'static StructDef {
        node_srna(self.bl_idname)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(self.name.clone()))),
            "location" => Ok(Field::Value(Value::FloatArray {
                data: small_f32(&self.location),
                shape: ArrayShape::Fixed(2),
                tag: math_tag(prop),
            })),
            "inputs" => Ok(Field::Coll(CollRef::Owned(Box::new(Sockets { sockets: &self.inputs })))),
            "outputs" => Ok(Field::Coll(CollRef::Owned(Box::new(Sockets { sockets: &self.outputs })))),
            id => Err(unbound(self.srna(), id)),
        }
    }
}
impl RnaMut for Node {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "name" => Ok(SetOutcome::NeedsHook),
            "location" => {
                self.location = f32_array::<2>(&v)?;
                Ok(SetOutcome::Done)
            }
            id => Err(unbound(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "inputs" => Ok(Box::new(SocketsMut { sockets: &mut self.inputs })),
            "outputs" => Ok(Box::new(SocketsMut {
                sockets: &mut self.outputs,
            })),
            id => Err(unbound(self.srna(), id)),
        }
    }
}

pub struct Sockets<'a> {
    pub sockets: &'a [Socket],
}
pub struct SocketsMut<'a> {
    pub sockets: &'a mut Vec<Socket>,
}

/// Socket lookup by name returns the FIRST ENABLED socket; identifiers are not keys (§16.1).
fn first_enabled(s: &[Socket], key: &str) -> Option<usize> {
    s.iter().position(|x| x.enabled && x.name == key)
}

impl RnaColl for Sockets<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::NODE_SOCKET
    }
    fn len(&self) -> usize {
        self.sockets.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        Some(self.sockets[i].name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        first_enabled(self.sockets, key)
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Borrowed(&self.sockets[i]))
    }
    /// Verified: `inputs['Base Color']` reprs as `inputs[0]` (F-TR-5).
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}
impl RnaColl for SocketsMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::NODE_SOCKET
    }
    fn len(&self) -> usize {
        self.sockets.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        Some(self.sockets[i].name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        first_enabled(self.sockets, key)
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Borrowed(&self.sockets[i]))
    }
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}
impl RnaCollMut for SocketsMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        Ok(Box::new(&mut self.sockets[i]))
    }
}

impl Rna for Socket {
    fn srna(&self) -> &'static StructDef {
        socket_srna(self.ty)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(self.name.clone()))),
            "identifier" => Ok(Field::Value(Value::Str(self.identifier.clone()))),
            "enabled" => Ok(Field::Value(Value::Bool(self.enabled))),
            "default_value" => Ok(Field::Value(match &self.default_value {
                SocketValue::Rgba(c) => Value::FloatArray {
                    data: small_f32(c),
                    shape: ArrayShape::Fixed(4),
                    tag: math_tag(prop),
                },
                SocketValue::Vector(v) => Value::float3(*v, MathTag::Vector),
                SocketValue::Float(f) => Value::Float(*f),
                SocketValue::None => Value::None,
            })),
            id => Err(unbound(self.srna(), id)),
        }
    }
}
impl RnaMut for Socket {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match (prop.identifier, &mut self.default_value) {
            ("default_value", SocketValue::Rgba(c)) => {
                *c = f32_array::<4>(&v)?;
                Ok(SetOutcome::Done)
            }
            ("default_value", SocketValue::Float(f)) => match v {
                Value::Float(x) => {
                    *f = x;
                    Ok(SetOutcome::Done)
                }
                _ => Err(bad_value("default_value")),
            },
            (id, _) => Err(unbound(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_enabled_socket_lookup() {
        for rgba in [false, true] {
            let mut t = NodeTree::new_material_tree();
            t.add_mix(rgba);
            let mix = t.nodes.iter().find(|n| n.name == "Mix").unwrap();
            let s = Sockets { sockets: &mix.inputs };
            let i = s.find("A").unwrap();
            assert_eq!(mix.inputs[i].identifier, if rgba { "A_Color" } else { "A_Float" });
            assert_eq!(s.find("A_Color"), None, "identifiers are not keys");
        }
    }

    #[test]
    fn socket_set_through_boxed_reborrow_guard() {
        let mut t = NodeTree::new_material_tree();
        {
            let mut nodes = t.coll_mut(spec::NODE_TREE_NODES).unwrap();
            let i = nodes.find("Principled BSDF").unwrap();
            let mut node = nodes.item_mut(i).unwrap();
            assert_eq!(node.srna().identifier, "ShaderNodeBsdfPrincipled");
            let mut inputs = node.coll_mut(spec::NODE_INPUTS).unwrap();
            let j = inputs.find("Base Color").unwrap();
            assert_eq!(inputs.item_seg(j), Seg::Index(0));
            let mut s = inputs.item_mut(j).unwrap();
            assert_eq!(s.srna().identifier, "NodeSocketColor");
            let p = s.srna().find_prop("default_value").unwrap();
            s.set(
                p,
                Value::FloatArray {
                    data: small_f32(&[1.0, 0.0, 0.0, 1.0]),
                    shape: ArrayShape::Fixed(4),
                    tag: MathTag::None,
                },
            )
            .unwrap();
        }
        assert_eq!(t.nodes[0].inputs[0].default_value, SocketValue::Rgba([1.0, 0.0, 0.0, 1.0]));
    }
}
