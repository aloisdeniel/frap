// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared code of the file operators (`wm.*`, `export_scene.gltf`, `import_scene.gltf`):
//! path resolution, atomic writes, `.frap` save/load into the session state (ARCHITECTURE
//! §12, §15, §17.2). A submodule of `wm__save_as_mainfile.rs` (§22.1: helpers live under a
//! file the WP owns; `automod` only includes the top-level files).

use std::path::{Path, PathBuf};

use frap_op::prelude::*;
use frap_project::scene_map;
use frap_scene::{EditState, Main, WindowState};

/// Directory of `bpy.data.filepath` (absolute), when the file was saved.
pub(crate) fn blend_dir(cx: &mut OpCtx) -> Option<PathBuf> {
    let fp = cx.main().filepath.clone()?;
    let abs = cx.services().paths().resolve(&fp);
    abs.parent().map(Path::to_path_buf)
}

/// Absolute path of a user path: `//` relative to the saved `.frap` (or the cwd when unsaved),
/// other relative paths against the cwd (`docs/deviations.md` D-08 for `export_scene.gltf`).
pub(crate) fn resolve(cx: &mut OpCtx, p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("//")
        && let Some(dir) = blend_dir(cx)
    {
        return dir.join(rest);
    }
    cx.services().paths().resolve(p)
}

/// The file name Blender's reports show (`Saved "part.frap"`).
pub(crate) fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// True for `.blend` paths (frap reads and writes `.frap` only, §15).
pub(crate) fn is_blend(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("blend"))
}

/// An OS error as Python's `strerror` spells it (`No such file or directory`).
pub(crate) fn os_error(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.find(" (os error ") {
        Some(i) => s[..i].to_owned(),
        None => s,
    }
}

/// `{"<prop>": …}` props the caller set that do not apply to frap: one INFO each (§15).
pub(crate) fn report_ignored(cx: &mut OpCtx, a: &OpArgs, applies: &[&str]) {
    let op = cx.op().idname;
    let ignored: Vec<&'static str> = a
        .values
        .keys()
        .copied()
        .filter(|k| !applies.contains(k) && a.was_set(k))
        .collect();
    for k in ignored {
        cx.report(
            ReportType::Info,
            format!("frap: {op}: '{k}' does not apply to frap and was ignored"),
        );
    }
}

/// Writes `files` (absolute path, bytes) atomically: every file goes to a temporary sibling
/// first, then all are renamed, so a failure never leaves a partial output (§12).
/// `create_dirs` creates missing parent directories (the glTF exporter does; Blender's file
/// save does not).
pub(crate) fn write_files(files: &[(PathBuf, Vec<u8>)], create_dirs: bool) -> Result<(), OpError> {
    let mut temps: Vec<(PathBuf, &Path)> = Vec::with_capacity(files.len());
    let cleanup = |temps: &[(PathBuf, &Path)]| {
        for (t, _) in temps {
            let _ = std::fs::remove_file(t);
        }
    };
    for (path, bytes) in files {
        if create_dirs
            && let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            cleanup(&temps);
            return Err(OpError::Io(format!(
                "Cannot create directory {}: {}",
                dir.display(),
                os_error(&e)
            )));
        }
        let tmp = frap_project::container::temp_path(path);
        if let Err(e) = std::fs::write(&tmp, bytes) {
            cleanup(&temps);
            return Err(OpError::Io(format!(
                "Cannot open file {} for writing: {}",
                tmp.display(),
                os_error(&e)
            )));
        }
        temps.push((tmp, path));
    }
    for (i, (tmp, path)) in temps.iter().enumerate() {
        if let Err(e) = std::fs::rename(tmp, path) {
            cleanup(&temps[i..]);
            return Err(OpError::Io(format!(
                "Cannot write {}: {}",
                path.display(),
                os_error(&e)
            )));
        }
    }
    Ok(())
}

/// How a save reports itself (`wm_save_as_mainfile_exec`, `wm_save_mainfile_exec`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SaveKind {
    /// `Saved "x.frap"` (`wm.save_mainfile`).
    Save,
    /// `Saved as "x.frap"` (`wm.save_as_mainfile`).
    SaveAs,
    /// `Saved copy as "x.frap"` (`copy=True`: `bpy.data.filepath` is kept).
    Copy,
}

/// Saves the session state to `path` (absolute) as a `.frap` (§15) and, unless it is a copy,
/// makes it `bpy.data.filepath` with `is_dirty` cleared.
pub(crate) fn save(cx: &mut OpCtx, path: &Path, kind: SaveKind) -> OpResult {
    if is_blend(path) {
        return Err(OpError::UnsupportedFormat(
            "frap cannot write .blend files; use a .frap path".into(),
        ));
    }
    let saved = {
        let (host, _) = cx.split_eval();
        let ctx = host.ctx();
        let view = |m| Some(host.mesh_view(m));
        scene_map::save(ctx.main, ctx.win, ctx.prefs, Some(&view))
            .map_err(|e| OpError::Runtime(format!("Cannot save {}: {e}", path.display())))?
    };
    // Blender does not create missing directories for a .blend save (verified: `Cannot open
    // file <path>@ for writing: No such file or directory`).
    write_files(&[(path.to_path_buf(), saved.bytes.clone())], false)?;
    let name = file_name(path);
    let msg = match kind {
        SaveKind::Save => format!("Saved \"{name}\""),
        SaveKind::SaveAs => format!("Saved as \"{name}\""),
        SaveKind::Copy => format!("Saved copy as \"{name}\""),
    };
    cx.report(ReportType::Info, msg);
    if !saved.dropped.is_empty() {
        cx.report(
            ReportType::Info,
            format!(
                "frap: {} data-block(s) without users were not saved: {}",
                saved.dropped.len(),
                saved.dropped.join(", ")
            ),
        );
    }
    if kind != SaveKind::Copy {
        let (main, _) = cx.main_mut();
        main.filepath = Some(path.to_string_lossy().into_owned());
        main.is_dirty = false;
    }
    cx.result("filepath", Value::Str(path.to_string_lossy().as_ref().into()));
    cx.result("bytes", Value::Int(saved.bytes.len() as i64));
    cx.result("orphans_dropped", Value::Int(saved.dropped.len() as i64));
    Ok(OpStatus::Finished)
}

/// Replaces the whole session state (`bpy.data`, window, edit state) with a loaded one.
pub(crate) fn replace_state(cx: &mut OpCtx, main: Main, win: WindowState) {
    {
        let (m, log) = cx.main_mut();
        for a in m.all_ids() {
            if let Some(h) = m.header(a) {
                log.removed.push((a, h.name.clone()));
            }
        }
        *m = main;
        for a in m.all_ids() {
            log.added.push(a);
        }
        log.context_changed = true;
    }
    *cx.win_mut() = win;
    *cx.split_edit().1 = EditState::default();
}

/// `wm.open_mainfile` / `wm.revert_mainfile`: loads the `.frap` at `path` (absolute) and makes
/// it `bpy.data.filepath`.
pub(crate) fn open(cx: &mut OpCtx, path: &Path) -> OpResult {
    if is_blend(path) {
        return Err(OpError::UnsupportedFormat(
            "frap cannot read .blend files; open a .frap file".into(),
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| {
        OpError::Io(format!(
            "Cannot read file \"{}\": {}",
            path.display(),
            os_error(&e)
        ))
    })?;
    let opened = scene_map::open(&bytes, false).map_err(|e| match e {
        scene_map::MapError::Project(frap_project::container::ProjectError::NotFrap(_)) => {
            OpError::UnsupportedFormat(format!(
                "File format is not supported in file \"{}\"",
                path.display()
            ))
        }
        e => OpError::Io(format!("Cannot read file \"{}\": {e}", path.display())),
    })?;
    let mut main = opened.main;
    main.filepath = Some(path.to_string_lossy().into_owned());
    main.is_dirty = false;
    replace_state(cx, main, opened.win);
    for w in opened.warnings {
        cx.report(ReportType::Warning, w);
    }
    if let Some(v) = opened.migrated_from {
        cx.report(
            ReportType::Info,
            format!("frap: {} was migrated from schema version {v}", file_name(path)),
        );
    }
    cx.result("filepath", Value::Str(path.to_string_lossy().as_ref().into()));
    Ok(OpStatus::Finished)
}

/// `wm.read_homefile` / `wm.read_factory_settings`: the factory or empty startup state, unsaved.
pub(crate) fn read_home(cx: &mut OpCtx, use_empty: bool) -> OpResult {
    let (mut main, win) = if use_empty {
        frap_scene::startup::empty()
    } else {
        frap_scene::startup::factory()
    };
    main.filepath = None;
    main.is_dirty = false;
    replace_state(cx, main, win);
    Ok(OpStatus::Finished)
}
