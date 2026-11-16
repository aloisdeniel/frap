// SPDX-License-Identifier: GPL-3.0-or-later
//! Factory startup (ARCHITECTURE §5.7). M0 builds it from `spec/m0/startup.json`, a dump of
//! Blender 5.2.4 `--factory-startup` (spec/m0/dump_m0.py). The grease-pencil material
//! "Dots Stroke" is omitted (listed deviation, §5.7).

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain, Mesh, SHARP_FACE};
use frap_nodes::{Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketType, SocketValue};
use frap_rna::{IdRef, MaterialId, ObjectId};
use serde_json::Value as J;
use smol_str::SmolStr;

use crate::context::{
    Base, BaseFlag, LayerCollection, LayerCollectionPath, ViewLayer, VirtualView3D, WindowState,
};
use crate::data::{ChangeLog, Main};
use crate::generated::plain::{View3DCursor, View3DShading};
use crate::id::{Camera, Collection, Light, Material, Object, SHADER_NODETREE_NAME, Scene};

const STARTUP_JSON: &str = include_str!("../../../spec/m0/startup.json");

fn f32s<const N: usize>(j: &J) -> [f32; N] {
    let a = j.as_array().expect("invariant: startup.json array");
    std::array::from_fn(|i| a[i].as_f64().expect("invariant: startup.json number") as f32)
}
fn s(j: &J) -> &str {
    j.as_str().expect("invariant: startup.json string")
}

/// The factory state: `(bpy.data, window)`.
pub fn factory() -> (Main, WindowState) {
    let j: J = serde_json::from_str(STARTUP_JSON).expect("invariant: startup.json parses");
    let mut log = ChangeLog::default();
    let mut main = Main::default();

    for jm in j["materials"].as_array().expect("invariant") {
        main.materials.insert(
            s(&jm["name"]),
            {
                // Factory "Material" (verified on 5.2.4): its viewport values equal the
                // Principled inputs of the M0 dump; alpha_threshold 0 (fresh: 0.5).
                let base_color = f32s::<4>(&jm["base_color"]);
                let metallic = jm["metallic"].as_f64().unwrap_or(0.0) as f32;
                let roughness = jm["roughness"].as_f64().unwrap_or(0.5) as f32;
                let mut m = Material::new("");
                m.node_tree = m0_principled_tree(base_color, metallic, roughness);
                m.diffuse_color = base_color;
                m.metallic = metallic;
                m.roughness = roughness;
                m.alpha_threshold = 0.0;
                m.use_backface_culling = jm["use_backface_culling"].as_bool().unwrap_or(false);
                m
            },
            &mut log,
        );
    }
    for jm in j["meshes"].as_array().expect("invariant") {
        let mesh = mesh_from_dump(jm, &main);
        main.meshes.insert(s(&jm["name"]), mesh, &mut log);
    }
    for name in j["cameras"].as_array().expect("invariant") {
        // Factory Camera (verified): clip_end 100 and ortho_scale 7.3143 (fresh: 1000, 6).
        let mut cam = Camera::new("");
        cam.fields.clip_end = 100.0;
        cam.fields.ortho_scale = 7.314_285_8;
        main.cameras.insert(s(name), cam, &mut log);
    }
    for jl in j["lights"].as_array().expect("invariant") {
        main.lights.insert(
            s(&jl["name"]),
            {
                // Factory Light (verified): 1000 W, shadow_soft_size 0.1 (fresh: 10 W, 0).
                let mut l = Light::new("", s(&jl["type"]));
                l.fields.energy = jl["energy"].as_f64().unwrap_or(1000.0) as f32;
                l.fields.shadow_soft_size = 0.1;
                l
            },
            &mut log,
        );
    }
    for jo in j["objects"].as_array().expect("invariant") {
        let data = jo["data"].as_str().and_then(|d| match s(&jo["type"]) {
            "MESH" => main.meshes.find(d).map(|r| r.any()),
            "CAMERA" => main.cameras.find(d).map(|r| r.any()),
            "LIGHT" => main.lights.find(d).map(|r| r.any()),
            _ => None,
        });
        let mut ob = Object::new("", data);
        ob.location = f32s::<3>(&jo["location"]);
        ob.rotation_mode = s(&jo["rotation_mode"]).into();
        ob.rotation_euler = f32s::<3>(&jo["rotation_euler"]);
        ob.scale = f32s::<3>(&jo["scale"]);
        main.objects.insert(s(&jo["name"]), ob, &mut log);
    }
    let find_ob = |main: &Main, n: &J| -> IdRef<ObjectId> {
        main.objects
            .find(s(n))
            .expect("invariant: startup object exists")
    };
    for jc in j["collections"].as_array().expect("invariant") {
        let objects = jc["objects"]
            .as_array()
            .expect("invariant")
            .iter()
            .map(|n| find_ob(&main, n))
            .collect();
        main.collections.insert(
            s(&jc["name"]),
            Collection {
                objects,
                ..Collection::new("")
            },
            &mut log,
        );
    }
    let js = &j["scene"];
    let selected: Vec<&str> = js["selected_objects"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(s)
        .collect();
    let bases = js["base_order"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(|n| Base {
            object: find_ob(&main, n),
            flag: if selected.contains(&s(n)) {
                BaseFlag::SELECTED
            } else {
                BaseFlag::empty()
            },
        })
        .collect();
    let children: Vec<_> = js["master_collection_children"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(|n| {
            main.collections
                .find(s(n))
                .expect("invariant: startup collection exists")
        })
        .collect();
    let mut layer_collection = LayerCollection::new(None);
    layer_collection.children = children
        .iter()
        .map(|c| LayerCollection::new(Some(*c)))
        .collect();
    let view_layer = ViewLayer {
        name: s(&js["view_layer"]).into(),
        bases,
        active: js["active_object"]
            .as_str()
            .and_then(|n| main.objects.find(n)),
        layer_collection,
        // Factory Layout: the active layer collection is "Collection" (new objects go there).
        active_layer_collection: LayerCollectionPath(if children.is_empty() {
            Default::default()
        } else {
            smallvec::smallvec![0]
        }),
    };
    let scene = main.scenes.insert(
        s(&js["name"]),
        {
            let mut sc = Scene::new("", vec![view_layer]);
            sc.collection.children = children;
            sc.frame_current = js["frame_current"].as_i64().unwrap_or(1) as i32;
            sc.cursor = View3DCursor {
                location: f32s::<3>(&js["cursor_location"]),
                ..View3DCursor::default()
            };
            // The factory scene camera is the "Camera" object (verified). The factory World
            // is not in the M0 dump; WP 1.10 loads it (and every other setting) from
            // `startup.json`.
            sc.camera = main.objects.find("Camera");
            sc
        },
        &mut log,
    );
    let win = WindowState {
        scene,
        view_layer: s(&js["view_layer"]).into(),
        // WP 1.10 dumps the factory Layout `region_3d.view_matrix` into startup.json (§5.7);
        // until then `align='VIEW'` is NotImplemented in the placement helper.
        view3d: VirtualView3D {
            view_matrix: glam::Mat4::IDENTITY,
            shading: View3DShading::default(),
        },
    };
    (main, win)
}

fn mesh_from_dump(jm: &J, main: &Main) -> Mesh {
    let positions: Vec<[f32; 3]> = jm["positions"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(f32s::<3>)
        .collect();
    let edges: Vec<[i32; 2]> = jm["edges"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(|e| {
            [
                e[0].as_i64().unwrap_or(0) as i32,
                e[1].as_i64().unwrap_or(0) as i32,
            ]
        })
        .collect();
    let mut offsets = vec![0i32];
    let mut corner_verts = Vec::new();
    for f in jm["faces"].as_array().expect("invariant") {
        corner_verts.extend(
            f.as_array()
                .expect("invariant")
                .iter()
                .map(|v| v.as_i64().unwrap_or(0) as i32),
        );
        offsets.push(corner_verts.len() as i32);
    }
    let mut mesh = Mesh::new("");
    mesh.set_positions(positions);
    mesh.set_topology(offsets, corner_verts, edges);
    if let Some(sharp) = jm["sharp_face"].as_array() {
        mesh.attributes_mut().insert(Attribute {
            name: SHARP_FACE.into(),
            domain: Domain::Face,
            data: AttrData::Bool(Arc::new(
                sharp.iter().map(|b| b.as_bool().unwrap_or(false)).collect(),
            )),
        });
    }
    for (name, data) in jm["uv_layers"].as_object().expect("invariant") {
        mesh.attributes_mut().insert(Attribute {
            name: SmolStr::new(name),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(
                data.as_array()
                    .expect("invariant")
                    .iter()
                    .map(f32s::<2>)
                    .collect(),
            )),
        });
    }
    mesh.uv_active = jm["uv_active"].as_str().map(SmolStr::new);
    mesh.uv_active_render = mesh.uv_active.clone();
    mesh.materials = jm["materials"]
        .as_array()
        .expect("invariant")
        .iter()
        .map(|n| n.as_str().and_then(|n| main.materials.find(n)))
        .collect::<Vec<Option<IdRef<MaterialId>>>>();
    mesh
}

/// The empty state of `frap --empty` and `wm.read_homefile(use_empty=True)`: one Scene with
/// its ViewLayer and master collection, no objects (§5.7). WP 1.10.
pub fn empty() -> (Main, WindowState) {
    todo!("WP 1.10: startup::empty (`read_homefile(use_empty=True)` dump)")
}

/// M0/M1-W0 stand-in for the factory material's node tree: "Principled BSDF" (Base Color,
/// Metallic, Roughness, Alpha inputs only) linked to "Material Output". Node names, types and
/// locations are the verified factory ones. WP 1.10 (startup) and WP 1.11 (`materials.new`
/// graph with every socket) replace it.
fn m0_principled_tree(base_color: [f32; 4], metallic: f32, roughness: f32) -> NodeTree {
    fn socket(name: &str, ty: SocketType, v: SocketValue) -> Socket {
        Socket {
            name: name.into(),
            identifier: name.into(),
            ty,
            enabled: true,
            hide_value: false,
            default_value: v,
        }
    }
    fn node(uid: u32, name: &str, bl_idname: &'static str, location: [f32; 2]) -> Node {
        Node {
            uid,
            name: name.into(),
            label: String::new(),
            bl_idname,
            location,
            width: 240.0,
            parent: None,
            mute: false,
            hide: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
            props: PropBag::default(),
        }
    }
    let mut out = node(
        0,
        "Material Output",
        "ShaderNodeOutputMaterial",
        [200.0, 100.0],
    );
    out.inputs = vec![socket("Surface", SocketType::Shader, SocketValue::None)];
    let mut bsdf = node(
        1,
        "Principled BSDF",
        "ShaderNodeBsdfPrincipled",
        [-200.0, 100.0],
    );
    bsdf.inputs = vec![
        socket(
            "Base Color",
            SocketType::Rgba,
            SocketValue::Color(base_color),
        ),
        socket("Metallic", SocketType::Value, SocketValue::Float(metallic)),
        socket(
            "Roughness",
            SocketType::Value,
            SocketValue::Float(roughness),
        ),
        socket("Alpha", SocketType::Value, SocketValue::Float(1.0)),
    ];
    bsdf.outputs = vec![socket("BSDF", SocketType::Shader, SocketValue::None)];
    let mut tree = NodeTree::new(SHADER_NODETREE_NAME, NodeTreeType::Shader, true);
    tree.nodes = vec![out, bsdf];
    tree.links = vec![Link {
        from_node: 1,
        from_socket: "BSDF".into(),
        to_node: 0,
        to_socket: "Surface".into(),
    }];
    tree.next_node_uid = 2;
    tree
}
