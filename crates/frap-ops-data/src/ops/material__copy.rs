// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.material.copy()`, tier T0 (`tests/golden/cases/object/data_m8/material_copy.py`).
//! Port of `copy_material_exec` and `material_slot_populated_poll`
//! (`editors/render/render_shading.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: the operator copies the `material` context member (the material shown
//! in the Properties editor) into the material copy buffer and reports the INFO `Copied
//! material to internal clipboard`; it changes nothing (no undo step). The poll only needs an
//! active object with a material slot (`failed, context is incorrect`).
//!
//! **Headless-extended** (D-159): a 3D View has no `material` context member, so Blender's
//! script call cancels; frap copies the active object's active material
//! (`context.object.active_material`, what the Properties editor shows). An empty active slot
//! cancels.
//!
//! **The copy buffer** lives outside the undo state, in the session's thread
//! ([`BUFFER`]): undo, redo, opening or resetting a file leave it as it is, and a later
//! `material.paste` reads it. Blender keeps it in a file (`copybuffer_material.blend` in the
//! temporary directory), so its buffer also survives across Blender processes; frap's lasts
//! for the session. Like the file, the buffer is a snapshot: the material's settings and node
//! tree at copy time, with the names its node tree's data-blocks had then (the paste relinks
//! them by name).

use std::cell::RefCell;

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdCode, IdRef, MaterialId};
use frap_scene::Context;
use frap_scene::id::{Material, slots};
use frap_spec_ops_data::material;
use smol_str::SmolStr;

/// What `material.copy` wrote: the material as it was, and the type and name of every
/// data-block its node tree referenced (`PartialWriteContext` with `ADD_DEPENDENCIES`).
#[derive(Clone, Debug)]
pub(crate) struct CopyBuffer {
    /// The copied material (`ID_FLAG_CLIPBOARD_MARK`).
    pub(crate) material: Material,
    /// Each referenced data-block and its name at copy time.
    pub(crate) names: Vec<(IdAny, IdCode, SmolStr)>,
}

thread_local! {
    /// The material copy buffer (`copybuffer_material.blend`), outside every undo state.
    pub(crate) static BUFFER: RefCell<Option<CopyBuffer>> = const { RefCell::new(None) };
}

/// The material the operators work on: the `material` context member as the Properties
/// editor gives it, the active object's active slot material (D-159).
pub(crate) fn context_material(c: &Context) -> Option<IdRef<MaterialId>> {
    let ob = c.object().and_then(|o| c.main.objects.get(o))?;
    let n = slots::slot_count(c.main, ob);
    if n == 0 {
        return None;
    }
    let act = (ob.active_material_index.max(0) as usize).min(n - 1);
    slots::slot(c.main, ob, act).1
}

/// `material_slot_populated_poll`: an active object whose `actcol` is set.
fn populated(c: &Context) -> Result<(), PollFail> {
    poll::active_object(c)?;
    let (_, ob) = poll::active(c).expect("invariant: checked by active_object");
    if slots::slot_count(c.main, ob) == 0 {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "the active object needs a material slot (bpy.ops.object.material_slot_add())",
        ));
    }
    Ok(())
}

/// `material_slot_populated_poll`.
const POPULATED_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: populated,
};

#[operator(
    poll = POPULATED_POLL,
    tier = T0,
    support = Partial("headless-extended (D-159): copies the active object's active material"),
    notes = [
        "the copy buffer lives outside the undo state for the whole session (Blender keeps it in a file of the temporary directory)",
    ]
)]
fn exec(cx: &mut OpCtx, _p: material::Copy) -> OpResult {
    let Some(ma) = context_material(&cx.ctx()) else {
        cx.report(
            ReportType::Warning,
            "frap: nothing copied: the active material slot of the active object is empty",
        );
        return Ok(OpStatus::Cancelled);
    };
    let main = cx.main();
    let Some(m) = main.materials.get(ma) else {
        return Ok(OpStatus::Cancelled);
    };
    let mut names: Vec<(IdAny, IdCode, SmolStr)> = Vec::new();
    frap_rna::IdType::for_each_ref(&m.node_tree, &mut |id| {
        if let Some(h) = main.header(id)
            && !names.iter().any(|(r, _, _)| *r == id)
        {
            names.push((id, id.code, h.name.clone()));
        }
    });
    let buffer = CopyBuffer {
        material: m.clone(),
        names,
    };
    BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
    cx.report(ReportType::Info, "Copied material to internal clipboard");
    Ok(OpStatus::Finished)
}
