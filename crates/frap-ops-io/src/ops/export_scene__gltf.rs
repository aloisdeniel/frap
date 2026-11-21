// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.export_scene.gltf` (ARCHITECTURE §17.2): the operator around
//! `frap_io_gltf::export`. All of Blender's props are accepted; the ones the M1 exporter does
//! not honour (`frap_io_gltf::export::is_supported` says which it does) give one WARNING
//! naming the prop when they differ from Blender's default, and the export proceeds as if they
//! had their default.
//!
//! As in the add-on (io_scene_gltf2 5.2.41, `ExportGLTF2.execute` → `check`): the file
//! extension is forced to the format's (`.glb`, or `.gltf` for `GLTF_SEPARATE` /
//! `GLTF_EMBEDDED`), `GLTF_SEPARATE` writes `<stem>.bin` and the textures next to the
//! `.gltf` (`export_texture_dir` relative to it), and missing directories are created.
//! `GLTF_EMBEDDED` is offered only with the add-on preference `allow_embedded_format` (§4.1),
//! off in factory settings (`enum "GLTF_EMBEDDED" not found in ('GLB', 'GLTF_SEPARATE')`).
//! `//` resolves against `bpy.data.filepath`, a deliberate deviation (D-08). Files are written
//! to temporary siblings and renamed after every fallible step (§12).

use frap_io_gltf::export::settings::{EvaluatedMeshes, ExportFormat, ExportSettings, is_supported};
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Support as S};
use frap_scene::{Context, EvalMode, Evaluator, HostRead, WindowState};
use frap_spec_ops_io::export_scene::{ExportSceneGltf as P, OPSPEC_EXPORT_SCENE_OT_GLTF};

use super::wm__save_as_mainfile::common;

// Port of `ensure_filepath_matches_export_format` (io_scene_gltf2 5.2.41 `__init__.py`,
// Blender 5.2.4, GPL-2.0-or-later).
/// The add-on's extension rule: a name without `.glb`/`.gltf` gets the format's extension, the
/// other one of the two is replaced; a path ending in a separator is left alone.
pub(crate) fn ensure_extension(filepath: &str, format: ExportFormat) -> String {
    let filename = filepath.rsplit(['/', '\\']).next().unwrap_or(filepath);
    if filename.is_empty() {
        return filepath.to_owned();
    }
    // `os.path.splitext`: the last dot not at the start of the name (leading dots belong to
    // the stem); a dot-only name like `.glb` has no extension.
    let ext_at = filename
        .char_indices()
        .rev()
        .find(|(_, c)| *c == '.')
        .map(|(i, _)| i)
        .filter(|&i| filename[..i].bytes().any(|b| b != b'.'));
    let (stem, ext) = match ext_at {
        Some(i) => (&filename[..i], &filename[i..]),
        None => (filename, ""),
    };
    let (_stem, ext) = if stem.starts_with('.') && ext.is_empty() {
        ("", stem)
    } else {
        (stem, ext)
    };
    let desired = if format == ExportFormat::Glb {
        ".glb"
    } else {
        ".gltf"
    };
    let lower = ext.to_ascii_lowercase();
    if lower != ".glb" && lower != ".gltf" {
        format!("{filepath}{desired}")
    } else if lower != desired {
        format!("{}{desired}", &filepath[..filepath.len() - ext.len()])
    } else {
        filepath.to_owned()
    }
}

/// `get_format_items`: `GLTF_EMBEDDED` only with the `allow_embedded_format` preference.
fn format_items(c: &Context) -> Vec<EnumItemOwned> {
    let item = |id: &str, name: &str, description: &str| EnumItemOwned {
        identifier: id.into(),
        name: name.into(),
        description: description.into(),
    };
    let mut items = vec![
        item(
            "GLB",
            "glTF Binary (.glb)",
            "Exports a single file, with all data packed in binary form. Most efficient and portable, but more difficult to edit later",
        ),
        item(
            "GLTF_SEPARATE",
            "glTF Separate (.gltf + .bin + textures)",
            "Exports multiple files, with separate JSON, binary and texture data. Easiest to edit later",
        ),
    ];
    let embedded = c
        .prefs
        .addons
        .get("io_scene_gltf2")
        .and_then(|p| p.get("allow_embedded_format"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if embedded {
        items.push(item(
            "GLTF_EMBEDDED",
            "glTF Embedded (.gltf)",
            "Exports a single file, with all data packed in JSON. Less efficient than binary, but easier to edit later",
        ));
    }
    items
}

/// One WARNING per prop the M1 exporter does not honour and the caller set to a non-default
/// value (the export behaves as if it had its default).
pub(crate) fn unsupported_props(a: &OpArgs) -> Vec<&'static str> {
    OPSPEC_EXPORT_SCENE_OT_GLTF
        .props
        .iter()
        .filter(|d| !is_supported(d.identifier))
        .filter(|d| {
            a.values
                .get(d.identifier)
                .is_some_and(|v| *v != Value::default_for(d))
        })
        .map(|d| d.identifier)
        .collect()
}

/// `Scene.objects`: the objects of the scene's collection hierarchy, each once, in
/// `FOREACH_SCENE_OBJECT` order (the collection's own objects, then its children).
fn scene_objects(
    main: &frap_scene::Main,
    scene: &frap_scene::id::Scene,
) -> Vec<frap_rna::IdRef<frap_rna::ObjectId>> {
    fn walk(
        main: &frap_scene::Main,
        c: &frap_scene::id::Collection,
        depth: usize,
        out: &mut Vec<frap_rna::IdRef<frap_rna::ObjectId>>,
    ) {
        if depth > 256 {
            return;
        }
        for o in &c.objects {
            if !out.contains(o) {
                out.push(*o);
            }
        }
        for ch in &c.children {
            if let Some(cc) = main.collections.get(*ch) {
                walk(main, cc, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(main, &scene.collection, 0, &mut out);
    out
}

/// `host` seen with another window state: the add-on gathers each exported scene while
/// `bpy.context.window.scene` is that scene.
struct SceneView<'a> {
    inner: &'a dyn HostRead,
    win: WindowState,
    /// `bpy.data` as evaluated instead of the host's (`export_skins`: ARMATURE modifiers
    /// disabled, WP 6.8).
    main: Option<&'a frap_scene::Main>,
}

impl HostRead for SceneView<'_> {
    fn main(&self) -> &frap_scene::Main {
        self.main.unwrap_or_else(|| self.inner.main())
    }
    fn win(&self) -> &WindowState {
        &self.win
    }
    fn edit(&self) -> &frap_scene::EditState {
        self.inner.edit()
    }
    fn ctx(&self) -> Context<'_> {
        Context {
            main: self.inner.main(),
            win: &self.win,
            prefs: self.inner.ctx().prefs,
        }
    }
    fn mesh_view(
        &self,
        mesh: frap_rna::IdRef<frap_rna::MeshId>,
    ) -> std::sync::Arc<frap_mesh::Mesh> {
        self.inner.mesh_view(mesh)
    }
    fn cwd(&self) -> Option<&std::path::Path> {
        self.inner.cwd()
    }
}

/// `export_apply=True`: the viewport-evaluated mesh of every MESH object the viewport
/// depsgraph evaluates (the add-on's `blender_object.evaluated_get(depsgraph).to_mesh()`), read
/// by the exporter's `apply.rs`. Objects outside the depsgraph (`hide_viewport`, in a hidden
/// collection: `apply::viewport_evaluated_objects`) and objects whose evaluation fails are
/// left out: the exporter then uses the original mesh, as `to_mesh()` of an original object.
///
/// Each exported scene is evaluated in its own depsgraph: the add-on gathers a scene's meshes
/// while `bpy.context.window.scene` is that scene (`VExportTree.construct`), so
/// `evaluated_depsgraph_get()` is the depsgraph of that scene's view layer (the window's view
/// layer name when the scene has it, else its first) at that scene's frame. An object linked
/// into several scenes is evaluated once per scene, so a frame-dependent modifier (BUILD, WAVE)
/// gives each scene's node that scene's result (M34S5-1: verified on 5.2.4, identical in every
/// run).
fn evaluated_meshes(
    host: &dyn HostRead,
    eval: &mut dyn Evaluator,
    settings: &ExportSettings,
    warnings: &mut Vec<frap_rna::Report>,
) -> EvaluatedMeshes {
    let user_win = host.win().clone();
    let scenes =
        frap_io_gltf::export::scenes::exported_scenes(host.main(), user_win.scene, settings);
    // `__gather_mesh`: with `export_skins` the ARMATURE modifiers are switched off in the
    // viewport while the mesh is evaluated (the skin deforms it in glTF), on a copy here.
    let unarmed = settings.props.export_skins.value.then(|| {
        let mut main = host.main().clone();
        let mut log = frap_scene::ChangeLog::default();
        let obs: Vec<_> = main
            .objects
            .iter()
            .filter(|(_, o)| {
                o.modifiers
                    .iter()
                    .any(|m| matches!(m.settings, frap_scene::id::ModifierSettings::Armature(_)))
            })
            .map(|(r, _)| r)
            .collect();
        for r in &obs {
            if let Some(o) = main.objects.get_mut(*r, &mut log) {
                for m in o.modifiers.iter_mut() {
                    if matches!(m.settings, frap_scene::id::ModifierSettings::Armature(_)) {
                        m.show_viewport = false;
                    }
                }
            }
        }
        (!obs.is_empty()).then_some(main)
    });
    let unarmed = unarmed.flatten();
    let mut out = EvaluatedMeshes::new();
    for scene in scenes {
        let mut win = user_win.clone();
        if scene != user_win.scene
            && frap_scene::window::set_scene(host.main(), &mut win, scene).is_err()
        {
            continue;
        }
        let view = SceneView {
            inner: host,
            win,
            main: unarmed.as_ref(),
        };
        let in_depsgraph =
            frap_io_gltf::export::apply::viewport_evaluated_objects(view.main(), &view.ctx());
        let obs: Vec<_> = {
            let main = view.main();
            let Some(sc) = main.scenes.get(scene) else {
                continue;
            };
            scene_objects(main, sc)
                .into_iter()
                .filter(|r| {
                    main.objects
                        .get(*r)
                        .is_some_and(|o| o.object_type() == "MESH")
                })
                .filter(|r| in_depsgraph.contains(r))
                .collect()
        };
        for ob in obs {
            if let Ok(ev) = eval.object(&view, ob, EvalMode::Viewport) {
                // D-52: a modifier past `--max-elements` is a WARNING of the export.
                for w in ev.limit_warnings() {
                    if !warnings.contains(w) {
                        warnings.push(w.clone());
                    }
                }
                if let Some(me) = ev.mesh.clone() {
                    out.insert((scene, ob), me);
                }
            }
        }
    }
    out
}

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    // `export.save`: `if bpy.context.active_object.mode != 'OBJECT':
    // bpy.ops.object.mode_set(mode='OBJECT')`, before anything else (an empty path still
    // leaves Edit Mode, verified on 5.2.4). Leaving Edit Mode writes every edit mesh back, so
    // the export sees the edited geometry, and the session stays in Object Mode.
    helpers::mode_set_object(cx)?;
    let p = P::from_args(a);
    let mut settings = ExportSettings::from_props(&p);
    if p.filepath.value.is_empty() {
        return Err(OpError::Io("No such file or directory: ''".into()));
    }
    let fixed = ensure_extension(&p.filepath.value, settings.export_format);
    let path = common::resolve(cx, &fixed);
    settings.file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scene".into());
    settings.blend_dir = common::blend_dir(cx);
    settings.cwd = Some(cx.services().paths().cwd.clone());
    // `VExportTree.variants_reset_to_original` (KHR_materials_variants, M7 WP 7.5): with the
    // add-on preference on, the exported mesh objects get the default material of each slot
    // back, on the Blender data itself (the add-on leaves them there).
    {
        let prefs = cx.ctx().prefs.clone();
        let win = cx.ctx().win.clone();
        let (main, log) = cx.main_mut();
        frap_io_gltf::export::variants::reset_to_original(main, &win, &prefs, &settings, log);
    }
    // `export.save`: `if not export_current_frame: scene.frame_set(0)`, then
    // `frame_set(original)` after the export: Blender exports the animation of frame 0 and
    // re-applies the current frame's animation to the originals afterwards. frap exports a
    // clone with frame 0 set and its animation applied, and leaves the originals (manual edits
    // of animated channels included) as they are (D-07, §7.3, WP 6.4).
    let mut snap = if p.export_current_frame.value {
        None
    } else {
        Some(cx.snapshot_at(Some(0))?)
    };
    if p.export_apply.value {
        let mut warnings = Vec::new();
        let meshes = match &mut snap {
            Some(s) => evaluated_meshes(&*s.host, &mut *s.eval, &settings, &mut warnings),
            None => {
                let (host, eval) = cx.split_eval();
                evaluated_meshes(host, eval, &settings, &mut warnings)
            }
        };
        for w in warnings {
            cx.report(w.level, w.message);
        }
        settings.evaluated_meshes = Some(std::sync::Arc::new(meshes));
    }
    for prop in unsupported_props(a) {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: export_scene.gltf: '{prop}' is not supported yet; exported as if it had its default value"
            ),
        );
    }
    let out = match &snap {
        Some(s) => frap_io_gltf::export(s.host.main(), &s.host.ctx(), &settings),
        None => frap_io_gltf::export(cx.main(), &cx.ctx(), &settings),
    }
    .map_err(|e| match e {
        frap_io_gltf::ExportError::NotImplemented(m) => OpError::NotImplemented(m),
        e => OpError::Runtime(e.to_string()),
    })?;
    let dir = path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let mut files = vec![(path.clone(), out.bytes.clone())];
    for (rel, bytes) in &out.files {
        files.push((dir.join(rel), bytes.clone()));
    }
    common::write_files(&files, true)?;
    for r in out.reports {
        cx.report(r.level, r.message);
    }
    cx.result("filepath", Value::Str(fixed.as_str().into()));
    cx.result("bytes", Value::Int(out.bytes.len() as i64));
    cx.result("nodes", Value::Int(out.nodes as i64));
    cx.result("meshes", Value::Int(out.meshes as i64));
    cx.result(
        "extensions_used",
        Value::Tuple(
            out.extensions_used
                .iter()
                .map(|e| Value::Str(e.into()))
                .collect(),
        ),
    );
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_op::DYN_ENUM_HINTS)]
static EMBEDDED_HINT: frap_op::DynEnumHint = frap_op::DynEnumHint {
    bl_idname: "EXPORT_SCENE_OT_gltf",
    prop: "export_format",
    item: "GLTF_EMBEDDED",
    hint: "GLTF_EMBEDDED is offered only with the glTF add-on preference: run bpy.context.preferences.addons['io_scene_gltf2'].preferences.allow_embedded_format = True first",
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_EXPORT_SCENE_OT_GLTF,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[("export_format", format_items)],
    tier: Tier::T2,
    support: S::Partial(
        "M1: static meshes, hierarchy, Principled base colour/metallic/roughness and base-colour textures; other props give a WARNING when not at their default",
    ),
    notes: &[
        "every Blender prop is accepted; props the M1 exporter does not honour give a WARNING naming the prop when set to a non-default value",
        "the file extension is forced to the format's (.glb / .gltf), as the add-on does",
        "missing parent directories are created, as Blender's add-on does",
        "'//' resolves relative to bpy.data.filepath (deviation D-08: the add-on writes it raw)",
        "GLTF_EMBEDDED needs the add-on preference allow_embedded_format (off in factory settings)",
        "an active object in another mode (Edit Mode) goes back to Object Mode first, writing its edits back, as the add-on does",
        "export_current_frame=False exports the animation of frame 0 from a clone of the state; the scene's originals keep their values (deviation D-07: the add-on's frame_set re-applies the current frame's animation to them)",
        "export_current_frame=True leaves scene.frame_current unchanged (deviation D-07: Blender's animation sampler leaves it on the last sampled frame)",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_rule_matches_the_addon() {
        let g = ExportFormat::Glb;
        let s = ExportFormat::GltfSeparate;
        assert_eq!(ensure_extension("out/a", g), "out/a.glb");
        assert_eq!(ensure_extension("out/a.glb", g), "out/a.glb");
        assert_eq!(ensure_extension("out/a.GLB", g), "out/a.GLB");
        assert_eq!(ensure_extension("out/a.gltf", g), "out/a.glb");
        assert_eq!(ensure_extension("out/a.glb", s), "out/a.gltf");
        assert_eq!(ensure_extension("out/a.b.c", s), "out/a.b.c.gltf");
        assert_eq!(ensure_extension("out/.glb", g), "out/.glb");
        assert_eq!(ensure_extension("out/.glb", s), "out/.gltf");
        assert_eq!(ensure_extension("out/", g), "out/");
        assert_eq!(ensure_extension("a.png", g), "a.png.glb");
    }
}
