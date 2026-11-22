// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.rip(mirror=False, release_confirm=False, use_accurate=False, location=(0, 0))`,
//! tier T1 (`tests/golden/cases/uv/edit_m8/rip*.py`). Port of `uv_rip_exec`
//! (`editors/uvedit/uvedit_rip.cc`), Blender 5.2.4, GPL-2.0-or-later; [`rip_object`] rips one
//! mesh.
//!
//! Poll `ED_operator_uvedit`. Detaches the selected UVs of visible faces from the unselected
//! ones they share a position with, by deselecting the side away from `location` (the mouse
//! position in UV space; `uv.rip_move` then moves what stays selected): a selected UV edge
//! loop is split into its two sides and the side facing away from `location` is deselected; a
//! single selected UV keeps only the corners of the fan closest to `location`; selected faces
//! are isolated (their other UVs deselected). Raises `RuntimeError` `Edge must have connected
//! edges` / `Vertex must have connected vertices` when nothing can be ripped, and refuses the
//! `SHARED_VERTEX` sticky mode (`Rip is not compatible with vertex sticky selection`).
//! `CANCELLED` when no mesh changed.

pub(crate) mod rip_object;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use super::weld::common;
use crate::select::{Sticky, Ts, Uv, UvSel, op};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Rip) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    match rip_multi(cx, ts, p.location.value)? {
        Some(_) => Ok(OpStatus::Finished),
        None => Ok(OpStatus::Cancelled),
    }
}

/// The ripped meshes and their `uv_select_sync_valid`.
pub(crate) type Ripped = Vec<(IdRef<MeshId>, bool)>;

/// `uv_rip_exec` over every mesh in Edit Mode with a UV map: the meshes with their
/// `uv_select_sync_valid` after the rip (for `uv.rip_move`'s translation), `None` when no mesh
/// changed (`CANCELLED`).
pub(crate) fn rip_multi(cx: &mut OpCtx, ts: Ts, co: Uv) -> Result<Option<Ripped>, OpError> {
    if ts.sticky == Sticky::Vert {
        // "Rip" is logically incompatible with sync-select (and with vertex sticky selection).
        return Err(OpError::Runtime(
            "Rip is not compatible with vertex sticky selection".into(),
        ));
    }
    if ts.sync && ts.sync_uvselect_ignore() {
        return Err(OpError::Runtime(
            "Rip is only compatible with sync-select with vertex/edge selection".into(),
        ));
    }
    // Only the active object defines the UV image (`ED_uvedit_get_aspect`).
    let aspect_y = {
        let edit = poll::edit_object(&cx.ctx()).and_then(|(ob, o)| Some((ob, o.data?.typed()?)));
        match edit {
            Some((ob, me)) => match op::edit_bm(cx, me) {
                Some(bm) => {
                    let base = uvedit::path_base(cx);
                    uvedit::aspect_y(cx.main(), &base, ob, &bm)
                }
                None => 1.0,
            },
            None => 1.0,
        }
    };
    let objects = common::objects(cx);
    if ts.sync {
        // `EDBM_selectmode_set_multi_ex(scene, objects, ts->selectmode)`.
        for &(_, me) in &objects {
            let bm = cx.bmesh_mut(me)?;
            if bm.select_mode != ts.selectmode {
                bm.selectmode_set(ts.selectmode);
            }
        }
    }
    let mut changed_multi = false;
    let mut error: Option<rip_object::RipError> = None;
    let mut out = Vec::with_capacity(objects.len());
    for &(_, me) in &objects {
        let r = common::edit(cx, me, ts, |s: &mut UvSel| {
            let r = rip_object::rip_object(s, co, aspect_y);
            ((r, s.sync_valid), false)
        })?;
        let Some((r, sync_valid)) = r else { continue };
        match r {
            Ok(changed) => changed_multi |= changed,
            Err(e) => {
                error.get_or_insert(e);
            }
        }
        out.push((me, sync_valid));
    }
    if let Some(e) = error {
        // `BKE_report(RPT_ERROR)`: the Python call raises it.
        return Err(OpError::Runtime(e.message().into()));
    }
    Ok(changed_multi.then_some(out))
}
