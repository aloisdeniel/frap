// SPDX-License-Identifier: GPL-3.0-or-later
//! `crates/frap-nodes/src/generated/**` (ARCHITECTURE §4.2, §16.1): one `NodeTypeDef` per dumped
//! shader node `bl_idname` (`types/<snake>.rs`) from `spec/blender-5.2/nodes/<bl_idname>.json`:
//! default name and width, input/output sockets with their defaults, `enabled` and `hide_value`
//! on a fresh node, and the fresh values of the node's own properties. Mode-dependent socket
//! availability stays in the dump and in `frap-nodes/src/availability/<node>.rs`.

use std::fmt::Write as _;

use serde_json::Value as J;

use super::Files;
use super::model::{Res, Spec, jf64};
use super::rust::{header, lit_f32, lit_str, screaming, snake};

pub const DIR: &str = "crates/frap-nodes/src/generated";

fn socket_type(t: &str) -> Res<&'static str> {
    Ok(match t {
        "VALUE" => "Value",
        "INT" => "Int",
        "BOOLEAN" => "Boolean",
        "VECTOR" => "Vector",
        "RGBA" => "Rgba",
        "SHADER" => "Shader",
        "STRING" => "String",
        "ROTATION" => "Rotation",
        "MATRIX" => "Matrix",
        "MENU" => "Menu",
        "GEOMETRY" => "Geometry",
        "OBJECT" => "Object",
        "IMAGE" => "Image",
        "COLLECTION" => "Collection",
        "MATERIAL" => "Material",
        "TEXTURE" => "Texture",
        "CUSTOM" => "Custom",
        other => {
            return Err(format!(
                "unknown socket type {other} (frap_nodes::SocketType)"
            ));
        }
    })
}

fn floats(j: &J) -> Res<Vec<String>> {
    j.as_array()
        .ok_or("not an array")?
        .iter()
        .map(|x| jf64(x).map(lit_f32))
        .collect()
}

fn socket_default(s: &J) -> Res<String> {
    let Some(v) = s.get("default_value") else {
        return Ok("SocketDefault::None".into());
    };
    let ty = s["default_value_type"].as_str().unwrap_or("");
    let sub = s["default_value_subtype"].as_str().unwrap_or("");
    Ok(match (ty, v) {
        ("FLOAT", J::Array(a)) if a.len() == 4 && sub.starts_with("COLOR") => {
            format!("SocketDefault::Color([{}])", floats(v)?.join(", "))
        }
        ("FLOAT", J::Array(_)) => format!("SocketDefault::Vector(&[{}])", floats(v)?.join(", ")),
        ("FLOAT", _) => format!("SocketDefault::Float({})", lit_f32(jf64(v)?)),
        ("INT", J::Number(n)) => format!("SocketDefault::Int({})", n.as_i64().unwrap_or(0)),
        ("BOOLEAN", J::Bool(b)) => format!("SocketDefault::Bool({b})"),
        ("STRING", J::String(x)) => format!("SocketDefault::Str({})", lit_str(x)),
        ("ENUM", J::String(x)) => format!("SocketDefault::Menu({})", lit_str(x)),
        _ => "SocketDefault::None".into(),
    })
}

fn prop_default(v: &J) -> Option<String> {
    Some(match v {
        J::Bool(b) => format!("NodePropDefault::Bool({b})"),
        J::Number(n) if n.is_i64() => format!("NodePropDefault::Int({})", n.as_i64()?),
        J::Number(n) => format!("NodePropDefault::Float({})", lit_f32(n.as_f64()?)),
        J::String(s) => format!("NodePropDefault::Str({})", lit_str(s)),
        J::Array(a) if a.iter().all(|x| x.is_number()) => format!(
            "NodePropDefault::Floats(&[{}])",
            a.iter()
                .map(|x| lit_f32(x.as_f64().unwrap_or(0.0)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        J::Array(a) if a.iter().all(|x| x.is_string()) => format!(
            "NodePropDefault::Flags(&[{}])",
            a.iter()
                .map(|x| lit_str(x.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        J::Null => "NodePropDefault::None".into(),
        _ => return None,
    })
}

const DEFS: &str = r#"
use frap_rna::StructDef;

use crate::SocketType;

/// A node type (`bpy.types.<bl_idname>` as created by `node_tree.nodes.new(type=…)`).
#[derive(Debug)]
pub struct NodeTypeDef {
    /// `bl_idname` ("ShaderNodeBsdfPrincipled").
    pub bl_idname: &'static str,
    /// RNA struct of the node.
    pub srna: &'static StructDef,
    /// Default node name ("Principled BSDF").
    pub name: &'static str,
    /// Default `width`.
    pub width: f32,
    /// Input sockets, in `node.inputs` order.
    pub inputs: &'static [SocketDef],
    /// Output sockets, in `node.outputs` order.
    pub outputs: &'static [SocketDef],
    /// Fresh values of the node's own (non read-only) properties, in RNA order.
    pub props: &'static [(&'static str, NodePropDefault)],
}

/// A socket of a fresh node.
#[derive(Debug)]
pub struct SocketDef {
    /// `name` ("Base Color").
    pub name: &'static str,
    /// `identifier` (unique per direction; "Base Color", "Value_001", …).
    pub identifier: &'static str,
    /// Socket `bl_idname` ("NodeSocketColor").
    pub bl_idname: &'static str,
    /// `type`.
    pub ty: SocketType,
    /// `default_value` of a fresh node.
    pub default: SocketDefault,
    /// `enabled` on a fresh node (mode-dependent sockets: `availability/<node>.rs`).
    pub enabled: bool,
    /// `hide`
    pub hide: bool,
    /// `hide_value`
    pub hide_value: bool,
}

/// `default_value` of a socket.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SocketDefault {
    /// The socket has no `default_value` (shader, geometry, image pointer, …).
    None,
    /// FLOAT
    Float(f32),
    /// INT
    Int(i64),
    /// BOOLEAN
    Bool(bool),
    /// FLOAT vector (2, 3 or 4 components; rotations are Euler).
    Vector(&'static [f32]),
    /// RGBA colour.
    Color([f32; 4]),
    /// STRING
    Str(&'static str),
    /// MENU item identifier.
    Menu(&'static str),
}

/// Fresh value of a node property.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodePropDefault {
    /// `None` pointer.
    None,
    /// BOOLEAN
    Bool(bool),
    /// INT
    Int(i64),
    /// FLOAT
    Float(f32),
    /// FLOAT array
    Floats(&'static [f32]),
    /// STRING or ENUM identifier.
    Str(&'static str),
    /// ENUM flag set.
    Flags(&'static [&'static str]),
}

impl NodeTypeDef {
    /// Input socket by identifier.
    pub fn input(&self, identifier: &str) -> Option<&'static SocketDef> {
        self.inputs.iter().find(|s| s.identifier == identifier)
    }
    /// Output socket by identifier.
    pub fn output(&self, identifier: &str) -> Option<&'static SocketDef> {
        self.outputs.iter().find(|s| s.identifier == identifier)
    }
}

/// Looks a node type up by `bl_idname`.
pub fn node_type(bl_idname: &str) -> Option<&'static NodeTypeDef> {
    NODE_TYPES
        .binary_search_by(|t| t.bl_idname.cmp(bl_idname))
        .ok()
        .map(|i| NODE_TYPES[i])
}
"#;

pub fn generate(spec: &Spec, files: &mut Files) -> Res<()> {
    let dir = spec.root.join(DIR);
    let mut m = header(
        "Generated node-type table of `frap-nodes` (ARCHITECTURE §16.1): one `NodeTypeDef` per\n//! dumped `bl_idname`.",
    );
    m.push_str("#![allow(clippy::all)]\n");
    m.push_str(DEFS);
    m.push_str("\n/// One module per node type.\npub mod types {\n");
    let mut all = Vec::new();
    for n in &spec.nodes {
        let bl = n["bl_idname"].as_str().ok_or("node without bl_idname")?;
        let srna = n["srna"].as_str().unwrap_or(bl);
        let st = spec
            .structs
            .get(srna)
            .ok_or_else(|| format!("node {bl}: struct {srna} not dumped"))?;
        let module = snake(bl);
        let up = screaming(bl);
        let mut s = header(&format!("`{bl}` ({})", n["name"].as_str().unwrap_or("")));
        s.push_str("#![allow(unused_imports, clippy::all)]\nuse crate::SocketType;\nuse crate::generated::{NodePropDefault, NodeTypeDef, SocketDef, SocketDefault};\n\n");
        let sockets = |key: &str| -> Res<String> {
            let mut out = String::new();
            for sk in n[key].as_array().into_iter().flatten() {
                let _ = writeln!(
                    out,
                    "    SocketDef {{ name: {}, identifier: {}, bl_idname: {}, ty: SocketType::{}, default: {}, enabled: {}, hide: {}, hide_value: {} }},",
                    lit_str(sk["name"].as_str().unwrap_or("")),
                    lit_str(sk["identifier"].as_str().unwrap_or("")),
                    lit_str(sk["bl_idname"].as_str().unwrap_or("")),
                    socket_type(sk["type"].as_str().unwrap_or(""))?,
                    socket_default(sk)?,
                    sk["enabled"].as_bool().unwrap_or(true),
                    sk["hide"].as_bool().unwrap_or(false),
                    sk["hide_value"].as_bool().unwrap_or(false),
                );
            }
            Ok(out)
        };
        let n_in = n["inputs"].as_array().map_or(0, |a| a.len());
        let n_out = n["outputs"].as_array().map_or(0, |a| a.len());
        let _ = write!(
            s,
            "static INPUTS: [SocketDef; {n_in}] = [\n{}];\n",
            sockets("inputs")?
        );
        let _ = write!(
            s,
            "static OUTPUTS: [SocketDef; {n_out}] = [\n{}];\n",
            sockets("outputs")?
        );
        let mut props = Vec::new();
        for p in &st.props {
            if p.has_flag("READONLY") {
                continue;
            }
            if let Some(v) = n["props"].get(&p.identifier)
                && let Some(d) = prop_default(v)
            {
                props.push(format!("    ({}, {d}),", lit_str(&p.identifier)));
            }
        }
        let _ = write!(
            s,
            "static PROPS: [(&str, NodePropDefault); {}] = [\n{}{}];\n",
            props.len(),
            props.join("\n"),
            if props.is_empty() { "" } else { "\n" }
        );
        let _ = writeln!(
            s,
            "/// `{bl}`\npub static {up}: NodeTypeDef = NodeTypeDef {{\n    bl_idname: {},\n    srna: &frap_spec_rna::{},\n    name: {},\n    width: {},\n    inputs: &INPUTS,\n    outputs: &OUTPUTS,\n    props: &PROPS,\n}};",
            lit_str(bl),
            screaming(srna),
            lit_str(n["name"].as_str().unwrap_or("")),
            lit_f32(jf64(&n["width"])?)
        );
        files.insert(dir.join(format!("types/{module}.rs")), s.into_bytes());
        let _ = writeln!(m, "    pub mod {module};");
        all.push(format!("&types::{module}::{up}"));
    }
    m.push_str("}\n\n");
    let _ = writeln!(
        m,
        "/// Every dumped node type, sorted by `bl_idname`.\npub static NODE_TYPES: [&NodeTypeDef; {}] = [\n    {},\n];",
        all.len(),
        all.join(",\n    ")
    );
    m.push_str("\n#[cfg(test)]\nmod tests;\n");
    files.insert(dir.join("mod.rs"), m.into_bytes());
    let mut t =
        header("Generated test: every `NodeTypeDef` equals its dump (`spec/blender-5.2/nodes/`).");
    t.push_str(NODE_TESTS);
    files.insert(dir.join("tests.rs"), t.into_bytes());
    Ok(())
}

const NODE_TESTS: &str = r#"#![allow(clippy::all)]
use super::*;
use serde_json::Value as J;

fn f(j: &J) -> f32 {
    j.as_f64().unwrap() as f32
}

#[test]
fn node_types_match_dumps() {
    let dir = format!("{}/../../spec/blender-5.2/nodes", env!("CARGO_MANIFEST_DIR"));
    let meta: J = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/_meta.json")).unwrap()).unwrap();
    let names = meta["node_types"].as_array().unwrap();
    assert_eq!(names.len(), NODE_TYPES.len());
    for name in names {
        let bl = name.as_str().unwrap();
        let d: J = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{bl}.json")).unwrap()).unwrap();
        let t = node_type(bl).unwrap_or_else(|| panic!("{bl} missing"));
        assert!(std::ptr::eq(t, node_type(bl).unwrap()));
        assert_eq!(t.name, d["name"].as_str().unwrap(), "{bl}");
        assert_eq!(t.width, f(&d["width"]), "{bl}");
        assert_eq!(t.srna.identifier, d["srna"].as_str().unwrap(), "{bl}");
        for (key, socks) in [("inputs", t.inputs), ("outputs", t.outputs)] {
            let js = d[key].as_array().unwrap();
            assert_eq!(js.len(), socks.len(), "{bl}.{key}");
            for (s, j) in socks.iter().zip(js) {
                let ctx = format!("{bl}.{key}[{}]", s.identifier);
                assert_eq!(s.name, j["name"].as_str().unwrap(), "{ctx}");
                assert_eq!(s.identifier, j["identifier"].as_str().unwrap(), "{ctx}");
                assert_eq!(s.bl_idname, j["bl_idname"].as_str().unwrap(), "{ctx}");
                assert_eq!(s.enabled, j["enabled"].as_bool().unwrap(), "{ctx}");
                assert_eq!(s.hide_value, j["hide_value"].as_bool().unwrap(), "{ctx}");
                match (s.default, j.get("default_value")) {
                    (SocketDefault::None, v) => assert!(v.is_none() || !v.unwrap().is_number() && !v.unwrap().is_boolean(), "{ctx}"),
                    (SocketDefault::Float(x), Some(v)) => assert_eq!(x, f(v), "{ctx}"),
                    (SocketDefault::Int(x), Some(v)) => assert_eq!(x, v.as_i64().unwrap(), "{ctx}"),
                    (SocketDefault::Bool(x), Some(v)) => assert_eq!(x, v.as_bool().unwrap(), "{ctx}"),
                    (SocketDefault::Vector(x), Some(v)) => {
                        assert_eq!(x.to_vec(), v.as_array().unwrap().iter().map(f).collect::<Vec<_>>(), "{ctx}")
                    }
                    (SocketDefault::Color(x), Some(v)) => {
                        assert_eq!(x.to_vec(), v.as_array().unwrap().iter().map(f).collect::<Vec<_>>(), "{ctx}")
                    }
                    (SocketDefault::Str(x) | SocketDefault::Menu(x), Some(v)) => assert_eq!(x, v.as_str().unwrap(), "{ctx}"),
                    (other, v) => panic!("{ctx}: {other:?} vs {v:?}"),
                }
            }
        }
        for (prop, v) in t.props {
            let want = &d["props"][*prop];
            let ctx = format!("{bl}.{prop}");
            match v {
                NodePropDefault::None => assert!(want.is_null(), "{ctx}"),
                NodePropDefault::Bool(b) => assert_eq!(*b, want.as_bool().unwrap(), "{ctx}"),
                NodePropDefault::Int(i) => assert_eq!(*i, want.as_i64().unwrap(), "{ctx}"),
                NodePropDefault::Float(x) => assert_eq!(*x, f(want), "{ctx}"),
                NodePropDefault::Floats(x) => assert_eq!(x.to_vec(), want.as_array().unwrap().iter().map(f).collect::<Vec<_>>(), "{ctx}"),
                NodePropDefault::Str(x) => assert_eq!(*x, want.as_str().unwrap(), "{ctx}"),
                NodePropDefault::Flags(x) => assert_eq!(x.to_vec(), want.as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect::<Vec<_>>(), "{ctx}"),
            }
            assert!(t.srna.find_prop(prop).is_some(), "{ctx}: not an RNA prop");
        }
    }
    let p = node_type("ShaderNodeBsdfPrincipled").unwrap();
    assert_eq!(p.name, "Principled BSDF");
    assert_eq!(p.input("Base Color").unwrap().default, SocketDefault::Color([0.8, 0.8, 0.8, 1.0]));
    assert_eq!(p.input("Metallic").unwrap().default, SocketDefault::Float(0.0));
}
"#;
