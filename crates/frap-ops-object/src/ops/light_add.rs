// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.light_add(type='POINT', radius=1, align='WORLD', location=…, rotation=…,
//! scale=…)`, tier T0 (`tests/golden/cases/object/add/light*.py`). Port of
//! `object_light_add_exec` and the light branch of `BKE_object_obdata_size_init`
//! (`editors/object/object_add.cc`, `blenkernel/intern/object.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - object and light data are both requested as the type's name (`Point`, `Sun`, `Spot`,
//!   `Area`) and uniquified independently; the data has the values of
//!   `bpy.data.lights.new(name, type)` (10 W) except a Sun, which gets 1.0 (strength);
//! - `radius` scales the size-like light values by `radius` (Point, Spot), `radius × 0.5`
//!   (Sun) or `radius × 4` (Area): only an Area light shows it (`size`, `size_y` 0.25 → `radius`),
//!   because `shadow_soft_size` starts at 0; object scale stays 1 and `scale` is ignored;
//! - placement like the primitives (cursor, `align`, set `rotation` wins); selected, active,
//!   linked to `context.collection`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::Light;
use frap_spec_ops_object::object::{self, light_add::Type};

/// `get_light_defname`: the object and data name of a light type.
fn default_name(t: Type) -> &'static str {
    match t {
        Type::Point => "Point",
        Type::Sun => "Sun",
        Type::Spot => "Spot",
        Type::Area => "Area",
    }
}

/// The light data `light_add` creates: `bpy.data.lights.new` values, then
/// `BKE_object_obdata_size_init(ob, size)` with the operator's per-type size, then the Sun's
/// strength.
fn light_data(name: &str, t: Type, radius: f32) -> Light {
    let mut l = Light::new(name, t.identifier());
    let size = match t {
        Type::Point | Type::Spot => radius,
        Type::Area => radius * 4.0,
        Type::Sun => radius * 0.5,
    };
    l.fields.shadow_soft_size *= size;
    l.fields.size *= size;
    l.fields.size_y *= size;
    if t == Type::Sun {
        l.fields.energy = 1.0;
    }
    l
}

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::LightAdd) -> OpResult {
    let align = helpers::Align::from_identifier(p.align.value.identifier())
        .unwrap_or(helpers::Align::World);
    let at = super::camera_add::common::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    let t = p.r#type.value;
    let name = default_name(t);
    let data = {
        let (main, log) = cx.main_mut();
        main.lights
            .insert(name, light_data(name, t, p.radius.value), log)
    };
    let ob = helpers::add_object_with_data(cx, name, Some(data.any()), at, false)?;
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_type_values() {
        let sun = light_data("Sun", Type::Sun, 3.0);
        assert_eq!(sun.fields.energy, 1.0);
        let point = light_data("Point", Type::Point, 3.0);
        assert_eq!(point.fields.energy, Light::new("x", "POINT").fields.energy);
        assert_eq!(point.fields.shadow_soft_size, 0.0);
        // Verified on 5.2.4: an Area light's size equals the radius (0.25 × 4 × radius).
        let area = light_data("Area", Type::Area, 3.0);
        assert_eq!((area.fields.size, area.fields.size_y), (3.0, 3.0));
        assert_eq!(light_data("Area", Type::Area, 1.0).fields.size, 1.0);
    }
}
