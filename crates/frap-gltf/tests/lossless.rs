// SPDX-License-Identifier: GPL-3.0-or-later
//! Unknown extensions and `extras` round-trip untouched through `.gltf` and GLB, wherever they
//! appear; typed extensions read and write through `Extensions::get/set` without disturbing
//! their neighbours.

use frap_gltf::ext::khr_texture_transform::KhrTextureTransform;
use frap_gltf::json::json_eq;
use frap_gltf::{Document, Glb, WriteOpts, read, read_glb, write_glb};
use serde_json::Value;

/// Unknown extensions and oddly formatted extras on the root, a node, a mesh, a primitive, a
/// material's texture reference (next to a typed extension), an accessor and an animation
/// channel target.
const DOC: &str = r#"{
  "asset": {"version": "2.0", "generator": "hand", "extras": {"k": "asset"}},
  "extensionsUsed": ["VENDOR_root", "VENDOR_node", "KHR_texture_transform", "EXT_future"],
  "extensions": {"VENDOR_root": {"list": [1, 2.5, "three", null, {"deep": [true]}], "n": 1.0}},
  "extras": {"spacing"   :  [ 1 ,2 ],
             "unicode": "café 😀", "big": 12345678901234567890, "neg": -0.0},
  "nodes": [{"name": "n", "extensions": {"VENDOR_node": {}}, "extras": 7}],
  "materials": [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0,
      "extensions": {"KHR_texture_transform": {"scale": [2, 2]}, "EXT_future": {"x": 1e-7}},
      "extras": "s"}}}],
  "textures": [{"source": 0}],
  "images": [{"uri": "a.png"}],
  "meshes": [{"primitives": [{"attributes": {"POSITION": 0},
      "extensions": {"EXT_future": {"compressed": {"bufferView": 0}}}, "extras": [null]}],
      "extras": {"targetNames": ["a", "b"]}}],
  "accessors": [{"componentType": 5126, "count": 1, "type": "VEC3", "min": [0, 0, 0],
      "max": [0, 0, 0], "extras": {}}],
  "animations": [{"channels": [{"sampler": 0, "target": {"path": "pointer",
      "extensions": {"KHR_animation_pointer": {"pointer": "/nodes/0/translation"}}}}],
      "samplers": [{"input": 0, "output": 0, "interpolation": "STEP"}]}]
}"#;

fn value(doc: &Document, opts: &WriteOpts) -> Value {
    serde_json::from_slice(&doc.to_vec(opts)).unwrap()
}

#[test]
fn unknown_extensions_and_extras_round_trip() {
    let src: Value = serde_json::from_str(DOC).unwrap();
    let doc = Document::from_slice(DOC.as_bytes()).unwrap();
    for opts in [
        WriteOpts::default(),
        WriteOpts {
            indent: Some("  ".into()),
        },
    ] {
        assert!(json_eq(&value(&doc, &opts), &src), "{}", value(&doc, &opts));
    }
    // Extras keep their exact text (formatting, big integers, escapes, -0.0).
    let extras = doc.extras.as_ref().unwrap().get();
    assert!(extras.contains(r#""spacing"   :  [ 1 ,2 ]"#), "{extras}");
    assert!(extras.contains("12345678901234567890"), "{extras}");
    assert!(extras.contains(r"café 😀"), "{extras}");
    assert!(extras.contains("-0.0"), "{extras}");
    // Through GLB: the JSON chunk carries the same text for extras.
    let glb = write_glb(
        &Glb {
            document: doc.clone(),
            bin: None,
        },
        &WriteOpts::default(),
    );
    let back = read_glb(&glb).unwrap();
    assert_eq!(back.document, doc);
    assert_eq!(back.document.extras.as_ref().unwrap().get(), extras);
    assert!(json_eq(&back.document.to_value(), &src));
    // Unknown animation path spellings survive as well.
    let other = DOC.replace(r#""path": "pointer""#, r#""path": "VENDOR_path""#);
    let d = read(other.as_bytes()).unwrap().document;
    assert!(
        String::from_utf8(d.to_vec(&WriteOpts::default()))
            .unwrap()
            .contains(r#""path":"VENDOR_path""#)
    );
}

#[test]
fn typed_access_preserves_neighbours() {
    let mut doc = Document::from_slice(DOC.as_bytes()).unwrap();
    let ti = doc.materials[0]
        .pbr_metallic_roughness
        .as_mut()
        .unwrap()
        .base_color_texture
        .as_mut()
        .unwrap();
    let mut t: KhrTextureTransform = ti.extensions.get().unwrap().unwrap();
    assert_eq!(t.scale, Some([2.0, 2.0]));
    t.rotation = Some(0.5);
    ti.extensions.set(Some(t));
    // Order and the unknown sibling are kept.
    assert_eq!(
        ti.extensions.names().collect::<Vec<_>>(),
        ["KHR_texture_transform", "EXT_future"]
    );
    assert_eq!(ti.extensions.get_raw("EXT_future").unwrap()["x"], 1e-7);
    let text = String::from_utf8(doc.to_vec(&WriteOpts::default())).unwrap();
    assert!(
        text.contains(r#""extensions":{"KHR_texture_transform":{"rotation":0.5,"scale":[2,2]},"EXT_future":{"x":1e-07}}"#),
        "{text}"
    );
}

#[test]
fn integral_floats_are_written_as_integers() {
    // io_scene_gltf2's __fix_json: `int(v)` for integral floats (exact at any magnitude);
    // other values print as Python's `repr`.
    let mut doc = Document::default();
    doc.nodes.push(frap_gltf::json::Node {
        translation: Some([1.0, -2.0, 0.800000011920929]),
        scale: Some([f64::from(0.1f32), 1e20, -0.0]),
        ..Default::default()
    });
    let text = String::from_utf8(doc.to_vec(&WriteOpts::default())).unwrap();
    assert!(
        text.contains(
            r#""scale":[0.10000000149011612,100000000000000000000,0],"translation":[1,-2,0.800000011920929]"#
        ),
        "{text}"
    );
    assert_eq!(
        Document::from_slice(text.as_bytes()).unwrap().nodes[0].translation,
        doc.nodes[0].translation
    );
}

#[test]
fn non_integral_floats_print_like_python_repr() {
    // `json.dumps` in Blender 5.2's Python writes `repr(float)`: exponent form below 1e-4 and
    // from 1e16, with a signed two-digit exponent.
    let mut doc = Document::default();
    doc.nodes.push(frap_gltf::json::Node {
        translation: Some([-1.9999999494757503e-05, 1.5e-8, 0.0001]),
        scale: Some([1e-7, 1.5e16 + 2.0, 12345.678]),
        ..Default::default()
    });
    let text = String::from_utf8(doc.to_vec(&WriteOpts::default())).unwrap();
    assert!(
        text.contains(
            r#""scale":[1e-07,15000000000000002,12345.678],"translation":[-1.9999999494757503e-05,1.5e-08,0.0001]"#
        ),
        "{text}"
    );
    let back = Document::from_slice(text.as_bytes()).unwrap();
    assert_eq!(back.nodes[0], doc.nodes[0]);
}

#[test]
fn null_extras_round_trip() {
    let src = r#"{"asset":{"version":"2.0"},"extras":null,"nodes":[{"extras":null},{}]}"#;
    let doc = Document::from_slice(src.as_bytes()).unwrap();
    assert_eq!(doc.extras.as_ref().map(|e| e.get()), Some("null"));
    assert_eq!(doc.nodes[0].extras.as_ref().map(|e| e.get()), Some("null"));
    assert!(doc.nodes[1].extras.is_none());
    let out = String::from_utf8(doc.to_vec(&WriteOpts::default())).unwrap();
    assert_eq!(
        out,
        r#"{"asset":{"version":"2.0"},"extras":null,"nodes":[{"extras":null},{}]}"#
    );
}

#[test]
fn integers_written_as_floats_still_read() {
    let src = r#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":4.0}],
        "bufferViews":[{"buffer":0.0,"byteLength":4,"byteOffset":0.0}],
        "extensions":{"VENDOR_x":{"n":2.0}}}"#;
    let doc = Document::from_slice(src.as_bytes()).unwrap();
    assert_eq!(doc.buffers[0].byte_length, 4);
    assert_eq!(doc.buffer_views[0].byte_offset, Some(0));
    // Extension values are kept as written.
    let out = String::from_utf8(doc.to_vec(&WriteOpts::default())).unwrap();
    assert!(out.contains(r#""VENDOR_x":{"n":2}"#), "{out}");
    // A fractional value in an integer property is still a parse error.
    let bad = src.replace("4.0", "4.5");
    let e = Document::from_slice(bad.as_bytes()).unwrap_err();
    assert!(e.to_string().contains("4.5"), "{e}");
}
