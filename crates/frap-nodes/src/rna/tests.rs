// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.11 acceptance tests of `frap-nodes`. Every expectation below was verified on Blender
//! 5.2.4 (`--factory-startup`, scripts in the WP 1.11 report).

use frap_rna::{
    ArrayShape, EmbeddedSlot, Field, IdAny, IdKey, IdNames, Item, MaterialId, MathTag, PropKind,
    PyExc, Rna, RnaColl, RnaError, RnaErrorKind, RnaMut, RnaPtr, Root, Seg, Value,
};
use smol_str::SmolStr;

use crate::generated::{NODE_TYPES, node_type};
use crate::{Link, NodeTree, NodeTreeType, SocketAddr, SocketType, SocketValue, WORLD_TREE_NAME};

// ----------------------------------------------------------------- a minimal path resolver

enum Tok {
    P(&'static str),
    K(&'static str),
    I(usize),
}
use Tok::{I, K, P};

/// Walks `toks` from `s`, recording the canonical segments the session resolver stores
/// (F-TR-5), and returns the final value (if the path ends on a value).
fn walk(s: &dyn Rna, toks: &[Tok], ptr: RnaPtr) -> (RnaPtr, Option<Value>) {
    let Some(P(p)) = toks.first() else {
        assert!(toks.is_empty(), "a struct is followed by a property");
        return (ptr, None);
    };
    let prop = s
        .srna()
        .find_prop(p)
        .unwrap_or_else(|| panic!("{}.{p} is not an RNA property", s.srna().identifier));
    let here = ptr.child(Seg::Prop(prop.identifier), s.srna());
    match s.get(prop).expect("get") {
        Field::Value(v) => {
            assert!(toks.len() == 1);
            (here, Some(v))
        }
        Field::Struct(r) => {
            let srna = r.srna();
            walk(&*r, &toks[1..], RnaPtr { srna, ..here })
        }
        Field::Coll(c) => {
            let i = match &toks[1] {
                K(k) => c.find(k).unwrap_or_else(|| panic!("KeyError {k}")),
                I(i) => *i,
                P(_) => panic!("collection followed by a property"),
            };
            let seg = c.item_seg(i);
            let Item::Struct(r) = c.item(i).expect("item") else {
                panic!("struct item expected")
            };
            let srna = r.srna();
            walk(&*r, &toks[2..], here.child(seg, srna))
        }
        _ => panic!("unexpected field"),
    }
}

fn set_path(s: &mut dyn RnaMut, toks: &[Tok], v: Value) -> Result<(), RnaError> {
    let Some(P(p)) = toks.first() else {
        panic!("path must start with a property")
    };
    let prop = s.srna().find_prop(p).expect("RNA property");
    if toks.len() == 1 {
        s.set(prop, v)?;
        return Ok(());
    }
    match prop.kind {
        PropKind::Collection { .. } => {
            let mut c = s.coll_mut(prop)?;
            let i = match &toks[1] {
                K(k) => c.find(k).expect("key"),
                I(i) => *i,
                P(_) => panic!(),
            };
            let mut item = c.item_mut(i)?;
            set_path(&mut *item, &toks[2..], v)
        }
        _ => {
            let mut g = s.struct_mut(prop)?;
            set_path(&mut *g, &toks[1..], v)
        }
    }
}

fn get(t: &NodeTree, toks: &[Tok]) -> Value {
    walk(t, toks, RnaPtr::new(Root::Data, t.srna()))
        .1
        .expect("a value")
}

fn set(t: &mut NodeTree, toks: &[Tok], v: Value) {
    set_path(t, toks, v).expect("set");
}

fn enum_v(s: &str) -> Value {
    Value::Enum(SmolStr::new(s))
}

struct Names;
impl IdNames for Names {
    fn id_name(&self, _: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        Some(("X".into(), None))
    }
}

fn link_tuples(t: &NodeTree) -> Vec<(String, String, String, String)> {
    t.links
        .iter()
        .map(|l| {
            (
                t.node(l.from_node).unwrap().name.to_string(),
                l.from_socket.to_string(),
                t.node(l.to_node).unwrap().name.to_string(),
                l.to_socket.to_string(),
            )
        })
        .collect()
}

fn tup(a: &str, b: &str, c: &str, d: &str) -> (String, String, String, String) {
    (a.into(), b.into(), c.into(), d.into())
}

// ------------------------------------------------------------------------- default graphs

#[test]
fn materials_new_graph_and_locations() {
    let t = NodeTree::default_material();
    assert_eq!(t.header.name, "Shader Nodetree");
    assert!(t.header.is_embedded_data);
    assert_eq!(t.ty, NodeTreeType::Shader);
    let summary: Vec<_> = t
        .nodes
        .iter()
        .map(|n| {
            (
                n.name.as_str(),
                n.bl_idname,
                n.location,
                n.width,
                n.label.as_str(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "Principled BSDF",
                "ShaderNodeBsdfPrincipled",
                [-200.0, 100.0],
                240.0,
                ""
            ),
            (
                "Material Output",
                "ShaderNodeOutputMaterial",
                [200.0, 100.0],
                140.0,
                ""
            ),
        ]
    );
    assert_eq!(
        link_tuples(&t),
        [tup("Principled BSDF", "BSDF", "Material Output", "Surface")]
    );
    let bsdf = &t.nodes[0];
    assert_eq!(bsdf.inputs.len(), 32);
    assert_eq!(
        bsdf.input("Base Color").unwrap().default_value,
        SocketValue::Color([0.8, 0.8, 0.8, 1.0])
    );
    assert_eq!(
        bsdf.input("Roughness").unwrap().default_value,
        SocketValue::Float(0.5)
    );
    assert_eq!(
        bsdf.input("Emission Color").unwrap().default_value,
        SocketValue::Color([1.0; 4])
    );
    // Disabled on a fresh Principled BSDF: the internal Weight and Subsurface IOR.
    let disabled: Vec<_> = bsdf
        .inputs
        .iter()
        .filter(|s| !s.enabled)
        .map(|s| s.identifier.as_str())
        .collect();
    assert_eq!(disabled, ["Weight", "Subsurface IOR"]);
    assert_eq!(bsdf.prop_str("distribution"), "MULTI_GGX");
    let out = &t.nodes[1];
    let ids: Vec<_> = out
        .inputs
        .iter()
        .map(|s| (s.identifier.as_str(), s.hide_value))
        .collect();
    assert_eq!(
        ids,
        [
            ("Surface", false),
            ("Volume", false),
            ("Displacement", true),
            ("Thickness", true)
        ]
    );
    assert_eq!(out.prop_str("target"), "ALL");
    assert_eq!(t.next_node_uid, 2);
    assert!(t.link_is_valid(&t.links[0]));
}

#[test]
fn worlds_new_graph() {
    let t = NodeTree::default_world();
    assert_eq!(t.header.name, WORLD_TREE_NAME);
    assert_eq!(t.header.name, "World Nodetree");
    let summary: Vec<_> = t
        .nodes
        .iter()
        .map(|n| (n.name.as_str(), n.location, n.width))
        .collect();
    assert_eq!(
        summary,
        [
            ("Background", [-200.0, 100.0], 140.0),
            ("World Output", [200.0, 100.0], 140.0)
        ]
    );
    let bg = &t.nodes[0];
    assert_eq!(
        bg.input("Color").unwrap().default_value,
        SocketValue::Color([0.05, 0.05, 0.05, 1.0])
    );
    assert_eq!(
        bg.input("Strength").unwrap().default_value,
        SocketValue::Float(1.0)
    );
    assert!(!bg.inputs[2].enabled, "Weight is disabled");
    assert_eq!(
        link_tuples(&t),
        [tup("Background", "Background", "World Output", "Surface")]
    );
}

// ---------------------------------------------------------------------------- nodes.new

#[test]
fn nodes_new_names_locations_and_errors() {
    let mut t = NodeTree::default_material();
    let a = t.new_node("ShaderNodeMix").unwrap();
    let b = t.new_node("ShaderNodeMix").unwrap();
    let c = t.new_node("ShaderNodeRGB").unwrap();
    let d = t.new_node("ShaderNodeTexImage").unwrap();
    assert_eq!(t.nodes[a].name, "Mix");
    assert_eq!(t.nodes[b].name, "Mix.001");
    assert_eq!(t.nodes[c].name, "Color", "ShaderNodeRGB's default name");
    assert_eq!(t.nodes[d].name, "Image Texture");
    assert_eq!((t.nodes[a].location, t.nodes[a].width), ([0.0, 0.0], 140.0));
    assert_eq!(t.nodes[d].width, 240.0);
    assert_eq!(t.nodes[d].prop("image"), Some(&Value::Id(None)));
    assert_eq!(t.nodes[d].prop("interpolation"), Some(&enum_v("Linear")));
    assert_eq!(t.nodes[d].prop("extension"), Some(&enum_v("REPEAT")));
    let uids: Vec<_> = t.nodes.iter().map(|n| n.uid).collect();
    assert_eq!(uids, [0, 1, 2, 3, 4, 5]);

    for bad in [
        "ShaderNodeSeparateRGB",
        "ShaderNodeCombineRGB",
        "ShaderNodeSeparateHSV",
        "Bogus",
    ] {
        let e = t.new_node(bad).unwrap_err();
        assert_eq!(e.exception, PyExc::RuntimeError);
        assert_eq!(e.message, format!("Error: Node type {bad} undefined"));
    }
    // Registered types of another tree type fail the poll (verified wording; Blender adds a
    // trailing newline, which frap's reports drop).
    for (bad, why) in [
        ("GeometryNodeMeshCube", "Not a geometry node tree"),
        ("FunctionNodeCompare", "Not a geometry node tree"),
        ("CompositorNodeBlur", "Not a compositor node tree"),
        ("TextureNodeChecker", "Not a texture node tree"),
        (
            "FunctionNodeAlignRotationToVector",
            "Not a geometry or compositor node tree",
        ),
        ("NodeGetBundleItem", "Not a geometry node tree"),
    ] {
        let e = t.new_node(bad).unwrap_err();
        assert_eq!(e.exception, PyExc::RuntimeError);
        assert_eq!(
            e.message,
            format!("Error: Cannot add node of type {bad} to node tree 'Shader Nodetree'\n  {why}")
        );
    }
    // Unregistered names with those prefixes stay undefined.
    let e = t.new_node("GeometryNodeFoo").unwrap_err();
    assert_eq!(e.message, "Error: Node type GeometryNodeFoo undefined");
    assert_eq!(t.nodes.len(), 6, "a failed nodes.new adds nothing");
    // A geometry tree rejects shader-only nodes and accepts shared ones (Math).
    let mut g = NodeTree::new("G", NodeTreeType::Geometry, false);
    let e = g.new_node("ShaderNodeTexImage").unwrap_err();
    assert_eq!(
        e.message,
        "Error: Cannot add node of type ShaderNodeTexImage to node tree 'G'\n  Not a shader node tree"
    );
    assert!(g.new_node("ShaderNodeMath").is_ok());
}

#[test]
fn every_generated_node_type_builds_and_reads() {
    // Every property of every node and socket either reads or reports "not implemented":
    // no panic, no wrong-kind value.
    let mut t = NodeTree::new("T", NodeTreeType::Shader, false);
    for def in NODE_TYPES {
        let i = t.add_node(def);
        let n = &t.nodes[i];
        assert_eq!(n.inputs.len(), def.inputs.len());
        assert_eq!(n.outputs.len(), def.outputs.len());
        let node = super::NodeRef { tree: &t, i };
        assert!(std::ptr::eq(node.srna(), def.srna));
        for prop in def.srna.all_props() {
            match node.get(prop) {
                Ok(_) => {}
                Err(e) => assert_eq!(
                    e.kind,
                    RnaErrorKind::NotImplemented,
                    "{}.{}",
                    def.bl_idname,
                    prop.identifier
                ),
            }
        }
        for (is_output, len) in [(false, n.inputs.len()), (true, n.outputs.len())] {
            for s in 0..len {
                let sock = super::SocketRef {
                    tree: &t,
                    node: i,
                    is_output,
                    i: s,
                };
                let srna = sock.srna();
                let sdef = if is_output {
                    &def.outputs[s]
                } else {
                    &def.inputs[s]
                };
                assert_eq!(srna.identifier, sdef.bl_idname, "{}", def.bl_idname);
                for prop in srna.all_props() {
                    match sock.get(prop) {
                        Ok(Field::Value(Value::FloatArray { data, shape, .. })) => {
                            assert_eq!(shape, prop.array);
                            assert_eq!(
                                data.len(),
                                prop.array_len(),
                                "{}.{}",
                                srna.identifier,
                                prop.identifier
                            );
                        }
                        Ok(_) => {}
                        Err(e) => assert_eq!(
                            e.kind,
                            RnaErrorKind::NotImplemented,
                            "{}.{}",
                            srna.identifier,
                            prop.identifier
                        ),
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------- embedded tree and its repr

#[test]
fn embedded_tree_repr_and_path_from_id() {
    let t = NodeTree::default_material();
    let owner = frap_rna::IdRef::<MaterialId>::from_key(IdKey::next()).any();
    let root = RnaPtr::new(
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        },
        t.srna(),
    );
    assert_eq!(root.repr(&Names), "bpy.data.materials['X'].node_tree");
    assert_eq!(t.srna().identifier, "ShaderNodeTree");
    assert_eq!(get(&t, &[P("is_embedded_data")]), Value::Bool(true));
    assert_eq!(get(&t, &[P("name")]), Value::Str("Shader Nodetree".into()));
    assert_eq!(get(&t, &[P("type")]), enum_v("SHADER"));

    let (node, _) = walk(&t, &[P("nodes"), K("Principled BSDF")], root.clone());
    assert_eq!(
        node.repr(&Names),
        "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"]"
    );
    assert_eq!(node.path_from_id().unwrap(), "nodes[\"Principled BSDF\"]");
    assert_eq!(node.srna.identifier, "ShaderNodeBsdfPrincipled");

    let (sock, _) = walk(
        &t,
        &[
            P("nodes"),
            K("Principled BSDF"),
            P("inputs"),
            K("Base Color"),
        ],
        root.clone(),
    );
    assert_eq!(
        sock.repr(&Names),
        "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"].inputs[0]"
    );
    assert_eq!(sock.srna.identifier, "NodeSocketColor");
    let (dv, v) = walk(
        &t,
        &[
            P("nodes"),
            K("Principled BSDF"),
            P("inputs"),
            K("Base Color"),
            P("default_value"),
        ],
        root.clone(),
    );
    assert_eq!(
        dv.path_from_id().unwrap(),
        "nodes[\"Principled BSDF\"].inputs[0].default_value"
    );
    assert_eq!(
        v.unwrap(),
        Value::FloatArray {
            data: [0.8, 0.8, 0.8, 1.0].into_iter().collect(),
            shape: ArrayShape::Fixed(4),
            tag: MathTag::None,
        }
    );
    let (out, _) = walk(
        &t,
        &[P("nodes"), K("Principled BSDF"), P("outputs"), I(0)],
        root,
    );
    assert_eq!(
        out.repr(&Names),
        "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"].outputs[0]"
    );
    // Socket metadata.
    let base = |p: &'static str| {
        get(
            &t,
            &[
                P("nodes"),
                K("Principled BSDF"),
                P("inputs"),
                K("Base Color"),
                P(p),
            ],
        )
    };
    assert_eq!(base("link_limit"), Value::Int(1));
    assert_eq!(base("is_output"), Value::Bool(false));
    assert_eq!(base("is_linked"), Value::Bool(false));
    assert_eq!(base("type"), enum_v("RGBA"));
    assert_eq!(base("bl_idname"), Value::Str("NodeSocketColor".into()));
    let bsdf_out = |p: &'static str| {
        get(
            &t,
            &[P("nodes"), K("Principled BSDF"), P("outputs"), I(0), P(p)],
        )
    };
    assert_eq!(bsdf_out("link_limit"), Value::Int(4095));
    assert_eq!(bsdf_out("is_linked"), Value::Bool(true));
    assert_eq!(bsdf_out("type"), enum_v("SHADER"));
    assert_eq!(
        get(
            &t,
            &[
                P("nodes"),
                K("Principled BSDF"),
                P("inputs"),
                K("Metallic"),
                P("bl_idname")
            ]
        ),
        Value::Str("NodeSocketFloatFactor".into())
    );
    // Links: nameless, ends are nodes and sockets.
    let (l, _) = walk(&t, &[P("links"), I(0)], RnaPtr::new(Root::Data, t.srna()));
    assert_eq!(l.path.last(), Some(&Seg::Index(0)));
    assert_eq!(
        get(&t, &[P("links"), I(0), P("from_node"), P("name")]),
        Value::Str("Principled BSDF".into())
    );
    assert_eq!(
        get(&t, &[P("links"), I(0), P("to_socket"), P("identifier")]),
        Value::Str("Surface".into())
    );
    assert_eq!(
        get(&t, &[P("links"), I(0), P("is_valid")]),
        Value::Bool(true)
    );
    // Missing keys.
    let nodes = super::NodesColl { tree: &t };
    assert_eq!(nodes.find("Nope"), None);
    assert_eq!(nodes.len(), 2);
}

#[test]
fn socket_and_node_writes() {
    let mut t = NodeTree::default_material();
    set(
        &mut t,
        &[
            P("nodes"),
            K("Principled BSDF"),
            P("inputs"),
            K("Metallic"),
            P("default_value"),
        ],
        Value::Float(1.0),
    );
    assert_eq!(
        t.nodes[0].input("Metallic").unwrap().default_value,
        SocketValue::Float(1.0)
    );
    set(
        &mut t,
        &[
            P("nodes"),
            K("Principled BSDF"),
            P("inputs"),
            I(0),
            P("default_value"),
        ],
        Value::FloatArray {
            data: [1.0, 0.0, 0.0, 1.0].into_iter().collect(),
            shape: ArrayShape::Fixed(4),
            tag: MathTag::None,
        },
    );
    assert_eq!(
        t.nodes[0].inputs[0].default_value,
        SocketValue::Color([1.0, 0.0, 0.0, 1.0])
    );
    set(
        &mut t,
        &[P("nodes"), K("Principled BSDF"), P("location")],
        Value::FloatArray {
            data: [5.0, 6.0].into_iter().collect(),
            shape: ArrayShape::Fixed(2),
            tag: MathTag::None,
        },
    );
    assert_eq!(t.nodes[0].location, [5.0, 6.0]);
    set(
        &mut t,
        &[P("nodes"), K("Principled BSDF"), P("label")],
        Value::Str("Steel".into()),
    );
    assert_eq!(t.nodes[0].label, "Steel");
    // Shader sockets have no default_value.
    assert!(t.srna().find_prop("nodes").is_some());
    let bsdf_out = super::socket_srna(&t.nodes[0], true, 0);
    assert!(bsdf_out.find_prop("default_value").is_none());
}

#[test]
fn node_rename_is_unique_per_tree() {
    let mut t = NodeTree::default_material();
    t.new_node("ShaderNodeMix").unwrap();
    t.new_node("ShaderNodeMix").unwrap();
    // `Mix` renamed to the taken `Mix.001` becomes `Mix.002` (verified).
    set(
        &mut t,
        &[P("nodes"), K("Mix"), P("name")],
        Value::Str("Mix.001".into()),
    );
    assert_eq!(t.nodes[2].name, "Mix.002");
    assert_eq!(t.nodes[3].name, "Mix.001");
    set(
        &mut t,
        &[P("nodes"), K("Mix.002"), P("name")],
        Value::Str("".into()),
    );
    assert_eq!(t.nodes[2].name, "Node");
    set(
        &mut t,
        &[P("nodes"), K("Node"), P("name")],
        Value::Str("Principled BSDF".into()),
    );
    assert_eq!(t.nodes[2].name, "Principled BSDF.001");
    // Renaming to its own name keeps it.
    set(
        &mut t,
        &[P("nodes"), K("Mix.001"), P("name")],
        Value::Str("Mix.001".into()),
    );
    assert_eq!(t.nodes[3].name, "Mix.001");
    // Links follow uids, not names.
    assert_eq!(
        link_tuples(&t),
        [tup("Principled BSDF", "BSDF", "Material Output", "Surface")]
    );
}

// ------------------------------------------------------------ first-enabled socket lookup

#[test]
fn mix_first_enabled_socket_lookup() {
    let mut t = NodeTree::default_material();
    t.new_node("ShaderNodeMix").unwrap();
    let enabled = |t: &NodeTree| -> (Vec<bool>, Vec<bool>) {
        let n = t.node_by_name("Mix").unwrap();
        (
            n.inputs.iter().map(|s| s.enabled).collect(),
            n.outputs.iter().map(|s| s.enabled).collect(),
        )
    };
    let ident = |t: &NodeTree, name: &'static str| {
        get(
            t,
            &[P("nodes"), K("Mix"), P("inputs"), K(name), P("identifier")],
        )
    };
    // Verified enabled sets (inputs: Factor_Float, Factor_Vector, A/B Float, A/B Vector,
    // A/B Color, A/B Rotation; outputs: Result Float/Vector/Color/Rotation).
    type Case = (
        &'static str,
        &'static str,
        [bool; 10],
        [bool; 4],
        &'static str,
    );
    let cases: [Case; 3] = [
        (
            "FLOAT",
            "A_Float",
            [
                true, false, true, true, false, false, false, false, false, false,
            ],
            [true, false, false, false],
            "Factor_Float",
        ),
        (
            "VECTOR",
            "A_Vector",
            [
                true, false, false, false, true, true, false, false, false, false,
            ],
            [false, true, false, false],
            "Factor_Float",
        ),
        (
            "RGBA",
            "A_Color",
            [
                true, false, false, false, false, false, true, true, false, false,
            ],
            [false, false, true, false],
            "Factor_Float",
        ),
    ];
    for (dt, a, ins, outs, factor) in cases {
        set(&mut t, &[P("nodes"), K("Mix"), P("data_type")], enum_v(dt));
        assert_eq!(get(&t, &[P("nodes"), K("Mix"), P("data_type")]), enum_v(dt));
        assert_eq!(ident(&t, "A"), Value::Str(a.into()), "{dt}");
        assert_eq!(ident(&t, "Factor"), Value::Str(factor.into()), "{dt}");
        assert_eq!(enabled(&t), (ins.to_vec(), outs.to_vec()), "{dt}");
        let out_id = get(
            &t,
            &[
                P("nodes"),
                K("Mix"),
                P("outputs"),
                K("Result"),
                P("identifier"),
            ],
        );
        assert_eq!(out_id, Value::Str(a.replace("A_", "Result_").into()));
        // The Rust API agrees.
        assert_eq!(
            t.node_by_name("Mix")
                .unwrap()
                .input("A")
                .unwrap()
                .identifier,
            a
        );
    }
    // Identifiers are not keys.
    let mix = t.node_by_name("Mix").unwrap();
    assert_eq!(crate::find_socket(&mix.inputs, "A_Color"), None);
    let coll = super::SocketsColl {
        tree: &t,
        node: 2,
        is_output: false,
    };
    assert_eq!(coll.find("A_Color"), None);
    assert_eq!(coll.item_seg(6), Seg::Index(6));
    // VECTOR + NON_UNIFORM switches the factor socket.
    set(
        &mut t,
        &[P("nodes"), K("Mix"), P("data_type")],
        enum_v("VECTOR"),
    );
    set(
        &mut t,
        &[P("nodes"), K("Mix"), P("factor_mode")],
        enum_v("NON_UNIFORM"),
    );
    assert_eq!(ident(&t, "Factor"), Value::Str("Factor_Vector".into()));
    assert_eq!(
        enabled(&t).0,
        [
            false, true, false, false, true, true, false, false, false, false
        ]
    );
}

#[test]
fn math_and_principled_availability() {
    let mut t = NodeTree::default_material();
    let m = t.new_node("ShaderNodeMath").unwrap();
    let en = |t: &NodeTree| {
        t.nodes[m]
            .inputs
            .iter()
            .map(|s| s.enabled)
            .collect::<Vec<_>>()
    };
    assert_eq!(en(&t), [true, true, false]);
    for (op, want) in [
        ("ROUND", [true, false, false]),
        ("WRAP", [true, true, true]),
        ("MULTIPLY", [true, true, false]),
    ] {
        assert!(t.nodes[m].set_prop("operation", enum_v(op)));
        assert_eq!(en(&t), want, "{op}");
    }
    let skin = |t: &NodeTree| t.nodes[0].input("Subsurface IOR").is_some();
    assert!(!skin(&t));
    t.nodes[0].set_prop("subsurface_method", enum_v("RANDOM_WALK_SKIN"));
    assert!(skin(&t));
    t.nodes[0].set_prop("subsurface_method", enum_v("BURLEY"));
    assert!(!skin(&t));
    assert!(!t.nodes[0].set_prop("not_a_prop", Value::Bool(true)));
}

/// `enabled` flags as a bit string ("1" = enabled), in socket order.
fn bits(s: &[crate::Socket]) -> String {
    s.iter()
        .map(|s| if s.enabled { '1' } else { '0' })
        .collect()
}

#[test]
fn vector_math_availability_matches_blender() {
    // Dumped on 5.2.4 (`--factory-startup`): for every `operation` of a fresh
    // `ShaderNodeVectorMath`, `[s.enabled for s in node.inputs]` (Vector, Vector_001,
    // Vector_002, Scale) and `[s.enabled for s in node.outputs]` (Vector, Value).
    const DUMP: [(&str, &str, &str); 30] = [
        ("ADD", "1100", "10"),
        ("SUBTRACT", "1100", "10"),
        ("MULTIPLY", "1100", "10"),
        ("DIVIDE", "1100", "10"),
        ("MULTIPLY_ADD", "1110", "10"),
        ("CROSS_PRODUCT", "1100", "10"),
        ("PROJECT", "1100", "10"),
        ("REFLECT", "1100", "10"),
        ("REFRACT", "1101", "10"),
        ("FACEFORWARD", "1110", "10"),
        ("DOT_PRODUCT", "1100", "01"),
        ("DISTANCE", "1100", "01"),
        ("LENGTH", "1000", "01"),
        ("SCALE", "1001", "10"),
        ("NORMALIZE", "1000", "10"),
        ("ABSOLUTE", "1000", "10"),
        ("POWER", "1100", "10"),
        ("SIGN", "1000", "10"),
        ("MINIMUM", "1100", "10"),
        ("MAXIMUM", "1100", "10"),
        ("ROUND", "1000", "10"),
        ("FLOOR", "1000", "10"),
        ("CEIL", "1000", "10"),
        ("FRACTION", "1000", "10"),
        ("MODULO", "1100", "10"),
        ("WRAP", "1110", "10"),
        ("SNAP", "1100", "10"),
        ("SINE", "1000", "10"),
        ("COSINE", "1000", "10"),
        ("TANGENT", "1000", "10"),
    ];
    let mut t = NodeTree::default_material();
    let i = t.new_node("ShaderNodeVectorMath").unwrap();
    assert_eq!(
        (bits(&t.nodes[i].inputs), bits(&t.nodes[i].outputs)),
        ("1100".into(), "10".into()),
        "fresh node (ADD)"
    );
    for (op, ins, outs) in DUMP {
        set(
            &mut t,
            &[P("nodes"), K("Vector Math"), P("operation")],
            enum_v(op),
        );
        let n = &t.nodes[i];
        assert_eq!(
            (bits(&n.inputs), bits(&n.outputs)),
            (ins.into(), outs.into()),
            "{op}"
        );
    }
    // outputs['Value'] is outputs[1] once DOT_PRODUCT enables it; outputs['Vector'] is gone.
    set(
        &mut t,
        &[P("nodes"), K("Vector Math"), P("operation")],
        enum_v("DOT_PRODUCT"),
    );
    let outs = super::SocketsColl {
        tree: &t,
        node: i,
        is_output: true,
    };
    assert_eq!(outs.find("Value"), Some(1));
    assert_eq!(outs.find("Vector"), None);
}

#[test]
fn mapping_availability_matches_blender() {
    // Dumped on 5.2.4: `[s.enabled for s in node.inputs]` (Vector, Location, Rotation, Scale)
    // per `vector_type`; inputs['Location'] raises KeyError for VECTOR and NORMAL.
    let mut t = NodeTree::default_material();
    let i = t.new_node("ShaderNodeMapping").unwrap();
    assert_eq!(bits(&t.nodes[i].inputs), "1111");
    for (vt, want) in [
        ("POINT", "1111"),
        ("TEXTURE", "1111"),
        ("VECTOR", "1011"),
        ("NORMAL", "1011"),
    ] {
        set(
            &mut t,
            &[P("nodes"), K("Mapping"), P("vector_type")],
            enum_v(vt),
        );
        assert_eq!(bits(&t.nodes[i].inputs), want, "{vt}");
        let ins = super::SocketsColl {
            tree: &t,
            node: i,
            is_output: false,
        };
        assert_eq!(ins.find("Location").is_some(), want == "1111", "{vt}");
    }
}

// ---------------------------------------------------------------------------- links.new

#[test]
fn links_new_replaces_the_link_into_an_input() {
    let mut t = NodeTree::default_material();
    let tex = t.new_node("ShaderNodeTexImage").unwrap();
    let rgb = t.new_node("ShaderNodeRGB").unwrap();
    let (tex, rgb) = (t.nodes[tex].uid, t.nodes[rgb].uid);
    let bsdf = t.nodes[0].uid;
    let out = |n: u32, i| SocketAddr::output(n, i);
    let inp = |n: u32, i| SocketAddr::input(n, i);
    t.link_new(out(tex, 0), inp(bsdf, 0), true).unwrap();
    t.link_new(out(tex, 1), inp(bsdf, 4), true).unwrap();
    assert_eq!(
        link_tuples(&t),
        [
            tup("Principled BSDF", "BSDF", "Material Output", "Surface"),
            tup("Image Texture", "Color", "Principled BSDF", "Base Color"),
            tup("Image Texture", "Alpha", "Principled BSDF", "Alpha"),
        ]
    );
    // Replacing the Base Color link removes the old one and appends the new one.
    let i = t.link_new(out(rgb, 0), inp(bsdf, 0), true).unwrap();
    assert_eq!(i, 2);
    assert_eq!(
        link_tuples(&t),
        [
            tup("Principled BSDF", "BSDF", "Material Output", "Surface"),
            tup("Image Texture", "Alpha", "Principled BSDF", "Alpha"),
            tup("Color", "Color", "Principled BSDF", "Base Color"),
        ]
    );
    // Arguments in the wrong order are swapped.
    t.link_new(inp(bsdf, 1), out(tex, 1), true).unwrap();
    assert_eq!(
        link_tuples(&t)[3],
        tup("Image Texture", "Alpha", "Principled BSDF", "Metallic")
    );
    // Re-linking an existing link moves it to the end.
    t.link_new(out(bsdf, 0), inp(1, 0), true).unwrap();
    assert_eq!(link_tuples(&t).len(), 4);
    assert_eq!(
        link_tuples(&t)[3],
        tup("Principled BSDF", "BSDF", "Material Output", "Surface")
    );
    // verify_limits=False appends a second link into the same input.
    t.link_new(out(rgb, 0), inp(bsdf, 4), false).unwrap();
    assert_eq!(link_tuples(&t).len(), 5);
    // Same direction.
    for (a, b) in [(out(bsdf, 0), out(tex, 0)), (inp(bsdf, 0), inp(tex, 0))] {
        let e = t.link_new(a, b, true).unwrap_err();
        assert_eq!(e.exception, PyExc::RuntimeError);
        assert_eq!(e.message, "Error: Same input/output direction of sockets");
    }
    // is_linked and validity.
    assert!(t.is_linked(inp(bsdf, 0)));
    assert!(!t.is_linked(inp(bsdf, 2)));
    let bad = Link {
        from_node: bsdf,
        from_socket: "BSDF".into(),
        to_node: rgb,
        to_socket: "x".into(),
    };
    assert!(!t.link_is_valid(&bad));
    let mix = t.new_node("ShaderNodeMix").unwrap();
    let mix = t.nodes[mix].uid;
    let k = t.link_new(out(bsdf, 0), inp(mix, 2), true).unwrap();
    assert!(
        !t.link_is_valid(&t.links[k]),
        "shader output into a float input"
    );
    let k = t.link_new(out(bsdf, 0), inp(bsdf, 0), true).unwrap();
    assert!(
        !t.link_is_valid(&t.links[k]),
        "self link from a shader output into a colour input"
    );
    // links.new by names (first enabled sockets).
    t.link_by_name((rgb, "Color"), (mix, "A")).unwrap();
    assert_eq!(t.links.last().unwrap().to_socket, "A_Float");
}

/// `(from node, from socket, to node, to socket, is_valid)` of every link.
fn validity(t: &NodeTree) -> Vec<(String, String, String, String, bool)> {
    link_tuples(t)
        .into_iter()
        .zip(t.links_valid())
        .map(|((a, b, c, d), v)| (a, b, c, d, v))
        .collect()
}

fn vtup(a: &str, b: &str, c: &str, d: &str, v: bool) -> (String, String, String, String, bool) {
    (a.into(), b.into(), c.into(), d.into(), v)
}

#[test]
fn link_validity_matches_blender() {
    // Every expectation below is Blender 5.2.4's `[l.is_valid for l in tree.links]` for the
    // same calls on a fresh `materials.new()` tree.
    let inp = |n: u32, i| SocketAddr::input(n, i);
    let out = |n: u32, i| SocketAddr::output(n, i);
    let uid = |t: &NodeTree, i: usize| t.nodes[i].uid;
    let surface = vtup(
        "Principled BSDF",
        "BSDF",
        "Material Output",
        "Surface",
        true,
    );

    // Links into or out of disabled sockets are invalid until the sockets are enabled.
    let mut t = NodeTree::default_material();
    let (mx, rgb) = (
        t.new_node("ShaderNodeMix").unwrap(),
        t.new_node("ShaderNodeRGB").unwrap(),
    );
    let (mx, rgb) = (uid(&t, mx), uid(&t, rgb));
    t.link_new(out(rgb, 0), inp(mx, 6), true).unwrap();
    t.link_new(out(mx, 2), inp(0, 0), true).unwrap();
    assert_eq!(
        validity(&t),
        [
            surface.clone(),
            vtup("Color", "Color", "Mix", "A_Color", false),
            vtup(
                "Mix",
                "Result_Color",
                "Principled BSDF",
                "Base Color",
                false
            ),
        ]
    );
    set(
        &mut t,
        &[P("nodes"), K("Mix"), P("data_type")],
        enum_v("RGBA"),
    );
    assert!(t.links_valid().iter().all(|v| *v));
    // Vector Math `Scale` is disabled for ADD.
    let vm = t.new_node("ShaderNodeVectorMath").unwrap();
    let vm = uid(&t, vm);
    let k = t.link_new(out(rgb, 0), inp(vm, 3), true).unwrap();
    assert!(!t.link_is_valid(&t.links[k]));

    // Extra links into one input (verify_limits=False) stay valid.
    let mut t = NodeTree::default_material();
    let (a, b) = (
        t.new_node("ShaderNodeRGB").unwrap(),
        t.new_node("ShaderNodeRGB").unwrap(),
    );
    let (a, b) = (uid(&t, a), uid(&t, b));
    t.link_new(out(a, 0), inp(0, 0), true).unwrap();
    t.link_new(out(b, 0), inp(0, 0), false).unwrap();
    assert_eq!(t.links_valid(), [true, true, true]);

    // A data self link is valid.
    let mut t = NodeTree::default_material();
    let m = t.new_node("ShaderNodeMath").unwrap();
    let m = uid(&t, m);
    t.link_new(out(m, 0), inp(m, 0), true).unwrap();
    assert_eq!(t.links_valid(), [true, true]);

    // A cycle: the back edge and the links into the node where the walk entered it are
    // invalid.
    let mut t = NodeTree::default_material();
    let ids: Vec<u32> = ["ShaderNodeMath", "ShaderNodeMath", "ShaderNodeValue"]
        .into_iter()
        .map(|ty| {
            let i = t.new_node(ty).unwrap();
            uid(&t, i)
        })
        .collect();
    let (m1, m2, v) = (ids[0], ids[1], ids[2]);
    t.link_new(out(m1, 0), inp(m2, 0), true).unwrap();
    t.link_new(out(m2, 0), inp(m1, 0), true).unwrap();
    t.link_by_name((m2, "Value"), (0, "Metallic")).unwrap();
    t.link_by_name((v, "Value"), (0, "Roughness")).unwrap();
    t.link_new(out(v, 0), inp(m1, 1), true).unwrap();
    assert_eq!(
        validity(&t),
        [
            surface.clone(),
            vtup("Math", "Value", "Math.001", "Value", true),
            vtup("Math.001", "Value", "Math", "Value", false),
            vtup("Math.001", "Value", "Principled BSDF", "Metallic", true),
            vtup("Value", "Value", "Principled BSDF", "Roughness", true),
            vtup("Value", "Value", "Math", "Value_001", false),
        ]
    );

    // A self link into the Principled BSDF's Metallic cuts its walk short: Alpha (after
    // Metallic) becomes invalid, Base Color (before it) stays valid.
    let mut t = NodeTree::default_material();
    let (rgb, v) = (
        t.new_node("ShaderNodeRGB").unwrap(),
        t.new_node("ShaderNodeValue").unwrap(),
    );
    let (rgb, v) = (uid(&t, rgb), uid(&t, v));
    t.link_by_name((rgb, "Color"), (0, "Base Color")).unwrap();
    t.link_by_name((v, "Value"), (0, "Alpha")).unwrap();
    t.link_by_name((0, "BSDF"), (0, "Metallic")).unwrap();
    assert_eq!(
        validity(&t),
        [
            surface.clone(),
            vtup("Color", "Color", "Principled BSDF", "Base Color", true),
            vtup("Value", "Value", "Principled BSDF", "Alpha", false),
            vtup(
                "Principled BSDF",
                "BSDF",
                "Principled BSDF",
                "Metallic",
                false
            ),
        ]
    );

    // A shader output into a colour input.
    let mut t = NodeTree::default_material();
    let e = t.new_node("ShaderNodeEmission").unwrap();
    let e = uid(&t, e);
    t.link_new(out(e, 0), inp(0, 0), true).unwrap();
    assert_eq!(t.links_valid(), [true, false]);
}

#[test]
fn first_output_node_is_active() {
    // Verified on 5.2.4: the first Material Output is active whatever its target; removing
    // it makes the next one active.
    let mut t = NodeTree::default_material();
    t.new_node("ShaderNodeOutputMaterial").unwrap();
    set(
        &mut t,
        &[P("nodes"), K("Material Output"), P("target")],
        enum_v("CYCLES"),
    );
    let active =
        |t: &NodeTree, name: &'static str| get(t, &[P("nodes"), K(name), P("is_active_output")]);
    assert_eq!(active(&t, "Material Output"), Value::Bool(true));
    assert_eq!(active(&t, "Material Output.001"), Value::Bool(false));
    t.remove_node(1).unwrap();
    assert_eq!(active(&t, "Material Output.001"), Value::Bool(true));
}

#[test]
fn nodes_remove_drops_links_and_parents() {
    let mut t = NodeTree::default_material();
    let tex = t.new_node("ShaderNodeTexImage").unwrap();
    let frame = t.new_node("NodeFrame").unwrap();
    let (tex_uid, frame_uid) = (t.nodes[tex].uid, t.nodes[frame].uid);
    t.nodes[tex].parent = Some(frame_uid);
    t.nodes[frame].location = [10.0, 20.0];
    t.nodes[tex].location = [1.0, 2.0];
    assert_eq!(t.location_absolute(tex), [11.0, 22.0]);
    t.link_by_name((tex_uid, "Color"), (0, "Base Color"))
        .unwrap();
    assert_eq!(t.links.len(), 2);
    t.remove_node(frame).unwrap();
    assert_eq!(t.node(tex_uid).unwrap().parent, None);
    assert_eq!(
        t.node(tex_uid).unwrap().location,
        [11.0, 22.0],
        "made absolute"
    );
    let tex = t.node_index(tex_uid).unwrap();
    t.remove_node(tex).unwrap();
    assert_eq!(
        link_tuples(&t),
        [tup("Principled BSDF", "BSDF", "Material Output", "Surface")]
    );
    assert!(t.remove_node(99).is_none());
    assert!(t.remove_link(0).is_some());
    assert!(t.links.is_empty());
}

#[test]
fn image_pointer_and_id_refs() {
    let mut t = NodeTree::default_material();
    let tex = t.new_node("ShaderNodeTexImage").unwrap();
    let img = frap_rna::IdRef::<frap_rna::ImageId>::from_key(IdKey::next()).any();
    set(
        &mut t,
        &[P("nodes"), K("Image Texture"), P("image")],
        Value::Id(Some(img)),
    );
    assert_eq!(t.nodes[tex].prop("image"), Some(&Value::Id(Some(img))));
    let node = super::NodeRef { tree: &t, i: tex };
    let prop = node.srna().find_prop("image").unwrap();
    assert!(matches!(node.get(prop), Ok(Field::Id(Some(x))) if x == img));
    let mut refs = vec![];
    frap_rna::IdType::for_each_ref(&t, &mut |a| refs.push(a));
    assert_eq!(refs, [img]);
    frap_rna::IdType::remap_ref(&mut t, img, None);
    assert_eq!(t.nodes[tex].prop("image"), Some(&Value::Id(None)));
}

#[test]
fn socket_types_and_values() {
    let t = NodeTree::default_material();
    let n = &t.nodes[0];
    assert_eq!(n.input("Normal").unwrap().ty, SocketType::Vector);
    assert_eq!(
        n.input("Normal").unwrap().default_value,
        SocketValue::Vector([0.0; 4])
    );
    let i = n.input_index("Normal").unwrap();
    let s = super::SocketRef {
        tree: &t,
        node: 0,
        is_output: false,
        i,
    };
    let p = s.srna().find_prop("default_value").unwrap();
    match s.get(p).unwrap() {
        Field::Value(Value::FloatArray { data, .. }) => assert_eq!(data.len(), 3),
        _ => panic!("vector"),
    }
    assert_eq!(super::socket_type_str(SocketType::Rgba), "RGBA");
    assert!(node_type("ShaderNodeSeparateRGB").is_none());
}
