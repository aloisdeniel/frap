// SPDX-License-Identifier: GPL-3.0-or-later
//! Validator tests: a valid document built with `BufferBuilder` passes, and each targeted
//! defect produces its issue code at the right JSON pointer.

use frap_gltf::accessor::AccessorData;
use frap_gltf::json::{
    Accessor, AccessorType, Animation, AnimationPath, AnimationSampler, Buffer, BufferView,
    Channel, ChannelTarget, ComponentType, Document, Mesh, Node, Primitive, Scene, Skin, Target,
};
use frap_gltf::{
    AccessorOpts, BufferBuilder, Glb, Report, Severity, WriteOpts, read_glb, validate,
    validate_data, validate_glb, write_glb,
};

/// A valid one-triangle document with an animation, and its buffer.
fn triangle() -> (Document, Vec<u8>) {
    let mut doc = Document::default();
    let mut b = BufferBuilder::new();
    let pos = b.push_accessor(
        &AccessorData::from_elements(&[[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        &AccessorOpts::attribute(true),
    );
    let idx = b.push_indices(&[0, 1, 2]);
    let input = b.push_accessor(
        &AccessorData::from_elements(&[0.0f32, 1.0]),
        &AccessorOpts {
            min_max: true,
            ..AccessorOpts::default()
        },
    );
    let output = b.push_accessor(
        &AccessorData::from_elements(&[[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0]]),
        &AccessorOpts::default(),
    );
    let bin = b.finish(&mut doc, None);
    let mut prim = Primitive {
        indices: Some(idx),
        ..Primitive::default()
    };
    prim.attributes.insert("POSITION".into(), pos);
    doc.meshes.push(Mesh {
        primitives: vec![prim],
        ..Mesh::default()
    });
    doc.nodes.push(Node {
        mesh: Some(0),
        ..Node::default()
    });
    doc.scenes.push(Scene {
        nodes: vec![0],
        ..Scene::default()
    });
    doc.scene = Some(0);
    doc.animations.push(Animation {
        channels: vec![Channel {
            sampler: 0,
            target: ChannelTarget {
                node: Some(0),
                path: AnimationPath::Translation,
                extensions: Default::default(),
                extras: None,
            },
            extensions: Default::default(),
            extras: None,
        }],
        samplers: vec![AnimationSampler {
            input,
            interpolation: None,
            output,
            extensions: Default::default(),
            extras: None,
        }],
        ..Animation::default()
    });
    (doc, bin)
}

fn assert_issue(r: &Report, code: &str, pointer: &str) {
    assert!(
        r.issues
            .iter()
            .any(|i| i.code == code && i.pointer == pointer && i.severity == Severity::Error),
        "expected {code} at {pointer}, got:\n{r}"
    );
}

#[test]
fn builder_output_is_valid() {
    let (doc, bin) = triangle();
    let r = validate_data(&doc, &[&bin]);
    assert!(r.issues.is_empty(), "{r}");
    let glb = Glb {
        document: doc,
        bin: Some(bin),
    };
    let back = read_glb(&write_glb(&glb, &WriteOpts::default())).unwrap();
    assert!(validate_glb(&back).issues.is_empty());
    assert!(gltf::Gltf::from_slice(&write_glb(&glb, &WriteOpts::default())).is_ok());
}

#[test]
fn unresolved_references() {
    let (mut doc, _) = triangle();
    doc.scene = Some(3);
    doc.nodes[0].mesh = Some(9);
    doc.meshes[0].primitives[0].material = Some(0);
    doc.meshes[0].primitives[0]
        .attributes
        .insert("NORMAL".into(), 42);
    doc.animations[0].channels[0].sampler = 5;
    doc.buffer_views[0].buffer = 2;
    let r = validate(&doc);
    assert_issue(&r, "UNRESOLVED_REFERENCE", "/scene");
    assert_issue(&r, "UNRESOLVED_REFERENCE", "/nodes/0/mesh");
    assert_issue(
        &r,
        "UNRESOLVED_REFERENCE",
        "/meshes/0/primitives/0/material",
    );
    assert_issue(
        &r,
        "UNRESOLVED_REFERENCE",
        "/meshes/0/primitives/0/attributes/NORMAL",
    );
    assert_issue(
        &r,
        "UNRESOLVED_REFERENCE",
        "/animations/0/channels/0/sampler",
    );
    assert_issue(&r, "UNRESOLVED_REFERENCE", "/bufferViews/0/buffer");
}

#[test]
fn accessor_and_buffer_view_layout() {
    let (mut doc, _) = triangle();
    doc.accessors[0].count = 4; // past the end of its view
    doc.accessors[1].byte_offset = Some(1); // u16 indices at an odd offset
    doc.buffer_views[2].byte_stride = Some(6);
    doc.buffer_views[3].byte_length = 10_000;
    doc.accessors[2].normalized = Some(true); // FLOAT cannot be normalized
    doc.accessors[3].min = Some(vec![0.0]);
    let r = validate(&doc);
    assert_issue(&r, "ACCESSOR_TOO_LONG", "/accessors/0");
    assert_issue(&r, "ACCESSOR_OFFSET_ALIGNMENT", "/accessors/1/byteOffset");
    assert_issue(
        &r,
        "BUFFER_VIEW_INVALID_BYTE_STRIDE",
        "/bufferViews/2/byteStride",
    );
    assert_issue(&r, "BUFFER_VIEW_TOO_LONG", "/bufferViews/3");
    assert_issue(&r, "ACCESSOR_NORMALIZED_INVALID", "/accessors/2/normalized");
    assert_issue(&r, "ACCESSOR_MIN_MAX_LENGTH", "/accessors/3/min");
}

#[test]
fn mesh_rules() {
    let (mut doc, _) = triangle();
    let prim = &mut doc.meshes[0].primitives[0];
    prim.attributes.insert("TEXCOORD_1".into(), 3); // 2 elements vs 3, VEC3, gap at set 0
    prim.indices = Some(0); // POSITION accessor as indices
    doc.accessors[0].min = None;
    let r = validate(&doc);
    assert_issue(
        &r,
        "MESH_PRIMITIVE_UNEQUAL_ACCESSOR_COUNT",
        "/meshes/0/primitives/0/attributes/TEXCOORD_1",
    );
    assert_issue(
        &r,
        "MESH_PRIMITIVE_ATTRIBUTE_FORMAT",
        "/meshes/0/primitives/0/attributes/TEXCOORD_1",
    );
    assert_issue(
        &r,
        "MESH_PRIMITIVE_INDEXED_SEMANTIC_CONTINUITY",
        "/meshes/0/primitives/0/attributes",
    );
    assert_issue(
        &r,
        "MESH_PRIMITIVE_INDICES_FORMAT",
        "/meshes/0/primitives/0/indices",
    );
    assert_issue(
        &r,
        "MESH_PRIMITIVE_POSITION_ACCESSOR_WITHOUT_BOUNDS",
        "/meshes/0/primitives/0/attributes/POSITION",
    );
}

#[test]
fn quantized_attributes_need_khr_mesh_quantization() {
    let mut doc = Document::default();
    let mut b = BufferBuilder::new();
    let pos = b.push_accessor(
        &AccessorData::from_elements(&[[0i16, 0, 0], [100, 0, 0], [0, 100, 0]]),
        &AccessorOpts::attribute(true),
    );
    let bin = b.finish(&mut doc, None);
    let mut prim = Primitive::default();
    prim.attributes.insert("POSITION".into(), pos);
    doc.meshes.push(Mesh {
        primitives: vec![prim],
        ..Mesh::default()
    });
    assert_issue(
        &validate(&doc),
        "MESH_PRIMITIVE_ATTRIBUTE_FORMAT",
        "/meshes/0/primitives/0/attributes/POSITION",
    );
    doc.use_extension("KHR_mesh_quantization", true);
    let r = validate_data(&doc, &[&bin]);
    assert!(r.is_ok(), "{r}");
    assert_eq!(doc.buffer_views[0].byte_stride, Some(8));
}

#[test]
fn node_hierarchy() {
    let mut doc = Document {
        nodes: vec![
            Node {
                children: vec![1],
                ..Node::default()
            },
            Node {
                children: vec![2],
                ..Node::default()
            },
            Node {
                children: vec![0],
                ..Node::default()
            },
            Node {
                children: vec![1],
                matrix: Some(frap_gltf::json::node::IDENTITY),
                translation: Some([1.0, 0.0, 0.0]),
                rotation: Some([0.0, 0.0, 0.0, 2.0]),
                skin: Some(0),
                ..Node::default()
            },
        ],
        ..Document::default()
    };
    doc.skins.push(Skin {
        joints: vec![0],
        ..Skin::default()
    });
    doc.scenes.push(Scene {
        nodes: vec![1, 3],
        ..Scene::default()
    });
    let r = validate(&doc);
    assert_issue(&r, "NODE_LOOP", "/nodes/0");
    assert_issue(&r, "NODE_PARENT_OVERRIDE", "/nodes/3/children/0");
    assert_issue(&r, "NODE_MATRIX_TRS", "/nodes/3");
    assert_issue(&r, "ROTATION_NON_UNIT", "/nodes/3/rotation");
    assert_issue(&r, "NODE_SKIN_NO_MESH", "/nodes/3/skin");
    assert_issue(&r, "SCENE_NON_ROOT_NODE", "/scenes/0/nodes/0");
}

#[test]
fn unsupported_required_extensions() {
    let (mut doc, _) = triangle();
    for (name, req) in [
        ("KHR_texture_transform", true),
        ("EXT_unknown", true),
        ("KHR_materials_emissive_strength", false),
    ] {
        doc.use_extension(name, req);
    }
    let typed = |n: &str| frap_gltf::json::extension_def(n).is_some();
    assert_eq!(
        frap_gltf::validate::unsupported_required(&doc, typed),
        ["EXT_unknown"]
    );
    assert_eq!(
        frap_gltf::validate::unsupported_required(&doc, |n| n.starts_with("EXT_")),
        ["KHR_texture_transform"]
    );
}

#[test]
fn extension_declarations() {
    let (mut doc, _) = triangle();
    doc.extensions_required.push("EXT_unknown".into());
    doc.extensions_used.push("KHR_never_used".into());
    doc.nodes[0]
        .extensions
        .insert_raw("VENDOR_thing", serde_json::json!({"a": 1}));
    let r = validate(&doc);
    assert_issue(&r, "NOT_IN_USED_EXTENSIONS", "/extensionsRequired/0");
    assert_issue(
        &r,
        "UNDECLARED_EXTENSION",
        "/nodes/0/extensions/VENDOR_thing",
    );
    assert!(
        r.issues
            .iter()
            .any(|i| i.code == "UNUSED_EXTENSION" && i.severity == Severity::Warning),
        "{r}"
    );
}

#[test]
fn data_checks() {
    // Index past the vertices, and the restart value.
    let (doc, mut bin) = triangle();
    let at = doc.buffer_views[1].byte_offset_or_default() as usize;
    bin[at..at + 2].copy_from_slice(&7u16.to_le_bytes());
    let r = validate_data(&doc, &[&bin]);
    assert_issue(&r, "ACCESSOR_INDEX_OOB", "/meshes/0/primitives/0/indices");
    bin[at..at + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let r = validate_data(&doc, &[&bin]);
    assert_issue(
        &r,
        "ACCESSOR_INDEX_PRIMITIVE_RESTART",
        "/meshes/0/primitives/0/indices",
    );

    // min/max that do not match the data.
    let (mut doc, bin) = triangle();
    doc.accessors[0].max = Some(vec![1.0, 2.0, 0.0]);
    let r = validate_data(&doc, &[&bin]);
    assert_issue(&r, "ACCESSOR_MAX_MISMATCH", "/accessors/0/max/1");

    // Animation times not increasing.
    let (doc, mut bin) = triangle();
    let at = doc.buffer_views[2].byte_offset_or_default() as usize;
    bin[at + 4..at + 8].copy_from_slice(&0.0f32.to_le_bytes());
    let r = validate_data(&doc, &[&bin]);
    assert_issue(
        &r,
        "ACCESSOR_ANIMATION_INPUT_NON_INCREASING",
        "/animations/0/samplers/0/input",
    );
    // Its min/max now also disagree with the data.
    assert_issue(&r, "ACCESSOR_MAX_MISMATCH", "/accessors/2/max/0");

    // Animation output count.
    let (mut doc, bin) = triangle();
    doc.animations[0].samplers[0].interpolation = Some(frap_gltf::json::Interpolation::CubicSpline);
    assert_issue(
        &validate_data(&doc, &[&bin]),
        "ANIMATION_SAMPLER_OUTPUT_ACCESSOR_INVALID_COUNT",
        "/animations/0/samplers/0/output",
    );
}

#[test]
fn sparse_indices_must_increase() {
    let dense = AccessorData::from_elements(&[[0.0f32; 3], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
    let mut doc = Document::default();
    let mut b = BufferBuilder::new();
    b.push_sparse(&dense, &[1, 2], None, &AccessorOpts::default())
        .unwrap();
    let mut bin = b.finish(&mut doc, None);
    assert!(validate_data(&doc, &[&bin]).is_ok());
    let iv = doc.accessors[0]
        .sparse
        .as_ref()
        .unwrap()
        .indices
        .buffer_view as usize;
    let at = doc.buffer_views[iv].byte_offset_or_default() as usize;
    bin.swap(at, at + 1);
    assert_issue(
        &validate_data(&doc, &[&bin]),
        "ACCESSOR_SPARSE_INDICES_NON_INCREASING",
        "/accessors/0/sparse/indices",
    );
}

#[test]
fn glb_container_rules() {
    let (doc, bin) = triangle();
    // Buffer 0 has no uri but the GLB has no BIN chunk.
    let r = validate_glb(&Glb {
        document: doc.clone(),
        bin: None,
    });
    assert_issue(&r, "BUFFER_MISSING_GLB_DATA", "/buffers/0");
    // BIN chunk much longer than byteLength.
    let mut long = bin.clone();
    long.extend_from_slice(&[0; 8]);
    let r = validate_glb(&Glb {
        document: doc.clone(),
        bin: Some(long),
    });
    assert_issue(&r, "BUFFER_BYTE_LENGTH_MISMATCH", "/buffers/0/byteLength");
    // Only buffer 0 may omit its uri.
    let mut two = doc.clone();
    two.buffers.push(Buffer {
        byte_length: 4,
        ..Buffer::default()
    });
    assert_issue(&validate(&two), "BUFFER_MISSING_GLB_DATA", "/buffers/1");
}

#[test]
fn image_and_view_rules() {
    let (mut doc, _) = triangle();
    doc.images.push(frap_gltf::json::Image {
        buffer_view: Some(0),
        ..Default::default()
    });
    doc.accessors.push(Accessor::new(
        Some(0),
        ComponentType::UnsignedInt,
        AccessorType::Scalar,
        1,
    ));
    doc.buffer_views.push(BufferView {
        buffer: 0,
        byte_length: 4,
        byte_stride: Some(4),
        target: Some(Target::ElementArrayBuffer),
        ..BufferView::default()
    });
    let r = validate(&doc);
    assert_issue(&r, "UNSATISFIED_DEPENDENCY", "/images/0/mimeType");
    assert_issue(&r, "BUFFER_VIEW_TARGET_OVERRIDE", "/images/0/bufferView");
    assert_issue(
        &r,
        "BUFFER_VIEW_INVALID_BYTE_STRIDE",
        "/bufferViews/4/byteStride",
    );
}
