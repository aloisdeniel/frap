// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.merge(type='CENTER', uvs=False)`, tier T1
//! (`tests/golden/cases/edit/delete/merge_*.py`). Port of `edbm_merge_exec`, `merge_target`,
//! `merge_firstlast` and `merge_type_itemf` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! `type` is a dynamic enum: `CENTER`, `CURSOR` and `COLLAPSE` always; in vertex select mode
//! `FIRST` when the active edit mesh's selection history starts with a vertex and `LAST` when
//! it ends with one. Another value raises Blender's conversion `TypeError` (`enum "FIRST" not
//! found in ('CENTER', 'CURSOR', 'COLLAPSE')`).
//!
//! On every mesh in Edit Mode with selected vertices:
//! - `CENTER` / `CURSOR`: `bmesh.ops.pointmerge` of the selected vertices at their mean / at
//!   the 3D cursor (in object space); the lowest-index selected vertex survives (its vertex
//!   data averaged); with `uvs`, `average_vert_facedata` first;
//! - `FIRST` / `LAST`: `pointmerge` into the first / last vertex of the selection history (it
//!   keeps its slot and position); with `uvs`, `pointmerge_facedata` first; only the active
//!   mesh is merged;
//! - `COLLAPSE`: `bmesh.ops.collapse` of the selected edges (each connected group to its
//!   centre).
//!
//! The survivor stays selected (and in the selection history when its cluster was); outside
//! vertex select mode everything is deselected. Always `FINISHED`.

use frap_bmesh::ops::collapse::{CollapseIn, collapse};
use frap_bmesh::ops::pointmerge::{
    PointmergeFacedataIn, PointmergeIn, average_vert_facedata, pointmerge, pointmerge_facedata,
};
use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode, VertId};
use frap_math::glam::Vec3;
use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{PropKind, PyExc, RnaError, RnaErrorKind};
use frap_scene::Context;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_mesh::mesh::{self, merge::Type};

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, sel_verts, totals};

/// `merge_type_itemf` for the active edit mesh.
pub(crate) fn merge_items(bm: Option<&BMesh>) -> Vec<Type> {
    let mut items = vec![Type::Center, Type::Cursor, Type::Collapse];
    let Some(bm) = bm else {
        return Type::ALL.to_vec();
    };
    if bm.select_mode.contains(SelectMode::VERT) {
        let first = matches!(bm.select_history.first(), Some(BMElem::Vert(_)));
        let last = matches!(bm.select_history.last(), Some(BMElem::Vert(_)));
        if first {
            items.push(Type::First);
        }
        if last {
            items.push(Type::Last);
        }
    }
    items
}

/// `merge_type_itemf` as far as the context shows it, for the argument conversion (§10.2): the
/// edit BMesh's selection history is not part of `Context` (RFC 0001 F-OP-5), so in vertex
/// select mode `FIRST` and `LAST` are both offered and `exec` checks them against the history
/// (with the same `TypeError`, `OP_ARG_ENUM`). Outside vertex select mode the list is exact.
fn type_items(c: &Context) -> Vec<EnumItemOwned> {
    let vert_mode = poll::edit_object(c).is_none() || c.tool_settings().mesh_select_mode[0];
    let items: Vec<Type> = if vert_mode {
        Type::ALL.to_vec()
    } else {
        vec![Type::Center, Type::Cursor, Type::Collapse]
    };
    let PropKind::Enum { items: all, .. } = &mesh::MERGE_PROPS[0].kind else {
        return Vec::new();
    };
    all.iter()
        .filter(|i| items.iter().any(|t| t.identifier() == i.identifier))
        .map(|i| EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// `merge_firstlast`: the merge vertex, when the history end is a selected vertex.
fn merge_vert(bm: &BMesh, use_first: bool) -> Option<VertId> {
    let ese = if use_first {
        bm.select_history.first()
    } else {
        bm.select_history.last()
    };
    match ese {
        Some(&BMElem::Vert(v)) if bm.vert(v).flag.contains(ElemFlag::SELECT) => Some(v),
        _ => None,
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    dyn_enums = [("type", type_items)],
    notes = [
        "type is a dynamic enum: FIRST/LAST exist only in vertex select mode when the selection history starts/ends with a vertex (use frap.select_elements to build it)",
        "CENTER/CURSOR keep the lowest-index selected vertex; FIRST/LAST keep the history vertex and merge the active mesh only",
        "result.removed: number of vertices merged away",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Merge) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let ty = p.r#type.value;
    if p.r#type.is_set {
        // The conversion cannot see the selection history (`type_items`), so `FIRST`/`LAST`
        // are checked here against it, with Blender's conversion message.
        let active = meshes.first().map(|&(_, me)| me);
        let items = match active {
            Some(me) => merge_items(edit_bm(cx, me)),
            None => Type::ALL.to_vec(),
        };
        if !items.contains(&ty) {
            let list = items
                .iter()
                .map(|t| format!("'{}'", t.identifier()))
                .collect::<Vec<_>>()
                .join(", ");
            let mut e = RnaError::new(
                RnaErrorKind::Enum,
                PyExc::TypeError,
                format!(
                    "Converting py args to operator properties: enum \"{}\" not found in ({list})",
                    ty.identifier()
                ),
            );
            e.detail.insert(
                "valid".into(),
                items
                    .iter()
                    .map(|t| t.identifier())
                    .collect::<Vec<_>>()
                    .into(),
            );
            return Err(OpError::Rna(e));
        }
    }
    let uvs = p.uvs.value;
    let cursor = cx.ctx().cursor().location;
    let mut removed = Removed::default();
    for (ob, me) in meshes {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().0 == 0) {
            continue;
        }
        // `invert_m4_m4(world_to_object, object_to_world)` for the cursor in object space.
        let cursor_local = {
            let world = world_matrix(&cx.main().objects, ob);
            let inv = world.inverse();
            inv.transform_point3(Vec3::from(cursor)).to_array()
        };
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let ok = match ty {
            Type::Center | Type::Cursor => {
                let verts = sel_verts(bm);
                let co = if ty == Type::Cursor {
                    cursor_local
                } else {
                    let mut cent = [0.0f32; 3];
                    let mut n = 0usize;
                    for (_, v) in bm.verts.iter() {
                        if !v.flag.contains(ElemFlag::SELECT) {
                            continue;
                        }
                        for (c, x) in cent.iter_mut().zip(v.co) {
                            *c += x;
                        }
                        n += 1;
                    }
                    let fac = 1.0 / n as f32;
                    [cent[0] * fac, cent[1] * fac, cent[2] * fac]
                };
                if uvs {
                    average_vert_facedata(bm, &verts).map_err(bm_err)?;
                }
                pointmerge(
                    bm,
                    &PointmergeIn {
                        verts,
                        merge_co: co,
                        vert_target: None,
                    },
                )
                .map_err(bm_err)?;
                true
            }
            Type::First | Type::Last => match merge_vert(bm, ty == Type::First) {
                Some(mergevert) => {
                    let verts = sel_verts(bm);
                    if uvs {
                        pointmerge_facedata(
                            bm,
                            &PointmergeFacedataIn {
                                verts: verts.clone(),
                                vert_target: Some(mergevert),
                            },
                        )
                        .map_err(bm_err)?;
                    }
                    let co = bm.vert(mergevert).co;
                    pointmerge(
                        bm,
                        &PointmergeIn {
                            verts,
                            merge_co: co,
                            vert_target: Some(mergevert),
                        },
                    )
                    .map_err(bm_err)?;
                    true
                }
                None => false,
            },
            Type::Collapse => {
                let edges = sel_edges(bm);
                collapse(bm, &CollapseIn { edges, uvs }).map_err(bm_err)?;
                true
            }
        };
        if !ok {
            continue;
        }
        // Once collapsed, we can't have edge/face selection.
        if !bm.select_mode.contains(SelectMode::VERT) {
            bm.deselect_all();
        }
        removed.add(old, totals(bm));
        // Only the active object is supported for FIRST/LAST.
        if matches!(ty, Type::First | Type::Last) {
            break;
        }
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_follow_the_history() {
        let mut bm = BMesh::new();
        let v0 = bm.vert_create([0.0; 3], None, frap_bmesh::CreateFlag::empty());
        let v1 = bm.vert_create([1.0, 0.0, 0.0], None, frap_bmesh::CreateFlag::empty());
        let e = bm.edge_create(v0, v1, None, frap_bmesh::CreateFlag::empty());
        assert_eq!(
            merge_items(Some(&bm)),
            [Type::Center, Type::Cursor, Type::Collapse]
        );
        bm.select_history = vec![BMElem::Vert(v0), BMElem::Edge(e)];
        assert_eq!(
            merge_items(Some(&bm)),
            [Type::Center, Type::Cursor, Type::Collapse, Type::First]
        );
        bm.select_history = vec![BMElem::Edge(e), BMElem::Vert(v1)];
        assert_eq!(
            merge_items(Some(&bm)),
            [Type::Center, Type::Cursor, Type::Collapse, Type::Last]
        );
        bm.select_mode = SelectMode::EDGE;
        assert_eq!(
            merge_items(Some(&bm)),
            [Type::Center, Type::Cursor, Type::Collapse]
        );
        assert_eq!(merge_items(None).len(), 5);
    }
}
