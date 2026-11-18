// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_by_attribute()`, tier T1 (`tests/golden/cases/edit/select/
//! by_attribute.py`). Port of `edbm_select_by_attribute_exec`, `edbm_select_by_attribute_poll`
//! and `domain_to_iter_type` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Selects the visible, unselected vertices, edges or faces whose value of the mesh's
//! **active** boolean attribute is true (`mesh.attributes.active`), then flushes the selection.
//! The poll fails without an active attribute (`There must be an active attribute`), when it is
//! not boolean (`The active attribute must have a boolean type`) or on the face-corner domain
//! (`The active attribute must be on the vertex, edge, or face domain`). A new attribute is not
//! made active (`attributes.new` leaves `attributes.active` at `None`, verified on 5.2.4).
//!
//! The active attribute is `Mesh::attributes_active` (WP 2.9), which `mesh.attributes.active`
//! reads and writes; set it in Object Mode, before entering Edit Mode.

use frap_bmesh::{BMElem, BMesh};
use frap_mesh::{AttrData, Mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::MeshId;
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;
use smallvec::smallvec;
use smol_str::SmolStr;

use super::select_more::common;

/// `BKE_attributes_active_name_get` of a mesh: the name stored in `Mesh::attributes_active`
/// (WP 2.9), `None` without one. The layer itself is looked up again in the edit BMesh, which is
/// authoritative in Edit Mode.
pub(crate) fn active_attribute_name(me: &Mesh) -> Option<SmolStr> {
    me.attributes_active.clone()
}

/// The domain of a BMesh custom-data layer (`BM_data_layer_lookup`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Domain {
    Point,
    Edge,
    Face,
    Corner,
}

/// `BM_data_layer_lookup(bm, name)`: the domain and data of the named layer.
fn layer_lookup<'b>(bm: &'b BMesh, name: &str) -> Option<(Domain, &'b AttrData)> {
    [
        (Domain::Point, &bm.vdata),
        (Domain::Edge, &bm.edata),
        (Domain::Face, &bm.pdata),
        (Domain::Corner, &bm.ldata),
    ]
    .into_iter()
    .find_map(|(d, cd)| cd.get(name).map(|l| (d, &l.data)))
}

/// The selection of `edbm_select_by_attribute_exec` for the layer `name` of one BMesh:
/// returns whether anything was selected (`None` when the layer is missing, not boolean or on
/// the corner domain).
pub(crate) fn select_by_bool_layer(bm: &mut BMesh, name: &str) -> Option<bool> {
    let (domain, values) = match layer_lookup(bm, name)? {
        (d, AttrData::Bool(v)) if d != Domain::Corner => (d, v.clone()),
        _ => return None,
    };
    let elems: Vec<BMElem> = match domain {
        Domain::Point => bm.vert_ids().into_iter().map(BMElem::Vert).collect(),
        Domain::Edge => bm.edge_ids().into_iter().map(BMElem::Edge).collect(),
        Domain::Face => bm.face_ids().into_iter().map(BMElem::Face).collect(),
        Domain::Corner => return None,
    };
    let mut changed = false;
    for e in elems {
        if common::hidden(bm, e) || common::selected(bm, e) {
            continue;
        }
        if values.get(e.slot()).copied().unwrap_or(false) {
            bm.select_set(e, true);
            changed = true;
        }
    }
    if changed {
        common::selectmode_flush(bm);
    }
    Some(changed)
}

fn check(c: &Context) -> Result<(), PollFail> {
    poll::editmesh(c)?;
    let msg = |why: &str| PollFail {
        message: Some(format!(
            "Operator bpy.ops.mesh.select_by_attribute.poll() {why}"
        )),
        unmet: smallvec![],
        hint: Some(
            "make a BOOLEAN vertex, edge or face attribute active: \
             bpy.context.object.data.attributes.active = bpy.context.object.data.attributes['Name']"
                .into(),
        ),
    };
    let me = poll::edit_object(c)
        .and_then(|(_, o)| o.data?.typed::<MeshId>())
        .and_then(|m| c.main.meshes.get(m));
    let Some(name) = me.and_then(active_attribute_name) else {
        return Err(msg("There must be an active attribute"));
    };
    // In Edit Mode the BMesh layers are authoritative; the mesh's attribute set is the closest
    // view the poll has (attributes added in Edit Mode are checked again in exec).
    if let Some(a) = me.and_then(|m| m.attributes().get(&name)) {
        if !matches!(a.data, AttrData::Bool(_)) {
            return Err(msg("The active attribute must have a boolean type"));
        }
        if a.domain == frap_mesh::Domain::Corner {
            return Err(msg(
                "The active attribute must be on the vertex, edge, or face domain",
            ));
        }
    }
    Ok(())
}

/// `edbm_select_by_attribute_poll`.
const POLL: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check,
};

#[operator(
    poll = POLL,
    tier = T1,
    notes = [
        "selects by the mesh's active BOOLEAN attribute (mesh.attributes.active); set it in Object Mode before entering Edit Mode",
    ]
)]
fn exec(cx: &mut OpCtx, _p: mesh::SelectByAttribute) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        let Some(name) = cx.main().meshes.get(me).and_then(active_attribute_name) else {
            continue;
        };
        let usable = common::read(cx, me, |bm| {
            matches!(layer_lookup(bm, &name), Some((d, AttrData::Bool(_))) if d != Domain::Corner)
        })
        .unwrap_or(false);
        if usable {
            select_by_bool_layer(cx.bmesh_mut(me)?, &name);
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_bmesh::CdLayer;
    use std::sync::Arc;

    #[test]
    fn selects_true_visible_elements_of_a_bool_layer() {
        let mut bm = BMesh::new();
        let v: Vec<_> = (0..3)
            .map(|i| bm.vert_create([i as f32, 0.0, 0.0], None, Default::default()))
            .collect();
        bm.vdata.add_layer(CdLayer {
            name: "flag".into(),
            data: AttrData::Bool(Arc::new(vec![true, false, true])),
        });
        bm.vdata.add_layer(CdLayer {
            name: "weight".into(),
            data: AttrData::Float(Arc::new(vec![1.0; 3])),
        });
        bm.vert_mut(v[2]).flag.insert(frap_bmesh::ElemFlag::HIDDEN);
        assert_eq!(select_by_bool_layer(&mut bm, "flag"), Some(true));
        assert!(bm.is_selected(BMElem::Vert(v[0])));
        assert!(!bm.is_selected(BMElem::Vert(v[1])));
        assert!(!bm.is_selected(BMElem::Vert(v[2])));
        // Already selected: nothing changes.
        assert_eq!(select_by_bool_layer(&mut bm, "flag"), Some(false));
        assert_eq!(select_by_bool_layer(&mut bm, "weight"), None);
        assert_eq!(select_by_bool_layer(&mut bm, "missing"), None);
    }
}
