// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.select_elements(type='VERT', indices=[…], action='SET', object='')`, tier T1
//! (`tests/golden/cases/edit/prereq/select_elements_*.py`): frap's selection by index for
//! agents, which cannot click (ARCHITECTURE §22.4, OPERATORS.md; spec
//! `spec/frap/ops/frap.select_elements.json`). Blender has no such operator: goldens run
//! `tools/blender/emulations.py::select_elements` on the Blender side, which this operator
//! follows step by step with BMesh calls of the same names:
//! 1. `SET` deselects every element (`select_set(False)` on every vertex, edge and face) and
//!    clears the selection history;
//! 2. each index, in the given order, selects (`SET`, `ADD`) or deselects (`REMOVE`) its
//!    element with `BM_elem_select_set` (an edge or face selects its vertices; deselecting
//!    follows the select mode); `SET` and `ADD` append the element to the selection history
//!    unless it is already there (`select_history.add`), so the last index is the active
//!    element;
//! 3. the selection is flushed for the select mode (`bm.select_flush_mode()`), which also
//!    drops deselected elements from the history.
//!
//! Indices are those of `mesh.vertices`, `mesh.edges` and `mesh.polygons` as read now (the
//! edit-mode view): element order is pool order. `object` names another object in Edit Mode
//! (multi-object edit); by default the indices refer to the active object. An index outside
//! `0..len-1` raises `IndexError` and a wrong `object` `RuntimeError`, before anything changes.

use frap_bmesh::{BMElem, BMesh, EdgeId, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId, PyExc, RnaError, RnaErrorKind};
use frap_scene::ObjectMode;
use frap_spec_ops_frap::frap::{
    self,
    select_elements::{Action, Type},
};

/// The integers of the `indices` value (a dynamic INT array).
fn indices(v: &Value) -> Vec<i64> {
    match v {
        Value::IntArray(a) => a.to_vec(),
        Value::Int(i) => vec![*i],
        Value::Tuple(t) => t
            .iter()
            .filter_map(|x| match x {
                Value::Int(i) => Some(*i),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The elements of `ty` in index order (live pool slots, ascending).
fn elements(bm: &BMesh, ty: Type) -> Vec<BMElem> {
    match ty {
        Type::Vert => bm
            .verts
            .iter()
            .map(|(i, _)| BMElem::Vert(VertId(i)))
            .collect(),
        Type::Edge => bm
            .edges
            .iter()
            .map(|(i, _)| BMElem::Edge(EdgeId(i)))
            .collect(),
        Type::Face => bm
            .faces
            .iter()
            .map(|(i, _)| BMElem::Face(FaceId(i)))
            .collect(),
    }
}

/// The edit object the indices refer to: `object` by name, else the active one.
fn target(cx: &OpCtx, object: &str) -> Result<IdRef<MeshId>, OpError> {
    let c = cx.ctx();
    let wrong = || {
        OpError::Runtime(format!(
            "frap.select_elements: object \"{object}\" is not a mesh in Edit Mode"
        ))
    };
    let ob: IdRef<ObjectId> = if object.is_empty() {
        poll::edit_object(&c).map(|(r, _)| r).ok_or_else(wrong)?
    } else {
        c.main.objects.find(object).ok_or_else(wrong)?
    };
    let o = c.main.objects.get(ob).ok_or_else(wrong)?;
    if o.mode != ObjectMode::Edit || o.object_type() != "MESH" {
        return Err(wrong());
    }
    o.data.and_then(|d| d.typed::<MeshId>()).ok_or_else(wrong)
}

/// Applies the selection change to one BMesh (`picked` are valid elements of it).
pub(crate) fn apply(bm: &mut BMesh, picked: &[BMElem], action: Action) {
    if action == Action::Set {
        let mut all = elements(bm, Type::Vert);
        all.extend(elements(bm, Type::Edge));
        all.extend(elements(bm, Type::Face));
        for e in all {
            bm.select_set(e, false);
        }
        bm.select_history.clear();
    }
    let select = action != Action::Remove;
    for &e in picked {
        bm.select_set(e, select);
        // `BMEditSelSeq.add` → `BM_select_history_store`: appended unless already stored.
        if select && !bm.select_history.contains(&e) {
            bm.select_history.push(e);
        }
    }
    bm.select_flush_mode();
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "frap extension: agents select by index (mesh.vertices / edges / polygons order of the edit-mode view)",
        "SET and ADD append to the selection history, so the last index becomes the active element",
        "goldens run tools/blender/emulations.py::select_elements on the Blender side",
    ]
)]
fn exec(cx: &mut OpCtx, p: frap::SelectElements) -> OpResult {
    let me = target(cx, p.object.value.as_str())?;
    let wanted = indices(&p.indices.value);
    let ty = p.r#type.value;
    let picked = {
        let (_, edit) = cx.split_edit();
        let em = edit.meshes.get(&me.key()).ok_or_else(|| {
            OpError::Runtime("frap.select_elements: the mesh has no edit data".into())
        })?;
        let all = elements(&em.bm, ty);
        let mut picked = Vec::with_capacity(wanted.len());
        for &i in &wanted {
            let e = usize::try_from(i).ok().and_then(|i| all.get(i).copied());
            let Some(e) = e else {
                return Err(OpError::Rna(
                    RnaError::new(
                        RnaErrorKind::IndexRange,
                        PyExc::IndexError,
                        format!(
                            "frap.select_elements: index {i} out of range ({})",
                            all.len()
                        ),
                    )
                    .with_detail("type", ty.identifier())
                    .with_detail("len", all.len()),
                ));
            };
            picked.push(e);
        }
        picked
    };
    apply(cx.bmesh_mut(me)?, &picked, p.action.value);
    Ok(OpStatus::Finished)
}
