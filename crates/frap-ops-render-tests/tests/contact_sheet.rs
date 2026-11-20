// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap.render_contact_sheet` through a real session (WP 2.3, ARCHITECTURE §19.6): tiles equal
//! individual renders, the layout snapshot, labels and background, warnings and errors, and an
//! unchanged session. Tests that render need an adapter (skipped with a notice under
//! `FRAP_GPU_OPTIONAL=1`).
//!
//! The layout snapshot lives in `tests/snapshots/contact_sheet_layout.txt`;
//! `FRAP_SNAPSHOT_BLESS=1` rewrites it.

mod common;

use std::path::Path;

use common::Env;
use frap_golden::compare::image_ssim;
use frap_imbuf::DecodedImage;
use serde_json::Value as J;

/// A small Workbench scene: plane, factory cube, smooth sphere, shadows and outline.
const SCENE: &str = "bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.5, 0.5, 0.8))
bpy.ops.object.shade_smooth()
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.display.shading.show_object_outline = True";

fn rgba8(img: &DecodedImage) -> Vec<u8> {
    image_ssim::to_rgba8(img)
}

fn load(p: &Path) -> DecodedImage {
    image_ssim::load_png(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The pixels of `rect` (`[x, y, w, h]`) of an RGBA8 image `width` wide.
fn crop(px: &[u8], width: u32, rect: [u32; 4]) -> Vec<u8> {
    let [x, y, w, h] = rect;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let a = ((row * width + x) * 4) as usize;
        out.extend_from_slice(&px[a..a + (w * 4) as usize]);
    }
    out
}

fn rect(t: &J) -> [u32; 4] {
    let v: Vec<u32> = t["rect"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u32)
        .collect();
    [v[0], v[1], v[2], v[3]]
}

fn floats(t: &J, key: &str) -> Vec<f64> {
    t[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} in {t}"))
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

/// Runs a sheet; `None` without a GPU (skipped).
fn sheet(e: &mut Env, args: &str) -> Option<J> {
    let j = e.render(&format!("bpy.ops.frap.render_contact_sheet({args})"))?;
    assert_eq!(j["ok"], true, "{j}");
    Some(j)
}

#[test]
fn errors_need_no_gpu() {
    let mut e = Env::new("cs_errors");
    let j = e.run("bpy.ops.frap.render_contact_sheet(views=set())");
    assert_eq!(j["error"]["exception"], "RuntimeError", "{j}");
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .contains("views is empty"),
        "{j}"
    );
    e.ok("bpy.context.scene.camera = None");
    let j = e.run("bpy.ops.frap.render_contact_sheet(views={'CAMERA'})");
    assert_eq!(
        j["error"]["message"], "Error: Cannot render, no camera",
        "{j}"
    );
    let j = e.run("bpy.ops.frap.render_contact_sheet(use_scene_camera=True)");
    assert_eq!(
        j["error"]["message"], "Error: Cannot render, no camera",
        "{j}"
    );
    let j = e.run("bpy.ops.frap.render_contact_sheet(views={'SIDE'})");
    assert_eq!(j["error"]["exception"], "ValueError", "{j}");
    assert_eq!(j["error"]["code"], "OP_ARG_ENUM", "{j}");
    assert!(!e.path("sheet.png").exists());
}

#[test]
fn paths_sizes_and_none_need_no_gpu() {
    let mut e = Env::new("cs_paths");
    // A file name is required: '' and '//' would write a hidden '.png'.
    for fp in ["''", "'//'"] {
        let j = e.run(&format!("bpy.ops.frap.render_contact_sheet(filepath={fp})"));
        assert_eq!(j["error"]["exception"], "RuntimeError", "{fp}: {j}");
        assert!(
            j["error"]["message"]
                .as_str()
                .unwrap()
                .contains("names no file"),
            "{fp}: {j}"
        );
    }
    assert!(!e.path(".png").exists());
    // Sheets above 4096 x 4096 are refused before anything renders.
    let j = e.run(
        "bpy.ops.frap.render_contact_sheet(views={'FRONT','BACK','RIGHT','LEFT','TOP','BOTTOM','ISO'}, view_size=4096)",
    );
    assert_eq!(j["error"]["exception"], "RuntimeError", "{j}");
    let m = j["error"]["message"].as_str().unwrap();
    assert!(
        m.contains("12288 x 12288") && m.contains("view_size"),
        "{m}"
    );
    // `frame=None` is the documented "unset" (§19.6), also in the short form; Blender's own
    // operators keep rejecting None for numbers.
    let j = e.run("bpy.ops.frap.render_contact_sheet(frame=None, views=set())");
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .contains("views is empty"),
        "frame=None accepted: {j}"
    );
    let j = e.run("frap.render_contact_sheet frame=None filepath=''");
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .contains("names no file"),
        "short form frame=None accepted: {j}"
    );
    let j = e.run("bpy.ops.mesh.primitive_cube_add(size=None)");
    assert_eq!(j["error"]["exception"], "TypeError", "{j}");
}

#[test]
fn a_set_frame_keeps_the_file_name() {
    let mut e = Env::new("cs_frame_path");
    e.ok(SCENE);
    let Some(j) = sheet(
        &mut e,
        "frame=10, filepath='//framed.png', views={'TOP'}, view_size=16",
    ) else {
        return;
    };
    assert_eq!(j["result"]["filepath"], "framed.png", "{j}");
    assert!(e.path("framed.png").is_file());
    assert!(!e.path("framed.png0010.png").exists());
    // Without an extension: still no frame number.
    let j = sheet(
        &mut e,
        "frame=3, filepath='//f', views={'TOP'}, view_size=16",
    )
    .unwrap();
    assert_eq!(j["result"]["filepath"], "f.png", "{j}");
}

#[test]
fn tiles_equal_individual_renders() {
    let mut e = Env::new("cs_tiles");
    e.ok(SCENE);
    let Some(all) = sheet(
        &mut e,
        "filepath='//all.png', views={'FRONT','TOP','ISO','CAMERA'}, view_size=96, show_labels=False",
    ) else {
        return;
    };
    let tiles = all["result"]["tiles"].as_array().unwrap().clone();
    let names: Vec<&str> = tiles.iter().map(|t| t["view"].as_str().unwrap()).collect();
    assert_eq!(names, ["FRONT", "TOP", "ISO", "CAMERA"], "item order");
    let all_img = load(&e.path("all.png"));
    assert_eq!((all_img.width, all_img.height), (192, 192));
    let all_px = rgba8(&all_img);

    for t in &tiles {
        let view = t["view"].as_str().unwrap();
        let r = rect(t);
        let tile = crop(&all_px, 192, r);
        // The same view on its own sheet: the same pixels.
        let one = sheet(
            &mut e,
            &format!(
                "filepath='//one_{view}.png', views={{'{view}'}}, view_size=96, show_labels=False"
            ),
        )
        .unwrap();
        let t1 = &one["result"]["tiles"][0];
        let img = load(&e.path(&format!("one_{view}.png")));
        let single = crop(&rgba8(&img), img.width, rect(t1));
        assert_eq!(
            tile, single,
            "{view}: tile differs from its single-view sheet"
        );

        // A real camera with the reported values renders the same tile.
        if view == "CAMERA" {
            assert_eq!(t["camera"], "Camera");
            assert_eq!(r[2..], [96, 54], "1920 x 1080 letterboxed in 96 px");
            // `render.render` of the scene camera at the tile size gives the tile.
            e.ok("bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.render.filepath = '//real_CAMERA'");
            let j = e.render("bpy.ops.render.render(write_still=True)").unwrap();
            assert_eq!(j["ok"], true, "{j}");
            let real = rgba8(&load(&e.path("real_CAMERA.png")));
            assert_eq!(real.len(), tile.len(), "CAMERA: tile size");
            let max = tile
                .iter()
                .zip(&real)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(max <= 1, "CAMERA: render.render differs by {max}");
            e.ok("bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080");
            continue;
        }
        if view == "ISO" {
            // The factory viewport rotation, as `align='VIEW'` gives it (no roll).
            e.ok("bpy.ops.object.camera_add(align='VIEW')");
            let j = e.run("bpy.context.active_object.rotation_euler[:]");
            let want: Vec<f64> = j["value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            let got = floats(t, "rotation_euler");
            for (a, b) in got.iter().zip(&want) {
                assert!(
                    (a - b).abs() < 1e-5,
                    "ISO rotation {got:?} vs align='VIEW' {want:?}"
                );
            }
            assert!(got[1].abs() < 1e-6, "ISO tile has no roll: {got:?}");
            e.ok("bpy.data.objects.remove(bpy.context.active_object)");
        }
        let loc = floats(t, "location");
        let rot = floats(t, "rotation_euler");
        let src = format!(
            "bpy.data.cameras.new('Tile_{view}')
bpy.data.cameras['Tile_{view}'].type = 'ORTHO'
bpy.data.cameras['Tile_{view}'].ortho_scale = {s}
bpy.data.cameras['Tile_{view}'].clip_start = {cs}
bpy.data.cameras['Tile_{view}'].clip_end = {ce}
bpy.data.objects.new('Tile_{view}', bpy.data.cameras['Tile_{view}'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Tile_{view}'])
bpy.data.objects['Tile_{view}'].location = ({l0}, {l1}, {l2})
bpy.data.objects['Tile_{view}'].rotation_euler = ({r0}, {r1}, {r2})
bpy.context.scene.camera = bpy.data.objects['Tile_{view}']
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 96
bpy.context.scene.render.filepath = '//real_{view}'",
            s = t["ortho_scale"],
            cs = t["clip_start"],
            ce = t["clip_end"],
            l0 = loc[0],
            l1 = loc[1],
            l2 = loc[2],
            r0 = rot[0],
            r1 = rot[1],
            r2 = rot[2],
        );
        e.ok(&src);
        let j = e.render("bpy.ops.render.render(write_still=True)").unwrap();
        assert_eq!(j["ok"], true, "{j}");
        let real = rgba8(&load(&e.path(&format!("real_{view}.png"))));
        let max = tile
            .iter()
            .zip(&real)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(max <= 1, "{view}: real camera render differs by {max}");
        // Back to the scene camera for the next views.
        e.ok("bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080");
        // The tile camera objects stay out of later tiles (they are not meshes).
    }
}

/// `size` and `[view, rect]` of each tile, one line.
fn layout_line(label: &str, j: &J) -> String {
    let r = &j["result"];
    let tiles: Vec<String> = r["tiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| format!("{} {:?}", t["view"].as_str().unwrap(), rect(t)))
        .collect();
    format!(
        "{label}: size {}x{} | {}",
        r["size"][0],
        r["size"][1],
        tiles.join(" | ")
    )
}

#[test]
fn layout_snapshot() {
    let mut e = Env::new("cs_layout");
    let configs = [
        ("default", "view_size=64"),
        ("three", "views={'TOP','FRONT','ISO'}, view_size=40"),
        (
            "all_views_3_columns",
            "views={'FRONT','BACK','RIGHT','LEFT','TOP','BOTTOM','ISO'}, use_scene_camera=True, columns=3, view_size=32",
        ),
        (
            "one_column",
            "views={'RIGHT','LEFT'}, columns=1, view_size=24",
        ),
        (
            "columns_above_tiles",
            "views={'TOP'}, columns=4, view_size=16",
        ),
        ("camera_only", "views={'CAMERA'}, view_size=100"),
    ];
    let mut lines = Vec::new();
    for (label, args) in configs {
        let Some(j) = sheet(
            &mut e,
            &format!("filepath='//{label}.png', show_labels=False, {args}"),
        ) else {
            return;
        };
        lines.push(layout_line(label, &j));
    }
    // Portrait output: the CAMERA tile is pillarboxed.
    e.ok("bpy.context.scene.render.resolution_x = 1080
bpy.context.scene.render.resolution_y = 1920");
    let j = sheet(
        &mut e,
        "filepath='//portrait.png', views={'CAMERA','TOP'}, view_size=100",
    )
    .unwrap();
    lines.push(layout_line("camera_portrait", &j));
    let got = lines.join("\n") + "\n";
    let snap =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/contact_sheet_layout.txt");
    if std::env::var("FRAP_SNAPSHOT_BLESS").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(snap.parent().unwrap()).unwrap();
        std::fs::write(&snap, &got).unwrap();
        eprintln!("blessed {}", snap.display());
    }
    let want = std::fs::read_to_string(&snap)
        .unwrap_or_else(|e| panic!("{}: {e} (FRAP_SNAPSHOT_BLESS=1 writes it)", snap.display()));
    assert_eq!(got, want, "contact-sheet layout changed");
}

#[test]
fn labels_background_and_warnings() {
    let mut e = Env::new("cs_labels");
    e.ok(SCENE);
    let Some(j) = sheet(
        &mut e,
        "filepath='//labels', views={'TOP','FRONT','ISO'}, view_size=160, engine='BLENDER_EEVEE', background=(1.0, 0.0, 1.0, 1.0)",
    ) else {
        return;
    };
    // `.png` added; the tiles render with EEVEE (M5, WP 5.1): no fallback WARNING.
    assert_eq!(j["result"]["filepath"], "labels.png", "{j}");
    assert_eq!(j["result"]["engine"], "BLENDER_EEVEE");
    let reports = common::reports(&j);
    assert!(
        !reports
            .iter()
            .any(|(t, m)| t == "WARNING" && m.contains("BLENDER_EEVEE")),
        "{reports:?}"
    );
    let img = load(&e.path("labels.png"));
    assert_eq!((img.width, img.height), (320, 320));
    let px = rgba8(&img);
    let at = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * 320 + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    // The fourth cell of the 2 x 2 grid is the background (display space, 8 bits).
    assert_eq!(at(240, 240), [255, 0, 255, 255]);
    assert_eq!(at(319, 319), [255, 0, 255, 255]);
    // Each tile's label: white glyph cells at its top-left corner ('F' of FRONT: top row ink).
    let f_tile = &j["result"]["tiles"][0];
    assert_eq!(f_tile["view"], "FRONT");
    let [x, y, _, _] = rect(f_tile);
    assert_eq!(at(x + 2, y + 2), [255, 255, 255, 255]);
    // Without labels the same pixel is the render.
    let j = sheet(
        &mut e,
        "filepath='//nolabels.png', views={'TOP','FRONT','ISO'}, view_size=160, show_labels=False",
    )
    .unwrap();
    assert!(common::reports(&j).is_empty(), "{j}");
    let img = load(&e.path("nolabels.png"));
    let px2 = rgba8(&img);
    let i = (((y + 2) * 320 + x + 2) * 4) as usize;
    assert_ne!(&px2[i..i + 4], &[255, 255, 255, 255]);
}

#[test]
fn the_session_does_not_change() {
    let mut e = Env::new("cs_state");
    e.ok(SCENE);
    let before = common::state(&e.session);
    let Some(j) = sheet(
        &mut e,
        "filepath='//s.png', view_size=32, use_scene_camera=True",
    ) else {
        return;
    };
    assert_eq!(j["changes"]["added"], serde_json::json!([]), "{j}");
    assert_eq!(j["changes"]["modified"], serde_json::json!([]), "{j}");
    common::assert_unchanged(&e.session, &before, "frap.render_contact_sheet");
    assert!(e.path("s.png").is_file());
    // No Render Result either.
    let j = e.run("bpy.data.images.find('Render Result')");
    assert_eq!(j["value"], -1, "{j}");
    // A frame other than the current one leaves frame_current alone.
    let j = sheet(
        &mut e,
        "filepath='//f.png', views={'TOP'}, view_size=16, frame=7",
    )
    .unwrap();
    assert_eq!(j["context"]["frame_current"], 1, "{j}");
    common::assert_unchanged(&e.session, &before, "frap.render_contact_sheet(frame=7)");
}

#[test]
fn selected_objects_only() {
    let mut e = Env::new("cs_selected");
    e.ok(SCENE);
    // Only the sphere is selected: the TOP tile frames it alone (smaller ortho scale) and the
    // plane is not drawn.
    e.ok("bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)");
    let Some(sel) = sheet(
        &mut e,
        "filepath='//sel.png', views={'TOP'}, objects='SELECTED', view_size=64, show_labels=False",
    ) else {
        return;
    };
    let vis = sheet(
        &mut e,
        "filepath='//vis.png', views={'TOP'}, objects='VISIBLE', view_size=64, show_labels=False",
    )
    .unwrap();
    let s_sel = sel["result"]["tiles"][0]["ortho_scale"].as_f64().unwrap();
    let s_vis = vis["result"]["tiles"][0]["ortho_scale"].as_f64().unwrap();
    assert!((s_sel - 1.2 * 1.1).abs() < 1e-4, "{s_sel}");
    assert!((s_vis - 6.0 * 1.1).abs() < 1e-4, "{s_vis}");
    let img = load(&e.path("sel.png"));
    let px = rgba8(&img);
    // Corner: background, not the plane.
    let bg = {
        let j = e.run("bpy.context.scene.world.color[0]");
        j["value"].as_f64().unwrap()
    };
    assert!(bg < 0.1);
    let corner = [px[0], px[1], px[2]];
    assert!(corner.iter().all(|c| *c < 80), "{corner:?}");
}

/// Nothing to frame (no object selected, or no mesh at all) still renders, framed on a 2 m
/// cube around the origin, and says so with a WARNING (M2 stabilization round 3).
#[test]
fn nothing_to_frame_warns() {
    let mut e = Env::new("cs_nothing");
    let warning = |j: &J| -> Option<String> {
        common::reports(j)
            .into_iter()
            .find(|(t, m)| t == "WARNING" && m.starts_with("frap: no objects to frame"))
            .map(|(_, m)| m)
    };
    e.ok("bpy.ops.object.select_all(action='DESELECT')");
    let Some(j) = sheet(
        &mut e,
        "filepath='//none.png', views={'TOP'}, objects='SELECTED', view_size=16",
    ) else {
        return;
    };
    assert_eq!(
        warning(&j).as_deref(),
        Some(
            "frap: no objects to frame (objects='SELECTED', none selected); the views frame a 2 m cube around the origin"
        ),
        "{j}"
    );
    let j = sheet(&mut e, "filepath='//vis.png', views={'TOP'}, view_size=16").unwrap();
    assert_eq!(warning(&j), None, "{j}");
    e.ok("bpy.data.objects.remove(bpy.data.objects['Cube'])");
    let j = sheet(
        &mut e,
        "filepath='//empty.png', views={'TOP'}, view_size=16",
    )
    .unwrap();
    assert!(
        warning(&j).is_some_and(|m| m.contains("objects='VISIBLE'")),
        "{j}"
    );
}

/// Cavity and depth of field are not rendered yet: both `render.render` and the sheet say so
/// (WARNING), also when EEVEE or CYCLES fall back to Workbench, and say nothing when X-ray
/// disables them (Blender ignores them there too).
#[test]
fn unrendered_workbench_effects_warn_in_both_ops() {
    let mut e = Env::new("cs_cavity");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 32
bpy.context.scene.render.resolution_y = 18
bpy.context.scene.render.filepath = '//cav'
bpy.context.scene.display.shading.show_cavity = True");
    let warned = |j: &J, what: &str| {
        common::reports(j)
            .iter()
            .any(|(t, m)| t == "WARNING" && m.contains(what))
    };
    let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
        return;
    };
    assert!(warned(&j, "Workbench cavity is not rendered"), "{j}");
    let j = sheet(&mut e, "filepath='//c.png', views={'TOP'}, view_size=16").unwrap();
    assert!(warned(&j, "Workbench cavity is not rendered"), "{j}");
    e.ok("bpy.context.scene.display.shading.show_xray = True");
    let j = e.render("bpy.ops.render.render(write_still=True)").unwrap();
    assert!(!warned(&j, "cavity"), "X-ray disables cavity: {j}");
    // EEVEE and CYCLES render with the EEVEE-like engine from M5 (WP 5.1; CYCLES with the D-12
    // WARNING): Workbench effects are not reported for them.
    for engine in ["BLENDER_EEVEE", "CYCLES"] {
        e.ok(&format!(
            "bpy.context.scene.display.shading.show_xray = False
bpy.context.scene.render.engine = '{engine}'"
        ));
        let j = e.render("bpy.ops.render.render(write_still=True)").unwrap();
        assert!(
            !warned(&j, "rendered with BLENDER_WORKBENCH"),
            "{engine}: {j}"
        );
        assert_eq!(
            warned(
                &j,
                "Engine 'CYCLES' is not available in frap; rendered with BLENDER_EEVEE"
            ),
            engine == "CYCLES",
            "{engine}: {j}"
        );
        assert!(
            !warned(&j, "Workbench cavity is not rendered"),
            "{engine}: {j}"
        );
    }
}
