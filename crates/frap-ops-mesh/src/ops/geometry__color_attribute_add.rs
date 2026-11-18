// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.color_attribute_add(name="", domain='POINT', data_type='FLOAT_COLOR',
//! color=(0, 0, 0, 1))`, tier T1 (`tests/golden/cases/edit/normals/color_attribute_*.py`).
//! Port of `geometry_color_attribute_add_exec` (`editors/geometry/geometry_attributes.cc`) and
//! `object_active_color_init` / `fill_mesh_color`
//! (`editors/sculpt_paint/mesh/paint_vertex.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Defaults ([MESH §4.10]): an empty name becomes `Attribute` (`.001`… on a clash), FLOAT_COLOR
//! on POINT, filled with opaque black. The new attribute becomes the **active colour**, and the
//! **default (render) colour** when that names no colour attribute; the active attribute is
//! unchanged. The fill writes every face corner (CORNER) or every vertex used by a face
//! (POINT: loose vertices keep the attribute default); a BYTE_COLOR is sRGB-encoded. In Edit
//! Mode the attribute is a BMesh layer and the edit-mode name checks of
//! `geometry.attribute_add` apply. glTF exports the default colour as `COLOR_0`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::geometry::{self, color_attribute_add};

use frap_mesh::Domain;

use super::geometry__attribute_add::{add, attrs, common, mesh_domain};

#[operator(poll = common::ATTRIBUTES_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: geometry::ColorAttributeAdd) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let domain: Option<Domain> = mesh_domain(match p.domain.value {
        color_attribute_add::Domain::Point => "POINT",
        color_attribute_add::Domain::Corner => "CORNER",
    });
    let data_type = match p.data_type.value {
        color_attribute_add::DataType::FloatColor => "FLOAT_COLOR",
        color_attribute_add::DataType::ByteColor => "BYTE_COLOR",
    };
    let requested = p.name.value.clone();
    let color = p.color.value;
    let r = common::with_store(cx, me, |store, names, vgroups| {
        let name = add(store, vgroups, &requested, domain, data_type)?;
        names.color_active = Some(name.clone());
        let list = store.layers();
        let default_found = names.color_default.as_deref().is_some_and(|n| {
            list.iter().any(|l| {
                l.name == n && attrs::is_color(l.domain, l.data_type)
            })
        });
        if !default_found {
            names.color_default = Some(name.clone());
        }
        store.fill_color(&name, color);
        Ok::<_, String>(())
    })?;
    match r {
        Ok(()) => Ok(OpStatus::Finished),
        Err(m) => Err(OpError::Runtime(m)),
    }
}
