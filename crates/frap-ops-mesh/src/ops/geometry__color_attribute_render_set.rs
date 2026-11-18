// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.color_attribute_render_set(name="Color")`, tier T1
//! (`tests/golden/cases/edit/normals/color_attribute_*.py`). Port of
//! `geometry_color_attribute_set_render_exec` (`editors/geometry/geometry_attributes.cc`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Makes the colour attribute `name` the **default (render) colour**
//! (`color_attributes.default_color_name`, exported by glTF as `COLOR_0`). A name that is not a
//! colour attribute (FLOAT_COLOR / BYTE_COLOR on POINT / CORNER) answers `CANCELLED`
//! (Blender is silent; frap adds a WARNING naming the colour attributes). In Edit Mode the
//! edit BMesh's layers are looked up.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::geometry;

use super::geometry__attribute_add::{attrs, common};

#[operator(poll = common::ATTRIBUTES_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: geometry::ColorAttributeRenderSet) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let name = p.name.value.clone();
    let colors = common::read(cx, me, |store, _| {
        store
            .layers()
            .into_iter()
            .filter(|l| attrs::is_color(l.domain, l.data_type))
            .map(|l| l.name)
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    if !colors.contains(&name) {
        let list = colors
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");
        cx.report(
            ReportType::Warning,
            format!(
                "frap: '{name}' is not a colour attribute of this mesh (colour attributes: [{list}])"
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    cx.mesh_mut(me)?.color_default = Some(name);
    Ok(OpStatus::Finished)
}
