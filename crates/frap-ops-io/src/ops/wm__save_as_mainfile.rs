// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.save_as_mainfile` (ARCHITECTURE §15): writes the session as a `.frap`. As in
//! Blender (`wm_save_as_mainfile_exec`, verified on 5.2.4): an empty `filepath` reuses
//! `bpy.data.filepath` (`Unable to save an unsaved file with an empty or unset "filepath"
//! property` when unsaved); `copy=True` keeps `bpy.data.filepath`; missing directories are an
//! error; the report is `Saved as "x.frap"` (`Saved copy as …` for a copy). A `.blend` path is
//! `IO_UNSUPPORTED_FORMAT`. Props that do not apply to `.frap` (`compress`, `relative_remap`,
//! the file-browser filters, …) are accepted and ignored with an INFO report. Orphans are not
//! written (§5.5) and counted in `result.orphans_dropped`.

pub(crate) mod common;

use frap_op::prelude::*;
use frap_spec_ops_io::wm::{OPSPEC_WM_OT_SAVE_AS_MAINFILE, SaveAsMainfile as P};

use common::SaveKind;

/// Blender's message for a save without a path while `bpy.data.filepath` is empty.
pub(crate) const UNSAVED: &str =
    "Unable to save an unsaved file with an empty or unset \"filepath\" property";

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    common::report_ignored(cx, a, &["filepath", "copy"]);
    let raw = if p.filepath.value.is_empty() {
        cx.main()
            .filepath
            .clone()
            .ok_or_else(|| OpError::Runtime(UNSAVED.into()))?
    } else {
        p.filepath.value.to_string()
    };
    let path = common::resolve(cx, &raw);
    let kind = if p.copy.value {
        SaveKind::Copy
    } else {
        SaveKind::SaveAs
    };
    common::save(cx, &path, kind)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_SAVE_AS_MAINFILE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Partial(".frap files only (a .blend path is IO_UNSUPPORTED_FORMAT)"),
    notes: &[
        "writes a .frap project (zip: manifest, scene.json, buffers, images); byte-identical for identical states",
        "data-blocks without users are not written, as in Blender (result.orphans_dropped)",
        "compress, relative_remap and the file-browser props are accepted and ignored with an INFO report",
    ],
};
