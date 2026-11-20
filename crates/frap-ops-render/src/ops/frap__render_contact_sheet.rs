// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.render_contact_sheet(filepath='//sheet.png', views={'FRONT','RIGHT','TOP','ISO'},
//! engine='BLENDER_WORKBENCH', objects='VISIBLE', view_size=512, columns=0, margin=0.05,
//! show_labels=True, background=(0.2, 0.2, 0.2, 1.0), use_scene_camera=False, frame=<unset>)`:
//! frap's multi-view sheet (ARCHITECTURE §19.6; WP 2.3). A frap extension with no Blender
//! counterpart; every tile is a real render of the scene (the `RenderScene` pipeline of
//! `render.render`) seen from a synthetic camera.
//!
//! - **Views** (tiles in the item order FRONT, BACK, RIGHT, LEFT, TOP, BOTTOM, ISO, CAMERA;
//!   `use_scene_camera` adds CAMERA): Blender's numpad orientations (`view3d_quat_axis`,
//!   `editors/space_view3d/view3d_utils.cc`, roll 0: FRONT looks along +Y, RIGHT along −X, TOP
//!   along −Z, BACK/LEFT/BOTTOM the opposite ways, +Z up in the side views), ISO the rotation of
//!   the factory viewport (the rotation `align='VIEW'` gives, from `viewquat`: no roll), CAMERA `scene.camera` at the scene's aspect ratio, letterboxed in its square cell.
//! - **Framing**: one orthographic camera per view, looking at the centre of the evaluated
//!   world-space bounds of the drawn objects (a 2 m cube around the origin without any);
//!   `ortho_scale` is the bounds' larger extent across the view × (1 + 2 × `margin`); the clip
//!   range spans the bounds' depth with room on both sides. Synthetic cameras exist only in the
//!   render scene, never in `bpy.data`; each tile reports its camera (`location`,
//!   `rotation_euler` XYZ, `ortho_scale`, `clip_start`, `clip_end`) so that an orthographic
//!   camera object with those values renders the same tile.
//! - **Objects**: `VISIBLE` (`context.visible_objects`), `SELECTED`
//!   (`context.selected_objects`) or `ALL` (everything a render sees); only render-visible
//!   meshes are drawn; lights and the world still light the scene.
//! - **Engine**: Workbench or `BLENDER_EEVEE` (M5); a render service without EEVEE renders
//!   `BLENDER_EEVEE` tiles with Workbench and a WARNING.
//! - **Frame**: unset (or `None`) renders the current frame; set, the state is cloned with
//!   animation applied at that frame, as `render.render` does. The session is never modified
//!   (not even `Render Result`). The sheet is a still: its path never gets a frame number.
//! - **Sheet**: `columns` cells per row (0: `ceil(sqrt(tiles))`), square `view_size` cells, no
//!   gutter, `background` (display space) wherever no tile covers. Each tile is colour-managed
//!   and dithered on its own (the scene's view settings, `render.dither_intensity`), so it holds
//!   the pixels of an 8-bit render of its view; labels (view names, `show_labels`) are drawn on
//!   top. The sheet is an 8-bit RGBA PNG at `filepath` (`//` = next to the saved `.frap`, else
//!   the working directory; `.png` added when missing; path templates as `render.filepath`; a
//!   path without a file name, `''` or `'//'`, is an error). At most [`MAX_SHEET_PIXELS`]
//!   pixels: larger sheets are refused before anything renders, and tiles are rendered,
//!   quantised and dropped one at a time.
//!
//! `result`: `filepath`, `size` (`[width, height]`), `engine`, `tiles`
//! (`[{"view", "rect": [x, y, w, h], …camera}]`, rectangles top-left based).

use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_math::EulerOrder;
use frap_math::euler::{eul_o_to_mat3, mat3_normalized_to_eul_o, quat_to_eul_o};
use frap_math::matrix::loc_eul_o_size_to_mat4;
use frap_math::quat::quat_to_mat3;
use frap_op::operator;
use frap_op::prelude::*;
use frap_render_scene::build::camera::{CameraParams, SensorFit, render_camera};
use frap_render_scene::{
    Film, RCamera, RenderError, RenderRequest, RenderScene, SheetLayout, SheetTile,
};
use frap_scene::EvalMode;
use frap_spec_ops_frap::frap::{self, render_contact_sheet as rcs};
use glam::{Mat3, Mat4, Vec3};
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::output::{self, OutputRules, TemplateVars, format};

/// The Workbench engine (also the fallback of a render service without EEVEE).
const WORKBENCH: &str = "BLENDER_WORKBENCH";

/// The largest sheet (pixels) frap composites: 4096 × 4096, one tile at the largest
/// `view_size`. The compositor holds the display tiles and the sheet as float RGBA (16 bytes a
/// pixel each), so this bounds the op at roughly 0.6 GB.
pub(crate) const MAX_SHEET_PIXELS: u64 = 4096 * 4096;

/// The views, in tile order.
const VIEW_ORDER: [&str; 8] = [
    "FRONT", "BACK", "RIGHT", "LEFT", "TOP", "BOTTOM", "ISO", "CAMERA",
];

/// `view3d_quat_axis[view][RV3D_VIEW_AXIS_ROLL_0]` (w, x, y, z): world → view rotations.
fn axis_quat(view: &str) -> Option<[f32; 4]> {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    Some(match view {
        "FRONT" => [h, -h, 0.0, 0.0],
        "BACK" => [0.0, 0.0, -h, -h],
        "LEFT" => [0.5, -0.5, 0.5, 0.5],
        "RIGHT" => [0.5, -0.5, -0.5, -0.5],
        "TOP" => [1.0, 0.0, 0.0, 0.0],
        "BOTTOM" => [0.0, -1.0, 0.0, 0.0],
        _ => return None,
    })
}

/// The XYZ Euler rotation of the factory viewport as `align='VIEW'` gives it
/// (`ED_object_rotation_from_view` over `rv3d->viewquat`; verified on 5.2.4:
/// `(1.108752, 0, 1.154878)`): `region_3d.view_rotation` of `startup.json`, the view → world
/// rotation, so a camera with it sees the viewport without roll. The dumped `view_matrix` is
/// stale against `viewquat` (0.76° of roll), as `frap-ops-mesh`'s `view_rotation` notes.
pub(crate) fn iso_euler() -> [f32; 3] {
    let q = frap_scene::startup::factory_dump()
        .get("view3d")
        .and_then(|v| v.get("region_3d"))
        .and_then(|v| v.get("view_rotation"))
        .and_then(|v| v.as_array())
        .filter(|a| a.len() == 4)
        .map(|a| std::array::from_fn::<f32, 4, _>(|i| a[i].as_f64().unwrap_or(0.0) as f32))
        .unwrap_or([1.0, 0.0, 0.0, 0.0]);
    quat_to_eul_o(q, EulerOrder::XYZ)
}

/// The world → view rotation of a synthetic view. `ISO` takes `iso_euler` ([`iso_euler`]).
pub(crate) fn view_rotation(view: &str, iso_euler: [f32; 3]) -> Mat3 {
    match axis_quat(view) {
        Some(q) => quat_to_mat3(q),
        None => eul_o_to_mat3(iso_euler, EulerOrder::XYZ).transpose(),
    }
}

/// A synthetic orthographic camera: its object transform and parameters.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SyntheticCamera {
    pub location: [f32; 3],
    pub rotation_euler: [f32; 3],
    pub ortho_scale: f32,
    pub clip_start: f32,
    pub clip_end: f32,
}

impl SyntheticCamera {
    /// `matrix_world` of a camera object with these values (`loc_eulO_size_to_mat4`).
    pub fn matrix_world(&self) -> Mat4 {
        loc_eul_o_size_to_mat4(
            Vec3::from(self.location),
            self.rotation_euler,
            EulerOrder::XYZ,
            Vec3::ONE,
        )
    }

    /// `BKE_camera_params_from_object` of an `ORTHO` camera with these values (other camera
    /// data at its defaults; sensor fit AUTO is horizontal on a square tile).
    pub fn params(&self) -> CameraParams {
        CameraParams {
            is_ortho: true,
            ortho_scale: self.ortho_scale,
            clip_start: self.clip_start,
            clip_end: self.clip_end,
            lens: 50.0,
            sensor_fit: SensorFit::Auto,
            ..CameraParams::default()
        }
    }

    /// The render camera of a `size × size` tile.
    pub fn render_camera(&self, size: u32) -> RCamera {
        let s = size.min(i32::MAX as u32) as i32;
        render_camera(&self.params(), &self.matrix_world(), s, s, 1.0, 1.0)
    }
}

/// Frames world-space `points` (`None`: a 2 m cube around the origin) seen with the world →
/// view rotation `rot`.
pub(crate) fn frame_points(points: &[Vec3], rot: &Mat3, margin: f32) -> SyntheticCamera {
    let default = [Vec3::splat(-1.0), Vec3::splat(1.0)];
    let pts: &[Vec3] = if points.is_empty() { &default } else { points };
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in pts {
        let v = *rot * *p;
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let size = hi - lo;
    let mut extent = size.x.max(size.y);
    if extent <= 1e-6 {
        extent = 2.0;
    }
    let depth = size.z.max(0.0);
    let pad = extent.max(depth) * 0.25 + 0.1;
    let centre_view = Vec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z + pad);
    let cam_rot = rot.transpose();
    let location = cam_rot * centre_view;
    let rotation_euler = mat3_normalized_to_eul_o(&cam_rot, EulerOrder::XYZ);
    SyntheticCamera {
        location: location.to_array(),
        rotation_euler,
        ortho_scale: extent * (1.0 + 2.0 * margin.max(0.0)),
        clip_start: pad * 0.5,
        clip_end: pad + depth + pad,
    }
}

/// World-space corners of the bounding boxes of every drawn mesh.
fn scene_points(scene: &RenderScene) -> Vec<Vec3> {
    let mut out = Vec::new();
    for ob in &scene.objects {
        let Some(mesh) = ob.mesh.and_then(|k| scene.meshes.get(&k)) else {
            continue;
        };
        if mesh.positions.is_empty() {
            continue;
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &mesh.positions {
            let p = Vec3::from(*p);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            );
            let w = ob.matrix_world.transform_point3(c);
            if w.is_finite() {
                out.push(w);
            }
        }
    }
    out
}

/// Columns and rows of a sheet of `n` tiles.
pub(crate) fn grid(n: usize, columns: i32) -> (u32, u32) {
    let n = n.max(1) as u32;
    let cols = if columns > 0 {
        (columns as u32).min(n)
    } else {
        (n as f64).sqrt().ceil() as u32
    };
    (cols, n.div_ceil(cols))
}

/// The rectangle of a `w x h` image centred in cell `i` of a `cols`-wide grid of `cell` px.
pub(crate) fn cell_rect(i: usize, cols: u32, cell: u32, w: u32, h: u32) -> [u32; 4] {
    let (cx, cy) = (i as u32 % cols, i as u32 / cols);
    [cx * cell + (cell - w) / 2, cy * cell + (cell - h) / 2, w, h]
}

/// The CAMERA tile size: the scene's render aspect (pixel aspect included) fitted in `cell`.
fn camera_tile(resolution: [i32; 2], aspect: [f32; 2], cell: u32) -> (u32, u32) {
    let w = resolution[0].max(1) as f64 * f64::from(aspect[0].max(1e-6));
    let h = resolution[1].max(1) as f64 * f64::from(aspect[1].max(1e-6));
    let c = f64::from(cell);
    if w >= h {
        (cell, ((c * h / w).round() as u32).clamp(1, cell))
    } else {
        (((c * w / h).round() as u32).clamp(1, cell), cell)
    }
}

/// Workbench effects the scene asks for and frap does not render (P2; `frap-render`'s
/// `workbench::cavity::unsupported`): cavity and depth of field, which X-ray mode disables.
/// `render.render` reports them too (WP 2.3).
pub(crate) fn workbench_left_out(w: &frap_render_scene::RWorkbenchShading) -> Vec<&'static str> {
    let xray = w.shading_type == "WIREFRAME" || (w.show_xray && w.xray_alpha < 1.0);
    let mut out = vec![];
    if !xray && w.show_cavity {
        out.push("cavity");
    }
    if !xray && w.use_dof {
        out.push("depth of field");
    }
    out
}

/// The WARNING for a `shading.color_type` frap renders as `MATERIAL` (deviations.md §2: the
/// Workbench `VERTEX` (Attribute) and `TEXTURE` colours come with M5's texture and colour
/// attribute upload, WP 5.5); `None` for the colour types frap renders, and in wireframe
/// mode, which colours by object.
pub(crate) fn color_type_left_out(color_type: &str, shading_type: &str) -> Option<String> {
    if shading_type == "WIREFRAME" || !matches!(color_type, "VERTEX" | "TEXTURE") {
        return None;
    }
    Some(format!(
        "frap: the Workbench colour type '{color_type}' is not rendered yet; rendered as \
         'MATERIAL' (the material's viewport colour)"
    ))
}

fn render_error(e: RenderError) -> OpError {
    match e {
        RenderError::Unsupported(m) => OpError::Runtime(m),
        other => OpError::Gpu(other.to_string()),
    }
}

fn floats(v: &[f32]) -> Value {
    Value::Tuple(v.iter().map(|f| Value::Float(*f)).collect())
}

/// One rendered tile: its view and camera.
struct TileJob {
    view: &'static str,
    camera: Option<SyntheticCamera>,
    camera_name: Option<SmolStr>,
}

#[operator(
    poll = poll::ALWAYS,
    tier = T4,
    spec = frap::OPSPEC_FRAP_OT_RENDER_CONTACT_SHEET,
    support = Partial("engine='BLENDER_EEVEE' renders its tiles with BLENDER_WORKBENCH (WARNING) when the render service has no EEVEE"),
    notes = [
        "frap extension: one orthographic tile per view (FRONT/BACK/RIGHT/LEFT/TOP/BOTTOM: Blender's numpad views; ISO: the factory viewport rotation; CAMERA: scene.camera), framing the drawn objects' bounds",
        "needs a GPU: GPU_UNAVAILABLE without an adapter or in a build without the gpu feature",
        "nothing in the session changes; synthetic cameras never enter bpy.data, and result.tiles reports each one",
        "tiles follow the item order FRONT, BACK, RIGHT, LEFT, TOP, BOTTOM, ISO, CAMERA, whatever the order of views",
        "each tile has the pixels of an 8-bit render of its view (same colour management and dither); the sheet is an RGBA PNG"
    ]
)]
fn exec(cx: &mut OpCtx, p: frap::RenderContactSheet) -> OpResult {
    // Views, in item order.
    let mut wanted: Vec<&'static str> = VIEW_ORDER
        .iter()
        .copied()
        .filter(|v| p.views.value.contains(*v))
        .collect();
    if p.use_scene_camera.value && !wanted.contains(&"CAMERA") {
        wanted.push("CAMERA");
    }
    if wanted.is_empty() {
        return Err(OpError::Runtime(format!(
            "frap: views is empty: name at least one of {}",
            VIEW_ORDER.join(", ")
        )));
    }
    let cell = p.view_size.value.clamp(16, 4096) as u32;
    let frame = p.frame.is_set.then_some(p.frame.value);
    let (cols, rows) = grid(wanted.len(), p.columns.value);
    let pixels = u64::from(cols * cell) * u64::from(rows * cell);
    if pixels > MAX_SHEET_PIXELS {
        return Err(OpError::Runtime(format!(
            "frap: the contact sheet would be {} x {} pixels ({:.1} Mpx), more than the {:.1} Mpx \
             frap composites: lower view_size or name fewer views",
            cols * cell,
            rows * cell,
            pixels as f64 / 1e6,
            MAX_SHEET_PIXELS as f64 / 1e6
        )));
    }

    // Everything read from the context before rendering.
    let cwd = cx.services().paths().cwd.clone();
    let (filepath, objects, iso, cm, dither, resolution, aspect, camera_ref) = {
        let c = cx.ctx();
        let scene = c.scene();
        let r = &scene.render;
        let camera_ref = scene.camera;
        if wanted.contains(&"CAMERA") && camera_ref.and_then(|r| c.main.objects.get(r)).is_none() {
            return Err(OpError::Runtime("Cannot render, no camera".into()));
        }
        let names = |refs: Vec<frap_rna::IdRef<frap_rna::ObjectId>>| -> Vec<SmolStr> {
            refs.iter()
                .filter_map(|r| c.main.objects.get(*r))
                .map(|o| o.header.name.clone())
                .collect()
        };
        let objects = match p.objects.value {
            rcs::Objects::Visible => Some(names(c.visible_objects())),
            rcs::Objects::Selected => Some(names(c.selected_objects())),
            rcs::Objects::All => None,
        };
        let resolution = frap_render_scene::build::camera::render_resolution(r);
        let blend_name = c.main.filepath.as_deref().map(|f| {
            std::path::Path::new(f)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let camera_name = camera_ref
            .and_then(|r| c.main.objects.get(r))
            .map(|o| o.header.name.to_string());
        let vars = TemplateVars::render(
            blend_name.as_deref(),
            &scene.header.name,
            camera_name.as_deref(),
            resolution,
            r.fps,
            r.fps_base,
        );
        let rules = OutputRules {
            filepath: &p.filepath.value,
            vars: &vars,
            blend_file: c.main.filepath.as_deref(),
            use_file_extension: true,
            file_format: "PNG",
        };
        // A still: no frame number, whatever `frame` is (as `render.render(write_still=True)`).
        let out = output::output_path(&rules, &cwd, None).map_err(OpError::Runtime)?;
        // `''` and `'//'` resolve to the bare extension (`.png`, a hidden file in the cwd).
        let name = out.absolute.file_name().map(|s| s.to_string_lossy());
        if name.is_none_or(|n| n.eq_ignore_ascii_case(".png")) {
            return Err(OpError::Runtime(format!(
                "frap: filepath '{}' names no file: give one, e.g. '//sheet.png'",
                p.filepath.value
            )));
        }
        let cm = format::color_management(&scene.view_settings, &scene.display_settings);
        (
            out,
            objects,
            iso_euler(),
            cm,
            r.dither_intensity,
            resolution,
            [r.pixel_aspect_x, r.pixel_aspect_y],
            camera_ref,
        )
    };

    // Why nothing may be framed (the WARNING below): no object in the chosen set at all.
    let objects_kind = match p.objects.value {
        rcs::Objects::Visible => "VISIBLE",
        rcs::Objects::Selected => "SELECTED",
        rcs::Objects::All => "ALL",
    };
    let none_listed = objects.as_ref().is_some_and(|o| o.is_empty());

    // One build of the (cloned, animation-applied) state; tiles only swap camera and size.
    let snap = cx.snapshot_at(frame)?;
    let mut eval = snap.eval;
    let req = RenderRequest {
        frame,
        resolution: Some([cell, cell]),
        camera: Some(RCamera::default()),
        objects,
    };
    let mut base = frap_render_scene::build(&*snap.host, &mut *eval, &req).map_err(render_error)?;
    let eevee_ok = cx.services().render()?.supports_engine("BLENDER_EEVEE");
    let engine = if p.engine.value == rcs::Engine::BlenderEevee && eevee_ok {
        "BLENDER_EEVEE"
    } else {
        WORKBENCH
    };
    base.settings.engine = engine.into();
    let points = scene_points(&base);

    // One tile at a time: render, colour-manage and quantise, then drop the float film, so
    // at most one render film is alive besides the 8-bit display tiles.
    let service = cx.services().render()?;
    let mut jobs: Vec<TileJob> = Vec::with_capacity(wanted.len());
    let mut tiles = Vec::with_capacity(wanted.len());
    let mut display = Vec::with_capacity(wanted.len());
    for (i, view) in wanted.iter().copied().enumerate() {
        let mut scene = base.clone();
        let job = if view == "CAMERA" {
            let cam_ref = camera_ref.expect("invariant: checked above");
            let host = &*snap.host;
            let ob = host
                .main()
                .objects
                .get(cam_ref)
                .ok_or_else(|| OpError::Runtime("Cannot render, no camera".into()))?;
            let params = frap_render_scene::build::camera_params(host.main(), ob);
            let ev = eval
                .object(host, cam_ref, EvalMode::Render)
                .map_err(|e| OpError::Runtime(e.message))?;
            let (w, h) = camera_tile(resolution, aspect, cell);
            scene.resolution = [w, h];
            scene.camera = render_camera(
                &params,
                &ev.matrix_world,
                w as i32,
                h as i32,
                aspect[0],
                aspect[1],
            );
            TileJob {
                view,
                camera: None,
                camera_name: Some(ob.header.name.clone()),
            }
        } else {
            let rot = view_rotation(view, iso);
            let cam = frame_points(&points, &rot, p.margin.value);
            scene.camera = cam.render_camera(cell);
            TileJob {
                view,
                camera: Some(cam),
                camera_name: None,
            }
        };
        let film = service.render(&scene).map_err(render_error)?;
        drop(scene);
        let d = format::display_image(&film.rgba, film.width, film.height, &cm);
        let bytes = d.to_rgba8(dither);
        drop(d);
        display.push(Film {
            width: film.width,
            height: film.height,
            rgba: bytes
                .chunks_exact(4)
                .map(|c| std::array::from_fn(|k| f32::from(c[k]) / 255.0))
                .collect(),
        });
        let rect = cell_rect(i, cols, cell, film.width, film.height);
        let label = p.show_labels.value.then(|| SmolStr::new(job.view));
        tiles.push(SheetTile::new(rect, label));
        jobs.push(job);
    }
    let bg = p
        .background
        .value
        .map(|v| f32::from(frap_color::display::unit_float_to_uchar_clamp(v)) / 255.0);
    let layout = SheetLayout::new([cols * cell, rows * cell], bg, tiles.clone());
    let sheet = service
        .composite_sheet(&display, &layout)
        .map_err(render_error)?;
    drop(display);
    let bytes: Vec<u8> = sheet
        .rgba
        .iter()
        .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    let img = DecodedImage::new(sheet.width, sheet.height, 4, PixelData::U8(bytes))
        .map_err(|e| OpError::Io(e.to_string()))?;
    let png = frap_imbuf::encode(
        &img,
        &EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        },
    )
    .map_err(|e| OpError::Io(e.to_string()))?;
    output::write_file(&filepath.absolute, &png).map_err(OpError::Runtime)?;

    if p.engine.value == rcs::Engine::BlenderEevee && engine == WORKBENCH {
        cx.report(
            ReportType::Warning,
            format!("Engine 'BLENDER_EEVEE' is not available in this build of frap; rendered with {WORKBENCH}"),
        );
    }
    if points.is_empty() {
        let why = match (none_listed, p.objects.value) {
            (true, rcs::Objects::Selected) => "none selected",
            (true, rcs::Objects::Visible) => "none visible",
            _ => "none of them has mesh geometry",
        };
        cx.report(
            ReportType::Warning,
            format!(
                "frap: no objects to frame (objects='{objects_kind}', {why}); the views frame a 2 m cube around the origin"
            ),
        );
    }
    for w in &cm.warnings {
        cx.report(ReportType::Warning, w.clone());
    }
    for effect in workbench_left_out(&base.settings.workbench) {
        cx.report(
            ReportType::Warning,
            format!("frap: the Workbench {effect} is not rendered yet"),
        );
    }
    if let Some(w) = color_type_left_out(
        &base.settings.shading_color_type,
        &base.settings.workbench.shading_type,
    ) {
        cx.report(ReportType::Warning, w);
    }

    let tile_results = jobs
        .iter()
        .zip(&tiles)
        .map(|(job, tile)| {
            let mut m: IndexMap<SmolStr, Value> = IndexMap::new();
            m.insert("view".into(), Value::Str(job.view.into()));
            m.insert(
                "rect".into(),
                Value::Tuple(
                    tile.rect
                        .iter()
                        .map(|v| Value::Int(i64::from(*v)))
                        .collect(),
                ),
            );
            if let Some(c) = &job.camera {
                m.insert("location".into(), floats(&c.location));
                m.insert("rotation_euler".into(), floats(&c.rotation_euler));
                m.insert("ortho_scale".into(), Value::Float(c.ortho_scale));
                m.insert("clip_start".into(), Value::Float(c.clip_start));
                m.insert("clip_end".into(), Value::Float(c.clip_end));
            }
            if let Some(n) = &job.camera_name {
                m.insert("camera".into(), Value::Str(n.clone()));
            }
            m
        })
        .collect();
    cx.result("filepath", Value::Str(filepath.path.as_str().into()));
    cx.result(
        "size",
        Value::Tuple(vec![
            Value::Int(i64::from(layout.size[0])),
            Value::Int(i64::from(layout.size[1])),
        ]),
    );
    cx.result("engine", Value::Str(engine.into()));
    cx.result("tiles", Value::Structs(tile_results));
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_types_rendered_as_material_are_reported() {
        assert_eq!(color_type_left_out("MATERIAL", "SOLID"), None);
        assert_eq!(color_type_left_out("RANDOM", "SOLID"), None);
        assert_eq!(color_type_left_out("TEXTURE", "WIREFRAME"), None);
        assert_eq!(
            color_type_left_out("TEXTURE", "SOLID").as_deref(),
            Some(
                "frap: the Workbench colour type 'TEXTURE' is not rendered yet; rendered as \
                 'MATERIAL' (the material's viewport colour)"
            )
        );
        assert!(color_type_left_out("VERTEX", "SOLID").is_some());
    }

    fn look_dir(rot: &Mat3) -> Vec3 {
        // The camera looks along its local -Z.
        rot.transpose() * Vec3::new(0.0, 0.0, -1.0)
    }

    #[test]
    fn numpad_views_look_the_right_way() {
        let id = [0.0; 3];
        let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-6;
        assert!(near(look_dir(&view_rotation("FRONT", id)), Vec3::Y));
        assert!(near(look_dir(&view_rotation("BACK", id)), -Vec3::Y));
        assert!(near(look_dir(&view_rotation("RIGHT", id)), -Vec3::X));
        assert!(near(look_dir(&view_rotation("LEFT", id)), Vec3::X));
        assert!(near(look_dir(&view_rotation("TOP", id)), -Vec3::Z));
        assert!(near(look_dir(&view_rotation("BOTTOM", id)), Vec3::Z));
        // Side views keep +Z up.
        for v in ["FRONT", "BACK", "RIGHT", "LEFT"] {
            let up = view_rotation(v, id).transpose() * Vec3::Y;
            assert!(near(up, Vec3::Z), "{v}: {up}");
        }
    }

    #[test]
    fn framing_centres_and_scales_the_bounds() {
        let pts = [Vec3::new(-1.0, -2.0, 0.0), Vec3::new(3.0, 2.0, 1.0)];
        let rot = view_rotation("TOP", [0.0; 3]);
        let cam = frame_points(&pts, &rot, 0.05);
        // Seen from the top: x extent 4, y extent 4 → 4 × 1.1.
        assert!((cam.ortho_scale - 4.4).abs() < 1e-5, "{cam:?}");
        assert!((cam.location[0] - 1.0).abs() < 1e-6 && cam.location[1].abs() < 1e-6);
        assert!(cam.location[2] > 1.0);
        assert!(cam.clip_start > 0.0 && cam.clip_end > cam.location[2] - 0.0);
        assert_eq!(cam.rotation_euler, [0.0, 0.0, 0.0]);
        // The camera matrix maps the bounds' centre onto its view axis.
        let m = cam.matrix_world();
        let local = m.inverse().transform_point3(Vec3::new(1.0, 0.0, 0.5));
        assert!(
            local.x.abs() < 1e-5 && local.y.abs() < 1e-5 && local.z < 0.0,
            "{local}"
        );
        // No objects: the 2 m cube.
        let empty = frame_points(&[], &rot, 0.0);
        assert!((empty.ortho_scale - 2.0).abs() < 1e-6);
    }

    #[test]
    fn iso_tile_camera_has_the_view_rotation() {
        // The factory viewport rotation `align='VIEW'` gives (verified on 5.2.4:
        // `camera_add(align='VIEW').rotation_euler == (1.108752, 0, 1.154878)`): the ISO tile
        // camera has exactly that rotation, so no roll.
        let iso = [1.108_752, 0.0, 1.154_878];
        for (a, b) in iso_euler().iter().zip(iso) {
            assert!((a - b).abs() < 1e-5, "{:?}", iso_euler());
        }
        let cam = frame_points(&[], &view_rotation("ISO", iso), 0.05);
        for (a, b) in cam.rotation_euler.iter().zip(iso) {
            assert!((a - b).abs() < 1e-5, "{:?}", cam.rotation_euler);
        }
    }

    #[test]
    fn grid_and_cells() {
        assert_eq!(grid(4, 0), (2, 2));
        assert_eq!(grid(3, 0), (2, 2));
        assert_eq!(grid(5, 0), (3, 2));
        assert_eq!(grid(8, 0), (3, 3));
        assert_eq!(grid(3, 1), (1, 3));
        assert_eq!(grid(3, 16), (3, 1));
        assert_eq!(cell_rect(3, 2, 100, 100, 100), [100, 100, 100, 100]);
        assert_eq!(cell_rect(1, 2, 100, 100, 56), [100, 22, 100, 56]);
        assert_eq!(camera_tile([1920, 1080], [1.0, 1.0], 512), (512, 288));
        assert_eq!(camera_tile([1080, 1920], [1.0, 1.0], 512), (288, 512));
        assert_eq!(camera_tile([100, 100], [2.0, 1.0], 64), (64, 32));
    }
}
