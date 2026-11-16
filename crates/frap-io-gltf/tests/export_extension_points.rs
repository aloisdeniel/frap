// SPDX-License-Identifier: GPL-3.0-or-later
//! The three extension-point slices of §17.2 drive the export: entries registered from a file
//! the driver does not name (this test crate) are picked up without editing `primitives.rs`.
//! - a test-only attribute writer adds a BYTE_COLOR-like `COLOR_0` (normalized UNSIGNED_SHORT,
//!   the add-on's encoding) and splits vertices through its split-key column;
//! - a test-only extractor adds a POINTS primitive for loose vertices (no corners: the corner
//!   writers leave NORMAL/TEXCOORD out, as the add-on does for points);
//! - a test-only post-processor sees every primitive with the open buffer.

mod export_support;

use std::sync::atomic::{AtomicUsize, Ordering};

use export_support::{Scene, settings};
use frap_gltf::BufferBuilder;
use frap_gltf::json::{AccessorType, ComponentType, Mode};
use frap_io_gltf::export::mesh::convert::byte_color_to_ushort;
use frap_io_gltf::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PRIMITIVE_EXTRACTORS,
    PRIMITIVE_POST_PROCESS, PrimitiveAttributeWriter, PrimitiveDraft, PrimitiveExtractor,
    PrimitivePostProcess, SplitKey,
};
use frap_mesh::primitives::cube;

/// Per-face colours of the test writer (sRGB bytes, like a BYTE_COLOR face attribute).
const FACE_COLORS: [[u8; 4]; 2] = [[255, 0, 0, 255], [0, 128, 255, 128]];

fn face_of_corner(mesh: &frap_mesh::Mesh, corner: usize) -> usize {
    (0..mesh.faces_num())
        .find(|f| mesh.face(*f).contains(&corner))
        .unwrap()
}

fn color_key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), frap_io_gltf::ExportError> {
    if !cx.mesh.header.name.starts_with("Colored") {
        return Ok(());
    }
    // One word per corner: the face's colour index (faces 0..3 → 0, others → 1).
    let words = (0..k.corners)
        .map(|c| (face_of_corner(cx.mesh, c) >= 3) as u32)
        .collect();
    k.push("test_color", 1, words);
    Ok(())
}

fn color_run(
    _: &MeshExportCx,
    k: &SplitKey,
    d: &mut PrimitiveDraft,
) -> Result<(), frap_io_gltf::ExportError> {
    let Some(col) = k.column("test_color") else {
        return Ok(());
    };
    let values = d
        .corners
        .iter()
        .map(|&c| byte_color_to_ushort(FACE_COLORS[col.words[c as usize] as usize]))
        .collect();
    d.attributes.insert(
        "COLOR_0".into(),
        AttrColumn {
            values: AttrValues::U16x4(values),
            normalized: true,
        },
    );
    Ok(())
}

#[allow(unsafe_code)]
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static TEST_COLOR: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: "test_color",
    order: 900,
    key: Some(color_key),
    run: color_run,
};

fn points_run(cx: &MeshExportCx, _: &SplitKey, out: &mut Vec<PrimitiveDraft>) {
    if !cx.mesh.header.name.starts_with("Points") {
        return;
    }
    let used: std::collections::BTreeSet<i32> = cx.mesh.corner_verts().iter().copied().collect();
    let loose: Vec<u32> = (0..cx.mesh.verts_num() as u32)
        .filter(|v| !used.contains(&(*v as i32)))
        .collect();
    if loose.is_empty() {
        return;
    }
    let mut d = PrimitiveDraft::default();
    d.mode = 0;
    d.positions = loose
        .iter()
        .map(|&v| frap_io_gltf::export::mesh::convert::yup(cx.mesh.positions()[v as usize]))
        .collect();
    d.indices = (0..loose.len() as u32).collect();
    d.corners = vec![u32::MAX; loose.len()];
    d.vertices = loose;
    out.push(d);
}

#[allow(unsafe_code)]
#[linkme::distributed_slice(PRIMITIVE_EXTRACTORS)]
static TEST_POINTS: PrimitiveExtractor = PrimitiveExtractor {
    name: "test_points",
    order: 900,
    run: points_run,
};

static POST_CALLS: AtomicUsize = AtomicUsize::new(0);

fn post_run(
    cx: &MeshExportCx,
    d: &mut PrimitiveDraft,
    buf: &mut BufferBuilder,
) -> Result<(), frap_io_gltf::ExportError> {
    assert!(!d.positions.is_empty());
    let _ = buf.bytes.len();
    // Counted for the meshes of the first test only (tests run in parallel).
    if matches!(cx.mesh.header.name.as_str(), "Colored" | "Points") {
        POST_CALLS.fetch_add(1, Ordering::SeqCst);
    }
    Ok(())
}

#[allow(unsafe_code)]
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static TEST_POST: PrimitivePostProcess = PrimitivePostProcess {
    name: "test_post",
    order: 900,
    run: post_run,
};

#[test]
fn test_writer_extractor_and_post_processor_are_picked_up() {
    let mut s = Scene::empty();
    // "Colored": the factory-style cube, faces 0..3 red, faces 3..6 blue.
    s.mesh_object("Colored", cube(2.0, false, [1.0; 3]));
    // "Points": two triangles' worth of vertices plus two loose ones.
    let mut pts = frap_mesh::Mesh::new("Points");
    pts.from_pydata(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [5.0, 0.0, 0.0],
            [0.0, 0.0, 5.0],
        ],
        &[],
        &[vec![0, 1, 2]],
        true,
    );
    s.mesh_object("Points", pts);
    let out = s.export(&settings());
    let d = &out.document;
    // Post-processor: once per primitive (Colored: 1, Points: triangles + points).
    assert_eq!(POST_CALLS.load(Ordering::SeqCst), 3);

    let colored = d
        .meshes
        .iter()
        .find(|m| m.name.as_deref() == Some("Colored"))
        .unwrap();
    let p = &colored.primitives[0];
    let keys: Vec<_> = p.attributes.keys().map(String::as_str).collect();
    assert_eq!(keys, ["POSITION", "NORMAL", "COLOR_0"]);
    let acc = &d.accessors[p.attributes["COLOR_0"] as usize];
    assert_eq!(acc.component_type, ComponentType::UnsignedShort);
    assert_eq!(acc.type_, AccessorType::Vec4);
    assert_eq!(acc.normalized, Some(true));
    // Flat cube faces already split every corner: 24 vertices, with or without the colour key.
    assert_eq!(acc.count, 24);
    // Decoded through the independent reader: BYTE_COLOR (0,128,255,128) → linear USHORT.
    let g = gltf::Gltf::from_slice(&out.bytes).unwrap();
    let blob = g.blob.clone().unwrap();
    let gm = g
        .document
        .meshes()
        .find(|m| m.name() == Some("Colored"))
        .unwrap();
    let prim = gm.primitives().next().unwrap();
    let r = prim.reader(|_| Some(blob.as_slice()));
    let colors: Vec<[u16; 4]> = r.read_colors(0).unwrap().into_rgba_u16().collect();
    assert!(colors.contains(&[65535, 0, 0, 65535]));
    assert!(colors.contains(&byte_color_to_ushort([0, 128, 255, 128])));
    assert!(colors.contains(&[0, 14146, 65535, 32896]));

    let points = d
        .meshes
        .iter()
        .find(|m| m.name.as_deref() == Some("Points"))
        .unwrap();
    assert_eq!(points.primitives.len(), 2);
    let pp = &points.primitives[1];
    assert_eq!(pp.mode, Some(Mode::Points));
    // Corner attributes are not written for vertices without corners.
    assert_eq!(pp.attributes.keys().collect::<Vec<_>>(), ["POSITION"]);
    assert_eq!(d.accessors[pp.attributes["POSITION"] as usize].count, 2);
}

#[test]
fn colour_key_splits_shared_vertices() {
    // Smooth cube: 8 shared vertices; the colour key splits the ones on both colour regions.
    let mut s = Scene::empty();
    let mut m = cube(2.0, false, [1.0; 3]);
    m.shade_smooth();
    s.mesh_object("Colored smooth", m);
    let out = s.export(&settings());
    let d = &out.document;
    let p = &d.meshes[0].primitives[0];
    let n = d.accessors[p.attributes["POSITION"] as usize].count;
    assert!(n > 8 && n < 24, "{n} vertices");
}
