// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.add_named(linked=False, name='', session_uid=0, matrix=…, drop_x=0,
//! drop_y=0)`, tier T0 (`tests/golden/cases/object/add_m8/add_named*.py`). Ported from
//! blender/source/blender/editors/object/object_add.cc (`object_add_named_exec`,
//! `object_add_duplicate_internal`, `object_add_sync_base_collection`) and
//! windowmanager/intern/wm_operator_props.cc
//! (`WM_operator_properties_id_lookup_from_name_or_session_uid`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `ED_operator_objectmode_poll_msg` (`Only supported in object mode`);
//! - the object is looked up by `session_uid` when set, else by `name` when set; neither, or
//!   no such object → `RuntimeError: Error: Object not found`; an object in Pose Mode →
//!   `Object could not be duplicated`;
//! - it is duplicated as `object.duplicate` does (`linked=True`: the data is shared), linked to
//!   every collection of the scene holding the original when the original's base is enabled in
//!   the viewport, else to `context.collection`; the copy's `hide_viewport` is cleared; every
//!   other visible object is deselected, the copy selected and made active;
//! - a set `matrix` places the copy (`BKE_object_apply_mat4`, compatible Euler, through its
//!   parent): each of the four sequences of `matrix` is a column (`matrix[3][:3]` is the
//!   translation);
//! - `drop_x` / `drop_y` place the copy under the mouse in the 3D View region; frap's virtual
//!   3D View has no region size, so they answer `NotImplementedError` (deviations §2).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::id::layers;
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;
use glam::Mat4;

use super::duplicate::NewIds;
use super::move_to_collection::common::{all_scene_collections, has_object, link_object};
use super::select_grouped::bases::Bases;
use crate::support::xform::{Worlds, apply_mat4};

/// `WM_operator_properties_id_lookup_from_name_or_session_uid` (shared with
/// `collection_instance_add`).
pub(crate) mod lookup {
    use frap_rna::{IdKind, IdRef, IdType, Prop};
    use frap_scene::IdMap;
    use smol_str::SmolStr;

    /// The ID with this `session_uid` when the prop is set, else the one with this `name` when
    /// set, else `None` (`BKE_libblock_find_session_uid` / `BKE_libblock_find_name`).
    pub(crate) fn id<T: IdType>(
        map: &IdMap<T>,
        name: &Prop<SmolStr>,
        session_uid: &Prop<i32>,
    ) -> Option<IdRef<T::Kind>>
    where
        T::Kind: IdKind,
    {
        if session_uid.is_set {
            // `uint32_t(RNA_property_int_get(…))`.
            let uid = session_uid.value as u32;
            return map
                .iter()
                .find(|(_, t)| t.header().session_uid == uid)
                .map(|(r, _)| r);
        }
        if name.is_set {
            return map.find(&name.value);
        }
        None
    }

    /// Whether `drop_x` or `drop_y` is set (`object_add_drop_xy_is_set`).
    pub(crate) fn drop_is_set(drop_x: &Prop<i32>, drop_y: &Prop<i32>) -> bool {
        drop_x.is_set || drop_y.is_set
    }
}

/// `ED_operator_objectmode_poll_msg`.
fn objectmode_msg(c: &Context) -> Result<(), PollFail> {
    poll::objectmode(c).map_err(|mut f| {
        f.message =
            Some("Operator bpy.ops.object.add_named.poll() Only supported in object mode".into());
        f
    })
}

/// `ED_operator_objectmode_poll_msg`
const OBJECTMODE_MSG: Poll = Poll {
    requires: &[PollReq::SceneEditable, PollReq::ModeObject],
    check: objectmode_msg,
};

#[operator(
    poll = OBJECTMODE_MSG,
    tier = T0,
    support = Partial("drop_x/drop_y (placement under the mouse) answer OP_NOT_IMPLEMENTED: the virtual 3D View has no region"),
    notes = ["looks the object up by session_uid when set, else by name; duplicates it as object.duplicate does"]
)]
fn exec(cx: &mut OpCtx, p: object::AddNamed) -> OpResult {
    let Some(src) = lookup::id(&cx.main().objects, &p.name, &p.session_uid) else {
        return Err(OpError::Runtime("Object not found".into()));
    };
    if !p.matrix.is_set && lookup::drop_is_set(&p.drop_x, &p.drop_y) {
        return Err(OpError::NotImplemented(
            "frap: add_named(drop_x=…, drop_y=…) places the object under the mouse in a 3D View region, which frap's virtual 3D View does not have; pass matrix=… instead".into(),
        ));
    }
    let linked = p.linked.value;
    let (scene, base_enabled, active_coll, worlds) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let enabled = vl.base(src).is_some()
            && c.base_visibility()
                .get(&src)
                .is_some_and(|v| v.enabled_viewport);
        (c.win.scene, enabled, c.collection(), Worlds::of(c.main))
    };
    if cx
        .main()
        .objects
        .get(src)
        .is_some_and(|o| o.mode == ObjectMode::Pose)
    {
        return Err(OpError::Runtime("Object could not be duplicated".into()));
    }
    let new = {
        let (main, log) = cx.main_mut();
        let mut ids = NewIds::default();
        let data = main.objects.get(src).and_then(|o| o.data);
        let Some(new) = ids
            .duplicate(main, log, src.any(), !linked)
            .typed::<ObjectId>()
            .filter(|n| *n != src)
        else {
            return Err(OpError::Runtime("Object could not be duplicated".into()));
        };
        if !linked && let Some(d) = data {
            let new_data = ids.duplicate(main, log, d, true);
            if let Some(o) = main.objects.get_mut(new, log) {
                o.data = Some(new_data);
            }
        }
        // `object_add_sync_base_collection`.
        if base_enabled {
            let mut linked_somewhere = false;
            for c in all_scene_collections(main, scene) {
                if has_object(main, scene, c, src) && link_object(main, log, scene, c, new) {
                    linked_somewhere = true;
                }
            }
            if !linked_somewhere {
                link_object(main, log, scene, None, new);
            }
        } else {
            link_object(main, log, scene, active_coll, new);
        }
        if let Some(o) = main.objects.get_mut(new, log) {
            o.hide_viewport = false;
        }
        ids.relink(main, log);
        layers::sync_all(main, log);
        new
    };
    let mut b = Bases::load(cx);
    b.deselect_all();
    if let Some(i) = b.find(new) {
        b.select(i, true);
        b.active = Some(new);
    }
    b.commit(cx);
    if p.matrix.is_set {
        // frap's matrix values arrive row by row (`frap_rna::Value::FloatArray`).
        place(
            cx,
            new,
            Mat4::from_cols_array_2d(&p.matrix.value).transpose(),
            &worlds,
        )?;
    }
    cx.result("object", new.any());
    Ok(OpStatus::Finished)
}

/// `BKE_object_apply_mat4(ob_add, matrix, true, true)`.
fn place(cx: &mut OpCtx, ob: IdRef<ObjectId>, m: Mat4, worlds: &Worlds) -> Result<(), OpError> {
    let parent_world = {
        let main = cx.main();
        main.objects
            .get(ob)
            .and_then(|o| worlds.parent_matrix(main, o))
    };
    let o = cx.object_mut(ob)?;
    apply_mat4(o, &m, true, parent_world.as_ref());
    Ok(())
}
