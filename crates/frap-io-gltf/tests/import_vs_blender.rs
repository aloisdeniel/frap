// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.17 acceptance: frap's in-process glTF import equals Blender 5.2.4's
//! `import_scene.gltf`, field by field, on the Blender-exported fixtures and the vendored
//! Khronos subset (`tests/data/gltf-blender/dumps/*.json`, written by
//! `tools/blender/make_gltf_fixtures.py`).
//!
//! Each dump holds the source file, the import settings and Blender's resulting state. The
//! test starts from the empty startup (`read_factory_settings(use_empty=True)`: one "Scene",
//! one "ViewLayer", nothing else), imports the same file with the same settings, writes frap's
//! state in the dump's JSON shape and compares the two (`frap-io-gltf` cannot dev-depend on
//! `frap-golden`, rule 2.2.11; the `GoldenState` comparison runs through the operator in WP
//! 1.23).
//!
//! Compared: scenes (collection tree, view layers, base order, selection, active object,
//! layer-collection exclusion), `bpy.data` order and names of every collection, objects
//! (type, data, parent, transform incl. `rotation_euler` and `matrix_parent_inverse`, empty
//! size, material slot links and materials, collections), meshes (positions, edges, faces,
//! corner normals, normals domain, UV maps, every non-internal attribute with its values,
//! material slots), materials (non-node properties), images (incl. `filepath` relative to the
//! source), cameras (incl. sensor and shift) and lights (incl. shadow), plus the custom
//! properties (glTF extras) of every one of these IDs. Material node graphs are compared by `material_graphs_match_blender`,
//! which needs WP 1.11's `raise`.

use std::path::{Path, PathBuf};

use frap_io_gltf::import::{ImportSettings, import_file};
use frap_mesh::{AttrData, Mesh};
use frap_rna::{IdAny, IdCode, IdHeader, IdProp, IdRef, ObjectId};
use frap_scene::generated::plain::View3DShading;
use frap_scene::id::{Collection, Scene, SlotLink};
use frap_scene::{
    ChangeLog, LayerCollection, LayerCollectionPath, Main, ViewLayer, VirtualView3D, WindowState,
};
use serde_json::{Value as J, json};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn dumps() -> Vec<PathBuf> {
    let dir = repo().join("tests/data/gltf-blender/dumps");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    v.sort();
    v
}

/// `read_factory_settings(use_empty=True)`: one scene with one view layer.
fn empty_state() -> (Main, WindowState) {
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    let vl = ViewLayer {
        name: "ViewLayer".into(),
        bases: Vec::new(),
        active: None,
        layer_collection: LayerCollection::new(None),
        active_layer_collection: LayerCollectionPath::default(),
    };
    let scene = main
        .scenes
        .insert("Scene", Scene::new("", vec![vl]), &mut log);
    let win = WindowState {
        scene,
        view_layer: "ViewLayer".into(),
        view3d: VirtualView3D {
            view_matrix: frap_math::glam::Mat4::IDENTITY,
            shading: View3DShading::default(),
        },
    };
    (main, win)
}

// ------------------------------------------------------------------ frap state as dump JSON

fn f(x: f32) -> J {
    json!(x as f64)
}
fn fv(v: &[f32]) -> J {
    J::Array(v.iter().map(|&x| f(x)).collect())
}

/// An ID property as `make_gltf_fixtures.py`'s `idprop` writes it.
fn idprop(p: &IdProp) -> J {
    match p {
        IdProp::Int(i) => json!(i),
        IdProp::Float(x) => json!(x),
        IdProp::Bool(b) => json!(b),
        IdProp::Str(s) => json!(s.as_str()),
        IdProp::IntArray(v) => json!(v),
        IdProp::FloatArray(v) => json!(v),
        IdProp::Group(g) => J::Object(g.iter().map(|(k, v)| (k.to_string(), idprop(v))).collect()),
        IdProp::List(v) => J::Array(v.iter().map(idprop).collect()),
        other => panic!("unexpected ID property {other:?}"),
    }
}

/// `ID.keys()` with their values.
fn custom(h: &IdHeader) -> J {
    J::Object(
        h.custom
            .iter()
            .map(|(k, v)| (k.to_string(), idprop(v)))
            .collect(),
    )
}

/// Matrix rows, as `[list(row) for row in matrix]` reads them (glam is column-major).
fn matrix_rows(m: &frap_math::glam::Mat4) -> J {
    J::Array((0..4).map(|r| fv(&m.row(r).to_array())).collect())
}

/// `Image.filepath` relative to the imported file's directory ("" when unset).
fn image_path(filepath: &str, source: &Path) -> String {
    if filepath.is_empty() {
        return String::new();
    }
    let base = source
        .parent()
        .and_then(|d| d.canonicalize().ok())
        .unwrap_or_default();
    let abs = Path::new(filepath)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filepath));
    abs.strip_prefix(&base)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| filepath.to_owned())
}

fn name_of(main: &Main, id: Option<IdAny>) -> J {
    match id.and_then(|a| main.header(a)) {
        Some(h) => json!(h.name.as_str()),
        None => J::Null,
    }
}

fn ob_name(main: &Main, o: IdRef<ObjectId>) -> J {
    name_of(main, Some(o.any()))
}

fn layer_collection(main: &Main, lc: &LayerCollection) -> J {
    json!({
        "collection": lc.collection.map_or(json!("Scene Collection"), |c| name_of(main, Some(c.any()))),
        "exclude": lc.exclude,
        "children": lc.children.iter().map(|c| layer_collection(main, c)).collect::<Vec<_>>(),
    })
}

fn collection_json(main: &Main, c: &Collection) -> (Vec<J>, Vec<J>) {
    (
        c.objects.iter().map(|o| ob_name(main, *o)).collect(),
        c.children
            .iter()
            .map(|c| name_of(main, Some(c.any())))
            .collect(),
    )
}

/// sRGB byte → linear float, as `ByteColorAttributeValue.color` reads it.
fn srgb_byte_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c < 0.040_449_936 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        ((c + 0.055) * (1.0 / 1.055)).powf(2.4)
    }
}

fn attribute_values(data: &AttrData) -> Vec<J> {
    match data {
        AttrData::Float(v) => v.iter().map(|&x| f(x)).collect(),
        AttrData::Int(v) => v.iter().map(|&x| json!(x)).collect(),
        AttrData::Int8(v) => v.iter().map(|&x| json!(x)).collect(),
        AttrData::Bool(v) => v.iter().map(|&x| json!(x)).collect(),
        AttrData::Float2(v) => v.iter().map(|x| fv(x)).collect(),
        AttrData::Float3(v) => v.iter().map(|x| fv(x)).collect(),
        AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            v.iter().map(|x| fv(x)).collect()
        }
        AttrData::ByteColor(v) => v
            .iter()
            .map(|c| {
                json!([
                    srgb_byte_to_linear(c[0]) as f64,
                    srgb_byte_to_linear(c[1]) as f64,
                    srgb_byte_to_linear(c[2]) as f64,
                    c[3] as f64 / 255.0
                ])
            })
            .collect(),
        AttrData::Int16x2(v) => v.iter().map(|x| json!([x[0], x[1]])).collect(),
        AttrData::Int32x2(v) => v.iter().map(|x| json!([x[0], x[1]])).collect(),
        AttrData::Float4x4(v) => v
            .iter()
            .map(|m| J::Array(m.iter().map(|r| fv(r)).collect()))
            .collect(),
        AttrData::String(v) => v.iter().map(|s| json!(s.as_str())).collect(),
        _ => Vec::new(),
    }
}

fn mesh_json(main: &Main, me: &Mesh) -> J {
    let uv_names: Vec<&str> = me.uv_maps().iter().map(|(n, _)| *n).collect();
    let faces: Vec<J> = (0..me.faces_num())
        .map(|i| json!(me.corner_verts()[me.face(i)]))
        .collect();
    let attributes: Vec<J> = me
        .attributes()
        .iter()
        .filter(|a| {
            !a.name.starts_with('.') && a.name != "position" && !uv_names.contains(&a.name.as_str())
        })
        .map(|a| {
            json!({
                "name": a.name.as_str(),
                "domain": a.domain.identifier(),
                "data_type": a.data.data_type(),
                "data": attribute_values(&a.data),
            })
        })
        .collect();
    json!({
        "name": me.header.name.as_str(),
        "positions": me.positions().iter().map(|p| fv(p)).collect::<Vec<_>>(),
        "edges": me.edges().iter().map(|e| json!(e)).collect::<Vec<_>>(),
        "faces": faces,
        "corner_normals": me.corner_normals().iter().map(|n| fv(n)).collect::<Vec<_>>(),
        "normals_domain": me.normals_domain().identifier(),
        "uv_layers": me.uv_maps().iter().map(|(n, d)| json!({"name": n, "data": d.iter().map(|uv| fv(uv)).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "attribute_names": me.attributes().iter().map(|a| json!(a.name.as_str())).collect::<Vec<_>>(),
        "attributes": attributes,
        "materials": me.materials.iter().map(|m| name_of(main, m.map(|m| m.any()))).collect::<Vec<_>>(),
        "custom": custom(&me.header),
    })
}

fn frap_state(main: &Main, win: &WindowState, source: &Path) -> J {
    let scenes: Vec<J> = main
        .scenes
        .iter()
        .map(|(_, sc)| {
            let (objects, children) = collection_json(main, &sc.collection);
            json!({
                "name": sc.header.name.as_str(),
                "custom": custom(&sc.header),
                "collection": {"objects": objects, "children": children},
                "view_layers": sc.view_layers.iter().map(|vl| json!({
                    "name": vl.name.as_str(),
                    "objects": vl.bases.iter().map(|b| ob_name(main, b.object)).collect::<Vec<_>>(),
                    "selected": vl.bases.iter().filter(|b| b.selected()).map(|b| ob_name(main, b.object)).collect::<Vec<_>>(),
                    "active": vl.active.map_or(J::Null, |a| ob_name(main, a)),
                    "layer_collection": layer_collection(main, &vl.layer_collection),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let collections: Vec<J> = main
        .collections
        .iter()
        .map(|(_, c)| {
            let (objects, children) = collection_json(main, c);
            json!({"name": c.header.name.as_str(), "objects": objects, "children": children})
        })
        .collect();
    let objects: Vec<J> = main
        .objects
        .iter()
        .map(|(r, ob)| {
            // `MaterialSlot.link` / `.material`: the data's list unless the slot's override
            // links the object's own material.
            let slots: Vec<J> = match ob.data {
                Some(d) if d.code == IdCode::ME => main
                    .meshes
                    .get(d.typed().unwrap())
                    .map(|me| {
                        me.materials
                            .iter()
                            .enumerate()
                            .map(|(i, m)| match ob.material_slot_overrides.get(i) {
                                Some(o) if o.link == SlotLink::Object => {
                                    json!(["OBJECT", name_of(main, o.material.map(|m| m.any()))])
                                }
                                _ => json!(["DATA", name_of(main, m.map(|m| m.any()))]),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            let mut users = Vec::new();
            for (_, c) in main.collections.iter() {
                if c.objects.contains(&r) {
                    users.push(json!(c.header.name.as_str()));
                }
            }
            for (_, s) in main.scenes.iter() {
                if s.collection.objects.contains(&r) {
                    users.push(json!("Scene Collection"));
                }
            }
            json!({
                "name": ob.header.name.as_str(),
                "type": ob.object_type(),
                "data": name_of(main, ob.data),
                "parent": ob.parent.map_or(J::Null, |p| ob_name(main, p)),
                "parent_type": ob.parent_type.as_str(),
                "location": fv(&ob.location),
                "rotation_mode": ob.rotation_mode.as_str(),
                "rotation_quaternion": fv(&ob.rotation_quaternion),
                "rotation_euler": fv(&ob.rotation_euler),
                "scale": fv(&ob.scale),
                "matrix_parent_inverse": matrix_rows(&ob.matrix_parent_inverse),
                "empty_display_size": f(ob.empty_display_size),
                "material_slots": slots,
                "users_collection": users,
                "custom": custom(&ob.header),
            })
        })
        .collect();
    let meshes: Vec<J> = main
        .meshes
        .iter()
        .map(|(_, me)| mesh_json(main, me))
        .collect();
    let materials: Vec<J> = main
        .materials
        .iter()
        .map(|(_, m)| {
            let mut nodes = serde_json::Map::new();
            for n in &m.node_tree.nodes {
                nodes.insert(
                    n.name.to_string(),
                    json!({"bl_idname": n.bl_idname, "mute": n.mute}),
                );
            }
            let uid_name = |uid: u32| {
                m.node_tree
                    .nodes
                    .iter()
                    .find(|n| n.uid == uid)
                    .map(|n| n.name.to_string())
                    .unwrap_or_default()
            };
            let mut links: Vec<Vec<String>> = m
                .node_tree
                .links
                .iter()
                .map(|l| {
                    vec![
                        uid_name(l.from_node),
                        l.from_socket.to_string(),
                        uid_name(l.to_node),
                        l.to_socket.to_string(),
                    ]
                })
                .collect();
            links.sort();
            json!({
                "name": m.header.name.as_str(),
                "use_backface_culling": m.use_backface_culling,
                "surface_render_method": m.surface_render_method.as_str(),
                "diffuse_color": fv(&m.diffuse_color),
                "metallic": f(m.metallic),
                "roughness": f(m.roughness),
                "custom": custom(&m.header),
                "nodes": nodes,
                "links": links,
            })
        })
        .collect();
    let images: Vec<J> = main
        .images
        .iter()
        .map(|(_, im)| {
            json!({
                "name": im.header.name.as_str(),
                "filepath": image_path(&im.filepath, source),
                "custom": custom(&im.header),
                "source": im.source.as_str(),
                "alpha_mode": im.alpha_mode.as_str(),
                "packed": im.packed_file.is_some(),
                "packed_size": im.packed_file.as_ref().map_or(0, |p| p.bytes.len()),
                "colorspace": im.colorspace_settings.name.as_str(),
            })
        })
        .collect();
    let cameras: Vec<J> = main
        .cameras
        .iter()
        .map(|(_, c)| {
            let fl = &c.fields;
            json!({
                "name": c.header.name.as_str(),
                "type": fl.r#type.as_str(),
                "lens": f(fl.lens),
                "sensor_fit": fl.sensor_fit.as_str(),
                "lens_unit": fl.lens_unit.as_str(),
                "ortho_scale": f(fl.ortho_scale),
                "clip_start": f(fl.clip_start),
                "clip_end": f(fl.clip_end),
                "sensor_width": f(fl.sensor_width),
                "sensor_height": f(fl.sensor_height),
                "shift_x": f(fl.shift_x),
                "shift_y": f(fl.shift_y),
                "custom": custom(&c.header),
            })
        })
        .collect();
    let lights: Vec<J> = main
        .lights
        .iter()
        .map(|(_, l)| {
            let mut j = json!({
                "name": l.header.name.as_str(),
                "type": l.r#type.as_str(),
                "energy": f(l.fields.energy),
                "color": fv(&l.fields.color),
                "use_shadow": l.fields.use_shadow,
                "shadow_soft_size": f(l.fields.shadow_soft_size),
                "custom": custom(&l.header),
            });
            if l.r#type == "SPOT" {
                j["spot_size"] = f(l.fields.spot_size);
                j["spot_blend"] = f(l.fields.spot_blend);
            }
            j
        })
        .collect();
    json!({
        "window_scene": name_of(main, Some(win.scene.any())),
        "scenes": scenes,
        "collections": collections,
        "objects": objects,
        "meshes": meshes,
        "materials": materials,
        "images": images,
        "cameras": cameras,
        "lights": lights,
    })
}

// ------------------------------------------------------------------ comparison

/// Absolute tolerance for a float at `path`.
fn tolerance(path: &str) -> f64 {
    if path.contains("/corner_normals/") {
        1e-5
    } else if path.contains("/data_type=BYTE_COLOR/") {
        // Blender converts with an SSE `powf` approximation: at most one byte apart.
        0.01
    } else {
        2e-6
    }
}

/// Compares `b` (Blender) with `f` (frap); `skip` lists object keys not compared.
fn compare(path: &str, b: &J, f: &J, skip: &[&str], out: &mut Vec<String>) {
    if out.len() > 40 {
        return;
    }
    match (b, f) {
        (J::Object(bo), J::Object(fo)) => {
            for (k, bv) in bo {
                if skip.contains(&k.as_str()) {
                    continue;
                }
                let mut p = format!("{path}/{k}");
                if k == "data"
                    && let Some(dt) = bo.get("data_type").and_then(|t| t.as_str())
                {
                    p = format!("{path}/data_type={dt}/data");
                }
                match fo.get(k) {
                    Some(fv) => compare(&p, bv, fv, skip, out),
                    None => out.push(format!("{p}: missing in frap")),
                }
            }
            for k in fo.keys() {
                if !bo.contains_key(k) && !skip.contains(&k.as_str()) {
                    out.push(format!("{path}/{k}: only in frap"));
                }
            }
        }
        (J::Array(ba), J::Array(fa)) => {
            if ba.len() != fa.len() {
                out.push(format!(
                    "{path}: length {} (Blender) != {} (frap); Blender {} / frap {}",
                    ba.len(),
                    fa.len(),
                    short(b),
                    short(f)
                ));
                return;
            }
            // Name-keyed lists get the name in the path.
            for (i, (x, y)) in ba.iter().zip(fa).enumerate() {
                let label = x
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map_or_else(|| i.to_string(), |n| format!("{i}:{n}"));
                compare(&format!("{path}/{label}"), x, y, skip, out);
            }
        }
        (J::Number(x), J::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            let tol = tolerance(path);
            if (x - y).abs() > tol && !(x.is_nan() && y.is_nan()) {
                out.push(format!("{path}: Blender {x} != frap {y}"));
            }
        }
        _ => {
            if b != f {
                out.push(format!("{path}: Blender {} != frap {}", short(b), short(f)));
            }
        }
    }
}

fn short(v: &J) -> String {
    let s = v.to_string();
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s
    }
}

/// Imports the dump's source with its settings into an empty state.
fn source_of(dump: &J) -> PathBuf {
    repo().join(dump["source"].as_str().unwrap())
}

fn run_case(dump: &J) -> (Main, WindowState, frap_io_gltf::import::ImportReport) {
    let src = source_of(dump);
    let mut settings = ImportSettings::default();
    for (k, v) in dump["settings"].as_object().unwrap() {
        assert!(settings.set(k, v), "unknown setting {k}={v}");
    }
    let (mut main, win) = empty_state();
    let mut log = ChangeLog::default();
    let report = import_file(&mut main, &win, &src, &settings, &mut log)
        .unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    (main, win, report)
}

/// Keys never compared: provenance, and the node graphs (WP 1.11's `raise`).
const NOT_COMPARED: [&str; 5] = ["blender", "source", "settings", "nodes", "links"];

#[test]
fn import_matches_blender_dumps() {
    let all = dumps();
    let mut failures = Vec::new();
    for p in &all {
        let dump: J = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        let (main, win, _report) = run_case(&dump);
        let mut state = frap_state(&main, &win, &source_of(&dump));
        // Only non-internal attribute names are compared (internal `.select_*` order is an
        // implementation detail of the RNA calls the add-on makes).
        for m in state["meshes"].as_array_mut().unwrap() {
            let names: Vec<J> = m["attribute_names"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| !n.as_str().unwrap().starts_with('.'))
                .cloned()
                .collect();
            m["attribute_names"] = J::Array(names);
        }
        let mut dump = dump;
        for m in dump["meshes"].as_array_mut().unwrap() {
            let names: Vec<J> = m["attribute_names"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| !n.as_str().unwrap().starts_with('.'))
                .cloned()
                .collect();
            m["attribute_names"] = J::Array(names);
        }
        let mut out = Vec::new();
        compare("", &dump, &state, &NOT_COMPARED, &mut out);
        if !out.is_empty() {
            failures.push(format!(
                "== {}\n  {}",
                p.file_stem().unwrap().to_string_lossy(),
                out.join("\n  ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from Blender:\n{}",
        failures.len(),
        all.len(),
        failures.join("\n")
    );
}

#[test]
fn fixture_set_covers_the_acceptance() {
    let all = dumps();
    let names: Vec<String> = all
        .iter()
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let blender_fixtures: std::collections::BTreeSet<&str> = names
        .iter()
        .filter(|n| !n.starts_with("khr_"))
        .map(|n| n.split("__").next().unwrap())
        .collect();
    assert!(blender_fixtures.len() >= 5, "{blender_fixtures:?}");
    for required in [
        "cube__merge",
        "sphere_smooth__flat",
        "mixed_shading",
        "mixed_shading__smooth",
        "hierarchy",
        "loose",
        "khr_mesh_primitive_modes",
        "khr_multiple_scenes",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing case {required}"
        );
    }
    // Every Blender-exported fixture GLB has at least one dump.
    let dir = repo().join("tests/data/gltf-blender/fixtures");
    for e in std::fs::read_dir(dir).unwrap() {
        let stem = e
            .unwrap()
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            blender_fixtures.contains(stem.as_str()),
            "fixture {stem} has no dump"
        );
    }
}

/// Material node graphs (built by WP 1.11's `frap_material::raise`) against Blender's.
#[test]
#[ignore = "needs WP 1.11's frap_material::raise (W2 soft dependency); run once it lands"]
fn material_graphs_match_blender() {
    let mut failures = Vec::new();
    for p in dumps() {
        let dump: J = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let (main, win, _) = run_case(&dump);
        let state = frap_state(&main, &win, &source_of(&dump));
        let mut out = Vec::new();
        let names = |v: &J| -> Vec<(String, J, J)> {
            v["materials"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| {
                    let nodes: serde_json::Map<String, J> = m["nodes"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, n)| (k.clone(), json!({"bl_idname": n["bl_idname"]})))
                        .collect();
                    (
                        m["name"].as_str().unwrap().to_owned(),
                        J::Object(nodes),
                        m["links"].clone(),
                    )
                })
                .collect()
        };
        for ((bn, bnodes, blinks), (_, fnodes, flinks)) in names(&dump).iter().zip(names(&state)) {
            compare(
                &format!("/materials/{bn}/nodes"),
                bnodes,
                &fnodes,
                &[],
                &mut out,
            );
            compare(
                &format!("/materials/{bn}/links"),
                blinks,
                &flinks,
                &[],
                &mut out,
            );
        }
        if !out.is_empty() {
            failures.push(format!("== {}\n  {}", p.display(), out.join("\n  ")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn locate_blender() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BLENDER_BIN") {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let c = dir.join(exe);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    [
        home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender"),
        home.join("Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// The committed dumps are what Blender produces today (re-dumped from the committed fixture
/// GLBs into a temporary directory). Skipped without Blender; `FRAP_REQUIRE_BLENDER=1` makes
/// that a failure.
#[test]
fn dumps_match_live_blender() {
    let Some(blender) = locate_blender() else {
        assert!(
            std::env::var_os("FRAP_REQUIRE_BLENDER").is_none(),
            "Blender not found"
        );
        eprintln!("dumps_match_live_blender: Blender not found, skipped");
        return;
    };
    let out = std::env::temp_dir().join(format!("frap-gltf-import-{}", std::process::id()));
    let status = std::process::Command::new(&blender)
        .args(["-b", "--factory-startup", "-t", "8", "--python"])
        .arg(repo().join("tools/blender/make_gltf_fixtures.py"))
        .arg("--")
        .arg("--out")
        .arg(&out)
        .arg("--no-export")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run Blender");
    assert!(status.success(), "make_gltf_fixtures.py failed");
    let mut stale = Vec::new();
    for committed in dumps() {
        let name = committed.file_name().unwrap();
        let read = |p: &Path| -> J {
            let mut v: J = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
            v.as_object_mut().unwrap().remove("blender");
            v
        };
        if read(&committed) != read(&out.join("dumps").join(name)) {
            stale.push(name.to_string_lossy().into_owned());
        }
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        stale.is_empty(),
        "stale dumps (re-run make_gltf_fixtures.py): {stale:?}"
    );
}
