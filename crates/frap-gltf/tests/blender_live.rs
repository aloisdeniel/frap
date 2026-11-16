// SPDX-License-Identifier: GPL-3.0-or-later
//! Blender 5.2 as an oracle for the format layer: a scene exported by io_scene_gltf2 (mesh,
//! texture with KHR_texture_transform, KHR_materials_emissive_strength, four
//! KHR_lights_punctual lights, an animation and extras) is read, validated, and written back
//! **byte-identical** — GLB container and compact JSON, and the tab-indented `.gltf` JSON.
//! This pins the key order, the integral-float rule (`__fix_json`), Python's float `repr`
//! (exponent forms included), separators and padding.
//!
//! Runs when Blender is found (`$BLENDER_BIN`, then `blender` on PATH, then the usual install
//! locations, as `frap-golden::locate_blender`), otherwise it is skipped with a notice;
//! `FRAP_REQUIRE_BLENDER=1` turns the skip into a failure.

use std::path::{Path, PathBuf};
use std::process::Command;

use frap_gltf::accessor::uri_to_path;
use frap_gltf::ext::khr_lights_punctual::KhrLightsPunctual;
use frap_gltf::ext::khr_materials_emissive_strength::KhrMaterialsEmissiveStrength;
use frap_gltf::{WriteOpts, load_buffers, read, validate_data, validate_glb, write_glb};

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

const SCRIPT: &str = r#"
import bpy, os, sys
out = sys.argv[sys.argv.index('--') + 1]
bpy.ops.object.light_add(type='SPOT', location=(1, 2, 3))
bpy.ops.object.light_add(type='SUN')
bpy.ops.object.light_add(type='POINT')
cube = bpy.data.objects['Cube']
mat = cube.active_material
nt = mat.node_tree
bsdf = nt.nodes['Principled BSDF']
bsdf.inputs['Emission Strength'].default_value = 5.0
bsdf.inputs['Emission Color'].default_value = (1, 0.5, 0.25, 1)
img = bpy.data.images.new('tex', 4, 4)
img.pixels = [0.5] * 64
img.filepath_raw = os.path.join(out, 'tex.png')
img.file_format = 'PNG'
img.save()
tex = nt.nodes.new('ShaderNodeTexImage')
tex.image = img
mapping = nt.nodes.new('ShaderNodeMapping')
mapping.inputs['Scale'].default_value = (2, 3, 1)
mapping.inputs['Location'].default_value = (0.25, 0.5, 0)
coord = nt.nodes.new('ShaderNodeTexCoord')
nt.links.new(coord.outputs['UV'], mapping.inputs['Vector'])
nt.links.new(mapping.outputs['Vector'], tex.inputs['Vector'])
nt.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
cube.keyframe_insert('location', frame=1)
cube.location = (1, 2, 3)
cube.keyframe_insert('location', frame=20)
cube['custom'] = {'a': 1, 'b': [1.5, 2]}
# Tiny and huge transforms: Python's repr writes -1.9999999494757503e-05 and
# 1.0000000116860974e-07, and int(v) for integral floats at any magnitude.
bpy.ops.mesh.primitive_plane_add(location=(0, 2e-5, 3))
plane = bpy.context.object
plane.scale = (1e-7, 0.5, 2.5e16)
common = dict(export_extras=True, export_lights=True)
bpy.ops.export_scene.gltf(filepath=os.path.join(out, 'scene.glb'), **common)
bpy.ops.export_scene.gltf(filepath=os.path.join(out, 'scene.gltf'),
                          export_format='GLTF_SEPARATE', **common)
"#;

fn json_chunk(glb: &[u8]) -> &[u8] {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    &glb[20..20 + len]
}

#[test]
fn blender_exports_round_trip_byte_identical() {
    let Some(blender) = locate_blender() else {
        if std::env::var_os("FRAP_REQUIRE_BLENDER").is_some() {
            panic!("FRAP_REQUIRE_BLENDER is set but no Blender was found");
        }
        eprintln!(
            "SKIPPED blender_exports_round_trip_byte_identical: Blender 5.2 not found (set BLENDER_BIN)"
        );
        return;
    };
    let dir = std::env::temp_dir().join(format!("frap-gltf-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("export.py");
    std::fs::write(&script, SCRIPT).unwrap();
    let status = Command::new(&blender)
        .args([
            "-b",
            "--factory-startup",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(&script)
        .arg("--")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    // GLB: container and JSON chunk byte-identical after read → write.
    let glb = std::fs::read(dir.join("scene.glb")).unwrap();
    let g = read(&glb).unwrap();
    let r = validate_glb(&g);
    assert!(r.issues.is_empty(), "{r}");
    let doc = &g.document;
    assert_eq!(
        doc.extensions
            .get::<KhrLightsPunctual>()
            .unwrap()
            .unwrap()
            .lights
            .len(),
        4
    );
    let e: KhrMaterialsEmissiveStrength = doc.materials[0].extensions.get().unwrap().unwrap();
    assert_eq!(e.emissive_strength, Some(5.0));
    // The scene exercises Python's exponent form.
    let json = String::from_utf8_lossy(json_chunk(&glb)).into_owned();
    assert!(json.contains("e-05") && json.contains("e-07"), "{json}");
    let ours = write_glb(&g, &WriteOpts::default());
    if ours != glb {
        panic!(
            "GLB differs\nblender: {}\nfrap:    {}",
            String::from_utf8_lossy(json_chunk(&glb)),
            String::from_utf8_lossy(json_chunk(&ours))
        );
    }

    // .gltf: tab-indented JSON identical (Blender appends a newline after the JSON).
    let path = dir.join("scene.gltf");
    let text = std::fs::read_to_string(&path).unwrap();
    let g = read(text.as_bytes()).unwrap();
    let bufs = load_buffers(&g.document, None, |u| {
        std::fs::read(Path::new(&dir).join(uri_to_path(u))).map_err(|e| e.to_string())
    })
    .unwrap();
    let r = validate_data(&g.document, &bufs);
    assert!(r.issues.is_empty(), "{r}");
    let ours = String::from_utf8(g.document.to_vec(&WriteOpts {
        indent: Some("\t".into()),
    }))
    .unwrap();
    assert_eq!(ours + "\n", text);
    let _ = std::fs::remove_dir_all(&dir);
}
