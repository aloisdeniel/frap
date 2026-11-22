// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.stitch(use_limit=False, snap_islands=True, limit=0.01, static_island=0,
//! active_object_index=0, midpoint_snap=False, clear_seams=True, mode='VERTEX',
//! stored_mode='VERTEX', selection=[], objects_selection_count=(0, 0, 0, 0, 0, 0))`, tier T1
//! (`tests/golden/cases/uv/edit_m8/stitch*.py`). Port of `stitch_exec`,
//! `stitch_init_from_operator`, `stitch_settings_init_for_operator`, `stitch_init_all` and
//! `stitch_extract_rna_selection` (`editors/uvedit/uvedit_smart_stitch.cc`), Blender 5.2.4,
//! GPL-2.0-or-later; [`state`] builds the per-mesh data, [`process`] stitches, [`element_map`]
//! is `BM_uv_element_map_create`.
//!
//! Poll `ED_operator_uvedit`. The selected UVs (vertex mode: every unique UV at a selected
//! corner's vertex; edge mode: the selected UV edges and their coincident twins) of the visible
//! faces of every mesh in Edit Mode with a UV map are joined to the UVs they share a mesh
//! vertex (or edge) with on another island: onto the static island (`static_island`, the next
//! stitchable island when that one has nothing to stitch) or at their midpoint
//! (`midpoint_snap`), only within `limit` with `use_limit`. With `snap_islands` the other
//! islands follow, translated by the mean offset and rotated by the mean angle between their
//! boundary normals (vertex mode) or edges. Stitched corners are UV-selected (with sync
//! selection only the UV flags change, as in Blender); `clear_seams` clears the seams of
//! stitched edges. `mode` defaults to the select mode (vertex with `uv_select_mode='VERTEX'`,
//! or the mesh's vertex mode with sync selection, else edge). `selection` (with
//! `objects_selection_count`, one count per mesh) replaces the selection by
//! `{'face_index': f, 'element_index': i}` corners read in `stored_mode`, as Blender's redo
//! does. Only the active object's islands (`active_object_index`) are stitched. `CANCELLED`
//! when no mesh has a visible UV. The unwrap "Original Bounds" variant
//! (`uv_stitch_selected_islands_for_original_bounds`) belongs to `uv.unwrap`.

pub(crate) mod element_map;
pub(crate) mod ghash;
pub(crate) mod process;
pub(crate) mod state;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use super::weld::common;
use frap_bmesh::SelectMode;

use crate::select::{Ts, UV_SELECT_VERT, UvSel, op};
use state::{Mode, Ssc, StitchState, UvElementId};

/// `RNA_MAX_ARRAY_LENGTH`.
const RNA_MAX_ARRAY_LENGTH: usize = 64;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Stitch) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    if objects.is_empty() {
        cx.report(ReportType::Error, "No objects selected");
        return Ok(OpStatus::Cancelled);
    }
    if objects.len() > RNA_MAX_ARRAY_LENGTH {
        cx.report(
            ReportType::Error,
            format!(
                "Stitching only works with less than {RNA_MAX_ARRAY_LENGTH} objects selected ({} selected)",
                objects.len()
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    let mode = if p.mode.is_set {
        mode_of(p.mode.value.identifier())
    } else if ts.sync {
        if ts.selectmode.contains(SelectMode::VERT) {
            Mode::Vert
        } else {
            Mode::Edge
        }
    } else if ts.uv_selectmode & UV_SELECT_VERT != 0 {
        Mode::Vert
    } else {
        Mode::Edge
    };
    let mut ssc = Ssc {
        clear_seams: p.clear_seams.value,
        use_limit: p.use_limit.value,
        limit_dist: p.limit.value,
        snap_islands: p.snap_islands.value,
        midpoints: p.midpoint_snap.value,
        mode,
        static_island: p.static_island.value,
        active_object_index: p.active_object_index.value,
    };
    let stored_mode = mode_of(p.stored_mode.value.identifier());

    // `stitch_extract_rna_selection`: the stored selection, sliced per object.
    let to_select: Option<Vec<Vec<UvElementId>>> =
        if p.selection.is_set && p.objects_selection_count.is_set {
            let all = selection_items(&p.selection.value)?;
            let counts = p.objects_selection_count.value;
            let mut offset = 0usize;
            let mut per_ob = Vec::with_capacity(objects.len());
            for i in 0..objects.len() {
                let n = counts.get(i).copied().unwrap_or(0).max(0) as usize;
                let end = (offset + n).min(all.len());
                per_ob.push(all[offset.min(end)..end].to_vec());
                offset = end;
            }
            Some(per_ob)
        } else {
            None
        };

    // `stitch_init_all`.
    let base = uvedit::path_base(cx);
    let mut states: Vec<(IdRef<MeshId>, StitchState)> = Vec::new();
    for (ob_index, &(ob, me)) in objects.iter().enumerate() {
        let Some(bm) = op::edit_bm(cx, me) else {
            continue;
        };
        let aspect = uvedit::aspect_y(cx.main(), &base, ob, &bm);
        let sel = to_select
            .as_ref()
            .map(|v| v[ob_index].clone())
            .unwrap_or_default();
        let st = with_sel(cx, me, ts, None, |s| {
            let mut st = StitchState::init(s, &ssc, aspect, stored_mode, &sel)?;
            process::process_data(&mut ssc, &mut st, s, false, false);
            st.sync_valid = s.sync_valid;
            Some(st)
        })?
        .flatten();
        if let Some(st) = st {
            states.push((me, st));
        }
    }
    if states.is_empty() {
        return Ok(OpStatus::Cancelled);
    }
    ssc.active_object_index %= states.len() as i32;
    let active = ssc.active_object_index as usize;
    ssc.static_island %= states[active].1.map.total_islands as i32;
    if !states[active].1.island_is_stitchable[ssc.static_island as usize] {
        let only: Vec<&StitchState> = states.iter().map(|(_, st)| st).collect();
        state::goto_next_island(&mut ssc, &only);
    }
    let active = ssc.active_object_index as usize;
    // Process the active object again now that it is the active one.
    run(cx, ts, &mut ssc, &mut states[active], true, false)?;

    // `stitch_exec`: `stitch_process_data_all(ssc, scene, final=true)`.
    for i in 0..states.len() {
        run(cx, ts, &mut ssc, &mut states[i], i == active, true)?;
    }
    Ok(OpStatus::Finished)
}

/// `stitch_process_data` on one state, writing the UVs back after the final pass.
fn run(
    cx: &mut OpCtx,
    ts: Ts,
    ssc: &mut Ssc,
    entry: &mut (IdRef<MeshId>, StitchState),
    is_active: bool,
    final_: bool,
) -> Result<(), OpError> {
    let (me, st) = entry;
    let name = common::uv_map_name(cx, *me);
    let sync_valid = st.sync_valid;
    with_sel(cx, *me, ts, Some(sync_valid), |s| {
        process::process_data(ssc, st, s, is_active, final_);
        if final_
            && is_active
            && let Some(name) = &name
        {
            common::write_back(s, name);
        }
    })?;
    Ok(())
}

/// Runs `f` on the UV selection of `me`'s edit BMesh, with `uv_select_sync_valid` restored.
fn with_sel<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    ts: Ts,
    sync_valid: Option<bool>,
    f: impl FnOnce(&mut UvSel) -> R,
) -> Result<Option<R>, OpError> {
    let bm = cx.bmesh_mut(me)?;
    let Some(mut s) = UvSel::new(ts, bm) else {
        return Ok(None);
    };
    if let Some(v) = sync_valid {
        s.sync_valid = v;
    }
    Ok(Some(f(&mut s)))
}

/// `stitch_modes` identifier.
fn mode_of(id: &str) -> Mode {
    if id == "EDGE" { Mode::Edge } else { Mode::Vert }
}

/// The `selection` collection as `(face_index, element_index)` pairs
/// (`SelectedUvElement`: `name`, `face_index`, `element_index`; `pyrna_pydict_to_props`
/// wants every one of them and nothing else).
fn selection_items(v: &Value) -> Result<Vec<UvElementId>, OpError> {
    let Value::Structs(items) = v else {
        return Ok(Vec::new());
    };
    let err = |what: String| {
        type_error(format!(
            "Converting py args to operator properties: UV_OT_stitch.selection error \
             converting a member of a collection from a dicts into an RNA collection, failed \
             with: TypeError: Converting a Python list to an RNA collection: {what}"
        ))
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let mut index = [0i32; 2];
        for (slot, key) in ["name", "face_index", "element_index"].into_iter().enumerate() {
            let Some(val) = it.get(key) else {
                return Err(err(format!("keyword \"{key}\" missing")));
            };
            match (slot, val) {
                (0, Value::Str(_)) => {}
                (0, _) => {
                    return Err(err(format!(
                        "SelectedUvElement.name expected a string type"
                    )));
                }
                (_, Value::Int(i)) => {
                    index[slot - 1] = i32::try_from(*i).unwrap_or(i32::MAX);
                }
                (_, _) => {
                    return Err(err(format!("SelectedUvElement.{key} expected an int type")));
                }
            }
        }
        if let Some(k) = it
            .keys()
            .find(|k| !matches!(k.as_str(), "name" | "face_index" | "element_index"))
        {
            return Err(err(format!("keyword \"{k}\" is invalid for this struct")));
        }
        out.push((index[0], index[1]));
    }
    Ok(out)
}

fn type_error(msg: String) -> OpError {
    OpError::Frap(Box::new(frap_rna::FrapError::new(
        frap_rna::PyExc::TypeError,
        frap_op::errors::OP_ARG_TYPE,
        msg,
    )))
}
