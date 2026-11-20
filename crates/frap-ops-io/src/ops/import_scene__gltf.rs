// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.import_scene.gltf` (ARCHITECTURE §17.2): the operator around
//! `frap_io_gltf::import::import_file` (WP 1.17's library, io_scene_gltf2 5.2.41 semantics).
//!
//! As in the add-on's `ImportGLTF2.import_gltf2` / `unit_import` (verified on 5.2.4): with
//! `files` set, every `directory/<name>` is imported, else `filepath`; a path that is not a file
//! is `Please select a file`; the importer's `ImportError`s become `RuntimeError("Error: …")`
//! (Blender reports them as ERROR, which `bpy.ops` raises). The importer's WARNING/INFO
//! messages (features not imported yet, missing images, …) are the operator's reports. `//`
//! resolves against `bpy.data.filepath`.
//!
//! Deviation: with `files`, Blender's `import_gltf2` imports every file it can, reports an ERROR
//! for each one that fails (so `bpy.ops` raises) and keeps the others; frap stops at the first
//! failure and rolls the statement back, as every failing statement does (§12).

use std::path::PathBuf;

use frap_io_gltf::import::{ImportSettings, import_file};
use frap_op::prelude::*;
use frap_spec_ops_io::import_scene::{ImportSceneGltf as P, OPSPEC_IMPORT_SCENE_OT_GLTF};

use super::wm__save_as_mainfile::common;

/// `files[*].name` of the operator's `files` collection.
fn file_names(v: &Value) -> Vec<String> {
    match v {
        Value::Structs(items) => items
            .iter()
            .filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_owned))
            .filter(|n| !n.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    let settings = ImportSettings::from(&p);
    let names = file_names(&p.files.value);
    let paths: Vec<PathBuf> = if names.is_empty() {
        vec![common::resolve(cx, &p.filepath.value)]
    } else {
        let dir = common::resolve(cx, &p.directory.value);
        names.iter().map(|n| dir.join(n)).collect()
    };
    let win = cx.ctx().win.clone();
    let mut created: Vec<Value> = Vec::new();
    for path in &paths {
        if p.filepath.value.is_empty() && names.is_empty() || !path.is_file() {
            return Err(OpError::Io("Please select a file".into()));
        }
        let report = {
            let (main, log) = cx.main_mut();
            import_file(main, &win, path, &settings, log)
                .map_err(|e| OpError::Runtime(e.to_string()))?
        };
        for r in report.reports {
            cx.report(r.level, r.message);
        }
        for ob in report.objects {
            if let Some(n) = cx.main().objects.name_of(ob) {
                created.push(Value::Str(n.into()));
            }
        }
    }
    cx.result("objects", Value::Tuple(created));
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_IMPORT_SCENE_OT_GLTF,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T2,
    support: Support::Partial(
        "static meshes, hierarchy, cameras, lights, Principled materials and textures, skins and morph targets (M6); animations (M6) and compressed meshes (M7) give a WARNING",
    ),
    notes: &[
        "with files=[{'name': …}] every directory/<name> is imported, else filepath",
        "with files=[…], the first file that fails raises and the whole statement is rolled back (frap statements are atomic, §12); Blender reports an ERROR per failing file and keeps the files it did import",
        "result.objects lists the created objects",
        "'//' resolves relative to bpy.data.filepath",
    ],
};
