// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.add(radius=1, type='EMPTY', enter_editmode=False, align='WORLD',
//! location=…, rotation=…, scale=…)`, tier T0 (`tests/golden/cases/object/add_m8/add_*.py`).
//! Ported from blender/source/blender/editors/object/object_add.cc (`object_add_exec`,
//! `add_type_with_obdata`) and blenkernel/intern/object.cc (`BKE_object_add`,
//! `BKE_object_obdata_add_from_type`, `BKE_object_obdata_size_init`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `ED_operator_objectmode`; the object and its data are both requested as the type's
//!   default name (`Mesh`, `Camera`, `Light`, `Armature`, `PointCloud`; `Empty` without data)
//!   and uniquified independently, with the values of a fresh datablock: an empty mesh, a
//!   point light (10 W), an armature without bones or bone collections (its pose has no
//!   channels), a point cloud without points;
//! - `radius` scales the size-like values (`BKE_object_obdata_size_init`): an empty's
//!   `empty_display_size`, a camera's `display_size`, a light's `shadow_soft_size`, `size`,
//!   `size_y` (a point light's soft size starts at 0); nothing else (the object's scale stays
//!   1, `scale` is ignored);
//! - placement like the other add operators (cursor, `align`, a set `rotation` wins);
//!   `scene.camera` is not set for a camera;
//! - `enter_editmode` enters Edit Mode for a mesh or an armature and is ignored by types
//!   without an Edit Mode (camera, light, empty);
//! - the types frap has no data for (`CURVE`, `SURFACE`, `META`, `FONT`, `CURVES`, `VOLUME`,
//!   `GREASEPENCIL`, `LATTICE`, `LIGHT_PROBE`, `SPEAKER`) answer `NotImplementedError`
//!   (`OP_NOT_IMPLEMENTED`) naming the type, before anything changes; so does
//!   `enter_editmode=True` with a point cloud (frap has no point-cloud Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_mesh::Mesh;
use frap_rna::IdAny;
use frap_scene::ObjectMode;
use frap_scene::id::{Armature, Camera, Light, Pose, PointCloud};
use frap_spec_ops_object::object::{self, add::Type};

/// The datablock `BKE_object_obdata_add_from_type` creates, or `None` for an empty; `Err` for
/// a type frap has no data for.
enum NewData {
    Empty,
    Mesh,
    Camera,
    Light,
    Armature,
    PointCloud,
}

/// `get_obdata_defname` and the data frap can create for `t`.
fn new_data(t: Type) -> Result<(&'static str, NewData), OpError> {
    Ok(match t {
        Type::Empty => ("Empty", NewData::Empty),
        Type::Mesh => ("Mesh", NewData::Mesh),
        Type::Camera => ("Camera", NewData::Camera),
        Type::Light => ("Light", NewData::Light),
        Type::Armature => ("Armature", NewData::Armature),
        Type::Pointcloud => ("PointCloud", NewData::PointCloud),
        other => {
            return Err(OpError::NotImplemented(format!(
                "frap: object.add(type='{}') is not implemented: frap has no {} data yet",
                other.identifier(),
                match other {
                    Type::Curve | Type::Surface | Type::Font => "curve",
                    Type::Meta => "metaball",
                    Type::Curves => "hair curves",
                    Type::Volume => "volume",
                    Type::Greasepencil => "grease pencil",
                    Type::Lattice => "lattice",
                    Type::LightProbe => "light probe",
                    _ => "speaker",
                }
            )));
        }
    })
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    support = Partial("types with data in frap: MESH, CAMERA, LIGHT, ARMATURE, EMPTY, POINTCLOUD; the others answer OP_NOT_IMPLEMENTED naming the type"),
    notes = ["radius scales the empty display size, the camera display size and the light sizes (BKE_object_obdata_size_init)"]
)]
fn exec(cx: &mut OpCtx, p: object::Add) -> OpResult {
    let (name, kind) = new_data(p.r#type.value)?;
    let edit = p.enter_editmode.value && matches!(kind, NewData::Mesh | NewData::Armature);
    if p.enter_editmode.value && matches!(kind, NewData::PointCloud) {
        // `editmode_enter_ex` on a point cloud: frap has no handler (fail before changing).
        helpers::mode_handler(ObjectMode::Edit, "POINTCLOUD")?;
    }
    let align = helpers::Align::from_identifier(p.align.value.identifier())
        .unwrap_or(helpers::Align::World);
    let at = super::camera_add::common::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    let radius = p.radius.value;
    let data: Option<IdAny> = {
        let (main, log) = cx.main_mut();
        match kind {
            NewData::Empty => None,
            NewData::Mesh => Some(main.meshes.insert(name, Mesh::new(name), log).any()),
            NewData::Camera => {
                let mut c = Camera::new(name);
                c.fields.display_size *= radius;
                Some(main.cameras.insert(name, c, log).any())
            }
            NewData::Light => {
                let mut l = Light::new(name, "POINT");
                l.fields.shadow_soft_size *= radius;
                l.fields.size *= radius;
                l.fields.size_y *= radius;
                Some(main.lights.insert(name, l, log).any())
            }
            NewData::Armature => Some(
                main.armatures
                    .insert(name, Armature::new(name), log)
                    .any(),
            ),
            NewData::PointCloud => Some(
                main.pointclouds
                    .insert(name, PointCloud::new(name), log)
                    .any(),
            ),
        }
    };
    let ob = helpers::add_object_with_data(cx, name, data, at, edit)?;
    let o = cx.object_mut(ob)?;
    match kind {
        NewData::Empty => o.empty_display_size *= radius,
        // `BKE_pose_rebuild` at the first evaluation: a pose without channels.
        NewData::Armature if !edit => Pose::rebuild(&mut o.pose, &Armature::new(name)),
        _ => {}
    }
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_without_data_are_not_implemented() {
        for t in Type::ALL {
            let r = new_data(t);
            let has = matches!(
                t,
                Type::Empty
                    | Type::Mesh
                    | Type::Camera
                    | Type::Light
                    | Type::Armature
                    | Type::Pointcloud
            );
            assert_eq!(r.is_ok(), has, "{t:?}");
            if let Err(OpError::NotImplemented(m)) = r {
                assert!(m.contains(t.identifier()), "{m}");
            }
        }
    }
}
