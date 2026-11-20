// SPDX-License-Identifier: GPL-3.0-or-later
//! ARMATURE (`ArmatureModifier`, WP 6.5): linear blend skinning by the evaluated pose of an
//! armature object. Every vertex is taken to the armature's space, moved by the weighted
//! average of its bones' deform matrices (`chan_mat = pose_mat · arm_mat⁻¹`) and taken back:
//!
//! - `use_vertex_groups`: the vertex groups named after a deforming bone (`use_deform`) give
//!   the weights (`use_envelope_multiply` scales a weight by the bone's envelope factor);
//! - `use_bone_envelopes`: a vertex no group deformed takes every deforming bone's envelope
//!   factor (`distfactor_to_bone` × `envelope_weight`);
//! - the weights are normalised by their sum; a vertex whose sum is below 1e-4 stays put;
//! - `vertex_group` (with `invert_vertex_group`) scales the whole effect per vertex;
//! - `use_deform_preserve_volume` blends dual quaternions instead of matrices;
//! - `use_multi_modifier` deforms the input of the previous ARMATURE modifier (the chain of
//!   ARMATURE modifiers right before it) and mixes the result with the current positions by
//!   `vertex_group` (`MOD_previous_vcos_store`; the coordinates travel in the evaluation's
//!   [`ModEvalCx::prev_coords_slot`], never on mesh data, so `modifier_apply` cannot leak them).
//!
//! An armature in Edit Mode (`arm->edbo`) does not deform (`verify_armature_deform_valid`).
//!
//! The armature-parent "virtual" modifier (`parent_type = 'ARMATURE'`, Blender's `PARSKEL`)
//! runs [`evaluate_virtual`] before the stack with `bArmature::deformflag`'s default
//! (vertex groups and envelopes), as `BKE_modifiers_get_virtual_modifierlist` puts it first.
//! A target that is not an armature disables the modifier (`is_disabled`). B-Bone segments
//! are deformed as one segment (frap evaluates no B-Bone curves; a WARNING says so).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_armature.cc,
//! blender/source/blender/blenkernel/intern/armature_deform.cc,
//! blender/source/blender/modifiers/intern/MOD_util.cc (`MOD_previous_vcos_store`) and
//! blender/source/blender/blenkernel/intern/modifier.cc
//! (`BKE_modifiers_get_virtual_modifierlist`), Blender 5.2.4, GPL-2.0-or-later.

mod dquat;

use std::sync::Arc;

use frap_mesh::DeformVert;
use frap_rna::{ArmatureId, IdAny, IdRef, ObjectId, Report, ReportType};
use frap_scene::Main;
use frap_scene::ObjectMode;
use frap_scene::edit_armature::rest::rest_pose;
use frap_scene::id::{Armature, Object};
use glam::{Mat4, Vec3};
use smol_str::SmolStr;

use self::dquat::DualQuat;
use crate::prelude::*;

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Armature(a) = s
        && let Some(o) = a.object
    {
        v.push(o);
    }
    v
}

fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Armature(a) if a.object.is_none() => Some("Armature: no object"),
        _ => None,
    }
}

#[modifier(
    ARMATURE,
    kind = OnlyDeform,
    flags = ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Armature(s) = &md.settings else {
        return Ok(mesh);
    };
    let Some(arm) = s
        .object
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|a| is_armature_object(cx.main(), *a))
    else {
        // `is_disabled`: no object, or one that is not an armature.
        return Ok(mesh);
    };
    let flags = DeformFlags {
        vgroup: s.use_vertex_groups,
        envelope: s.use_bone_envelopes,
        quaternion: s.use_deform_preserve_volume,
        invert_vgroup: s.invert_vertex_group,
    };
    let order: Vec<Modifier> = crate::stack::stack_order(&cx.object().modifiers)
        .cloned()
        .collect();
    let rest = order
        .iter()
        .position(|m| m.name == md.name)
        .map_or(&[][..], |i| &order[i + 1..]);
    // `amd->vert_coords_prev`: stored by an earlier ARMATURE modifier of the chain when this
    // one is `use_multi_modifier` (`MOD_previous_vcos_store`); freed after use either way.
    let stored = cx.prev_coords_slot().and_then(Option::take);
    let prev = stored.filter(|p| s.use_multi_modifier && p.len() == mesh.verts_num());
    // `MOD_previous_vcos_store(md, positions)`: this modifier's input goes to the multi
    // modifiers right after it, unless they already hold the chain's input.
    if chain_runs(rest, cx) {
        let p = prev.clone().unwrap_or_else(|| mesh.positions().to_vec());
        if let Some(slot) = cx.prev_coords_slot() {
            *slot = Some(p);
        }
    }
    deform_mesh(mesh, arm, flags, &s.vertex_group, prev.as_deref(), cx)
}

/// The modifier's `deformflag` bits (`ARM_DEF_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DeformFlags {
    /// `ARM_DEF_VGROUP` (`use_vertex_groups`)
    pub vgroup: bool,
    /// `ARM_DEF_ENVELOPE` (`use_bone_envelopes`)
    pub envelope: bool,
    /// `ARM_DEF_QUATERNION` (`use_deform_preserve_volume`)
    pub quaternion: bool,
    /// `ARM_DEF_INVERT_VGROUP` (`invert_vertex_group`)
    pub invert_vgroup: bool,
}

/// `bArmature::deformflag` of every armature (`ARM_DEF_VGROUP | ARM_DEF_ENVELOPE`, not
/// exposed in RNA): the flags of the armature-parent virtual modifier.
pub const ARMATURE_DEFORMFLAG: DeformFlags = DeformFlags {
    vgroup: true,
    envelope: true,
    quaternion: false,
    invert_vgroup: false,
};

/// Whether `ob` is an armature object of `main`.
pub fn is_armature_object(main: &Main, ob: IdRef<ObjectId>) -> bool {
    armature_of(main, ob).is_some()
}

fn armature_of(main: &Main, ob: IdRef<ObjectId>) -> Option<&Armature> {
    let d = main.objects.get(ob)?.data?;
    main.armatures.get(d.typed::<ArmatureId>()?)
}

/// The armature of the armature-parent virtual modifier of `ob`: its parent when that is an
/// armature object and `parent_type == 'ARMATURE'` (`PARSKEL`).
pub fn virtual_parent(main: &Main, ob: &Object) -> Option<IdRef<ObjectId>> {
    let p = ob.parent?;
    (ob.parent_type == "ARMATURE" && is_armature_object(main, p)).then_some(p)
}

/// Runs the armature-parent virtual modifier on `base` (the stack walk calls it first; the
/// virtual modifier is not shown in Edit Mode, `eModifierMode_Editmode` is off).
pub fn evaluate_virtual(
    base: Arc<Mesh>,
    arm: IdRef<ObjectId>,
    cx: &mut dyn ModEvalCx,
) -> Arc<Mesh> {
    let mesh = Arc::unwrap_or_clone(base);
    let order: Vec<Modifier> = crate::stack::stack_order(&cx.object().modifiers)
        .cloned()
        .collect();
    if chain_runs(&order, cx) {
        let p = mesh.positions().to_vec();
        if let Some(slot) = cx.prev_coords_slot() {
            *slot = Some(p);
        }
    }
    let fallback = mesh.clone();
    match deform_mesh(mesh, arm, ARMATURE_DEFORMFLAG, "", None, cx) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            cx.report(Report::new(
                ReportType::Warning,
                format!("Armature parent: {e}"),
            ));
            Arc::new(fallback)
        }
    }
}

/// Whether one of the `use_multi_modifier` ARMATURE modifiers at the start of `after` (the
/// modifiers `MOD_previous_vcos_store` hands the coordinates to) runs in this evaluation.
fn chain_runs(after: &[Modifier], cx: &dyn ModEvalCx) -> bool {
    let in_edit = cx.object().mode == ObjectMode::Edit;
    for m in after {
        let ModifierSettings::Armature(s) = &m.settings else {
            return false;
        };
        if !s.use_multi_modifier {
            return false;
        }
        let target = s.object.and_then(IdRef::<ObjectId>::from_any);
        if target.is_some_and(|a| is_armature_object(cx.main(), a))
            && crate::stack::enabled(m, cx.mode(), in_edit)
        {
            return true;
        }
    }
    false
}

/// `distfactor_to_bone`: the envelope influence of a bone from `head` to `tail` (armature or
/// world space alike) on `position`: 1 inside the radius interpolated along the bone, falling
/// off quadratically to 0 over `falloff_distance`.
pub fn distfactor_to_bone(
    position: Vec3,
    head: Vec3,
    tail: Vec3,
    radius_head: f32,
    radius_tail: f32,
    falloff_distance: f32,
) -> f32 {
    let d = tail - head;
    let bone_length = d.length();
    let bone_axis = if bone_length > 0.0 {
        d / bone_length
    } else {
        Vec3::ZERO
    };
    let height = (position - head).dot(bone_axis);
    if height < 0.0 {
        let distance_squared = position.distance_squared(head);
        bone_envelope_falloff(distance_squared, radius_head, falloff_distance)
    } else if height > bone_length {
        let distance_squared = tail.distance_squared(position);
        bone_envelope_falloff(distance_squared, radius_tail, falloff_distance)
    } else {
        let distance_squared = position.distance_squared(head) - height * height;
        let closest_radius = if bone_length != 0.0 {
            let t = height / bone_length;
            radius_head * (1.0 - t) + radius_tail * t
        } else {
            radius_head
        };
        bone_envelope_falloff(distance_squared, closest_radius, falloff_distance)
    }
}

fn bone_envelope_falloff(distance_squared: f32, closest_radius: f32, falloff_distance: f32) -> f32 {
    if distance_squared < closest_radius * closest_radius {
        return 1.0;
    }
    let outer = closest_radius + falloff_distance;
    if falloff_distance == 0.0 || distance_squared >= outer * outer {
        return 0.0;
    }
    let dist_envelope = distance_squared.sqrt() - closest_radius;
    1.0 - (dist_envelope * dist_envelope) / (falloff_distance * falloff_distance)
}

/// `mul_m4_m4m4(R, A, B)` in Blender's SSE2 summation order.
pub(crate) fn mul_m4_m4m4(a: &Mat4, b: &Mat4) -> Mat4 {
    let a = a.to_cols_array_2d();
    let b = b.to_cols_array_2d();
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] =
                (b[i][0] * a[0][j] + b[i][1] * a[1][j]) + (b[i][2] * a[2][j] + b[i][3] * a[3][j]);
        }
    }
    Mat4::from_cols_array_2d(&r)
}

/// `invert_m4_m4` (a singular matrix gives zero).
fn invert_m4(m: &Mat4) -> Mat4 {
    if m.determinant() == 0.0 {
        Mat4::ZERO
    } else {
        m.inverse()
    }
}

/// `invert_m4_m4_safe` (`Object::world_to_object()`, `BKE_object_eval_transform_final`): a
/// singular matrix is inverted with 1e-8 added to its diagonal, else gives the identity.
fn invert_m4_safe(m: &Mat4) -> Mat4 {
    if m.determinant() != 0.0 {
        return m.inverse();
    }
    let mut t = *m;
    t.x_axis.x += 1e-8;
    t.y_axis.y += 1e-8;
    t.z_axis.z += 1e-8;
    if t.determinant() != 0.0 {
        t.inverse()
    } else {
        Mat4::IDENTITY
    }
}

/// `math::transform_point`: the 3×3 part times `p`, plus the location.
fn transform_point(m: &Mat4, p: Vec3) -> Vec3 {
    m.x_axis.truncate() * p.x
        + m.y_axis.truncate() * p.y
        + m.z_axis.truncate() * p.z
        + m.w_axis.truncate()
}

/// One pose channel as the deform reads it.
struct Channel {
    name: SmolStr,
    /// `!(bone->flag & BONE_NO_DEFORM)`
    deform: bool,
    /// `chan_mat` after pose evaluation (`pose_mat · arm_mat⁻¹`).
    chan_mat: Mat4,
    /// `runtime.deform_dual_quat` (preserve volume only).
    dq: DualQuat,
    arm_head: Vec3,
    arm_tail: Vec3,
    rad_head: f32,
    rad_tail: f32,
    dist: f32,
    weight: f32,
    mult_vg_env: bool,
    segments: i32,
}

/// The accumulator of `BoneDeformLinearMixer` / `BoneDeformDualQuaternionMixer`.
enum Mixer {
    Linear(Vec3),
    Dual(DualQuat),
}

impl Mixer {
    fn new(quaternion: bool) -> Mixer {
        if quaternion {
            Mixer::Dual(DualQuat::default())
        } else {
            Mixer::Linear(Vec3::ZERO)
        }
    }

    fn accumulate(&mut self, ch: &Channel, co: Vec3, weight: f32) {
        match self {
            Mixer::Linear(delta) => {
                *delta += (transform_point(&ch.chan_mat, co) - co) * weight;
            }
            Mixer::Dual(dq) => dquat::add_weighted_dq_dq_pivot(dq, &ch.dq, co, weight),
        }
    }

    /// `finalize`: the position delta.
    fn finalize(self, co: Vec3, total: f32, armature_weight: f32) -> Vec3 {
        match self {
            Mixer::Linear(delta) => delta * (armature_weight / total),
            Mixer::Dual(mut dq) => {
                dquat::normalize_dq(&mut dq, total);
                let dco = dquat::mul_v3m3_dq(co, &dq);
                (dco - co) * armature_weight
            }
        }
    }
}

/// `BKE_defvert_find_weight`.
fn find_weight(dv: &DeformVert, g: u32) -> f32 {
    dv.weights
        .iter()
        .find(|(x, _)| *x == g)
        .map_or(0.0, |(_, w)| *w)
}

/// `BKE_armature_deform_coords_with_mesh` on the positions of `mesh`, deformed by `arm` with
/// `flags`; `defgrp_name` masks the effect; `prev` holds the coordinates a multi modifier
/// starts from.
pub fn deform_mesh(
    mesh: Mesh,
    arm: IdRef<ObjectId>,
    flags: DeformFlags,
    defgrp_name: &str,
    prev: Option<&[[f32; 3]]>,
    cx: &mut dyn ModEvalCx,
) -> Result<Mesh, ModError> {
    // `verify_armature_deform_valid`: not in armature Edit Mode (`arm->edbo`).
    if !is_armature_object(cx.main(), arm) || cx.armature_in_edit_mode(arm) {
        return Ok(mesh);
    }
    let pose = cx.pose(arm)?;
    let own = cx.object_ref();
    let target_world = cx.object_matrix_world(own);
    let arm_world = cx.object_matrix_world(arm);
    let channels = {
        let armature = armature_of(cx.main(), arm).expect("invariant: checked above");
        channels(armature, &pose.bones, flags.quaternion)
    };
    let bbones: Vec<SmolStr> = channels
        .iter()
        .filter(|c| c.deform && c.segments > 1)
        .map(|c| c.name.clone())
        .collect();
    if !bbones.is_empty() {
        cx.report(Report::new(
            ReportType::Warning,
            format!(
                "frap: ARMATURE deform treats the B-Bone segments of {} as one segment",
                bbones
                    .iter()
                    .map(|n| format!("'{n}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    // `ob_target.world_to_object() * ob_arm.object_to_world()` and its inverse.
    let armature_to_target = invert_m4_safe(&target_world) * arm_world;
    let target_to_armature = invert_m4(&armature_to_target);
    let names = &mesh.vertex_group_names;
    let dverts: Option<&[DeformVert]> = mesh.deform_verts().filter(|d| !d.is_empty());
    let use_dverts = dverts.is_some() && flags.vgroup;
    let by_group: Vec<Option<usize>> = if use_dverts {
        names
            .iter()
            .map(|n| {
                channels
                    .iter()
                    .position(|c| c.name == *n)
                    .filter(|&i| channels[i].deform)
            })
            .collect()
    } else {
        Vec::new()
    };
    let def_nr: Option<u32> = if defgrp_name.is_empty() {
        None
    } else {
        names
            .iter()
            .position(|n| n == defgrp_name)
            .map(|i| i as u32)
    };
    let mut cos = mesh.positions().to_vec();
    let p = Params {
        channels: &channels,
        by_group: &by_group,
        use_dverts,
        envelope: flags.envelope,
        quaternion: flags.quaternion,
        invert_vgroup: flags.invert_vgroup,
        def_nr,
        target_to_armature,
        armature_to_target,
    };
    for (i, co) in cos.iter_mut().enumerate() {
        let dvert = if use_dverts || def_nr.is_some() {
            dverts.and_then(|d| d.get(i))
        } else {
            None
        };
        let start = prev.and_then(|pv| pv.get(i).copied());
        *co = vert_task(&p, *co, start, prev.is_some(), dvert);
    }
    let mut mesh = mesh;
    mesh.set_positions(cos);
    Ok(mesh)
}

/// The pose channels of `armature` in bone order with their deform matrices.
fn channels(armature: &Armature, pose: &[(SmolStr, Mat4)], quaternion: bool) -> Vec<Channel> {
    let rest = rest_pose(armature);
    armature
        .bones
        .iter()
        .zip(rest)
        .filter_map(|(b, r)| {
            let pose_mat = pose.iter().find(|(n, _)| *n == b.name)?.1;
            let arm_mat = r.arm_mat;
            let chan_mat = mul_m4_m4m4(&pose_mat, &invert_m4(&arm_mat));
            let dq = if quaternion && b.use_deform {
                dquat::mat4_to_dquat(&arm_mat, &chan_mat)
            } else {
                DualQuat::default()
            };
            Some(Channel {
                name: b.name.clone(),
                deform: b.use_deform,
                chan_mat,
                dq,
                arm_head: r.arm_head,
                arm_tail: r.arm_tail,
                rad_head: b.head_radius,
                rad_tail: b.tail_radius,
                dist: b.envelope_distance,
                weight: b.envelope_weight,
                mult_vg_env: b.use_envelope_multiply,
                segments: b.bbone_segments,
            })
        })
        .collect()
}

struct Params<'a> {
    channels: &'a [Channel],
    by_group: &'a [Option<usize>],
    use_dverts: bool,
    envelope: bool,
    quaternion: bool,
    invert_vgroup: bool,
    def_nr: Option<u32>,
    target_to_armature: Mat4,
    armature_to_target: Mat4,
}

/// `armature_vert_task_with_mixer` for one vertex: its new position.
fn vert_task(
    p: &Params<'_>,
    current: Vec3Arr,
    start: Option<Vec3Arr>,
    has_prev: bool,
    dvert: Option<&DeformVert>,
) -> Vec3Arr {
    let mut armature_weight = 1.0f32;
    let mut prevco_weight = 0.0f32;
    if let (Some(def_nr), Some(dv)) = (p.def_nr, dvert) {
        let mask_weight = find_weight(dv, def_nr);
        if has_prev {
            prevco_weight = if p.invert_vgroup {
                mask_weight
            } else {
                1.0 - mask_weight
            };
            if prevco_weight == 1.0 {
                return current;
            }
        } else {
            armature_weight = if p.invert_vgroup {
                1.0 - mask_weight
            } else {
                mask_weight
            };
            if armature_weight == 0.0 {
                return current;
            }
        }
    }
    let input = start.unwrap_or(current);
    let mut co = transform_point(&p.target_to_armature, Vec3::from_array(input));
    let mut mixer = Mixer::new(p.quaternion);
    let mut contrib = 0.0f32;
    let mut deformed = false;
    if p.use_dverts
        && let Some(dv) = dvert
    {
        for &(g, w) in &dv.weights {
            let Some(ci) = p.by_group.get(g as usize).copied().flatten() else {
                continue;
            };
            let ch = &p.channels[ci];
            let mut weight = w;
            if ch.mult_vg_env {
                weight *= distfactor_to_bone(
                    co,
                    ch.arm_head,
                    ch.arm_tail,
                    ch.rad_head,
                    ch.rad_tail,
                    ch.dist,
                );
            }
            // `pchan_bone_deform`
            if weight != 0.0 {
                mixer.accumulate(ch, co, weight);
                contrib += weight;
            }
            deformed = true;
        }
    }
    if !deformed && p.envelope {
        for ch in p.channels.iter().filter(|c| c.deform) {
            // `dist_bone_deform`
            if ch.weight == 0.0 {
                continue;
            }
            let fac = distfactor_to_bone(
                co,
                ch.arm_head,
                ch.arm_tail,
                ch.rad_head,
                ch.rad_tail,
                ch.dist,
            );
            if fac == 0.0 {
                continue;
            }
            let weight = fac * ch.weight;
            mixer.accumulate(ch, co, weight);
            contrib += weight;
        }
    }
    const CONTRIB_THRESHOLD: f32 = 0.0001;
    if contrib > CONTRIB_THRESHOLD {
        co += mixer.finalize(co, contrib, armature_weight);
    }
    let co = transform_point(&p.armature_to_target, co);
    if has_prev {
        // math::interpolate(co, vert_coords[i], prevco_weight)
        let cur = Vec3::from_array(current);
        (co * (1.0 - prevco_weight) + cur * prevco_weight).to_array()
    } else {
        co.to_array()
    }
}

type Vec3Arr = [f32; 3];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_factor_matches_blender_cases() {
        let (h, t) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
        // Inside the interpolated radius.
        assert_eq!(
            distfactor_to_bone(Vec3::new(0.05, 0.0, 0.5), h, t, 0.1, 0.05, 0.25),
            1.0
        );
        // Beyond radius + falloff.
        assert_eq!(
            distfactor_to_bone(Vec3::new(1.0, 0.0, 0.5), h, t, 0.1, 0.05, 0.25),
            0.0
        );
        // In the falloff: 1 - (d - r)² / f².
        let f = distfactor_to_bone(Vec3::new(0.2, 0.0, 0.0), h, t, 0.1, 0.05, 0.25);
        assert!((f - (1.0 - 0.01 / 0.0625)).abs() < 1e-6);
        // Below the head uses the head radius, past the tail the tail radius.
        let f = distfactor_to_bone(Vec3::new(0.0, 0.0, -0.2), h, t, 0.1, 0.05, 0.25);
        assert!((f - (1.0 - 0.01 / 0.0625)).abs() < 1e-6);
        let f = distfactor_to_bone(Vec3::new(0.0, 0.0, 1.2), h, t, 0.1, 0.05, 0.25);
        assert!((f - (1.0 - 0.0225 / 0.0625)).abs() < 1e-6);
        // No falloff distance: a hard edge.
        assert_eq!(
            distfactor_to_bone(Vec3::new(0.2, 0.0, 0.5), h, t, 0.1, 0.1, 0.0),
            0.0
        );
    }

    #[test]
    fn mul_matches_glam_on_exact_values() {
        let a = Mat4::from_cols_array(&[
            1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 4.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ]);
        let b = Mat4::from_cols_array(&[
            2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, -1.0, 4.0, 2.0, 1.0,
        ]);
        assert_eq!(mul_m4_m4m4(&a, &b), a * b);
    }

    fn channel(chan_mat: Mat4) -> Channel {
        Channel {
            name: "B".into(),
            deform: true,
            chan_mat,
            dq: dquat::mat4_to_dquat(&Mat4::IDENTITY, &chan_mat),
            arm_head: Vec3::ZERO,
            arm_tail: Vec3::new(0.0, 0.0, 1.0),
            rad_head: 0.1,
            rad_tail: 0.05,
            dist: 0.25,
            weight: 1.0,
            mult_vg_env: false,
            segments: 1,
        }
    }

    fn params<'a>(channels: &'a [Channel], by_group: &'a [Option<usize>]) -> Params<'a> {
        Params {
            channels,
            by_group,
            use_dverts: true,
            envelope: false,
            quaternion: false,
            invert_vgroup: false,
            def_nr: None,
            target_to_armature: Mat4::IDENTITY,
            armature_to_target: Mat4::IDENTITY,
        }
    }

    #[test]
    fn weights_are_normalised_by_their_sum() {
        let chs = [
            channel(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))),
            channel(Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0))),
        ];
        let by_group = [Some(0), Some(1)];
        let p = params(&chs, &by_group);
        let dv = DeformVert {
            weights: vec![(0, 0.25), (1, 0.25)].into(),
        };
        let out = vert_task(&p, [0.0; 3], None, false, Some(&dv));
        assert_eq!(out, [0.5, 1.0, 0.0]);
        // A weight sum below 1e-4 leaves the vertex alone.
        let dv = DeformVert {
            weights: vec![(0, 0.00005)].into(),
        };
        assert_eq!(vert_task(&p, [0.0; 3], None, false, Some(&dv)), [0.0; 3]);
    }

    #[test]
    fn mask_group_scales_and_multi_mixes() {
        let chs = [channel(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)))];
        let by_group = [Some(0), None];
        let mut p = params(&chs, &by_group);
        p.def_nr = Some(1);
        let dv = DeformVert {
            weights: vec![(0, 1.0), (1, 0.5)].into(),
        };
        assert_eq!(
            vert_task(&p, [0.0; 3], None, false, Some(&dv)),
            [0.5, 0.0, 0.0]
        );
        p.invert_vgroup = true;
        let dv0 = DeformVert {
            weights: vec![(0, 1.0), (1, 1.0)].into(),
        };
        // Inverted full mask: untouched.
        assert_eq!(vert_task(&p, [3.0; 3], None, false, Some(&dv0)), [3.0; 3]);
        p.invert_vgroup = false;
        // Multi: deform the previous input, mix with the current position by the mask.
        let out = vert_task(&p, [0.0, 4.0, 0.0], Some([0.0; 3]), true, Some(&dv));
        assert_eq!(out, [0.5, 2.0, 0.0]);
    }

    #[test]
    fn envelopes_only_when_no_group_deformed() {
        let chs = [channel(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)))];
        let by_group = [None];
        let mut p = params(&chs, &by_group);
        p.envelope = true;
        // In a group that maps to no bone: not deformed by groups, so envelopes apply.
        let dv = DeformVert {
            weights: vec![(0, 1.0)].into(),
        };
        assert_eq!(
            vert_task(&p, [0.0, 0.0, 0.5], None, false, Some(&dv)),
            [1.0, 0.0, 0.5]
        );
    }
}
