// SPDX-License-Identifier: GPL-3.0-or-later
//! `render.render` through a real session (WP 1.24, ARCHITECTURE §19.4). Error paths run
//! everywhere (they never reach the GPU); tests that render need an adapter and are skipped
//! with a notice under `FRAP_GPU_OPTIONAL=1`. Expected Blender behaviour was verified on
//! 5.2.4 (see `crates/frap-ops-render/src/ops/render__render.rs`).

mod common;

use common::Env;
use frap_session::{Session, SessionOptions};
use serde_json::json;

/// A small, fast Workbench setup.
const SMALL: &str = "bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 64
bpy.context.scene.render.resolution_y = 36";

fn error(j: &serde_json::Value) -> (String, String, String) {
    (
        j["error"]["exception"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        j["error"]["code"].as_str().unwrap_or_default().to_owned(),
        j["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    )
}

fn runtime(msg: &str) -> (String, String, String) {
    (
        "RuntimeError".into(),
        "OP_EXEC_ERROR".into(),
        format!("Error: {msg}"),
    )
}

/// Runs a statement that must fail with `want` and leave the state untouched.
fn fails(e: &mut Env, call: &str, want: (String, String, String)) {
    let before = e.session.state_dump();
    let j = e.run(call);
    assert_eq!(error(&j), want, "{call}");
    assert_eq!(j["state"], "unchanged", "{call}");
    common::assert_same_state(&e.session.state_dump(), &before, call);
}

/// The checks Blender makes before rendering, in its order; none starts the GPU service and
/// none changes the state.
#[test]
fn checks_in_blender_order_without_gpu() {
    let mut e = Env::new("render_checks");
    e.ok(SMALL);
    e.ok("bpy.context.scene.camera = None");
    for call in [
        "bpy.ops.render.render()",
        "bpy.ops.render.render(write_still=True)",
        "bpy.ops.render.render(animation=True)",
    ] {
        fails(&mut e, call, runtime("Cannot render, no camera"));
    }
    // The frame range is checked before the camera, the size before the camera.
    let frames = runtime("Start frame is larger than end frame");
    fails(
        &mut e,
        "bpy.ops.render.render(animation=True, frame_start=300)",
        frames.clone(),
    );
    fails(
        &mut e,
        "bpy.ops.render.render(animation=True, frame_start=5, frame_end=4)",
        frames,
    );
    e.ok("bpy.context.scene.render.resolution_percentage = 1");
    fails(
        &mut e,
        "bpy.ops.render.render()",
        runtime("Image too small"),
    );
    e.ok("bpy.context.scene.render.resolution_percentage = 100");

    e.ok("bpy.context.scene.camera = bpy.data.objects['Camera']");
    // Templates are checked after the camera, only when a file is written.
    e.ok("bpy.context.scene.render.filepath = 'out/{nope}{scene_name:##}'");
    let template = runtime(
        "Parse errors in path 'out/{nope}{scene_name:##}':\n\
         - Unknown variable referenced in template expression '{nope}'.\n\
         - Invalid format specifier in template expression '{scene_name:##}'.",
    );
    fails(
        &mut e,
        "bpy.ops.render.render(write_still=True)",
        template.clone(),
    );
    fails(
        &mut e,
        "bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)",
        template,
    );
    e.ok("bpy.context.scene.render.filepath = 'out/x'");
    e.ok("bpy.context.scene.render.image_settings.media_type = 'VIDEO'");
    fails(
        &mut e,
        "bpy.ops.render.render(write_still=True)",
        runtime("Cannot write a single file with an animation format selected"),
    );
    e.ok("bpy.context.scene.render.image_settings.media_type = 'IMAGE'");
    e.ok("bpy.context.scene.render.image_settings.file_format = 'JPEG2000'");
    fails(
        &mut e,
        "bpy.ops.render.render(write_still=True)",
        (
            "NotImplementedError".into(),
            "OP_NOT_IMPLEMENTED".into(),
            "frap: render output image_settings.file_format='JPEG2000' is not supported yet".into(),
        ),
    );

    assert!(!e.session.render_service_started());
    assert!(!e.path("out").exists());
    assert!(e.session.main().images.find("Render Result").is_none());
}

/// `GPU_UNAVAILABLE` when no adapter can be used (here an unknown `FRAP_WGPU_BACKEND`; no adapter
/// at all and a build without the `gpu` feature give the same error): nothing changes, nothing
/// is written.
#[test]
fn gpu_unavailable_changes_nothing() {
    let dir = frap_golden::blender::ScratchDir::new("render_nogpu").unwrap();
    let mut opts = SessionOptions::default();
    opts.cwd = dir.path().to_owned();
    opts.gpu.backend = Some("none".into());
    let mut s = Session::new(opts).unwrap();
    for r in s.exec_source(SMALL) {
        assert!(r.ok());
    }
    s.exec_source("bpy.context.scene.render.filepath = 'out/x'");
    let before = common::state(&s);
    let j = s.exec_source("bpy.ops.render.render(write_still=True)")[0].to_json();
    assert_eq!(j["error"]["exception"], "RuntimeError");
    assert_eq!(j["error"]["code"], "GPU_UNAVAILABLE");
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Error: no GPU adapter"),
        "{j}"
    );
    common::assert_unchanged(&s, &before, "GPU_UNAVAILABLE");
    assert!(!dir.path().join("out").exists());
}

/// Rendering adds `Render Result` and changes nothing else (the scene is rendered from a
/// clone); a second render updates the same image.
#[test]
fn render_result_and_nothing_else() {
    let mut e = Env::new("render_result");
    e.ok(SMALL);
    e.ok("bpy.context.scene.frame_current = 7");
    let before_main = e.session.main().clone();
    let before_rest = e
        .session
        .state_dump()
        .split_once('\n')
        .unwrap()
        .1
        .to_owned();
    let Some(j) = e.render("bpy.ops.render.render()") else {
        return;
    };
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(
        j["result"],
        json!({"engine": "BLENDER_WORKBENCH", "resolution": [64, 36], "image": "Render Result", "frames": 1})
    );
    assert_eq!(
        j["changes"]["added"],
        json!(["bpy.data.images['Render Result']"])
    );
    assert_eq!(j["changes"]["modified"], json!([]));
    assert!(reports_empty(&j), "{j}");

    let mut after = e.session.main().clone();
    after.images = before_main.images.clone();
    after.is_dirty = before_main.is_dirty;
    assert_eq!(format!("{after:?}"), format!("{before_main:?}"));
    // The operator call is an evaluation boundary (`bpy.ops` updates the view layer first,
    // §7.3, WP 6.4): the pending `frame_current = 7` is consumed, nothing else changes.
    // The last line is `time_dirty anim_tagged`.
    assert!(before_rest.ends_with("\ntrue []"), "{before_rest}");
    assert_eq!(
        e.session.state_dump().split_once('\n').unwrap().1,
        format!("{}false []", before_rest.strip_suffix("true []").unwrap())
    );

    for (path, want) in [
        (
            "bpy.data.images['Render Result'].type",
            json!("RENDER_RESULT"),
        ),
        ("bpy.data.images['Render Result'].source", json!("VIEWER")),
        ("bpy.data.images['Render Result'].size", json!([0, 0])),
        (
            "bpy.data.images['Render Result'].file_format",
            json!("TARGA"),
        ),
        ("bpy.data.images['Render Result'].has_data", json!(false)),
        (
            "bpy.data.images['Render Result'].colorspace_settings.name",
            json!(""),
        ),
        ("bpy.context.scene.frame_current", json!(7)),
    ] {
        let g = e.run(path);
        assert_eq!(g["value"], want, "{path}: {g}");
    }

    let j = e.render("bpy.ops.render.render()").unwrap();
    assert_eq!(j["changes"]["added"], json!([]));
    assert_eq!(
        j["changes"]["modified"],
        json!(["bpy.data.images['Render Result']"])
    );
    let names: Vec<_> = e
        .session
        .main()
        .images
        .iter()
        .map(|(_, i)| i.header.name.to_string())
        .collect();
    assert_eq!(names, ["Render Result"]);
    // Nothing is written without write_still.
    assert_eq!(std::fs::read_dir(e.cwd()).unwrap().count(), 0);
}

fn reports_empty(j: &serde_json::Value) -> bool {
    common::reports(j).is_empty()
}

/// `BLENDER_EEVEE` (factory) renders with the EEVEE-like engine from M5 (WP 5.1), `CYCLES` with
/// it and the D-12 WARNING; the pixels of both are the EEVEE ones.
#[test]
fn engine_fallback_warns() {
    let mut e = Env::new("render_engines");
    e.ok(SMALL);
    let mut pngs = Vec::new();
    for (engine, warning) in [
        ("BLENDER_WORKBENCH", None),
        ("BLENDER_EEVEE", None),
        (
            "CYCLES",
            Some("Engine 'CYCLES' is not available in frap; rendered with BLENDER_EEVEE"),
        ),
    ] {
        e.ok(&format!("bpy.context.scene.render.engine = '{engine}'"));
        e.ok(&format!(
            "bpy.context.scene.render.filepath = 'out/{engine}'"
        ));
        let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
            return;
        };
        assert_eq!(j["ok"], true, "{j}");
        let want: Vec<(String, String)> = warning
            .map(|w| vec![("WARNING".to_owned(), w.to_owned())])
            .unwrap_or_default();
        assert_eq!(common::reports(&j), want, "{engine}");
        let used = if engine == "BLENDER_WORKBENCH" {
            engine
        } else {
            "BLENDER_EEVEE"
        };
        assert_eq!(j["result"]["engine"], used, "{j}");
        pngs.push(frap_imbuf::load(&e.path(&format!("out/{engine}.png"))).unwrap());
    }
    assert_ne!(pngs[0], pngs[1]);
    assert_eq!(pngs[1], pngs[2]);
}

/// Output paths: cwd-relative, `//` (unsaved: the cwd), templates, `use_file_extension`; a
/// path without a folder fails to save, as in Blender 5.2.4 (M5+M6 stabilization).
#[test]
fn output_paths() {
    let mut e = Env::new("render_paths");
    e.ok(SMALL);
    for (filepath, written, extra) in [
        ("out/a", "out/a.png", ""),
        ("//out/b", "out/b.png", ""),
        ("out/c.jpg", "out/c.png", ""),
        (
            "out/{scene_name}_{resolution_x:####}x{resolution_y}",
            "out/Scene_0064x36.png",
            "",
        ),
        ("out/hash_##", "out/hash_##.png", ""),
        ("out/dir/", "out/dir/Untitled.png", ""),
        (
            "out/noext",
            "out/noext",
            "bpy.context.scene.render.use_file_extension = False",
        ),
    ] {
        if !extra.is_empty() {
            e.ok(extra);
        }
        e.ok(&format!("bpy.context.scene.render.filepath = '{filepath}'"));
        let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
            return;
        };
        assert_eq!(j["ok"], true, "{j}");
        assert_eq!(j["result"]["filepath"], written, "{filepath}");
        assert_eq!(j["result"]["files"], json!([written]), "{filepath}");
        let img =
            frap_imbuf::load(&e.path(written)).unwrap_or_else(|err| panic!("{written}: {err}"));
        assert_eq!((img.width, img.height, img.channels), (64, 36, 4));
    }
    // A path without a folder: `Render error (No such file or directory) cannot save:
    // 'shot.png'` (verified on 5.2.4, also for `//shot` unsaved and `./shot`).
    e.ok("bpy.context.scene.render.use_file_extension = True");
    for filepath in ["shot", "//shot", "./shot"] {
        e.ok(&format!("bpy.context.scene.render.filepath = '{filepath}'"));
        let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
            return;
        };
        assert_eq!(j["ok"], false, "{filepath}: {j}");
        let msg = j["error"]["message"].as_str().unwrap();
        assert!(msg.ends_with(") cannot save: 'shot.png'"), "{msg}");
        #[cfg(unix)]
        assert!(msg.contains("(No such file or directory)"), "{msg}");
        assert!(!e.path("shot.png").exists());
    }
    // An absolute path is used as is.
    let abs = e.path("abs/z");
    e.ok(&format!(
        "bpy.context.scene.render.filepath = {:?}",
        abs.to_string_lossy()
    ));
    e.ok("bpy.context.scene.render.use_file_extension = True");
    let j = e.render("bpy.ops.render.render(write_still=True)").unwrap();
    assert_eq!(
        j["result"]["filepath"],
        format!("{}.png", abs.to_string_lossy())
    );
    assert!(e.path("abs/z.png").is_file());
}

/// `animation=True`: the scene range with `frame_step`, operator overrides, `#` padding, every
/// frame written, `frame_current` unchanged.
#[test]
fn animation_writes_every_frame() {
    let mut e = Env::new("render_anim");
    e.ok(SMALL);
    e.ok("bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 5
bpy.context.scene.frame_step = 2
bpy.context.scene.frame_current = 4
bpy.context.scene.render.filepath = 'out/f_'");
    let Some(j) = e.render("bpy.ops.render.render(animation=True)") else {
        return;
    };
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(
        j["result"]["files"],
        json!(["out/f_0001.png", "out/f_0003.png", "out/f_0005.png"])
    );
    assert_eq!(j["result"]["frames"], 3);
    assert_eq!(j["result"]["filepath"], "out/f_0005.png");
    e.ok("bpy.context.scene.frame_step = 1
bpy.context.scene.render.filepath = 'out/g_###_x'");
    let j = e
        .render("bpy.ops.render.render(animation=True, frame_start=-1, frame_end=0)")
        .unwrap();
    assert_eq!(
        j["result"]["files"],
        json!(["out/g_-001_x.png", "out/g_000_x.png"])
    );
    let j = e
        .render("bpy.ops.render.render(animation=True, frame_end=1)")
        .unwrap();
    assert_eq!(j["result"]["files"], json!(["out/g_001_x.png"]));
    for f in [
        "f_0001", "f_0003", "f_0005", "g_-001_x", "g_000_x", "g_001_x",
    ] {
        assert!(e.path(&format!("out/{f}.png")).is_file(), "{f}");
    }
    assert_eq!(e.run("bpy.context.scene.frame_current")["value"], 4);
}

/// `image_settings`: formats, colour modes and depths.
#[test]
fn file_formats() {
    let mut e = Env::new("render_formats");
    e.ok(SMALL);
    let cases: [(&[&str], &str, u8, u8); 8] = [
        (
            &["file_format = 'PNG'", "color_mode = 'BW'"],
            "out/x.png",
            1,
            8,
        ),
        (
            &["color_mode = 'RGB'", "color_depth = '16'"],
            "out/x.png",
            3,
            16,
        ),
        (
            &["file_format = 'JPEG'", "color_depth = '8'"],
            "out/x.jpg",
            3,
            8,
        ),
        (&["file_format = 'BMP'"], "out/x.bmp", 3, 8),
        (
            &["file_format = 'TARGA'", "color_mode = 'RGBA'"],
            "out/x.tga",
            4,
            8,
        ),
        (
            &["file_format = 'OPEN_EXR'", "color_depth = '32'"],
            "out/x.exr",
            4,
            32,
        ),
        (&["file_format = 'HDR'"], "out/x.hdr", 3, 32),
        (
            // HDR left `color_mode` RGB (`BKE_image_format_set`): RGBA again.
            &[
                "file_format = 'WEBP'",
                "color_mode = 'RGBA'",
                "color_depth = '8'",
                "quality = 100",
            ],
            "out/x.webp",
            4,
            8,
        ),
    ];
    for (assigns, file, channels, depth) in cases {
        let setup = assigns.join(", ");
        for a in assigns {
            e.ok(&format!("bpy.context.scene.render.image_settings.{a}"));
        }
        e.ok("bpy.context.scene.render.filepath = 'out/x'");
        let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
            return;
        };
        assert_eq!(j["ok"], true, "{setup}: {j}");
        assert_eq!(j["result"]["filepath"], file, "{setup}");
        let img = frap_imbuf::load(&e.path(file)).unwrap_or_else(|err| panic!("{file}: {err}"));
        assert_eq!(
            (img.width, img.height, img.channels, img.bit_depth()),
            (64, 36, channels, depth),
            "{setup}"
        );
        std::fs::remove_file(e.path(file)).unwrap();
    }
}

/// `scene=`/`layer=` naming nothing are ignored (as in Blender); `use_viewport` renders the
/// scene camera (frap's virtual viewport looks through it).
#[test]
fn ignored_arguments() {
    let mut e = Env::new("render_args");
    e.ok(SMALL);
    for call in [
        "bpy.ops.render.render(scene='Nope')",
        "bpy.ops.render.render(layer='Nope')",
        "bpy.ops.render.render(scene='Scene', layer='ViewLayer')",
        "bpy.ops.render.render(use_viewport=True)",
    ] {
        let Some(j) = e.render(call) else {
            return;
        };
        assert_eq!(j["ok"], true, "{call}: {j}");
    }
    let j = e.run("bpy.ops.render.render(use_sequencer_scene=True)");
    assert_eq!(j["error"]["exception"], "NotImplementedError");
}
