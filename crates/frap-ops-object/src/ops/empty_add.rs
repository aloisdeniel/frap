// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.empty_add(type='PLAIN_AXES', radius=1, align='WORLD', location=…,
//! rotation=…, scale=…)`, tier T0 (`tests/golden/cases/object/add/empty*.py`). Port of
//! `object_empty_add_exec` (`editors/object/object_add.cc`, Blender 5.2.4, GPL-2.0-or-later).
//! Verified on 5.2.4: an object `Empty` (uniquified) without data, `empty_display_type` =
//! `type`, `empty_display_size` = `radius` (`BKE_object_obdata_size_init`), object scale 1
//! (`scale` is ignored); placement like the primitives; selected, active, linked to
//! `context.collection`. The image settings of an `IMAGE` empty (`empty_image_offset`,
//! depth, side) are not modelled by frap's `Object` (no image empties before M8).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::EmptyAdd) -> OpResult {
    let align = helpers::Align::from_identifier(p.align.value.identifier())
        .unwrap_or(helpers::Align::World);
    let at = super::camera_add::common::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    let ob = helpers::add_object_with_data(cx, "Empty", None, at, false)?;
    let o = cx.object_mut(ob)?;
    o.empty_display_type = p.r#type.value.identifier().into();
    o.empty_display_size *= p.radius.value;
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}
