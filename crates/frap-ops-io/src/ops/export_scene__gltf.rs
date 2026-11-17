// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.export_scene.gltf` (ARCHITECTURE §17.2): the operator around
//! `frap_io_gltf::export`. All of Blender's props are accepted; the ones the M1 exporter does
//! not honour (`frap_io_gltf::export::SUPPORTED_PROPS` lists those it does) give one WARNING
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

use frap_io_gltf::export::settings::{ExportFormat, ExportSettings, SUPPORTED_PROPS};
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Support as S};
use frap_scene::Context;
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
        .filter(|d| !SUPPORTED_PROPS.contains(&d.identifier))
        .filter(|d| {
            a.values
                .get(d.identifier)
                .is_some_and(|v| *v != Value::default_for(d))
        })
        .map(|d| d.identifier)
        .collect()
}

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
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
    for prop in unsupported_props(a) {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: export_scene.gltf: '{prop}' is not supported yet; exported as if it had its default value"
            ),
        );
    }
    let out = frap_io_gltf::export(cx.main(), &cx.ctx(), &settings).map_err(|e| match e {
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
    cx.result(
        "files",
        Value::Tuple(
            files
                .iter()
                .map(|(f, _)| Value::Str(f.to_string_lossy().as_ref().into()))
                .collect(),
        ),
    );
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
