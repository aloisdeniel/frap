// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_add(from_mix=True)`, tier T0 (`tests/golden/cases/weights_shapes/
//! shape_key_ops.py`). Port of `shape_key_add_exec` / `object_shape_key_add` and the shared
//! shape-key polls (`editors/object/object_shapekey.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! a block named `Basis` (first) or `Key N` holding the mesh (`from_mix=False`, and always for
//! the first block) or the current mix, with value 1, becomes the active block. Poll
//! `shape_key_mode_poll`: an editable active object with editable data, not in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::Key;
use frap_spec_ops_object::object;

/// The shape-key polls (`object_shapekey.cc`), shared by the `shape_key_*` operators.
pub(crate) mod common {
    use frap_op::prelude::*;
    use frap_op::{PollFail, PollReq};
    use frap_rna::{IdRef, KeyId, ObjectId};
    use frap_scene::id::Key;
    use frap_scene::{Context, ObjectMode};
    use smallvec::smallvec;

    /// `shape_key_poll`: an editable active object with editable data.
    pub(crate) fn shape_key_poll(c: &Context) -> Result<(), PollFail> {
        let Some((_, ob)) = poll::active(c) else {
            return Err(poll::fail(
                PollReq::ActiveObject,
                "make a mesh active: bpy.context.view_layer.objects.active = \
                 bpy.data.objects['Name']",
            ));
        };
        let data_editable = ob
            .data
            .and_then(|d| c.main.header(d))
            .is_some_and(|h| h.library.is_none());
        if ob.header.library.is_some() || !data_editable {
            return Err(poll::fail(
                PollReq::ActiveEditable,
                "the active object or its data is linked from a library",
            ));
        }
        Ok(())
    }

    /// `shape_key_mode_poll`: [`shape_key_poll`], not in Edit Mode.
    pub(crate) fn mode_poll(c: &Context) -> Result<(), PollFail> {
        shape_key_poll(c)?;
        let (_, ob) = poll::active(c).expect("invariant: checked by shape_key_poll");
        if ob.mode == ObjectMode::Edit {
            return Err(PollFail {
                message: None,
                unmet: smallvec![PollReq::ModeObject],
                hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
            });
        }
        Ok(())
    }

    /// The key of the active object's mesh.
    pub(crate) fn key_of(c: &Context, ob: IdRef<ObjectId>) -> Option<IdRef<KeyId>> {
        let o = c.main.objects.get(ob)?;
        let me = o.data?.typed::<frap_rna::MeshId>()?;
        let k = c.main.meshes.get(me)?.shape_keys?;
        c.main.shape_keys.get(k).map(|_| k)
    }

    fn no_block() -> PollFail {
        PollFail {
            message: None,
            unmet: smallvec![],
            hint: Some(
                "the active object has no shape key: bpy.ops.object.shape_key_add(from_mix=False)"
                    .into(),
            ),
        }
    }

    /// `shape_key_mode_exists_poll`: [`mode_poll`] and an active key block.
    pub(crate) fn mode_exists_poll(c: &Context) -> Result<(), PollFail> {
        mode_poll(c)?;
        let (ob, o) = poll::active(c).expect("invariant: checked by mode_poll");
        let exists = key_of(c, ob)
            .and_then(|k| c.main.shape_keys.get(k))
            .is_some_and(|k: &Key| {
                usize::try_from(o.active_shape_key_index).is_ok_and(|a| a < k.key_blocks.len())
            });
        if !exists {
            return Err(no_block());
        }
        Ok(())
    }

    /// `shape_key_move_poll`: [`mode_poll`] and at least two blocks.
    pub(crate) fn move_poll(c: &Context) -> Result<(), PollFail> {
        mode_poll(c)?;
        let (ob, _) = poll::active(c).expect("invariant: checked by mode_poll");
        if !key_of(c, ob)
            .and_then(|k| c.main.shape_keys.get(k))
            .is_some_and(|k| k.key_blocks.len() > 1)
        {
            return Err(no_block());
        }
        Ok(())
    }

    /// `shape_key_is_selected`: the block is selected or active (`shapenr` is 1-based).
    pub(crate) fn is_selected(key: &Key, i: usize, shapenr: i64) -> bool {
        key.key_blocks[i].select || i as i64 == shapenr - 1
    }
}

/// `shape_key_mode_poll`.
const ADD_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: add_poll,
};

fn add_poll(c: &Context) -> Result<(), PollFail> {
    common::mode_poll(c)
}

#[operator(poll = ADD_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyAdd) -> OpResult {
    let Some((ob, _)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    if let Some((k, i)) = Key::object_insert(main, log, ob, None, p.from_mix.value)
        && let Some(key) = main.shape_keys.get_mut(k, log)
    {
        // Shape keys created by this operator get the value 1.
        key.key_blocks[i].value = 1.0;
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.active_shape_key_index = i as i32;
        }
    }
    Ok(OpStatus::Finished)
}
