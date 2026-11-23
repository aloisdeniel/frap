// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.render.render(animation=False, write_still=False, use_viewport=False,
//! use_sequencer_scene=False, layer='', scene='', frame_start=0, frame_end=0)`, tier T4
//! (ARCHITECTURE §19.4; WP 1.24, animation render WP 6.6). Behaviour of `screen_render_exec` /
//! `RE_RenderFrame` / `RE_RenderAnim` (`editors/render/render_internal.cc`,
//! `render/intern/pipeline.cc`, Blender 5.2.4, GPL-2.0-or-later), verified on 5.2.4:
//!
//! 1. Checks, in Blender's order (each a `RuntimeError: Error: …`, none needs a GPU):
//!    `frame_start`/`frame_end` set without `animation`: `Frame start/end specified in a
//!    non-animation render` (set, not non-zero: `frame_start=0` counts); `animation=True` with
//!    `frame_start > frame_end` (the operator's `frame_start`/`frame_end` override the scene's
//!    only when set, `get_render_operator_frame_range`): `Start frame is larger than end
//!    frame`; `write_still` with a video `media_type`: `Cannot write a single file with an
//!    animation format selected`; `animation=True` with `render.save_output` off
//!    (`RE_disable_save_output_allowed`; frap has no compositing node group, so no File Output
//!    node can take over): `Render output and compositing disabled in Output properties`, or
//!    with `use_compositing` `Render output disabled in Output properties and no active
//!    compositing File Output nodes`; a resolution that rounds to zero: `Image too small`; no
//!    `scene.camera`: `Cannot render, no camera`; then the output path templates (`Parse errors
//!    in path …`). `scene=`/`layer=` naming no scene/view layer are ignored, as in Blender;
//!    naming another one is `NotImplementedError` (frap renders the context scene and view
//!    layer).
//! 2. Each frame renders from its own COW clone of the state with animation applied at that
//!    frame (`OpCtx::snapshot_at`, §7.3: one clone per frame, dropped after the frame): the
//!    `RenderScene` is built from the clone and rendered through `cx.services().render()` (the
//!    only way to the GPU: `GPU_UNAVAILABLE` when there is no adapter or no `gpu` feature).
//!    Nothing of the session changes except the `Render Result` image (D-07: Blender's
//!    `ED_update_for_newframe` after the render re-applies the animation of `frame_current` to
//!    the originals, overwriting manual edits of animated channels; frap keeps them) and
//!    `scene.camera`, which follows the timeline markers as in Blender (WP 7.6): each frame
//!    renders with the markers' camera of that frame (`BKE_scene_camera_switch_update` in
//!    `render_update_depsgraph` / `do_render_full_pipeline`, on the original scene), else with
//!    `scene.camera` (`Cannot render, no camera` when a frame has neither), `{camera_name}`
//!    in output paths names it, and afterwards the scene camera is the markers' camera of
//!    `frame_current` (`ED_update_for_newframe`).
//! 3. `BLENDER_EEVEE` renders with frap's EEVEE-like renderer (M5); `CYCLES` renders with
//!    `BLENDER_EEVEE`, with a WARNING (D-12). A render service without EEVEE falls back to
//!    Workbench, with a WARNING. EEVEE renders also report what the world graph leaves out.
//! 4. The film (scene-linear) becomes `bpy.data.images['Render Result']` (created when missing
//!    with Blender's values: `source='VIEWER'`, `type='RENDER_RESULT'`, `size` `(0, 0)`
//!    headless); its pixels go to the session's image cache. `write_still` writes
//!    `scene.render.filepath` following [`crate::output`]'s rules, colour-managed (§19.5).
//! 5. `animation=True` (`RE_RenderAnim`) renders `frame_start`, `frame_start + frame_step`, …
//!    up to `frame_end` (only `frame_start` with `frame_step = 0`) and writes each frame to its
//!    frame path (`#` runs replaced by the
//!    zero-padded frame, else a 4-digit frame appended, then the extension). Before a frame:
//!    with `use_overwrite` off an existing file skips the frame (not rendered; when every frame
//!    is skipped the INFO `No frames rendered, skipped to not overwrite` is reported and
//!    `Render Result` is left empty); with `use_placeholder` an empty file is created first
//!    (with its directories) and removed again, when still empty, if the frame fails. Still
//!    renders ignore both settings, as in Blender. `frame_current` is the same afterwards
//!    (Blender restores `cfra` and `subframe`, verified).
//!
//! Output paths are computed before anything is rendered, so a template error fails without a
//! render. Files are written before `Render Result` is updated: a failed write leaves the
//! session untouched (the files of earlier frames stay, as in Blender). File errors name the
//! path as Blender prints it (relative stays relative, normalised: `out/../out/a_` →
//! `out/a_`); a directory that cannot be created fails as Blender's save does (`Render error
//! (<strerror>) cannot save: '<path>'`).
//!
//! `result`: `engine` (the engine that rendered), `resolution`, `image` (`"Render Result"`),
//! `frames` (frames rendered), and with files `filepath` (the last file) and `files` (every
//! file written, as Blender prints them: relative to the cwd unless absolute or `//`); with
//! `use_overwrite` off, `skipped` lists the existing files whose frames were skipped.

use std::path::Path;
use std::sync::Arc;

use frap_imbuf::{DecodedImage, PixelData};
use frap_op::operator;
use frap_op::prelude::*;
use frap_render_scene::{Film, RenderError, RenderRequest};
use frap_rna::{IdRef, ImageId};
use frap_scene::id::Image;
use frap_spec_ops_render::render;

use crate::output::{self, OutputPath, OutputRules, TemplateVars, format};

/// The Workbench engine.
pub(crate) const WORKBENCH: &str = "BLENDER_WORKBENCH";
/// Name of the render result image.
const RENDER_RESULT: &str = "Render Result";

fn render_error(e: RenderError) -> OpError {
    match e {
        RenderError::Unsupported(m) => OpError::Runtime(m),
        other => OpError::Gpu(other.to_string()),
    }
}

/// `BLENDER_EEVEE`.
const EEVEE: &str = "BLENDER_EEVEE";

/// The engine that renders `requested` (M5+M6 W0): `CYCLES` renders with `BLENDER_EEVEE` once
/// the render service supports it (§19.3, D-12), `BLENDER_EEVEE` with itself; anything the
/// service does not support falls back to `BLENDER_WORKBENCH`.
pub(crate) fn effective_engine(requested: &str, eevee: bool) -> &'static str {
    match requested {
        "BLENDER_EEVEE" | "CYCLES" if eevee => EEVEE,
        _ => WORKBENCH,
    }
}

/// The WARNING for an engine frap renders with another one (deviations.md D-12).
fn engine_fallback(engine: &str, eevee: bool) -> Option<String> {
    let used = effective_engine(engine, eevee);
    match engine {
        WORKBENCH => None,
        EEVEE if eevee => None,
        "CYCLES" => Some(format!(
            "Engine 'CYCLES' is not available in frap; rendered with {used}"
        )),
        other => Some(format!(
            "Engine '{other}' is not available in this build of frap; rendered with {used}"
        )),
    }
}

/// Everything read from the context before rendering.
struct Plan {
    engine: String,
    /// `[x, y]` render size (`render_resolution`).
    resolution: [i32; 2],
    frames: Vec<Option<i32>>,
    /// Output file per frame (`None`: nothing is written).
    outputs: Vec<Option<OutputPath>>,
    /// The output settings of the context scene (a still render; an animation render reads
    /// them again from each frame's scene, [`frame_output`]).
    out: FrameOutput,
    view_layer: String,
    /// `RE_RenderAnim`'s per-frame file options (animation renders only).
    anim_files: AnimFiles,
}

/// What a frame is written with: `BKE_image_render_write` reads `scene->r.im_format`,
/// `scene->view_settings`/`display_settings` and `scene->r.dither_intensity` from the scene
/// whose animation `RE_RenderAnim` evaluated for that frame, so animated exposure, gamma,
/// look, dither or quality apply per frame (verified on 5.2.4: an exposure keyed 0 to 2 over
/// frames 1..4 gives frame means 66.7..131.6).
struct FrameOutput {
    write: Option<format::WriteSettings>,
    cm: format::ColorManagement,
    dither: f32,
}

/// [`FrameOutput`] of `scene` (`writes`: the render writes files).
fn frame_output(
    scene: &frap_scene::id::Scene,
    view_layer: &str,
    writes: bool,
) -> Result<FrameOutput, OpError> {
    let r = &scene.render;
    let write = if writes {
        Some(format::render_write_settings(r, view_layer).map_err(OpError::NotImplemented)?)
    } else {
        None
    };
    let cm = if r.image_settings.color_management == "OVERRIDE" {
        format::color_management(
            &r.image_settings.view_settings,
            &r.image_settings.display_settings,
        )
    } else {
        format::color_management(&scene.view_settings, &scene.display_settings)
    };
    Ok(FrameOutput {
        write,
        cm,
        dither: r.dither_intensity,
    })
}

/// `render.use_overwrite` / `render.use_placeholder` as `RE_RenderAnim` applies them: only to
/// animation renders that write image files (`R_NO_OVERWRITE`, `R_TOUCH`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct AnimFiles {
    /// `!use_overwrite`: a frame whose file exists is skipped.
    skip_existing: bool,
    /// `use_placeholder`: an empty file is created before the frame renders.
    placeholder: bool,
}

/// The frames of an animation render: `start`, `start + step`, … up to `end`. `RE_RenderAnim`
/// walks `cfra` by 1 and renders when `cfra == nfra`, then `nfra += tfra`: with
/// `frame_step = 0` (the RNA minimum) only `start` renders.
fn animation_frames(start: i32, end: i32, step: i32) -> Vec<i32> {
    if step <= 0 {
        return if start <= end {
            vec![start]
        } else {
            Vec::new()
        };
    }
    (i64::from(start)..=i64::from(end))
        .step_by(step as usize)
        .map(|f| f as i32)
        .collect()
}

/// `RE_disable_save_output_allowed` for a scene without sequencer strips or compositing node
/// group (frap has neither): `Err` is the message of the error report.
fn save_output_allowed(
    animation: bool,
    save_output: bool,
    use_compositing: bool,
) -> Result<(), &'static str> {
    match (animation, save_output, use_compositing) {
        (true, false, false) => Err("Render output and compositing disabled in Output properties"),
        // `node_tree_has_linked_file_output(nullptr)` is false.
        (true, false, true) => Err(
            "Render output disabled in Output properties and no active compositing File Output \
             nodes",
        ),
        _ => Ok(()),
    }
}

fn plan(cx: &mut OpCtx, p: &render::Render) -> Result<Plan, OpError> {
    let cwd = cx.services().paths().cwd.clone();
    let c = cx.ctx();
    let scene = c.scene();
    let r = &scene.render;
    if p.use_sequencer_scene.value {
        return Err(OpError::NotImplemented(
            "frap: render.render(use_sequencer_scene=True) is not supported yet".into(),
        ));
    }
    // `render_operator_has_custom_frame_range`: set, whatever the value.
    if !p.animation.value && (p.frame_start.is_set || p.frame_end.is_set) {
        return Err(OpError::Runtime(
            "Frame start/end specified in a non-animation render".into(),
        ));
    }
    let other_scene = c
        .main
        .scenes
        .iter()
        .any(|(_, s)| s.header.name == p.scene.value && s.header.name != scene.header.name);
    if other_scene {
        return Err(OpError::NotImplemented(format!(
            "frap: render.render(scene='{}') is not supported yet: only the context scene \
             ('{}') renders",
            p.scene.value, scene.header.name
        )));
    }
    let view_layer = &c.view_layer().name;
    if scene
        .view_layers
        .iter()
        .any(|vl| vl.name == p.layer.value && vl.name != *view_layer)
    {
        return Err(OpError::NotImplemented(format!(
            "frap: render.render(layer='{}') is not supported yet: only the context view \
             layer ('{view_layer}') renders",
            p.layer.value
        )));
    }
    let frames: Vec<Option<i32>> = if p.animation.value {
        let (start, end) = helpers::frame_range_or_scene(cx, &p.frame_start, &p.frame_end);
        if start > end {
            return Err(OpError::Runtime(
                "Start frame is larger than end frame".into(),
            ));
        }
        animation_frames(start, end, scene.frame_step)
            .into_iter()
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    if p.write_still.value && !p.animation.value && r.image_settings.media_type == "VIDEO" {
        return Err(OpError::Runtime(
            "Cannot write a single file with an animation format selected".into(),
        ));
    }
    save_output_allowed(p.animation.value, r.save_output, r.use_compositing)
        .map_err(|m| OpError::Runtime(m.into()))?;
    let resolution = frap_render_scene::build::camera::render_resolution(r);
    if resolution[0] < 1 || resolution[1] < 1 {
        return Err(OpError::Runtime("Image too small".into()));
    }
    // The camera of each frame: the timeline markers' camera at that frame
    // (`BKE_scene_camera_switch_update` before the frame renders, WP 7.6), else `scene.camera`
    // (verified on 5.2.4: with `scene.camera = None` a marker camera still renders).
    let frame_camera = |f: Option<i32>| {
        let at = f.unwrap_or(scene.frame_current);
        frap_scene::id::timeline_marker::camera_switch_find(c.main, scene, at)
            .or(scene.camera)
            .and_then(|r| c.main.objects.get(r))
    };
    if frames.iter().any(|f| frame_camera(*f).is_none()) {
        return Err(OpError::Runtime("Cannot render, no camera".into()));
    }

    let writes = p.animation.value || p.write_still.value;
    let out = frame_output(scene, view_layer, writes)?;

    let mut outputs = Vec::with_capacity(frames.len());
    if writes {
        let blend_name = c.main.filepath.as_deref().map(|f| {
            std::path::Path::new(f)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        for f in &frames {
            // `{camera_name}` is the frame's camera (the path is made after the switch).
            let vars = TemplateVars::render(
                blend_name.as_deref(),
                &scene.header.name,
                frame_camera(*f).map(|o| o.header.name.as_str()),
                resolution,
                r.fps,
                r.fps_base,
            );
            let rules = OutputRules {
                filepath: &r.filepath,
                vars: &vars,
                blend_file: c.main.filepath.as_deref(),
                use_file_extension: r.use_file_extension,
                file_format: &r.image_settings.file_format,
            };
            let o = output::output_path(&rules, &cwd, *f).map_err(OpError::Runtime)?;
            outputs.push(Some(o));
        }
    } else {
        outputs.resize(frames.len(), None);
    }

    let anim_files = if p.animation.value {
        AnimFiles {
            skip_existing: !r.use_overwrite,
            placeholder: r.use_placeholder,
        }
    } else {
        AnimFiles::default()
    };
    Ok(Plan {
        engine: r.engine.to_string(),
        resolution,
        frames,
        outputs,
        out,
        view_layer: view_layer.to_string(),
        anim_files,
    })
}

/// `touch_file` (`render/intern/pipeline.cc`): creates an empty file (and its directories)
/// unless one exists. Failures are only logged by Blender, so they are ignored here.
fn touch_file(path: &Path) {
    if path.exists() {
        return;
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
        && std::fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path);
}

/// After a failed frame, `RE_RenderAnim` removes the placeholder when it is still empty
/// (`BLI_file_size(filepath) == 0`).
fn remove_empty_placeholder(path: &Path) {
    if std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == 0) {
        let _ = std::fs::remove_file(path);
    }
}

/// A file error as Blender reports it: the path as computed (`OutputPath::path`, relative
/// stays relative), not the absolute path frap wrote to.
fn printed_path_error(msg: String, out: &OutputPath) -> String {
    let abs = out.absolute.display().to_string();
    if abs == out.path {
        msg
    } else {
        msg.replace(&abs, &out.path)
    }
}

/// Renders one frame and writes its file (when it has one). `warnings` collects the colour
/// management fallbacks of the frame (once each).
fn render_and_write(
    cx: &mut OpCtx,
    plan: &Plan,
    frame: Option<i32>,
    out: Option<&OutputPath>,
    engine: &str,
    warnings: &mut Vec<String>,
) -> Result<Film, OpError> {
    let (film, fo) = render_frame(cx, plan, frame, engine)?;
    let fo = fo.as_ref().unwrap_or(&plan.out);
    for w in &fo.cm.warnings {
        if !warnings.contains(w) {
            warnings.push(w.clone());
        }
    }
    if let (Some(out), Some(ws)) = (out, &fo.write) {
        // A path without a folder (`filepath='st_'`): `BLI_file_ensure_parent_dir_exists`
        // cannot make the folder `''` and the image writers give up (`ENOENT`); the OpenEXR
        // writer ignores that and opens the file in the working directory (verified on 5.2.4:
        // `Render error (No such file or directory) cannot save: 'st_0001.png'`, `e_.exr`
        // written).
        let folderless = Path::new(&out.path)
            .parent()
            .is_none_or(|p| p.as_os_str().is_empty());
        if folderless && !matches!(ws.encode, frap_imbuf::EncodeFormat::OpenExr { .. }) {
            let e = std::io::Error::from_raw_os_error(2);
            return Err(OpError::Runtime(format!(
                "Render error ({}) cannot save: '{}'",
                frap_rna::error::strerror(&e),
                out.path
            )));
        }
        format::write_render(
            &out.absolute,
            &film.rgba,
            film.width,
            film.height,
            ws,
            &fo.cm,
            fo.dither,
        )
        .map_err(|e| OpError::Runtime(printed_path_error(e, out)))?;
    }
    Ok(film)
}

/// Renders one frame on a fresh clone of the state; an animation frame also answers its
/// [`FrameOutput`] (read from the frame's scene).
fn render_frame(
    cx: &mut OpCtx,
    plan: &Plan,
    frame: Option<i32>,
    engine: &str,
) -> Result<(Film, Option<FrameOutput>), OpError> {
    let snap = cx.snapshot_at(frame)?;
    let fo = match frame {
        Some(_) => Some(frame_output(
            snap.host.ctx().scene(),
            &plan.view_layer,
            plan.out.write.is_some(),
        )?),
        None => None,
    };
    let req = RenderRequest {
        frame,
        ..RenderRequest::default()
    };
    let mut eval = snap.eval;
    let mut scene =
        frap_render_scene::build(&*snap.host, &mut *eval, &req).map_err(render_error)?;
    crate::report_limit_warnings(cx, &*snap.host, &mut *eval, &req);
    // The engine the service renders with (§19.3; the fallback WARNING is reported once).
    scene.settings.engine = engine.into();
    let film = cx
        .services()
        .render()?
        .render(&scene)
        .map_err(render_error)?;
    if film.rgba.len() != film.width as usize * film.height as usize {
        return Err(OpError::Gpu(format!(
            "the renderer returned {} pixels for {}x{}",
            film.rgba.len(),
            film.width,
            film.height
        )));
    }
    Ok((film, fo))
}

/// `BKE_image_ensure_viewer(bmain, IMA_TYPE_R_RESULT, "Render Result")`: the existing render
/// result image, or a new one.
fn ensure_render_result(cx: &mut OpCtx) -> IdRef<ImageId> {
    let (main, log) = cx.main_mut();
    let existing = main
        .images
        .iter()
        .find(|(_, im)| {
            im.source == "VIEWER" && im.header.name == RENDER_RESULT && im.header.library.is_none()
        })
        .map(|(r, _)| r);
    match existing {
        Some(r) => {
            // A new generation for the new pixels (the image cache is keyed by it).
            let _ = main.images.get_mut(r, log);
            r
        }
        None => {
            let mut im = Image::new(RENDER_RESULT, 0, 0);
            im.source = "VIEWER".into();
            // Blender's values for the factory `Render Result` (5.2.4).
            im.file_format = "TARGA".into();
            im.generated_type = "UV_GRID".into();
            im.generated_width = 1024;
            im.generated_height = 1024;
            im.generated_color = [0.0; 4];
            im.colorspace_settings.name = "".into();
            im.size = [0, 0];
            main.images.insert(RENDER_RESULT, im, log)
        }
    }
}

fn store_render_result(cx: &mut OpCtx, film: Film) {
    let r = ensure_render_result(cx);
    let generation = cx.main().images.get(r).map_or(0, |im| im.header.generation);
    let pixels = DecodedImage {
        width: film.width,
        height: film.height,
        channels: 4,
        pixels: PixelData::F32(film.rgba.into_iter().flatten().collect()),
    };
    let cache = cx.services().images();
    cache.evict(r.key());
    cache.put(r, generation, Arc::new(pixels));
}

#[operator(
    poll = poll::ALWAYS,
    tier = T4,
    spec = render::OPSPEC_RENDER_OT_RENDER,
    support = Partial(
        "CYCLES renders with BLENDER_EEVEE (WARNING); scene=/layer= other than the context \
         ones and use_sequencer_scene=True are not supported"
    ),
    notes = [
        "needs a GPU: GPU_UNAVAILABLE without an adapter or in a build without the gpu feature",
        "the scene is rendered from a copy: nothing changes except bpy.data.images['Render Result'] and scene.camera, which follows the timeline markers as in Blender (each frame renders with the markers' camera of that frame)",
        "animation=True renders frame_start..frame_end by scene.frame_step (frame_step=0: frame_start only), each frame from its own copy with the animation of that frame; frame_current is unchanged afterwards; render.use_overwrite/use_placeholder apply as in Blender",
        "output paths follow Blender 5.2: templates ({scene_name}, {resolution_x:####}, ...), // relative to the saved .frap (else the cwd), relative paths against the cwd, # runs for animation frames, use_file_extension",
        "a path without a directory part ('shot.png') is written to the cwd (Blender 5.2.4 fails to save it)",
        "file formats: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA, BMP"
    ]
)]
fn exec(cx: &mut OpCtx, p: render::Render) -> OpResult {
    run(cx, &p, None)
}

/// The body of `render.render`; `forced_engine` renders with that engine whatever
/// `scene.render.engine` says, without the fallback WARNING (`render.opengl`, WP 8.3.6).
pub(crate) fn run(
    cx: &mut OpCtx,
    p: &render::Render,
    forced_engine: Option<&'static str>,
) -> OpResult {
    let plan = plan(cx, p)?;
    let (engine, warning) = match forced_engine {
        Some(e) => (e, None),
        None => {
            let eevee = cx.services().render()?.supports_engine(EEVEE);
            (
                effective_engine(&plan.engine, eevee),
                engine_fallback(&plan.engine, eevee),
            )
        }
    };
    let mut files: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut rendered = 0usize;
    let mut last: Option<Film> = None;
    let mut cm_warnings: Vec<String> = Vec::new();
    for (frame, out) in plan.frames.iter().zip(&plan.outputs) {
        if let Some(out) = out {
            if plan.anim_files.skip_existing && out.absolute.exists() {
                skipped.push(Value::Str(out.path.as_str().into()));
                continue;
            }
            if plan.anim_files.placeholder {
                touch_file(&out.absolute);
            }
        }
        // `render_update_depsgraph` / `do_render_full_pipeline`: the scene camera follows the
        // timeline markers at the frame, on the original scene, before the clone is made.
        {
            let scene = cx.ctx().win.scene;
            let (main, log) = cx.main_mut();
            frap_scene::id::timeline_marker::camera_switch_update_at(main, log, scene, *frame);
        }
        // One COW clone per frame (`render_frame`), dropped before the next frame.
        let film = match render_and_write(cx, &plan, *frame, out.as_ref(), engine, &mut cm_warnings)
        {
            Ok(film) => film,
            Err(e) => {
                if let Some(out) = out.as_ref().filter(|_| plan.anim_files.placeholder) {
                    remove_empty_placeholder(&out.absolute);
                }
                return Err(e);
            }
        };
        rendered += 1;
        if let Some(out) = out {
            files.push(Value::Str(out.path.as_str().into()));
        }
        last = Some(film);
    }
    // `screen_render_exec` ends with `ED_update_for_newframe` at the restored frame: the scene
    // camera is the markers' camera of `frame_current` (verified on 5.2.4: after rendering
    // frames 1..6 with markers binding `Camera.001` at 3 and `Camera` at 5, the scene camera is
    // `Camera.001`, the left-most marker's, for frame 1).
    {
        let scene = cx.ctx().win.scene;
        let (main, log) = cx.main_mut();
        frap_scene::id::timeline_marker::camera_switch_update(main, log, scene);
    }
    let resolution = match last {
        Some(film) => {
            let r = (film.width, film.height);
            store_render_result(cx, film);
            r
        }
        None => {
            // Every frame was skipped: `RE_InitState` left an empty render result.
            let r = ensure_render_result(cx);
            cx.services().images().evict(r.key());
            cx.report(
                ReportType::Info,
                "No frames rendered, skipped to not overwrite",
            );
            (plan.resolution[0] as u32, plan.resolution[1] as u32)
        }
    };

    if let Some(w) = warning {
        cx.report(ReportType::Warning, w);
    }
    if engine == EEVEE {
        let main = cx.main();
        let world = cx.ctx().scene().world.and_then(|w| main.worlds.get(w));
        for w in frap_render_scene::build::world::warnings(world) {
            cx.report(ReportType::Warning, w);
        }
    }
    if rendered == 0 {
        cm_warnings.clone_from(&plan.out.cm.warnings);
    }
    for w in cm_warnings {
        cx.report(ReportType::Warning, w);
    }
    // Workbench effects frap does not render yet, as `frap.render_contact_sheet` reports them
    // (WP 2.3), whenever Workbench renders (M2 leftover, M4 W0).
    if engine == WORKBENCH {
        let shading = frap_render_scene::build::workbench::shading(cx.ctx().scene());
        for effect in super::frap__render_contact_sheet::workbench_left_out(&shading) {
            cx.report(
                ReportType::Warning,
                format!("frap: the Workbench {effect} is not rendered yet"),
            );
        }
        let color_type = cx.ctx().scene().display.shading.color_type.clone();
        if let Some(w) = super::frap__render_contact_sheet::color_type_left_out(
            &color_type,
            &shading.shading_type,
        ) {
            cx.report(ReportType::Warning, w);
        }
    }
    cx.result("engine", Value::Str(engine.into()));
    cx.result(
        "resolution",
        Value::Tuple(vec![
            Value::Int(i64::from(resolution.0)),
            Value::Int(i64::from(resolution.1)),
        ]),
    );
    cx.result("image", Value::Str(RENDER_RESULT.into()));
    cx.result("frames", Value::Int(rendered as i64));
    if let Some(Value::Str(f)) = files.last() {
        cx.result("filepath", Value::Str(f.clone()));
    }
    if !files.is_empty() {
        cx.result("files", Value::Tuple(files));
    }
    if !skipped.is_empty() {
        cx.result("skipped", Value::Tuple(skipped));
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn frames_walk_by_step() {
        assert_eq!(animation_frames(1, 5, 2), [1, 3, 5]);
        assert_eq!(animation_frames(1, 20, 7), [1, 8, 15]);
        assert_eq!(animation_frames(-2, 1, 1), [-2, -1, 0, 1]);
        assert_eq!(animation_frames(3, 3, 9), [3]);
        assert_eq!(animation_frames(4, 5, 2), [4]);
        // `frame_step = 0` (Blender 5.2.4 writes only the first frame).
        assert_eq!(animation_frames(1, 4, 0), [1]);
        assert_eq!(animation_frames(5, 5, 0), [5]);
        assert_eq!(
            animation_frames(i32::MAX - 1, i32::MAX, 1),
            [i32::MAX - 1, i32::MAX]
        );
    }

    #[test]
    fn save_output_checks() {
        assert_eq!(save_output_allowed(false, false, false), Ok(()));
        assert_eq!(save_output_allowed(true, true, false), Ok(()));
        assert_eq!(
            save_output_allowed(true, false, false),
            Err("Render output and compositing disabled in Output properties")
        );
        assert_eq!(
            save_output_allowed(true, false, true),
            Err(
                "Render output disabled in Output properties and no active compositing File \
                 Output nodes"
            )
        );
    }

    #[test]
    fn printed_paths_in_file_errors() {
        let out = OutputPath {
            path: "out/w_0001.webp".into(),
            absolute: PathBuf::from("/work/out/w_0001.webp"),
        };
        let abs = out.absolute.display().to_string();
        assert_eq!(
            printed_path_error(format!("Render error (x) cannot save: '{abs}'"), &out),
            "Render error (x) cannot save: 'out/w_0001.webp'"
        );
    }

    #[test]
    fn placeholders() {
        let dir = std::env::temp_dir().join(format!("frap_render_touch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("a/b/c_0001.png");
        touch_file(&f);
        assert_eq!(std::fs::metadata(&f).unwrap().len(), 0);
        remove_empty_placeholder(&f);
        assert!(!f.exists());
        // A file with content is kept, and touching it leaves the content.
        std::fs::write(&f, b"png").unwrap();
        touch_file(&f);
        remove_empty_placeholder(&f);
        assert_eq!(std::fs::read(&f).unwrap(), b"png");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
