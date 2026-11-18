// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.attribute_set(value_float=0, value_float_vector_2d=(0, 0), …,
//! value_color=(1, 1, 1, 1), value_bool=False)`, tier T1
//! (`tests/golden/cases/edit/normals/attribute_set_*.py`). Port of `mesh_set_attribute_exec`,
//! `mesh_active_attribute_poll` and the selected-value setters
//! (`editors/mesh/editmesh_attribute.cc`) with `rna_property_for_attribute_type_retrieve_value`
//! (`editors/geometry/geometry_attributes.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Sets the **active attribute** (of the edit object's mesh) on the selected elements of its
//! domain, in every mesh in Edit Mode (unique data) that has an attribute of that name:
//! - the value is read from the property of the active attribute's type (`value_float` for
//!   FLOAT, `value_float_vector_2d/3d/4d` for FLOAT2 / FLOAT_VECTOR / FLOAT4, `value_int` for
//!   INT and INT8, `value_int_vector_2d` for INT16_2D and INT32_2D, `value_color` for colours,
//!   sRGB-encoded for BYTE_COLOR, `value_bool` for BOOLEAN), then converted implicitly to the
//!   type of each mesh's attribute;
//! - POINT / EDGE / FACE: the selected vertices, edges or faces; CORNER: in face select mode
//!   the corners of selected faces, in vertex or edge select mode the corners of selected
//!   vertices.
//!
//! Poll: Edit Mode with an active attribute that is not STRING, FLOAT4X4 or QUATERNION
//! (`No active attribute`, `The active attribute has an unsupported type`). Nothing written →
//! `CANCELLED`.

use frap_bmesh::{BMesh, ElemFlag, SelectMode};
use frap_mesh::Domain;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::edit_meshes_where;
use super::geometry__attribute_add::attrs::{self, Store};
use super::geometry__attribute_add::common;
use super::geometry__attribute_add::convert::{self, Val};

const IDNAME: &str = "mesh.attribute_set";

/// The poll messages for an active attribute of `data_type` (`None`: no active attribute).
fn check_type(data_type: Option<&str>) -> Result<(), &'static str> {
    match data_type {
        None => Err("No active attribute"),
        Some("STRING" | "FLOAT4X4" | "QUATERNION") => {
            Err("The active attribute has an unsupported type")
        }
        Some(_) => Ok(()),
    }
}

/// The active attribute's type in `store`, if any.
fn active_type(store: &dyn Store, names: &attrs::Names) -> Option<&'static str> {
    let name = attrs::active_name(store, names)?;
    store
        .layers()
        .into_iter()
        .find(|l| l.name == name)
        .map(|l| l.data_type)
}

/// `mesh_active_attribute_poll` (the active attribute is checked on the mesh in `Main`;
/// `exec` re-checks it on the edit BMesh).
fn set_poll(c: &Context) -> Result<(), PollFail> {
    poll::editmesh(c)?;
    let me = common::context_mesh_data(c).expect("checked by editmesh");
    let names = attrs::Names::of(me);
    if names.active.is_none() {
        return Err(common::poll_msg(
            IDNAME,
            "No active attribute",
            "set mesh.attributes.active (or bpy.ops.geometry.attribute_add(...)) first",
        ));
    }
    Ok(())
}

const SET_POLL: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check: set_poll,
};

/// The operator's value for an attribute of `data_type`.
fn value_for(p: &mesh::AttributeSet, data_type: &str) -> Option<Val> {
    Some(match data_type {
        "FLOAT" => Val::F(p.value_float.value),
        "FLOAT2" => Val::F2(p.value_float_vector_2d.value),
        "FLOAT_VECTOR" => Val::F3(p.value_float_vector_3d.value),
        "FLOAT4" => Val::F4(p.value_float_vector_4d.value),
        "FLOAT_COLOR" => Val::C(p.value_color.value),
        "BYTE_COLOR" => Val::Bc(attrs::encode_byte_color(p.value_color.value)),
        "BOOLEAN" => Val::B(p.value_bool.value),
        "INT8" => Val::I8(p.value_int.value as i8),
        "INT" => Val::I(p.value_int.value),
        "INT16_2D" => {
            let v = p.value_int_vector_2d.value;
            Val::S2([v[0] as i16, v[1] as i16])
        }
        "INT32_2D" => Val::I2(p.value_int_vector_2d.value),
        _ => return None,
    })
}

/// The slots of the elements `attribute_set` writes on `domain`.
fn selected_slots(bm: &BMesh, domain: Domain) -> Vec<usize> {
    let sel = |f: ElemFlag| f.contains(ElemFlag::SELECT);
    match domain {
        Domain::Point => bm
            .vert_ids()
            .into_iter()
            .filter(|&v| sel(bm.vert(v).flag))
            .map(|v| v.slot())
            .collect(),
        Domain::Edge => bm
            .edge_ids()
            .into_iter()
            .filter(|&e| sel(bm.edge(e).flag))
            .map(|e| e.slot())
            .collect(),
        Domain::Face => bm
            .face_ids()
            .into_iter()
            .filter(|&f| sel(bm.face(f).flag))
            .map(|f| f.slot())
            .collect(),
        Domain::Corner => {
            let mut out = Vec::new();
            if bm.select_mode.contains(SelectMode::FACE) {
                for f in bm.face_ids() {
                    if sel(bm.face(f).flag) {
                        out.extend(bm.face_loops(f).into_iter().map(|l| l.slot()));
                    }
                }
            }
            if bm
                .select_mode
                .intersects(SelectMode::VERT | SelectMode::EDGE)
            {
                for v in bm.vert_ids() {
                    if sel(bm.vert(v).flag) {
                        out.extend(bm.vert_loops(v).into_iter().map(|l| l.slot()));
                    }
                }
            }
            out
        }
    }
}

#[operator(poll = SET_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::AttributeSet) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let active = common::read(cx, me, |s, n| {
        let t = active_type(s, n);
        (attrs::active_name(s, n), t)
    })
    .unwrap_or((None, None));
    if !common::in_edit(cx, me) {
        return Err(common::exec_poll_failed(IDNAME));
    }
    if let Err(m) = check_type(active.1) {
        return Err(common::exec_poll_error(IDNAME, m));
    }
    let (Some(name), Some(data_type)) = active else {
        return Err(common::exec_poll_error(IDNAME, "No active attribute"));
    };
    let value = value_for(&p, data_type).ok_or_else(|| {
        common::exec_poll_error(IDNAME, "The active attribute has an unsupported type")
    })?;
    let mut changed = false;
    for m in edit_meshes_where(cx, |bm| Store::column(bm, &name).is_some()) {
        let bm = cx.bmesh_mut(m)?;
        let Some((domain, mut data)) = Store::column(bm, &name) else {
            continue;
        };
        let Some(v) = convert::convert(value, data.data_type()) else {
            continue;
        };
        for s in selected_slots(bm, domain) {
            convert::put(&mut data, s, v);
        }
        Store::replace(bm, &name, data);
        changed = true;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
