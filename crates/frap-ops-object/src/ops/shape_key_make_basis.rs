// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_make_basis()`, tier T1 (`tests/golden/cases/object/shape_keys_m8/
//! make_basis_apply.py`). Port of `shape_key_make_basis_exec` / `shape_key_make_basis_poll`
//! (`editors/object/object_shapekey.cc`) and `BKE_keyblock_move` (`blenkernel/intern/key.cc`),
//! Blender 5.2.4, GPL-2.0-or-later: the active block moves to the top of the list
//! (`relative_key` indices and the active index follow), the new and the old basis become
//! relative to the new basis, and the mesh takes the new basis' coordinates (so new shape keys
//! start from it). The block is applied at 100%, whatever its value.
//!
//! Poll `shape_key_make_basis_poll`: `shape_key_exists_poll` (Edit Mode allowed) with an
//! active block that is not the basis.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::shape_key_lock::exists_poll;
use crate::support::weights;

/// `shape_key_make_basis_poll` (and `shape_key_apply_to_basis_poll` without its mesh check):
/// [`exists_poll`] and `shapenr > 1`.
pub(crate) fn not_basis_poll(c: &Context) -> Result<(), PollFail> {
    exists_poll(c)?;
    let (ob, _) = poll::active(c).expect("invariant: checked by exists_poll");
    if weights::active_block(c, ob).is_none_or(|(_, a)| a == 0) {
        return Err(PollFail {
            message: None,
            unmet: smallvec![],
            hint: Some(
                "make another shape key active: bpy.context.object.active_shape_key_index = 1"
                    .into(),
            ),
        });
    }
    Ok(())
}

/// `shape_key_make_basis_poll`.
const MAKE_BASIS_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: not_basis_poll,
};

#[operator(poll = MAKE_BASIS_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::ShapeKeyMakeBasis) -> OpResult {
    let Some((ob, o)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let mut active = i64::from(o.active_shape_key_index);
    let Some((k, _)) = weights::active_block(&cx.ctx(), ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let me = weights::active_mesh(cx).map(|(_, me)| me);
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    if !key.move_block(None, 0, &mut active) {
        return Ok(OpStatus::Cancelled);
    }
    // The old basis is now second: every block above the moved one shifted down by one.
    key.key_blocks[0].relative_key = 0;
    if let Some(old) = key.key_blocks.get_mut(1) {
        old.relative_key = 0;
    }
    let basis = key.key_blocks[0].data.clone();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.active_shape_key_index = active as i32;
    }
    // `BKE_keyblock_convert_to_mesh`: as many positions as both have.
    if let Some(me) = me
        && let Some(m) = main.meshes.get_mut(me, log)
    {
        let pos = m.positions_mut();
        let n = basis.len().min(pos.len());
        pos[..n].copy_from_slice(&basis[..n]);
    }
    Ok(OpStatus::Finished)
}
