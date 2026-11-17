// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.save_mainfile` (ARCHITECTURE §15): saves to `filepath`, or to
//! `bpy.data.filepath` when it is empty, and makes it `bpy.data.filepath`; reports
//! `Saved "x.frap"` (verified on 5.2.4). Unsaved without a path: Blender's `Unable to save an
//! unsaved file with an empty or unset "filepath" property`.

use frap_op::prelude::*;
use frap_spec_ops_io::wm::{OPSPEC_WM_OT_SAVE_MAINFILE, SaveMainfile as P};

use super::wm__save_as_mainfile::{UNSAVED, common};

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    common::report_ignored(cx, a, &["filepath"]);
    let raw = if p.filepath.value.is_empty() {
        cx.main()
            .filepath
            .clone()
            .ok_or_else(|| OpError::Runtime(UNSAVED.into()))?
    } else {
        p.filepath.value.to_string()
    };
    let path = common::resolve(cx, &raw);
    common::save(cx, &path, common::SaveKind::Save)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_SAVE_MAINFILE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Partial(".frap files only (a .blend path is IO_UNSUPPORTED_FORMAT)"),
    notes: &[
        "saves to bpy.data.filepath when filepath is empty",
        "compress, relative_remap, exit and the file-browser props are accepted and ignored with an INFO report",
    ],
};
