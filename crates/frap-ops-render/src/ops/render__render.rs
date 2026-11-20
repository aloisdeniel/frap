// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.render.render(animation=False, write_still=False, use_viewport=False,
//! use_sequencer_scene=False, layer='', scene='', frame_start=0, frame_end=0)`, tier T4
//! (ARCHITECTURE §19.4; WP 1.24). Behaviour of `screen_render_exec` / `RE_RenderFrame` /
//! `RE_RenderAnim` (`editors/render/render_internal.cc`, `render/intern/pipeline.cc`, Blender
//! 5.2.4, GPL-2.0-or-later), verified on 5.2.4:
//!
//! 1. Checks, in Blender's order (each a `RuntimeError: Error: …`, none needs a GPU):
//!    `write_still` with a video `media_type`: `Cannot write a single file with an animation
//!    format selected`; `animation=True` with `frame_start > frame_end` (the operator's
//!    `frame_start`/`frame_end` override the scene's only when set): `Start frame is larger
//!    than end frame`; a resolution that rounds to zero: `Image too small`; no `scene.camera`:
//!    `Cannot render, no camera`; then the output path templates (`Parse errors in path …`).
//!    `scene=`/`layer=` naming no scene/view layer are ignored, as in Blender; naming another
//!    one is `NotImplementedError` (frap renders the context scene and view layer).
//! 2. The state is cloned with animation applied at the frame (`OpCtx::snapshot_at`, §7.3), the
//!    `RenderScene` is built from the clone and rendered through `cx.services().render()` (the
//!    only way to the GPU: `GPU_UNAVAILABLE` when there is no adapter or no `gpu` feature).
//!    Nothing of the session changes except the `Render Result` image (D-07).
//! 3. `BLENDER_EEVEE` and `CYCLES` render with Workbench in M1, with a WARNING.
//! 4. The film (scene-linear) becomes `bpy.data.images['Render Result']` (created when missing
//!    with Blender's values: `source='VIEWER'`, `type='RENDER_RESULT'`, `size` `(0, 0)`
//!    headless); its pixels go to the session's image cache. `write_still` writes
//!    `scene.render.filepath` following [`crate::output`]'s rules, colour-managed (§19.5).
//! 5. `animation=True` renders `frame_start..=frame_end` step `frame_step`, each frame on a
//!    fresh clone, and always writes. `frame_current` is unchanged afterwards (verified).
//!
//! Output paths are computed before anything is rendered, so a template error fails without a
//! render. Files are written before `Render Result` is updated: a failed write leaves the
//! session untouched (the files of earlier frames stay, as in Blender).
//!
//! `result`: `engine` (the engine that rendered), `resolution`, `image` (`"Render Result"`),
//! `frames`, and with files `filepath` (the last file) and `files` (every file, as Blender
//! prints them: relative to the cwd unless absolute or `//`).

use std::sync::Arc;

use frap_imbuf::{DecodedImage, PixelData};
use frap_op::operator;
use frap_op::prelude::*;
use frap_render_scene::{Film, RenderError, RenderRequest};
use frap_rna::{IdRef, ImageId};
use frap_scene::id::Image;
use frap_spec_ops_render::render;

use crate::output::{self, OutputPath, OutputRules, TemplateVars, format};

/// The engine every M1 render uses.
const WORKBENCH: &str = "BLENDER_WORKBENCH";
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

/// The WARNING for an engine frap renders with another one (deviations.md D-12; §2 until M5).
fn engine_fallback(engine: &str, eevee: bool) -> Option<String> {
    let used = effective_engine(engine, eevee);
    match engine {
        WORKBENCH => None,
        EEVEE if eevee => None,
        "CYCLES" => Some(format!(
            "Engine 'CYCLES' is not available in frap; rendered with {used}"
        )),
        other => Some(format!(
            "Engine '{other}' is not available in frap yet (M5); rendered with {used}"
        )),
    }
}

/// Everything read from the context before rendering.
struct Plan {
    engine: String,
    frames: Vec<Option<i32>>,
    /// Output file per frame (`None`: nothing is written).
    outputs: Vec<Option<OutputPath>>,
    write: Option<format::WriteSettings>,
    cm: format::ColorManagement,
    dither: f32,
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
    if p.write_still.value && !p.animation.value && r.image_settings.media_type == "VIDEO" {
        return Err(OpError::Runtime(
            "Cannot write a single file with an animation format selected".into(),
        ));
    }

    let frames: Vec<Option<i32>> = if p.animation.value {
        let (start, end) = helpers::frame_range_or_scene(cx, &p.frame_start, &p.frame_end);
        if start > end {
            return Err(OpError::Runtime(
                "Start frame is larger than end frame".into(),
            ));
        }
        let step = scene.frame_step.max(1) as usize;
        (i64::from(start)..=i64::from(end))
            .step_by(step)
            .map(|f| Some(f as i32))
            .collect()
    } else {
        vec![None]
    };
    let resolution = frap_render_scene::build::camera::render_resolution(r);
    if resolution[0] < 1 || resolution[1] < 1 {
        return Err(OpError::Runtime("Image too small".into()));
    }
    let camera = scene
        .camera
        .and_then(|r| c.main.objects.get(r))
        .ok_or_else(|| OpError::Runtime("Cannot render, no camera".into()))?;

    let writes = p.animation.value || p.write_still.value;
    let write = if writes {
        Some(format::render_write_settings(r).map_err(OpError::NotImplemented)?)
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

    let mut outputs = Vec::with_capacity(frames.len());
    if writes {
        let blend_name = c.main.filepath.as_deref().map(|f| {
            std::path::Path::new(f)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let vars = TemplateVars::render(
            blend_name.as_deref(),
            &scene.header.name,
            Some(camera.header.name.as_str()),
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
        for f in &frames {
            let o = output::output_path(&rules, &cwd, *f).map_err(OpError::Runtime)?;
            outputs.push(Some(o));
        }
    } else {
        outputs.resize(frames.len(), None);
    }

    Ok(Plan {
        engine: r.engine.to_string(),
        frames,
        outputs,
        write,
        cm,
        dither: r.dither_intensity,
    })
}

/// Renders one frame on a fresh clone of the state.
fn render_frame(cx: &mut OpCtx, frame: Option<i32>, engine: &str) -> Result<Film, OpError> {
    let snap = cx.snapshot_at(frame)?;
    let req = RenderRequest {
        frame,
        ..RenderRequest::default()
    };
    let mut eval = snap.eval;
    let mut scene =
        frap_render_scene::build(&*snap.host, &mut *eval, &req).map_err(render_error)?;
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
    Ok(film)
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
        "BLENDER_EEVEE and CYCLES render with BLENDER_WORKBENCH (WARNING) until M5; \
         animation=True renders every frame of the range but animation is evaluated from M6; \
         scene=/layer= other than the context ones and use_sequencer_scene=True are not \
         supported"
    ),
    notes = [
        "needs a GPU: GPU_UNAVAILABLE without an adapter or in a build without the gpu feature",
        "the scene is rendered from a copy: nothing changes except bpy.data.images['Render Result']",
        "output paths follow Blender 5.2: templates ({scene_name}, {resolution_x:####}, ...), // relative to the saved .frap (else the cwd), relative paths against the cwd, # runs for animation frames, use_file_extension",
        "a path without a directory part ('shot.png') is written to the cwd (Blender 5.2.4 fails to save it)",
        "file formats: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA, BMP"
    ]
)]
fn exec(cx: &mut OpCtx, p: render::Render) -> OpResult {
    let plan = plan(cx, &p)?;
    let eevee = cx.services().render()?.supports_engine(EEVEE);
    let engine = effective_engine(&plan.engine, eevee);
    let warning = engine_fallback(&plan.engine, eevee);
    let mut files: Vec<Value> = Vec::new();
    let mut last: Option<Film> = None;
    for (frame, out) in plan.frames.iter().zip(&plan.outputs) {
        let film = render_frame(cx, *frame, engine)?;
        if let (Some(out), Some(ws)) = (out, &plan.write) {
            format::write_render(
                &out.absolute,
                &film.rgba,
                film.width,
                film.height,
                ws,
                &plan.cm,
                plan.dither,
            )
            .map_err(OpError::Runtime)?;
            files.push(Value::Str(out.path.as_str().into()));
        }
        last = Some(film);
    }
    let film = last.expect("invariant: at least one frame is rendered");
    let resolution = (film.width, film.height);
    store_render_result(cx, film);

    if let Some(w) = warning {
        cx.report(ReportType::Warning, w);
    }
    for w in &plan.cm.warnings {
        cx.report(ReportType::Warning, w.clone());
    }
    // Workbench effects frap does not render yet, as `frap.render_contact_sheet` reports them
    // (WP 2.3), whenever Workbench renders (also as the fallback of EEVEE and CYCLES until M5;
    // M2 leftover, M4 W0).
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
    cx.result("engine", Value::Str(WORKBENCH.into()));
    cx.result(
        "resolution",
        Value::Tuple(vec![
            Value::Int(i64::from(resolution.0)),
            Value::Int(i64::from(resolution.1)),
        ]),
    );
    cx.result("image", Value::Str(RENDER_RESULT.into()));
    cx.result("frames", Value::Int(plan.frames.len() as i64));
    if let Some(Value::Str(f)) = files.last() {
        cx.result("filepath", Value::Str(f.clone()));
    }
    if !files.is_empty() {
        cx.result("files", Value::Tuple(files));
    }
    Ok(OpStatus::Finished)
}
