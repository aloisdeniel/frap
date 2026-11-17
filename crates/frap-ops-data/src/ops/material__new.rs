// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.material.new()`, tier T0 (`tests/golden/cases/object/slots/material_new.py`).
//! Port of `new_material_exec` and `ED_node_shader_default` (`editors/render/
//! render_shading.cc`, `editors/space_node/node_edit.cc`, Blender 5.2.4, GPL-2.0-or-later).
//! Verified on 5.2.4:
//! - adds `Material` (uniquified) with the default surface graph, identical to
//!   `bpy.data.materials.new('Material')` (Principled BSDF at (−200, 100) → Material Output at
//!   (200, 100), same socket values and material settings);
//! - from a script it is **not** assigned to any slot: the assignment goes through the
//!   Properties editor's `template_ID` button, which scripts do not have;
//! - poll `object_materials_supported_poll`: an active object (`Context missing active
//!   object`), local, not hidden in the viewport (`Cannot edit hidden object`; `hide_set` does
//!   not count), of a type with materials (meshes in frap; a camera fails with `failed,
//!   context is incorrect`).
//!
//! Deviation: Blender's new material has one (dangling) user, so it survives a save; frap's has
//! none, like `bpy.data.materials.new`, and an unassigned one is dropped on save (§5.5).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::id::{Material, SlotLink, slots};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_data::material;

/// `object_materials_supported_poll` with Blender's messages (the same check as
/// `object.material_slot_add`, frap-ops-object).
fn supported(c: &Context) -> Result<(), PollFail> {
    poll::active_editable(c).map_err(|mut f| {
        let why = match poll::active(c) {
            None => "Context missing active object",
            Some((_, ob)) if ob.header.library.is_some() => {
                "Cannot edit library linked or non-editable override object"
            }
            Some(_) => "Cannot edit hidden object",
        };
        f.message = Some(format!("Operator bpy.ops.material.new.poll() {why}"));
        f
    })?;
    let (_, ob) = poll::active(c).expect("invariant: checked by active_editable");
    if ob.object_type() != "MESH" {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh object active, or use bpy.data.materials.new('Name')",
        ));
    }
    let act = ob.active_material_index.max(0) as usize;
    let object_linked = act < slots::slot_count(c.main, ob)
        && ob
            .material_slot_overrides
            .get(act)
            .is_some_and(|s| s.link == SlotLink::Object);
    let data_local = ob
        .data
        .and_then(|d| c.main.header(d))
        .is_some_and(|h| h.library.is_none());
    if object_linked || data_local || ob.mode == ObjectMode::Edit {
        return Ok(());
    }
    Err(poll::fail(
        PollReq::ActiveEditable,
        "the object's data is linked from a library",
    ))
}

/// `object_materials_supported_poll`
const SUPPORTED: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: supported,
};

#[operator(
    poll = SUPPORTED,
    tier = T0,
    spec = material::OPSPEC_MATERIAL_OT_NEW,
    notes = [
        "does not assign the material to a slot from scripts (as Blender); use mesh.materials.append(mat)",
    ]
)]
fn exec(cx: &mut OpCtx, _p: material::MaterialNew) -> OpResult {
    let mut m = Material::new("Material");
    m.node_tree.add_default_material_graph();
    let (main, log) = cx.main_mut();
    let r = main.materials.insert("Material", m, log);
    cx.result("material", r.any());
    Ok(OpStatus::Finished)
}
