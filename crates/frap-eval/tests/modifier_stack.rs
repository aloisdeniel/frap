// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 4.1 acceptance (frap-side; the Blender comparisons are the `modifiers/stack/*` goldens):
//! NODES dispatches on the node group's (library file name, name) and ignores a same-named local
//! group; `depends_on` targets are part of the fingerprint; Edit Mode skips types that do not
//! support it; a `RequiresOriginalData` type after a non-deforming modifier is the bad-stack-
//! position error; `EvalCache::apply_modifier` evaluates one modifier on the original mesh.

mod common;

use common::TestHost;
use frap_eval::EvalCache;
use frap_mesh::{SHARP_EDGE, SHARP_FACE};
use frap_modifiers::stack::edit;
use frap_modifiers::types::nodes::smooth_by_angle as sba;
use frap_rna::{IdRef, LibraryId, NodeTreeId, ObjectId};
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::{GnInput, GnInputs, GnValue, Library, ModifierSettings};
use frap_scene::{EditMesh, EvalMode, Evaluator, ModifierStatus, ViewCache};

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

/// A host with a mesh object "Box" and the linked "Smooth by Angle" group (as
/// `object.shade_auto_smooth` links it); returns the object and the group.
fn host() -> (TestHost, IdRef<ObjectId>, IdRef<NodeTreeId>) {
    let mut h = TestHost::factory();
    let me = h.add_mesh("Box", &CUBE, &cube_faces());
    let ob = h.add_object("Box", Some(me.any()));
    let mut lib = Library::new("", sba::ESSENTIALS_FILEPATH);
    lib.is_archive = true;
    let lib: IdRef<LibraryId> = h
        .main
        .libraries
        .insert(sba::ARCHIVE_LIBRARY_NAME, lib, &mut h.log);
    let group = h
        .main
        .node_groups
        .insert(sba::NAME, sba::build(lib), &mut h.log);
    (h, ob, group)
}

fn inputs(angle: f32) -> GnInputs {
    let mut g = GnInputs::default();
    g.0.insert(sba::INPUT_MESH.into(), GnInput::new(GnValue::None));
    g.0.insert(sba::INPUT_ANGLE.into(), GnInput::new(GnValue::Float(angle)));
    g.0.insert(
        sba::INPUT_IGNORE_SHARPNESS.into(),
        GnInput::new(GnValue::Bool(false)),
    );
    g
}

/// Adds a NODES modifier using `group` to `ob`.
fn add_nodes(h: &mut TestHost, ob: IdRef<ObjectId>, group: IdRef<NodeTreeId>, angle: f32) {
    h.edit_obj(ob, |o| {
        let i = edit::add(o, None, ModifierType::Nodes).unwrap();
        if let ModifierSettings::Nodes(s) = &mut o.modifiers[i].settings {
            s.node_group = Some(group.any());
            s.inputs = inputs(angle);
        }
    });
}

fn eval(h: &TestHost, ob: IdRef<ObjectId>) -> std::sync::Arc<frap_scene::EvaluatedObject> {
    EvalCache::new().object(h, ob, EvalMode::Viewport).unwrap()
}

#[test]
fn nodes_evaluates_the_linked_smooth_by_angle() {
    let (mut h, ob, group) = host();
    h.edit_obj(ob, |o| o.modifiers.clear());
    add_nodes(&mut h, ob, group, 2.0);
    let e = eval(&h, ob);
    assert_eq!(e.modifier_status[0].1, ModifierStatus::Evaluated);
    let mesh = e.mesh.as_ref().unwrap();
    // 2 rad > 90°: every edge is smooth; sharp_face is removed.
    assert_eq!(mesh.sharp_edges().unwrap(), &[false; 12][..]);
    assert!(mesh.attributes().get(SHARP_FACE).is_none());
    // The data is untouched.
    let me = h.main.meshes.find("Box").unwrap();
    assert!(
        h.main
            .meshes
            .get(me)
            .unwrap()
            .attributes()
            .get(SHARP_EDGE)
            .is_none()
    );
}

#[test]
fn a_local_group_named_smooth_by_angle_is_not_evaluated() {
    let (mut h, ob, _) = host();
    let mut local = sba::build(IdRef::<LibraryId>::from_key(frap_rna::IdKey::next()));
    local.header.library = None;
    local.header.is_linked_packed = false;
    let local = h.main.node_groups.insert(sba::NAME, local, &mut h.log);
    add_nodes(&mut h, ob, local, 2.0);
    let e = eval(&h, ob);
    assert_eq!(
        e.modifier_status[0].1,
        ModifierStatus::Error(
            "Unsupported node group \"Smooth by Angle\" (frap supports: Smooth by Angle)".into()
        )
    );
    assert!(
        e.mesh
            .as_ref()
            .unwrap()
            .attributes()
            .get(SHARP_EDGE)
            .is_none()
    );
    assert_eq!(
        e.warnings[0].message,
        "Modifier 'GeometryNodes': Unsupported node group \"Smooth by Angle\" (frap supports: Smooth by Angle)"
    );
    // Without a group the modifier is disabled.
    h.edit_obj(ob, |o| {
        if let ModifierSettings::Nodes(s) = &mut o.modifiers[0].settings {
            s.node_group = None;
        }
    });
    assert_eq!(
        eval(&h, ob).modifier_status[0].1,
        ModifierStatus::Disabled("no node group".into())
    );
}

#[test]
fn depends_on_targets_are_part_of_the_fingerprint() {
    let (mut h, ob, group) = host();
    add_nodes(&mut h, ob, group, 0.5);
    let before = EvalCache::fingerprint(&h, ob, EvalMode::Viewport).unwrap();
    // A change of the node group (and of nothing else) invalidates the object.
    let t = h.main.node_groups.get_mut(group, &mut h.log).unwrap();
    t.nodes[0].location[0] += 1.0;
    let after = EvalCache::fingerprint(&h, ob, EvalMode::Viewport).unwrap();
    assert_ne!(before, after);
    // An object operand: a Boolean whose operand moves.
    let other = h.add_object("Other", None);
    h.edit_obj(ob, |o| {
        let i = edit::add(o, None, ModifierType::Boolean).unwrap();
        if let ModifierSettings::Boolean(s) = &mut o.modifiers[i].settings {
            s.object = Some(other.any());
        }
    });
    let a = EvalCache::fingerprint(&h, ob, EvalMode::Viewport).unwrap();
    h.edit_obj(other, |o| o.location = [1.0, 0.0, 0.0]);
    let b = EvalCache::fingerprint(&h, ob, EvalMode::Viewport).unwrap();
    if frap_modifiers::registry::find(ModifierType::Boolean).is_some() {
        assert_ne!(a, b, "the operand's fingerprint is part of the object's");
    }
}

#[test]
fn edit_mode_skips_types_without_edit_mode_support() {
    let (mut h, ob, group) = host();
    add_nodes(&mut h, ob, group, 2.0);
    h.edit_obj(ob, |o| {
        // DECIMATE does not support Edit Mode, even with show_in_editmode on.
        let i = edit::add(o, None, ModifierType::Decimate).unwrap();
        o.modifiers[i].show_in_editmode = true;
    });
    let me = h.main.meshes.find("Box").unwrap();
    let mesh = h.main.meshes.get(me).unwrap().clone();
    h.edit.meshes.insert(
        me.key(),
        EditMesh {
            bm: Default::default(),
            bm_gen: 1,
            view: ViewCache::default(),
        },
    );
    h.edit_view = Some(std::sync::Arc::new(mesh));
    let e = eval(&h, ob);
    let st: Vec<_> = e
        .modifier_status
        .iter()
        .map(|(n, s)| (n.as_str(), s.clone()))
        .collect();
    assert_eq!(st[0], ("GeometryNodes", ModifierStatus::Evaluated));
    assert_eq!(st[1], ("Decimate", ModifierStatus::Skipped));
}

#[test]
fn requires_original_data_after_a_constructive_modifier_is_an_error() {
    let (mut h, ob, group) = host();
    add_nodes(&mut h, ob, group, 2.0);
    // Put MULTIRES after the NODES modifier (a raw stack: the editing rules would refuse).
    h.edit_obj(ob, |o| {
        let mut m = edit::new_modifier(ModifierType::Multires);
        m.name = "Multires".into();
        o.modifiers.push(m);
    });
    let e = eval(&h, ob);
    assert_eq!(
        e.modifier_status[1].1,
        ModifierStatus::Error(frap_modifiers::stack::BAD_STACK_POSITION.into())
    );
}

#[test]
fn apply_modifier_evaluates_one_modifier_on_the_original_mesh() {
    let (mut h, ob, group) = host();
    add_nodes(&mut h, ob, group, 2.0);
    h.edit_obj(ob, |o| o.modifiers[0].show_viewport = false);
    let a = EvalCache::new().apply_modifier(&h, ob, 0).unwrap();
    let m = a.mesh.unwrap();
    assert_eq!(m.sharp_edges().unwrap(), &[false; 12][..]);
    // Disabled (no group): `None`.
    h.edit_obj(ob, |o| {
        if let ModifierSettings::Nodes(s) = &mut o.modifiers[0].settings {
            s.node_group = None;
        }
    });
    assert!(
        EvalCache::new()
            .apply_modifier(&h, ob, 0)
            .unwrap()
            .mesh
            .is_none()
    );
    // A type without an evaluator cannot be applied.
    h.edit_obj(ob, |o| {
        edit::add(o, None, ModifierType::Remesh).unwrap();
    });
    assert!(EvalCache::new().apply_modifier(&h, ob, 1).is_err());
}
