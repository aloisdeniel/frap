// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.remove_doubles(threshold=0.02, use_unselected=False, use_shared_vertex=False)`,
//! tier T1 (`tests/golden/cases/uv/edit_m8/remove_doubles*.py`). Port of
//! `uv_remove_doubles_exec`, `uv_remove_doubles_to_selected`,
//! `uv_remove_doubles_to_unselected` and `uv_remove_doubles_to_selected_shared_vertex`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. "Merge UVs by Distance" over every mesh in Edit Mode with a UV
//! map:
//! - default: the selected UVs of visible faces closer than `threshold` are clustered with
//!   `kdtree_calc_duplicates_fast` (2D kd-tree, [`kdtree`]) and each cluster moves to its
//!   average;
//! - `use_unselected`: each selected UV closer than `threshold` to a visible unselected UV
//!   snaps to the nearest one;
//! - `use_shared_vertex`: per mesh vertex, its selected UVs closer than `threshold` to the UV
//!   nearest their average are merged at their average, repeatedly.
//!
//! Always finishes.

pub(crate) mod kdtree;

use frap_bmesh::LoopId;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use super::weld::common;
use crate::select::{Uv, op};
use kdtree::KdTree2;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::RemoveDoubles) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let threshold = p.threshold.value;
    if p.use_unselected.value {
        to_unselected(cx, threshold)?;
    } else if p.use_shared_vertex.value {
        to_selected_shared_vertex(cx, threshold)?;
    } else {
        to_selected(cx, threshold)?;
    }
    Ok(OpStatus::Finished)
}

/// `uv_remove_doubles_to_selected`.
fn to_selected(cx: &mut OpCtx, threshold: f32) -> Result<(), OpError> {
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    // The visited UVs of every object (`uv_map_arr`, by object).
    let mut per_ob: Vec<(Vec<LoopId>, Vec<Uv>)> = Vec::new();
    for &(_, me) in &objects {
        let got = op::read(cx, me, ts, |s| {
            let loops = common::foreach_loops(s, true, true);
            let uvs = loops.iter().map(|&l| s.uv(l)).collect::<Vec<_>>();
            (loops, uvs)
        });
        per_ob.push(got.unwrap_or_default());
    }
    let total: usize = per_ob.iter().map(|(l, _)| l.len()).sum();
    let mut tree = KdTree2::new(total);
    let mut uv_map_arr: Vec<Uv> = Vec::with_capacity(total);
    let mut owner: Vec<usize> = Vec::with_capacity(total);
    for (ob_index, (_, uvs)) in per_ob.iter().enumerate() {
        for &uv in uvs {
            tree.insert(uv_map_arr.len() as i32, uv);
            uv_map_arr.push(uv);
            owner.push(ob_index);
        }
    }
    let mut duplicates = vec![-1i32; total];
    tree.balance();
    let found = tree.calc_duplicates_fast(threshold, &mut duplicates);
    if found <= 0 {
        return Ok(());
    }
    // Calculate average UV for duplicates.
    let mut count = vec![0i32; total];
    for i in 0..total {
        let d = duplicates[i];
        if d == -1 {
            count[i] += 1;
            continue;
        }
        let d = d as usize;
        if d != i {
            let add = uv_map_arr[i];
            uv_map_arr[d][0] += add[0];
            uv_map_arr[d][1] += add[1];
        }
        count[d] += 1;
    }
    for i in 0..total {
        if count[i] < 2 {
            continue;
        }
        let f = 1.0 / count[i] as f32;
        uv_map_arr[i][0] *= f;
        uv_map_arr[i][1] *= f;
    }
    let mut changed = vec![false; objects.len()];
    for i in 0..total {
        if duplicates[i] == -1 {
            continue;
        }
        uv_map_arr[i] = uv_map_arr[duplicates[i] as usize];
        changed[owner[i]] = true;
    }
    // Write the visited UVs back (all of them: the unchanged ones keep their value).
    let mut cursor = 0usize;
    for (ob_index, &(_, me)) in objects.iter().enumerate() {
        let (loops, _) = &per_ob[ob_index];
        let n = loops.len();
        let slice = &uv_map_arr[cursor..cursor + n];
        cursor += n;
        if !changed[ob_index] {
            continue;
        }
        common::edit(cx, me, ts, |s| {
            for (&l, &uv) in loops.iter().zip(slice) {
                s.uv[l.slot()] = uv;
            }
            ((), true)
        })?;
    }
    Ok(())
}

/// `uv_remove_doubles_to_unselected`.
fn to_unselected(cx: &mut OpCtx, threshold: f32) -> Result<(), OpError> {
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    let mut uv_map_arr: Vec<Uv> = Vec::new();
    common::foreach_uv_multi(cx, &objects, ts, false, |uv| uv_map_arr.push(uv));
    let mut tree = KdTree2::new(uv_map_arr.len());
    for (i, &uv) in uv_map_arr.iter().enumerate() {
        tree.insert(i as i32, uv);
    }
    tree.balance();
    for &(_, me) in &objects {
        common::edit(cx, me, ts, |s| {
            let mut changed = false;
            common::foreach_uv(s, true, true, |luv| {
                if let Some((i, dist)) = tree.find_nearest(*luv)
                    && dist < threshold
                {
                    *luv = uv_map_arr[i as usize];
                    changed = true;
                }
            });
            ((), changed)
        })?;
    }
    Ok(())
}

/// `uv_remove_doubles_to_selected_shared_vertex`.
fn to_selected_shared_vertex(cx: &mut OpCtx, threshold: f32) -> Result<(), OpError> {
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    let threshold_sq = threshold * threshold;
    for &(_, me) in &objects {
        common::edit(cx, me, ts, |s| {
            let mut changed = false;
            let mut uvs: Vec<LoopId> = Vec::with_capacity(32);
            for v in s.bm.vert_ids() {
                uvs.clear();
                for l in s.bm.vert_loops(v) {
                    if s.uv_select_test(l) {
                        uvs.push(l);
                    }
                }
                if uvs.len() <= 1 {
                    continue;
                }
                while uvs.len() > 1 {
                    let uvs_num = uvs.len();
                    let mut avg = [0.0f32; 2];
                    for &l in &uvs {
                        let u = s.uv(l);
                        avg[0] += u[0];
                        avg[1] += u[1];
                    }
                    avg[0] /= uvs_num as f32;
                    avg[1] /= uvs_num as f32;

                    // The loop closest to the average is the base of the distances.
                    let mut dist_best_sq = common::len_squared(avg, s.uv(uvs[0]));
                    let mut uv_ref = uvs[0];
                    let mut uv_ref_index = 0usize;
                    for (i, &l) in uvs.iter().enumerate().skip(1) {
                        let d = common::len_squared(avg, s.uv(l));
                        if d < dist_best_sq {
                            dist_best_sq = d;
                            uv_ref = l;
                            uv_ref_index = i;
                        }
                    }
                    let uvs_end = uvs_num - 1;
                    uvs.swap(uv_ref_index, uvs_end);

                    // Move the UVs within the threshold to the end of the array.
                    let ref_uv = s.uv(uv_ref);
                    let mut merged = ref_uv;
                    let mut i = 0usize;
                    let mut uvs_num_merged = 1usize;
                    while uvs[i] != uv_ref && i < uvs_num - uvs_num_merged {
                        let d = common::len_squared(ref_uv, s.uv(uvs[i]));
                        if d < threshold_sq {
                            let u = s.uv(uvs[i]);
                            merged[0] += u[0];
                            merged[1] += u[1];
                            uvs.swap(i, uvs_end - uvs_num_merged);
                            uvs_num_merged += 1;
                            if d != 0.0 {
                                changed = true;
                            }
                        } else {
                            i += 1;
                        }
                    }
                    if uvs_num_merged > 1 {
                        merged[0] /= uvs_num_merged as f32;
                        merged[1] /= uvs_num_merged as f32;
                        for &l in &uvs[uvs_num - uvs_num_merged..uvs_num] {
                            s.uv[l.slot()] = merged;
                        }
                    }
                    uvs.truncate(uvs_num - uvs_num_merged);
                }
            }
            ((), changed)
        })?;
    }
    Ok(())
}
