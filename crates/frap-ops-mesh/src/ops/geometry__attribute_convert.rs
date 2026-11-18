// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.attribute_convert(mode='GENERIC', domain='POINT', data_type='FLOAT')`,
//! tier T1 (`tests/golden/cases/edit/normals/attribute_*.py`). Port of
//! `geometry_attribute_convert_exec`, `geometry_attribute_convert_poll` and
//! `convert_attribute` (`editors/geometry/geometry_attributes.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Object Mode only (`Operation is not allowed in edit mode`), on the **active attribute**
//! (poll: there is one):
//! - `GENERIC`: the attribute is re-created on `domain` as `data_type`: values interpolated
//!   between domains (booleans with Blender's any-of / all-of rules, numbers averaged), then
//!   converted with the implicit type conversions; it moves to the end of the attribute list
//!   and stays active (and the active/default colour or active/render UV map when it was one).
//!   `STRING` is refused (`Cannot convert to the selected type`); `CURVE`, `INSTANCE` and
//!   `LAYER` are not mesh domains.
//! - `VERTEX_GROUP`: the attribute's values on POINT become the weights (above 0) of a new
//!   vertex group of the same name, which becomes the object's active vertex group; the
//!   attribute is removed and the activity moves one index down (to the nearest attribute that
//!   is not internal).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::geometry::{self, attribute_convert};

use super::geometry__attribute_add::{attrs, common, convert, mesh_domain};

const IDNAME: &str = "geometry.attribute_convert";

/// `geometry_attribute_convert_poll`.
fn convert_poll(c: &Context) -> Result<(), PollFail> {
    common::attributes_poll(c)?;
    common::not_in_edit(c, IDNAME)?;
    let me = common::context_mesh_data(c).expect("checked by attributes_poll");
    if attrs::active_name(me, &attrs::Names::of(me)).is_none() {
        return Err(common::poll_fail(
            "set mesh.attributes.active (or bpy.ops.geometry.attribute_add(...)) first",
        ));
    }
    Ok(())
}

const CONVERT_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: convert_poll,
};

#[operator(
    poll = CONVERT_POLL,
    tier = T1,
    dyn_enums = [("domain", super::geometry__attribute_add::mesh_domain_items)]
)]
fn exec(cx: &mut OpCtx, p: geometry::AttributeConvert) -> OpResult {
    let Some((ob, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let m = cx.mesh_mut(me)?;
    let mut names = attrs::Names::of(m);
    let Some(name) = attrs::active_name(m, &names) else {
        return Ok(OpStatus::Cancelled);
    };
    match p.mode.value {
        attribute_convert::Mode::Generic => {
            let Some(domain) = mesh_domain(p.domain.value.identifier()) else {
                return Err(super::geometry__attribute_add::domain_enum_error(
                    p.domain.value.identifier(),
                ));
            };
            let data_type = p.data_type.value.identifier();
            convert::convert_attribute(m, &mut names, &name, domain, data_type)
                .map_err(OpError::Runtime)?;
            names.active = Some(name);
            names.write(m);
            Ok(OpStatus::Finished)
        }
        attribute_convert::Mode::VertexGroup => {
            let src = m.attributes().get(&name).cloned().expect("active exists");
            let weights: Vec<f32> = match convert::lookup_converted(
                m,
                &src.data,
                src.domain,
                frap_mesh::Domain::Point,
                "FLOAT",
            ) {
                Some(frap_mesh::AttrData::Float(v)) => v.as_ref().clone(),
                _ => vec![0.0; m.verts_num()],
            };
            let before = attrs::Store::layers(m);
            let mut index = attrs::index_of(&before, Some(&name));
            attrs::remove(m, &mut names, &name).map_err(OpError::Runtime)?;
            let group_name = frap_rna::naming::unique_subname(
                &|n: &str| m.vertex_group_names.iter().any(|g| g == n),
                &name,
                attrs::MAX_NAME,
                "Group",
            );
            m.vertex_group_names.push(group_name);
            let group = (m.vertex_group_names.len() - 1) as u32;
            let dverts = m.deform_verts_mut();
            for (dv, &w) in dverts.iter_mut().zip(&weights) {
                if w > 0.0 {
                    dv.weights.push((group, w));
                }
            }
            if index > 0 {
                index -= 1;
            }
            let after = attrs::Store::layers(m);
            let index = attrs::validate_index(&after, index);
            names.active = attrs::name_at(&after, index, false);
            names.write(m);
            cx.object_mut(ob)?.vertex_groups_active_index = group as i32;
            Ok(OpStatus::Finished)
        }
    }
}
