// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.origin_set(type='GEOMETRY_ORIGIN', center='MEDIAN')`, tier T1
//! (`tests/golden/cases/object/origin/*.py`). Port of `object_origin_set_exec`
//! (`editors/object/object_transform.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4 (mesh objects; frap has no curves, fonts, lattices or metaballs yet):
//! - `GEOMETRY_ORIGIN` moves the mesh so that its center (median of the vertices, or the
//!   middle of their bounds) is at the origin; the object does not move;
//! - `ORIGIN_GEOMETRY`, `ORIGIN_CURSOR` (the 3D cursor in object space),
//!   `ORIGIN_CENTER_OF_MASS` (surface centroid) and `ORIGIN_CENTER_OF_VOLUME` move the mesh by
//!   minus that point and the object's location by it (through its rotation and scale), so
//!   nothing moves in world space; children keep their world matrix (`ignore_parent_tx`);
//! - an unset `center` follows the pivot point (`BOUNDING_BOX_CENTER` → bounds);
//! - the active object goes first; another selected object sharing its mesh is only moved
//!   (by the offset through its own world rotation and scale), the mesh is not moved twice;
//! - objects without mesh data are left alone (`Set Origin not supported for Camera
//!   object(s)` INFO for cameras, lights, …); nothing selected → `CANCELLED`;
//! - in Edit Mode only `GEOMETRY_ORIGIN` is allowed (`Operation cannot be performed in edit
//!   mode`; Edit Mode arrives in M2).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdCode, IdRef, MeshId, ObjectId};
use frap_scene::Context;
use frap_spec_ops_object::object::{self, origin_set::{Center, Type}};
use glam::{Mat3, Vec3};

use crate::support::xform::{self, Worlds};
use crate::support::{center, mesh};

/// `BKE_idtype_idcode_to_name_plural`-like singular names used by the unsupported-type report.
fn type_name(code: IdCode) -> &'static str {
    match code {
        IdCode::CA => "Camera",
        IdCode::LA => "Light",
        IdCode::AR => "Armature",
        IdCode::PT => "PointCloud",
        _ => "Object",
    }
}

/// The selected editable objects, the active object first.
fn objects(c: &Context) -> Vec<IdRef<ObjectId>> {
    let mut objs = c.selected_editable_objects();
    if objs.len() > 1
        && let Some(a) = c.active_object()
        && let Some(i) = objs.iter().position(|o| *o == a)
    {
        let a = objs.remove(i);
        objs.insert(0, a);
    }
    objs
}

#[operator(
    poll = poll::SCENE_EDITABLE,
    tier = T1,
    support = Partial(
        "mesh objects in Object Mode; Edit Mode arrives in M2; frap has no curves, fonts, \
         lattices, metaballs or collection instances to center"
    )
)]
fn exec(cx: &mut OpCtx, p: object::OriginSet) -> OpResult {
    let mode = p.r#type.value;
    let (objs, around_bounds, cursor, in_edit) = {
        let c = cx.ctx();
        let around_bounds = if p.center.is_set {
            p.center.value == Center::Bounds
        } else {
            c.tool_settings().transform_pivot_point == "BOUNDING_BOX_CENTER"
        };
        (
            objects(&c),
            around_bounds,
            Vec3::from_array(c.cursor().location),
            c.edit_object().is_some(),
        )
    };
    if in_edit {
        if mode != Type::GeometryOrigin {
            return Err(OpError::Runtime(
                "Operation cannot be performed in edit mode".into(),
            ));
        }
        return Err(OpError::NotImplemented(
            "frap: origin_set in Edit Mode arrives with Edit Mode in M2".into(),
        ));
    }
    if objs.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing to set: no visible, editable object is selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    let mut worlds = Worlds::of(cx.main());
    let mut done: Vec<IdRef<ObjectId>> = Vec::new();
    let mut done_meshes: Vec<IdRef<MeshId>> = Vec::new();
    let mut unsupported: Vec<&'static str> = Vec::new();
    let mut tot_lib_error = 0usize;
    let mut tot_change = 0usize;
    let (main, log) = cx.main_mut();
    for &ob in &objs {
        if done.contains(&ob) {
            continue;
        }
        done.push(ob);
        let Some(o) = main.objects.get(ob) else {
            continue;
        };
        let world = worlds.get(ob);
        let mut cent = if mode == Type::OriginCursor {
            world.inverse().transform_point3(cursor)
        } else {
            Vec3::ZERO
        };
        let Some(data) = o.data else {
            // Empties (collection instances are not supported by frap).
            continue;
        };
        if main.header(data).is_some_and(|h| h.library.is_some()) {
            tot_lib_error += 1;
            continue;
        }
        let Some(m) = data.typed::<MeshId>().filter(|_| data.code == IdCode::ME) else {
            let name = type_name(data.code);
            if !unsupported.contains(&name) {
                unsupported.push(name);
            }
            continue;
        };
        if !done_meshes.contains(&m) {
            let Some(me) = main.meshes.get(m) else {
                continue;
            };
            cent = match mode {
                Type::OriginCursor => cent,
                Type::OriginCenterOfMass => center::of_surface(me),
                Type::OriginCenterOfVolume => center::of_volume(me),
                _ if around_bounds => center::bounds(me),
                _ => center::median(me),
            };
            if let Some(me) = main.meshes.get_mut(m, log) {
                mesh::translate(me, -cent);
            }
            done_meshes.push(m);
            tot_change += 1;
        } else {
            // The mesh was already centered (the other users were offset with it).
            continue;
        }
        if mode == Type::GeometryOrigin {
            continue;
        }
        // Offset the object (in its parent space) so that nothing moves in world space.
        let Some(o) = main.objects.get_mut(ob, log) else {
            continue;
        };
        let centn = o.transform().to_mat3() * cent;
        o.location = (Vec3::from_array(o.location) + centn).to_array();
        let pw = o.parent.map(|p| worlds.get(p));
        let new = xform::where_is(o, pw.as_ref());
        worlds.set(ob, new);
        xform::ignore_parent_tx(main, log, ob, &new, &worlds);
        // Other selected users of the same mesh move with it.
        let others: Vec<IdRef<ObjectId>> = objs
            .iter()
            .copied()
            .filter(|x| !done.contains(x))
            .filter(|x| main.objects.get(*x).and_then(|o| o.data) == Some(m.any()))
            .collect();
        for other in others {
            done.push(other);
            let w = Mat3::from_mat4(worlds.get(other));
            let Some(oo) = main.objects.get_mut(other, log) else {
                continue;
            };
            oo.location = (Vec3::from_array(oo.location) + w * cent).to_array();
            let pw = oo.parent.map(|p| worlds.get(p));
            let new = xform::where_is(oo, pw.as_ref());
            worlds.set(other, new);
            xform::ignore_parent_tx(main, log, other, &new, &worlds);
        }
    }
    for name in unsupported {
        cx.report(
            ReportType::Info,
            format!("Set Origin not supported for {name} object(s)"),
        );
    }
    if tot_lib_error > 0 {
        cx.report(
            ReportType::Warning,
            format!("{tot_lib_error} object(s) not centered, {tot_change} changed:"),
        );
        cx.report(
            ReportType::Warning,
            format!("|{tot_lib_error} linked library object(s)"),
        );
    }
    Ok(OpStatus::Finished)
}
