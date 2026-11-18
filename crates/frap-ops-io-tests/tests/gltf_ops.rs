// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_scene.gltf` / `import_scene.gltf` through a session (WP 1.23, §17.2): every one of
//! Blender's props is accepted, unsupported non-default values give a WARNING naming the prop,
//! the add-on's file-name rules, GLTF_SEPARATE side files, import errors.

mod support;

use frap_rna::{ArrayShape, Defaults, EnumDefault, PropDef, PropKind};
use frap_spec_ops_io::export_scene::OPSPEC_EXPORT_SCENE_OT_GLTF;
use frap_spec_ops_io::import_scene::OPSPEC_IMPORT_SCENE_OT_GLTF;
use serde_json::json;
use support::*;

/// A Python literal for a value of `p` that differs from its default and is valid, or `None`
/// for props without one (collections, read-only bookkeeping).
fn non_default(p: &PropDef) -> Option<String> {
    if p.array != ArrayShape::Scalar {
        return None;
    }
    Some(match &p.kind {
        PropKind::Bool { default } => match default {
            Defaults::Scalar(b) => if *b { "False" } else { "True" }.to_owned(),
            Defaults::Array(_) => return None,
        },
        PropKind::Int { default, hard, .. } => {
            let d = match default {
                Defaults::Scalar(d) => *d,
                Defaults::Array(_) => return None,
            };
            let v = if d < hard.1 { d + 1 } else { d - 1 };
            v.to_string()
        }
        PropKind::Float { default, hard, .. } => {
            let d = match default {
                Defaults::Scalar(d) => *d,
                Defaults::Array(_) => return None,
            };
            let v = if d + 0.5 <= hard.1 { d + 0.5 } else { d - 0.5 };
            format!("{v:?}")
        }
        PropKind::String { .. } => "'x'".to_owned(),
        PropKind::Enum {
            items,
            default,
            dynamic,
        } => {
            if dynamic.is_some() {
                return None;
            }
            let d = match default {
                EnumDefault::Item(d) => *d,
                _ => return None,
            };
            let other = items.iter().find(|i| i.identifier != d)?;
            format!("'{}'", other.identifier)
        }
        _ => return None,
    })
}

#[test]
fn every_export_prop_is_accepted_and_unsupported_ones_warn() {
    let d = Scratch::new("export-props");
    let mut s = session_in(d.path(), false);
    let spec = &OPSPEC_EXPORT_SCENE_OT_GLTF;
    assert!(spec.props.len() >= 110, "{} props", spec.props.len());
    let mut tried = 0;
    for p in spec.props {
        // Props that name the output or change what is written in ways this test does not
        // control (they are covered below or by the goldens).
        if matches!(
            p.identifier,
            "filepath" | "export_format" | "collection" | "export_texture_dir"
        ) {
            continue;
        }
        let Some(v) = non_default(p) else { continue };
        tried += 1;
        let stmt = format!(
            "bpy.ops.export_scene.gltf(filepath='p.glb', {}={v})",
            p.identifier
        );
        let r = run(&mut s, &stmt);
        assert!(r.ok(), "{stmt}: {}", r.to_json());
        let warned = r.reports.iter().any(|x| {
            x.message.contains(&format!("'{}'", p.identifier))
                && x.message.contains("not supported")
        });
        let supported = frap_io_gltf::export::is_supported(p.identifier);
        assert_eq!(warned, !supported, "{stmt}: {:?}", r.reports);
    }
    assert!(tried >= 100, "only {tried} props tried");
    // Default values never warn.
    let r = run(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='p.glb', export_tangents=False, export_lights=False)",
    );
    assert!(r.reports.is_empty(), "{:?}", r.reports);
}

#[test]
fn every_import_prop_is_accepted() {
    let d = Scratch::new("import-props");
    let mut s = session_in(d.path(), false);
    ok(&mut s, "bpy.ops.export_scene.gltf(filepath='src.glb')");
    let spec = &OPSPEC_IMPORT_SCENE_OT_GLTF;
    assert_eq!(spec.props.len(), 20);
    for p in spec.props {
        if matches!(
            p.identifier,
            "filepath" | "directory" | "files" | "filter_glob"
        ) {
            continue;
        }
        let Some(v) = non_default(p) else { continue };
        let stmt = format!(
            "bpy.ops.import_scene.gltf(filepath='src.glb', {}={v})",
            p.identifier
        );
        let r = run(&mut s, &stmt);
        assert!(r.ok(), "{stmt}: {}", r.to_json());
    }
}

#[test]
fn export_file_names_follow_the_addon() {
    let d = Scratch::new("export-names");
    let mut s = session_in(d.path(), false);
    ok(&mut s, "bpy.ops.export_scene.gltf(filepath='a')");
    assert!(d.path().join("a.glb").is_file());
    ok(&mut s, "bpy.ops.export_scene.gltf(filepath='b.gltf')");
    assert!(d.path().join("b.glb").is_file());
    let r = ok(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='sep/c.glb', export_format='GLTF_SEPARATE')",
    );
    assert_eq!(
        r[0].result.as_ref().unwrap()["filepath"],
        json!("sep/c.gltf")
    );
    assert!(d.path().join("sep/c.gltf").is_file());
    assert!(d.path().join("sep/c.bin").is_file());
    let gltf: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("sep/c.gltf")).unwrap()).unwrap();
    assert_eq!(gltf["buffers"][0]["uri"], json!("c.bin"));
    // GLTF_EMBEDDED needs the add-on preference (off): Blender's enum error.
    let (code, msg) = err_code(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='e.gltf', export_format='GLTF_EMBEDDED')",
    );
    assert_eq!(code, "OP_ARG_ENUM");
    assert!(
        msg.contains("enum \"GLTF_EMBEDDED\" not found in ('GLB', 'GLTF_SEPARATE')"),
        "{msg}"
    );
    // Nothing but the outputs is left in the directory (temporary files renamed).
    let mut names: Vec<String> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.glb", "b.glb", "sep"]);
}

#[test]
fn export_then_import_through_the_operators() {
    let d = Scratch::new("export-import");
    let mut s = session_in(d.path(), false);
    ok(
        &mut s,
        "bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0\n\
         bpy.ops.export_scene.gltf(filepath='m.glb')\n\
         bpy.ops.wm.read_homefile(use_empty=True)",
    );
    let r = ok(&mut s, "bpy.ops.import_scene.gltf(filepath='m.glb')");
    assert_eq!(r[0].result.as_ref().unwrap()["objects"], json!(["Cube"]));
    assert_eq!(
        get(&mut s, "bpy.data.objects['Cube'].data.materials[0].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value").as_f64(),
        Some(1.0)
    );
    assert_eq!(get(&mut s, "bpy.context.active_object.name"), json!("Cube"));
    // files + directory imports every listed file.
    let dir = d.path().to_string_lossy().into_owned();
    let r = ok(
        &mut s,
        &format!(
            "bpy.ops.import_scene.gltf(directory='{dir}', files=[{{'name': 'm.glb'}}, {{'name': 'm.glb'}}])"
        ),
    );
    assert_eq!(
        r[0].result.as_ref().unwrap()["objects"],
        json!(["Cube.001", "Cube.002"])
    );
}

#[test]
fn import_errors() {
    let d = Scratch::new("import-errors");
    let mut s = session_in(d.path(), false);
    let (code, msg) = err_code(&mut s, "bpy.ops.import_scene.gltf(filepath='missing.glb')");
    assert_eq!(
        (code.as_str(), msg.as_str()),
        ("IO_ERROR", "Error: Please select a file")
    );
    let (_, msg) = err_code(&mut s, "bpy.ops.import_scene.gltf()");
    assert_eq!(msg, "Error: Please select a file");
    std::fs::write(d.path().join("bad.glb"), b"not gltf").unwrap();
    let (code, _) = err_code(&mut s, "bpy.ops.import_scene.gltf(filepath='bad.glb')");
    assert_eq!(code, "OP_EXEC_ERROR");
    // A failed import changes nothing.
    assert_eq!(s.main().objects.len(), 3);
}

/// Blender-made GLBs (`tests/data/gltf-blender/fixtures`, written by Blender 5.2.4's exporter)
/// imported through the operator give Blender's import: objects (names, types, data, parents,
/// transforms, empty display sizes), meshes (names, vertex and face counts, material slots) and
/// materials, compared with Blender's own `import_scene.gltf` dumps (`dumps/<name>.json`, taken
/// after `read_factory_settings(use_empty=True)`, the same empty startup as here).
#[test]
fn blender_made_files_import_like_blender() {
    const TOL: f64 = 1e-5;
    let root = repo_root().join("tests/data/gltf-blender");
    for name in [
        "cube",
        "hierarchy",
        "multi_material",
        "sphere_smooth",
        "shared_mesh",
        "textured",
        "mixed_shading",
        "loose",
    ] {
        let dump: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("dumps/{name}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            dump["settings"],
            json!({}),
            "{name}: default import settings"
        );
        let src = root.join(format!("fixtures/{name}.glb"));
        let d = Scratch::new("blender-import");
        let mut s = session_in(d.path(), true);
        ok(
            &mut s,
            &format!(
                "bpy.ops.import_scene.gltf(filepath='{}')",
                src.to_string_lossy().replace('\\', "\\\\")
            ),
        );
        let main = s.main().clone();
        let names = |v: &serde_json::Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x["name"].as_str().unwrap().to_owned())
                .collect()
        };
        let mut frap_objects: Vec<String> = main
            .objects
            .iter()
            .map(|(_, o)| o.header.name.to_string())
            .collect();
        frap_objects.sort();
        assert_eq!(frap_objects, names(&dump["objects"]), "{name}: objects");
        let mut frap_materials: Vec<String> = main
            .materials
            .iter()
            .map(|(_, m)| m.header.name.to_string())
            .collect();
        frap_materials.sort();
        assert_eq!(
            frap_materials,
            names(&dump["materials"]),
            "{name}: materials"
        );
        let close = |what: &str, frap: &[f32], blender: &serde_json::Value| {
            let b: Vec<f64> = blender
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            assert_eq!(frap.len(), b.len(), "{name}: {what}");
            for (f, b) in frap.iter().zip(&b) {
                assert!(
                    (f64::from(*f) - b).abs() <= TOL,
                    "{name}: {what}: frap {frap:?} vs Blender {b:?}"
                );
            }
        };
        for bo in dump["objects"].as_array().unwrap() {
            let on = bo["name"].as_str().unwrap();
            let (_, o) = main
                .objects
                .iter()
                .find(|(_, o)| o.header.name == on)
                .unwrap();
            let ty = get(&mut s, &format!("bpy.data.objects['{on}'].type"));
            assert_eq!(ty, bo["type"], "{name}: {on}.type");
            let data = o
                .data
                .and_then(|a| main.header(a))
                .map(|h| h.name.to_string());
            assert_eq!(json!(data), bo["data"], "{name}: {on}.data");
            let parent = o
                .parent
                .and_then(|p| main.objects.get(p))
                .map(|p| p.header.name.to_string());
            assert_eq!(json!(parent), bo["parent"], "{name}: {on}.parent");
            assert_eq!(
                o.rotation_mode.as_str(),
                bo["rotation_mode"].as_str().unwrap(),
                "{name}: {on}.rotation_mode"
            );
            close(&format!("{on}.location"), &o.location, &bo["location"]);
            close(
                &format!("{on}.rotation_quaternion"),
                &o.rotation_quaternion,
                &bo["rotation_quaternion"],
            );
            close(&format!("{on}.scale"), &o.scale, &bo["scale"]);
            close(
                &format!("{on}.empty_display_size"),
                &[o.empty_display_size],
                &json!([bo["empty_display_size"]]),
            );
        }
        let mut frap_meshes: Vec<String> = main
            .meshes
            .iter()
            .map(|(_, m)| m.header.name.to_string())
            .collect();
        frap_meshes.sort();
        assert_eq!(frap_meshes, names(&dump["meshes"]), "{name}: meshes");
        for bm in dump["meshes"].as_array().unwrap() {
            let mn = bm["name"].as_str().unwrap();
            let (_, m) = main
                .meshes
                .iter()
                .find(|(_, m)| m.header.name == mn)
                .unwrap();
            assert_eq!(
                m.verts_num(),
                bm["positions"].as_array().unwrap().len(),
                "{name}: {mn} vertices"
            );
            assert_eq!(
                m.faces_num(),
                bm["faces"].as_array().unwrap().len(),
                "{name}: {mn} faces"
            );
            let slots: Vec<Option<String>> = m
                .materials
                .iter()
                .map(|r| {
                    r.and_then(|r| main.materials.get(r))
                        .map(|x| x.header.name.to_string())
                })
                .collect();
            assert_eq!(json!(slots), bm["materials"], "{name}: {mn} materials");
        }
    }
}

/// `files=[…]` with a missing file: frap raises Blender's message and rolls the whole statement
/// back, including the files imported before it (frap statements are atomic, §12; Blender keeps
/// them and raises at the end, see the operator's --describe notes).
#[test]
fn a_failing_file_rolls_back_the_whole_files_import() {
    let d = Scratch::new("import-files-rollback");
    let mut s = session_in(d.path(), false);
    ok(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='m.glb')\nbpy.ops.wm.read_homefile(use_empty=True)",
    );
    let dir = d.path().to_string_lossy().into_owned();
    let (code, msg) = err_code(
        &mut s,
        &format!(
            "bpy.ops.import_scene.gltf(directory='{dir}', files=[{{'name': 'm.glb'}}, {{'name': 'missing.glb'}}])"
        ),
    );
    assert_eq!(
        (code.as_str(), msg.as_str()),
        ("IO_ERROR", "Error: Please select a file")
    );
    assert_eq!(s.main().objects.len(), 0);
    assert_eq!(s.main().meshes.len(), 0);
}
