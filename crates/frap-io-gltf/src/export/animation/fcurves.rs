// SPDX-License-Identifier: GPL-3.0-or-later
//! F-curve channel groups and the non-sampled export (`export_force_sampling=False`).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/animation/fcurves/channels.py` (`get_channel_groups`, `needs_baking`,
//! `__get_channel_group_sorted`, `gather_animation_fcurves_channels`),
//! `fcurves/keyframes.py` (`gather_fcurve_keyframes`, `gather_non_keyed_values`,
//! `complete_key`) and `fcurves/sampler.py` (`__convert_keyframes`, `__gather_interpolation`),
//! with `blender/com/data_path.py` and `blender/com/conversion.py` (`get_target`,
//! `get_channel_from_target`).
//!
//! The F-curves of the slot are grouped by target (the object, a pose bone, the shape keys)
//! and RNA property; each group whose keys share frames and a BEZIER, LINEAR or CONSTANT
//! interpolation becomes one CUBICSPLINE, LINEAR or STEP channel with Blender's keys (BEZIER
//! tangents from the handles, the first in- and last out-tangent zero); the other groups are
//! sampled. Values are transformed by `matrix_parent_inverse` (objects with a parent) or the
//! bone's correction matrix, as the add-on does.

use frap_anim::{Action, FCurve, Interpolation};
use frap_gltf::json::accessor::AccessorType;
use frap_gltf::json::animation::AnimationPath;
use frap_math::glam::Mat4;
use frap_math::matrix::decompose_wxyz;
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::id::Object;
use indexmap::IndexMap;
use smol_str::SmolStr;

use super::exp::Exp;
use super::keys::{Convert, Interp, Key, target_len};
use super::world::exported_shape_keys;
use super::{GChannel, GSampler, Negative};

/// The kind of a channel target.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TType {
    /// The object's own transforms.
    Object,
    /// A pose bone.
    Bone,
    /// The shape keys (`weights`).
    Sk,
}

/// Which datablock or bone a group of F-curves animates.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum TargetId {
    Object,
    Bone(SmolStr),
    Key,
}

/// The F-curves of one target, by RNA property.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    /// The kind.
    pub ty: TType,
    /// The bone (bones only).
    pub bone: Option<SmolStr>,
    /// RNA property → its F-curves (shape keys: one per exported shape key, `None` where a
    /// key is not animated).
    pub props: IndexMap<SmolStr, Vec<Option<FCurve>>>,
}

/// A group that must be sampled: `(type, channel, bone)`. The channel is
/// `get_channel_from_target(get_target(prop))`: `None` for the shape keys and for an object
/// property that is not a transform (`hide_render`, `color`, …), which the add-on then samples
/// as nothing.
pub(crate) type ToSample = (TType, Option<&'static str>, Option<SmolStr>);

/// What `get_channel_groups` returns.
#[derive(Clone, Debug, Default)]
pub(crate) struct Groups {
    /// The groups that can be exported from the F-curves (or every group with
    /// `no_sample_option`).
    pub targets: Vec<Target>,
    /// `(type, channel, bone)` that must be sampled.
    pub to_be_sampled: Vec<ToSample>,
}

/// `get_target(property)`: the glTF path of an RNA property.
pub(crate) fn get_target(prop: &str) -> Option<AnimationPath> {
    match prop {
        "delta_location" | "location" => Some(AnimationPath::Translation),
        "delta_rotation_euler"
        | "delta_rotation_quaternion"
        | "rotation_axis_angle"
        | "rotation_euler"
        | "rotation_quaternion" => Some(AnimationPath::Rotation),
        "delta_scale" | "scale" => Some(AnimationPath::Scale),
        "value" => Some(AnimationPath::Weights),
        _ => None,
    }
}

/// `get_channel_from_target(target)`.
pub(crate) fn channel_from_target(t: &AnimationPath) -> Option<&'static str> {
    match t {
        AnimationPath::Rotation => Some("rotation_quaternion"),
        AnimationPath::Translation => Some("location"),
        AnimationPath::Scale => Some("scale"),
        _ => None,
    }
}

/// `get_target_property_name(data_path)`: the last path segment (`None` for a path ending in
/// ``).
pub(crate) fn target_property(path: &str) -> Option<&str> {
    if path.ends_with(']') {
        None
    } else {
        Some(path.rsplit('.').next().unwrap_or(path))
    }
}

/// `get_target_object_path(data_path)`.
fn target_object_path(path: &str) -> String {
    if path.ends_with(']') {
        return path.rsplit_once('[').map_or("", |(a, _)| a).to_owned();
    }
    if path.starts_with("pose.bones[") {
        let end = path.find("\"]").unwrap_or(path.len());
        return format!("{}\"]", &path[..end]);
    }
    match path.rsplit_once('.') {
        Some((a, _)) => a.to_owned(),
        None => String::new(),
    }
}

/// `get_rotation_modes(target_property)`.
fn rotation_modes(prop: &str) -> Option<&'static [&'static str]> {
    match prop {
        "rotation_euler" | "delta_rotation_euler" => {
            Some(&["XYZ", "XZY", "YXZ", "YZX", "ZXY", "ZYX"])
        }
        "rotation_quaternion" | "delta_rotation_quaternion" => Some(&["QUATERNION"]),
        "rotation_axis_angle" => Some(&["AXIS_ANGLE"]),
        _ => None,
    }
}

/// The name inside `…["name"]…` (the first quoted key), unescaped.
fn quoted_name(path: &str) -> Option<String> {
    let start = path.find("[\"")? + 2;
    let mut out = String::new();
    let mut chars = path[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// The F-curves of `slot` in `action` (`get_channelbag_for_slot`: the first strip of the
/// first layer).
pub(crate) fn slot_fcurves(action: &Action, handle: i32) -> &[FCurve] {
    action
        .channelbag(handle)
        .map_or(&[], |b| b.fcurves.as_slice())
}

/// `get_channel_groups(obj, action, slot, no_sample_option)`.
pub(crate) fn channel_groups(
    exp: &mut Exp<'_>,
    ob_ref: IdRef<ObjectId>,
    action: &Action,
    handle: i32,
    no_sample_option: bool,
) -> Groups {
    let Some(ob) = exp.world.ob(ob_ref).cloned() else {
        return Groups::default();
    };
    let ob_type = ob.object_type();
    let bones: Vec<SmolStr> = ob
        .data
        .and_then(|d| d.typed::<ArmatureId>())
        .and_then(|a| exp.world.main.armatures.get(a))
        .map(|a| a.bones.iter().map(|b| b.name.clone()).collect())
        .unwrap_or_default();
    let key = exp.world.key(ob_ref).cloned();
    let mut targets: IndexMap<TargetId, Target> = IndexMap::new();
    let mut multiple_rotation: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for fc in slot_fcurves(action, handle) {
        if fc.keyframe_points.is_empty() {
            continue;
        }
        let path = fc.data_path.as_str();
        let prop = target_property(path);
        let object_path = target_object_path(path);
        let (id, ty, rotation_mode, target_name): (TargetId, TType, Option<SmolStr>, String) =
            if object_path.is_empty() {
                if path.starts_with('[') {
                    continue; // EXTRA: custom properties (extra animations only)
                }
                (
                    TargetId::Object,
                    TType::Object,
                    Some(ob.rotation_mode.clone()),
                    ob.header.name.to_string(),
                )
            } else if ob_type == "ARMATURE" && path.starts_with("pose.bones[") {
                let Some(name) = quoted_name(path).filter(|n| bones.iter().any(|b| b == n)) else {
                    warnings.push(format!("Animation target {object_path} not found"));
                    continue;
                };
                if prop.and_then(get_target).is_none() {
                    continue; // EXTRA
                }
                let mode = ob
                    .pose
                    .as_ref()
                    .and_then(|p| p.bones.iter().find(|b| b.name == name))
                    .map_or_else(|| SmolStr::new("QUATERNION"), |b| b.rotation_mode.clone());
                (
                    TargetId::Bone(name.clone().into()),
                    TType::Bone,
                    Some(mode),
                    name,
                )
            } else if ob_type == "MESH" && object_path.starts_with("key_blocks") {
                let Some(k) = &key else {
                    continue;
                };
                let block = match quoted_name(&object_path) {
                    Some(n) => k.key_blocks.iter().position(|b| b.name == n),
                    None => object_path
                        .trim_start_matches("key_blocks[")
                        .trim_end_matches(']')
                        .parse::<usize>()
                        .ok()
                        .filter(|i| *i < k.key_blocks.len()),
                };
                let Some(bi) = block else {
                    warnings.push(format!(
                        "Invalid animation fcurve data path on action {}",
                        action.header.name
                    ));
                    continue;
                };
                if !exported_shape_keys(k).contains(&bi) {
                    continue;
                }
                (TargetId::Key, TType::Sk, None, k.header.name.to_string())
            } else {
                continue; // EXTRA (modifiers, materials, …) or not resolvable
            };
        let prop = prop.unwrap_or(path);
        if let Some(modes) = rotation_modes(prop)
            && !rotation_mode.as_deref().is_some_and(|m| modes.contains(&m))
        {
            if !multiple_rotation.contains(&target_name) {
                multiple_rotation.push(target_name);
            }
            continue;
        }
        let t = targets.entry(id.clone()).or_insert_with(|| Target {
            ty,
            bone: match &id {
                TargetId::Bone(b) => Some(b.clone()),
                _ => None,
            },
            props: IndexMap::new(),
        });
        t.props
            .entry(SmolStr::new(prop))
            .or_default()
            .push(Some(fc.clone()));
    }
    for w in warnings {
        exp.warn(w);
    }
    for n in multiple_rotation {
        exp.warn(format!("Multiple rotation mode detected for {n}"));
    }
    let mut to_be_sampled: Vec<ToSample> = Vec::new();
    let push = |v: &mut Vec<ToSample>, e| {
        if !v.contains(&e) {
            v.push(e);
        }
    };
    let mut out = Vec::new();
    for (_, mut t) in targets {
        // Normal + delta transforms of one path cannot be exported from F-curves.
        let paths: Vec<Option<AnimationPath>> = t.props.keys().map(|p| get_target(p)).collect();
        let mut kept = IndexMap::new();
        for (prop, chans) in std::mem::take(&mut t.props) {
            let gt = get_target(&prop);
            if paths.iter().filter(|p| **p == gt).count() > 1 {
                // `None`, because no delta exists on bones.
                let c = gt.as_ref().and_then(channel_from_target);
                push(&mut to_be_sampled, (t.ty, c, None));
            } else {
                kept.insert(prop, chans);
            }
        }
        let mut props = IndexMap::new();
        for (prop, chans) in kept {
            if !no_sample_option && needs_baking(exp, &chans) {
                let c = get_target(&prop).as_ref().and_then(channel_from_target);
                push(&mut to_be_sampled, (t.ty, c, t.bone.clone()));
            } else {
                props.insert(prop, chans);
            }
        }
        if t.ty == TType::Sk
            && let Some(k) = &key
        {
            for chans in props.values_mut() {
                *chans = sorted_sk_channels(k, std::mem::take(chans));
            }
        }
        t.props = props;
        out.push(t);
    }
    Groups {
        targets: out,
        to_be_sampled,
    }
}

/// `__get_channel_group_sorted`: shape-key F-curves in exported-shape-key order, `None` for a
/// shape key without one.
fn sorted_sk_channels(
    key: &frap_scene::id::Key,
    chans: Vec<Option<FCurve>>,
) -> Vec<Option<FCurve>> {
    let exported = exported_shape_keys(key);
    let mut slots: Vec<Option<FCurve>> = vec![None; exported.len()];
    let mut any = false;
    for c in chans.iter().flatten() {
        let name = quoted_name(&c.data_path);
        let bi = match name {
            Some(n) => key.key_blocks.iter().position(|b| b.name == n),
            None => target_object_path(&c.data_path)
                .trim_start_matches("key_blocks[")
                .trim_end_matches(']')
                .parse::<usize>()
                .ok(),
        };
        if let Some(pos) = bi.and_then(|bi| exported.iter().position(|e| *e == bi)) {
            slots[pos] = Some(c.clone());
            any = true;
        }
    }
    if any { slots } else { chans }
}

/// `needs_baking(obj, channels)`.
fn needs_baking(exp: &mut Exp<'_>, chans: &[Option<FCurve>]) -> bool {
    let present: Vec<&FCurve> = chans.iter().flatten().collect();
    let Some(first) = present.first() else {
        return true;
    };
    let ipo = first.keyframe_points[0].interpolation;
    if !matches!(
        ipo,
        Interpolation::Bezier | Interpolation::Linear | Interpolation::Constant
    ) {
        exp.warn(format!(
            "Baking animation because of an unsupported interpolation method: {}",
            ipo.identifier()
        ));
        return true;
    }
    if present
        .iter()
        .any(|c| c.keyframe_points.iter().any(|k| k.interpolation != ipo))
    {
        exp.warn(
            "Baking animation because there are keyframes with different interpolation methods in one channel",
        );
        return true;
    }
    let n = first.keyframe_points.len();
    if present.iter().any(|c| c.keyframe_points.len() != n) {
        exp.warn(
            "Baking animation because the number of keyframes is not equal for all channel tracks",
        );
        return true;
    }
    if n <= 1 {
        return true;
    }
    let frames = |c: &FCurve| {
        c.keyframe_points
            .iter()
            .map(|k| k.co[0])
            .collect::<Vec<_>>()
    };
    let f0 = frames(first);
    if present.iter().any(|c| frames(c) != f0) {
        exp.warn("Baking animation because of differently located keyframes in one channel");
        return true;
    }
    false
}

/// `gather_non_keyed_values` for one group.
fn non_keyed_values(
    exp: &Exp<'_>,
    ob_ref: IdRef<ObjectId>,
    ob: &Object,
    prop: &str,
    chans: &[Option<FCurve>],
    bone: Option<&str>,
) -> Vec<Option<f32>> {
    if chans.iter().all(Option::is_some) {
        if prop == "value" {
            return vec![None; chans.len()];
        }
        let len = target_len(prop, chans.len());
        let indices: Vec<i32> = chans.iter().flatten().map(|c| c.array_index).collect();
        let current: Vec<f32> = match bone {
            None => match prop {
                "delta_location" => ob.delta_location.to_vec(),
                "delta_rotation_euler" => ob.delta_rotation_euler.to_vec(),
                "delta_rotation_quaternion" => ob.delta_rotation_quaternion.to_vec(),
                "delta_scale" => ob.delta_scale.to_vec(),
                "location" => ob.location.to_vec(),
                "rotation_axis_angle" => ob.rotation_axis_angle.to_vec(),
                "rotation_euler" => ob.rotation_euler.to_vec(),
                "rotation_quaternion" => ob.rotation_quaternion.to_vec(),
                "scale" => ob.scale.to_vec(),
                _ => return Vec::new(),
            },
            Some(b) => {
                // `pose.bones[bone].matrix_basis.decompose()`.
                let basis = ob
                    .pose
                    .as_ref()
                    .and_then(|p| p.bones.iter().find(|pb| pb.name == b))
                    .map_or(Mat4::IDENTITY, pose_bone_basis);
                let (t, r, s) = decompose_wxyz(&basis);
                match prop {
                    "location" => t.to_array().to_vec(),
                    "scale" => s.to_array().to_vec(),
                    _ => r.to_vec(),
                }
            }
        };
        return (0..len)
            .map(|i| {
                if indices.contains(&(i as i32)) {
                    None
                } else {
                    current.get(i).copied()
                }
            })
            .collect();
    }
    // Shape keys with some keys not animated: their current values.
    let Some(key) = exp.world.key(ob_ref) else {
        return vec![None; chans.len()];
    };
    let exported = exported_shape_keys(key);
    chans
        .iter()
        .enumerate()
        .map(|(i, c)| match c {
            Some(_) => None,
            None => exported.get(i).map(|bi| key.key_blocks[*bi].value),
        })
        .collect()
}

/// `PoseBone.matrix_basis` (`BKE_pchan_to_mat4`).
fn pose_bone_basis(pb: &frap_scene::id::PoseBone) -> Mat4 {
    use frap_math::matrix::{ObjectTransform, RotationMode, RotationValues};
    ObjectTransform {
        location: pb.location,
        rotation_mode: RotationMode::from_blender(&pb.rotation_mode).unwrap_or_default(),
        rotation: RotationValues {
            quaternion: pb.rotation_quaternion,
            euler: pb.rotation_euler,
            axis_angle: pb.rotation_axis_angle,
        },
        scale: pb.scale,
        delta_location: [0.0; 3],
        delta_rotation_euler: [0.0; 3],
        delta_rotation_quaternion: [1.0, 0.0, 0.0, 0.0],
        delta_scale: [1.0; 3],
    }
    .to_mat4()
}

/// `__set_indexed`: a full-length value with the keyed components at their indices.
fn set_indexed(len: usize, indices: &[usize], vals: &[f64]) -> Vec<f32> {
    let mut out = vec![0.0f32; len];
    for (i, v) in indices.iter().zip(vals) {
        if let Some(slot) = out.get_mut(*i) {
            *slot = *v as f32;
        }
    }
    out
}

/// `gather_fcurve_keyframes(obj, group, bone, custom_range)`.
fn fcurve_keys(
    exp: &Exp<'_>,
    ob_ref: IdRef<ObjectId>,
    prop: &str,
    chans: &[Option<FCurve>],
    bone: Option<&str>,
    custom_range: Option<[f32; 2]>,
) -> Option<Vec<Key>> {
    let ob = exp.world.ob(ob_ref)?.clone();
    let non_keyed = non_keyed_values(exp, ob_ref, &ob, prop, chans, bone);
    let present: Vec<&FCurve> = chans.iter().flatten().collect();
    let first = present.first()?;
    let mut frames: Vec<f32> = first.keyframe_points.iter().map(|k| k.co[0]).collect();
    frames.sort_by(f32::total_cmp);
    frames.dedup();
    if exp.s.negative == Negative::Crop {
        frames.retain(|f| *f >= 0.0);
    }
    if exp.s.frame_range {
        let (a, b) = (exp.frame_start as f32, exp.frame_end as f32);
        frames.retain(|f| *f >= a && *f <= b);
    }
    if let Some([a, b]) = custom_range {
        frames.retain(|f| *f >= a && *f <= b);
    }
    if frames.is_empty() {
        return None;
    }
    let is_value = prop == "value";
    let len = target_len(prop, chans.len());
    let indices: Vec<usize> = if is_value {
        chans
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_some())
            .map(|(i, _)| i)
            .collect()
    } else {
        present
            .iter()
            .map(|c| usize::try_from(c.array_index).unwrap_or(usize::MAX))
            .collect()
    };
    let complete = |v: &mut Vec<f32>| {
        if present.len() != len {
            for (i, slot) in v.iter_mut().enumerate() {
                if !indices.contains(&i)
                    && let Some(Some(nk)) = non_keyed.get(i)
                {
                    *slot = *nk;
                }
            }
        }
    };
    let bezier = first.keyframe_points[0].interpolation == Interpolation::Bezier;
    let (f_first, f_last) = (frames[0], frames[frames.len() - 1]);
    let mut keys = Vec::with_capacity(frames.len());
    for (i, &frame) in frames.iter().enumerate() {
        let vals: Vec<f64> = present
            .iter()
            .map(|c| f64::from(frap_anim::eval::evaluate_fcurve(c, frame)))
            .collect();
        let mut value = set_indexed(len, &indices, &vals);
        complete(&mut value);
        let mut key = Key::new(f64::from(frame), value.clone());
        if bezier {
            let control = |left: bool| -> Vec<f32> {
                let vals: Vec<f64> = present
                    .iter()
                    .map(|c| {
                        let Some(k) = c.keyframe_points.get(i) else {
                            return 0.0;
                        };
                        let h = if left { k.handle_left } else { k.handle_right };
                        let (cx, cy) = (f64::from(k.co[0]), f64::from(k.co[1]));
                        cy + (f64::from(h[1]) - cy) / (f64::from(h[0]) - cx)
                    })
                    .collect();
                let mut t = set_indexed(len, &indices, &vals);
                // `__complete_key_tangents`.
                for (j, slot) in t.iter_mut().enumerate() {
                    if !indices.contains(&j)
                        && let Some(Some(nk)) = non_keyed.get(j)
                    {
                        *slot = *nk;
                    }
                }
                t
            };
            key.in_tangent = Some(if frame == f_first {
                value.clone()
            } else {
                control(true)
            });
            key.out_tangent = Some(if frame == f_last {
                value.clone()
            } else {
                control(false)
            });
        }
        keys.push(key);
    }
    Some(keys)
}

/// `gather_animation_fcurves_channels(obj, action, slot)`: the channels of the groups that
/// need no sampling (and `to_be_sampled` for the others).
pub(crate) fn fcurve_channels(
    exp: &mut Exp<'_>,
    ob_ref: IdRef<ObjectId>,
    action: &Action,
    handle: i32,
) -> (Vec<GChannel>, Vec<ToSample>) {
    let groups = channel_groups(exp, ob_ref, action, handle, false);
    let custom_range = action.use_frame_range.then_some(action.frame_range);
    let mut out = Vec::new();
    for t in &groups.targets {
        for (prop, chans) in &t.props {
            if let Some(c) = fcurve_channel(exp, ob_ref, t, prop, chans, custom_range) {
                out.push(c);
            }
        }
    }
    (out, groups.to_be_sampled)
}

/// `__gather_animation_fcurve_channel` for one property group.
fn fcurve_channel(
    exp: &mut Exp<'_>,
    ob_ref: IdRef<ObjectId>,
    t: &Target,
    prop: &str,
    chans: &[Option<FCurve>],
    custom_range: Option<[f32; 2]>,
) -> Option<GChannel> {
    let path = get_target(prop)?;
    let node = match &t.bone {
        Some(b) => exp.nodes.bone(exp.scene, ob_ref, b)?,
        None => exp.node_of(ob_ref)?,
    };
    if path == AnimationPath::Weights && !exp.morph_nodes.contains(&node) {
        return None;
    }
    let keys = fcurve_keys(exp, ob_ref, prop, chans, t.bone.as_deref(), custom_range)?;
    let ob = exp.world.ob(ob_ref)?;
    let matrix = match &t.bone {
        Some(b) => exp.bone_correction(ob_ref, b),
        None if ob.parent.is_some() => ob.matrix_parent_inverse,
        None => Mat4::IDENTITY,
    };
    let conv = Convert {
        target: prop,
        matrix,
        correction: exp.needs_correction(ob_ref),
        yup: exp.s.yup && t.bone.is_none(),
        fps: exp.fps,
    };
    let first = chans.iter().flatten().next()?;
    let interp = Interp::from_blender(first.keyframe_points[0].interpolation, exp.s.fallback);
    let ty = if path == AnimationPath::Weights {
        AccessorType::Scalar
    } else if keys[0].value.len() == 4 || path == AnimationPath::Rotation {
        AccessorType::Vec4
    } else {
        AccessorType::Vec3
    };
    let times: Vec<f64> = keys.iter().map(|k| exp.seconds(k.frame)).collect();
    let values = conv.output(&keys);
    Some(GChannel {
        node,
        path,
        sampler: GSampler::new(times, values, ty, interp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_path_helpers() {
        assert_eq!(target_property("location"), Some("location"));
        assert_eq!(target_property("pose.bones[\"B\"].scale"), Some("scale"));
        assert_eq!(target_property("[\"prop\"]"), None);
        assert_eq!(target_object_path("location"), "");
        assert_eq!(
            target_object_path("pose.bones[\"B.1\"].scale"),
            "pose.bones[\"B.1\"]"
        );
        assert_eq!(
            target_object_path("key_blocks[\"K\"].value"),
            "key_blocks[\"K\"]"
        );
        assert_eq!(
            quoted_name("pose.bones[\"a\\\"b\"].x").as_deref(),
            Some("a\"b")
        );
        assert_eq!(get_target("rotation_euler"), Some(AnimationPath::Rotation));
        assert_eq!(channel_from_target(&AnimationPath::Weights), None);
    }
}
