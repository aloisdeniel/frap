// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.open_mainfile` (ARCHITECTURE §15): replaces the session state with a `.frap`
//! (data, window scene and view layer, selection, active object, cursor, tool settings) and
//! makes it `bpy.data.filepath` (absolute) with `is_dirty` cleared. Blender's messages
//! (verified on 5.2.4): `Cannot read file "<path>": No such file or directory`, `File format is
//! not supported in file "<path>"` (code `IO_UNSUPPORTED_FORMAT`, also for `.blend` paths).
//! `load_ui`, `use_scripts` and the file-browser props are accepted and ignored with an INFO
//! report.

use frap_op::prelude::*;
use frap_spec_ops_io::wm::{OPSPEC_WM_OT_OPEN_MAINFILE, OpenMainfile as P};

use super::wm__save_as_mainfile::common;

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    common::report_ignored(cx, a, &["filepath"]);
    let path = common::resolve(cx, &p.filepath.value);
    if path.is_dir() {
        return Err(OpError::UnsupportedFormat(format!(
            "File format is not supported in file \"{}\"",
            path.display()
        )));
    }
    common::open(cx, &path)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_OPEN_MAINFILE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Partial(".frap files only (a .blend path is IO_UNSUPPORTED_FORMAT)"),
    notes: &[
        "restores data, window scene and view layer, selection, active object and tool settings",
        "meshes saved in Edit Mode are loaded back into Edit Mode (selection history is not kept)",
        "load_ui, use_scripts and the file-browser props are accepted and ignored with an INFO report",
    ],
};
