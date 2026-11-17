// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.13 acceptance: fingerprint invalidation on every `gen` bump, reads never change a
//! fingerprint, cycle report, the evaluated mesh shares the original `Arc` without modifiers,
//! modifier statuses, cache pruning and the `apply_pending_animation` boundary API.

mod common;

use std::sync::Arc;

use common::TestHost;
use frap_eval::animate::{AnimationApplied, apply_animation_at, apply_pending_animation};
use frap_eval::{EvalCache, bound_box, dimensions};
use frap_rna::{IdRef, ObjectId, ReportType};
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::{Key, Modifier, ModifierSettings};
use frap_scene::{EditMesh, EvalMode, Evaluator, ModifierStatus, ViewCache};
use glam::{Mat4, Vec3};

const CUBE: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];

fn cube_faces() -> Vec<Vec<i32>> {
    vec![
        vec![0, 3, 2, 1],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![1, 2, 6, 5],
        vec![2, 3, 7, 6],
        vec![3, 0, 4, 7],
    ]
}

/// `Root` (empty) ← `Mid` (mesh "Box") ← `Leaf` (empty), plus an unrelated `Other`.
fn scene() -> (TestHost, [IdRef<ObjectId>; 4]) {
    let mut h = TestHost::factory();
    let me = h.add_mesh("Box", &CUBE, &cube_faces());
    let root = h.add_object("Root", None);
    let mid = h.add_object("Mid", Some(me.any()));
    let leaf = h.add_object("Leaf", None);
    let other = h.add_object("Other", None);
    h.edit_obj(root, |o| {
        o.location = [1.0, 2.0, 3.0];
        o.scale = [2.0, 2.0, 2.0];
    });
    h.edit_obj(mid, |o| {
        o.parent = Some(root);
        o.location = [0.5, 0.0, 0.0];
        o.rotation_euler = [0.0, 0.0, 0.5];
    });
    h.edit_obj(leaf, |o| {
        o.parent = Some(mid);
        o.location = [0.0, 1.0, 0.0];
    });
    (h, [root, mid, leaf, other])
}

fn fp(h: &TestHost, ob: IdRef<ObjectId>) -> u64 {
    EvalCache::fingerprint(h, ob, EvalMode::Viewport).expect("object exists")
}

fn eval(h: &mut TestHost, ob: IdRef<ObjectId>) -> Arc<frap_scene::EvaluatedObject> {
    let mut cache = std::mem::take(&mut h.eval);
    let r = cache.object(h, ob, EvalMode::Viewport).expect("evaluates");
    h.eval = cache;
    r
}

#[test]
fn world_matrix_composes_parents_and_parent_inverse() {
    let (mut h, [root, mid, leaf, _]) = scene();
    let inv = Mat4::from_translation(Vec3::new(-1.0, 0.0, 0.0));
    h.edit_obj(leaf, |o| o.matrix_parent_inverse = inv);
    let r = eval(&mut h, root).matrix_world;
    let m = eval(&mut h, mid).matrix_world;
    let l = eval(&mut h, leaf).matrix_world;
    let basis = |name: &str| frap_eval::matrix_basis(h.main.objects.get(h.obj(name)).unwrap());
    assert!(r.abs_diff_eq(basis("Root"), 0.0));
    assert!(m.abs_diff_eq(r * basis("Mid"), 1e-6));
    assert!(l.abs_diff_eq(m * inv * basis("Leaf"), 1e-6));
    // Root (1,2,3) scaled 2; Mid +0.5 x; Leaf: (0,1,0) rotated 0.5 rad about z, inverse −1 x.
    let (s, c) = 0.5f32.sin_cos();
    let want = Vec3::new(1.0, 2.0, 3.0)
        + 2.0 * (Vec3::new(0.5, 0.0, 0.0) + Vec3::new(-c - s, -s + c, 0.0));
    assert!(l.w_axis.truncate().abs_diff_eq(want, 1e-5), "{l:?}");
}

#[test]
fn bounds_and_dimensions_include_world_scale() {
    let (mut h, [root, mid, leaf, _]) = scene();
    let e = eval(&mut h, mid);
    assert_eq!(e.bounds, Some([[-1.0; 3], [1.0; 3]]));
    // Root's scale 2 reaches the child's dimensions; the rotation does not.
    let d = dimensions(&e);
    assert!(d.iter().all(|v| (v - 4.0).abs() < 1e-6), "{d:?}");
    assert_eq!(bound_box(&e)[6], [1.0; 3]);
    for ob in [root, leaf] {
        let e = eval(&mut h, ob);
        assert_eq!(e.bounds, None);
        assert_eq!(dimensions(&e), [0.0; 3]);
        assert!(e.mesh.is_none());
    }
}

#[test]
fn evaluated_mesh_shares_the_original_arc() {
    let (mut h, [_, mid, _, _]) = scene();
    let me = h.main.meshes.find("Box").unwrap();
    let e = eval(&mut h, mid);
    let original = h.main.meshes.get_arc(me).unwrap();
    assert!(Arc::ptr_eq(e.mesh.as_ref().unwrap(), &original));
    assert!(e.modifier_status.is_empty() && e.warnings.is_empty());
}

fn modifier(name: &str, ty: ModifierType) -> Modifier {
    Modifier {
        name: name.into(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: true,
        show_on_cage: false,
        show_expanded: true,
        is_active: false,
        use_pin_to_last: false,
        persistent_uid: 1,
        settings: ModifierSettings::new(ty),
    }
}

#[test]
fn modifiers_pass_through_with_a_warning_until_m4() {
    let (mut h, [_, mid, _, _]) = scene();
    let me = h.main.meshes.find("Box").unwrap();
    h.edit_obj(mid, |o| {
        let mut pinned = modifier("Pinned", ModifierType::Triangulate);
        pinned.use_pin_to_last = true;
        let mut hidden = modifier("Hidden", ModifierType::Bevel);
        hidden.show_viewport = false;
        let mut no_render = modifier("NoRender", ModifierType::Mirror);
        no_render.show_render = false;
        o.modifiers = vec![
            pinned,
            modifier("Subdivision", ModifierType::Subsurf),
            hidden,
            no_render,
        ];
    });
    let e = eval(&mut h, mid);
    // Pass-through keeps the original mesh.
    assert!(Arc::ptr_eq(
        e.mesh.as_ref().unwrap(),
        &h.main.meshes.get_arc(me).unwrap()
    ));
    let st: Vec<(&str, &ModifierStatus)> = e
        .modifier_status
        .iter()
        .map(|(n, s)| (n.as_str(), s))
        .collect();
    assert_eq!(
        st,
        [
            ("Subdivision", &ModifierStatus::PassedThrough),
            ("Hidden", &ModifierStatus::Skipped),
            ("NoRender", &ModifierStatus::PassedThrough),
            ("Pinned", &ModifierStatus::PassedThrough),
        ]
    );
    let msgs: Vec<&str> = e.warnings.iter().map(|r| r.message.as_str()).collect();
    assert_eq!(
        msgs,
        [
            "Modifier 'Subdivision' (SUBSURF) is not evaluated by frap; passed through",
            "Modifier 'NoRender' (MIRROR) is not evaluated by frap; passed through",
            "Modifier 'Pinned' (TRIANGULATE) is not evaluated by frap; passed through",
        ]
    );
    assert!(e.warnings.iter().all(|r| r.level == ReportType::Warning));

    // Render mode uses `show_render`.
    let mut cache = EvalCache::new();
    let r = cache.object(&h, mid, EvalMode::Render).unwrap();
    let skipped: Vec<&str> = r
        .modifier_status
        .iter()
        .filter(|(_, s)| *s == ModifierStatus::Skipped)
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(skipped, ["NoRender"]);
}

#[test]
fn edit_mode_uses_the_mesh_view_and_show_in_editmode() {
    let (mut h, [_, mid, _, _]) = scene();
    let me = h.main.meshes.find("Box").unwrap();
    h.edit_obj(mid, |o| {
        let mut m = modifier("NotInEdit", ModifierType::Subsurf);
        m.show_in_editmode = false;
        o.modifiers = vec![m];
    });
    let mut view = frap_mesh::Mesh::new("Box");
    view.from_pydata(&CUBE[..4], &[], &[vec![0, 1, 2, 3]], false);
    let view = Arc::new(view);
    h.edit_view = Some(view.clone());
    let before = fp(&h, mid);
    h.edit.meshes.insert(
        me.key(),
        EditMesh {
            bm: Default::default(),
            bm_gen: 7,
            view: ViewCache::default(),
        },
    );
    assert_ne!(
        fp(&h, mid),
        before,
        "entering Edit Mode changes the fingerprint"
    );
    let e = eval(&mut h, mid);
    assert!(Arc::ptr_eq(e.mesh.as_ref().unwrap(), &view));
    assert_eq!(e.bounds, Some([[-1.0, -1.0, -1.0], [1.0, 1.0, -1.0]]));
    assert_eq!(e.modifier_status[0].1, ModifierStatus::Skipped);
    // Every BMesh edit (bm_gen bump) invalidates.
    let at7 = fp(&h, mid);
    h.edit.meshes.get_mut(&me.key()).unwrap().bm_gen = 8;
    assert_ne!(fp(&h, mid), at7);
}

#[test]
fn cache_hits_while_nothing_changes() {
    let (mut h, [_, _, leaf, _]) = scene();
    let a = eval(&mut h, leaf);
    let b = eval(&mut h, leaf);
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(h.eval.len(), 3, "leaf, mid and root are cached");
}

#[test]
fn every_generation_bump_invalidates() {
    let (mut h, [root, mid, leaf, other]) = scene();
    let me = h.main.meshes.find("Box").unwrap();

    let check =
        |h: &mut TestHost, what: &str, bump: &dyn Fn(&mut TestHost), hit: &[IdRef<ObjectId>]| {
            let before: Vec<_> = [root, mid, leaf, other]
                .iter()
                .map(|&o| (fp(h, o), eval(h, o)))
                .collect();
            bump(h);
            for (i, &o) in [root, mid, leaf, other].iter().enumerate() {
                let (f0, e0) = &before[i];
                let e1 = eval(h, o);
                if hit.contains(&o) {
                    assert_ne!(fp(h, o), *f0, "{what}: fingerprint of {o:?}");
                    assert!(!Arc::ptr_eq(e0, &e1), "{what}: re-evaluated {o:?}");
                } else {
                    assert_eq!(fp(h, o), *f0, "{what}: unaffected {o:?}");
                    assert!(Arc::ptr_eq(e0, &e1), "{what}: cached {o:?}");
                }
            }
        };

    // A write that changes nothing still bumps the generation.
    check(
        &mut h,
        "root gen",
        &|h| h.edit_obj(root, |_| {}),
        &[root, mid, leaf],
    );
    check(
        &mut h,
        "mid gen",
        &|h| h.edit_obj(mid, |_| {}),
        &[mid, leaf],
    );
    check(&mut h, "leaf gen", &|h| h.edit_obj(leaf, |_| {}), &[leaf]);
    check(
        &mut h,
        "other gen",
        &|h| h.edit_obj(other, |_| {}),
        &[other],
    );
    check(
        &mut h,
        "mesh gen",
        &|h| {
            h.main.meshes.get_mut(me, &mut h.log).unwrap();
        },
        &[mid, leaf],
    );
    // Shape keys: a Key datablock on the mesh, then a write to it.
    check(
        &mut h,
        "shape key assigned",
        &|h| {
            let k = h.main.shape_keys.insert("Key", Key::new("Key"), &mut h.log);
            h.main.meshes.get_mut(me, &mut h.log).unwrap().shape_keys = Some(k);
        },
        &[mid, leaf],
    );
    check(
        &mut h,
        "shape key gen",
        &|h| {
            let k = h.main.shape_keys.find("Key").unwrap();
            h.main.shape_keys.get_mut(k, &mut h.log).unwrap();
        },
        &[mid, leaf],
    );
    // The scene frame invalidates everything; other scene writes do not.
    let scene = h.win.scene;
    check(
        &mut h,
        "scene write",
        &|h| h.main.scenes.get_mut(scene, &mut h.log).unwrap().frame_end = 99,
        &[],
    );
    check(
        &mut h,
        "frame",
        &|h| {
            h.main
                .scenes
                .get_mut(scene, &mut h.log)
                .unwrap()
                .frame_current = 5
        },
        &[root, mid, leaf, other],
    );
    // Re-parenting moves the dependency.
    check(
        &mut h,
        "reparent",
        &|h| h.edit_obj(leaf, |o| o.parent = Some(other)),
        &[leaf],
    );
    check(&mut h, "old parent", &|h| h.edit_obj(mid, |_| {}), &[mid]);
    check(
        &mut h,
        "new parent",
        &|h| h.edit_obj(other, |_| {}),
        &[leaf, other],
    );
}

#[test]
fn reads_never_change_a_fingerprint_or_the_state() {
    let (mut h, obs) = scene();
    let gens = h.generations();
    let fps: Vec<u64> = obs.iter().map(|&o| fp(&h, o)).collect();
    let log_len = h.log.modified.len();
    for _ in 0..3 {
        for &o in &obs {
            let e = eval(&mut h, o);
            let _ = (
                dimensions(&e),
                bound_box(&e),
                e.mesh.as_ref().map(|m| m.vert_normals()),
            );
        }
        let mut cache = std::mem::take(&mut h.eval);
        cache.depsgraph_update(&h);
        let _ = cache.object(&h, obs[2], EvalMode::Render);
        h.eval = cache;
    }
    assert_eq!(h.generations(), gens);
    assert_eq!(h.log.modified.len(), log_len);
    assert_eq!(obs.iter().map(|&o| fp(&h, o)).collect::<Vec<_>>(), fps);
}

#[test]
fn parent_cycle_is_reported_and_evaluated_in_data_order() {
    let mut h = TestHost::factory();
    let b = h.add_object("B", None);
    let a = h.add_object("A", None);
    let child = h.add_object("Child", None);
    // Not possible through RNA (Blender refuses it), but a loaded state may hold it.
    h.edit_obj(a, |o| {
        o.parent = Some(b);
        o.location = [1.0, 0.0, 0.0];
    });
    h.edit_obj(b, |o| {
        o.parent = Some(a);
        o.location = [0.0, 2.0, 0.0];
    });
    h.edit_obj(child, |o| {
        o.parent = Some(b);
        o.location = [0.0, 0.0, 3.0];
    });
    // Query order does not matter.
    let mut c1 = EvalCache::new();
    let mut c2 = EvalCache::new();
    let a1 = c1.object(&h, a, EvalMode::Viewport).unwrap();
    let b2 = c2.object(&h, b, EvalMode::Viewport).unwrap();
    let b1 = c1.object(&h, b, EvalMode::Viewport).unwrap();
    let a2 = c2.object(&h, a, EvalMode::Viewport).unwrap();
    assert_eq!(a1.matrix_world, a2.matrix_world);
    assert_eq!(b1.matrix_world, b2.matrix_world);
    // "A" comes first in bpy.data: evaluated first, with a not-yet-evaluated parent.
    assert_eq!(a1.matrix_world.w_axis.truncate(), Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(b1.matrix_world.w_axis.truncate(), Vec3::new(1.0, 2.0, 0.0));
    let want = "Dependency cycle detected:\n  OBA/TRANSFORM_PARENT() depends on\n  \
                OBB/TRANSFORM_FINAL() via 'Parent'\n  OBA/TRANSFORM_FINAL() via 'Parent'";
    for e in [&a1, &b1] {
        assert_eq!(e.warnings.len(), 1);
        assert_eq!(e.warnings[0].level, ReportType::Warning);
        assert_eq!(e.warnings[0].message, want);
    }
    // A child of the cycle is evaluated normally and is not itself reported.
    let ch = c1.object(&h, child, EvalMode::Viewport).unwrap();
    assert!(ch.warnings.is_empty());
    assert_eq!(ch.matrix_world.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));

    // Breaking the cycle at B invalidates A (whose evaluation order depended on it).
    let fa = fp(&h, a);
    h.edit_obj(b, |o| o.parent = None);
    assert_ne!(fp(&h, a), fa);
    let a3 = c1.object(&h, a, EvalMode::Viewport).unwrap();
    assert!(a3.warnings.is_empty());
    assert_eq!(a3.matrix_world.w_axis.truncate(), Vec3::new(1.0, 2.0, 0.0));

    // A self-parent is a cycle of one.
    h.edit_obj(child, |o| o.parent = Some(child));
    let s = c1.object(&h, child, EvalMode::Viewport).unwrap();
    assert!(
        s.warnings[0]
            .message
            .starts_with("Dependency cycle detected:\n  OBChild/")
    );
    assert_eq!(s.matrix_world.w_axis.truncate(), Vec3::new(0.0, 0.0, 3.0));
}

#[test]
fn dangling_parent_and_missing_object() {
    let (mut h, [root, mid, _, _]) = scene();
    let mut cache = EvalCache::new();
    h.main.objects.remove(root, &mut h.log).unwrap();
    // `Mid.parent` now names a removed object: evaluated as unparented.
    let e = cache.object(&h, mid, EvalMode::Viewport).unwrap();
    let basis = frap_eval::matrix_basis(h.main.objects.get(mid).unwrap());
    assert_eq!(e.matrix_world, basis);
    let err = cache.object(&h, root, EvalMode::Viewport).unwrap_err();
    assert!(err.message.contains("does not exist"), "{err}");
    assert_eq!(EvalCache::fingerprint(&h, root, EvalMode::Viewport), None);
}

#[test]
fn depsgraph_update_evaluates_everything_and_prunes_removed_objects() {
    let (mut h, [root, mid, leaf, other]) = scene();
    let mut cache = EvalCache::new();
    cache.depsgraph_update(&h);
    // The factory Cube, Camera and Light plus the four test objects.
    assert_eq!(cache.len(), h.main.objects.len());
    let e = cache.object(&h, leaf, EvalMode::Viewport).unwrap();
    let mut fresh = EvalCache::new();
    assert_eq!(
        e.matrix_world,
        fresh
            .object(&h, leaf, EvalMode::Viewport)
            .unwrap()
            .matrix_world
    );
    h.main.objects.remove(other, &mut h.log).unwrap();
    cache.depsgraph_update(&h);
    assert_eq!(cache.len(), h.main.objects.len());
    let _ = (root, mid);
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn deep_hierarchies_do_not_recurse() {
    let mut h = TestHost::factory();
    let mut prev: Option<IdRef<ObjectId>> = None;
    let mut last = None;
    for i in 0..5_000 {
        let ob = h.add_object(&format!("N{i:05}"), None);
        h.edit_obj(ob, |o| {
            o.parent = prev;
            o.location = [0.0, 0.0, 1.0];
        });
        prev = Some(ob);
        last = Some(ob);
    }
    let mut cache = EvalCache::new();
    let e = cache.object(&h, last.unwrap(), EvalMode::Viewport).unwrap();
    assert_eq!(e.matrix_world.w_axis.z, 5_000.0);
    cache.depsgraph_update(&h);
}

#[test]
fn animation_boundary_is_a_no_op_until_m6() {
    let (mut h, _) = scene();
    let gens = h.generations();
    assert_eq!(
        apply_pending_animation(&mut h, false).unwrap(),
        AnimationApplied::default()
    );
    assert_eq!(apply_pending_animation(&mut h, true).unwrap().channels, 0);
    assert_eq!(apply_animation_at(&mut h, 10.0).unwrap().channels, 0);
    assert_eq!(h.set_paths, 0);
    assert!(h.reports.is_empty());
    assert_eq!(h.generations(), gens);
}

mod props {
    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    enum Op {
        Move(usize, [f32; 3]),
        Rotate(usize, [f32; 3]),
        Reparent(usize, Option<usize>),
        ParentInverse(usize, [f32; 3]),
        TouchMesh,
        Frame(i32),
    }

    fn op() -> impl Strategy<Value = Op> {
        let v = prop::array::uniform3(-3.0f32..3.0);
        prop_oneof![
            (0..6usize, v.clone()).prop_map(|(i, v)| Op::Move(i, v)),
            (0..6usize, v.clone()).prop_map(|(i, v)| Op::Rotate(i, v)),
            (0..6usize, prop::option::of(0..6usize)).prop_map(|(i, p)| Op::Reparent(i, p)),
            (0..6usize, v).prop_map(|(i, v)| Op::ParentInverse(i, v)),
            Just(Op::TouchMesh),
            (0..20i32).prop_map(Op::Frame),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        /// After any sequence of writes, the cached evaluation equals a fresh one for every
        /// object (cycles included: reparenting is unchecked here).
        #[test]
        fn cached_equals_fresh(ops in prop::collection::vec(op(), 1..40)) {
            let mut h = TestHost::factory();
            let me = h.add_mesh("Box", &CUBE, &cube_faces());
            let obs: Vec<IdRef<ObjectId>> = (0..6)
                .map(|i| h.add_object(&format!("O{i}"), (i % 2 == 0).then(|| me.any())))
                .collect();
            let mut cache = EvalCache::new();
            for op in ops {
                match op {
                    Op::Move(i, v) => h.edit_obj(obs[i], |o| o.location = v),
                    Op::Rotate(i, v) => h.edit_obj(obs[i], |o| o.rotation_euler = v),
                    Op::Reparent(i, p) => h.edit_obj(obs[i], |o| o.parent = p.map(|p| obs[p])),
                    Op::ParentInverse(i, v) => h.edit_obj(obs[i], |o| {
                        o.matrix_parent_inverse = Mat4::from_translation(Vec3::from_array(v));
                    }),
                    Op::TouchMesh => {
                        h.main.meshes.get_mut(me, &mut h.log).unwrap().positions_mut()[0][0] += 0.5;
                    }
                    Op::Frame(f) => {
                        let s = h.win.scene;
                        h.main.scenes.get_mut(s, &mut h.log).unwrap().frame_current = f;
                    }
                }
                for &ob in &obs {
                    let cached = cache.object(&h, ob, EvalMode::Viewport).unwrap();
                    let fresh = EvalCache::new().object(&h, ob, EvalMode::Viewport).unwrap();
                    prop_assert_eq!(cached.matrix_world.to_cols_array().map(f32::to_bits),
                        fresh.matrix_world.to_cols_array().map(f32::to_bits));
                    prop_assert_eq!(cached.bounds, fresh.bounds);
                    prop_assert_eq!(&cached.warnings, &fresh.warnings);
                }
            }
        }
    }
}
