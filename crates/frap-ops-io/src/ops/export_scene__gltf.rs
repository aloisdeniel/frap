// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.export_scene.gltf` (ARCHITECTURE §17.2; M0: `filepath`, `export_format='GLB'`).
//! Like Blender's add-on, missing parent directories are created. The file is written to a
//! temporary sibling and renamed, after every fallible step (§12). Moved from
//! `frap-session/src/ops/` by M1 W0; WP 1.23 owns it from W4.

use frap_io_gltf::{ExportFormat, ExportSettings};
use frap_op::prelude::*;
use frap_spec_ops_io::export_scene::{ExportSceneGltf as P, OPSPEC_EXPORT_SCENE_OT_GLTF};

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    if p.filepath.value.is_empty() {
        return Err(OpError::Io("filepath not set".into()));
    }
    let settings = ExportSettings {
        export_format: if p.export_format.value == "GLB" {
            ExportFormat::Glb
        } else {
            ExportFormat::GltfSeparate
        },
        ..ExportSettings::default()
    };
    let out = frap_io_gltf::export(cx.main(), &cx.ctx(), &settings)
        .map_err(|e| OpError::NotImplemented(e.to_string()))?;
    let path = cx.services().paths().resolve(&p.filepath.value);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| OpError::Io(format!("cannot create {}: {e}", dir.display())))?;
    }
    let tmp = path.with_extension("glb.tmp");
    std::fs::write(&tmp, &out.bytes)
        .map_err(|e| OpError::Io(format!("cannot write {}: {e}", path.display())))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        OpError::Io(format!("cannot write {}: {e}", path.display()))
    })?;
    cx.result("filepath", Value::Str(p.filepath.value.clone()));
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

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1); #[operator] emits this
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_EXPORT_SCENE_OT_GLTF,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T2,
    support: Support::Partial("M0 subset: filepath and export_format='GLB'"),
    notes: &[
        "M0: static meshes, Principled base factors; other export props arrive in M1/M3",
        "missing parent directories are created, as Blender's add-on does",
    ],
};
