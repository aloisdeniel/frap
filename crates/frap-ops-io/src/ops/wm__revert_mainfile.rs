// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.revert_mainfile` (ARCHITECTURE §15): reloads `bpy.data.filepath`. Its poll
//! (`wm_revert_mainfile_poll`) fails when the file was never saved (verified on 5.2.4:
//! `Operator bpy.ops.wm.revert_mainfile.poll() failed, context is incorrect`).

use frap_op::PollFail;
use frap_op::prelude::*;
use frap_scene::Context;
use frap_spec_ops_io::wm::OPSPEC_WM_OT_REVERT_MAINFILE;

use super::wm__save_as_mainfile::common;

fn saved(c: &Context) -> Result<(), PollFail> {
    if c.main.filepath.as_deref().is_some_and(|f| !f.is_empty()) {
        return Ok(());
    }
    Err(PollFail {
        message: None,
        unmet: Default::default(),
        hint: Some(
            "save the file first, e.g. bpy.ops.wm.save_as_mainfile(filepath='scene.frap')".into(),
        ),
    })
}

/// `bpy.data.filepath` is set.
const SAVED: frap_op::Poll = frap_op::Poll {
    requires: &[],
    check: saved,
};

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    common::report_ignored(cx, a, &[]);
    let fp = cx
        .main()
        .filepath
        .clone()
        .ok_or_else(|| OpError::Runtime("the file was never saved".into()))?;
    let path = common::resolve(cx, &fp);
    common::open(cx, &path)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_REVERT_MAINFILE,
    poll: SAVED,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Full,
    notes: &[
        "reloads bpy.data.filepath (a .frap)",
        "poll: needs a saved file (bpy.data.filepath set); it fails with OP_POLL_FAILED while the file is unsaved (not expressible in `requires` yet)",
    ],
};
