// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_linked(delimit={'SEAM'})`, tier T1 (`tests/golden/cases/edit/select/
//! linked*.py`). Port of `edbm_select_linked_exec`, `select_linked_delimit_test`,
//! `select_linked_delimit_default_from_op` and `select_linked_delimit_validate/begin`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Selects everything connected to the selection of every mesh in Edit Mode, without crossing
//! the `delimit` edges (`SEAM`, `SHARP`, `NORMAL`: faces whose normals flip, `MATERIAL`, `UV`:
//! UV seams of the active UV map, dropped when the mesh has no UV map). Vertex/edge select mode
//! walk connected edges (`BMW_VERT_SHELL`), or with delimiters face corners and wire edges
//! (`BMW_LOOP_SHELL_WIRE`); face mode walks face islands (`BMW_ISLAND`).
//!
//! **Default `delimit`** (`USE_LINKED_SELECT_DEFAULT_HACK`): when `delimit` is not passed,
//! Blender uses the last value passed in the same select-mode family (vertex/edge vs. face)
//! during the session, initially none in vertex/edge mode and `{'SEAM'}` in face-only mode.
//! frap keeps no per-session memory of it: an unset `delimit` is always that initial default.

use frap_bmesh::walkers::WalkDelimit;
use frap_bmesh::{BMesh, EdgeId, FaceId, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loop_select::walk::{Island, LoopShellWire, ShellItem, Start, VertShell};
use super::select_more::common;

/// `BMO_DELIM_*` from the `delimit` identifiers.
pub(crate) fn delimit_from(set: &std::collections::BTreeSet<smol_str::SmolStr>) -> WalkDelimit {
    let has = |s: &str| set.iter().any(|x| x == s);
    WalkDelimit {
        seam: has("SEAM"),
        sharp: has("SHARP"),
        material: has("MATERIAL"),
        normal: has("NORMAL"),
        uv: has("UV"),
    }
}

fn is_empty(d: WalkDelimit) -> bool {
    d == WalkDelimit::default()
}

/// `select_linked_delimit_validate`: `UV` needs a UV map.
fn validate(bm: &BMesh, mut d: WalkDelimit) -> WalkDelimit {
    if d.uv && bm.ldata.float2_names().is_empty() {
        d.uv = false;
    }
    d
}

/// The linked selection of one BMesh (the body of the per-object loop).
pub(crate) fn select_linked(bm: &mut BMesh, delimit: WalkDelimit) {
    let delimit = validate(bm, delimit);
    let any = !is_empty(delimit);
    // `BMO_ELE_TAG` of `select_linked_delimit_begin`: the edges a walk may cross.
    let walk_ok: Vec<bool> = {
        let ids = bm.edge_ids();
        let len = ids.iter().map(|e| e.slot() + 1).max().unwrap_or(0);
        let mut ok = vec![true; len];
        if any {
            for e in ids {
                ok[e.slot()] = !bm.edge_delimits(e, delimit);
            }
        }
        ok
    };
    let ok = |e: EdgeId| walk_ok.get(e.slot()).copied().unwrap_or(true);
    let mode = common::primary_mode(bm.select_mode);
    if mode == SelectMode::VERT {
        let mut tag: Vec<VertId> = bm
            .vert_ids()
            .into_iter()
            .filter(|&v| common::selected(bm, v))
            .collect();
        if any {
            // Exclude delimited vertices next to selected faces (this supports stepping off
            // isolated vertices which would otherwise be ignored).
            let mut untag: Vec<VertId> = Vec::new();
            for e in bm.edge_ids() {
                if !ok(e)
                    && bm
                        .edge_faces(e)
                        .into_iter()
                        .any(|f| common::selected(bm, f))
                {
                    untag.extend(bm.edge(e).v);
                }
            }
            tag.retain(|v| !untag.contains(v));
        }
        let mut tagged: rustc_hash::FxHashSet<VertId> = tag.iter().copied().collect();
        if any {
            let mask = |e: EdgeId| ok(e);
            let mut w = LoopShellWire::new(&mask);
            for v in tag {
                if !tagged.contains(&v) {
                    continue;
                }
                for item in w.walk(bm, Start::Vert(v)) {
                    match item {
                        ShellItem::Loop(l) => {
                            let lv = bm.lp(l).v;
                            bm.vert_select_set(lv, true);
                            tagged.remove(&lv);
                        }
                        ShellItem::Edge(e) => {
                            bm.edge_select_set(e, true);
                            for x in bm.edge(e).v {
                                tagged.remove(&x);
                            }
                        }
                    }
                }
            }
        } else {
            let mut w = VertShell::new();
            for v in tag {
                if !tagged.contains(&v) {
                    continue;
                }
                for e in w.walk(bm, Start::Vert(v)) {
                    bm.edge_select_set(e, true);
                }
            }
        }
        common::selectmode_flush(bm);
    } else if mode == SelectMode::EDGE {
        let tag: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| {
                common::selected(bm, e)
                    && (!any
                        || ok(e)
                        || !bm
                            .edge_faces(e)
                            .into_iter()
                            .any(|f| common::selected(bm, f)))
            })
            .collect();
        let mut tagged: rustc_hash::FxHashSet<EdgeId> = tag.iter().copied().collect();
        if any {
            let mask = |e: EdgeId| ok(e);
            let mut w = LoopShellWire::new(&mask);
            for e in tag {
                if !tagged.contains(&e) {
                    continue;
                }
                for item in w.walk(bm, Start::Edge(e)) {
                    match item {
                        ShellItem::Loop(l) => {
                            let (le, prev_e) = (bm.lp(l).e, bm.lp(bm.lp(l).prev).e);
                            bm.edge_select_set(le, true);
                            bm.edge_select_set(prev_e, true);
                            tagged.remove(&le);
                        }
                        ShellItem::Edge(x) => {
                            bm.edge_select_set(x, true);
                            tagged.remove(&x);
                        }
                    }
                }
            }
        } else {
            let mut w = VertShell::new();
            for e in tag {
                if !tagged.contains(&e) {
                    continue;
                }
                for x in w.walk(bm, Start::Edge(e)) {
                    bm.edge_select_set(x, true);
                    tagged.remove(&x);
                }
            }
        }
        common::selectmode_flush(bm);
    } else {
        let tag: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| common::selected(bm, f))
            .collect();
        let mut tagged: rustc_hash::FxHashSet<FaceId> = tag.iter().copied().collect();
        let mask = |e: EdgeId| ok(e);
        let mut w = Island::new(&mask);
        for f in tag {
            if !tagged.contains(&f) {
                continue;
            }
            for x in w.walk(bm, f) {
                bm.face_select_set(x, true);
                tagged.remove(&x);
            }
        }
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "an unset delimit is Blender's initial default for the select mode: none in vertex/edge mode, {'SEAM'} in face-only mode (Blender then remembers the last delimit passed in the session; frap does not)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectLinked) -> OpResult {
    let delimit = if p.delimit.is_set {
        delimit_from(&p.delimit.value)
    } else {
        let mode = {
            let c = cx.ctx();
            c.scene().tool_settings.mesh_select_mode
        };
        // `delimit_last_store[(selectmode & (VERTEX | EDGE)) == 0]` = {0, SEAM}.
        WalkDelimit {
            seam: !mode[0] && !mode[1],
            ..WalkDelimit::default()
        }
    };
    for (_, me) in common::edit_meshes(cx) {
        select_linked(cx.bmesh_mut(me)?, delimit);
    }
    Ok(OpStatus::Finished)
}
