// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose-mode `TRANSFORM_MODE_HANDLERS` entry (WP 6.2, `crates/frap-ops-transform/src/pose/**`):
//! `transform.translate/rotate/resize` on the selected pose bones through the shared core
//! (orientation, constraint, pivot), writing `PoseBone.location` / rotation / `scale` in bone
//! space as `transform_convert_armature.cc` does. The armature Edit Mode entry (bone heads
//! and tails) is [`edit_armature`]. The dispatcher (`core/`) stays WP 1.21's.
//!
//! Ports of `transform_convert_pose_transflags_update`, `add_pose_transdata`,
//! `createTransPose` (blender/source/blender/editors/transform/transform_convert_armature.cc),
//! the `CTX_POSE_BONE` branches of `ElementRotation_ex` and `ElementResize`, the
//! `protected*Bits` lock helpers (blender/source/blender/editors/transform/transform_mode.cc),
//! `transdata_elem_translate` (transform_mode_translate.cc) and the pose branch of
//! `getTransformOrientation_ex` (transform_orientations.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! - The transformed bones are the selected, visible pose bones of every armature in Pose
//!   Mode, minus the descendants of another transformed bone (a connected hinge child keeps
//!   its flag, and with `INDIVIDUAL_ORIGINS` rotation a selected child rotates in place).
//! - Values are world vectors, turned into bone space through the parent transform
//!   (`loc_mat` for translate, `rotscale_mat` for rotate/resize) and the object matrix.
//! - `LOCAL` / `NORMAL` orientation is the active bone's axes (or the sum of the transformed
//!   bones' axes), `GIMBAL` the active bone's gimbal axes.
//! - Transform locks (`lock_location`, `lock_rotation[_w]`, `lock_rotations_4d`, `lock_scale`)
//!   are honoured.
//!
//! Not implemented (`NotImplementedError` before anything changes): proportional editing,
//! `Pose.use_mirror_x`, `Pose.use_auto_ik`, translating only connected bones (Blender then
//! rotates instead), `pose_position = 'REST'` (Blender's error report `Cannot change Pose when 'Rest Position'
//! is enabled`, a `RuntimeError`).

pub mod edit_armature;

use frap_math::EulerOrder;
use frap_math::euler::{eul_o_to_mat3, mat3_to_compatible_eul_o, quat_to_eul_o};
use frap_math::matrix::RotationMode;
use frap_math::quat::{
    axis_angle_to_quat, mat3_to_quat, mul_qt_fl, mul_qt_qtqt, normalize_qt, quat_to_axis_angle,
};
use frap_op::prelude::*;
use frap_op::{TRANSFORM_MODE_HANDLERS, TransformModeHandler, TransformOp};
use frap_rna::{ArmatureId, IdRef, ObjectId, OpArgs};
use frap_scene::edit_armature::math::{BoneParentTransform, Channels};
use frap_scene::edit_armature::posing::{parent_transform, where_is};
use frap_scene::edit_armature::rest::rest_pose;
use frap_scene::id::transform::world_matrix;
use frap_scene::{Context, ContextMode, ObjectMode};
use glam::{Mat3, Mat4, Vec3};

use crate::core::constraint::{Constraint, ConstraintSpace};
use crate::core::math::{
    axis_angle_normalized_to_mat3, invert_m3_safe_ortho, normalize_m3, size_to_mat3,
    trans_mat3_to_size,
};
use crate::core::orient::{OrientSource, OrientType, Orientation, euler_gimbal_axes};
use crate::core::params::TransformParams;
use crate::core::pivot::{self, PivotPoint};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(TRANSFORM_MODE_HANDLERS)]
static POSE: TransformModeHandler = TransformModeHandler {
    mode: ContextMode::Pose,
    apply,
};

/// Transform locks (`protectflag`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Locks {
    /// `lock_location`
    pub loc: [bool; 3],
    /// `lock_rotation`
    pub rot: [bool; 3],
    /// `lock_rotation_w`
    pub rot_w: bool,
    /// `lock_rotations_4d`
    pub rot_4d: bool,
    /// `lock_scale`
    pub scale: [bool; 3],
}

impl Locks {
    fn any_rot(&self) -> bool {
        self.rot.iter().any(|b| *b) || self.rot_w
    }
}

/// `protectedTransBits`.
fn protect_loc(l: &Locks, mut v: Vec3) -> Vec3 {
    for i in 0..3 {
        if l.loc[i] {
            v[i] = 0.0;
        }
    }
    v
}

/// `protectedScaleBits`.
fn protect_scale(l: &Locks, mut v: Vec3) -> Vec3 {
    for i in 0..3 {
        if l.scale[i] {
            v[i] = 1.0;
        }
    }
    v
}

/// `protectedQuaternionBits`.
fn protect_quat(l: &Locks, quat: [f32; 4], old: [f32; 4]) -> [f32; 4] {
    if !l.any_rot() {
        return quat;
    }
    let mut q = quat;
    if l.rot_4d {
        if l.rot_w {
            q[0] = old[0];
        }
        for i in 0..3 {
            if l.rot[i] {
                q[i + 1] = old[i + 1];
            }
        }
        return q;
    }
    let (nquat, qlen) = normalize_qt(q);
    let (nold, _) = normalize_qt(old);
    let mut eul = quat_to_eul_o(nquat, EulerOrder::XYZ);
    let oldeul = quat_to_eul_o(nold, EulerOrder::XYZ);
    for i in 0..3 {
        if l.rot[i] {
            eul[i] = oldeul[i];
        }
    }
    q = frap_math::euler::eul_o_to_quat(eul, EulerOrder::XYZ);
    q = mul_qt_fl(q, qlen);
    if (nquat[0] < 0.0 && q[0] > 0.0) || (nquat[0] > 0.0 && q[0] < 0.0) {
        q = mul_qt_fl(q, -1.0);
    }
    q
}

/// `protectedRotateBits`.
fn protect_euler(l: &Locks, mut eul: [f32; 3], old: [f32; 3]) -> [f32; 3] {
    for i in 0..3 {
        if l.rot[i] {
            eul[i] = old[i];
        }
    }
    eul
}

/// `protectedAxisAngleBits`.
fn protect_axis_angle(
    l: &Locks,
    axis: [f32; 3],
    angle: f32,
    old_axis: [f32; 3],
    old_angle: f32,
) -> ([f32; 3], f32) {
    if !l.any_rot() {
        return (axis, angle);
    }
    let (mut axis, mut angle) = (axis, angle);
    if l.rot_4d {
        if l.rot_w {
            angle = old_angle;
        }
        for i in 0..3 {
            if l.rot[i] {
                axis[i] = old_axis[i];
            }
        }
        return (axis, angle);
    }
    let mut eul = frap_math::quat::axis_angle_to_eul_o(axis, angle, EulerOrder::XYZ);
    let oldeul = frap_math::quat::axis_angle_to_eul_o(old_axis, old_angle, EulerOrder::XYZ);
    for i in 0..3 {
        if l.rot[i] {
            eul[i] = oldeul[i];
        }
    }
    let (a, an) = frap_math::quat::eul_o_to_axis_angle(eul, EulerOrder::XYZ);
    axis = a;
    angle = an;
    if (axis[0] - axis[1]).abs() < f32::EPSILON && (axis[1] - axis[2]).abs() < f32::EPSILON {
        axis[1] = 1.0;
    }
    (axis, angle)
}

/// One transformed pose bone (`TransData` + `TransDataExtension`).
#[derive(Clone, Debug)]
pub struct PoseTd {
    /// The armature object.
    pub ob: IdRef<ObjectId>,
    /// The channel's name.
    pub name: smol_str::SmolStr,
    /// `td->center` (armature space).
    pub center: Vec3,
    /// `TD_NOCENTER`
    pub no_center: bool,
    /// The bone is connected to a parent (its location channel has no effect).
    pub connected: bool,
    /// `TD_NO_LOC`
    pub no_loc: bool,
    /// `TD_PBONE_LOCAL_MTX_P` / `_C` (rotation of a bone without local location).
    pub local_mtx: LocalMtx,
    /// `td->mtx`, `td->smtx`.
    pub mtx: Mat3,
    pub smtx: Mat3,
    /// `td_ext->r_mtx`, `r_smtx`.
    pub r_mtx: Mat3,
    pub r_smtx: Mat3,
    /// `td_ext->l_smtx`.
    pub l_smtx: Mat3,
    /// `td->axismtx`.
    pub axismtx: Mat3,
    /// `td_ext->axismtx_gimbal`.
    pub gimbal: Mat3,
    /// The object's 3×3 world matrix and its inverse (`tc->mat3`, `tc->imat3`).
    pub omat: Mat3,
    pub iomat: Mat3,
    /// Initial channels.
    pub init: Channels,
    /// Locks.
    pub locks: Locks,
}

/// The `TD_PBONE_LOCAL_MTX_*` exception.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LocalMtx {
    /// Neither flag.
    None,
    /// `TD_PBONE_LOCAL_MTX_P`: a parentless bone without local location.
    Parentless,
    /// `TD_PBONE_LOCAL_MTX_C`: a child bone without local location.
    Child,
}

fn locks_of(pb: &frap_scene::id::PoseBone) -> Locks {
    Locks {
        loc: pb.lock_location,
        rot: pb.lock_rotation,
        rot_w: pb.lock_rotation_w,
        rot_4d: pb.lock_rotations_4d,
        scale: pb.lock_scale,
    }
}

/// `gimbal_axis_pose`: the gimbal axes of a pose bone in world space (Euler and axis-angle
/// modes; `None` for quaternions).
fn gimbal_pose(ch: &Channels, bpt: &BoneParentTransform, omat: &Mat3) -> Option<Mat3> {
    let mat = match ch.mode {
        RotationMode::Euler(order) => euler_gimbal_axes(ch.euler, order),
        RotationMode::AxisAngle => crate::core::orient::axis_angle_gimbal_axes(ch.axis, ch.angle),
        RotationMode::Quaternion => return None,
    };
    let rot = Mat3::from_mat4(bpt.rotscale);
    let tmat = *omat * rot;
    Some(normalize_m3(&(tmat * mat)))
}

/// Which bones of `ob` are transformed (`transform_convert_pose_transflags_update`):
/// `(bone index, no_center, no_loc)`.
fn transformed_bones(
    arm: &frap_scene::id::Armature,
    pose: &frap_scene::id::Pose,
    op: TransformOp,
    local_origins: bool,
) -> Vec<(usize, bool, bool)> {
    let n = arm.bones.len();
    let mut transform = vec![false; n];
    let mut hinge_child = vec![false; n];
    let mut transform_child = vec![false; n];
    for (i, b) in arm.bones.iter().enumerate() {
        let Some(pb) = pose.bone(&b.name) else {
            continue;
        };
        transform[i] = pb.select && !pb.hide && arm.bone_collections_visible(b);
    }
    let descendants = |i: usize| -> Vec<usize> {
        (0..n)
            .filter(|&j| {
                let mut p = arm.bones[j].parent;
                let mut guard = 0;
                while let Some(x) = p {
                    if x == i {
                        return true;
                    }
                    guard += 1;
                    if guard > n {
                        break;
                    }
                    p = arm.bones[x].parent;
                }
                false
            })
            .collect()
    };
    for i in 0..n {
        if !transform[i] {
            continue;
        }
        for c in descendants(i) {
            let b = &arm.bones[c];
            if !b.use_inherit_rotation && b.use_connect {
                hinge_child[c] = true;
            } else if transform[c] && op == TransformOp::Rotate && local_origins {
                transform_child[c] = true;
            } else {
                transform[c] = false;
            }
        }
    }
    (0..n)
        .filter(|&i| transform[i])
        .map(|i| (i, hinge_child[i] || transform_child[i], transform_child[i]))
        .collect()
}

/// The pose objects of the context: every armature in Pose Mode (active first).
pub fn pose_objects(c: &Context) -> Vec<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
    c.objects_in_mode()
        .into_iter()
        .filter_map(|ob| {
            let o = c.main.objects.get(ob)?;
            if o.mode != ObjectMode::Pose {
                return None;
            }
            Some((ob, o.data?.typed::<ArmatureId>()?))
        })
        .collect()
}

/// `createTransPose` for one object.
fn create(
    c: &Context,
    ob: IdRef<ObjectId>,
    arm_ref: IdRef<ArmatureId>,
    op: TransformOp,
    local_origins: bool,
) -> Vec<PoseTd> {
    let (Some(o), Some(arm)) = (c.main.objects.get(ob), c.main.armatures.get(arm_ref)) else {
        return Vec::new();
    };
    let Some(pose) = o.pose.as_ref() else {
        return Vec::new();
    };
    let rest = rest_pose(arm);
    let mats = where_is(arm, &rest, Some(pose), false);
    let obmat = world_matrix(&c.main.objects, ob);
    let omat = Mat3::from_mat4(obmat);
    let iomat = invert_m3_safe_ortho(&omat);
    transformed_bones(arm, pose, op, local_origins)
        .into_iter()
        .filter_map(|(i, no_center, no_loc)| {
            let b = &arm.bones[i];
            let pb = pose.bone(&b.name)?;
            let parent_pose = b.parent.map(|p| mats[p].pose_mat);
            let bpt = parent_transform(arm, &rest, i, parent_pose.as_ref());
            let pmat = if op == TransformOp::Translate {
                Mat3::from_mat4(bpt.loc)
            } else {
                Mat3::from_mat4(bpt.rotscale)
            };
            let rpmat = Mat3::from_mat4(bpt.rotscale);
            let mtx = omat * pmat;
            let r_mtx = omat * rpmat;
            let smtx = invert_m3_safe_ortho(&mtx);
            let r_smtx = invert_m3_safe_ortho(&r_mtx);
            let mut local_mtx = LocalMtx::None;
            let mut l_smtx = Mat3::IDENTITY;
            if op == TransformOp::Rotate && !b.use_local_location {
                if b.parent.is_some() {
                    local_mtx = LocalMtx::Child;
                    l_smtx = rest[i].bone_mat * smtx;
                } else {
                    local_mtx = LocalMtx::Parentless;
                }
            }
            let pose_mat3 = Mat3::from_mat4(mats[i].pose_mat);
            let axismtx = normalize_m3(&(omat * pose_mat3));
            let ch = pb.channels();
            let gimbal = gimbal_pose(&ch, &bpt, &omat).unwrap_or(axismtx);
            Some(PoseTd {
                ob,
                name: b.name.clone(),
                center: mats[i].pose_mat.w_axis.truncate(),
                no_center,
                connected: b.parent.is_some() && b.use_connect,
                no_loc,
                local_mtx,
                mtx,
                smtx,
                r_mtx,
                r_smtx,
                l_smtx,
                axismtx,
                gimbal,
                omat,
                iomat,
                init: ch,
                locks: locks_of(pb),
            })
        })
        .collect()
}

/// The orientation inputs in Pose Mode: LOCAL/NORMAL from the bones
/// (`getTransformOrientation_ex`), GIMBAL from the active bone.
fn orient_source(c: &Context, pivot: PivotPoint, tds: &[PoseTd]) -> OrientSource {
    let mut src = crate::object::orient_source(c);
    src.active_world = None;
    src.active_gimbal = None;
    let Some(active) = c.active_object() else {
        return src;
    };
    let obmat = Mat3::from_mat4(world_matrix(&c.main.objects, active));
    let active_bone = c.active_pose_bone().filter(|b| b.object == active);
    let (mut normal, mut plane) = (Vec3::ZERO, Vec3::ZERO);
    let mut ok = false;
    if let (Some(o), Some(arm)) = (
        c.main.objects.get(active),
        c.main
            .objects
            .get(active)
            .and_then(|o| o.data)
            .and_then(|d| d.typed::<ArmatureId>())
            .and_then(|a| c.main.armatures.get(a)),
    ) {
        let rest = rest_pose(arm);
        let mats = where_is(arm, &rest, o.pose.as_ref(), false);
        let bone_mat = |name: &str| {
            arm.bone_index(name)
                .map(|i| Mat3::from_mat4(mats[i].pose_mat))
        };
        if pivot == PivotPoint::ActiveElement
            && let Some(m) = active_bone.as_ref().and_then(|b| bone_mat(&b.bone))
        {
            normal += m.z_axis;
            plane += m.y_axis;
            ok = true;
        } else {
            for td in tds.iter().filter(|t| t.ob == active) {
                if let Some(m) = bone_mat(&td.name) {
                    normal += m.z_axis;
                    plane += m.y_axis;
                    ok = true;
                }
            }
        }
        if let Some(b) = active_bone.as_ref()
            && let (Some(i), Some(pb)) = (
                arm.bone_index(&b.bone),
                o.pose.as_ref().and_then(|p| p.bone(&b.bone)),
            )
        {
            let parent_pose = arm.bones[i].parent.map(|p| mats[p].pose_mat);
            let bpt = parent_transform(arm, &rest, i, parent_pose.as_ref());
            src.active_gimbal = gimbal_pose(&pb.channels(), &bpt, &obmat);
        }
    }
    if ok {
        let m = invert_m3_safe_ortho(&obmat).transpose();
        let n = crate::edit::mathx::normalize(m * normal).0;
        let p = crate::edit::mathx::normalize(m * plane).0;
        if let Some(space) = crate::edit::mathx::create_space_normal_tangent(n, p) {
            src.active_world = Some(space);
        }
    }
    src
}

/// The new channels of one bone.
fn element(op: TransformOp, s: &Setup, td: &PoseTd) -> Channels {
    let mut out = td.init;
    let center_local = |td: &PoseTd| -> Vec3 {
        if s.pivot.is_local() {
            td.center
        } else {
            s.centers_local
                .iter()
                .find(|(o, _)| *o == td.ob)
                .map_or(Vec3::ZERO, |(_, c)| *c)
        }
    };
    match op {
        TransformOp::Rotate => {
            let mut axis = s.vec;
            if let Some(con) = s.con
                && con.space == ConstraintSpace::PerElement
            {
                axis = con.rot_axis(&s.elem_axes(td));
            }
            let mat = axis_angle_normalized_to_mat3(axis, s.angle);
            let center = center_local(td);
            if !td.no_loc {
                let mut vec = td.center - center;
                vec = td.omat * vec;
                vec = mat * vec;
                vec = td.iomat * vec;
                vec += center;
                vec -= td.center;
                vec = match td.local_mtx {
                    LocalMtx::Parentless => vec,
                    LocalMtx::Child => td.l_smtx * (td.omat * vec),
                    LocalMtx::None => td.smtx * (td.omat * vec),
                };
                vec = protect_loc(&td.locks, vec);
                out.location = td.init.location + vec;
            }
            match td.init.mode {
                RotationMode::Quaternion => {
                    let fmat = td.r_smtx * mat * td.r_mtx;
                    let quat = mat3_to_quat(&fmat);
                    let q = mul_qt_qtqt(quat, td.init.quat);
                    out.quat = protect_quat(&td.locks, q, td.init.quat);
                }
                RotationMode::AxisAngle => {
                    let iquat = axis_angle_to_quat(td.init.axis, td.init.angle);
                    let fmat = td.r_smtx * mat * td.r_mtx;
                    let quat = mat3_to_quat(&fmat);
                    let tquat = mul_qt_qtqt(quat, iquat);
                    let (axis, angle) = quat_to_axis_angle(tquat);
                    let (axis, angle) =
                        protect_axis_angle(&td.locks, axis, angle, td.init.axis, td.init.angle);
                    out.axis = axis;
                    out.angle = angle;
                }
                RotationMode::Euler(order) => {
                    let totmat = mat * td.r_mtx;
                    let smat = td.r_smtx * totmat;
                    let eulmat = eul_o_to_mat3(td.init.euler, order);
                    let fmat = smat * eulmat;
                    let eul = mat3_to_compatible_eul_o(&fmat, td.init.euler, order);
                    out.euler = protect_euler(&td.locks, eul, td.init.euler);
                }
            }
        }
        TransformOp::Resize => {
            let mut tmat = s.mat;
            if let Some(con) = s.con {
                tmat = con.apply_size_elem(&s.elem_axes(td), tmat);
            }
            let center = center_local(td);
            let fscale = trans_mat3_to_size(&(tmat * td.axismtx), &td.axismtx);
            let fscale = protect_scale(&td.locks, fscale);
            out.scale = td.init.scale * fscale;
            let mut vec = tmat * (td.center - center) + center;
            vec -= td.center;
            vec = td.omat * vec;
            vec = td.smtx * vec;
            vec = protect_loc(&td.locks, vec);
            out.location = td.init.location + vec;
        }
        _ => {
            let tvec = match s.con {
                Some(con) => con.apply_vec_elem(&s.orient, &s.elem_axes(td), s.vec),
                None => s.vec,
            };
            let tvec = protect_loc(&td.locks, td.smtx * tvec);
            out.location = td.init.location + tvec;
        }
    }
    out
}

/// What every element shares.
struct Setup {
    orient: Orientation,
    con: Option<Constraint>,
    pivot: PivotPoint,
    /// The center in each object's space (`tc->center_local`).
    centers_local: Vec<(IdRef<ObjectId>, Vec3)>,
    vec: Vec3,
    angle: f32,
    mat: Mat3,
}

impl Setup {
    fn elem_axes(&self, td: &PoseTd) -> Mat3 {
        if self.orient.kind == OrientType::Gimbal {
            td.gimbal
        } else {
            td.axismtx
        }
    }
}

fn unsupported(
    p: &TransformParams,
    c: &Context,
    objects: &[(IdRef<ObjectId>, IdRef<ArmatureId>)],
) -> Result<(), OpError> {
    let what = if p.proportional.is_some() {
        "proportional editing"
    } else if p.cursor_transform {
        "cursor_transform=True"
    } else if objects.iter().any(|(ob, _)| {
        c.main
            .objects
            .get(*ob)
            .and_then(|o| o.pose.as_ref())
            .is_some_and(|p| p.use_mirror_x || p.use_auto_ik)
    }) {
        "Pose.use_mirror_x / use_auto_ik"
    } else {
        return Ok(());
    };
    Err(OpError::NotImplemented(format!(
        "frap: pose-mode transforms with {what} are not implemented yet"
    )))
}

/// The pose-mode handler.
fn apply(cx: &mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult {
    let p = TransformParams::from_args(op, args);
    let mut reports: Vec<(ReportType, &'static str)> = Vec::new();
    let results: Option<Vec<(PoseTd, Channels)>> = {
        let c = cx.ctx();
        let objects = pose_objects(&c);
        unsupported(&p, &c, &objects)?;
        let pivot = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
        let local_origins = pivot == PivotPoint::IndividualOrigins;
        let mut tds: Vec<PoseTd> = Vec::new();
        for &(ob, arm) in &objects {
            if c.main
                .armatures
                .get(arm)
                .is_some_and(|a| a.pose_position == "REST")
            {
                if !transformed_bones_empty(&c, ob, arm) {
                    reports.push((
                        ReportType::Error,
                        "Cannot change Pose when 'Rest Position' is enabled",
                    ));
                }
                continue;
            }
            tds.extend(create(&c, ob, arm, op, local_origins));
        }
        if tds.is_empty() {
            if reports.is_empty() {
                reports.push((
                    ReportType::Warning,
                    "frap: nothing to transform: no visible pose bone is selected",
                ));
            }
            None
        } else {
            // `createTransPose`: without a bone that can move, Blender rotates (or resizes)
            // instead of translating.
            if op == TransformOp::Translate
                && !tds.iter().any(|td| {
                    if td.connected {
                        td.no_center
                    } else {
                        td.locks.loc != [true; 3]
                    }
                })
            {
                return Err(OpError::NotImplemented(
                "frap: translating only connected (or location-locked) pose bones is not implemented yet (Blender rotates them instead)".into(),
            ));
            }
            let src = orient_source(&c, pivot, &tds);
            let orient = Orientation::from_props(
                p.orient_type.as_deref(),
                p.orient_matrix,
                &p.orient_matrix_type,
                &src,
            );
            let con = Constraint::from_props(
                p.value_is_set_array(),
                p.constraint_axis,
                &orient,
                tds.len(),
            );
            let world_centers: Vec<Vec3> = tds
                .iter()
                .filter(|t| !t.no_center)
                .map(|t| world_matrix(&c.main.objects, t.ob).transform_point3(t.center))
                .collect();
            let active_center = c.active_pose_bone().and_then(|b| {
                tds.iter()
                    .find(|t| t.ob == b.object && t.name == b.bone)
                    .map(|t| world_matrix(&c.main.objects, t.ob).transform_point3(t.center))
                    .or_else(|| active_bone_center(&c, b.object, &b.bone))
            });
            let center = pivot::center(
                pivot,
                &world_centers,
                Vec3::from_array(c.cursor().location),
                active_center,
                p.center_override,
            );
            let centers_local = objects
                .iter()
                .map(|&(ob, _)| {
                    let m: Mat4 = world_matrix(&c.main.objects, ob);
                    let im = if m.determinant() != 0.0 {
                        m.inverse()
                    } else {
                        Mat4::IDENTITY
                    };
                    (ob, im.transform_point3(center))
                })
                .collect();
            let values = p.value.unwrap_or(Vec3::ZERO);
            let (mut vec, mut angle, mut mat) = (Vec3::ZERO, 0.0, Mat3::IDENTITY);
            match op {
                TransformOp::Rotate => {
                    angle = p.angle();
                    vec = match con {
                        Some(con) => {
                            let axes = match (con.space, tds.first()) {
                                (ConstraintSpace::PerElement, Some(td))
                                    if orient.kind == OrientType::Gimbal =>
                                {
                                    td.gimbal
                                }
                                (ConstraintSpace::PerElement, Some(td)) => td.axismtx,
                                _ => orient.matrix,
                            };
                            con.rot_axis(&axes)
                        }
                        None => -orient.matrix.col(p.orient_axis),
                    };
                }
                TransformOp::Resize => {
                    mat = size_to_mat3(values);
                    if let Some(con) = con {
                        mat = con.apply_size(&orient, mat);
                    }
                }
                _ => {
                    vec = orient.matrix * values;
                    if let Some(con) = con {
                        vec = con.apply_vec(&orient, vec);
                    }
                }
            }
            let s = Setup {
                orient,
                con,
                pivot,
                centers_local,
                vec,
                angle,
                mat,
            };
            Some(
                tds.into_iter()
                    .map(|td| {
                        let out = element(op, &s, &td);
                        (td, out)
                    })
                    .collect(),
            )
        }
    };
    for (level, msg) in reports {
        if level == ReportType::Error {
            return Err(OpError::Runtime(msg.into()));
        }
        cx.report(level, msg);
    }
    let Some(results) = results else {
        return Ok(OpStatus::Cancelled);
    };
    for (td, out) in results {
        if out == td.init {
            continue;
        }
        let o = cx.object_mut(td.ob)?;
        if let Some(pb) = o.pose.as_mut().and_then(|p| p.bone_mut(&td.name)) {
            pb.set_channels(&out);
        }
    }
    Ok(OpStatus::Finished)
}

/// Whether no bone of `ob` would be transformed (the rest-position report needs a selection).
fn transformed_bones_empty(c: &Context, ob: IdRef<ObjectId>, arm: IdRef<ArmatureId>) -> bool {
    let (Some(o), Some(a)) = (c.main.objects.get(ob), c.main.armatures.get(arm)) else {
        return true;
    };
    o.pose
        .as_ref()
        .is_none_or(|p| transformed_bones(a, p, TransformOp::Translate, false).is_empty())
}

/// `calc_active_center_for_posemode` (world space) for an active bone that is not transformed.
fn active_bone_center(c: &Context, ob: IdRef<ObjectId>, bone: &str) -> Option<Vec3> {
    let o = c.main.objects.get(ob)?;
    let arm = c.main.armatures.get(o.data?.typed::<ArmatureId>()?)?;
    let pb = o.pose.as_ref()?.bone(bone)?;
    if !pb.select {
        return None;
    }
    let rest = rest_pose(arm);
    let mats = where_is(arm, &rest, o.pose.as_ref(), false);
    let i = arm.bone_index(bone)?;
    Some(world_matrix(&c.main.objects, ob).transform_point3(mats[i].pose_mat.w_axis.truncate()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternion_locks_4d_keep_components() {
        let l = Locks {
            rot_w: true,
            rot_4d: true,
            ..Locks::default()
        };
        let q = protect_quat(&l, [0.5, 0.5, 0.5, 0.5], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(q, [1.0, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn location_locks_zero_components() {
        let l = Locks {
            loc: [true, false, true],
            ..Locks::default()
        };
        assert_eq!(protect_loc(&l, Vec3::ONE), Vec3::new(0.0, 1.0, 0.0));
    }
}
