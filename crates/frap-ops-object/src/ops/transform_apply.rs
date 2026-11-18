// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.transform_apply(location=True, rotation=True, scale=True, properties=True,
//! corrective_flip_normals=True, isolate_users=False)`, tier T1
//! (`tests/golden/cases/object/apply/*.py`). Port of `object_transform_apply_exec`,
//! `apply_objects_internal`, `apply_objects_internal_can_multiuser` and `ignore_parent_tx`
//! (`editors/object/object_transform.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4:
//! - the selected editable objects' channels are baked into their data and reset, deltas
//!   included (`delta_location` 0, `delta_scale` 1, delta rotations identity); a rotation-only
//!   apply keeps the scale out of the data (`S⁻¹ · R · S`), a location-only apply moves the
//!   data by the location expressed in the object's rotated, scaled space;
//! - a negative determinant flips the face winding unless `corrective_flip_normals=False`;
//! - children keep their world matrix: their channels take its decomposition and their parent
//!   inverse the inverse of the parent's new matrix, which Blender computes with the parent's
//!   **old delta channels** (a parent with deltas therefore moves its children, reproduced);
//! - shared data: `Cannot apply to a multi user: Object "Cube", Mesh "Cube", aborting` (one
//!   report per object, nothing changes) unless every selected object shares the active
//!   object's data, then the data is transformed once and the other users get the inverse
//!   transform; `isolate_users=True` gives the object a copy of its data (`Cube.001`) first;
//! - empties multiply `empty_display_size` by their largest absolute scale; cameras and
//!   non-area lights are skipped; area lights accept only a scale (`Area Lights can only have
//!   scale applied: "Light"`), which goes into `size`/`size_y` (a non-square scale turns a
//!   square into a rectangle);
//! - nothing to apply → `CANCELLED` (`Objects have no data to transform` when the selection
//!   has no supported data); all three toggles off → `FINISHED`, nothing changes.
//!
//! `properties=True` only concerns curves, fonts and armature envelopes (none in M1); applying
//! to armatures arrives with them in M6.

use frap_math::matrix::is_negative_m3;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdAny, IdCode, IdRef, MeshId, ObjectId};
use frap_scene::id::Object;
use frap_scene::{ChangeLog, Context, Main};
use frap_spec_ops_object::object;
use glam::{Mat3, Mat4, Vec3};

use crate::support::mesh;
use crate::support::xform::{self, Worlds};

/// `BKE_idtype_idcode_to_name`.
fn id_type_name(code: IdCode) -> &'static str {
    match code {
        IdCode::ME => "Mesh",
        IdCode::CA => "Camera",
        IdCode::LA => "Light",
        IdCode::AR => "Armature",
        IdCode::PT => "PointCloud",
        _ => "ID",
    }
}

/// `apply_objects_internal_can_multiuser`: the active object has shared data, is selected, and
/// the selected editable objects are exactly the users of that data (verified: with an
/// unselected user left, Blender reports the multi-user error).
fn can_multiuser(c: &Context, objects: &[IdRef<ObjectId>]) -> bool {
    let Some(act) = c.active_object() else {
        return false;
    };
    let Some(data) = c.main.objects.get(act).and_then(|o| o.data) else {
        return false;
    };
    let users = c.main.users(data) as usize;
    if users <= 1 || !objects.contains(&act) || users != objects.len() {
        return false;
    }
    objects
        .iter()
        .all(|o| c.main.objects.get(*o).and_then(|o| o.data) == Some(data))
}

/// The matrix applied to an object's data (`rsmat` + translation of `apply_objects_internal`).
fn apply_matrix(o: &Object, loc: bool, rot: bool, sca: bool) -> (Mat3, Mat4) {
    let t = o.transform();
    let size = Vec3::from_array(o.scale) * Vec3::from_array(o.delta_scale);
    let rsmat = if sca && rot {
        t.to_mat3()
    } else if sca {
        Mat3::from_diagonal(size)
    } else if rot {
        let tmat = Mat3::from_diagonal(size);
        tmat.inverse() * t.rot_to_mat3(true) * tmat
    } else {
        Mat3::IDENTITY
    };
    let mut mat = Mat4::from_mat3(rsmat);
    if loc {
        let mut l = Vec3::from_array(o.location) + Vec3::from_array(o.delta_location);
        if !(sca && rot) {
            let obmat = t.to_mat3();
            l = rsmat * (obmat.inverse() * l);
        }
        mat.w_axis = l.extend(1.0);
    }
    (rsmat, mat)
}

/// Resets the applied channels (`apply_objects_internal` after the data).
fn reset_channels(o: &mut Object, loc: bool, rot: bool, sca: bool) {
    if loc {
        o.location = [0.0; 3];
        o.delta_location = [0.0; 3];
    }
    if sca {
        o.scale = [1.0; 3];
        o.delta_scale = [1.0; 3];
    }
    if rot {
        o.rotation_euler = [0.0; 3];
        o.delta_rotation_euler = [0.0; 3];
        o.rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
        o.delta_rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
        o.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0];
    }
}

/// `compare_ff_relative(a, b, FLT_EPSILON, 64)`.
fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: i32) -> bool {
    if (a - b).abs() <= max_diff {
        return true;
    }
    let (ia, ib) = (a.to_bits() as i32, b.to_bits() as i32);
    (ia < 0) == (ib < 0) && (ia - ib).abs() <= max_ulps
}

/// `single_obdata_user_make`: `ob` gets its own copy of its data (`BKE_id_copy`, named like
/// `ID.copy()`).
fn single_user(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>, data: IdAny) {
    fn dup<T: frap_rna::IdType + Clone>(
        m: &mut frap_scene::IdMap<T>,
        a: IdAny,
        log: &mut ChangeLog,
    ) -> Option<IdAny> {
        let r = a.typed::<T::Kind>()?;
        let mut v = m.get(r)?.clone();
        let name = v.header().name.clone();
        let h = v.header_mut();
        h.use_fake_user = false;
        h.library = None;
        Some(m.insert(&name, v, log).any())
    }
    let new = match data.code {
        IdCode::ME => dup(&mut main.meshes, data, log),
        IdCode::CA => dup(&mut main.cameras, data, log),
        IdCode::LA => dup(&mut main.lights, data, log),
        _ => None,
    };
    if let (Some(new), Some(o)) = (new, main.objects.get_mut(ob, log)) {
        o.data = Some(new);
    }
}

/// The new evaluated world matrix Blender gives an object after the apply: its new channels
/// with the **old** delta channels (`BKE_object_transform_copy` copies no deltas).
fn eval_world(o: &Object, old: &Object, worlds: &Worlds) -> Mat4 {
    let mut ev = o.clone();
    ev.delta_location = old.delta_location;
    ev.delta_rotation_euler = old.delta_rotation_euler;
    ev.delta_rotation_quaternion = old.delta_rotation_quaternion;
    ev.delta_scale = old.delta_scale;
    let pw = o.parent.map(|p| worlds.get(p));
    xform::where_is(&ev, pw.as_ref())
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T1,
    support = Partial(
        "armatures arrive in M6 (NotImplementedError); properties=True has nothing to scale in \
         M1 (curves, fonts and bone envelopes); shape keys are not transformed until M6"
    )
)]
fn exec(cx: &mut OpCtx, p: object::TransformApply) -> OpResult {
    let (loc, rot, sca) = (p.location.value, p.rotation.value, p.scale.value);
    if !(loc || rot || sca) {
        return Ok(OpStatus::Finished);
    }
    let isolate = p.isolate_users.value;
    let flip = p.corrective_flip_normals.value;
    let (mut objects, do_multi, active) = {
        let c = cx.ctx();
        let objects = c.selected_editable_objects();
        let do_multi = can_multiuser(&c, &objects);
        (objects, do_multi, c.active_object())
    };
    if objects.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing to apply: no visible, editable object is selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    // First check if we can execute.
    let mut errors = Vec::new();
    {
        let main = cx.main();
        for &ob in &objects {
            let Some(o) = main.objects.get(ob) else {
                continue;
            };
            let Some(data) = o.data else {
                continue;
            };
            let (ob_name, data_name) = (
                o.header.name.as_str(),
                main.header(data).map_or("", |h| h.name.as_str()),
            );
            if !do_multi && !isolate && main.users(data) > 1 {
                errors.push(format!(
                    "Cannot apply to a multi user: Object \"{ob_name}\", {} \"{data_name}\", aborting",
                    id_type_name(data.code)
                ));
            }
            if main.header(data).is_some_and(|h| h.library.is_some()) {
                errors.push(format!(
                    "Cannot apply to library or override data: Object \"{ob_name}\", {} \"{data_name}\", aborting",
                    id_type_name(data.code)
                ));
            }
            if data.code == IdCode::AR {
                return Err(OpError::NotImplemented(
                    "frap: applying transforms to armatures arrives with armatures in M6".into(),
                ));
            }
            let area = data
                .typed()
                .and_then(|l| main.lights.get(l))
                .is_some_and(|l| l.r#type == "AREA");
            if area && (rot || loc) {
                errors.push(format!(
                    "Area Lights can only have scale applied: \"{ob_name}\""
                ));
            }
        }
    }
    if !errors.is_empty() {
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    let worlds_before = Worlds::of(cx.main());
    let (main, log) = cx.main_mut();
    if isolate && !do_multi {
        for &ob in &objects {
            if let Some(d) = main.objects.get(ob).and_then(|o| o.data)
                && main.users(d) > 1
            {
                single_user(main, log, ob, d);
            }
        }
    }
    if do_multi
        && let Some(a) = active
        && let Some(i) = objects.iter().position(|o| *o == a)
    {
        let a = objects.remove(i);
        objects.insert(0, a);
    }
    let mut worlds = worlds_before.clone();
    let mut changed = false;
    let mut active_mat: Option<Mat4> = None;
    for &ob in &objects {
        let Some(old) = main.objects.get(ob).cloned() else {
            continue;
        };
        let (rsmat, mat) = apply_matrix(&old, loc, rot, sca);
        if do_multi && Some(ob) != active {
            // Shares the active object's data: compensated below.
            changed = true;
            continue;
        }
        match old.data {
            None => {
                // Empty: the largest scale goes into the display size.
                if sca && let Some(o) = main.objects.get_mut(ob, log) {
                    let s = old.scale;
                    o.empty_display_size *= s[0].abs().max(s[1].abs()).max(s[2].abs());
                }
            }
            Some(d) if d.code == IdCode::ME => {
                let Some(m) = d.typed::<MeshId>() else {
                    continue;
                };
                if let Some(me) = main.meshes.get_mut(m, log) {
                    mesh::transform(me, &mat);
                    if flip && is_negative_m3(&Mat3::from_mat4(mat)) {
                        mesh::flip_faces(me);
                    }
                }
                if do_multi {
                    active_mat = Some(mat);
                }
            }
            Some(d) if d.code == IdCode::LA => {
                let Some(l) = d.typed().and_then(|l| main.lights.get_mut(l, log)) else {
                    continue;
                };
                if l.r#type != "AREA" {
                    continue;
                }
                let keeps_aspect =
                    compare_ff_relative(rsmat.x_axis.x, rsmat.y_axis.y, f32::EPSILON, 64);
                let f = &mut l.fields;
                if !keeps_aspect && f.shape == "SQUARE" {
                    f.shape = "RECTANGLE".into();
                    f.size_y = f.size;
                } else if !keeps_aspect && f.shape == "DISK" {
                    f.shape = "ELLIPSE".into();
                    f.size_y = f.size;
                }
                f.size *= rsmat.x_axis.x;
                f.size_y *= rsmat.y_axis.y;
            }
            // Cameras (no motion-tracking clip in frap) and other data are skipped.
            Some(_) => continue,
        }
        let Some(o) = main.objects.get_mut(ob, log) else {
            continue;
        };
        reset_channels(o, loc, rot, sca);
        let new = eval_world(o, &old, &worlds);
        worlds.set(ob, new);
        xform::ignore_parent_tx(main, log, ob, &new, &worlds);
        changed = true;
    }
    if let Some(amat) = active_mat {
        let inv = amat.inverse();
        for &ob in objects.iter().filter(|o| Some(**o) != active) {
            let w = worlds_before.get(ob) * inv;
            let pw = main
                .objects
                .get(ob)
                .and_then(|o| o.parent)
                .map(|p| worlds.get(p));
            let Some(o) = main.objects.get_mut(ob, log) else {
                continue;
            };
            xform::apply_mat4(o, &w, false, pw.as_ref());
            let new = xform::where_is(o, pw.as_ref());
            worlds.set(ob, new);
            xform::ignore_parent_tx(main, log, ob, &new, &worlds);
        }
    }
    if !changed {
        cx.report(ReportType::Warning, "Objects have no data to transform");
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Object {
        let mut o = Object::new("Cube", None);
        o.location = [1.0, 2.0, 3.0];
        o.rotation_euler = [0.3, 0.0, 0.5];
        o.scale = [2.0, 1.0, 0.5];
        o
    }

    #[test]
    fn rotation_only_keeps_scale_out_of_the_data() {
        // Vertex (1, 1, 1) → (0.684, 1.6676, 1.5464) (verified on 5.2.4).
        let (_, m) = apply_matrix(&cube(), false, true, false);
        let v = m.transform_point3(Vec3::ONE);
        assert!(
            v.abs_diff_eq(Vec3::new(0.683996, 1.667566, 1.546377), 1e-5),
            "{v:?}"
        );
    }

    #[test]
    fn location_only_is_expressed_in_object_space() {
        // Vertex (1, 1, 1) → (1.918217, 3.105321, 5.978005) (verified on 5.2.4).
        let (_, m) = apply_matrix(&cube(), true, false, false);
        let v = m.transform_point3(Vec3::ONE);
        assert!(
            v.abs_diff_eq(Vec3::new(1.918217, 3.105321, 5.978005), 1e-5),
            "{v:?}"
        );
    }

    #[test]
    fn full_apply_is_the_basis_matrix() {
        let o = cube();
        let (_, m) = apply_matrix(&o, true, true, true);
        assert!(m.abs_diff_eq(o.matrix_basis(), 1e-6));
        let (r, m) = apply_matrix(&o, false, false, false);
        assert_eq!((r, m), (Mat3::IDENTITY, Mat4::IDENTITY));
    }

    #[test]
    fn relative_compare() {
        assert!(compare_ff_relative(2.0, 2.0, f32::EPSILON, 64));
        assert!(!compare_ff_relative(2.0, 3.0, f32::EPSILON, 64));
        assert!(compare_ff_relative(
            1.0,
            1.0 + 2.0 * f32::EPSILON,
            f32::EPSILON,
            64
        ));
    }
}
