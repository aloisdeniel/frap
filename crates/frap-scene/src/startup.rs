// SPDX-License-Identifier: GPL-3.0-or-later
//! Factory and empty startup (ARCHITECTURE §5.7), built from `spec/blender-5.2/startup.json`
//! (`tools/blender/dump_startup.py` on Blender 5.2.4 `--factory-startup`, compiled in).
//!
//! Every bound RNA value of every datablock is loaded **through RNA** from the dump (a
//! generic loader walks the dumped properties, coerces each value against its `PropDef` and
//! writes it with `RnaMut::set`), so the startup state reads back equal to the dump through the
//! same bindings (`tests/startup.rs`). Structure (IDs, pointers, collections, view layers,
//! meshes) is built explicitly.
//!
//! Omitted, as listed deviations or gaps: the grease-pencil material `Dots Stroke` (D-11) and
//! the `Render Result` / `Viewer Node` images (runtime images of the render and compositor;
//! `render.render` creates `Render Result`, WP 1.24).
//!
//! The empty startup (`frap --empty`, `wm.read_homefile(use_empty=True)`) is the factory scene
//! with its settings but without objects, collections, camera or world (verified with a dump of
//! `read_homefile(use_empty=True)`: `crates/frap-scene/tests/data/empty_startup.json`).

use std::sync::{Arc, OnceLock};

use frap_mesh::{AttrData, Attribute, AttributeSet, Domain, Mesh};
use frap_nodes::generated::{NodePropDefault, NodeTypeDef, SocketDefault, node_type};
use frap_nodes::{Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketValue};
use frap_rna::{
    ArrayShape, FloatStorage, IdAny, IdRef, MaterialId, MathTag, ObjectId, PropFlags, PropKind,
    RnaMut, Value,
};
use glam::Mat4;
use serde_json::Value as J;
use smol_str::SmolStr;

use crate::context::{Base, BaseFlag, LayerCollection, ViewLayer, VirtualView3D, WindowState};
use crate::data::{ChangeLog, Main};
use crate::generated::plain::View3DShading;
use crate::id::{
    Camera, Collection, Light, Material, Object, SHADER_NODETREE_NAME, Scene, World, layers,
};

const STARTUP_JSON: &str = include_str!("../../../spec/blender-5.2/startup.json");

/// Materials of the factory file that frap does not create (D-11).
pub const OMITTED_MATERIALS: [&str; 1] = ["Dots Stroke"];
/// Images of the factory file that frap does not create (runtime render/compositor images).
pub const OMITTED_IMAGES: [&str; 2] = ["Render Result", "Viewer Node"];

/// The parsed factory dump (`spec/blender-5.2/startup.json`).
pub fn factory_dump() -> &'static J {
    static DUMP: OnceLock<J> = OnceLock::new();
    DUMP.get_or_init(|| serde_json::from_str(STARTUP_JSON).expect("invariant: startup.json parses"))
}

fn s(j: &J) -> &str {
    j.as_str().unwrap_or_default()
}

fn f32s<const N: usize>(j: &J) -> [f32; N] {
    std::array::from_fn(|i| j[i].as_f64().unwrap_or(0.0) as f32)
}

/// `{"$ref": "bpy.data.objects['Cube']"}` → `("objects", "Cube")`.
fn ref_of(j: &J) -> Option<(&str, String)> {
    let r = j.get("$ref")?.as_str()?;
    let rest = r.strip_prefix("bpy.data.")?;
    let (coll, key) = rest.split_once('[')?;
    let key = key.strip_suffix(']')?;
    let name = key.get(1..key.len().checked_sub(1)?)?;
    Some((coll, name.replace("\\'", "'")))
}

fn ref_id(main: &Main, j: &J) -> Option<IdAny> {
    let (coll, name) = ref_of(j)?;
    main.find(coll, &name)
}

/// A dumped JSON value as the typed value of `prop`, **unclamped**: the dump holds Blender's
/// stored values, some of which lie outside the RNA hard range (e.g. the image output's
/// `view_settings.white_balance_temperature` is 0) or are empty enums.
fn json_value(prop: &frap_rna::PropDef, v: &J) -> Option<Value> {
    fn flat(j: &J, out: &mut Vec<J>) {
        match j {
            J::Array(a) => a.iter().for_each(|x| flat(x, out)),
            other => out.push(other.clone()),
        }
    }
    let scalar = matches!(prop.array, ArrayShape::Scalar);
    let mut items = Vec::new();
    flat(v, &mut items);
    Some(match prop.kind {
        PropKind::Bool { .. } if scalar => Value::Bool(v.as_bool()?),
        PropKind::Bool { .. } => {
            Value::BoolArray(items.iter().map(J::as_bool).collect::<Option<_>>()?)
        }
        PropKind::Int { .. } if scalar => Value::Int(v.as_i64()?),
        PropKind::Int { .. } => {
            Value::IntArray(items.iter().map(J::as_i64).collect::<Option<_>>()?)
        }
        PropKind::Float { storage, .. } if scalar => match storage {
            FloatStorage::F64 => Value::Double(v.as_f64()?),
            _ => Value::Float(v.as_f64()? as f32),
        },
        PropKind::Float { .. } => Value::FloatArray {
            data: items
                .iter()
                .map(|x| x.as_f64().map(|f| f as f32))
                .collect::<Option<_>>()?,
            shape: prop.array,
            tag: MathTag::for_subtype(prop.subtype),
        },
        PropKind::String { .. } => Value::Str(v.as_str()?.into()),
        PropKind::Enum { .. } if prop.flags.contains(PropFlags::ENUM_FLAG) => Value::EnumSet(
            v.as_array()?
                .iter()
                .map(|x| x.as_str().map(SmolStr::new))
                .collect::<Option<_>>()?,
        ),
        PropKind::Enum { .. } => Value::Enum(v.as_str()?.into()),
        _ => return None,
    })
}

/// Loads every dumped value property of `j` into `t` through RNA `set` (nested plain
/// structs recursively). Properties a binding serves through a hook, read-only properties, pointers
/// and collections are skipped; so are the names in `skip`. Returns the values RNA refused.
pub fn load_rna(t: &mut dyn RnaMut, j: &J, skip: &[&str]) -> Vec<String> {
    let mut errs = Vec::new();
    load_into(t, j, skip, &mut errs);
    errs
}

fn load_into(t: &mut dyn RnaMut, j: &J, skip: &[&str], errs: &mut Vec<String>) {
    let Some(obj) = j.as_object() else {
        return;
    };
    let srna = t.srna();
    for (k, v) in obj {
        if k.starts_with('$') || skip.contains(&k.as_str()) {
            continue;
        }
        let Some(prop) = srna.find_prop(k) else {
            continue;
        };
        match prop.kind {
            PropKind::Pointer { is_id: false, .. } => {
                if v.get("$type").is_some()
                    && let Ok(mut g) = t.struct_mut(prop)
                {
                    load_into(&mut *g, v, &[], errs);
                }
            }
            PropKind::Pointer { .. } | PropKind::Collection { .. } => {}
            _ if prop.is_readonly() => {}
            _ => {
                let Some(value) = json_value(prop, v) else {
                    continue;
                };
                match t.set(prop, value) {
                    Ok(_) => {}
                    Err(e) if e.kind == frap_rna::RnaErrorKind::NoAttribute => {}
                    Err(e) => errs.push(format!("{}.{k}: {}", srna.identifier, e.message)),
                }
            }
        }
    }
}

/// Builds a node with every socket and property of a fresh `nodes.new(bl_idname)` (from the
/// generated `NodeTypeDef`).
fn node_from_def(def: &'static NodeTypeDef, uid: u32, location: [f32; 2]) -> Node {
    fn value(d: &SocketDefault) -> SocketValue {
        match *d {
            SocketDefault::None => SocketValue::None,
            SocketDefault::Float(f) => SocketValue::Float(f),
            SocketDefault::Int(i) => SocketValue::Int(i as i32),
            SocketDefault::Bool(b) => SocketValue::Bool(b),
            SocketDefault::Vector(v) => {
                let mut a = [0.0; 4];
                a[..v.len().min(4)].copy_from_slice(&v[..v.len().min(4)]);
                SocketValue::Vector(a)
            }
            SocketDefault::Color(c) => SocketValue::Color(c),
            SocketDefault::Str(x) => SocketValue::String(x.into()),
            SocketDefault::Menu(x) => SocketValue::Menu(x.into()),
        }
    }
    let socket = |d: &frap_nodes::generated::SocketDef| Socket {
        name: d.name.into(),
        identifier: d.identifier.into(),
        ty: d.ty,
        enabled: d.enabled,
        hide_value: d.hide_value,
        default_value: value(&d.default),
    };
    let mut props = PropBag::default();
    for (name, d) in def.props {
        let Some(prop) = def.srna.find_prop(name) else {
            continue;
        };
        let v = match *d {
            NodePropDefault::None => Value::Id(None),
            NodePropDefault::Bool(b) => Value::Bool(b),
            NodePropDefault::Int(i) => Value::Int(i),
            NodePropDefault::Float(f) => Value::Float(f),
            NodePropDefault::Floats(f) => Value::FloatArray {
                data: f.iter().copied().collect(),
                shape: prop.array,
                tag: MathTag::for_subtype(prop.subtype),
            },
            NodePropDefault::Str(x) => match prop.kind {
                PropKind::Enum { .. } => Value::Enum(x.into()),
                _ => Value::Str(x.into()),
            },
            NodePropDefault::Flags(f) => Value::EnumSet(f.iter().map(|x| (*x).into()).collect()),
        };
        props.values.insert(prop.identifier, v);
    }
    Node {
        uid,
        name: def.name.into(),
        label: String::new(),
        bl_idname: def.bl_idname,
        location,
        width: def.width,
        parent: None,
        mute: false,
        hide: false,
        inputs: def.inputs.iter().map(socket).collect(),
        outputs: def.outputs.iter().map(socket).collect(),
        props,
    }
}

/// The factory shader graphs (verified on 5.2.4): a surface node at (-200, 100) linked to an
/// active output node at (200, 100), the output first in `nodes`.
fn surface_tree(output: &str, surface: &str, socket: &str) -> NodeTree {
    let def = |id: &str| node_type(id).expect("invariant: generated node type exists");
    let mut out = node_from_def(def(output), 0, [200.0, 100.0]);
    out.props
        .values
        .insert("is_active_output", Value::Bool(true));
    let surf = node_from_def(def(surface), 1, [-200.0, 100.0]);
    let mut tree = NodeTree::new(SHADER_NODETREE_NAME, NodeTreeType::Shader, true);
    tree.nodes = vec![out, surf];
    tree.links = vec![Link {
        from_node: 1,
        from_socket: socket.into(),
        to_node: 0,
        to_socket: "Surface".into(),
    }];
    tree.next_node_uid = 2;
    tree
}

fn mesh_from_dump(jm: &J, main: &Main) -> Mesh {
    let positions: Vec<[f32; 3]> = jm["positions"]
        .as_array()
        .map(|a| a.iter().map(f32s::<3>).collect())
        .unwrap_or_default();
    let edges: Vec<[i32; 2]> = jm["edges"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| {
                    [
                        e[0].as_i64().unwrap_or(0) as i32,
                        e[1].as_i64().unwrap_or(0) as i32,
                    ]
                })
                .collect()
        })
        .unwrap_or_default();
    let ints = |k: &str| -> Vec<i32> {
        jm[k]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect())
            .unwrap_or_default()
    };
    let mut offsets = ints("loop_start");
    offsets.push(jm["corner_verts"].as_array().map_or(0, |a| a.len()) as i32);
    let mut mesh = Mesh::new("");
    mesh.set_positions(positions);
    mesh.set_topology(offsets, ints("corner_verts"), edges);
    // The attributes exactly as dumped, in Blender's order.
    let mut attrs = AttributeSet::default();
    for a in jm["attributes"].as_array().into_iter().flatten() {
        let domain = match s(&a["domain"]) {
            "POINT" => Domain::Point,
            "EDGE" => Domain::Edge,
            "FACE" => Domain::Face,
            _ => Domain::Corner,
        };
        let d = a["data"].as_array().cloned().unwrap_or_default();
        let data = match s(&a["data_type"]) {
            "FLOAT" => AttrData::Float(Arc::new(
                d.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
            )),
            "INT" => AttrData::Int(Arc::new(
                d.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect(),
            )),
            "BOOLEAN" => AttrData::Bool(Arc::new(
                d.iter().map(|x| x.as_bool().unwrap_or(false)).collect(),
            )),
            "FLOAT2" => AttrData::Float2(Arc::new(d.iter().map(f32s::<2>).collect())),
            "FLOAT_VECTOR" => AttrData::Float3(Arc::new(d.iter().map(f32s::<3>).collect())),
            "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(d.iter().map(f32s::<4>).collect())),
            "INT32_2D" => AttrData::Int32x2(Arc::new(
                d.iter()
                    .map(|x| {
                        [
                            x[0].as_i64().unwrap_or(0) as i32,
                            x[1].as_i64().unwrap_or(0) as i32,
                        ]
                    })
                    .collect(),
            )),
            _ => continue,
        };
        attrs.insert(Attribute {
            name: SmolStr::new(s(&a["name"])),
            domain,
            data,
        });
    }
    *mesh.attributes_mut() = attrs;
    for uv in jm["uv_layers"].as_array().into_iter().flatten() {
        if uv["active"].as_bool() == Some(true) {
            mesh.uv_active = Some(SmolStr::new(s(&uv["name"])));
        }
        if uv["active_render"].as_bool() == Some(true) {
            mesh.uv_active_render = Some(SmolStr::new(s(&uv["name"])));
        }
    }
    mesh.materials = jm["materials"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| ref_id(main, m).and_then(IdRef::<MaterialId>::from_any))
        .collect();
    mesh
}

/// The factory Layout viewport (`startup.json` `view3d`): view matrix and shading.
fn virtual_view3d(j: &J) -> VirtualView3D {
    let rows = &j["view3d"]["region_3d"]["view_matrix"];
    let mut view_matrix = Mat4::IDENTITY;
    if rows.as_array().is_some_and(|r| r.len() == 4) {
        let r: [[f32; 4]; 4] = std::array::from_fn(|i| f32s::<4>(&rows[i]));
        // Dumped row-major (mathutils rows); glam is column-major.
        view_matrix = Mat4::from_cols_array_2d(&r).transpose();
    }
    let mut shading = View3DShading::default();
    let _ = load_rna(&mut shading, &j["view3d"]["space"]["shading"], &[]);
    VirtualView3D {
        view_matrix,
        shading,
    }
}

/// Object properties derived from others or served by hooks: never loaded generically.
const OBJECT_SKIP: &[&str] = &[
    "matrix_world",
    "matrix_local",
    "matrix_basis",
    "dimensions",
    "rotation_mode",
    "hide_select",
    "hide_viewport",
];
const CAMERA_SKIP: &[&str] = &["angle", "angle_x", "angle_y"];
const SCENE_SKIP: &[&str] = &["frame_current", "frame_start", "frame_end"];

/// The factory state: `(bpy.data, window)` of Blender 5.2.4 `--factory-startup`.
pub fn factory() -> (Main, WindowState) {
    build(factory_dump(), false)
}

/// The empty state of `frap --empty` and `wm.read_homefile(use_empty=True)`: the factory
/// scene (all its settings, its view layer and master collection) with no objects, collections,
/// camera or world (§5.7, verified).
pub fn empty() -> (Main, WindowState) {
    build(factory_dump(), true)
}

fn build(j: &J, empty: bool) -> (Main, WindowState) {
    let mut log = ChangeLog::default();
    let mut main = Main::default();
    let ids = &j["ids"];
    let list = |k: &str| ids[k].as_array().cloned().unwrap_or_default();

    if !empty {
        for jm in list("materials") {
            if OMITTED_MATERIALS.contains(&s(&jm["name"])) {
                continue;
            }
            let mut m = Material::new("");
            m.node_tree = surface_tree(
                "ShaderNodeOutputMaterial",
                "ShaderNodeBsdfPrincipled",
                "BSDF",
            );
            let _ = load_rna(&mut m, &jm, &[]);
            main.materials.insert(s(&jm["name"]), m, &mut log);
        }
        for jm in j["meshes_exact"].as_array().into_iter().flatten() {
            let mesh = mesh_from_dump(jm, &main);
            main.meshes.insert(s(&jm["name"]), mesh, &mut log);
        }
        for jc in list("cameras") {
            let mut cam = Camera::new("");
            let _ = load_rna(&mut cam, &jc, CAMERA_SKIP);
            main.cameras.insert(s(&jc["name"]), cam, &mut log);
        }
        for jl in list("lights") {
            let mut l = Light::new("", s(&jl["type"]));
            let _ = load_rna(&mut l, &jl, &[]);
            main.lights.insert(s(&jl["name"]), l, &mut log);
        }
        for jw in list("worlds") {
            let mut w = World::new("");
            w.node_tree = surface_tree(
                "ShaderNodeOutputWorld",
                "ShaderNodeBackground",
                "Background",
            );
            // The factory Background colour is the world colour (verified: 0.0509).
            if let Some(bg) = w.node_tree.nodes.get_mut(1)
                && let Some(c) = bg.inputs.iter_mut().find(|x| x.identifier == "Color")
            {
                let rgb = f32s::<3>(&jw["color"]);
                c.default_value = SocketValue::Color([rgb[0], rgb[1], rgb[2], 1.0]);
            }
            let _ = load_rna(&mut w, &jw, &[]);
            main.worlds.insert(s(&jw["name"]), w, &mut log);
        }
        let jobjects = list("objects");
        for jo in &jobjects {
            let mut ob = Object::new("", ref_id(&main, &jo["data"]));
            ob.rotation_mode = SmolStr::new(s(&jo["rotation_mode"]));
            ob.hide_select = jo["hide_select"].as_bool().unwrap_or(false);
            ob.hide_viewport = jo["hide_viewport"].as_bool().unwrap_or(false);
            let _ = load_rna(&mut ob, jo, OBJECT_SKIP);
            main.objects.insert(s(&jo["name"]), ob, &mut log);
        }
        for jo in &jobjects {
            let parent = ref_id(&main, &jo["parent"]).and_then(IdRef::<ObjectId>::from_any);
            if let (Some(p), Some(me)) = (parent, main.objects.find(s(&jo["name"])))
                && let Some(o) = main.objects.get_mut(me, &mut log)
            {
                o.parent = Some(p);
            }
        }
        for jc in list("collections") {
            let mut c = Collection::new("");
            c.hide_select = jc["hide_select"].as_bool().unwrap_or(false);
            c.hide_viewport = jc["hide_viewport"].as_bool().unwrap_or(false);
            let _ = load_rna(&mut c, &jc, &["hide_select", "hide_viewport"]);
            c.objects = jc["objects"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|o| ref_id(&main, o).and_then(IdRef::from_any))
                .collect();
            main.collections.insert(s(&jc["name"]), c, &mut log);
        }
        for jc in list("collections") {
            let children: Vec<_> = jc["collection_children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|o| ref_id(&main, o).and_then(IdRef::from_any))
                .collect();
            if let Some(r) = main.collections.find(s(&jc["name"]))
                && !children.is_empty()
                && let Some(c) = main.collections.get_mut(r, &mut log)
            {
                c.children = children;
            }
        }
    }

    // The scene. The master collection's children are the top-level collections (the dump
    // does not descend into the embedded master collection; the factory has one, "Collection").
    let js = &list("scenes")[0];
    let top: Vec<_> = main
        .collections
        .iter()
        .map(|(r, _)| r)
        .filter(|r| !main.collections.iter().any(|(_, c)| c.children.contains(r)))
        .collect();
    let mut sc = Scene::new("", Vec::new());
    let _ = load_rna(&mut sc, js, SCENE_SKIP);
    sc.frame_start = js["frame_start"].as_i64().unwrap_or(1) as i32;
    sc.frame_end = js["frame_end"].as_i64().unwrap_or(250) as i32;
    sc.frame_current = js["frame_current"].as_i64().unwrap_or(1) as i32;
    sc.collection.children = top;
    if !empty {
        sc.camera = ref_id(&main, &js["camera"]).and_then(IdRef::from_any);
        sc.world = ref_id(&main, &js["world"]).and_then(IdRef::from_any);
    }
    let jvl = &js["view_layers"][0];
    let mut vl = ViewLayer::new_synced(s(&jvl["name"]), &sc, &main.collections, &main.objects);
    // Factory selection, active object and active layer collection.
    let selected: Vec<&str> = j["context"]["selected_objects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(s)
        .collect();
    for b in &mut vl.bases {
        if selected.contains(&main.objects.name_of(b.object).unwrap_or_default()) {
            b.flag.insert(BaseFlag::SELECTED);
        }
    }
    vl.active = j["context"]["active_object"]
        .as_str()
        .and_then(|n| main.objects.find(n))
        .filter(|a| vl.bases.iter().any(|b: &Base| b.object == *a));
    let alc =
        ref_id(&main, &jvl["active_layer_collection"]["collection"]).and_then(IdRef::from_any);
    vl.active_layer_collection = vl.layer_collection.path_of(alc).unwrap_or_default();
    load_layer_flags(&mut vl.layer_collection, &jvl["layer_collection"]);
    sc.view_layers = vec![vl];
    let scene = main.scenes.insert(s(&js["name"]), sc, &mut log);
    layers::sync_scene(&mut main, scene, &mut log);
    let win = WindowState {
        scene,
        view_layer: SmolStr::new(s(&j["window"]["view_layer"])),
        view3d: virtual_view3d(j),
    };
    (main, win)
}

/// The dumped flags of the root layer collection (the dump does not descend into children).
fn load_layer_flags(lc: &mut LayerCollection, j: &J) {
    let b = |k: &str, d: bool| j[k].as_bool().unwrap_or(d);
    lc.exclude = b("exclude", lc.exclude);
    lc.hide_viewport = b("hide_viewport", lc.hide_viewport);
    lc.holdout = b("holdout", lc.holdout);
    lc.indirect_only = b("indirect_only", lc.indirect_only);
}
