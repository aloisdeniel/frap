// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.fill_grid(span=1, offset=0, use_interp_simple=False)`, tier T1
//! (`tests/golden/cases/edit/create/fill_grid_*.py`). Port of `edbm_fill_grid_exec`,
//! `edbm_fill_grid_prepare` and the split/join helpers (`editors/mesh/editmesh_tools.cc`,
//! Blender 5.2.4, GPL-2.0-or-later) over `bmesh.ops.grid_fill`.
//!
//! - A single selected closed loop with an even vertex count is cut into two rails: the
//!   corner is the active vertex when it is on the loop, else the sharpest corner, moved by
//!   `offset`; `span` (when given) is the number of grid columns, else the next sharpest
//!   corner decides (or a quarter of the loop).
//! - Two open loops are bridged as they are.
//! - With faces selected, the selected region is first split off (WP 2.10 `split`), the grid
//!   fills the island's border, the island is deleted, the new faces are reversed (WP 2.9
//!   `reverse_faces`) and welded back (WP 2.7 `weld_verts`).
//!
//! Errors of `grid_fill` (`Select two edge loops or a single closed edge loop…`, `Closed
//! loops unsupported`, `Loops are not connected by wire/boundary edges`, `Connecting edge
//! loops overlap`) are INFO reports; the operator always finishes.

use frap_bmesh::ops::bridge_loops::edgeloop;
use frap_bmesh::ops::contextual_create::{bli, construct};
use frap_bmesh::ops::grid_fill::{GridFillIn, grid_fill};
use frap_bmesh::{BMElem, BMesh, BmError, ElemFlag, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

/// `edbm_fill_grid_vert_tag_angle`: how far the loop turns at `v` (tagged edges).
fn vert_tag_angle(bm: &BMesh, v: VertId) -> f32 {
    let pair: Vec<VertId> = bm
        .vert_edges(v)
        .into_iter()
        .filter(|&e| bm.edge(e).flag.contains(ElemFlag::TAG))
        .map(|e| bm.edge_other_vert(e, v))
        .collect();
    let (a, b) = (pair[0], pair[1]);
    (bli::PI - bli::angle_v3v3v3(bm.vert(a).co, bm.vert(v).co, bm.vert(b).co)).abs()
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc (edbm_fill_grid_prepare).
/// Tags the rail edges of a single selected loop; `None` when the operator should use the
/// selection as is.
fn fill_grid_prepare(bm: &mut BMesh, offset: i32, span_in: Option<i32>) -> Option<i32> {
    let eps_even = 1e-3f32;
    let eloops = edgeloop::mesh_edgeloops_find(bm, &|bm, e| {
        bm.edge(e).flag.contains(ElemFlag::SELECT)
    });
    if eloops.len() != 1 {
        return None;
    }
    let mut el = eloops.into_iter().next().expect("one loop");
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    let verts_len = el.len() as i32;
    let edges_len = verts_len - if el.closed { 0 } else { 1 };
    let mut edges = el.edges(bm);
    for &e in &edges {
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
    }
    let span_calc = span_in.is_none();
    let mut span = match span_in {
        None => verts_len / 4,
        Some(s) => s.min((verts_len / 2) - 1),
    };
    let offset = bli::mod_i(offset, verts_len);
    if verts_len % 2 == 0 && verts_len == edges_len {
        let v_act = match bm.select_history_active() {
            Some(BMElem::Vert(v)) => Some(v),
            _ => None,
        };
        let idx = match v_act.and_then(|v| el.verts.iter().position(|&x| x == v)) {
            Some(i) => i,
            None => {
                let mut best: Option<usize> = None;
                let mut angle_best = -1.0f32;
                for (i, &v) in el.verts.iter().enumerate() {
                    let a = vert_tag_angle(bm, v);
                    if a > angle_best || best.is_none() {
                        angle_best = a;
                        best = Some(i);
                    }
                }
                best.expect("loop has vertices")
            }
        };
        el.rotate_first(idx);
        if offset != 0 {
            el.rotate_first(offset as usize);
        }
        edges = el.edges(bm);
        if span_calc {
            let mut sorted: Vec<(f32, usize)> = el
                .verts
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let a = vert_tag_angle(bm, v);
                    let a = if i == 0 || i == (verts_len / 2) as usize { 0.0 } else { a };
                    (a, i)
                })
                .collect();
            // `qsort(..., BLI_sortutil_cmp_float_reverse)`.
            bli::c_qsort(&mut sorted, &bli::cmp_float_reverse);
            if sorted[0].0 - sorted[(verts_len - 3) as usize].0 > eps_even {
                span = sorted[0].1 as i32;
            }
        }
        let mut start = 0;
        if span > verts_len / 2 {
            span = verts_len - span;
            start = (verts_len / 2) - span;
        }
        for i in start..start + span {
            let a = edges[i as usize];
            let b = edges[((verts_len / 2) + i) as usize];
            bm.edge_mut(a).flag.remove(ElemFlag::TAG);
            bm.edge_mut(b).flag.remove(ElemFlag::TAG);
        }
    }
    Some(span)
}

/// The state `edbm_fill_grid_split_join_init` prepares.
struct SplitJoin {
    weld: Vec<(VertId, VertId)>,
    island: Vec<FaceId>,
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (bm_faces_select_to_tag_with_manifold_bounds_check).
fn faces_select_to_tag(bm: &mut BMesh) -> bool {
    construct::verts_flag_disable_all(bm, ElemFlag::TAG);
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::SELECT) {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
            for l in bm.face_loops(f) {
                let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
        } else {
            bm.face_mut(f).flag.remove(ElemFlag::TAG);
        }
    }
    let mut counts: rustc_hash::FxHashMap<VertId, u8> = Default::default();
    let mut boundary = 0;
    for e in bm.edge_ids() {
        let tagged = bm
            .edge_faces(e)
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
            .take(2)
            .count();
        if tagged == 1 {
            boundary += 1;
            for v in bm.edge(e).v {
                let c = counts.entry(v).or_insert(0);
                *c += 1;
                if *c == 3 {
                    return false;
                }
            }
        }
    }
    boundary != 0
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (edbm_fill_grid_split_join_init).
fn split_join_init(bm: &mut BMesh) -> Result<Option<SplitJoin>, BmError> {
    if !faces_select_to_tag(bm) {
        return Ok(None);
    }
    let mut geom: Vec<BMElem> = Vec::new();
    geom.extend(bm.vert_ids().into_iter().filter(|&v| bm.vert(v).flag.contains(ElemFlag::TAG)).map(BMElem::Vert));
    geom.extend(bm.edge_ids().into_iter().filter(|&e| bm.edge(e).flag.contains(ElemFlag::TAG)).map(BMElem::Edge));
    geom.extend(bm.face_ids().into_iter().filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG)).map(BMElem::Face));
    let out = frap_bmesh::ops::split::split(
        bm,
        &frap_bmesh::ops::split::SplitIn {
            geom,
            ..Default::default()
        },
    )?;
    let island: Vec<FaceId> = out
        .geom
        .iter()
        .filter_map(|g| match g {
            BMElem::Face(f) => Some(*f),
            _ => None,
        })
        .collect();
    let mut weld = Vec::new();
    // `boundary_map.out`: source boundary edge -> its copy (a deleted source maps from its copy).
    for &(e, e_dst) in &out.boundary_map {
        bm.edge_mut(e_dst).flag.insert(ElemFlag::SELECT);
        if bm.edges.contains(e.0) && e != e_dst {
            bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
            let src = bm.edge(e).v;
            let dst = bm.edge(e_dst).v;
            for k in 0..2 {
                if bm.vert(src[k]).flag.contains(ElemFlag::SELECT) {
                    bm.vert_mut(src[k]).flag.remove(ElemFlag::SELECT);
                    bm.vert_mut(dst[k]).flag.insert(ElemFlag::SELECT);
                    weld.push((src[k], dst[k]));
                }
            }
        }
    }
    for &f in &island {
        bm.face_mut(f).flag.insert(ElemFlag::SELECT);
    }
    let island: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
        .collect();
    Ok(Some(SplitJoin { weld, island }))
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (edbm_fill_grid_split_join_finish).
fn split_join_finish(bm: &mut BMesh, sj: SplitJoin, changed: bool) {
    if changed {
        let geom: Vec<BMElem> = sj
            .island
            .iter()
            .filter(|&&f| bm.faces.contains(f.0))
            .map(|&f| BMElem::Face(f))
            .collect();
        let _ = frap_bmesh::ops::delete::delete(
            bm,
            &frap_bmesh::ops::delete::DeleteIn {
                geom,
                context: "FACES",
            },
        );
        let faces = edbm::sel_faces(bm);
        let _ = frap_bmesh::ops::reverse_faces::reverse_faces(
            bm,
            &frap_bmesh::ops::reverse_faces::ReverseFacesIn {
                faces,
                flip_multires: true,
            },
        );
    } else {
        let geom: Vec<BMElem> = sj.island.iter().map(|&f| BMElem::Face(f)).collect();
        edbm::select_enable(bm, &geom, true);
    }
    let _ = frap_bmesh::ops::weld_verts::weld_verts(
        bm,
        &frap_bmesh::ops::weld_verts::WeldVertsIn {
            targetmap: sj.weld,
            ..Default::default()
        },
    );
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::FillGrid) -> OpResult {
    run(cx, p)
}

fn run(cx: &mut OpCtx, p: mesh::FillGrid) -> OpResult {
    let use_interp_simple = p.use_interp_simple.value;
    let span_in = p.span.is_set.then_some(p.span.value);
    let offset = p.offset.value;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let counts = edbm::with_bm(cx, me, |bm| bm.selected_counts());
        let Some((_, te, tf)) = counts else {
            continue;
        };
        if te == 0 {
            continue;
        }
        let mat_nr = edbm::mat_nr(cx, ob);
        let bm = cx.bmesh_mut(me)?;
        let use_smooth = edbm::smooth_get(bm);
        let mut split_join = None;
        if tf != 0 {
            let r = split_join_init(bm);
            match edbm::finish(cx, r)? {
                Some(Some(sj)) => split_join = Some(sj),
                _ => continue,
            }
        }
        let bm = cx.bmesh_mut(me)?;
        let use_prepare = fill_grid_prepare(bm, offset, span_in).is_some();
        let flag = if use_prepare { ElemFlag::TAG } else { ElemFlag::SELECT };
        let edges = edbm::edges_with(bm, flag);
        let r = grid_fill(
            bm,
            &GridFillIn {
                edges,
                mat_nr,
                use_smooth,
                use_interp_simple,
            },
        );
        let out = edbm::finish(cx, r)?;
        let bm = cx.bmesh_mut(me)?;
        let changed = match out {
            Some(o) => {
                let faces: Vec<BMElem> = o.faces.iter().map(|&f| BMElem::Face(f)).collect();
                edbm::select_enable(bm, &faces, true);
                !o.faces.is_empty()
            }
            None => false,
        };
        if let Some(sj) = split_join {
            split_join_finish(bm, sj, changed);
        }
    }
    Ok(OpStatus::Finished)
}
