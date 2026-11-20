// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the animation exporter: the settings' coupling rules, bone and shape-key
//! channels (their nodes come from WP 6.8 at integration, so the tests record them in the
//! `NodeMap` and add the morph targets themselves), track merging and the `fps_base`
//! WARNING. The Blender comparisons are the `tests/golden/cases/gltf/animation/` goldens.

use frap_anim::{
    Action, ActionLayer, ActionStrip, AnimData, Channelbag, FCurve, Interpolation, Keyframe,
    KeyframeStrip,
};
use frap_gltf::accessor::read_accessor;
use frap_gltf::json::animation::{AnimationPath, Interpolation as GInterp};
use frap_gltf::json::{Index, Node};
use frap_rna::{ActionId, IdRef, ObjectId, OpArgs, OpProps, Value};
use frap_scene::id::{Armature, Bone, Key, Object, ShapeKey};
use frap_scene::{Base, BaseFlag, ChangeLog, Context, Main, Preferences, WindowState, startup};
use frap_spec_ops_io::export_scene::ExportSceneGltf;

use super::*;
use crate::export::gather::Gather;
use crate::export::node_map::NodeMap;
use crate::export::{ExportSettings, scenes, write};

struct Scene {
    main: Main,
    win: WindowState,
    prefs: Preferences,
    log: ChangeLog,
}

impl Scene {
    /// The factory scene without its objects.
    fn empty() -> Scene {
        let (main, win) = startup::factory();
        let mut s = Scene {
            main,
            win,
            prefs: Preferences::default(),
            log: ChangeLog::default(),
        };
        let coll = s.main.collections.find("Collection").unwrap();
        s.main
            .collections
            .get_mut(coll, &mut s.log)
            .unwrap()
            .objects
            .clear();
        let sc = s.win.scene;
        s.main.scenes.get_mut(sc, &mut s.log).unwrap().view_layers[0]
            .bases
            .clear();
        s
    }

    fn link(&mut self, ob: Object, name: &str) -> IdRef<ObjectId> {
        let r = self.main.objects.insert(name, ob, &mut self.log);
        let coll = self.main.collections.find("Collection").unwrap();
        self.main
            .collections
            .get_mut(coll, &mut self.log)
            .unwrap()
            .objects
            .push(r);
        let sc = self.win.scene;
        self.main
            .scenes
            .get_mut(sc, &mut self.log)
            .unwrap()
            .view_layers[0]
            .bases
            .push(Base {
                object: r,
                flag: BaseFlag::empty(),
            });
        r
    }

    /// An action with one slot `identifier` whose F-curves are `curves`.
    fn action(
        &mut self,
        name: &str,
        identifier: &str,
        ty: &str,
        curves: Vec<FCurve>,
    ) -> (IdRef<ActionId>, i32) {
        let mut a = Action::new(name);
        let prefix_type = ty;
        let h = a.slot_add_for_id_type(prefix_type, identifier.get(2..).unwrap_or(""));
        let mut layer = ActionLayer::new("Layer");
        layer.strips.push(ActionStrip::Keyframe(KeyframeStrip {
            channelbags: vec![Channelbag {
                slot_handle: h,
                groups: Vec::new(),
                fcurves: curves,
            }],
        }));
        a.layers.push(layer);
        (self.main.actions.insert(name, a, &mut self.log), h)
    }

    fn assign(&mut self, ob: IdRef<ObjectId>, a: (IdRef<ActionId>, i32)) {
        let o = self.main.objects.get_mut(ob, &mut self.log).unwrap();
        let adt = o.animation_data.get_or_insert_with(AnimData::default);
        adt.action = Some(a.0);
        adt.action_slot_handle = a.1;
    }
}

/// An F-curve with LINEAR keys.
fn curve(path: &str, index: i32, keys: &[(f32, f32)], ipo: Interpolation) -> FCurve {
    let mut fc = FCurve::new(path, index);
    for &(f, v) in keys {
        let mut k = Keyframe::new(f, v);
        k.interpolation = ipo;
        fc.keyframe_points.push(k);
    }
    frap_anim::eval::recalc_handles(&mut fc);
    fc
}

fn settings_with(f: impl FnOnce(&mut OpArgs)) -> ExportSettings {
    let mut a = OpArgs::default();
    for def in ExportSceneGltf::spec().props {
        a.values.insert(def.identifier, Value::default_for(def));
    }
    f(&mut a);
    ExportSettings::from_props(&ExportSceneGltf::from_args(&a))
}

fn set(a: &mut OpArgs, k: &'static str, v: Value) {
    let i = a.values.get_index_of(k).unwrap();
    a.values.insert(k, v);
    a.is_set.insert(i);
}

/// Exports `s`; `prep` runs between the scenes and the animations (WP 6.8's bone nodes and
/// morph targets are recorded there).
fn export_with(
    s: &Scene,
    settings: &ExportSettings,
    prep: impl FnOnce(&mut Gather),
) -> (frap_gltf::Document, Vec<Vec<u8>>, Vec<String>) {
    let ctx = Context {
        main: &s.main,
        win: &s.win,
        prefs: &s.prefs,
    };
    let mut g = Gather::new(&s.main, &ctx, settings);
    scenes::gather(&mut g).unwrap();
    prep(&mut g);
    gather(&mut g).unwrap();
    let out = write::finish(g).unwrap();
    let glb = frap_gltf::read(&out.bytes).unwrap();
    let buffers = vec![glb.bin.unwrap_or_default()];
    let warnings = out.reports.iter().map(|r| r.message.clone()).collect();
    (glb.document, buffers, warnings)
}

fn floats(doc: &frap_gltf::Document, bufs: &[Vec<u8>], i: Index) -> Vec<f32> {
    read_accessor(doc, bufs, i).unwrap().to_f32()
}

#[test]
fn settings_follow_the_addon_coupling_rules() {
    let s = AnimSettings::from(&settings_with(|a| {
        set(a, "export_animation_mode", Value::Str("NLA_TRACKS".into()));
        set(a, "export_force_sampling", Value::Bool(false));
        set(a, "export_merge_animation", Value::Str("ACTION".into()));
    }));
    assert!(s.force_sampling, "NLA_TRACKS forces sampling");
    assert_eq!(
        s.merge,
        Merge::NlaTrack,
        "modes other than ACTIONS merge by track"
    );
    let s = AnimSettings::from(&settings_with(|a| {
        set(a, "export_force_sampling", Value::Bool(false));
        set(a, "export_bake_animation", Value::Bool(true));
    }));
    assert!(!s.bake, "export_force_sampling=False turns baking off");
    let s = AnimSettings::from(&settings_with(|a| {
        set(a, "export_animations", Value::Bool(false));
        set(a, "export_optimize_animation_size", Value::Bool(true));
    }));
    assert!(!s.animations && !s.optimize && !s.force_sampling);
    let s = AnimSettings::from(&settings_with(|a| {
        set(a, "export_morph", Value::Bool(false));
    }));
    assert!(!s.morph_anim, "export_morph=False disables morph animation");
    let s = AnimSettings::from(&settings_with(|_| {}));
    assert!(!s.split_object && s.mode == Mode::Actions && s.merge == Merge::Action);
    for p in PROPS {
        assert!(
            ExportSceneGltf::spec()
                .props
                .iter()
                .any(|d| d.identifier == *p),
            "{p} is not an export_scene.gltf prop"
        );
        assert!(crate::export::is_supported(p));
    }
}

#[test]
fn animations_off_writes_none() {
    let mut s = Scene::empty();
    let ob = s.link(Object::new("", None), "E");
    let a = s.action(
        "A",
        "OBE",
        "OBJECT",
        vec![curve(
            "location",
            0,
            &[(1.0, 0.0), (5.0, 2.0)],
            Interpolation::Linear,
        )],
    );
    s.assign(ob, a);
    let st = settings_with(|a| set(a, "export_animations", Value::Bool(false)));
    let (doc, _, _) = export_with(&s, &st, |_| {});
    assert!(doc.animations.is_empty());
    let (doc, bufs, _) = export_with(&s, &settings_with(|_| {}), |_| {});
    assert_eq!(doc.animations.len(), 1);
    let ch = &doc.animations[0].channels[0];
    assert_eq!(ch.target.path, AnimationPath::Translation);
    let smp = &doc.animations[0].samplers[ch.sampler as usize];
    let t = floats(&doc, &bufs, smp.input);
    assert_eq!(t.len(), 5);
    assert!((t[0] - 1.0 / 24.0).abs() < 1e-7 && (t[4] - 5.0 / 24.0).abs() < 1e-7);
    let acc = &doc.accessors[smp.input as usize];
    assert!(
        acc.min.is_some() && acc.max.is_some(),
        "inputs carry min/max"
    );
    let v = floats(&doc, &bufs, smp.output);
    assert!((v[12] - 2.0).abs() < 1e-6 && (v[6] - 1.0).abs() < 1e-6);
}

#[test]
fn fps_base_quirk_and_warning() {
    let mut s = Scene::empty();
    let sc = s.win.scene;
    {
        let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
        scene.render.fps = 30;
        scene.render.fps_base = 1.001;
    }
    let ob = s.link(Object::new("", None), "E");
    let a = s.action(
        "A",
        "OBE",
        "OBJECT",
        vec![curve(
            "location",
            2,
            &[(0.0, 0.0), (30.0, 1.0)],
            Interpolation::Linear,
        )],
    );
    s.assign(ob, a);
    let (doc, bufs, warnings) = export_with(&s, &settings_with(|_| {}), |_| {});
    let smp = &doc.animations[0].samplers[0];
    let t = floats(&doc, &bufs, smp.input);
    // frame / (fps * fps_base), not frame * fps_base / fps.
    let expected = (30.0f64 / (30.0 * f64::from(1.001f32))) as f32;
    assert_eq!(*t.last().unwrap(), expected);
    assert!(warnings.iter().any(|w| w.contains("fps_base")));
}

/// A one-bone armature object (`Bone` from the origin to +Z) with `curves` on its slot.
fn armature_scene(curves: Vec<FCurve>) -> (Scene, IdRef<ObjectId>) {
    let mut s = Scene::empty();
    let mut arm = Armature::new("Armature");
    arm.bones.push(Bone::new("Bone"));
    let ar = s.main.armatures.insert("Armature", arm, &mut s.log);
    let ob = s.link(Object::new("", Some(ar.any())), "Armature");
    let a = s.action("ArmAction", "OBArmature", "OBJECT", curves);
    s.assign(ob, a);
    (s, ob)
}

/// Records a joint node for `Bone` of `ob` (WP 6.8's `NodeMap::bones`).
fn add_bone_node(g: &mut Gather, ob: IdRef<ObjectId>) -> Index {
    g.doc.nodes.push(Node {
        name: Some("Bone".into()),
        ..Node::default()
    });
    let i = (g.doc.nodes.len() - 1) as Index;
    let scene = g.scene;
    g.state::<NodeMap>()
        .bones
        .insert((scene, ob, "Bone".into()), i);
    i
}

#[test]
fn bone_channels_are_sampled_per_exported_bone() {
    let (s, ob) = armature_scene(vec![curve(
        "pose.bones[\"Bone\"].location",
        0,
        &[(1.0, 0.0), (3.0, 1.0)],
        Interpolation::Linear,
    )]);
    let mut joint = 0;
    let (doc, bufs, _) = export_with(&s, &settings_with(|_| {}), |g| joint = add_bone_node(g, ob));
    assert_eq!(doc.animations.len(), 1);
    let a = &doc.animations[0];
    assert_eq!(a.name.as_deref(), Some("ArmAction"));
    let on_joint: Vec<_> = a
        .channels
        .iter()
        .filter(|c| c.target.node == Some(joint))
        .collect();
    // The keyed translation, and (export_optimize_animation_keep_anim_armature) the constant
    // rotation and scale as two STEP keys; the armature object's own TRS is not keyed.
    assert_eq!(on_joint.len(), 3);
    for c in &on_joint {
        let smp = &a.samplers[c.sampler as usize];
        let n = floats(&doc, &bufs, smp.input).len();
        match c.target.path {
            AnimationPath::Translation => {
                assert_eq!((n, smp.interpolation), (3, Some(GInterp::Linear)));
                let v = floats(&doc, &bufs, smp.output);
                // A unit move along the bone's local X stays a unit move.
                let last = &v[6..9];
                let len = last.iter().map(|x| x * x).sum::<f32>().sqrt();
                assert!((len - 1.0).abs() < 1e-5, "{last:?}");
                assert!(v[0..3].iter().all(|x| x.abs() < 1e-6));
            }
            _ => assert_eq!((n, smp.interpolation), (2, Some(GInterp::Step))),
        }
    }
    assert_eq!(a.channels.len(), 3);
    // Without the joint node (WP 6.8 not recording it) no bone channel is written.
    let (doc, _, _) = export_with(&s, &settings_with(|_| {}), |_| {});
    assert!(doc.animations.is_empty());
}

#[test]
fn bone_curves_export_as_cubicspline_on_the_joint() {
    let (s, ob) = armature_scene(vec![
        curve(
            "pose.bones[\"Bone\"].location",
            0,
            &[(1.0, 0.0), (5.0, 1.0)],
            Interpolation::Bezier,
        ),
        curve(
            "pose.bones[\"Bone\"].location",
            1,
            &[(1.0, 0.0), (5.0, 0.0)],
            Interpolation::Bezier,
        ),
        curve(
            "pose.bones[\"Bone\"].location",
            2,
            &[(1.0, 0.0), (5.0, 0.0)],
            Interpolation::Bezier,
        ),
    ]);
    let st = settings_with(|a| set(a, "export_force_sampling", Value::Bool(false)));
    let mut joint = 0;
    let (doc, bufs, _) = export_with(&s, &st, |g| joint = add_bone_node(g, ob));
    let a = &doc.animations[0];
    assert_eq!(a.channels.len(), 1);
    let c = &a.channels[0];
    assert_eq!(
        (c.target.node, c.target.path.clone()),
        (Some(joint), AnimationPath::Translation)
    );
    let smp = &a.samplers[c.sampler as usize];
    assert_eq!(smp.interpolation, Some(GInterp::CubicSpline));
    let v = floats(&doc, &bufs, smp.output);
    // 2 keys × (in-tangent, value, out-tangent) × VEC3; the first in- and last out-tangent
    // are zero.
    assert_eq!(v.len(), 18);
    assert!(v[0..3].iter().all(|x| x.abs() < 1e-6) && v[15..18].iter().all(|x| x.abs() < 1e-6));
}

#[test]
fn shape_key_weights_need_morph_targets() {
    let mut s = Scene::empty();
    let mut me = frap_mesh::Mesh::new("Mesh");
    let mut key = Key::new("Key");
    key.key_blocks.push(ShapeKey::new("Basis", vec![]));
    key.key_blocks.push(ShapeKey::new("A", vec![]));
    key.key_blocks.push(ShapeKey::new("B", vec![]));
    let kr = s.main.shape_keys.insert("Key", key, &mut s.log);
    me.shape_keys = Some(kr);
    let mr = s.main.meshes.insert("Mesh", me, &mut s.log);
    let ob = s.link(Object::new("", Some(mr.any())), "M");
    let a = s.action(
        "KeyAction",
        "KEKey",
        "KEY",
        vec![curve(
            "key_blocks[\"B\"].value",
            0,
            &[(1.0, 0.0), (3.0, 1.0)],
            Interpolation::Linear,
        )],
    );
    let k = s.main.shape_keys.get_mut(kr, &mut s.log).unwrap();
    let adt = k.animation_data.get_or_insert_with(AnimData::default);
    adt.action = Some(a.0);
    adt.action_slot_handle = a.1;
    // Without morph targets on the mesh (WP 6.8), no `weights` channel.
    let (doc, _, _) = export_with(&s, &settings_with(|_| {}), |_| {});
    assert!(doc.animations.is_empty());
    let node_of = |g: &mut Gather| {
        let scene = g.scene;
        g.state::<NodeMap>().object(scene, ob).unwrap()
    };
    let add_targets = |g: &mut Gather| {
        let n = node_of(g);
        let m = match g.doc.nodes[n as usize].mesh {
            Some(m) => m,
            None => {
                g.doc.meshes.push(frap_gltf::json::Mesh::default());
                let m = (g.doc.meshes.len() - 1) as Index;
                g.doc.nodes[n as usize].mesh = Some(m);
                m
            }
        };
        let mesh = &mut g.doc.meshes[m as usize];
        if mesh.primitives.is_empty() {
            mesh.primitives.push(frap_gltf::json::Primitive::default());
        }
        mesh.primitives[0].targets = vec![Default::default(), Default::default()];
    };
    let (doc, bufs, _) = export_with(&s, &settings_with(|_| {}), add_targets);
    let a = &doc.animations[0];
    assert_eq!(a.name.as_deref(), Some("KeyAction"));
    let c = &a.channels[0];
    assert_eq!(c.target.path, AnimationPath::Weights);
    let smp = &a.samplers[c.sampler as usize];
    assert_eq!(floats(&doc, &bufs, smp.input).len(), 3);
    // Every exported shape key per key: A stays 0, B goes 0 → 1.
    assert_eq!(
        floats(&doc, &bufs, smp.output),
        vec![0.0, 0.0, 0.0, 0.5, 0.0, 1.0]
    );
    // Not sampled: the keyed shape key's curve, the other filled with its value.
    let st = settings_with(|a| set(a, "export_force_sampling", Value::Bool(false)));
    let (doc, bufs, _) = export_with(&s, &st, add_targets);
    let smp = &doc.animations[0].samplers[0];
    assert_eq!(smp.interpolation, Some(GInterp::Linear));
    assert_eq!(floats(&doc, &bufs, smp.output), vec![0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn merged_tracks_skip_duplicate_channels() {
    let s = AnimSettings::from(&settings_with(|_| {}));
    let tree = crate::export::vtree::VTree::default();
    let nodes = NodeMap::default();
    let (main, _) = startup::factory();
    let morph = std::collections::HashSet::new();
    let mut exp = Exp::new(
        &s,
        &tree,
        main.scenes.iter().next().unwrap().0,
        &nodes,
        &main,
        &morph,
    );
    let ch = |node: Index| GChannel {
        node,
        path: AnimationPath::Translation,
        sampler: GSampler::new(vec![0.0], vec![0.0; 3], AccessorType::Vec3, Interp::Linear),
    };
    let anim = |name: &str, chans: Vec<GChannel>| GAnim {
        name: name.into(),
        channels: chans,
        sources: Vec::new(),
    };
    let anims = vec![
        anim("A", vec![ch(0)]),
        anim("A", vec![ch(0), ch(1)]),
        anim("B", vec![ch(2)]),
    ];
    let merged = IndexMap::from([
        (SmolStr::new("Walk"), vec![0, 1]),
        (SmolStr::new("Run"), vec![2]),
    ]);
    let out = merge_tracks(&mut exp, merged, anims);
    assert_eq!(out.len(), 2);
    assert_eq!((out[0].name.as_str(), out[0].channels.len()), ("Walk", 2));
    assert_eq!(out[1].name, "Run");
    assert!(
        exp.warnings
            .iter()
            .any(|w| w.contains("same channel animation"))
    );
}

#[test]
fn non_transform_properties_are_never_sampled_as_a_channel() {
    // `get_channel_from_target(get_target('hide_render'))` is None: a group of a non-TRS
    // object property queued for sampling adds no channel (golden non_transform).
    let curves_fs = |s: &mut Scene, ob, extra: Vec<FCurve>| {
        let mut curves = vec![curve(
            "hide_render",
            0,
            &[(3.0, 0.0)],
            Interpolation::Constant,
        )];
        curves.extend(extra);
        let a = s.action("CubeAction", "OBCube", "OBJECT", curves);
        s.assign(ob, a);
    };
    let st = settings_with(|a| set(a, "export_force_sampling", Value::Bool(false)));
    let mut s = Scene::empty();
    let ob = s.link(Object::new("", None), "Cube");
    curves_fs(&mut s, ob, Vec::new());
    let (doc, _, _) = export_with(&s, &st, |_| {});
    assert!(doc.animations.is_empty(), "{:?}", doc.animations);

    let mut s = Scene::empty();
    let ob = s.link(Object::new("", None), "Cube");
    let scale = (0..3)
        .map(|i| {
            curve(
                "scale",
                i,
                &[(1.0, 1.0), (10.0, 1.0 + i as f32)],
                Interpolation::Bezier,
            )
        })
        .collect();
    curves_fs(&mut s, ob, scale);
    let (doc, _, _) = export_with(&s, &st, |_| {});
    assert_eq!(doc.animations.len(), 1);
    let chans = &doc.animations[0].channels;
    assert_eq!(chans.len(), 1, "one channel per (node, path)");
    assert_eq!(chans[0].target.path, AnimationPath::Scale);
    let smp = &doc.animations[0].samplers[chans[0].sampler as usize];
    assert_eq!(smp.interpolation_or_default(), GInterp::CubicSpline);
}
