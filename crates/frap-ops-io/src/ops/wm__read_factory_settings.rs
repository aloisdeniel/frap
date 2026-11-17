// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.read_factory_settings` (ARCHITECTURE §5.7): the factory startup scene (or the
//! empty one with `use_empty=True`), unsaved. frap keeps no user preferences, so only the scene
//! is reset.

use frap_op::prelude::*;
use frap_spec_ops_io::wm::{OPSPEC_WM_OT_READ_FACTORY_SETTINGS, ReadFactorySettings as P};

use super::wm__save_as_mainfile::common;

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    common::report_ignored(cx, a, &["use_empty"]);
    common::read_home(cx, p.use_empty.value, true)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_READ_FACTORY_SETTINGS,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Full,
    notes: &[
        "resets the scene to the factory startup and the glTF add-on preferences to their defaults (frap has no other user preferences)",
    ],
};
