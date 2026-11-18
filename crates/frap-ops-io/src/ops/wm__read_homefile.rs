// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.wm.read_homefile` (ARCHITECTURE §5.7, §15): the factory startup scene, or with
//! `use_empty=True` the empty one (the factory scene without objects, collections, camera or
//! world, verified on 5.2.4). `bpy.data.filepath` becomes empty. frap has no user startup
//! file, so the home file is always the factory one; `filepath` names a `.frap` to load as the
//! startup state (`bpy.data.filepath` stays empty, as for Blender's startup file). Errors use
//! Blender's messages: `Cannot read alternative start-up file: "<filepath>"` for a missing file
//! (`IO_ERROR`), `Could not read "<filepath>"` for a file that is not a `.frap`
//! (`IO_UNSUPPORTED_FORMAT`; Blender says it for any non-`.blend` file).

use frap_op::prelude::*;
use frap_project::scene_map;
use frap_spec_ops_io::wm::{OPSPEC_WM_OT_READ_HOMEFILE, ReadHomefile as P};

use super::wm__save_as_mainfile::common;

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    common::report_ignored(cx, a, &["filepath", "use_empty", "use_factory_startup"]);
    if p.filepath.value.is_empty() {
        return common::read_home(cx, p.use_empty.value, false);
    }
    let given = p.filepath.value.as_str();
    let path = common::resolve(cx, given);
    // Blender's messages name the path as given (verified on 5.2.4).
    if !path.is_file() {
        return Err(OpError::Io(format!(
            "Cannot read alternative start-up file: \"{given}\""
        )));
    }
    let opened = scene_map::open_file(&path, false).map_err(|e| match e {
        scene_map::MapError::Project(frap_project::container::ProjectError::NotFrap(_)) => {
            OpError::UnsupportedFormat(format!("Could not read \"{given}\""))
        }
        e => OpError::Io(format!("Could not read \"{given}\": {e}")),
    })?;
    let mut main = opened.main;
    main.filepath = None;
    main.is_dirty = false;
    common::replace_state(cx, main, opened.win)?;
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_WM_OT_READ_HOMEFILE,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Full,
    notes: &[
        "the home file is always the factory startup (frap has no user startup file)",
        "filepath loads a .frap as the startup state",
        "load_ui, use_splash and app_template are accepted and ignored with an INFO report",
    ],
};
