// SPDX-License-Identifier: GPL-3.0-or-later
//! Custom properties → `extras` (M3, WP 3.7), written only with `export_extras` (default False).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/com/extras.py` (`generate_extras`,
//! `__to_json_compatible`, `BLACK_LIST`), Apache-2.0, The glTF-Blender-IO authors, and the
//! callers that decide which datablock speaks for each glTF object: `blender/exp/nodes.py` (the
//! object), `blender/exp/obj_data.py` (the mesh or point cloud data), `material/material_utils.py`
//! (the material) and `blender/exp/gather.py` (the scene, or the exported collection with a
//! `collection` export).
//!
//! Every custom property of the ID except the black-listed ones becomes one `extras` member, in
//! the ID's key order: ints, floats, bools (Python bools are ints), strings, int/float arrays
//! (`to_list()`), groups (`to_dict()`) and lists of groups, recursively. No custom property
//! writes no `extras`. W0 wired `nodes.rs`, `mesh/gather.rs`, `materials/mod.rs` and `scenes.rs`
//! to these functions; the point cloud gatherer (`mesh/extract/pointcloud.rs`), the camera
//! gatherer (`cameras.rs`, `camera.py`) and the light gatherer (`lights.rs`, `lights.py`) use
//! [`id`].

use frap_gltf::json::Extras;
use frap_mesh::Mesh;
use frap_rna::{IdHeader, IdProp};
use frap_scene::id::{Material, Object, Scene};
use serde_json::{Map, Number, Value as J};

use crate::export::gather::Gather;

/// `export_extras` is honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_extras"];

/// Custom properties that are never exported (`BLACK_LIST`).
pub const BLACK_LIST: [&str; 9] = [
    "cycles",
    "cycles_visibility",
    "cycles_curves",
    "glTF2ExportSettings",
    "gltf2_mesh_applied",
    "gltf2_KHR_materials_variants_variants",
    "gltf2_active_variant",
    "gltf2_variant_default_materials",
    "gltf2_variant_mesh_data",
];

/// A float as JSON. Non-finite values are not valid JSON: the add-on's `json.dumps(…,
/// allow_nan=False)` fails the whole export ([`NonFinite`]); `null` only stands in until then.
fn float(x: f64) -> J {
    Number::from_f64(x).map_or(J::Null, J::Number)
}

/// `__to_json_compatible`: the JSON form of one ID property.
pub fn to_json(p: &IdProp) -> J {
    match p {
        IdProp::Int(i) => J::Number((*i).into()),
        IdProp::Float(f) => float(*f),
        IdProp::Bool(b) => J::Bool(*b),
        IdProp::Str(s) => J::String(s.to_string()),
        IdProp::IntArray(v) => J::Array(v.iter().map(|i| J::Number((*i).into())).collect()),
        IdProp::FloatArray(v) => J::Array(v.iter().map(|f| float(*f)).collect()),
        IdProp::Group(g) => J::Object(
            g.iter()
                .map(|(k, v)| (k.to_string(), to_json(v)))
                .collect::<Map<_, _>>(),
        ),
        IdProp::List(v) => J::Array(v.iter().map(to_json).collect()),
        // A kind added later (non-exhaustive): not JSON-convertible, as the add-on skips it.
        _ => J::Null,
    }
}

/// The first exported custom property holding a non-finite float, as the export error Blender
/// raises when it writes the JSON (`ValueError: Out of range float values are not JSON
/// compliant: nan`, a `RuntimeError` from the operator, nothing written). Checked by
/// [`crate::export::export`] after gathering.
#[derive(Debug, Default)]
pub struct NonFinite(pub Option<String>);

/// Python's `repr` of a non-finite float.
fn py_repr(x: f64) -> &'static str {
    if x.is_nan() {
        "nan"
    } else if x > 0.0 {
        "inf"
    } else {
        "-inf"
    }
}

/// The first non-finite float inside `p`.
fn non_finite(p: &IdProp) -> Option<f64> {
    match p {
        IdProp::Float(f) => (!f.is_finite()).then_some(*f),
        IdProp::FloatArray(v) => v.iter().copied().find(|f| !f.is_finite()),
        IdProp::Group(g) => g.values().find_map(non_finite),
        IdProp::List(v) => v.iter().find_map(non_finite),
        _ => None,
    }
}

/// Records the first non-finite custom property of `header` (exported keys only).
fn check_finite(g: &mut Gather, header: &IdHeader) {
    if g.state::<NonFinite>().0.is_some() {
        return;
    }
    let found = header
        .custom
        .iter()
        .filter(|(k, _)| !BLACK_LIST.contains(&k.as_str()))
        .find_map(|(k, v)| non_finite(v).map(|x| (k.clone(), x)));
    if let Some((key, x)) = found {
        g.state::<NonFinite>().0 = Some(format!(
            "Out of range float values are not JSON compliant: {} (custom property '{key}' of '{}' with export_extras)",
            py_repr(x),
            header.name
        ));
    }
}

/// `generate_extras(blender_element)`: the ID's custom properties, `None` without any.
pub fn generate(header: &IdHeader) -> Option<Extras> {
    let map: Map<String, J> = header
        .custom
        .iter()
        .filter(|(k, _)| !BLACK_LIST.contains(&k.as_str()))
        .map(|(k, v)| (k.to_string(), to_json(v)))
        .filter(|(_, v)| !v.is_null())
        .collect();
    (!map.is_empty()).then(|| Extras::from_value(&J::Object(map)))
}

/// `extras` of any ID when `export_extras` is on.
pub fn id(g: &mut Gather, header: &IdHeader) -> Option<Extras> {
    if g.settings.props.export_extras.value {
        check_finite(g, header);
        generate(header)
    } else {
        None
    }
}

/// `extras` of an object's node (`nodes.py`, `__gather_extras`).
pub fn object(g: &mut Gather, ob: &Object) -> Option<Extras> {
    id(g, &ob.header)
}

/// `extras` of a mesh (`obj_data.py`, `__gather_extras`; `targetNames` of shape keys is M6).
pub fn mesh(g: &mut Gather, mesh: &Mesh) -> Option<Extras> {
    id(g, &mesh.header)
}

/// `extras` of a material (`material_utils.py`, `gather_extras`).
pub fn material(g: &mut Gather, mat: &Material) -> Option<Extras> {
    id(g, &mat.header)
}

/// `extras` of a scene (`gather.py`, `__gather_extras`): with a `collection` export, the
/// exported collection's custom properties instead of the scene's.
pub fn scene(g: &mut Gather, scene: &Scene) -> Option<Extras> {
    if !g.settings.props.export_extras.value {
        return None;
    }
    if g.settings.collection.is_empty() {
        check_finite(g, &scene.header);
        return generate(&scene.header);
    }
    let main = g.main;
    let col = main
        .collections
        .find_in(&g.settings.collection, None)
        .and_then(|c| main.collections.get(c))?;
    check_finite(g, &col.header);
    generate(&col.header)
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use serde_json::json;
    use smol_str::SmolStr;

    use super::*;

    #[test]
    fn every_kind_in_key_order() {
        let mut h = IdHeader::new("X");
        let mut g = IndexMap::new();
        g.insert(SmolStr::new("k"), IdProp::FloatArray(vec![1.0, 2.5]));
        for (k, v) in [
            ("s", IdProp::Str("x".into())),
            ("i", IdProp::Int(-3)),
            ("f", IdProp::Float(0.25)),
            ("b", IdProp::Bool(true)),
            ("cycles", IdProp::Int(1)),
            ("ia", IdProp::IntArray(vec![1, 2])),
            ("g", IdProp::Group(g)),
            ("l", IdProp::List(vec![IdProp::Str("a".into())])),
        ] {
            h.custom.insert(SmolStr::new(k), v);
        }
        let e = generate(&h).unwrap();
        let v: J = serde_json::from_str(e.get()).unwrap();
        assert_eq!(
            v,
            json!({"s": "x", "i": -3, "f": 0.25, "b": true, "ia": [1, 2],
                   "g": {"k": [1.0, 2.5]}, "l": ["a"]})
        );
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys[0], "s");
    }

    /// The custom properties of `tests/golden/cases/gltf/attributes/extras.py` (set by
    /// Blender-only lines there: frap's `ID[key] = value` arrives with ID properties, M8), and
    /// Blender 5.2.4's extras for them, exported with `export_extras=True` (with and without
    /// `collection='Collection'`).
    #[test]
    fn factory_scene_extras_equal_blenders() {
        use frap_scene::{Context, Preferences, startup};
        let (mut main, win) = startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let set = |h: &mut IdHeader, k: &str, v: IdProp| {
            h.custom.insert(SmolStr::new(k), v);
        };
        let cube = main.objects.find("Cube").unwrap();
        let ob = main.objects.get_mut(cube, &mut log).unwrap();
        let mut d = IndexMap::new();
        d.insert(SmolStr::new("d"), IdProp::FloatArray(vec![2.0, 3.0]));
        let mut g = IndexMap::new();
        g.insert(SmolStr::new("a"), IdProp::Int(1));
        g.insert(SmolStr::new("b"), IdProp::Str("x".into()));
        g.insert(SmolStr::new("c"), IdProp::Group(d));
        for (k, v) in [
            ("int", IdProp::Int(3)),
            ("float", IdProp::Float(0.25)),
            ("flag", IdProp::Bool(true)),
            ("text", IdProp::Str("hello".into())),
            ("ints", IdProp::IntArray(vec![1, 2, 3])),
            ("floats", IdProp::FloatArray(vec![0.5, 1.5])),
            ("group", IdProp::Group(g)),
            ("cycles", IdProp::Int(1)),
        ] {
            set(&mut ob.header, k, v);
        }
        let me = main.meshes.find("Cube").unwrap();
        set(
            &mut main.meshes.get_mut(me, &mut log).unwrap().header,
            "mesh_prop",
            IdProp::Str("m".into()),
        );
        let ma = main.materials.find("Material").unwrap();
        set(
            &mut main.materials.get_mut(ma, &mut log).unwrap().header,
            "mat_prop",
            IdProp::Int(7),
        );
        let sc = win.scene;
        set(
            &mut main.scenes.get_mut(sc, &mut log).unwrap().header,
            "scene_prop",
            IdProp::Str("scene".into()),
        );
        let co = main.collections.find("Collection").unwrap();
        set(
            &mut main.collections.get_mut(co, &mut log).unwrap().header,
            "coll_prop",
            IdProp::Float(1.5),
        );
        let prefs = Preferences::default();
        let ctx = Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let extras = |collection: &str, on: bool| {
            let mut p = crate::export::settings::default_props();
            p.export_extras.value = on;
            let s = crate::export::ExportSettings {
                collection: collection.into(),
                props: std::sync::Arc::new(p),
                ..crate::export::ExportSettings::default()
            };
            let doc = crate::export::export(&main, &ctx, &s).unwrap().document;
            let e = |x: Option<&Extras>| x.map(|e| serde_json::from_str::<J>(e.get()).unwrap());
            json!({
                "nodes": doc.nodes.iter().map(|n| e(n.extras.as_ref())).collect::<Vec<_>>(),
                "meshes": doc.meshes.iter().map(|n| e(n.extras.as_ref())).collect::<Vec<_>>(),
                "materials": doc.materials.iter().map(|n| e(n.extras.as_ref())).collect::<Vec<_>>(),
                "scenes": doc.scenes.iter().map(|n| e(n.extras.as_ref())).collect::<Vec<_>>(),
            })
        };
        let node = json!({"int": 3, "float": 0.25, "flag": true, "text": "hello",
            "ints": [1, 2, 3], "floats": [0.5, 1.5],
            "group": {"a": 1, "b": "x", "c": {"d": [2.0, 3.0]}}});
        assert_eq!(
            extras("", true),
            json!({"nodes": [node], "meshes": [{"mesh_prop": "m"}],
                   "materials": [{"mat_prop": 7}], "scenes": [{"scene_prop": "scene"}]})
        );
        assert_eq!(
            extras("Collection", true),
            json!({"nodes": [node], "meshes": [{"mesh_prop": "m"}],
                   "materials": [{"mat_prop": 7}], "scenes": [{"coll_prop": 1.5}]})
        );
        assert_eq!(
            extras("", false),
            json!({"nodes": [null], "meshes": [null], "materials": [null], "scenes": [null]})
        );
    }

    /// Import → export round trip of extras on every glTF object that carries them (scene,
    /// nodes, mesh, material, camera, `KHR_lights_punctual` light), against Blender 5.2.4:
    /// `tests/golden/fixtures/gltf/attributes/extras_roundtrip/in.gltf` imported into the
    /// factory startup and exported with `export_extras`, `export_cameras` and `export_lights`
    /// (`blender_extras.json`, `extras` of each object by name). The importer keeps the
    /// extras as ID properties (big ints and mixed lists stringified, `null` dropped); the
    /// exporter writes them back on the same objects.
    #[test]
    fn import_export_round_trip_equals_blenders() {
        use frap_scene::{ChangeLog, Context, Preferences, startup};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/fixtures/gltf/attributes/extras_roundtrip");
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        crate::import::import_file(
            &mut main,
            &win,
            &dir.join("in.gltf"),
            &crate::import::ImportSettings::default(),
            &mut log,
        )
        .unwrap();
        let prefs = Preferences::default();
        let ctx = Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.export_extras.value = true;
        p.export_cameras.value = true;
        p.export_lights.value = true;
        let s = crate::export::ExportSettings {
            props: std::sync::Arc::new(p),
            ..crate::export::ExportSettings::default()
        };
        let doc = crate::export::export(&main, &ctx, &s).unwrap().document;
        let e =
            |x: Option<&Extras>| x.map_or(J::Null, |e| serde_json::from_str::<J>(e.get()).unwrap());
        let by_name = |items: Vec<(Option<String>, J)>| -> Map<String, J> {
            items
                .into_iter()
                .map(|(n, x)| (n.unwrap_or_default(), x))
                .collect()
        };
        let lights = doc
            .extensions
            .get::<frap_gltf::ext::khr_lights_punctual::KhrLightsPunctual>()
            .unwrap()
            .unwrap_or_default()
            .lights;
        let ours = json!({
            "scenes": by_name(doc.scenes.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
            "nodes": by_name(doc.nodes.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
            "meshes": by_name(doc.meshes.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
            "materials": by_name(doc.materials.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
            "cameras": by_name(doc.cameras.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
            "lights": by_name(lights.iter().map(|x| (x.name.clone(), e(x.extras.as_ref()))).collect()),
        });
        let blender: J = serde_json::from_str(
            &std::fs::read_to_string(dir.join("blender_extras.json")).unwrap(),
        )
        .unwrap();
        let theirs: Map<String, J> = blender["extras"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let items = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| (x["name"].as_str().map(str::to_owned), x["extras"].clone()))
                    .collect();
                (k.clone(), J::Object(by_name(items)))
            })
            .collect();
        assert_eq!(ours, J::Object(theirs));
        // Off: no extras anywhere.
        let doc = crate::export::export(&main, &ctx, &crate::export::ExportSettings::default())
            .unwrap()
            .document;
        assert!(doc.cameras.iter().all(|c| c.extras.is_none()));
        assert!(doc.nodes.iter().all(|c| c.extras.is_none()));
    }

    #[test]
    fn nothing_or_only_black_listed_writes_none() {
        let mut h = IdHeader::new("X");
        assert!(generate(&h).is_none());
        h.custom
            .insert(SmolStr::new("glTF2ExportSettings"), IdProp::Int(1));
        assert!(generate(&h).is_none());
    }
}
