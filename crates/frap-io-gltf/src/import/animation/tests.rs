// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the animation importer on hand-built documents: names, the stash layout,
//! interpolations, the `fps · fps_base` frames, shape-key weights into existing shape keys
//! and bone channels into pose bones (the shape keys and bones are WP 6.8's import at
//! integration; the tests create them). The Blender comparisons are the
//! `tests/golden/cases/gltf/animation/import_*.py` goldens.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use frap_anim::{HandleType, Interpolation};
use frap_gltf::Document;
use frap_gltf::accessor::{AccessorData, Components};
use frap_gltf::builder::{AccessorOpts, BufferBuilder};
use frap_gltf::json::accessor::AccessorType;
use frap_gltf::json::animation::{
    Animation, AnimationPath, AnimationSampler, Channel, ChannelTarget, Interpolation as GInterp,
};
use frap_gltf::json::{Index, Node};
use frap_rna::{IdRef, ObjectId};
use frap_scene::id::{Armature, Bone, Key, Object, ShapeKey};
use frap_scene::{ChangeLog, Main, startup};

use super::*;
use crate::import::convert::Units;
use crate::import::scenes::CollTarget;
use crate::import::settings::ImportSettings;
use crate::import::vnode::VTree;

fn acc(b: &mut BufferBuilder, ty: AccessorType, v: Vec<f32>) -> Index {
    let d = AccessorData::new(ty, false, Components::F32(v)).unwrap();
    b.push_accessor(&d, &AccessorOpts::default())
}

/// A document with `nodes` and one animation per `(name, channels)`; each channel is
/// `(node, path, interpolation, times, values)`.
#[allow(clippy::type_complexity)]
fn doc(
    nodes: Vec<Node>,
    anims: Vec<(
        Option<&str>,
        Vec<(u32, AnimationPath, GInterp, Vec<f32>, Vec<f32>)>,
    )>,
) -> (Document, Vec<Vec<u8>>) {
    let mut d = Document {
        nodes,
        ..Document::default()
    };
    let mut b = BufferBuilder::new();
    for (name, chans) in anims {
        let mut a = Animation {
            name: name.map(str::to_owned),
            ..Animation::default()
        };
        for (node, path, interp, t, v) in chans {
            let ty = match path {
                AnimationPath::Rotation => AccessorType::Vec4,
                AnimationPath::Weights => AccessorType::Scalar,
                _ => AccessorType::Vec3,
            };
            let input = acc(&mut b, AccessorType::Scalar, t);
            let output = acc(&mut b, ty, v);
            a.samplers.push(AnimationSampler {
                extensions: Default::default(),
                extras: None,
                input,
                interpolation: Some(interp),
                output,
            });
            a.channels.push(Channel {
                extensions: Default::default(),
                extras: None,
                sampler: (a.samplers.len() - 1) as Index,
                target: ChannelTarget {
                    extensions: Default::default(),
                    extras: None,
                    node: Some(node),
                    path,
                },
            });
        }
        d.animations.push(a);
    }
    let bytes = b.finish(&mut d, None);
    (d, vec![bytes])
}

/// Runs `create` with vnode `i` bound to `objects[i]`.
fn run(
    main: &mut Main,
    doc: Document,
    buffers: Vec<Vec<u8>>,
    objects: &[Option<IdRef<ObjectId>>],
    bones: &[(usize, &str)],
) {
    let settings = ImportSettings::default();
    let scene = main.scenes.iter().next().unwrap().0;
    let units = Units::new(1.0);
    let mut cx = Gltf {
        vtree: VTree::compute(&doc, units),
        doc,
        buffers,
        dir: PathBuf::from("."),
        settings: &settings,
        units,
        reports: Vec::new(),
        warned: BTreeSet::new(),
        materials: HashMap::new(),
        images: HashMap::new(),
        roughness_images: HashMap::new(),
        mesh_cache: HashMap::new(),
        scene,
        view_layer: "ViewLayer".into(),
        collections: HashMap::new(),
        blender_scenes: Vec::new(),
        active_collection: CollTarget::Master(scene),
        objects: Vec::new(),
    };
    for (i, o) in objects.iter().enumerate() {
        cx.vtree.nodes[i].blender_object = *o;
    }
    for &(i, b) in bones {
        cx.vtree.nodes[i].blender_bone_name = Some(b.into());
    }
    let mut log = ChangeLog::default();
    create(&mut cx, main, &mut log).unwrap();
}

fn node(name: &str) -> Node {
    Node {
        name: Some(name.into()),
        ..Node::default()
    }
}

#[test]
fn unused_names() {
    let mut h = HashSet::new();
    assert_eq!(find_unused_name(&h, "Walk"), "Walk");
    h.insert("Walk".to_owned());
    h.insert("Walk.001".to_owned());
    assert_eq!(find_unused_name(&h, "Walk"), "Walk.002");
    let long = "x".repeat(70);
    assert_eq!(find_unused_name(&HashSet::new(), &long).len(), 63);
    let mut h = HashSet::new();
    h.insert("y".repeat(63));
    assert_eq!(
        find_unused_name(&h, &"y".repeat(63)),
        format!("{}.001", "y".repeat(59))
    );
}

#[test]
fn actions_slots_and_stash() {
    let (mut main, _) = startup::factory();
    let mut log = ChangeLog::default();
    let a = main.objects.insert("A", Object::new("", None), &mut log);
    let b = main.objects.insert("B", Object::new("", None), &mut log);
    let (d, bufs) = doc(
        vec![node("A"), node("B")],
        vec![
            (
                Some("Walk"),
                vec![
                    (
                        0,
                        AnimationPath::Translation,
                        GInterp::Linear,
                        vec![0.0, 1.0],
                        vec![0., 0., 0., 1., 2., 3.],
                    ),
                    (
                        1,
                        AnimationPath::Scale,
                        GInterp::Step,
                        vec![0.5, 1.0],
                        vec![1., 1., 1., 2., 2., 2.],
                    ),
                ],
            ),
            (
                None,
                vec![(
                    0,
                    AnimationPath::Rotation,
                    GInterp::CubicSpline,
                    vec![0.0, 1.0],
                    [
                        [0.0f32, 0., 0., 0.],
                        [0., 0., 0., 1.],
                        [0., 0., 0., 0.],
                        [0., 0., 0., 0.],
                        // The second key is the opposite sign of the shorter path: flipped.
                        [0., 0., 0., -1.],
                        [0., 0., 0., 0.],
                    ]
                    .concat(),
                )],
            ),
        ],
    );
    run(&mut main, d, bufs, &[Some(a), Some(b)], &[]);
    let walk = main.actions.find("Walk").unwrap();
    let anim1 = main.actions.find("Anim_1").unwrap();
    let w = main.actions.get(walk).unwrap();
    let ids: Vec<&str> = w.slots.iter().map(|s| s.identifier.as_str()).collect();
    assert_eq!(ids, ["OBA", "OBB"]);
    let bag = w.channelbag(w.slots[0].handle).unwrap();
    assert_eq!(bag.fcurves.len(), 3);
    assert_eq!(bag.groups[0].name, "Object Transforms");
    // Translation (x, y, z) glTF → (x, -z, y), frames t · 24.
    let fz = &bag.fcurves[2];
    assert_eq!((fz.data_path.as_str(), fz.array_index), ("location", 2));
    assert_eq!(fz.keyframe_points[1].co, [24.0, 2.0]);
    assert_eq!(bag.fcurves[1].keyframe_points[1].co, [24.0, -3.0]);
    assert_eq!(fz.keyframe_points[0].interpolation, Interpolation::Linear);
    let sb = w.channelbag(w.slots[1].handle).unwrap();
    assert_eq!(
        sb.fcurves[0].keyframe_points[0].interpolation,
        Interpolation::Constant
    );
    assert_eq!(sb.fcurves[0].keyframe_points[0].co, [12.0, 1.0]);
    let r = main.actions.get(anim1).unwrap();
    let rb = r.channelbag(r.slots[0].handle).unwrap();
    assert_eq!(rb.fcurves.len(), 4);
    let w0 = &rb.fcurves[0];
    assert_eq!(w0.data_path, "rotation_quaternion");
    assert_eq!(w0.keyframe_points[0].interpolation, Interpolation::Bezier);
    assert_eq!(w0.keyframe_points[0].handle_left_type, HandleType::Auto);
    assert_eq!(
        (w0.keyframe_points[0].co[1], w0.keyframe_points[1].co[1]),
        (1.0, 1.0)
    );
    // A: two stash tracks, the first animation on top and assigned.
    let oa = main
        .objects
        .get(a)
        .unwrap()
        .animation_data
        .as_ref()
        .unwrap();
    let names: Vec<&str> = oa.nla_tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Anim_1", "Walk"]);
    assert!(oa.nla_tracks.iter().all(|t| t.mute && t.lock));
    assert!(oa.nla_tracks[1].active && !oa.nla_tracks[0].active);
    assert_eq!(oa.action, Some(walk));
    assert_eq!(oa.last_slot_identifier, "OBA");
    let s = &oa.nla_tracks[1].strips[0];
    assert_eq!(
        (s.name.as_str(), s.frame_start, s.frame_end),
        ("Walk", 1.0, 25.0)
    );
    assert_eq!((s.action_frame_start, s.action_frame_end), (0.0, 24.0));
    // B is only in Walk: it gets Walk, its strip spans the whole action.
    let ob = main
        .objects
        .get(b)
        .unwrap()
        .animation_data
        .as_ref()
        .unwrap();
    assert_eq!(ob.nla_tracks.len(), 1);
    assert_eq!(ob.action, Some(walk));
    assert_eq!(ob.nla_tracks[0].strips[0].action_frame_start, 0.0);
}

#[test]
fn shape_key_names() {
    let mut d = Document::default();
    let mut m = frap_gltf::json::Mesh::default();
    let mut p = frap_gltf::json::Primitive::default();
    let t = |pos: bool| {
        let mut t = indexmap::IndexMap::new();
        if pos {
            t.insert("POSITION".to_owned(), 0);
        } else {
            t.insert("NORMAL".to_owned(), 0);
        }
        t
    };
    p.targets = vec![t(true), t(false), t(true), t(true)];
    m.primitives.push(p);
    m.extras = Some(frap_gltf::json::Extras::from_value(&serde_json::json!({
        "targetNames": ["Smile", "N", "", "Basis"]
    })));
    d.meshes.push(m);
    d.accessors.push(frap_gltf::json::Accessor::new(
        None,
        frap_gltf::json::accessor::ComponentType::Float,
        AccessorType::Vec3,
        1,
    ));
    let names = shapekey_names(&d);
    assert_eq!(
        names[0],
        [
            Some("Smile".to_owned()),
            None,
            Some("target_2".to_owned()),
            Some("Basis.001".to_owned())
        ]
    );
}

#[test]
fn weights_drive_existing_shape_keys() {
    let (mut main, _) = startup::factory();
    let mut log = ChangeLog::default();
    let mut key = Key::new("Key");
    key.key_blocks.push(ShapeKey::new("Basis", vec![]));
    key.key_blocks.push(ShapeKey::new("target_0", vec![]));
    key.key_blocks.push(ShapeKey::new("target_1", vec![]));
    let kr = main.shape_keys.insert("Key", key, &mut log);
    let mut me = frap_mesh::Mesh::new("M");
    me.shape_keys = Some(kr);
    let mr = main.meshes.insert("M", me, &mut log);
    let ob = main
        .objects
        .insert("M", Object::new("", Some(mr.any())), &mut log);
    let (mut d, bufs) = doc(
        vec![Node {
            mesh: Some(0),
            ..node("M")
        }],
        vec![(
            Some("Morph"),
            vec![(
                0,
                AnimationPath::Weights,
                GInterp::Linear,
                vec![0.0, 1.0],
                vec![2.0, 0.0, 0.5, 1.0],
            )],
        )],
    );
    let mut m = frap_gltf::json::Mesh::default();
    let mut p = frap_gltf::json::Primitive::default();
    let mut t = indexmap::IndexMap::new();
    t.insert("POSITION".to_owned(), 0);
    p.targets = vec![t.clone(), t];
    m.primitives.push(p);
    d.meshes.push(m);
    run(&mut main, d, bufs, &[Some(ob)], &[]);
    let act = main
        .actions
        .get(main.actions.find("Morph").unwrap())
        .unwrap();
    assert_eq!(
        act.slots[0].identifier, "KEM",
        "the KEY slot is named after the object"
    );
    let bag = act.channelbag(act.slots[0].handle).unwrap();
    let paths: Vec<&str> = bag.fcurves.iter().map(|f| f.data_path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "key_blocks[\"target_0\"].value",
            "key_blocks[\"target_1\"].value"
        ]
    );
    assert_eq!(bag.fcurves[0].keyframe_points[1].co, [24.0, 0.5]);
    assert!(bag.groups.is_empty());
    let k = main.shape_keys.get(kr).unwrap();
    // The slider grows to the first weight.
    assert_eq!(k.key_blocks[1].slider_max, 2.0);
    let adt = k.animation_data.as_ref().unwrap();
    assert_eq!(adt.nla_tracks[0].name, "Morph");
    assert!(
        adt.action.is_some(),
        "the first animation is assigned to the shape keys"
    );
    assert!(main.objects.get(ob).unwrap().animation_data.is_none());
}

#[test]
fn bone_channels_target_pose_bones() {
    let (mut main, _) = startup::factory();
    let mut log = ChangeLog::default();
    let mut arm = Armature::new("Armature");
    let mut bone = Bone::new("Bone");
    // A rest bone at (0, 0, 1).
    bone.head_local = [0.0, 0.0, 1.0];
    bone.tail_local = [0.0, 0.0, 2.0];
    bone.matrix_local[3] = [0.0, 0.0, 1.0, 1.0];
    arm.bones.push(bone);
    let ar = main.armatures.insert("Armature", arm, &mut log);
    let ob = main
        .objects
        .insert("Armature", Object::new("", Some(ar.any())), &mut log);
    let (d, bufs) = doc(
        vec![
            Node {
                children: vec![1],
                ..node("Armature")
            },
            node("Bone"),
        ],
        vec![(
            Some("Pose"),
            vec![(
                1,
                AnimationPath::Translation,
                GInterp::Linear,
                vec![0.0, 1.0],
                vec![0., 1., 0., 0., 2., 0.],
            )],
        )],
    );
    run(&mut main, d, bufs, &[Some(ob), None], &[(1, "Bone")]);
    let act = main
        .actions
        .get(main.actions.find("Pose").unwrap())
        .unwrap();
    assert_eq!(act.slots[0].identifier, "OBArmature");
    let bag = act.channelbag(act.slots[0].handle).unwrap();
    assert_eq!(bag.groups[0].name, "Bone");
    let f = &bag.fcurves[0];
    assert!(f.data_path.starts_with("pose.bones[\"Bone\"].location"));
    // glTF (0, 1, 0) is Blender (0, 0, 1), the bone's rest head: no pose offset at t = 0.
    let at0: Vec<f32> = bag
        .fcurves
        .iter()
        .map(|f| f.keyframe_points[0].co[1])
        .collect();
    assert!(at0.iter().all(|v| v.abs() < 1e-6), "{at0:?}");
    let at1: Vec<f32> = bag
        .fcurves
        .iter()
        .map(|f| f.keyframe_points[1].co[1])
        .collect();
    let len = at1.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((len - 1.0).abs() < 1e-5, "{at1:?}");
    let adt = main
        .objects
        .get(ob)
        .unwrap()
        .animation_data
        .as_ref()
        .unwrap();
    assert_eq!(adt.nla_tracks[0].name, "Pose");
}

/// Blender 5.2.4's import of Khronos samples (`dump_blender_import.py`): the same actions,
/// slots, F-curves (keys, interpolations, handles) and NLA layout.
#[test]
fn khronos_samples_import_like_blender() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("blender_import.json")).unwrap();
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples");
    let close = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol * a.abs().max(b.abs()).max(1.0);
    for (rel, want) in fixture.as_object().unwrap() {
        let (mut main, win) = startup::empty();
        let mut log = ChangeLog::default();
        crate::import::import_file(
            &mut main,
            &win,
            &root.join(rel),
            &ImportSettings::default(),
            &mut log,
        )
        .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let actions = want["actions"].as_object().unwrap();
        assert_eq!(main.actions.len(), actions.len(), "{rel}: action count");
        for (name, wa) in actions {
            let a = main
                .actions
                .get(
                    main.actions
                        .find(name)
                        .unwrap_or_else(|| panic!("{rel}: {name}")),
                )
                .unwrap();
            let slots: Vec<&str> = a.slots.iter().map(|s| s.identifier.as_str()).collect();
            let wslots: Vec<&str> = wa["slots"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap())
                .collect();
            assert_eq!(slots, wslots, "{rel}: {name} slots");
            let range = a.frame_range_get();
            for (i, r) in wa["frame_range"].as_array().unwrap().iter().enumerate() {
                assert!(
                    close(f64::from(range[i]), r.as_f64().unwrap(), 1e-5),
                    "{rel}: {name} range"
                );
            }
            let mut curves = Vec::new();
            for l in &a.layers {
                for s in &l.strips {
                    for b in &s.keyframe().unwrap().channelbags {
                        let id = a.slot(b.slot_handle).unwrap().identifier.clone();
                        curves.extend(b.fcurves.iter().map(|f| (id.clone(), f)));
                    }
                }
            }
            let wcurves = wa["fcurves"].as_array().unwrap();
            assert_eq!(curves.len(), wcurves.len(), "{rel}: {name} curves");
            for ((slot, fc), w) in curves.iter().zip(wcurves) {
                let what = format!("{rel}: {name} {}[{}]", fc.data_path, fc.array_index);
                assert_eq!(slot.as_str(), w["slot"].as_str().unwrap(), "{what}");
                assert_eq!(
                    fc.data_path.as_str(),
                    w["data_path"].as_str().unwrap(),
                    "{what}"
                );
                assert_eq!(
                    i64::from(fc.array_index),
                    w["array_index"].as_i64().unwrap(),
                    "{what}"
                );
                assert_eq!(fc.group.as_deref(), w["group"].as_str(), "{what}");
                let keys = w["keys"].as_array().unwrap();
                assert_eq!(fc.keyframe_points.len(), keys.len(), "{what}");
                for (k, wk) in fc.keyframe_points.iter().zip(keys) {
                    let v = |i: usize, j: usize| wk[i][j].as_f64().unwrap();
                    for j in 0..2 {
                        assert!(
                            close(f64::from(k.co[j]), v(0, j), 1e-5),
                            "{what} co {:?} {wk}",
                            k.co
                        );
                        assert!(
                            close(f64::from(k.handle_left[j]), v(2, j), 1e-4),
                            "{what} hl {:?} {wk}",
                            k.handle_left
                        );
                        assert!(
                            close(f64::from(k.handle_right[j]), v(3, j), 1e-4),
                            "{what} hr {:?} {wk}",
                            k.handle_right
                        );
                    }
                    assert_eq!(
                        k.interpolation.identifier(),
                        wk[1].as_str().unwrap(),
                        "{what}"
                    );
                    assert_eq!(
                        k.handle_left_type.identifier(),
                        wk[4].as_str().unwrap(),
                        "{what}"
                    );
                    assert_eq!(
                        k.handle_right_type.identifier(),
                        wk[5].as_str().unwrap(),
                        "{what}"
                    );
                }
            }
        }
        for (oname, wo) in want["objects"].as_object().unwrap() {
            let ob = main.objects.get(main.objects.find(oname).unwrap()).unwrap();
            let adt = ob
                .animation_data
                .as_ref()
                .unwrap_or_else(|| panic!("{rel}: {oname}"));
            let aname = adt
                .action
                .and_then(|a| main.actions.get(a))
                .map(|a| a.header.name.to_string());
            assert_eq!(
                aname.as_deref(),
                wo["action"].as_str(),
                "{rel}: {oname} action"
            );
            let slot = adt
                .action
                .and_then(|a| main.actions.get(a)?.slot(adt.action_slot_handle))
                .map(|s| s.identifier.to_string());
            assert_eq!(slot.as_deref(), wo["slot"].as_str(), "{rel}: {oname} slot");
            let tracks = wo["tracks"].as_array().unwrap();
            assert_eq!(adt.nla_tracks.len(), tracks.len(), "{rel}: {oname} tracks");
            for (t, wt) in adt.nla_tracks.iter().zip(tracks) {
                assert_eq!(t.name.as_str(), wt["name"].as_str().unwrap());
                assert_eq!(
                    (t.mute, t.lock),
                    (wt["mute"].as_bool().unwrap(), wt["lock"].as_bool().unwrap())
                );
                let strips = wt["strips"].as_array().unwrap();
                assert_eq!(t.strips.len(), strips.len());
                for (s, ws) in t.strips.iter().zip(strips) {
                    assert_eq!(s.name.as_str(), ws["name"].as_str().unwrap());
                    let sa = s.action.and_then(|a| main.actions.get(a)).unwrap();
                    assert_eq!(sa.header.name.as_str(), ws["action"].as_str().unwrap());
                    assert_eq!(
                        sa.slot(s.action_slot_handle).unwrap().identifier.as_str(),
                        ws["slot"].as_str().unwrap()
                    );
                    for (got, key) in [
                        (s.frame_start, "frame_start"),
                        (s.frame_end, "frame_end"),
                        (s.action_frame_start, "action_frame_start"),
                        (s.action_frame_end, "action_frame_end"),
                    ] {
                        assert!(
                            close(f64::from(got), ws[key].as_f64().unwrap(), 1e-5),
                            "{rel}: {oname} {key} {got}"
                        );
                    }
                }
            }
        }
    }
}

/// Import then export: the animations Blender 5.2.4 writes for the same round trip (name,
/// channel count; values are compared by the `roundtrip` golden).
#[test]
fn khronos_samples_reexport_like_blender() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples");
    let expected: [(&str, &[(&str, usize)]); 2] = [
        (
            "InterpolationTest/glTF-Binary/InterpolationTest.glb",
            &[
                ("CubicSpline Rotation", 1),
                ("CubicSpline Scale", 1),
                ("CubicSpline Translation", 1),
                ("Linear Rotation", 1),
                ("Linear Scale", 1),
                ("Linear Translation", 1),
                ("Step Rotation", 1),
                ("Step Scale", 1),
                ("Step Translation", 1),
            ],
        ),
        (
            "AnimatedTriangle/glTF-Embedded/AnimatedTriangle.gltf",
            &[("Anim_0", 1)],
        ),
    ];
    for (rel, want) in expected {
        let (mut main, win) = startup::empty();
        let mut log = ChangeLog::default();
        let settings = ImportSettings::default();
        crate::import::import_file(&mut main, &win, &root.join(rel), &settings, &mut log).unwrap();
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let es = crate::export::ExportSettings::default();
        let out = crate::export::export(&main, &ctx, &es).unwrap();
        let mut got: Vec<(String, usize)> = out
            .document
            .animations
            .iter()
            .map(|a| (a.name.clone().unwrap_or_default(), a.channels.len()))
            .collect();
        got.sort();
        let want: Vec<(String, usize)> = want.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect();
        assert_eq!(got, want, "{rel}");
    }
}
