// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-scene animation export state: frame ranges (`export_settings['ranges']`), slides
//! (`export_settings['slide']`), the sampling cache (`get_cache_data`) and the sampled
//! object, bone and shape-key channels.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/cache.py` (`datacache`), `blender/exp/animation/sampled/sampling_cache.py`
//! (`get_cache_data`, `get_range`, `object_caching`, `armature_caching`),
//! `sampled/object/{channels,keyframes,sampler}.py`, `sampled/armature/{channels,keyframes,
//! sampler}.py`, `sampled/shapekeys/{channels,keyframes,sampler}.py` and
//! `anim_utils.py` (`add_slide_data`).
//!
//! The cache keeps the add-on's semantics: the first request sweeps the whole range of the
//! scene (`get_range`: every recorded range, or the track's in `NLA_TRACKS` mode) calling
//! `frame_set` and records every object of the tree under its *currently assigned* action and
//! slot (or under the object itself without one); a request for an object already cached
//! under another action sweeps again for that object only.

use std::collections::{BTreeMap, HashMap};

use frap_gltf::json::Index;
use frap_gltf::json::accessor::AccessorType;
use frap_gltf::json::animation::AnimationPath;
use frap_math::glam::Mat4;
use frap_math::matrix::decompose_wxyz;
use frap_rna::{ArmatureId, IdRef, ObjectId, SceneId};
use frap_scene::edit_armature::rest::rest_pose;
use indexmap::IndexMap;
use smol_str::SmolStr;

use super::keys::{Convert, Interp, Key, optimize, sampled_interpolation};
use super::world::{World, exported_shape_keys};
use super::{AnimSettings, GChannel, GSampler, Mode, SamplerOrigin};
use crate::export::node_map::NodeMap;
use crate::export::vtree::{VTree, inverted_safe, matmul};

/// What a range, a slide or a cache entry is recorded under: an action or NLA-track name, or
/// the object itself when it is baked without an action (`obj_uuid`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AKey {
    /// An action (or, in `NLA_TRACKS` mode, a track) name.
    Name(SmolStr),
    /// The object's own key.
    Obj(IdRef<ObjectId>),
}

/// One cached datum path (`'matrix'`, `'bone'`, `'sk'`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CPath {
    /// The object's matrix relative to its exported parent.
    Matrix,
    /// A bone's matrix relative to its exported parent bone.
    Bone(SmolStr),
    /// The values of the exported shape keys.
    Sk,
}

#[derive(Clone, Debug, Default)]
struct Entry {
    matrix: BTreeMap<i32, Mat4>,
    bones: HashMap<SmolStr, BTreeMap<i32, Mat4>>,
    sk: BTreeMap<i32, Vec<f32>>,
}

type ObjCache = IndexMap<AKey, IndexMap<Option<SmolStr>, Entry>>;

/// A cached value.
enum Datum {
    Mat(Mat4),
    Values(Vec<f32>),
}

/// The scene being gathered.
pub(crate) struct Exp<'a> {
    pub s: &'a AnimSettings,
    pub tree: &'a VTree,
    pub scene: IdRef<SceneId>,
    pub nodes: &'a NodeMap,
    /// `scene.render.fps * scene.render.fps_base` (the add-on's time base).
    pub fps: f64,
    /// `scene.frame_start`, `scene.frame_end`.
    pub frame_start: i32,
    pub frame_end: i32,
    /// `len(bpy.data.actions) != 0`.
    pub has_actions: bool,
    /// Nodes whose mesh has morph targets (a `weights` channel needs them).
    pub morph_nodes: &'a std::collections::HashSet<Index>,
    pub world: World,
    pub ranges: IndexMap<IdRef<ObjectId>, IndexMap<AKey, (i32, i32)>>,
    pub slide: HashMap<IdRef<ObjectId>, HashMap<AKey, i32>>,
    cache: HashMap<IdRef<ObjectId>, ObjCache>,
    /// WARNINGs (`export_settings['log'].warning`).
    pub warnings: Vec<String>,
}

impl<'a> Exp<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        s: &'a AnimSettings,
        tree: &'a VTree,
        scene: IdRef<SceneId>,
        nodes: &'a NodeMap,
        main: &frap_scene::Main,
        morph_nodes: &'a std::collections::HashSet<Index>,
    ) -> Exp<'a> {
        let sc = main.scenes.get(scene);
        let (fps, frame_start, frame_end) = sc.map_or((24.0, 1, 250), |sc| {
            (
                f64::from(sc.render.fps) * f64::from(sc.render.fps_base),
                sc.frame_start,
                sc.frame_end,
            )
        });
        Exp {
            s,
            tree,
            scene,
            nodes,
            fps,
            frame_start,
            frame_end,
            has_actions: !main.actions.is_empty(),
            morph_nodes,
            world: World::new(main),
            ranges: IndexMap::new(),
            slide: HashMap::new(),
            cache: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// A WARNING for the operator.
    pub(crate) fn warn(&mut self, m: impl Into<String>) {
        let m = m.into();
        if !self.warnings.contains(&m) {
            self.warnings.push(m);
        }
    }

    /// `vtree.get_all_objects()` that are exported (`node is not None`): (vtree index, object,
    /// glTF node), in construction order.
    ///
    /// Each object once: tree nodes that are not objects of their own (WP 6.8's bones) name
    /// their armature object, which is listed at its own node only.
    pub(crate) fn exported_objects(&self) -> Vec<(usize, IdRef<ObjectId>, Index)> {
        let mut seen = std::collections::HashSet::new();
        self.tree
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let node = self.nodes.object(self.scene, n.object)?;
                (n.keep && seen.insert(n.object)).then_some((i, n.object, node))
            })
            .collect()
    }

    /// The glTF node of object `ob`.
    pub(crate) fn node_of(&self, ob: IdRef<ObjectId>) -> Option<Index> {
        self.nodes.object(self.scene, ob)
    }

    /// The vtree index of `ob`.
    fn vtree_index(&self, ob: IdRef<ObjectId>) -> Option<usize> {
        self.tree.nodes.iter().position(|n| n.object == ob)
    }

    /// `need_rotation_correction` of the samplers: exported cameras and lights.
    pub(crate) fn needs_correction(&self, ob: IdRef<ObjectId>) -> bool {
        match self.world.ob(ob).map(|o| o.object_type()) {
            Some("CAMERA") => self.s.cameras,
            Some("LIGHT") => self.s.lights,
            _ => false,
        }
    }

    /// `seconds` of a frame (`frame / (fps * fps_base)`).
    pub(crate) fn seconds(&self, frame: f64) -> f64 {
        frame / self.fps
    }

    /// `add_slide_data(start_frame, obj, key)`.
    pub(crate) fn add_slide(&mut self, start: i32, ob: IdRef<ObjectId>, key: AKey) {
        self.slide.entry(ob).or_default().insert(key, start);
    }

    /// `get_range(obj, key)`.
    fn range(&self, ob: IdRef<ObjectId>, key: &AKey) -> Option<(i32, i32)> {
        if self.s.mode == Mode::NlaTracks {
            return self.ranges.get(&ob)?.get(key).copied();
        }
        let mut out: Option<(i32, i32)> = None;
        for r in self.ranges.values().flat_map(|m| m.values()) {
            out = Some(match out {
                None => *r,
                Some((a, b)) => (a.min(r.0), b.max(r.1)),
            });
        }
        out
    }

    /// `get_cache_data.reset_cache()`.
    pub(crate) fn reset_cache(&mut self) {
        self.cache.clear();
    }

    /// The (action, slot) key an object's matrix is cached under at the current state.
    fn object_key(
        &self,
        o: IdRef<ObjectId>,
        param: &AKey,
        slot: &Option<SmolStr>,
    ) -> (AKey, Option<SmolStr>) {
        let assigned = self.world.ob(o).and_then(|ob| {
            let adt = ob.animation_data.as_ref()?;
            let a = self.world.main.actions.get(adt.action?)?;
            let sl = a.slot(adt.action_slot_handle)?;
            Some((
                AKey::Name(a.header.name.clone()),
                Some(sl.identifier.clone()),
            ))
        });
        match (assigned, self.s.mode) {
            (Some(k), Mode::Actions | Mode::ActiveActions | Mode::Broadcast) => k,
            (_, Mode::NlaTracks) => (param.clone(), slot.clone()),
            _ => (AKey::Obj(o), None),
        }
    }

    /// `object_caching` + `armature_caching` of one object at the current frame.
    fn object_caching(
        &self,
        out: &mut HashMap<IdRef<ObjectId>, ObjCache>,
        o: IdRef<ObjectId>,
        param: &AKey,
        slot: &Option<SmolStr>,
        frame: i32,
    ) {
        let Some(vi) = self.vtree_index(o) else {
            return;
        };
        let parent = match self.tree.nodes[vi].parent {
            Some(p) => self.world.matrix_world(self.tree.nodes[p].object),
            None => Mat4::IDENTITY,
        };
        let mat = matmul(&inverted_safe(&parent), &self.world.matrix_world(o));
        let (k, sl) = self.object_key(o, param, slot);
        let oc = out.entry(o).or_default();
        oc.entry(k.clone())
            .or_default()
            .entry(sl.clone())
            .or_default()
            .matrix
            .insert(frame, mat);
        let Some(ob) = self.world.ob(o) else {
            return;
        };
        if ob.object_type() == "ARMATURE" {
            // `armature_caching`: the assigned action's key needs `animation_data`.
            let has_adt = ob.animation_data.is_some();
            let (bk, bsl) = match self.s.mode {
                Mode::NlaTracks if has_adt => (param.clone(), slot.clone()),
                Mode::NlaTracks => (AKey::Obj(o), None),
                _ => (k, sl),
            };
            for (name, m) in self.bone_matrices(o) {
                oc.entry(bk.clone())
                    .or_default()
                    .entry(bsl.clone())
                    .or_default()
                    .bones
                    .entry(name)
                    .or_default()
                    .insert(frame, m);
            }
        }
        if self.s.morph_anim && ob.object_type() == "MESH" {
            let Some(key) = self.world.key(o) else {
                return;
            };
            let kk = match &key.animation_data {
                Some(adt) => {
                    let assigned = adt.action.and_then(|a| {
                        let a = self.world.main.actions.get(a)?;
                        let sl = a.slot(adt.action_slot_handle)?;
                        Some((
                            AKey::Name(a.header.name.clone()),
                            Some(sl.identifier.clone()),
                        ))
                    });
                    match (assigned, self.s.mode) {
                        (Some(k), Mode::Actions | Mode::ActiveActions | Mode::Broadcast) => k,
                        (_, Mode::NlaTracks) => (param.clone(), slot.clone()),
                        _ => (AKey::Obj(o), None),
                    }
                }
                None => (AKey::Obj(o), None),
            };
            let values: Vec<f32> = exported_shape_keys(key)
                .into_iter()
                .map(|i| key.key_blocks[i].value)
                .collect();
            oc.entry(kk.0)
                .or_default()
                .entry(kk.1)
                .or_default()
                .sk
                .insert(frame, values);
        }
    }

    /// The exported bones of armature object `o`, parents first: (bone index, name, nearest
    /// exported ancestor bone index).
    pub(crate) fn exported_bones(
        &self,
        o: IdRef<ObjectId>,
    ) -> Vec<(usize, SmolStr, Option<usize>)> {
        let Some(arm) = self
            .world
            .ob(o)
            .and_then(|ob| ob.data?.typed::<ArmatureId>())
            .and_then(|a| self.world.main.armatures.get(a))
        else {
            return Vec::new();
        };
        let exported = |i: usize| self.nodes.bone(self.scene, o, &arm.bones[i].name).is_some();
        let mut out = Vec::new();
        fn visit(
            arm: &frap_scene::id::Armature,
            i: usize,
            anc: Option<usize>,
            exported: &dyn Fn(usize) -> bool,
            out: &mut Vec<(usize, SmolStr, Option<usize>)>,
            depth: usize,
        ) {
            if depth > arm.bones.len() {
                return;
            }
            let here = exported(i);
            if here {
                out.push((i, arm.bones[i].name.clone(), anc));
            }
            let next = if here { Some(i) } else { anc };
            for (c, b) in arm.bones.iter().enumerate() {
                if b.parent == Some(i) && c != i {
                    visit(arm, c, next, exported, out, depth + 1);
                }
            }
        }
        for (i, b) in arm.bones.iter().enumerate() {
            if b.parent.is_none_or(|p| p >= arm.bones.len()) {
                visit(arm, i, None, &exported, &mut out, 0);
            }
        }
        out
    }

    /// `armature_caching`'s matrices of every exported bone at the current state.
    fn bone_matrices(&self, o: IdRef<ObjectId>) -> Vec<(SmolStr, Mat4)> {
        let Some(arm) = self
            .world
            .ob(o)
            .and_then(|ob| ob.data?.typed::<ArmatureId>())
            .and_then(|a| self.world.main.armatures.get(a))
        else {
            return Vec::new();
        };
        let Some(pose) = self.world.pose(o) else {
            return Vec::new();
        };
        let rest = rest_pose(arm);
        let pm = |i: usize| pose.bones.get(i).map_or(Mat4::IDENTITY, |b| b.1);
        self.exported_bones(o)
            .into_iter()
            .map(|(i, name, anc)| {
                let m = match anc {
                    Some(p) => {
                        let rest_mat = matmul(&inverted_safe(&rest[p].arm_mat), &rest[i].arm_mat);
                        matmul(
                            &matmul(&inverted_safe(&rest_mat), &inverted_safe(&pm(p))),
                            &pm(i),
                        )
                    }
                    None if arm.bones[i].parent.is_none() => {
                        matmul(&inverted_safe(&rest[i].arm_mat), &pm(i))
                    }
                    None => pm(i),
                };
                (name, m)
            })
            .collect()
    }

    /// The sweep of `get_cache_data`: `frame_set` over the range, recording every object (or
    /// only `ob`).
    fn sweep(
        &mut self,
        ob: IdRef<ObjectId>,
        key: &AKey,
        slot: &Option<SmolStr>,
        only: bool,
    ) -> HashMap<IdRef<ObjectId>, ObjCache> {
        let mut out = HashMap::new();
        let Some((min, max)) = self.range(ob, key) else {
            return out;
        };
        let objs: Vec<IdRef<ObjectId>> = if only || self.s.mode == Mode::NlaTracks {
            vec![ob]
        } else {
            let mut v: Vec<IdRef<ObjectId>> = Vec::new();
            for n in &self.tree.nodes {
                if !v.contains(&n.object) {
                    v.push(n.object);
                }
            }
            v
        };
        let step = self.s.frame_step.max(1);
        let mut frame = min;
        while frame <= max {
            // `--stmt-timeout` cancellation point per sampled frame and object (§12).
            frap_math::budget::check_now();
            self.world.frame_set(frame);
            for &o in &objs {
                frap_math::budget::check();
                self.object_caching(&mut out, o, key, slot, frame);
            }
            frame += step;
        }
        out
    }

    /// `get_cache_data(path, obj, bone, action_name, frame, step, slot)` with `datacache`'s
    /// lookup rules.
    fn get(
        &mut self,
        path: &CPath,
        ob: IdRef<ObjectId>,
        key: &AKey,
        slot: &Option<SmolStr>,
        frame: i32,
    ) -> Option<Datum> {
        if !self.cache.contains_key(&ob) {
            self.cache = self.sweep(ob, key, slot, false);
        } else if !self.cache[&ob].contains_key(key) {
            let r = self.sweep(ob, key, slot, true);
            self.cache.extend(r);
        } else if !self.cache[&ob][key].contains_key(slot) {
            if slot.is_none() && *key == AKey::Obj(ob) {
                // Baking: use the data of any slot already cached for this key.
                let e = self.cache[&ob][key].values().next()?;
                return lookup(e, path, frame);
            }
            let r = self.sweep(ob, key, slot, true);
            self.cache.extend(r);
        }
        let e = self.cache.get(&ob)?.get(key)?.get(slot)?;
        lookup(e, path, frame)
    }

    /// The frames of a sampled channel (`start..=end` by `export_frame_step`).
    fn frames(&self, ob: IdRef<ObjectId>, key: &AKey) -> Vec<i32> {
        let Some(&(start, end)) = self.ranges.get(&ob).and_then(|r| r.get(key)) else {
            return Vec::new();
        };
        let step = self.s.frame_step.max(1);
        let mut out = Vec::new();
        let mut f = start;
        while f <= end {
            out.push(f);
            f += step;
        }
        out
    }

    /// The sampler of `keys` (`__convert_keyframes` of the sampled samplers): slid times,
    /// converted values.
    fn sampler(
        &self,
        ob: IdRef<ObjectId>,
        key: &AKey,
        mut keys: Vec<Key>,
        conv: Convert<'_>,
        ty: AccessorType,
        interp: Interp,
    ) -> GSampler {
        if let Some(sl) = self.slide.get(&ob).and_then(|m| m.get(key)) {
            for k in &mut keys {
                k.frame -= f64::from(*sl);
            }
        }
        let times: Vec<f64> = keys.iter().map(|k| self.seconds(k.frame)).collect();
        GSampler::new(times, conv.output(&keys), ty, interp)
    }

    /// `gather_object_sampled_channels(obj, action_name, slot)`: the TRS channels of an
    /// object; `animated` maps the keyed channels (`location`, `rotation_quaternion`,
    /// `scale`) to their interpolation.
    pub(crate) fn object_channels(
        &mut self,
        ob: IdRef<ObjectId>,
        key: &AKey,
        slot: &Option<SmolStr>,
        animated: &HashMap<&'static str, Interp>,
    ) -> Vec<GChannel> {
        let mut out = Vec::new();
        for p in ["location", "rotation_quaternion", "scale"] {
            let interp = animated.get(p).copied();
            if let Some(c) = self.object_channel(ob, p, key, slot, interp) {
                out.push(c);
            }
        }
        out
    }

    /// `gather_sampled_object_channel(obj, channel, action, slot, animated, interpolation)`;
    /// `interp` is `Some` for a keyed channel.
    pub(crate) fn object_channel(
        &mut self,
        ob: IdRef<ObjectId>,
        channel: &'static str,
        key: &AKey,
        slot: &Option<SmolStr>,
        interp: Option<Interp>,
    ) -> Option<GChannel> {
        let node = self.node_of(ob)?;
        let animated = interp.is_some();
        let interp = interp.unwrap_or(self.s.fallback);
        let mut keys = Vec::new();
        for f in self.frames(ob, key) {
            frap_math::budget::check();
            let Some(Datum::Mat(m)) = self.get(&CPath::Matrix, ob, key, slot, f) else {
                continue;
            };
            keys.push(Key::new(f64::from(f), decomposed(&m, channel)));
        }
        let keys = optimize(keys, self.s.optimize, animated, self.s.keep_object)?;
        let conv = Convert {
            target: channel,
            matrix: Mat4::IDENTITY,
            correction: self.needs_correction(ob),
            yup: self.s.yup,
            fps: self.fps,
        };
        let interp = sampled_interpolation(animated, interp, &keys, self.s.fallback);
        let ty = if channel == "rotation_quaternion" {
            AccessorType::Vec4
        } else {
            AccessorType::Vec3
        };
        let sampler = self
            .sampler(ob, key, keys, conv, ty, interp)
            .built_by(SamplerOrigin::Object);
        Some(GChannel {
            node,
            path: trs_path(channel),
            sampler,
        })
    }

    /// `gather_sampled_bone_channel(armature, bone, channel, action, slot, animated,
    /// interpolation)`.
    pub(crate) fn bone_channel(
        &mut self,
        ob: IdRef<ObjectId>,
        bone: &SmolStr,
        channel: &'static str,
        key: &AKey,
        slot: &Option<SmolStr>,
        interp: Option<Interp>,
    ) -> Option<GChannel> {
        let node = self.nodes.bone(self.scene, ob, bone)?;
        let animated = interp.is_some();
        let interp = interp.unwrap_or(self.s.fallback);
        let mut keys = Vec::new();
        for f in self.frames(ob, key) {
            frap_math::budget::check();
            let Some(Datum::Mat(m)) = self.get(&CPath::Bone(bone.clone()), ob, key, slot, f) else {
                continue;
            };
            keys.push(Key::new(f64::from(f), decomposed(&m, channel)));
        }
        let keys = optimize(keys, self.s.optimize, animated, self.s.keep_armature)?;
        let conv = Convert {
            target: channel,
            matrix: self.bone_correction(ob, bone),
            correction: false,
            yup: false,
            fps: self.fps,
        };
        let interp = sampled_interpolation(animated, interp, &keys, self.s.fallback);
        let ty = if channel == "rotation_quaternion" {
            AccessorType::Vec4
        } else {
            AccessorType::Vec3
        };
        let sampler = self
            .sampler(ob, key, keys, conv, ty, interp)
            .built_by(SamplerOrigin::Armature);
        Some(GChannel {
            node,
            path: trs_path(channel),
            sampler,
        })
    }

    /// `correction_matrix_local` of a bone's samplers: the axis change times the rest matrix
    /// for a root bone, the rest matrix relative to the exported parent bone otherwise.
    pub(crate) fn bone_correction(&self, ob: IdRef<ObjectId>, bone: &str) -> Mat4 {
        let axis = if self.s.yup {
            Mat4::from_cols_array_2d(&[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        } else {
            Mat4::IDENTITY
        };
        let Some(arm) = self
            .world
            .ob(ob)
            .and_then(|o| o.data?.typed::<ArmatureId>())
            .and_then(|a| self.world.main.armatures.get(a))
        else {
            return axis;
        };
        let Some(i) = arm.bones.iter().position(|b| b.name == bone) else {
            return axis;
        };
        let rest = rest_pose(arm);
        if arm.bones[i].parent.is_none() {
            return matmul(&axis, &rest[i].arm_mat);
        }
        let anc = self
            .exported_bones(ob)
            .into_iter()
            .find(|(j, _, _)| *j == i)
            .and_then(|(_, _, a)| a);
        match anc {
            Some(p) => matmul(&inverted_safe(&rest[p].arm_mat), &rest[i].arm_mat),
            None => axis,
        }
    }

    /// `gather_sampled_sk_channel(obj, action, slot)`: the `weights` channel.
    pub(crate) fn sk_channel(
        &mut self,
        ob: IdRef<ObjectId>,
        key: &AKey,
        slot: &Option<SmolStr>,
    ) -> Option<GChannel> {
        let node = self.node_of(ob)?;
        if !self.morph_nodes.contains(&node) {
            return None;
        }
        let mut keys = Vec::new();
        for f in self.frames(ob, key) {
            frap_math::budget::check();
            let Some(Datum::Values(v)) = self.get(&CPath::Sk, ob, key, slot, f) else {
                continue;
            };
            keys.push(Key::new(f64::from(f), v));
        }
        if keys.is_empty() || keys.iter().any(|k| k.value.is_empty()) {
            return None;
        }
        let keys = optimize(keys, self.s.optimize, true, true)?;
        let conv = Convert {
            target: "value",
            matrix: Mat4::IDENTITY,
            correction: false,
            yup: false,
            fps: self.fps,
        };
        let interp = self.s.fallback;
        let sampler = self
            .sampler(ob, key, keys, conv, AccessorType::Scalar, interp)
            .built_by(SamplerOrigin::ShapeKeys);
        Some(GChannel {
            node,
            path: AnimationPath::Weights,
            sampler,
        })
    }

    /// `gather_armature_sampled_channels(armature, action, slot)`: every exported bone's TRS,
    /// then the armature object's own channels. `bone_animated` maps (bone, channel) to the
    /// keyed interpolation, `object_animated` the armature's own keyed channels (`None`: bake
    /// all three).
    pub(crate) fn armature_channels(
        &mut self,
        ob: IdRef<ObjectId>,
        key: &AKey,
        slot: &Option<SmolStr>,
        bone_animated: &HashMap<(SmolStr, &'static str), Interp>,
        object_animated: Option<&[(&'static str, Interp)]>,
    ) -> Vec<GChannel> {
        let mut out = Vec::new();
        for (_, bone, _) in self.exported_bones(ob) {
            for p in ["location", "rotation_quaternion", "scale"] {
                let interp = bone_animated.get(&(bone.clone(), p)).copied();
                if let Some(c) = self.bone_channel(ob, &bone, p, key, slot, interp) {
                    out.push(c);
                }
            }
        }
        match object_animated {
            None => {
                for p in ["location", "rotation_quaternion", "scale"] {
                    if let Some(c) = self.object_channel(ob, p, key, slot, None) {
                        out.push(c);
                    }
                }
            }
            Some(list) => {
                // A property keyed twice (`location` and `delta_location`) is one channel (the
                // add-on's `@cached` returns the same channel object for both).
                let mut seen: Vec<&str> = Vec::new();
                for &(p, i) in list {
                    if seen.contains(&p) {
                        continue;
                    }
                    seen.push(p);
                    if let Some(c) = self.object_channel(ob, p, key, slot, Some(i)) {
                        out.push(c);
                    }
                }
            }
        }
        out
    }
}

fn lookup(e: &Entry, path: &CPath, frame: i32) -> Option<Datum> {
    match path {
        CPath::Matrix => e.matrix.get(&frame).copied().map(Datum::Mat),
        CPath::Bone(b) => e.bones.get(b)?.get(&frame).copied().map(Datum::Mat),
        CPath::Sk => e.sk.get(&frame).cloned().map(Datum::Values),
    }
}

/// `mat.decompose()` component of a channel (Blender order: quaternion w first).
fn decomposed(m: &Mat4, channel: &str) -> Vec<f32> {
    let (t, q, s) = decompose_wxyz(m);
    match channel {
        "location" => t.to_array().to_vec(),
        "rotation_quaternion" => q.to_vec(),
        _ => s.to_array().to_vec(),
    }
}

/// The glTF path of a TRS channel.
pub(crate) fn trs_path(channel: &str) -> AnimationPath {
    match channel {
        "location" => AnimationPath::Translation,
        "rotation_quaternion" => AnimationPath::Rotation,
        _ => AnimationPath::Scale,
    }
}
