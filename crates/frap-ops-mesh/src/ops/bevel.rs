// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.bevel(offset_type='OFFSET', offset=0, offset_pct=0, segments=1, profile=0.5,
//! affect='EDGES', …)`, tier T2 (one segment at T1; `tests/golden/cases/edit/bevel/*.py`).
//! Port of `edbm_bevel_exec` / `edbm_bevel_init` / `edbm_bevel_calc` / `edbm_bevel_exit`
//! (`editors/mesh/editmesh_bevel.cc`, Blender 5.2.4, GPL-2.0-or-later); the geometry is
//! `frap_bmesh::ops::bevel` (`BM_mesh_bevel`).
//!
//! Every mesh in Edit Mode with at least one selected vertex takes part (multi-object edit,
//! one entry per mesh); without any, the operator is cancelled (Blender is silent; frap adds
//! a WARNING, D-51). Per mesh:
//! - `geom` is the selected vertices and edges; `offset` is `offset_pct` when `offset_type`
//!   is `PERCENT`; `material` is clamped to `[-1, slots - 1]`;
//! - with a non-zero offset everything is deselected (and the selection history cleared),
//!   then the new bevel faces are selected (with their vertices and edges); a vertex bevel
//!   also selects the new vertices and edges and flushes the selection as in vertex mode;
//! - outside face select mode the selection is flushed for the select mode.
//!
//! **`offset=0` (the default) bevels nothing** and the operator still finishes, as in
//! Blender: frap adds the response hint `NO_OP_DEFAULT` naming `offset` (`offset_pct` for
//! `PERCENT`). Not supported: `profile_type='CUSTOM'`
//! (`OP_NOT_IMPLEMENTED`), `harden_normals` (the bevel is made, normals are not hardened, a
//! WARNING says so), and Auto Merge after the bevel (`tool_settings.use_mesh_automerge`, a
//! WARNING).

use frap_bmesh::ops::bevel::{BevelIn, bevel};
use frap_bmesh::{BMElem, BmError, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::slots;
use frap_spec_ops_mesh::mesh::{
    self,
    bevel::{OffsetType, ProfileType},
};

/// The selected vertices, then the selected edges (`geom=%hev`, `BM_ELEM_SELECT`).
fn selected_geom(bm: &frap_bmesh::BMesh) -> Vec<BMElem> {
    let mut geom: Vec<BMElem> = bm
        .verts
        .iter()
        .filter(|(_, v)| v.flag.contains(ElemFlag::SELECT))
        .map(|(i, _)| BMElem::Vert(frap_bmesh::VertId(i)))
        .collect();
    geom.extend(
        bm.edges
            .iter()
            .filter(|(_, e)| e.flag.contains(ElemFlag::SELECT))
            .map(|(i, _)| BMElem::Edge(frap_bmesh::EdgeId(i))),
    );
    geom
}

/// `bm->totvertsel > 0`.
fn any_vert_selected(bm: &frap_bmesh::BMesh) -> bool {
    bm.verts
        .iter()
        .any(|(_, v)| v.flag.contains(ElemFlag::SELECT))
}

/// Hint code: the arguments (here the default `offset=0`) make the operator do nothing.
const NO_OP_DEFAULT: &str = "NO_OP_DEFAULT";

fn bm_error(e: BmError) -> OpError {
    match e {
        BmError::Invalid(m) if m.contains("CUSTOM") => OpError::NotImplemented(m),
        e => OpError::Runtime(e.to_string()),
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T2,
    support = Partial("profile_type='CUSTOM' answers OP_NOT_IMPLEMENTED; harden_normals and Auto Merge are not applied (WARNING)"),
    notes = [
        "offset defaults to 0, which bevels nothing: pass offset (or offset_pct with offset_type='PERCENT'); frap answers the hint NO_OP_DEFAULT",
        "segments=1 matches Blender at T1; more segments match within 1e-5 in Blender's element order (T2)",
        "selection after: the new bevel faces (vertex bevel: also the new vertices and edges); the selection history is cleared",
        "nothing selected cancels (Blender is silent; frap adds a WARNING)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Bevel) -> OpResult {
    if p.profile_type.value == ProfileType::Custom {
        return Err(OpError::NotImplemented(
            "mesh.bevel: profile_type='CUSTOM' (the tool settings' custom bevel profile) is not \
             supported yet; use profile_type='SUPERELLIPSE' with profile"
                .into(),
        ));
    }
    let percent = p.offset_type.value == OffsetType::Percent;
    let offset = if percent {
        p.offset_pct.value
    } else {
        p.offset.value
    };
    let affect_vertices = p.affect.value == mesh::bevel::Affect::Vertices;

    // Objects whose edit mesh has a selected vertex (`edbm_bevel_init`).
    let stores: Vec<_> = {
        let meshes = helpers::iter_edit_meshes(cx);
        let (_, edit) = cx.split_edit();
        meshes
            .into_iter()
            .filter(|(_, me)| {
                edit.meshes
                    .get(&me.key())
                    .is_some_and(|em| any_vert_selected(&em.bm))
            })
            .collect()
    };
    if stores.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing beveled: no vertex is selected in the meshes in Edit Mode",
        );
        return Ok(OpStatus::Cancelled);
    }

    let automerge = {
        let ctx = cx.ctx();
        ctx.main
            .scenes
            .get(ctx.win.scene)
            .is_some_and(|s| s.tool_settings.use_mesh_automerge)
    };

    for (ob, me) in stores {
        let totcol = cx
            .main()
            .objects
            .get(ob)
            .map_or(0, |o| slots::slot_count(cx.main(), o)) as i32;
        let material = p.material.value.clamp(-1, totcol - 1);
        let bm = cx.bmesh_mut(me)?;
        let input = BevelIn {
            geom: selected_geom(bm),
            offset,
            offset_type: p.offset_type.value.identifier(),
            profile_type: p.profile_type.value.identifier(),
            segments: p.segments.value,
            profile: p.profile.value,
            affect: p.affect.value.identifier(),
            clamp_overlap: p.clamp_overlap.value,
            material,
            loop_slide: p.loop_slide.value,
            mark_seam: p.mark_seam.value,
            mark_sharp: p.mark_sharp.value,
            harden_normals: p.harden_normals.value,
            face_strength_mode: p.face_strength_mode.value.identifier(),
            miter_outer: p.miter_outer.value.identifier(),
            miter_inner: p.miter_inner.value.identifier(),
            spread: p.spread.value,
            custom_profile: None,
            vmesh_method: p.vmesh_method.value.identifier(),
        };
        let out = bevel(bm, &input).map_err(bm_error)?;
        if offset != 0.0 {
            // Loose geometry that won't get beveled is better not left selected.
            bm.deselect_all();
            for &f in &out.faces {
                bm.face_select_set(f, true);
            }
            if affect_vertices {
                for &v in &out.verts {
                    bm.vert_select_set(v, true);
                }
                for &e in &out.edges {
                    bm.edge_select_set(e, true);
                }
                if !bm.select_mode.contains(SelectMode::VERT) {
                    bm.select_flush_mode_ex(SelectMode::VERT, false);
                }
            }
        }
        // `edbm_bevel_exit`: without this, faces surrounded by selected edges/verts would be
        // unselected.
        if !bm.select_mode.contains(SelectMode::FACE) {
            bm.select_flush_mode();
        }
    }

    if offset == 0.0 {
        // Blender finishes silently; frap answers the `NO_OP_DEFAULT` hint (§13.2).
        let message = if percent {
            "mesh.bevel offset_pct=0 is a no-op with offset_type='PERCENT'; pass offset_pct, \
             e.g. bpy.ops.mesh.bevel(offset_type='PERCENT', offset_pct=10)"
        } else {
            "mesh.bevel offset=0 is a no-op; pass offset, e.g. bpy.ops.mesh.bevel(offset=0.1, \
             segments=2)"
        };
        cx.hint(NO_OP_DEFAULT, message);
    }
    if p.harden_normals.value && offset != 0.0 {
        cx.report(
            ReportType::Warning,
            "frap: harden_normals is not supported yet: the bevel was made without hardening \
             the normals",
        );
    }
    if automerge {
        cx.report(
            ReportType::Warning,
            "frap: Auto Merge (tool_settings.use_mesh_automerge) is not applied after mesh.bevel",
        );
    }
    Ok(OpStatus::Finished)
}
