// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.reveal(select=True)`, tier T1 (`tests/golden/cases/uv/select/hide*.py`). Port
//! of `uv_reveal_exec` (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. With sync selection it is `mesh.reveal(select=…)`. Without it,
//! the faces "hidden" in the UV editor (visible but unselected in the mesh) are selected
//! again in the mesh, their UVs (un)selected by `select` (in vertex or edge mesh mode with
//! the face-UV mode, only faces without a selected vertex/edge get their UVs). Always
//! finishes.

use frap_bmesh::{ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{UV_SELECT_FACE, UvSel, mesh, op};

/// `uv_reveal_exec` for one mesh without sync selection.
fn reveal_no_sync(s: &mut UvSel, select: bool) {
    let use_face_center = s.ts.uv_selectmode == UV_SELECT_FACE;
    let em_mode = s.bm.select_mode;
    let faces = s.faces();
    for &f in &faces {
        s.set_fflag(f, ElemFlag::TAG, false);
        if s.fflag(f, ElemFlag::HIDDEN) || s.fflag(f, ElemFlag::SELECT) {
            continue;
        }
        let loops = s.face_loops(f);
        let set_uvs = if use_face_center && em_mode != SelectMode::FACE {
            let totsel = loops
                .iter()
                .filter(|&&l| {
                    if em_mode == SelectMode::VERT {
                        s.vflag(s.lv(l), ElemFlag::SELECT)
                    } else if em_mode == SelectMode::EDGE {
                        s.eflag(s.le(l), ElemFlag::SELECT)
                    } else {
                        false
                    }
                })
                .count();
            totsel == 0
        } else {
            true
        };
        if set_uvs {
            for &l in &loops {
                s.vert_select_set_no_sync(l, select);
                s.edge_select_set_no_sync(l, select);
            }
        }
        s.face_select_set_no_sync(f, select);
        s.set_fflag(f, ElemFlag::TAG, true);
    }
    // `BM_mesh_elem_hflag_enable_test(bm, BM_FACE, BM_ELEM_SELECT, true, false, BM_ELEM_TAG)`.
    for &f in &faces {
        if s.fflag(f, ElemFlag::TAG) && !s.fflag(f, ElemFlag::HIDDEN) {
            s.bm.face_select_set(f, true);
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Reveal) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let select = p.select.value;
    for (_, me) in op::objects(cx) {
        op::edit(cx, me, ts, |s| {
            if s.ts.sync {
                mesh::mesh_reveal(s.bm, select);
            } else {
                reveal_no_sync(s, select);
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
