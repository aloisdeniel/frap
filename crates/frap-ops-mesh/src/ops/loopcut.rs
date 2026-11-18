// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.loopcut(number_cuts=1, smoothness=0, falloff='INVERSE_SQUARE',
//! object_index=-1, edge_index=-1)`, tier T1 (`tests/golden/cases/edit/subdivide/loopcut_*.py`).
//! Port of the non-interactive path of `loopcut_init`, `edgering_select` and `ringsel_finish`
//! (`editors/mesh/editmesh_loopcut.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! `object_index` indexes the objects in Edit Mode (active first, then base order, shared
//! meshes included) and `edge_index` the edges of that object's mesh; an index out of range
//! (or the default `-1`) cancels. Every edit mesh is deselected, the edge ring through the
//! edge (`BMW_EDGERING`, stopping at n-gons, skipping hidden edges) is selected and
//! subdivided `number_cuts` times (`BM_mesh_esubdivide` with `PATH` corners, grid fill,
//! `use_smooth_even`), so only the new loop edges stay selected. A wire edge selects its new
//! vertices and edges; an edge without an adjacent quad only splits the ring (its end vertices
//! are deselected). Face select mode switches to edge select mode, as does vertex mode when
//! run as the first step of `loopcut_slide` with several cuts.
//!
//! Blender 5.2.4 runs this headless with a 3D Viewport region in the context (the golden
//! runner's override), so the goldens call the operator itself.

use frap_bmesh::ops::subdivide_edges::{ESubdivide, SubdivSelect, mesh_esubdivide};
use frap_bmesh::walkers::{LoopDelimit, WalkOpts};
use frap_bmesh::{BMesh, EdgeId, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::ObjectMode;
use frap_spec_ops_mesh::mesh;

/// The objects in Edit Mode as `BKE_view_layer_array_from_bases_in_edit_mode` lists them:
/// the active object first, then the bases of the view layer in order, visible only, objects
/// of the active object's type, shared data **not** deduplicated.
pub(crate) fn edit_mode_bases(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, Option<IdRef<MeshId>>)> {
    let c = cx.ctx();
    let Some((active, act)) = poll::active(&c) else {
        return Vec::new();
    };
    let ty = act.object_type();
    let vl = c.view_layer();
    let order =
        std::iter::once(active).chain(vl.bases.iter().map(|b| b.object).filter(|r| *r != active));
    let mut out = Vec::new();
    for r in order {
        let Some(base) = vl.bases.iter().find(|b| b.object == r) else {
            continue;
        };
        let Some(ob) = c.main.objects.get(r) else {
            continue;
        };
        let visible = !base.flag.contains(frap_scene::BaseFlag::HIDDEN) && !ob.hide_viewport;
        if ob.mode == ObjectMode::Edit && ob.object_type() == ty && visible {
            out.push((r, ob.data.and_then(|d| d.typed::<MeshId>())));
        }
    }
    out
}

/// `EDBM_selectmode_disable_multi(C, disable, fallback)`: every edit mesh (unique data) whose
/// select mode has `disable` switches to the mode without it (`fallback` when it was the only
/// mode), and `tool_settings.mesh_select_mode` follows. Returns whether any mesh changed.
pub(crate) fn selectmode_disable_multi(
    cx: &mut OpCtx,
    disable: SelectMode,
    fallback: SelectMode,
) -> Result<bool, OpError> {
    let mut changed = None;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let mode = {
            let (_, edit) = cx.split_edit();
            match edit.meshes.get(&me.key()) {
                Some(em) => em.bm.select_mode,
                None => continue,
            }
        };
        if mode.intersects(disable) {
            let new = if mode == disable {
                fallback
            } else {
                mode - disable
            };
            cx.bmesh_mut(me)?.selectmode_set(new);
            changed = Some(new);
        }
    }
    let Some(new) = changed else {
        return Ok(false);
    };
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.tool_settings.mesh_select_mode = [
            new.contains(SelectMode::VERT),
            new.contains(SelectMode::EDGE),
            new.contains(SelectMode::FACE),
        ];
    }
    log.context_changed = true;
    Ok(true)
}

/// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`.
fn deselect_all(bm: &mut BMesh) {
    bm.select_history.clear();
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
    }
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
    }
    for f in bm.face_ids() {
        bm.face_mut(f).flag.remove(ElemFlag::SELECT);
    }
}

/// `BM_edge_is_any_face_len_test(e, 4)`.
fn edge_has_quad(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge_loops(e)
        .into_iter()
        .any(|l| bm.face(bm.lp(l).f).len == 4)
}

/// Runs the loop cut (`ringsel_finish` with `do_cut`) on the edge `e_index` of the
/// `base_index`-th edit-mode object; `None` when an index is out of range (cancelled).
pub(crate) fn loopcut_run(
    cx: &mut OpCtx,
    p: &mesh::Loopcut,
    is_macro: bool,
) -> Result<OpStatus, OpError> {
    // "Use for redo - intentionally wrap int to uint".
    let base_index = p.object_index.value as u32 as usize;
    let e_index = p.edge_index.value as u32 as usize;
    let bases = edit_mode_bases(cx);
    let Some(&(_, Some(me))) = bases.get(base_index) else {
        return Ok(OpStatus::Cancelled);
    };
    let edge = {
        let (_, edit) = cx.split_edit();
        let Some(em) = edit.meshes.get(&me.key()) else {
            return Ok(OpStatus::Cancelled);
        };
        // `BM_edge_at_index` after `BM_mesh_elem_table_ensure`: the edge in iteration order.
        match em.bm.edges.ids().nth(e_index) {
            Some(s) => EdgeId(s),
            None => return Ok(OpStatus::Cancelled),
        }
    };
    let cuts = p.number_cuts.value;

    // `edgering_select`: deselect every edit mesh, then select the ring.
    let mut seen = Vec::new();
    for (_, m) in &bases {
        let Some(m) = *m else { continue };
        if seen.contains(&m) {
            continue;
        }
        seen.push(m);
        deselect_all(cx.bmesh_mut(m)?);
    }
    let bm = cx.bmesh_mut(me)?;
    let v_eed_orig = bm.edge(edge).v;
    let ring = bm.walk_edge_ring_ex(
        edge,
        &WalkOpts {
            test_hidden: true,
            delimit: LoopDelimit::RING_NGONS,
            ..WalkOpts::default()
        },
    );
    for e in ring {
        bm.edge_select_set(e, true);
    }

    let is_edge_wire = bm.edge_is_wire(edge);
    let is_single = is_edge_wire || !edge_has_quad(bm, edge);
    let seltype = if is_edge_wire {
        SubdivSelect::Inner
    } else if is_single {
        SubdivSelect::None
    } else {
        SubdivSelect::Loopcut
    };
    // Grid fill so that intersecting loop cuts work as one would expect (#31939).
    mesh_esubdivide(
        bm,
        &ESubdivide {
            smooth: p.smoothness.value,
            smooth_falloff: p.falloff.value.identifier(),
            use_smooth_even: true,
            fractal: 0.0,
            along_normal: 0.0,
            numcuts: cuts,
            seltype,
            cornertype: "PATH",
            use_single_edge: false,
            use_grid_fill: true,
            use_only_quads: false,
            seed: 0,
        },
    )
    .map_err(|e| OpError::Runtime(e.to_string()))?;

    if is_single {
        // De-select the end points.
        bm.vert_select_set(v_eed_orig[0], false);
        bm.vert_select_set(v_eed_orig[1], false);
        bm.select_flush_mode_ex(SelectMode::VERT, false);
    } else if is_macro && cuts > 1 && bm.select_mode.contains(SelectMode::VERT) {
        // Edge slide cannot slide several edges in vertex select mode.
        selectmode_disable_multi(cx, SelectMode::VERT, SelectMode::EDGE)?;
    } else if selectmode_disable_multi(cx, SelectMode::FACE, SelectMode::EDGE)? {
        // The mode change flushed the selection.
    } else {
        cx.bmesh_mut(me)?.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "headless: give object_index (edit-mode objects, active first) and edge_index (edge of that mesh); -1 cancels",
        "only the new loop edges stay selected; face select mode switches to edge select mode",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Loopcut) -> OpResult {
    let is_macro = cx.is_macro_step();
    loopcut_run(cx, &p, is_macro)
}
