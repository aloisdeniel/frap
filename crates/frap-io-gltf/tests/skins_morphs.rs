// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 6.8: skins and morph targets through the public import/export API. The Blender
//! comparisons are the goldens of `tests/golden/cases/gltf/skins/`; these tests pin what a
//! golden dump does not show: no "imported from M6" WARNING any more, the armature and its
//! bones, vertex groups, the ARMATURE modifier, shape keys, and a frap export of an imported
//! rig that imports back to the same armature (the differential round trip).

use std::path::{Path, PathBuf};

use frap_io_gltf::import::{ImportSettings, import_bytes, import_file};
use frap_io_gltf::{ExportSettings, export};
use frap_rna::{ArmatureId, MeshId, ReportType};
use frap_scene::id::ModifierSettings;
use frap_scene::{ChangeLog, Context, Main, Preferences, WindowState, startup};

fn samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples")
}

fn import(rel: &str) -> (Main, WindowState, Vec<String>) {
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    let r = import_file(
        &mut main,
        &win,
        &samples().join(rel),
        &ImportSettings::default(),
        &mut log,
    )
    .unwrap_or_else(|e| panic!("{rel}: {e}"));
    let warnings = r
        .reports
        .iter()
        .filter(|m| m.level == ReportType::Warning)
        .map(|m| m.message.clone())
        .collect();
    (main, win, warnings)
}

fn bones(main: &Main, ob: &str) -> Vec<String> {
    let o = main.objects.get(main.objects.find(ob).unwrap()).unwrap();
    let a = o.data.unwrap().typed::<ArmatureId>().unwrap();
    main.armatures
        .get(a)
        .unwrap()
        .bones
        .iter()
        .map(|b| b.name.to_string())
        .collect()
}

#[test]
fn skins_import_as_armatures_with_vertex_groups() {
    let (main, _, warnings) = import("RiggedSimple/glTF-Binary/RiggedSimple.glb");
    assert!(
        !warnings.iter().any(|w| w.contains("skins")),
        "{warnings:?}"
    );
    assert_eq!(bones(&main, "Armature"), ["Bone", "Bone.001"]);
    let cyl = main
        .objects
        .get(main.objects.find("Cylinder").unwrap())
        .unwrap();
    // Parented to the armature, deformed by it, one vertex group per joint.
    assert_eq!(cyl.parent, main.objects.find("Armature"));
    let arm = main.objects.find("Armature").unwrap().any();
    assert!(cyl.modifiers.iter().any(|m| matches!(
        &m.settings,
        ModifierSettings::Armature(a) if a.object == Some(arm)
    )));
    let me = main
        .meshes
        .get(cyl.data.unwrap().typed::<MeshId>().unwrap())
        .unwrap();
    assert_eq!(me.vertex_group_names, ["Bone", "Bone.001"]);
    let dv = me.deform_verts().unwrap();
    assert!(dv.iter().all(|d| !d.weights.is_empty()));
    // The bone shape of the BLENDER heuristic lives in the hidden special collection.
    let coll = main.collections.find("glTF_not_exported").unwrap();
    let coll = main.collections.get(coll).unwrap();
    assert!(coll.hide_viewport && coll.hide_render);
    assert_eq!(coll.objects, [main.objects.find("Icosphere").unwrap()]);
}

#[test]
fn morph_targets_import_as_shape_keys() {
    let (main, _, warnings) = import("AnimatedMorphCube/glTF-Binary/AnimatedMorphCube.glb");
    assert!(
        !warnings.iter().any(|w| w.contains("morph targets")),
        "{warnings:?}"
    );
    let ob = main
        .objects
        .get(main.objects.find("AnimatedMorphCube").unwrap())
        .unwrap();
    let me = main
        .meshes
        .get(ob.data.unwrap().typed::<MeshId>().unwrap())
        .unwrap();
    let key = main.shape_keys.get(me.shape_keys.unwrap()).unwrap();
    let names: Vec<&str> = key.key_blocks.iter().map(|k| k.name.as_str()).collect();
    // No targetNames: named after the targets' POSITION accessors ("thin", "angle").
    assert_eq!(names[0], "Basis");
    assert_eq!(names.len(), 3);
    assert!(
        key.key_blocks
            .iter()
            .all(|k| k.data.len() == me.verts_num())
    );
    assert_eq!(key.key_blocks[0].value, 1.0);
    // The restored animation (`Square`, WP 6.7) at frame 1 drives the weights; Blender 5.2.4
    // reads thin 0.0022289131, angle 0.0 right after the import.
    assert!(
        (key.key_blocks[1].value - 0.002_228_913).abs() < 1e-6,
        "{}",
        key.key_blocks[1].value
    );
    assert_eq!(key.key_blocks[2].value, 0.0);
}

#[test]
fn an_exported_rig_imports_back_to_the_same_armature() {
    let (main, win, _) = import("RiggedSimple/glTF-Binary/RiggedSimple.glb");
    let prefs = Preferences::default();
    let ctx = Context {
        main: &main,
        win: &win,
        prefs: &prefs,
    };
    let settings = ExportSettings {
        generator: "frap test".into(),
        ..ExportSettings::default()
    };
    let out = export(&main, &ctx, &settings).unwrap();
    let doc = &out.document;
    assert_eq!(doc.skins.len(), 1);
    let skin = &doc.skins[0];
    let joints: Vec<&str> = skin
        .joints
        .iter()
        .map(|&j| doc.nodes[j as usize].name.as_deref().unwrap())
        .collect();
    assert_eq!(joints, ["Bone", "Bone.001"]);
    let ibm = doc.accessors[skin.inverse_bind_matrices.unwrap() as usize].count;
    assert_eq!(ibm, 2);
    let skinned = doc.nodes.iter().find(|n| n.skin.is_some()).unwrap();
    assert_eq!(skinned.name.as_deref(), Some("Cylinder"));
    let prim = &doc.meshes[skinned.mesh.unwrap() as usize].primitives[0];
    assert!(prim.attributes.contains_key("JOINTS_0") && prim.attributes.contains_key("WEIGHTS_0"));

    let (mut main2, win2) = startup::factory();
    let mut log = ChangeLog::default();
    import_bytes(
        &mut main2,
        &win2,
        &out.bytes,
        Path::new("rt.glb"),
        &ImportSettings::default(),
        &mut log,
    )
    .unwrap();
    assert_eq!(bones(&main2, "Armature"), ["Bone", "Bone.001"]);
    let a = |m: &Main| {
        let o = m.objects.get(m.objects.find("Armature").unwrap()).unwrap();
        let arm = m
            .armatures
            .get(o.data.unwrap().typed::<ArmatureId>().unwrap())
            .unwrap();
        arm.bones
            .iter()
            .map(|b| (b.head_local, b.tail_local))
            .collect::<Vec<_>>()
    };
    for ((h1, t1), (h2, t2)) in a(&main).iter().zip(a(&main2)) {
        for k in 0..3 {
            assert!((h1[k] - h2[k]).abs() < 1e-4 && (t1[k] - t2[k]).abs() < 1e-4);
        }
    }
}
