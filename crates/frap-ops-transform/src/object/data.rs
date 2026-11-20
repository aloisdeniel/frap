// SPDX-License-Identifier: GPL-3.0-or-later
//! Object transform data: which objects a transform moves and the per-object matrices it
//! needs. Ports of `createTransObject`, `ObjectToTransData` and
//! `set_trans_object_base_flags` (`editors/transform/transform_convert_object.cc`, Blender
//! 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: the selected **visible** objects move (a hidden selected object stays);
//! an object whose parent (or any ancestor) is also selected is not transformed itself, it
//! follows its parent, except for a rotation about individual origins, where it rotates in
//! place (`BA_TRANSFORM_CHILD`: its location is left alone and it does not count for the
//! center); with proportional editing every other visible, selectable, editable object takes
//! part with a factor, children of the selection included (they then also follow their
//! parent), but **not the ancestors** of a selected object (`count_proportional_objects`
//! flags them `BA_TRANSFORM_PARENT`): with Light → Cube (selected) → Camera, a proportional
//! translate leaves Light (and a grandparent) in place and moves Camera twice, by its parent
//! and by its own factor. Blender's `mark_children` (`BA_TRANSFORM_CHILD`) reads the legacy
//! object `SELECT` flag, which 5.2 no longer sets, so it never excludes a child. A rotation
//! about individual origins skips that marking and takes the ancestors too (verified).

use frap_math::matrix::RotationMode;
use frap_math::quat::QuatWxyz;
use frap_rna::{IdRef, ObjectId};
use frap_scene::Context;
use frap_scene::id::Object;
use frap_scene::id::transform::world_matrix_in;
use glam::{Mat3, Vec3};

use crate::core::math::{axes_of, invert_m3_safe_ortho, orthogonalize_m3_zero_axes};
use crate::core::orient::gimbal_axis_object;

/// `TransData` + `TransDataExtension` of one object.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectTd {
    /// The object.
    pub ob: IdRef<ObjectId>,
    /// `TD_SELECTED` (proportional elements are not).
    pub selected: bool,
    /// `TD_SKIP`: linked (not editable) objects count for the center but do not move.
    pub skip: bool,
    /// `TD_NOCENTER` (a child rotating about its own origin).
    pub no_center: bool,
    /// `TD_NO_LOC` (same).
    pub no_loc: bool,
    /// Proportional factor (1 for the selection).
    pub factor: f32,
    /// World location (`td->center`).
    pub center: Vec3,
    /// Normalized world axes (`td->axismtx`).
    pub axismtx: Mat3,
    /// World gimbal axes (`td->ext->axismtx_gimbal`; `axismtx` for a quaternion, verified).
    pub gimbal: Mat3,
    /// Local data space to global (`td->mtx`).
    pub mtx: Mat3,
    /// Global to local data space (`td->smtx`).
    pub smtx: Mat3,
    /// Initial channels.
    pub init: Channels,
}

/// The transform channels a transform writes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Channels {
    /// `location`
    pub location: Vec3,
    /// `rotation_mode`
    pub mode: RotationMode,
    /// `rotation_euler`
    pub euler: [f32; 3],
    /// `rotation_quaternion` (w-first)
    pub quat: QuatWxyz,
    /// `rotation_axis_angle` axis
    pub axis: [f32; 3],
    /// `rotation_axis_angle` angle
    pub angle: f32,
    /// `scale`
    pub scale: Vec3,
    /// `delta_rotation_euler` (read only)
    pub drot: [f32; 3],
    /// `delta_rotation_quaternion` (read only)
    pub dquat: QuatWxyz,
}

impl Channels {
    /// The channels of an object.
    pub fn of(o: &Object) -> Channels {
        Channels {
            location: Vec3::from_array(o.location),
            mode: o.rotation_mode_enum(),
            euler: o.rotation_euler,
            quat: o.rotation_quaternion,
            axis: [
                o.rotation_axis_angle[1],
                o.rotation_axis_angle[2],
                o.rotation_axis_angle[3],
            ],
            angle: o.rotation_axis_angle[0],
            scale: Vec3::from_array(o.scale),
            drot: o.delta_rotation_euler,
            dquat: o.delta_rotation_quaternion,
        }
    }
    /// Writes the channels into an object (location, rotation values, scale).
    pub fn write(&self, o: &mut Object) {
        o.location = self.location.to_array();
        o.rotation_euler = self.euler;
        o.rotation_quaternion = self.quat;
        o.rotation_axis_angle = [self.angle, self.axis[0], self.axis[1], self.axis[2]];
        o.scale = self.scale.to_array();
    }
}

/// `ObjectToTransData`.
pub fn object_td(c: &Context, ob: IdRef<ObjectId>, o: &Object) -> ObjectTd {
    let objects = &c.main.objects;
    let world = world_matrix_in(c.main, ob);
    let world3 = Mat3::from_mat4(world);
    let axismtx = axes_of(&world3);
    let parent_world = o
        .parent
        .filter(|p| objects.contains(*p))
        .map(|p| Mat3::from_mat4(world_matrix_in(c.main, p)));
    let gimbal = gimbal_axis_object(
        o.rotation_mode_enum(),
        o.rotation_euler,
        o.rotation_axis_angle,
        parent_world.as_ref(),
    )
    .unwrap_or(axismtx);
    let (mtx, smtx) = if o.parent.is_some() {
        let mut obmtx = o.transform().to_mat3();
        let mut totmat = world3;
        orthogonalize_m3_zero_axes(&mut obmtx, 1.0);
        orthogonalize_m3_zero_axes(&mut totmat, 1.0);
        let obinv = invert_m3_safe_ortho(&totmat);
        let smtx = obmtx * obinv;
        (invert_m3_safe_ortho(&smtx), smtx)
    } else {
        (Mat3::IDENTITY, Mat3::IDENTITY)
    };
    ObjectTd {
        ob,
        selected: true,
        skip: o.header.library.is_some(),
        no_center: false,
        no_loc: false,
        factor: 1.0,
        center: world.w_axis.truncate(),
        axismtx,
        gimbal,
        mtx,
        smtx,
        init: Channels::of(o),
    }
}

/// True when an ancestor of `o` is in `set`.
fn has_ancestor_in(c: &Context, o: &Object, set: &[IdRef<ObjectId>]) -> bool {
    let mut cur = o.parent;
    let mut n = 0;
    while let Some(p) = cur {
        if set.contains(&p) {
            return true;
        }
        n += 1;
        if n > frap_scene::id::transform::MAX_PARENT_DEPTH {
            return false;
        }
        cur = c.main.objects.get(p).and_then(|o| o.parent);
    }
    false
}

/// `createTransObject`: the selected visible objects in base order (minus those following a
/// selected ancestor, or flagged to rotate in place when `children_in_place`), then, with
/// proportional editing, every other visible, selectable and editable object that is not an
/// ancestor of the selection (ancestors included when `children_in_place`).
pub fn create(c: &Context, children_in_place: bool, proportional: bool) -> Vec<ObjectTd> {
    let selected = c.selected_objects();
    // `set_trans_object_base_flags` looks at selected editable bases only.
    let editable = c.selected_editable_objects();
    let mut out = Vec::new();
    for &ob in &selected {
        let Some(o) = c.main.objects.get(ob) else {
            continue;
        };
        let mut td = object_td(c, ob, o);
        if editable.contains(&ob) && has_ancestor_in(c, o, &editable) {
            if !children_in_place {
                continue;
            }
            td.no_center = true;
            td.no_loc = true;
        }
        out.push(td);
    }
    if proportional {
        let vis = c.base_visibility();
        // `BA_TRANSFORM_PARENT`: every ancestor of a selected, editable, visible and
        // selectable object, except for a rotation about individual origins, which takes
        // every object.
        let mut parents = Vec::new();
        for &ob in editable.iter().filter(|_| !children_in_place) {
            let v = vis.get(&ob).copied().unwrap_or_default();
            if !v.visible || !v.selectable {
                continue;
            }
            let mut cur = c.main.objects.get(ob).and_then(|o| o.parent);
            let mut n = 0;
            while let Some(p) = cur {
                if parents.contains(&p) || n > frap_scene::id::transform::MAX_PARENT_DEPTH {
                    break;
                }
                parents.push(p);
                n += 1;
                cur = c.main.objects.get(p).and_then(|o| o.parent);
            }
        }
        for b in &c.view_layer().bases {
            if b.selected() || parents.contains(&b.object) {
                continue;
            }
            let v = vis.get(&b.object).copied().unwrap_or_default();
            let Some(o) = c.main.objects.get(b.object) else {
                continue;
            };
            if !v.visible || !v.selectable || o.header.library.is_some() {
                continue;
            }
            let mut td = object_td(c, b.object, o);
            td.selected = false;
            out.push(td);
        }
    }
    out
}
