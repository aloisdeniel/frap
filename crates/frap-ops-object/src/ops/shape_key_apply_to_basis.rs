// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_apply_to_basis()`, tier T1 (`tests/golden/cases/object/
//! shape_keys_m8/make_basis_apply.py`). Port of `shape_key_apply_to_basis_exec` /
//! `shape_key_apply_to_basis_poll` (`editors/object/object_shapekey.cc`) with
//! `BKE_key_evaluate_object_ex(…, keys_to_process)` and `BKE_keyblock_get_dependent_keys`
//! (`blenkernel/intern/key.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The selected blocks and the active one, except locked ones (`Skipped N locked shape keys`,
//! INFO), are mixed into the basis at their current values (with their vertex groups) and
//! removed; the mesh takes the new basis. Every block that depends on the basis (relative to
//! it, directly or through other blocks) moves by the same offsets, so its own shape is kept
//! on top of the new basis. The basis becomes active. Nothing to apply: `CANCELLED`.
//!
//! Poll `shape_key_apply_to_basis_poll`: `shape_key_exists_poll` on a mesh with an active
//! block that is not the basis.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::Key;
use frap_spec_ops_object::object;

use super::shape_key_add::common;
use super::shape_key_make_basis::not_basis_poll;
use crate::support::weights;

fn apply_poll(c: &Context) -> Result<(), PollFail> {
    common::shape_key_poll(c)?;
    let (_, ob) = poll::active(c).expect("invariant: checked by shape_key_poll");
    if ob.object_type() != "MESH" {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "shape keys are applied on meshes: make a mesh active",
        ));
    }
    not_basis_poll(c)
}

/// `shape_key_apply_to_basis_poll`.
const APPLY_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: apply_poll,
};

/// `BKE_keyblock_get_dependent_keys(key, index)`: the blocks relative to block `index`,
/// directly or through other blocks (`index` itself excluded); `None` for an absolute key or
/// when no block depends on it.
pub(crate) fn dependent_keys(key: &Key, index: usize) -> Option<Vec<bool>> {
    if !key.use_relative {
        return None;
    }
    let count = key.key_blocks.len();
    if index >= count {
        return None;
    }
    let mut marked = vec![false; count];
    marked[index] = true;
    let mut found = false;
    loop {
        let mut updated = false;
        for i in 0..count {
            let r = key.key_blocks[i].relative_key;
            if !marked[i] && r < count && marked[r] {
                marked[i] = true;
                updated = true;
                found = true;
            }
        }
        if !updated {
            break;
        }
    }
    if !found {
        return None;
    }
    marked[index] = false;
    Some(marked)
}

#[operator(poll = APPLY_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::ShapeKeyApplyToBasis) -> OpResult {
    let Some((ob, o)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let (shapenr, shape_lock) = (
        i64::from(o.active_shape_key_index) + 1,
        o.show_only_shape_key,
    );
    let Some((ob, me)) = weights::active_mesh(cx).filter(|(r, _)| *r == ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&cx.ctx(), ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(key) = cx.main().shape_keys.get(k).cloned() else {
        return Ok(OpStatus::Cancelled);
    };
    let mut locked = 0;
    let process: Vec<bool> = (0..key.key_blocks.len())
        .map(|i| {
            if !common::is_selected(&key, i, shapenr) {
                return false;
            }
            if key.key_blocks[i].lock_shape {
                locked += 1;
                return false;
            }
            true
        })
        .collect();
    if locked != 0 {
        cx.report(
            ReportType::Info,
            format!("Skipped {locked} locked shape keys"),
        );
    }
    if !process.contains(&true) {
        return Ok(OpStatus::Cancelled);
    }
    // `BKE_key_evaluate_object_ex(…, keys_to_process)` into the mesh positions: the relative
    // mix skips the blocks not to process (muting them is the same there); absolute keys and
    // `show_only_shape_key` ignore the mask.
    let mut masked = key.clone();
    if masked.use_relative && !shape_lock {
        for (kb, p) in masked.key_blocks.iter_mut().zip(&process) {
            if !p {
                kb.mute = true;
            }
        }
    }
    let active = usize::try_from(shapenr - 1)
        .ok()
        .filter(|&a| a < key.key_blocks.len());
    let (positions, mixed) = {
        let Some(m) = cx.main().meshes.get(me) else {
            return Ok(OpStatus::Cancelled);
        };
        let mixed = masked.evaluate_mesh(m.verts_num(), active, shape_lock, &|n| {
            weights::group_weights(m, n)
        });
        (m.positions().to_vec(), mixed)
    };
    let new_positions = mixed.unwrap_or(positions);
    let basis = &key.key_blocks[0].data;
    let translations: Vec<[f32; 3]> = new_positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let b = basis.get(i).copied().unwrap_or([0.0; 3]);
            [p[0] - b[0], p[1] - b[1], p[2] - b[2]]
        })
        .collect();
    {
        let m = cx.mesh_mut(me)?;
        let pos = m.positions_mut();
        let n = pos.len().min(new_positions.len());
        pos[..n].copy_from_slice(&new_positions[..n]);
    }
    let (main, log) = cx.main_mut();
    if let Some(key) = main.shape_keys.get_mut(k, log)
        && let Some(b) = key.key_blocks.first_mut()
    {
        let n = b.data.len().min(new_positions.len());
        b.data[..n].copy_from_slice(&new_positions[..n]);
    }
    // Removing in descending order keeps the lower indices valid; the outcome does not depend
    // on the order.
    for i in (0..process.len()).rev() {
        if process[i] {
            Key::object_remove(main, log, ob, i);
        }
    }
    if let Some(k2) = main
        .meshes
        .get(me)
        .and_then(|m| m.shape_keys)
        .filter(|k2| main.shape_keys.get(*k2).is_some())
        && let Some(key) = main.shape_keys.get_mut(k2, log)
        && let Some(dependent) = dependent_keys(key, 0)
    {
        // The basis itself is never marked.
        for (i, kb) in key.key_blocks.iter_mut().enumerate() {
            if !dependent[i] {
                continue;
            }
            for (c, t) in kb.data.iter_mut().zip(&translations) {
                c[0] += t[0];
                c[1] += t[1];
                c[2] += t[2];
            }
        }
    }
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.active_shape_key_index = 0;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::id::ShapeKey;

    #[test]
    fn dependent_keys_follow_relative_chains() {
        let mut key = Key::new("Key");
        key.use_relative = true;
        for (i, rel) in [0usize, 0, 1, 3].iter().enumerate() {
            let mut kb = ShapeKey::new(format!("K{i}"), Vec::new());
            kb.relative_key = *rel;
            key.key_blocks.push(kb);
        }
        // K1 -> K0, K2 -> K1 -> K0; K3 is relative to itself.
        assert_eq!(dependent_keys(&key, 0), Some(vec![false, true, true, false]));
        assert_eq!(dependent_keys(&key, 2), None);
        key.use_relative = false;
        assert_eq!(dependent_keys(&key, 0), None);
    }
}
