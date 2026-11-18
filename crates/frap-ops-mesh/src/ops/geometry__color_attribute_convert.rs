// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.color_attribute_convert(domain='POINT', data_type='FLOAT_COLOR')`, tier
//! T1 (`tests/golden/cases/edit/normals/color_attribute_*.py`). Port of
//! `geometry_color_attribute_convert_exec` / `_poll` with `convert_attribute`
//! (`editors/geometry/geometry_attributes.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Object Mode only (`Operation is not allowed in edit mode`). Re-creates the **active colour
//! attribute** (poll: it names a colour attribute) on `domain` (POINT ↔ CORNER: corners take
//! their vertex's colour, vertices the average of their corners) as `data_type` (FLOAT_COLOR ↔
//! BYTE_COLOR, sRGB-encoded); it moves to the end of the attribute list and stays the active
//! (and, if it was, the default) colour.

use frap_mesh::Domain;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::geometry::{self, color_attribute_convert};

use super::geometry__attribute_add::{attrs, common, convert};
use super::geometry__color_attribute_remove::active_color_found;

const IDNAME: &str = "geometry.color_attribute_convert";

/// `geometry_color_attribute_convert_poll`.
fn convert_poll(c: &Context) -> Result<(), PollFail> {
    common::attributes_poll(c)?;
    common::not_in_edit(c, IDNAME)?;
    let me = common::context_mesh_data(c).expect("checked by attributes_poll");
    if !active_color_found(me, &attrs::Names::of(me)) {
        return Err(common::poll_fail(
            "bpy.ops.geometry.color_attribute_add() or set mesh.color_attributes.active_color",
        ));
    }
    Ok(())
}

const CONVERT_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: convert_poll,
};

#[operator(poll = CONVERT_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: geometry::ColorAttributeConvert) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let domain = match p.domain.value {
        color_attribute_convert::Domain::Point => Domain::Point,
        color_attribute_convert::Domain::Corner => Domain::Corner,
    };
    let data_type = match p.data_type.value {
        color_attribute_convert::DataType::FloatColor => "FLOAT_COLOR",
        color_attribute_convert::DataType::ByteColor => "BYTE_COLOR",
    };
    let m = cx.mesh_mut(me)?;
    let mut names = attrs::Names::of(m);
    let Some(name) = names.color_active.clone() else {
        return Ok(OpStatus::Cancelled);
    };
    convert::convert_attribute(m, &mut names, &name, domain, data_type)
        .map_err(OpError::Runtime)?;
    names.write(m);
    Ok(OpStatus::Finished)
}
