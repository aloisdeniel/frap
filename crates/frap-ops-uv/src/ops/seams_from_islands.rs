// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.seams_from_islands(mark_seams=True, mark_sharp=False)`, tier T1
//! (`tests/golden/cases/uv/seams/from_islands_*.py`). Port of `uv_seams_from_islands_exec`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On every mesh in Edit Mode with faces and a UV map, each
//! UV-selected edge of a visible face whose faces do not share both UVs (an island boundary
//! in UV space) is marked as a seam (`mark_seams`) and/or sharp (`mark_sharp`); mesh
//! boundary edges are left alone. A smart-projected cube gets all 12 edges. `FINISHED` when
//! some edge was marked, otherwise `CANCELLED`.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SeamsFromIslands) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects_with_uvs(cx);
    let mut changed_multi = false;
    for o in UvOb::gather(cx, &objects) {
        let bm = o.bm.clone();
        if bm.faces_num() == 0 {
            continue;
        }
        let sel = o.sel(ts);
        let mut edges = Vec::new();
        for f in uvedit::faces(&bm) {
            if !sel.face_visible(f) {
                continue;
            }
            for l in bm.face_loops(f) {
                if bm.lp(l).radial_next == l || !sel.edge_select(l) {
                    continue;
                }
                let mut other = bm.lp(l).radial_next;
                let mut mark = false;
                while other != l {
                    if !sel.share_edge(l, other) {
                        mark = true;
                        break;
                    }
                    other = bm.lp(other).radial_next;
                }
                if mark {
                    edges.push(bm.lp(l).e);
                }
            }
        }
        if edges.is_empty() {
            continue;
        }
        changed_multi = true;
        let bm = cx.bmesh_mut(o.me)?;
        for e in edges {
            if p.mark_seams.value {
                bm.edge_mut(e).flag.insert(ElemFlag::SEAM);
            }
            if p.mark_sharp.value {
                bm.edge_mut(e).flag.remove(ElemFlag::SMOOTH);
            }
        }
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
