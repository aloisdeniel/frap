// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.mode_set_with_submode(mode='OBJECT', toggle=False, mesh_select_mode=set())`,
//! tier T0 (`tests/golden/cases/object/select_m8/mode_set_with_submode.py`). Ported from
//! blender/source/blender/editors/object/object_edit.cc (`object_mode_set_exec` with
//! `use_submode`, `OBJECT_OT_mode_set_with_submode`) and editors/mesh/editmesh_select.cc
//! (`EDBM_selectmode_set_multi`, `EDBM_selectmode_set`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The same operator as `object.mode_set` (poll, `mode` items following the active object's
//! type, toggling: frap runs `object.mode_set`), then, when the active object is a mesh in
//! Edit Mode and `mesh_select_mode` is set and not empty, the select mode of the scene's tool
//! settings and of every mesh in Edit Mode becomes exactly that set, each edit mesh whose mode
//! differs re-deriving its selection (`EDBM_selectmode_set`: no conversion). No register / undo
//! flags, as `mode_set`. Verified on 5.2.4.

use frap_bmesh::SelectMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollFail, PollReq};
use frap_rna::{Input, PropKind};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;

/// `object_mode_set_poll` → `ED_operator_object_active_editable_ex`, with its poll messages.
fn active_editable(c: &Context) -> Result<(), PollFail> {
    poll::active_editable(c).map_err(|mut f| {
        let why = match poll::active(c) {
            None => "Context missing active object",
            Some((_, ob)) if ob.header.library.is_some() => "Cannot edit library linked object",
            Some(_) => "Cannot edit hidden object",
        };
        f.message = Some(format!(
            "Operator bpy.ops.object.mode_set_with_submode.poll() {why}"
        ));
        f
    })
}

/// `object_mode_set_poll`
const ACTIVE_EDITABLE: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: active_editable,
};

/// `ED_object_mode_compat_test` (as `object.mode_set`).
fn compatible(object_type: &str, mode: &str) -> bool {
    if mode == "OBJECT" {
        return true;
    }
    let modes: &[&str] = match object_type {
        "MESH" => &[
            "EDIT",
            "SCULPT",
            "VERTEX_PAINT",
            "WEIGHT_PAINT",
            "TEXTURE_PAINT",
        ],
        "CURVE" | "SURFACE" | "FONT" | "META" | "LATTICE" | "POINTCLOUD" => &["EDIT"],
        "ARMATURE" => &["EDIT", "POSE"],
        "CURVES" => &["EDIT", "SCULPT_CURVES"],
        "GREASEPENCIL" => &[
            "EDIT",
            "SCULPT_GREASE_PENCIL",
            "PAINT_GREASE_PENCIL",
            "WEIGHT_GREASE_PENCIL",
            "VERTEX_GREASE_PENCIL",
        ],
        _ => &[],
    };
    modes.contains(&mode)
}

/// `object_mode_set_itemf`: the modes of the active object's type, in RNA order (`OBJECT` only
/// without an active object).
fn mode_items(c: &Context) -> Vec<EnumItemOwned> {
    let PropKind::Enum { items, .. } = &object::MODE_SET_WITH_SUBMODE_PROPS[0].kind else {
        return Vec::new();
    };
    let ty = poll::active(c).map(|(_, o)| o.object_type());
    items
        .iter()
        .filter(|i| match ty {
            Some(t) => compatible(t, i.identifier),
            None => i.identifier == "OBJECT",
        })
        .map(|i| EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// `mesh_select_mode` as `ts->selectmode` bits.
fn select_mode(items: &std::collections::BTreeSet<smol_str::SmolStr>) -> SelectMode {
    let mut m = SelectMode::empty();
    for i in items {
        match i.as_str() {
            "VERT" => m |= SelectMode::VERT,
            "EDGE" => m |= SelectMode::EDGE,
            "FACE" => m |= SelectMode::FACE,
            _ => {}
        }
    }
    m
}

#[operator(
    poll = ACTIVE_EDITABLE,
    tier = T0,
    support = Partial("as object.mode_set: mode handlers exist for OBJECT, EDIT (meshes and armatures) and POSE (armatures)"),
    notes = ["mesh_select_mode sets the select mode of every mesh in Edit Mode and of the tool settings (no selection conversion)"],
    dyn_enums = [("mode", mode_items)]
)]
fn exec(cx: &mut OpCtx, p: object::ModeSetWithSubmode) -> OpResult {
    let def = frap_op::lookup("object.mode_set").map_err(|e| OpError::Runtime(e.message))?;
    let (args, _, _) = frap_op::convert_args(
        def.spec,
        Vec::new(),
        vec![
            ("mode".into(), Input::Str(p.mode.value.identifier().into())),
            ("toggle".into(), Input::Bool(p.toggle.value)),
        ],
    )
    .map_err(|e| OpError::Runtime(e.message))?;
    if matches!(cx.run_op("object.mode_set", args)?, OpStatus::Cancelled) {
        return Ok(OpStatus::Cancelled);
    }
    let in_edit_mesh = poll::active(&cx.ctx())
        .is_some_and(|(_, o)| o.object_type() == "MESH" && o.mode == ObjectMode::Edit);
    let mode = select_mode(&p.mesh_select_mode.value);
    if !in_edit_mesh || !p.mesh_select_mode.is_set || mode.is_empty() {
        return Ok(OpStatus::Finished);
    }
    // `EDBM_selectmode_set_multi_ex`.
    let scene = cx.ctx().win.scene;
    let bits = [
        mode.contains(SelectMode::VERT),
        mode.contains(SelectMode::EDGE),
        mode.contains(SelectMode::FACE),
    ];
    {
        let (main, log) = cx.main_mut();
        if main
            .scenes
            .get(scene)
            .is_some_and(|s| s.tool_settings.mesh_select_mode != bits)
            && let Some(s) = main.scenes.get_mut(scene, log)
        {
            s.tool_settings.mesh_select_mode = bits;
            log.context_changed = true;
        }
    }
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let current = {
            let (_, edit) = cx.split_edit();
            edit.meshes.get(&me.key()).map(|em| em.bm.select_mode)
        };
        if current.is_some_and(|m| m != mode) {
            cx.bmesh_mut(me)?.selectmode_set(mode);
        }
    }
    Ok(OpStatus::Finished)
}
