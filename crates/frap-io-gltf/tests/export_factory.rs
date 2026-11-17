// SPDX-License-Identifier: GPL-3.0-or-later
//! The exported GLB is valid for the independent `gltf =1.4.1` reader (with validation) and
//! matches the structure Blender 5.2.4 writes for the same scene (verified by hand against
//! `bpy.ops.export_scene.gltf` on the factory scene; the live import test is in frap-cli).

use frap_io_gltf::{ExportSettings, export};
use frap_scene::{Context, startup};

#[test]
fn factory_scene_exports_valid_glb() {
    let (main, win) = startup::factory();
    let prefs = frap_scene::Preferences::default();
    let ctx = Context {
        main: &main,
        win: &win,
        prefs: &prefs,
    };
    let out = export(&main, &ctx, &ExportSettings::default()).unwrap();
    // Validating parse (gltf::Gltf::from_slice runs the crate's validation).
    let g = gltf::Gltf::from_slice(&out.bytes).expect("gltf 1.4.1 validation");
    let doc = &g.document;
    assert_eq!(
        doc.nodes().count(),
        1,
        "cameras and lights are not exported by default"
    );
    let node = doc.nodes().next().unwrap();
    assert_eq!(node.name(), Some("Cube"));
    let mesh = node.mesh().unwrap();
    assert_eq!(mesh.name(), Some("Cube"));
    let prim = mesh.primitives().next().unwrap();
    assert_eq!(prim.get(&gltf::Semantic::Positions).unwrap().count(), 24);
    assert_eq!(prim.indices().unwrap().count(), 36);
    assert_eq!(
        prim.indices().unwrap().data_type(),
        gltf::accessor::DataType::U16
    );
    let mat = prim.material();
    assert_eq!(mat.name(), Some("Material"));
    assert!(mat.double_sided());
    assert_eq!(mat.pbr_metallic_roughness().roughness_factor(), 0.5);
    assert_eq!(mat.pbr_metallic_roughness().metallic_factor(), 0.0);
    // Blender writes 840 BIN bytes for the default cube (research doc §1).
    let blob = g.blob.as_ref().unwrap();
    assert_eq!(blob.len(), 840);
    // Positions read back: the factory Cube vertex 0 (1, 1, 1) comes first (Y-up: 1, 1, -1), as in Blender.
    let reader = prim.reader(|_| Some(blob.as_slice()));
    let pos: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
    assert_eq!(pos[0], [1.0, 1.0, -1.0]);
    let idx: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();
    // Blender 5.2.4's index buffer for the factory cube (see crate docs / probe).
    assert_eq!(
        idx,
        [
            1, 13, 19, 1, 19, 7, 9, 6, 18, 9, 18, 21, 23, 20, 14, 23, 14, 17, 16, 4, 10, 16, 10,
            22, 5, 2, 8, 5, 8, 11, 15, 12, 0, 15, 0, 3
        ]
    );
}
