// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 7.4 acceptance tests of the spec-complete writer: each option on the factory scene (and
//! small variations), against the default export: the decoded content is unchanged, the
//! layout or the declarations change as documented, and every output passes frap's
//! validator and, when `FRAP_GLTF_VALIDATOR` names a directory holding
//! `node_modules/gltf-validator` (`npm install gltf-validator@2.0.0-dev.3.10`), the Khronos
//! glTF-Validator with no error and no warning (skipped visibly otherwise). The Blender side
//! (Blender imports every file these options write) is the `tests/golden/cases/gltf/spec/`
//! goldens, whose input files [`golden_inputs_are_current`] keeps in sync.

use std::path::{Path, PathBuf};

use frap_anim::{
    Action, ActionLayer, ActionStrip, AnimData, Channelbag, FCurve, Interpolation, Keyframe,
    KeyframeStrip,
};
use frap_gltf::accessor::read_accessor;
use frap_gltf::json::{ComponentType, Document, Index, Target};
use frap_nodes::SocketValue;
use frap_rna::{IdRef, ObjectId, OpArgs, OpProps, Value};
use frap_scene::{ChangeLog, Context, Main, Preferences, WindowState, startup};
use frap_spec_ops_io::export_scene::ExportSceneGltf;

use crate::export::{ExportOutput, ExportSettings, export};

struct Scene {
    main: Main,
    win: WindowState,
    prefs: Preferences,
    log: ChangeLog,
}

impl Scene {
    fn factory() -> Scene {
        let (main, win) = startup::factory();
        Scene {
            main,
            win,
            prefs: Preferences::default(),
            log: ChangeLog::default(),
        }
    }

    fn object(&mut self, name: &str) -> IdRef<ObjectId> {
        self.main.objects.find(name).unwrap()
    }

    fn export(&self, props: &[(&'static str, Value)]) -> ExportOutput {
        let ctx = Context {
            main: &self.main,
            win: &self.win,
            prefs: &self.prefs,
        };
        export(&self.main, &ctx, &settings(props)).unwrap()
    }

    /// Keys `location[2]` of `ob` at frames 0 and 30.
    fn animate(&mut self, ob: IdRef<ObjectId>, ipo: Interpolation) {
        let mut fc = FCurve::new("location", 2);
        for (f, v) in [(0.0, 0.0), (30.0, 1.0), (45.0, 3.0)] {
            let mut k = Keyframe::new(f, v);
            k.interpolation = ipo;
            fc.keyframe_points.push(k);
        }
        frap_anim::eval::recalc_handles(&mut fc);
        let mut a = Action::new("Move");
        let h = a.slot_add_for_id_type("OBJECT", "Cube");
        let mut layer = ActionLayer::new("Layer");
        layer.strips.push(ActionStrip::Keyframe(KeyframeStrip {
            channelbags: vec![Channelbag {
                slot_handle: h,
                groups: Vec::new(),
                fcurves: vec![fc],
            }],
        }));
        a.layers.push(layer);
        let a = self.main.actions.insert("Move", a, &mut self.log);
        let o = self.main.objects.get_mut(ob, &mut self.log).unwrap();
        let adt = o.animation_data.get_or_insert_with(AnimData::default);
        adt.action = Some(a);
        adt.action_slot_handle = h;
    }
}

fn settings(props: &[(&'static str, Value)]) -> ExportSettings {
    let mut a = OpArgs::default();
    for def in ExportSceneGltf::spec().props {
        a.values.insert(def.identifier, Value::default_for(def));
    }
    for (k, v) in props {
        let i = a.values.get_index_of(k).unwrap();
        a.values.insert(k, v.clone());
        a.is_set.insert(i);
    }
    ExportSettings::from_props(&ExportSceneGltf::from_args(&a))
}

fn spec() -> (&'static str, Value) {
    ("frap_compat", Value::Str("SPEC".into()))
}

fn on(k: &'static str) -> (&'static str, Value) {
    (k, Value::Bool(true))
}

fn parse(out: &ExportOutput) -> (Document, Vec<Vec<u8>>) {
    let glb = frap_gltf::read(&out.bytes).unwrap();
    (glb.document, vec![glb.bin.unwrap_or_default()])
}

/// Every attribute of every primitive, decoded, by (mesh, primitive, semantic).
fn attributes(out: &ExportOutput) -> Vec<(String, Vec<f32>)> {
    let (doc, bufs) = parse(out);
    let mut v = Vec::new();
    for (m, mesh) in doc.meshes.iter().enumerate() {
        for (p, prim) in mesh.primitives.iter().enumerate() {
            for (sem, &a) in &prim.attributes {
                let d = read_accessor(&doc, &bufs, a).unwrap();
                v.push((format!("{m}/{p}/{sem}"), d.to_f32()));
            }
            if let Some(i) = prim.indices {
                let d = read_accessor(&doc, &bufs, i).unwrap();
                v.push((
                    format!("{m}/{p}/indices"),
                    d.to_u32().unwrap().into_iter().map(|i| i as f32).collect(),
                ));
            }
        }
    }
    v
}

/// frap's validator (data included) finds nothing, and the Khronos one when available.
fn valid(out: &ExportOutput, what: &str) {
    let r = frap_gltf::validate_file(&out.bytes).unwrap();
    assert!(r.issues.is_empty(), "{what}: {r}");
    khronos(&out.bytes, what);
}

/// The Khronos glTF-Validator's report: no error, no warning (infos allowed: the validator
/// does not know `KHR_node_visibility` and lists unused objects).
fn khronos(glb: &[u8], what: &str) {
    let Some(dir) = std::env::var_os("FRAP_GLTF_VALIDATOR") else {
        eprintln!(
            "skipped: Khronos glTF-Validator ({what}): set FRAP_GLTF_VALIDATOR to a directory with node_modules/gltf-validator"
        );
        return;
    };
    let dir = PathBuf::from(dir);
    let tmp = std::env::temp_dir().join(format!(
        "frap-spec-{}-{}",
        std::process::id(),
        what.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    std::fs::write(&tmp, glb).unwrap();
    let script = r#"
const v = require('gltf-validator');
const fs = require('fs');
v.validateBytes(new Uint8Array(fs.readFileSync(process.argv[1])), {maxIssues: 0})
  .then(r => { console.log(JSON.stringify(r.issues)); })
  .catch(e => { console.error(e); process.exit(2); });
"#;
    let out = std::process::Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&tmp)
        .env("NODE_PATH", dir.join("node_modules"))
        .output()
        .expect("node runs");
    let _ = std::fs::remove_file(&tmp);
    assert!(
        out.status.success(),
        "{what}: validator failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let issues: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        (issues["numErrors"].as_u64(), issues["numWarnings"].as_u64()),
        (Some(0), Some(0)),
        "{what}: {issues:#}"
    );
}

#[test]
fn defaults_write_blenders_layout() {
    let s = Scene::factory();
    let out = s.export(&[]);
    let (doc, _) = parse(&out);
    // One view per accessor, UNSIGNED_SHORT indices (the add-on's layout).
    assert_eq!(doc.buffer_views.len(), doc.accessors.len());
    assert!(doc.buffer_views.iter().all(|v| v.byte_stride.is_none()));
    let ix = doc.meshes[0].primitives[0].indices.unwrap();
    assert_eq!(
        doc.accessors[ix as usize].component_type,
        ComponentType::UnsignedShort
    );
    // The props are honoured: the operator does not warn about them.
    for p in [
        "frap_compat",
        "frap_interleave",
        "frap_node_visibility",
        "frap_diffuse_transmission",
    ] {
        assert!(crate::export::is_supported(p), "{p}");
    }
    valid(&out, "defaults");
}

#[test]
fn spec_writes_byte_indices_and_optional_extensions() {
    let s = Scene::factory();
    let lights = ("export_lights", Value::Bool(true));
    let blender = s.export(std::slice::from_ref(&lights));
    let out = s.export(&[lights, spec()]);
    let (doc, _) = parse(&out);
    // KHR_lights_punctual: required by the add-on, optional under SPEC.
    assert_eq!(
        blender.document.extensions_required,
        ["KHR_lights_punctual"]
    );
    assert!(doc.extensions_required.is_empty());
    assert_eq!(doc.extensions_used, ["KHR_lights_punctual"]);
    // The cube's 36 indices into 24 vertices fit UNSIGNED_BYTE.
    let ix = doc.meshes[0].primitives[0].indices.unwrap();
    let acc = &doc.accessors[ix as usize];
    assert_eq!(acc.component_type, ComponentType::UnsignedByte);
    let view = &doc.buffer_views[acc.buffer_view.unwrap() as usize];
    assert_eq!(
        (view.byte_length, view.target),
        (36, Some(Target::ElementArrayBuffer))
    );
    assert_eq!(attributes(&out), attributes(&blender));
    valid(&out, "spec");
}

#[test]
fn spec_keeps_wide_indices() {
    let mut s = Scene::factory();
    // A 64×64 grid: 4225 vertices, UNSIGNED_SHORT under SPEC too.
    let cube = s.object("Cube");
    let mesh = frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
        x_subdivisions: 64,
        y_subdivisions: 64,
        calc_uvs: true,
        ..Default::default()
    });
    let me = s.main.meshes.insert("Grid", mesh, &mut s.log);
    s.main.objects.get_mut(cube, &mut s.log).unwrap().data = Some(me.any());
    let out = s.export(&[spec()]);
    let (doc, _) = parse(&out);
    let ix = doc.meshes[0].primitives[0].indices.unwrap();
    assert_eq!(
        doc.accessors[ix as usize].component_type,
        ComponentType::UnsignedShort
    );
    valid(&out, "spec wide");
}

#[test]
fn interleave_shares_one_strided_view_per_primitive() {
    let s = Scene::factory();
    let plain = s.export(&[]);
    let out = s.export(&[on("frap_interleave")]);
    let (doc, _) = parse(&out);
    let prim = &doc.meshes[0].primitives[0];
    let views: Vec<Option<Index>> = prim
        .attributes
        .values()
        .map(|&a| doc.accessors[a as usize].buffer_view)
        .collect();
    assert_eq!(views.len(), 3, "POSITION, NORMAL, TEXCOORD_0");
    assert!(views.windows(2).all(|w| w[0] == w[1]), "{views:?}");
    let view = &doc.buffer_views[views[0].unwrap() as usize];
    // VEC3 + VEC3 + VEC2 floats.
    assert_eq!(view.byte_stride, Some(32));
    assert_eq!(view.target, Some(Target::ArrayBuffer));
    assert_eq!(view.byte_length, 32 * 24);
    let offsets: Vec<u64> = prim
        .attributes
        .values()
        .map(|&a| doc.accessors[a as usize].byte_offset_or_default())
        .collect();
    assert_eq!(offsets, [0, 12, 24]);
    // Two fewer views; same content.
    assert_eq!(
        doc.buffer_views.len(),
        plain.document.buffer_views.len() - 2
    );
    assert_eq!(attributes(&out), attributes(&plain));
    valid(&out, "interleave");
    // With SPEC as well (byte indices in the same buffer).
    let both = s.export(&[on("frap_interleave"), spec()]);
    assert_eq!(attributes(&both), attributes(&plain));
    valid(&both, "interleave spec");
}

#[test]
fn interleave_with_textures_and_separate_files() {
    // A textured material (the image view stays, renumbered) written as .gltf + .bin.
    let mut s = Scene::factory();
    let mut log = ChangeLog::default();
    let img = frap_scene::id::Image::new("Grid", 2, 2);
    let im = s.main.images.insert("Grid", img, &mut log);
    let mr = s.main.materials.find("Material").unwrap();
    {
        let m = s.main.materials.get_mut(mr, &mut log).unwrap();
        let i = m.node_tree.new_node("ShaderNodeTexImage").unwrap();
        let tex = m.node_tree.nodes[i].uid;
        m.node_tree
            .node_mut(tex)
            .unwrap()
            .set_prop("image", Value::Id(Some(im.any())));
        let bsdf = m
            .node_tree
            .first_of_type("ShaderNodeBsdfPrincipled")
            .unwrap()
            .uid;
        m.node_tree
            .link_by_name((tex, "Color"), (bsdf, "Base Color"))
            .unwrap();
    }
    let plain = s.export(&[]);
    let out = s.export(&[on("frap_interleave"), spec()]);
    let (doc, bufs) = parse(&out);
    assert_eq!(doc.images.len(), 1);
    let iv = doc.images[0].buffer_view.unwrap();
    let (pdoc, pbufs) = parse(&plain);
    let pv = pdoc.images[0].buffer_view.unwrap();
    let bytes = |d: &Document, b: &[Vec<u8>], v: Index| {
        frap_gltf::accessor::buffer_view_bytes(d, b, v)
            .unwrap()
            .to_vec()
    };
    assert_eq!(bytes(&doc, &bufs, iv), bytes(&pdoc, &pbufs, pv));
    assert_eq!(attributes(&out), attributes(&plain));
    valid(&out, "interleave textured");
}

#[test]
fn node_visibility_from_hide_render() {
    let mut s = Scene::factory();
    let plain = s.export(&[on("frap_node_visibility")]);
    assert!(plain.document.extensions_used.is_empty(), "nothing hidden");
    let cube = s.object("Cube");
    s.main
        .objects
        .get_mut(cube, &mut s.log)
        .unwrap()
        .hide_render = true;
    // Off by default: Blender's output.
    let blender = s.export(&[]);
    assert!(
        blender
            .document
            .nodes
            .iter()
            .all(|n| n.extensions.is_empty())
    );
    let out = s.export(&[on("frap_node_visibility")]);
    let doc = &out.document;
    let n = doc
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Cube"))
        .unwrap();
    assert_eq!(
        n.extensions.get_raw("KHR_node_visibility"),
        Some(&serde_json::json!({"visible": false}))
    );
    assert_eq!(doc.extensions_used, ["KHR_node_visibility"]);
    assert!(doc.extensions_required.is_empty());
    assert!(out.reports.is_empty(), "{:?}", out.reports);
    valid(&out, "visibility");
    // A renderable child of the hidden cube: hidden by the extension, with a WARNING.
    let cam = s.object("Camera");
    s.main.objects.get_mut(cam, &mut s.log).unwrap().parent = Some(cube);
    let out = s.export(&[
        on("frap_node_visibility"),
        ("export_cameras", Value::Bool(true)),
    ]);
    let w: Vec<&str> = out.reports.iter().map(|r| r.message.as_str()).collect();
    assert!(
        w.iter()
            .any(|m| m.contains("hides 'Camera' with its hidden ancestor 'Cube'")),
        "{w:?}"
    );
    valid(&out, "visibility hierarchy");
}

#[test]
fn diffuse_transmission_from_thin_walled_subsurface() {
    let mut s = Scene::factory();
    let mr = s.main.materials.find("Material").unwrap();
    {
        let m = s.main.materials.get_mut(mr, &mut s.log).unwrap();
        let bsdf = m
            .node_tree
            .nodes
            .iter_mut()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap();
        bsdf.input_mut("Thin Wall").unwrap().default_value = SocketValue::Bool(true);
        bsdf.input_mut("Subsurface Weight").unwrap().default_value = SocketValue::Float(0.25);
        bsdf.input_mut("Base Color").unwrap().default_value =
            SocketValue::Color([0.5, 0.25, 1.0, 1.0]);
    }
    let blender = s.export(&[]);
    assert!(
        blender.document.materials[0]
            .extensions
            .get_raw("KHR_materials_diffuse_transmission")
            .is_none()
    );
    let out = s.export(&[on("frap_diffuse_transmission")]);
    let m = &out.document.materials[0];
    let e = m
        .extensions
        .get_raw("KHR_materials_diffuse_transmission")
        .unwrap();
    assert_eq!(e["diffuseTransmissionFactor"], 0.25);
    assert_eq!(
        e["diffuseTransmissionColorFactor"],
        serde_json::json!([0.5, 0.25, 1.0])
    );
    assert!(
        out.document
            .extensions_used
            .iter()
            .any(|n| n == "KHR_materials_diffuse_transmission")
    );
    assert!(out.document.extensions_required.is_empty());
    valid(&out, "diffuse transmission");
    // frap imports it back into the same Principled settings.
    let dir = std::env::temp_dir().join(format!("frap-spec-dt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dt.glb");
    std::fs::write(&path, &out.bytes).unwrap();
    let (mut main, win) = startup::factory();
    let mut log = ChangeLog::default();
    let report = crate::import::import_file(
        &mut main,
        &win,
        &path,
        &crate::import::ImportSettings::default(),
        &mut log,
    )
    .unwrap();
    assert!(report.reports.is_empty(), "{:?}", report.reports);
    let _ = std::fs::remove_dir_all(&dir);
    let mat = main
        .materials
        .iter()
        .find(|(_, m)| m.header.name == "Material.001")
        .map(|(_, m)| m)
        .unwrap();
    let bsdf = mat
        .node_tree
        .first_of_type("ShaderNodeBsdfPrincipled")
        .unwrap();
    assert_eq!(
        bsdf.input("Thin Wall").unwrap().default_value,
        SocketValue::Bool(true)
    );
    assert_eq!(
        bsdf.input("Subsurface Weight").unwrap().default_value,
        SocketValue::Float(0.25)
    );
}

#[test]
fn spec_animation_times_follow_blenders_clock() {
    for ipo in [Interpolation::Linear, Interpolation::Bezier] {
        let mut s = Scene::factory();
        let sc = s.win.scene;
        {
            let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
            scene.render.fps = 30;
            scene.render.fps_base = 1.001;
        }
        let cube = s.object("Cube");
        s.animate(cube, ipo);
        let sampled = (
            "export_force_sampling",
            Value::Bool(ipo == Interpolation::Linear),
        );
        let blender = s.export(std::slice::from_ref(&sampled));
        let out = s.export(&[sampled, spec()]);
        let b = f64::from(1.001f32);
        let times = |o: &ExportOutput| {
            let (doc, bufs) = parse(o);
            let smp = &doc.animations[0].samplers[0];
            let t = read_accessor(&doc, &bufs, smp.input).unwrap().to_f32();
            let v = read_accessor(&doc, &bufs, smp.output).unwrap().to_f32();
            (t, v, doc.accessors[smp.input as usize].max.clone().unwrap())
        };
        let (tb, vb, _) = times(&blender);
        let (ts, vs, max) = times(&out);
        // The add-on: frame / (fps × fps_base); SPEC: frame × fps_base / fps.
        let last = *ts.last().unwrap();
        assert!((f64::from(last) - 45.0 * b / 30.0).abs() < 1e-6, "{last}");
        assert!((f64::from(*tb.last().unwrap()) - 45.0 / (30.0 * b)).abs() < 1e-6);
        assert_eq!(max, [f64::from(last)]);
        assert_eq!(ts.len(), tb.len());
        if ipo == Interpolation::Bezier {
            // CUBICSPLINE: values equal, tangents per second scaled by 1 / fps_base².
            for (k, (x, y)) in vs.iter().zip(&vb).enumerate() {
                let role = (k / 3) % 3;
                let want = if role == 1 {
                    *y
                } else {
                    (f64::from(*y) / (b * b)) as f32
                };
                assert!((x - want).abs() < 1e-5, "{k}: {x} vs {want}");
            }
        } else {
            assert_eq!(vs, vb);
        }
        let warned = |o: &ExportOutput| o.reports.iter().any(|r| r.message.contains("fps_base"));
        assert!(warned(&blender) && !warned(&out));
        valid(&out, "spec times");
    }
}

/// The committed inputs of the `gltf/spec` goldens are what frap writes now
/// (`FRAP_BLESS_SPEC_INPUTS=1` rewrites them).
#[test]
fn golden_inputs_are_current() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/cases/gltf/spec/import_spec.inputs");
    let bless = std::env::var_os("FRAP_BLESS_SPEC_INPUTS").is_some();
    for (name, bytes) in golden_inputs() {
        let path = dir.join(name);
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let committed = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e} (bless with FRAP_BLESS_SPEC_INPUTS=1)",
                path.display()
            )
        });
        let norm = |b: &[u8]| {
            let mut g = frap_gltf::read(b).unwrap();
            g.document.asset.generator = None;
            (g.document, g.bin)
        };
        assert!(
            norm(&committed) == norm(&bytes),
            "{name} is stale (bless with FRAP_BLESS_SPEC_INPUTS=1)"
        );
    }
}

/// The files `import_spec.py` imports: every option on one scene (a hidden, interleaved,
/// byte-indexed, thin-walled cube with a renderable child and a light).
fn golden_inputs() -> Vec<(&'static str, Vec<u8>)> {
    let mut s = Scene::factory();
    let cube = s.object("Cube");
    s.main
        .objects
        .get_mut(cube, &mut s.log)
        .unwrap()
        .hide_render = true;
    let mr = s.main.materials.find("Material").unwrap();
    {
        let m = s.main.materials.get_mut(mr, &mut s.log).unwrap();
        let bsdf = m
            .node_tree
            .nodes
            .iter_mut()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap();
        bsdf.input_mut("Thin Wall").unwrap().default_value = SocketValue::Bool(true);
        bsdf.input_mut("Subsurface Weight").unwrap().default_value = SocketValue::Float(0.5);
    }
    let all = [
        spec(),
        on("frap_interleave"),
        on("frap_node_visibility"),
        on("frap_diffuse_transmission"),
        ("export_lights", Value::Bool(true)),
    ];
    let out = s.export(&all);
    valid(&out, "golden input");
    vec![("spec.glb", out.bytes)]
}
