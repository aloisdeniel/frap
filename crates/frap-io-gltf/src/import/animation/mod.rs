// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF animation import (WP 6.7, `crates/frap-io-gltf/src/import/animation/**`): one action
//! per glTF animation (named after it, made unique as the add-on's `find_unused_name` does,
//! `Anim_<i>` without a name) with one slot per animated object (`OB<name>`) and per animated
//! shape-key datablock (`KE<name>`, named after the object), F-curves from the samplers
//! (LINEAR → LINEAR, STEP → CONSTANT, CUBICSPLINE → BEZIER with AUTO handles; the CUBICSPLINE
//! tangents are dropped), key times `t · fps · fps_base` (the add-on's quirk). Every action is
//! stashed on a muted, locked NLA track named after the animation (`simulate_stash`, the last
//! animation at the bottom), and the first animation is assigned
//! (`BlenderAnimation.restore_animation`).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/imp/scene.py` (`create_animations`), `blender/imp/blender_gltf.py` (`pre_compute`:
//! track names, the per-node channel lists, the shape-key names), `blender/imp/animation.py`
//! (`BlenderAnimation.anim`, `restore_animation`), `animation_node.py`, `animation_weight.py`
//! and `animation_utils.py` (`simulate_stash`, `restore_animation_on_object`, `make_fcurve`,
//! `get_or_create_action_and_slot`), and `blender/com/gltf2_blender_utils.py`
//! (`find_unused_name`). Bone channels target `pose.bones["<name>"]` of the armature through
//! `VNode::blender_bone_name` (WP 6.8), relative to the bone's edit-mode rest transform.
//! `KHR_animation_pointer` channels into node TRS and morph weights (`/nodes/<i>/…`,
//! `/meshes/<i>/weights`) join the node's channel lists as in `pre_compute`; the camera, light
//! and material ones are WP 6.9's `import/pointer/`, called per animation with the same action
//! (`BlenderPointerAnim.anim`) and restored with the objects (M5+M6 W3 integration).

use std::collections::{HashMap, HashSet};

use frap_anim::fcurve::keys;
use frap_anim::{
    Action, ActionLayer, ActionStrip, AnimData, HandleType, Interpolation, KeyframeStrip, NlaStrip,
    NlaTrack,
};
use frap_gltf::json::animation::{AnimationPath, Interpolation as GInterp};
use frap_math::matrix::decompose_wxyz;
use frap_math::quat::QuatWxyz;
use frap_rna::naming::unique_subname;
use frap_rna::{ActionId, ArmatureId, IdRef, KeyId, MeshId, ObjectId};
use frap_scene::edit_armature::rest::rest_pose;
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::convert::{qconj, qmul, qrot, quat, scale, scale_rot_swap};
use super::pointer;
use crate::import::{Gltf, ImportError};

#[cfg(test)]
mod tests;

/// Whether animations are imported (WP 6.7).
pub(crate) const IMPLEMENTED: bool = true;

/// `MINAFRAMEF` / `MAXFRAMEF`.
const MINAFRAME: f32 = -1_048_574.0;
const MAXFRAME: f32 = 1_048_574.0;

/// `find_unused_name(haystack, desired_name)`: `desired_name` (at most 63 UTF-8 bytes), else
/// `desired_name.001`, `.002`, …
pub(crate) fn find_unused_name(haystack: &HashSet<String>, desired: &str) -> String {
    let mut stem: Vec<char> = desired.chars().take(63).collect();
    let mut suffix = String::new();
    let mut cntr = 1;
    loop {
        let name: String = stem.iter().collect::<String>() + &suffix;
        if name.len() > 63 {
            stem.pop();
            continue;
        }
        if !haystack.contains(&name) {
            return name;
        }
        suffix = format!(".{cntr:03}");
        cntr += 1;
    }
}

/// The shape-key name of every morph target of every mesh (`mesh.shapekey_names`, `None` for
/// a target that does not morph `POSITION`).
pub(crate) fn shapekey_names(doc: &frap_gltf::Document) -> Vec<Vec<Option<String>>> {
    let mut out = Vec::with_capacity(doc.meshes.len());
    for mesh in &doc.meshes {
        let mut names = Vec::new();
        let mut used: HashSet<String> = HashSet::from(["Basis".to_owned()]);
        let target_names: Option<Vec<serde_json::Value>> = mesh
            .extras
            .as_ref()
            .and_then(|e| serde_json::from_str::<serde_json::Value>(e.0.get()).ok())
            .and_then(|v| v.get("targetNames")?.as_array().cloned());
        if let Some(prim) = mesh.primitives.iter().find(|p| !p.targets.is_empty()) {
            for sk in 0..prim.targets.len() {
                let morphs_position = mesh.primitives.iter().any(|p| {
                    p.targets
                        .get(sk)
                        .is_some_and(|t| t.contains_key("POSITION"))
                });
                if !morphs_position {
                    names.push(None);
                    continue;
                }
                let mut name: Option<String> = target_names
                    .as_ref()
                    .and_then(|t| t.get(sk))
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        v => v.to_string(),
                    })
                    .filter(|s| !s.is_empty());
                if name.is_none() {
                    name = mesh.primitives[0]
                        .targets
                        .get(sk)
                        .and_then(|t| t.get("POSITION"))
                        .and_then(|a| doc.accessors.get(*a as usize))
                        .and_then(|a| a.name.clone());
                }
                let name = name.unwrap_or_else(|| format!("target_{sk}"));
                let name = find_unused_name(&used, &name);
                used.insert(name.clone());
                names.push(Some(name));
            }
        }
        out.push(names);
    }
    out
}

/// What a slot animates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum Owner {
    Object(IdRef<ObjectId>),
    Key(IdRef<KeyId>),
}

/// `gltf.action_cache[anim_idx]` and `gltf.needs_stash`.
struct AnimState {
    track_name: String,
    action: Option<IdRef<ActionId>>,
    /// (object name, `OBJECT`/`KEY`) → slot handle.
    slots: HashMap<(SmolStr, bool), i32>,
    needs_stash: Vec<(Owner, i32)>,
}

/// Creates the actions of every glTF animation (after the scenes and their objects).
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<(), ImportError> {
    if cx.doc.animations.is_empty() {
        return Ok(());
    }
    // `pre_compute`: track names and the channels of every node per animation.
    let mut used: HashSet<String> = HashSet::new();
    let mut track_names = Vec::with_capacity(cx.doc.animations.len());
    let mut node_anims: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut mesh_weights: HashMap<usize, (usize, usize)> = HashMap::new();
    for (ai, a) in cx.doc.animations.iter().enumerate() {
        let desired = crate::import::truthy_name(&a.name).unwrap_or_else(|| format!("Anim_{ai}"));
        let name = find_unused_name(&used, &desired);
        used.insert(name.clone());
        track_names.push(name);
        for (ci, ch) in a.channels.iter().enumerate() {
            // Core channels, and `dispatch_animation_pointer` for the node TRS / weights
            // pointers of a channel without a node.
            let n = match (ch.target.node, pointer::dispatch(ch)) {
                (Some(n), _) => n as usize,
                (None, Some(pointer::Target::Node { node, .. })) => node,
                (None, Some(pointer::Target::MeshWeights { mesh })) => {
                    // `gltf.data.meshes[int(pointer_tab[2])].weight_animation_on_mesh`: the
                    // last such channel of the file wins.
                    if mesh >= cx.doc.meshes.len() {
                        return Err(bad_channel(
                            cx,
                            ai,
                            ci,
                            &format!(
                                "mesh {mesh} does not exist (IndexError: list index out of range)"
                            ),
                        ));
                    }
                    mesh_weights.insert(mesh, (ai, ci));
                    continue;
                }
                (None, _) => continue,
            };
            // `gltf.data.nodes[channel.target.node]`.
            if n >= cx.doc.nodes.len() {
                return Err(bad_channel(
                    cx,
                    ai,
                    ci,
                    &format!("node {n} does not exist (IndexError: list index out of range)"),
                ));
            }
            node_anims.entry((n, ai)).or_default().push(ci);
        }
    }
    // Mesh weight pointers are broadcast to the nodes instancing the mesh.
    if cx
        .doc
        .extensions_used
        .iter()
        .any(|e| e == "KHR_animation_pointer")
    {
        for (n, node) in cx.doc.nodes.iter().enumerate() {
            if let Some(&(ai, ci)) = node.mesh.and_then(|m| mesh_weights.get(&(m as usize))) {
                node_anims.entry((n, ai)).or_default().push(ci);
            }
        }
    }
    let sk_names = shapekey_names(&cx.doc);
    let (fps, frame_start) = main.scenes.get(cx.scene).map_or((24.0, 1.0), |s| {
        (
            f64::from(s.render.fps) * f64::from(s.render.fps_base),
            s.frame_start as f32,
        )
    });
    // The camera, light and material datablocks and their channel records (WP 6.9).
    let mut pointer_st = pointer::ImportState::of(cx, main);
    // NLA tracks are added bottom to top: the last animation first.
    for ai in (0..cx.doc.animations.len()).rev() {
        let mut st = AnimState {
            track_name: track_names[ai].clone(),
            action: None,
            slots: HashMap::new(),
            needs_stash: Vec::new(),
        };
        for v in 0..cx.vtree.nodes.len() {
            if let Some(n) = cx.vtree.nodes[v].gltf_node {
                for ci in node_anims.get(&(n, ai)).cloned().unwrap_or_default() {
                    node_channel(cx, main, log, &mut st, ai, v, ci, fps)?;
                }
            }
            weight_channel(cx, main, log, &mut st, ai, v, &node_anims, &sk_names, fps)?;
        }
        // `BlenderPointerAnim.anim` for the cameras, lights and materials, into the same
        // action (`gltf.action_cache` is shared).
        let mut cache = pointer::ActionCache {
            action: st.action,
            ..Default::default()
        };
        pointer::anim(
            cx,
            main,
            log,
            ai,
            &st.track_name,
            &mut pointer_st,
            &mut cache,
        )?;
        st.action = st.action.or(cache.action);
        if let Some(action) = st.action {
            for (owner, handle) in std::mem::take(&mut st.needs_stash) {
                stash(
                    main,
                    log,
                    owner,
                    &st.track_name,
                    action,
                    handle,
                    frame_start,
                );
            }
        }
        for (root, action, slot) in cache.needs_stash {
            pointer::simulate_stash(
                main,
                log,
                root,
                &st.track_name,
                action,
                slot,
                frame_start as i32,
            );
        }
    }
    let restored = restore_animation(cx, main, log, &track_names[0]);
    pointer::restore(main, log, &pointer_st.created, &track_names[0]);
    apply_restored(main, log, cx.scene, &restored);
    Ok(())
}

/// The object that holds the channels of vnode `v`: its own object, or for a bone (WP 6.8)
/// the armature object it belongs to, with the bone's name.
fn animated_object(cx: &Gltf, main: &Main, v: usize) -> Option<(IdRef<ObjectId>, Option<SmolStr>)> {
    let vn = &cx.vtree.nodes[v];
    let Some(bone) = vn.blender_bone_name.clone() else {
        return vn.blender_object.map(|o| (o, None));
    };
    let is_armature = |o: IdRef<ObjectId>| {
        main.objects
            .get(o)
            .is_some_and(|x| x.object_type() == "ARMATURE")
    };
    let mut cur = Some(v);
    let mut depth = 0;
    while let Some(i) = cur {
        if let Some(o) = cx.vtree.nodes[i].blender_object.filter(|o| is_armature(*o)) {
            return Some((o, Some(bone)));
        }
        cur = cx.vtree.nodes[i].parent;
        depth += 1;
        if depth > cx.vtree.nodes.len() {
            break;
        }
    }
    None
}

/// `get_or_create_action_and_slot(gltf, vnode, anim, path)`.
fn action_and_slot(
    main: &mut Main,
    log: &mut ChangeLog,
    st: &mut AnimState,
    ob: IdRef<ObjectId>,
    key: bool,
) -> Option<(IdRef<ActionId>, i32)> {
    let ob_name = main.objects.get(ob)?.header.name.clone();
    let owner = if key {
        let me = main.objects.get(ob)?.data?.typed::<MeshId>()?;
        Owner::Key(main.meshes.get(me)?.shape_keys?)
    } else {
        Owner::Object(ob)
    };
    let action = match st.action {
        Some(a) => a,
        None => {
            // `bpy.data.actions.new(track_name)`, `layers.new('layer0')`,
            // `strips.new(type='KEYFRAME')`.
            let mut act = Action::new(st.track_name.as_str());
            let mut layer = ActionLayer::new("layer0");
            layer
                .strips
                .push(ActionStrip::Keyframe(KeyframeStrip::default()));
            act.layers.push(layer);
            let r = main.actions.insert(&st.track_name, act, log);
            st.action = Some(r);
            r
        }
    };
    if let Some(&h) = st.slots.get(&(ob_name.clone(), key)) {
        return Some((action, h));
    }
    let act = main.actions.get_mut(action, log)?;
    let handle = act.slot_add_for_id_type(if key { "KEY" } else { "OBJECT" }, &ob_name);
    if let Some(ActionStrip::Keyframe(k)) = act.layers.first_mut()?.strips.first_mut() {
        k.channelbag_ensure(handle);
    }
    st.slots.insert((ob_name, key), handle);
    st.needs_stash.push((owner, handle));
    Some((action, handle))
}

/// `make_fcurve(action, slot, co, data_path, index, group_name, interpolation)`.
#[allow(clippy::too_many_arguments)]
fn make_fcurve(
    main: &mut Main,
    log: &mut ChangeLog,
    action: IdRef<ActionId>,
    handle: i32,
    co: &[[f32; 2]],
    data_path: &str,
    index: i32,
    group: &str,
    interp: GInterp,
) {
    let Some(act) = main.actions.get_mut(action, log) else {
        return;
    };
    let Some(bag) = act.channelbag_mut(handle) else {
        return;
    };
    let group = (!group.is_empty()).then_some(group);
    // `fcurves.new` raises when the F-curve exists: the add-on skips it.
    let Some(i) = bag.fcurve_create_unique(data_path, index, group) else {
        return;
    };
    let fc = &mut bag.fcurves[i];
    keys::add_zeroed(fc, co.len());
    let ipo = match interp {
        GInterp::CubicSpline => Interpolation::Bezier,
        GInterp::Step => Interpolation::Constant,
        GInterp::Linear => Interpolation::Linear,
    };
    for (k, c) in fc.keyframe_points.iter_mut().zip(co) {
        k.co = *c;
        k.interpolation = ipo;
        if interp == GInterp::CubicSpline {
            k.handle_left_type = HandleType::Auto;
            k.handle_right_type = HandleType::Auto;
        }
    }
    // `fcurve.update()`.
    keys::update(fc);
}

/// A decoded sampler: key times, per-key values (each element at the output accessor's own
/// width), interpolation.
type Decoded = (Vec<f32>, Vec<Vec<f64>>, GInterp);

/// The add-on's Python failure on malformed animation data, as an import error naming the
/// animation and channel (Blender's operator fails with the traceback, verified on 5.2.4).
fn bad_channel(cx: &Gltf, ai: usize, ci: usize, what: &str) -> ImportError {
    let name = cx.doc.animations[ai].name.as_deref().unwrap_or("");
    ImportError::Data(format!("animation {ai} ('{name}') channel {ci}: {what}"))
}

/// The elements of accessor `i` at its own width (`BinaryData.get_data_from_accessor`).
fn elements(cx: &Gltf, i: u32) -> Result<Vec<Vec<f64>>, ImportError> {
    let a = cx.accessor(i)?;
    let n = a.type_.components().max(1);
    Ok(a.to_f32()
        .chunks(n)
        .map(|c| c.iter().map(|v| f64::from(*v)).collect())
        .collect())
}

/// Decodes the sampler of channel `ci`: key times and per-key values (CUBICSPLINE: the values
/// only, `values[1::3]`). A missing sampler is the add-on's `IndexError`.
fn sampler_data(cx: &Gltf, ai: usize, ci: usize) -> Result<Decoded, ImportError> {
    let anim = &cx.doc.animations[ai];
    let sampler = anim.channels[ci].sampler;
    let Some(s) = anim.samplers.get(sampler as usize) else {
        return Err(bad_channel(
            cx,
            ai,
            ci,
            &format!("sampler {sampler} does not exist (IndexError: list index out of range)"),
        ));
    };
    let times: Vec<f32> = elements(cx, s.input)?
        .iter()
        .map(|k| k.first().copied().unwrap_or(0.0) as f32)
        .collect();
    let mut vals = elements(cx, s.output)?;
    let interp = s.interpolation_or_default();
    if interp == GInterp::CubicSpline {
        vals = vals.into_iter().skip(1).step_by(3).collect();
    }
    Ok((times, vals, interp))
}

/// The edit-mode rest transform of `bone` relative to its parent bone (`editbone_trans`,
/// `editbone_rot`), from the armature the import created.
fn editbone_trs(main: &Main, ob: IdRef<ObjectId>, bone: &str) -> Option<([f32; 3], QuatWxyz)> {
    let arm = main
        .armatures
        .get(main.objects.get(ob)?.data?.typed::<ArmatureId>()?)?;
    let i = arm.bones.iter().position(|b| b.name == bone)?;
    let rest = rest_pose(arm);
    let local = match arm.bones[i].parent.filter(|p| *p < rest.len()) {
        Some(p) => crate::export::vtree::matmul(
            &crate::export::vtree::inverted_safe(&rest[p].arm_mat),
            &rest[i].arm_mat,
        ),
        None => rest[i].arm_mat,
    };
    let (t, q, _) = decompose_wxyz(&local);
    Some((t.to_array(), q))
}

/// `BlenderNodeAnim.do_channel`: one TRS channel of a node.
#[allow(clippy::too_many_arguments)]
fn node_channel(
    cx: &Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    st: &mut AnimState,
    ai: usize,
    v: usize,
    ci: usize,
    fps: f64,
) -> Result<(), ImportError> {
    let ch = &cx.doc.animations[ai].channels[ci];
    let (prop, width) = match channel_path(ch).as_deref() {
        Some("translation") => ("location", 3),
        Some("rotation") => ("rotation_quaternion", 4),
        Some("scale") => ("scale", 3),
        _ => return Ok(()),
    };
    let Some((ob, bone)) = animated_object(cx, main, v) else {
        return Ok(());
    };
    let Some((action, handle)) = action_and_slot(main, log, st, ob, false) else {
        return Ok(());
    };
    let (times, vals, interp) = sampler_data(cx, ai, ci)?;
    // `gltf.loc_gltf_to_blender(vals)` and the quaternion / scale conversions index the
    // element's components; `make_fcurve`'s `coords[1::2] = …` needs one value per key.
    if let Some(e) = vals.iter().find(|e| e.len() < width) {
        return Err(bad_channel(
            cx,
            ai,
            ci,
            &format!(
                "{} output elements have {} components, {width} needed (IndexError: list index out of range)",
                prop,
                e.len()
            ),
        ));
    }
    if vals.len() != times.len() {
        return Err(bad_channel(
            cx,
            ai,
            ci,
            &format!(
                "{} output values for {} keys (ValueError: attempt to assign sequence of size {} to extended slice of size {})",
                vals.len(),
                times.len(),
                vals.len(),
                times.len()
            ),
        ));
    }
    let vn = &cx.vtree.nodes[v];
    let (ra, rb) = (vn.rotation_after, vn.rotation_before);
    let mut values: Vec<Vec<f32>> = vals
        .iter()
        .map(|x| {
            let g = |i: usize| x[i];
            match width {
                4 if prop == "rotation_quaternion" => {
                    let q = quat([g(0), g(1), g(2), g(3)]);
                    qmul(qmul(ra, q), rb).to_vec()
                }
                _ if prop == "location" => qrot(ra, cx.units.loc([g(0), g(1), g(2)])).to_vec(),
                _ => scale_rot_swap(rb, scale([g(0), g(1), g(2)])).to_vec(),
            }
        })
        .collect();
    let mut group = "Object Transforms".to_owned();
    let mut path = prop.to_owned();
    if let Some(b) = &bone {
        group = b.to_string();
        path = format!("pose.bones[\"{}\"].{prop}", escape(b));
        if let Some((et, er)) = editbone_trs(main, ob, b) {
            let inv = qconj(er);
            match prop {
                "location" => {
                    for v in &mut values {
                        let d = [v[0] - et[0], v[1] - et[1], v[2] - et[2]];
                        *v = qrot(inv, d).to_vec();
                    }
                }
                "rotation_quaternion" => {
                    for v in &mut values {
                        *v = qmul(inv, [v[0], v[1], v[2], v[3]]).to_vec();
                    }
                }
                _ => {}
            }
        }
    }
    if prop == "rotation_quaternion" {
        // Shortest path between consecutive keys.
        for i in 1..values.len() {
            let d: f32 = (0..4).map(|k| values[i][k] * values[i - 1][k]).sum();
            if d < 0.0 {
                for c in &mut values[i] {
                    *c = -*c;
                }
            }
        }
    }
    let frames: Vec<f32> = times.iter().map(|t| (f64::from(*t) * fps) as f32).collect();
    for i in 0..width {
        let co: Vec<[f32; 2]> = frames
            .iter()
            .zip(&values)
            .map(|(f, v)| [*f, v.get(i).copied().unwrap_or(0.0)])
            .collect();
        make_fcurve(
            main, log, action, handle, &co, &path, i as i32, &group, interp,
        );
    }
    Ok(())
}

/// The glTF path a channel animates: its `target.path`, or for a `KHR_animation_pointer`
/// channel the node property of `/nodes/<i>/<path>` (`weights` for `/meshes/<i>/weights`).
fn channel_path(ch: &frap_gltf::json::animation::Channel) -> Option<String> {
    match ch.target.path {
        AnimationPath::Translation => Some("translation".into()),
        AnimationPath::Rotation => Some("rotation".into()),
        AnimationPath::Scale => Some("scale".into()),
        AnimationPath::Weights => Some("weights".into()),
        AnimationPath::Pointer => match pointer::dispatch(ch)? {
            pointer::Target::Node { path, .. } => Some(path),
            pointer::Target::MeshWeights { .. } => Some("weights".into()),
            _ => None,
        },
        AnimationPath::Other(_) => None,
    }
}

/// `BlenderWeightAnim.anim`: the `weights` channel of a mesh node into its shape keys.
#[allow(clippy::too_many_arguments)]
fn weight_channel(
    cx: &Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    st: &mut AnimState,
    ai: usize,
    v: usize,
    node_anims: &HashMap<(usize, usize), Vec<usize>>,
    sk_names: &[Vec<Option<String>>],
    fps: f64,
) -> Result<(), ImportError> {
    let vn = &cx.vtree.nodes[v];
    let Some(node_idx) = vn.mesh_node_idx else {
        return Ok(());
    };
    let Some(ob) = vn.blender_object else {
        return Ok(());
    };
    let Some(chans) = node_anims.get(&(node_idx, ai)) else {
        return Ok(());
    };
    let anim = &cx.doc.animations[ai];
    let Some(&ci) = chans
        .iter()
        .find(|&&c| channel_path(&anim.channels[c]).as_deref() == Some("weights"))
    else {
        return Ok(());
    };
    let Some(mesh) = cx.doc.nodes[node_idx].mesh else {
        return Ok(());
    };
    let names = sk_names.get(mesh as usize).cloned().unwrap_or_default();
    let Some((action, handle)) = action_and_slot(main, log, st, ob, true) else {
        return Ok(());
    };
    let (times, _, interp) = sampler_data(cx, ai, ci)?;
    let values = elements(cx, anim.samplers[anim.channels[ci].sampler as usize].output)?;
    let nb = names.len();
    let (offset, stride) = if interp == GInterp::CubicSpline {
        (nb, 3 * nb)
    } else {
        (0, nb)
    };
    let Some(key) = main
        .objects
        .get(ob)
        .and_then(|o| o.data?.typed::<MeshId>())
        .and_then(|m| main.meshes.get(m)?.shape_keys)
    else {
        return Ok(());
    };
    for (sk, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        // `values[offset + stride * i + sk][0]`.
        let mut co: Vec<[f32; 2]> = Vec::with_capacity(times.len());
        for (i, t) in times.iter().enumerate() {
            let Some(v) = values.get(offset + stride * i + sk).and_then(|e| e.first()) else {
                return Err(bad_channel(
                    cx,
                    ai,
                    ci,
                    &format!(
                        "weights output has {} values, too few for {} keys of {nb} targets (IndexError: list index out of range)",
                        values.len(),
                        times.len()
                    ),
                ));
            };
            co.push([(f64::from(*t) * fps) as f32, *v as f32]);
        }
        let path = format!("key_blocks[\"{}\"].value", escape(name));
        make_fcurve(main, log, action, handle, &co, &path, 0, "", interp);
        // The slider range grows to the first weight (the add-on reads `coords[1:2]`).
        if let Some(first) = co.first().map(|c| c[1])
            && let Some(k) = main.shape_keys.get_mut(key, log)
            && let Some(kb) = k.key_blocks.iter_mut().find(|b| b.name == *name)
        {
            if first < kb.slider_min {
                kb.set_slider_min(first);
            }
            if first > kb.slider_max {
                kb.set_slider_max(first);
            }
        }
    }
    Ok(())
}

/// `bpy.utils.escape_identifier`.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The `AnimData` of a slot owner, created when missing (`animation_data_create()`).
fn adt_mut<'m>(main: &'m mut Main, log: &mut ChangeLog, owner: Owner) -> Option<&'m mut AnimData> {
    let adt = match owner {
        Owner::Object(o) => &mut main.objects.get_mut(o, log)?.animation_data,
        Owner::Key(k) => &mut main.shape_keys.get_mut(k, log)?.animation_data,
    };
    Some(adt.get_or_insert_with(AnimData::default))
}

/// `simulate_stash(data, track_name, action, slot)`: a new top NLA track named `track_name`
/// holding the action at `scene.frame_start` over its whole range, muted and locked.
fn stash(
    main: &mut Main,
    log: &mut ChangeLog,
    owner: Owner,
    track_name: &str,
    action: IdRef<ActionId>,
    handle: i32,
    start: f32,
) {
    let (id_type, prefix, name) = match owner {
        Owner::Object(o) => (
            "OBJECT",
            "OB",
            main.objects.get(o).map(|x| x.header.name.clone()),
        ),
        Owner::Key(k) => (
            "KEY",
            "KE",
            main.shape_keys.get(k).map(|x| x.header.name.clone()),
        ),
    };
    let Some(name) = name else { return };
    let Some(act) = main.actions.get(action) else {
        return;
    };
    let action_name = act.header.name.clone();
    let action_range = act.frame_range_get();
    // `strips.new(action.name, start, action)`: the slot the strip auto-assigns gives the
    // initial range (`BKE_nlastrip_new`).
    let auto = act
        .slot_for_autoassign(id_type, &format!("{prefix}{name}"), "")
        .or_else(|| match act.slots.as_slice() {
            [only] if only.is_suitable_for(id_type) => Some(only.handle),
            _ => None,
        });
    let range = slot_frame_range(act, auto.unwrap_or(0));
    let mut strip = NlaStrip::new_clip("", Some(action), range[0], range);
    strip.use_sync_length = !act.use_frame_range;
    strip.use_animated_time_cyclic = act.use_cyclic && act.use_frame_range;
    strip.frame_end += start - strip.frame_start;
    strip.frame_start = start;
    // `strip.action_slot = slot`.
    strip.action_slot_handle = handle;
    strip.last_slot_identifier = act
        .slot(handle)
        .map(|s| s.identifier.clone())
        .unwrap_or_default();
    // `action_frame_start` / `action_frame_end` setters (`BKE_nlastrip_recalculate_bounds`).
    strip.action_frame_start = action_range[0].clamp(MINAFRAME, strip.action_frame_end);
    recalc_bounds(&mut strip);
    strip.action_frame_end = action_range[1].clamp(strip.action_frame_start, MAXFRAME);
    recalc_bounds(&mut strip);
    let Some(adt) = adt_mut(main, log, owner) else {
        return;
    };
    // `nla_tracks.new(prev=None)`: a new active track on top named `NlaTrack`, renamed.
    let default = unique_subname(
        &|n| adt.nla_tracks.iter().any(|t| t.name == n),
        "NlaTrack",
        63,
        "NlaTrack",
    );
    for t in &mut adt.nla_tracks {
        t.active = false;
    }
    adt.nla_tracks.push(NlaTrack::new(default));
    let t = adt.nla_tracks.len() - 1;
    let unique = unique_subname(
        &|n| {
            adt.nla_tracks
                .iter()
                .enumerate()
                .any(|(i, x)| i != t && x.name == n)
        },
        track_name,
        63,
        "NlaTrack",
    );
    adt.nla_tracks[t].name = unique;
    // The strip's name, unique among every strip of the animation data.
    let strip_name = unique_subname(
        &|n| {
            adt.nla_tracks
                .iter()
                .flat_map(|x| &x.strips)
                .any(|s| s.name == n)
        },
        &action_name,
        63,
        "NlaStrip",
    );
    strip.name = strip_name;
    let track = &mut adt.nla_tracks[t];
    track.strips.push(strip);
    track.lock = true;
    track.mute = true;
}

/// `BKE_nlastrip_recalculate_bounds` for a lone action clip.
fn recalc_bounds(s: &mut NlaStrip) {
    let len = if s.action_frame_end <= s.action_frame_start {
        1.0
    } else {
        s.action_frame_end - s.action_frame_start
    };
    let mapping = s.scale * s.repeat;
    if mapping.abs() > f32::EPSILON {
        s.frame_end = len * mapping + s.frame_start;
    }
}

/// `Action::get_frame_range_of_slot`.
fn slot_frame_range(act: &Action, handle: i32) -> [f32; 2] {
    if act.use_frame_range {
        return act.frame_range;
    }
    let mut r: Option<[f32; 2]> = None;
    for layer in &act.layers {
        for strip in &layer.strips {
            let Some(k) = strip.keyframe() else { continue };
            for bag in k.channelbags.iter().filter(|b| b.slot_handle == handle) {
                for fc in &bag.fcurves {
                    if let Some(x) = keys::range(fc) {
                        r = Some(match r {
                            None => x,
                            Some(y) => [y[0].min(x[0]), y[1].max(x[1])],
                        });
                    }
                }
            }
        }
    }
    r.map_or([0.0, 0.0], |r| [r[0].max(MINAFRAME), r[1].min(MAXFRAME)])
}

/// `restore_animation_on_object(data, name)`: the action and slot of the track `name`, or no
/// action.
fn restore_on(main: &mut Main, log: &mut ChangeLog, owner: Owner, name: &str) {
    let adt = match owner {
        Owner::Object(o) => main
            .objects
            .get_mut(o, log)
            .and_then(|x| x.animation_data.as_mut()),
        Owner::Key(k) => main
            .shape_keys
            .get_mut(k, log)
            .and_then(|x| x.animation_data.as_mut()),
    };
    let Some(adt) = adt else { return };
    let found = adt
        .nla_tracks
        .iter()
        .find(|t| t.name == name)
        .and_then(|t| t.strips.first())
        .map(|s| {
            (
                s.action,
                s.action_slot_handle,
                s.last_slot_identifier.clone(),
            )
        });
    match found {
        Some((action, handle, ident)) => {
            adt.action = action;
            adt.action_slot_handle = handle;
            adt.last_slot_identifier = ident;
        }
        None => {
            adt.action = None;
            adt.action_slot_handle = 0;
        }
    }
}

/// `BlenderAnimation.restore_animation(gltf, name)`: every imported object (an armature for
/// its bones) and its shape keys get the first animation's action. Returns the owners whose
/// animation data it touched.
fn restore_animation(cx: &Gltf, main: &mut Main, log: &mut ChangeLog, name: &str) -> Vec<Owner> {
    let mut owners = Vec::new();
    let mut done: HashSet<IdRef<ObjectId>> = HashSet::new();
    for v in 0..cx.vtree.nodes.len() {
        let Some((ob, _)) = animated_object(cx, main, v) else {
            continue;
        };
        if cx.vtree.nodes[v].kind != super::vnode::VKind::Object || !done.insert(ob) {
            continue;
        }
        restore_on(main, log, Owner::Object(ob), name);
        owners.push(Owner::Object(ob));
        let key = main
            .objects
            .get(ob)
            .and_then(|o| o.data?.typed::<MeshId>())
            .and_then(|m| main.meshes.get(m)?.shape_keys);
        if let Some(k) = key {
            restore_on(main, log, Owner::Key(k), name);
            owners.push(Owner::Key(k));
        }
    }
    owners
}

/// The animation of the restored owners at `scene.frame_current`, written into the originals.
///
/// In Blender the new objects and shape keys are tagged for a full depsgraph update and
/// `bpy.ops` updates the view layer after the FINISHED import, so the depsgraph evaluates the
/// restored action at the current frame and flushes it to the originals
/// (`animsys_write_orig_anim_rna`; verified on 5.2.4: AnimatedTriangle imported at
/// `frame_current=30` has `rotation_quaternion == (-1, 0, 0, 0)` right after the import, not
/// its rest rotation). The importer writes only these paths: the object's and the pose bones'
/// `location` / `rotation_quaternion` / `scale` and `key_blocks["…"].value` (clamped to the
/// slider range, as the RNA setter does).
fn apply_restored(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: frap_rna::IdRef<frap_rna::SceneId>,
    owners: &[Owner],
) {
    let Some(frame) = main.scenes.get(scene).map(|s| s.frame_current as f32) else {
        return;
    };
    for &owner in owners {
        let adt = match owner {
            Owner::Object(o) => main.objects.get(o).and_then(|x| x.animation_data.as_ref()),
            Owner::Key(k) => main
                .shape_keys
                .get(k)
                .and_then(|x| x.animation_data.as_ref()),
        };
        let Some(adt) = adt else { continue };
        if adt.action.is_none() && adt.nla_tracks.is_empty() {
            continue;
        }
        let channels =
            frap_anim::eval::evaluate_anim(adt, &frap_eval::animate::MainActions(main), frame);
        if channels.is_empty() {
            continue;
        }
        match owner {
            Owner::Object(o) => {
                let Some(ob) = main.objects.get_mut(o, log) else {
                    continue;
                };
                for c in &channels {
                    let (bone, prop) = match split_bone_path(&c.data_path) {
                        Some((b, p)) => (Some(b), p),
                        None => (None, c.data_path.to_string()),
                    };
                    let target: Option<&mut [f32]> = match &bone {
                        None => match prop.as_str() {
                            "location" => Some(&mut ob.location[..]),
                            "rotation_quaternion" => Some(&mut ob.rotation_quaternion[..]),
                            "scale" => Some(&mut ob.scale[..]),
                            _ => None,
                        },
                        Some(b) => ob
                            .pose
                            .as_mut()
                            .and_then(|p| p.bones.iter_mut().find(|pb| pb.name == *b))
                            .and_then(|pb| match prop.as_str() {
                                "location" => Some(&mut pb.location[..]),
                                "rotation_quaternion" => Some(&mut pb.rotation_quaternion[..]),
                                "scale" => Some(&mut pb.scale[..]),
                                _ => None,
                            }),
                    };
                    if let Some(slot) =
                        target.and_then(|t| t.get_mut(usize::try_from(c.array_index).ok()?))
                    {
                        *slot = c.value;
                    }
                }
            }
            Owner::Key(k) => {
                let Some(key) = main.shape_keys.get_mut(k, log) else {
                    continue;
                };
                for c in &channels {
                    let Some(name) = c
                        .data_path
                        .strip_prefix("key_blocks[\"")
                        .and_then(|r| r.strip_suffix("\"].value"))
                        .map(unescape)
                    else {
                        continue;
                    };
                    if let Some(kb) = key.key_blocks.iter_mut().find(|b| b.name == name) {
                        kb.set_value(c.value);
                    }
                }
            }
        }
    }
}

/// `pose.bones["name"].prop` → `(name, prop)`.
fn split_bone_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("pose.bones[\"")?;
    let mut name = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => name.push(chars.next()?.1),
            '"' => {
                let prop = rest[i + 1..].strip_prefix("].")?;
                return Some((name, prop.to_owned()));
            }
            c => name.push(c),
        }
    }
    None
}

/// Inverse of [`escape`].
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    out
}
