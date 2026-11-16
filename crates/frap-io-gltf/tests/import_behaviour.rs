// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.17: importer behaviour that the Blender dumps do not pin: settings defaults, refused
//! files, the static part of files using later-milestone features, importing into a
//! non-empty scene, and every vendored Khronos sample importing without error.

use std::path::{Path, PathBuf};

use frap_io_gltf::import::{ImportError, ImportSettings, ImportShading, import_bytes, import_file};
use frap_rna::ReportType;
use frap_scene::{ChangeLog, startup};

fn samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/gltf-blender/fixtures")
        .join(format!("{name}.glb"))
}

#[test]
fn settings_default_to_the_operator_spec() {
    use frap_spec_ops_io::import_scene::GLTF_PROPS;
    let d = ImportSettings::default();
    let mut s = ImportSettings::default();
    for p in GLTF_PROPS.iter() {
        let v = match &p.kind {
            frap_rna::PropKind::Bool {
                default: frap_rna::Defaults::Scalar(b),
            } => serde_json::json!(*b),
            frap_rna::PropKind::Enum {
                default: frap_rna::EnumDefault::Item(i),
                ..
            } => serde_json::json!(*i),
            frap_rna::PropKind::Int {
                default: frap_rna::Defaults::Scalar(i),
                ..
            } => serde_json::json!(*i),
            frap_rna::PropKind::Float {
                default: frap_rna::Defaults::Scalar(f),
                ..
            } => serde_json::json!(*f),
            _ => continue,
        };
        assert!(s.set(p.identifier, &v), "{} not settable", p.identifier);
    }
    assert_eq!(
        s, d,
        "ImportSettings::default() differs from the dumped defaults"
    );
    assert_eq!(d.import_shading, ImportShading::Normals);
}

fn import_json(json: &str) -> Result<frap_io_gltf::import::ImportReport, ImportError> {
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    import_bytes(
        &mut main,
        &win,
        json.as_bytes(),
        Path::new("mem.gltf"),
        &ImportSettings::default(),
        &mut log,
    )
}

#[test]
fn refuses_what_blender_refuses() {
    let e = import_json(r#"{"asset":{"version":"1.0"}}"#).unwrap_err();
    assert_eq!(e.to_string(), "glTF version must be 2.0; got 1.0");
    let e = import_json(
        r#"{"asset":{"version":"2.0"},"extensionsUsed":["EXT_foo"],"extensionsRequired":["EXT_foo"]}"#,
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Extension EXT_foo is not available on this addon version"
    );
    let e = import_json(
        r#"{"asset":{"version":"2.0"},"extensionsRequired":["KHR_texture_transform"]}"#,
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Extension required must be in Extension Used too"
    );
    let e =
        import_json(r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"nope.bin","byteLength":4}]}"#)
            .unwrap_err();
    assert_eq!(e.to_string(), "Missing resource, 'nope.bin'.");
    assert!(matches!(
        import_json("not json").unwrap_err(),
        ImportError::Parse(_)
    ));
    // An empty document imports nothing.
    let r = import_json(r#"{"asset":{"version":"2.0"}}"#).unwrap();
    assert!(r.objects.is_empty());
}

#[test]
fn later_milestone_features_import_their_static_part_with_warnings() {
    for (rel, expect) in [
        ("RiggedSimple/glTF-Binary/RiggedSimple.glb", "skins"),
        ("SimpleSkin/glTF-Embedded/SimpleSkin.gltf", "skins"),
        (
            "AnimatedMorphCube/glTF-Binary/AnimatedMorphCube.glb",
            "morph targets",
        ),
        (
            "AnimatedMorphCube/glTF-Quantized/AnimatedMorphCube.gltf",
            "morph targets",
        ),
        (
            "InterpolationTest/glTF-Binary/InterpolationTest.glb",
            "animations",
        ),
        (
            "SimpleInstancing/glTF-Binary/SimpleInstancing.glb",
            "EXT_mesh_gpu_instancing",
        ),
    ] {
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
        assert!(!r.objects.is_empty(), "{rel}");
        assert!(
            r.reports
                .iter()
                .any(|m| m.level == ReportType::Warning && m.message.contains(expect)),
            "{rel}: {:?}",
            r.reports
        );
    }
}

#[test]
fn every_khronos_sample_imports() {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "gltf" || e == "glb") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&samples(), &mut files);
    assert!(files.len() >= 20);
    for p in files {
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let r = match import_file(&mut main, &win, &p, &ImportSettings::default(), &mut log) {
            Ok(r) => r,
            // LightVisibility requires KHR_node_visibility, which Blender refuses too.
            Err(ImportError::UnsupportedExtension(e)) if e == "KHR_node_visibility" => continue,
            Err(e) => panic!("{}: {e}", p.display()),
        };
        // Every created object is linked somewhere and every mesh is consistent.
        for ob in &r.objects {
            assert!(main.objects.get(*ob).is_some());
        }
        for (_, me) in main.meshes.iter() {
            let n = me.verts_num() as i32;
            assert!(me.corner_verts().iter().all(|&v| v < n), "{}", p.display());
            assert!(
                me.edges().iter().flatten().all(|&v| v < n),
                "{}",
                p.display()
            );
            assert_eq!(me.corner_normals().len(), me.corners_num());
        }
    }
}

#[test]
fn import_into_the_factory_scene_links_into_the_active_collection() {
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    let r = import_file(
        &mut main,
        &win,
        &fixture("cube"),
        &ImportSettings::default(),
        &mut log,
    )
    .unwrap();
    // The factory "Cube" exists: the imported object, mesh and material are uniquified.
    let ob = r.objects[0];
    assert_eq!(main.objects.name_of(ob), Some("Cube.001"));
    let o = main.objects.get(ob).unwrap();
    let me = main.meshes.get(o.data.unwrap().typed().unwrap()).unwrap();
    assert_eq!(me.header.name, "Cube.001");
    assert_eq!(
        me.materials[0].and_then(|m| main.materials.name_of(m)),
        Some("Material.001")
    );
    // Linked into the active collection ("Collection"), selected and active alone.
    let coll = main.collections.find("Collection").unwrap();
    assert!(main.collections.get(coll).unwrap().objects.contains(&ob));
    let scene = main.scenes.get(win.scene).unwrap();
    let vl = &scene.view_layers[0];
    assert_eq!(vl.active, Some(ob));
    let selected: Vec<_> = vl
        .bases
        .iter()
        .filter(|b| b.selected())
        .map(|b| b.object)
        .collect();
    assert_eq!(selected, vec![ob]);
    // The factory objects keep their bases.
    assert_eq!(vl.bases.len(), 4);
    assert!(log.added.contains(&ob.any()));
}

#[test]
fn shading_options_drive_sharp_faces() {
    for (shading, all_sharp) in [
        (ImportShading::Flat, Some(true)),
        (ImportShading::Smooth, Some(false)),
    ] {
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let s = ImportSettings {
            import_shading: shading,
            ..ImportSettings::default()
        };
        let r = import_file(&mut main, &win, &fixture("sphere_smooth"), &s, &mut log).unwrap();
        let o = main.objects.get(r.objects[0]).unwrap();
        let me = main.meshes.get(o.data.unwrap().typed().unwrap()).unwrap();
        let sharp = me.sharp_faces().unwrap();
        assert_eq!(sharp.iter().all(|b| *b), all_sharp == Some(true));
        assert_eq!(sharp.iter().any(|b| *b), all_sharp == Some(true));
        // Custom normals only with NORMALS.
        assert!(me.attributes().get("custom_normal").is_none());
    }
}

#[test]
fn unit_scale_is_divided_out() {
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    main.scenes
        .get_mut(win.scene, &mut log)
        .unwrap()
        .unit_settings
        .scale_length = 2.0;
    let r = import_file(
        &mut main,
        &win,
        &fixture("hierarchy"),
        &ImportSettings::default(),
        &mut log,
    )
    .unwrap();
    let root = r
        .objects
        .iter()
        .find(|o| main.objects.name_of(**o) == Some("Root"))
        .unwrap();
    // Root is at (1, 2, 3) in the fixture; with 2 m per Blender unit it lands at half that.
    assert_eq!(main.objects.get(*root).unwrap().location, [0.5, 1.0, 1.5]);
    let cone = main.meshes.find("Cone").unwrap();
    let z_max = main
        .meshes
        .get(cone)
        .unwrap()
        .positions()
        .iter()
        .map(|p| p[2])
        .fold(f32::MIN, f32::max);
    assert_eq!(z_max, 0.5);
}

#[test]
fn points_stay_loose_vertices_until_point_clouds_land() {
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    let s = ImportSettings {
        import_point_as_pointcloud: true,
        ..ImportSettings::default()
    };
    let r = import_file(
        &mut main,
        &win,
        &samples().join("MeshPrimitiveModes/glTF-Embedded/MeshPrimitiveModes.gltf"),
        &s,
        &mut log,
    )
    .unwrap();
    assert!(r.reports.iter().any(|m| m.message.contains("point clouds")));
    let me = main
        .meshes
        .get(main.meshes.find("mesh with POINTS").unwrap())
        .unwrap();
    assert_eq!((me.verts_num(), me.edges_num(), me.faces_num()), (7, 0, 0));
}
