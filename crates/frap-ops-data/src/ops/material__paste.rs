// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.material.paste()`, tier T0 (`tests/golden/cases/object/data_m8/material_copy.py`).
//! Port of `paste_material_exec`, `paste_material_nodetree_ids_relink_or_clear` and
//! `object_materials_supported_poll` (`editors/render/render_shading.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4 (with the `material` context member):
//! - the material copy buffer (see `material.copy`) replaces the material's node tree and its
//!   settings `paste_material_exec` swaps: viewport display colour, metallic and roughness,
//!   `pass_index`, `alpha_threshold`, `blend_method`, `use_backface_culling` (and the other
//!   `blend_flag` bits); `surface_render_method` and the material's own animation stay; the
//!   node tree keeps the material's node-tree animation;
//! - the data-blocks the pasted nodes referenced are relinked **by name** to this file's
//!   (images, node groups, texts and objects, the first one of that name; frap has no texts),
//!   or cleared when there is none (and for every other type);
//! - an empty buffer: `RuntimeError: Error: Internal clipboard is empty`; no material:
//!   WARNING `Cannot paste without a material`, `CANCELLED`;
//! - poll `object_materials_supported_poll` (an active, local, visible object of a type with
//!   materials whose active slot is object-linked or whose data is local).
//!
//! **Headless-extended** (D-159): frap pastes into the active object's active material, the
//! `material` context member of the Properties editor.

use std::collections::HashMap;

use frap_nodes::{NodeTree, SocketValue};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdCode, Value};
use frap_scene::id::{SlotLink, slots};
use frap_scene::{Context, Main, ObjectMode};
use frap_spec_ops_data::material;

use super::material__copy::{BUFFER, context_material};

/// `object_materials_supported_poll` with Blender's messages.
fn supported(c: &Context) -> Result<(), PollFail> {
    poll::active_editable(c).map_err(|mut f| {
        let why = match poll::active(c) {
            None => "Context missing active object",
            Some((_, ob)) if ob.header.library.is_some() => {
                "Cannot edit library linked or non-editable override object"
            }
            Some(_) => "Cannot edit hidden object",
        };
        f.message = Some(format!("Operator bpy.ops.material.paste.poll() {why}"));
        f
    })?;
    let (_, ob) = poll::active(c).expect("invariant: checked by active_editable");
    if !matches!(ob.object_type(), "MESH" | "POINTCLOUD") {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make an object with materials (a mesh) active",
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
        "the active object's data is linked from a library",
    ))
}

/// `object_materials_supported_poll`.
const SUPPORTED_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: supported,
};

/// The ID types `paste_material_exec` reads from the buffer (`ntree_filter`; frap has no texts).
fn kept(code: IdCode) -> bool {
    matches!(code, IdCode::IM | IdCode::NT | IdCode::OB)
}

/// `paste_material_nodetree_ids_relink_or_clear`: every ID pointer of the pasted tree, by the
/// name it had at copy time, to the first local data-block of that type and name, else none.
fn relink(tree: &mut NodeTree, main: &Main, names: &[(IdAny, IdCode, smol_str::SmolStr)]) {
    let map: HashMap<IdAny, Option<IdAny>> = names
        .iter()
        .map(|(id, code, name)| {
            let to = if kept(*code) {
                main.find(code.collection(), name)
            } else {
                None
            };
            (*id, to)
        })
        .collect();
    let target = |id: IdAny| map.get(&id).copied().flatten();
    for n in &mut tree.nodes {
        for v in n.props.values.values_mut() {
            if let Value::Id(Some(id)) = v {
                *v = Value::Id(target(*id));
            }
        }
        for s in n.inputs.iter_mut().chain(&mut n.outputs) {
            if let SocketValue::Id(Some(id)) = s.default_value {
                s.default_value = SocketValue::Id(target(id));
            }
        }
    }
}

#[operator(
    poll = SUPPORTED_POLL,
    tier = T0,
    support = Partial("headless-extended (D-159): pastes into the active object's active material"),
    notes = [
        "the pasted node tree's images, node groups, texts and objects are relinked by name, other data-blocks cleared, as in Blender",
    ]
)]
fn exec(cx: &mut OpCtx, _p: material::Paste) -> OpResult {
    let Some(ma) = context_material(&cx.ctx()) else {
        cx.report(ReportType::Warning, "Cannot paste without a material");
        return Ok(OpStatus::Cancelled);
    };
    let Some(buffer) = BUFFER.with(|b| b.borrow().clone()) else {
        return Err(OpError::Runtime("Internal clipboard is empty".into()));
    };
    let from = buffer.material;
    let mut tree = from.node_tree;
    relink(&mut tree, cx.main(), &buffer.names);
    let (main, log) = cx.main_mut();
    let Some(m) = main.materials.get_mut(ma, log) else {
        return Ok(OpStatus::Cancelled);
    };
    // The embedded tree keeps its identity and the material's node-tree animation
    // (`std::swap(ma->nodetree->adt, ma_from->nodetree->adt)`).
    tree.header = m.node_tree.header.clone();
    tree.animation_data = m.node_tree.animation_data.take();
    m.node_tree = tree;
    // `SWAP_MEMBER`: flag, r/g/b/a, spec, roughness, metallic, index, line art, alpha
    // threshold, refraction depth, blend method, blend shadow and blend flags.
    m.diffuse_color = from.diffuse_color;
    m.metallic = from.metallic;
    m.roughness = from.roughness;
    m.pass_index = from.pass_index;
    m.alpha_threshold = from.alpha_threshold;
    m.blend_method = from.blend_method;
    m.use_backface_culling = from.use_backface_culling;
    Ok(OpStatus::Finished)
}
