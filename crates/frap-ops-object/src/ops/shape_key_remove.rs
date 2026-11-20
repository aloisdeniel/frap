// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_remove(all=False, apply_mix=False)`, tier T0. Port of
//! `shape_key_remove_exec` (`editors/object/object_shapekey.cc`) and
//! `BKE_object_shapekey_remove/free` (`blenkernel/intern/object.cc`), Blender 5.2.4,
//! GPL-2.0-or-later:
//! - `all`: every block goes with the key (`apply_mix` first writes the current mix into the
//!   mesh); a locked block (`lock_shape`) refuses it: `RuntimeError: Error: The object Cube has
//!   locked shape keys`;
//! - otherwise the selected blocks and the active one go, locked ones are kept and reported
//!   (`Could not delete N locked shape key(s)`: WARNING when something was removed, else an
//!   error); removing the basis writes the next block into the mesh and stops "active" from
//!   following it; the active index moves down; the last block takes the key with it.
//!
//! Poll `shape_key_mode_exists_poll` (an active block, not in Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::Key;
use frap_spec_ops_object::object;

use super::shape_key_add::common;

fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::mode_exists_poll(c)
}

/// `shape_key_mode_exists_poll`.
const REMOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: remove_poll,
};

#[operator(poll = REMOVE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyRemove) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&c, ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let name = o.header.name.clone();
    let mut shapenr = i64::from(o.active_shape_key_index) + 1;
    let mut changed = false;
    if p.all.value {
        if c.main
            .shape_keys
            .get(k)
            .is_some_and(|key| key.key_blocks.iter().any(|kb| kb.lock_shape))
        {
            return Err(OpError::Runtime(format!(
                "The object {name} has locked shape keys"
            )));
        }
        if p.apply_mix.value
            && let Some(mix) = Key::evaluate_object(c.main, ob)
        {
            let me = o
                .data
                .and_then(|d| d.typed::<frap_rna::MeshId>())
                .ok_or_else(|| OpError::Runtime("the object has no mesh".into()))?;
            let pos = cx.mesh_mut(me)?.positions_mut();
            let n = mix.len().min(pos.len());
            pos[..n].copy_from_slice(&mix[..n]);
        }
        let (main, log) = cx.main_mut();
        changed = Key::object_free(main, log, ob);
    } else {
        let mut locked = 0usize;
        // `items_mutable()`: the blocks as they were, each looked up again by name.
        let names: Vec<_> = c
            .main
            .shape_keys
            .get(k)
            .map(|key| key.key_blocks.iter().map(|kb| kb.name.clone()).collect())
            .unwrap_or_default();
        for n in names {
            let (main, log) = cx.main_mut();
            let Some(key) = main.shape_keys.get(k) else {
                break;
            };
            let Some(cur) = key.find_block(&n) else {
                continue;
            };
            if !common::is_selected(key, cur, shapenr) {
                continue;
            }
            if key.key_blocks[cur].lock_shape {
                locked += 1;
                continue;
            }
            let last = key.key_blocks.len() == 1;
            changed |= Key::object_remove(main, log, ob, cur);
            // `BKE_object_shapekey_remove`'s own update of `shapenr`.
            if last {
                shapenr = 0;
            } else if shapenr > 1 {
                shapenr -= 1;
            }
            if cur == 0 {
                shapenr = 0;
            }
        }
        if locked > 0 {
            let msg = format!("Could not delete {locked} locked shape key(s)");
            if !changed {
                return Err(OpError::Runtime(msg));
            }
            cx.report(ReportType::Warning, msg);
        }
    }
    // Keep a block active while blocks remain.
    let remaining = common::key_of(&cx.ctx(), ob)
        .and_then(|k| cx.main().shape_keys.get(k))
        .map_or(0, |k| k.key_blocks.len());
    if shapenr == 0 && remaining > 0 {
        shapenr = 1;
    }
    if changed {
        cx.object_mut(ob)?.active_shape_key_index = (shapenr - 1) as i32;
        Ok(OpStatus::Finished)
    } else {
        Ok(OpStatus::Cancelled)
    }
}
