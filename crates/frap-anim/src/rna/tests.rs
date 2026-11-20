// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding checks: every bound identifier exists in the generated definitions; reads and
//! writes through the proxies.

use frap_rna::*;

use super::action::{self, Loc, locate};
use super::anim_data;
use crate::action::Action;
use crate::anim_data::{AnimData, NlaStrip, NlaTrack};

#[test]
fn bound_properties_exist_in_the_definitions() {
    for (s, props) in action::BOUND
        .iter()
        .chain(anim_data::BOUND)
        .chain(anim_data::WRITABLE)
    {
        let def = frap_spec_rna::find_struct(s).unwrap_or_else(|| panic!("struct {s}"));
        for p in *props {
            assert!(
                def.find_prop(p).is_some(),
                "{s}.{p} is not in the definitions"
            );
        }
    }
}

fn prop(s: &'static StructDef, p: &str) -> &'static PropDef {
    s.find_prop(p)
        .unwrap_or_else(|| panic!("{}.{p}", s.identifier))
}

fn sample() -> Action {
    let mut a = Action::new("A");
    let h = a.slot_add_for_id_type("OBJECT", "Cube");
    let bag = a.channelbag_ensure(h);
    let f = bag.fcurve_create("location", 1, Some("Object Transforms"));
    let fc = &mut bag.fcurves[f];
    for (t, v) in [(1.0, 0.0), (10.0, 5.0)] {
        crate::fcurve::keys::insert_vert(fc, [t, v], &Default::default(), Default::default());
    }
    a
}

#[test]
fn reads_through_the_proxies() {
    let a = sample();
    let path = [
        Seg::Prop("layers"),
        Seg::Key("Layer".into()),
        Seg::Prop("strips"),
        Seg::Index(0),
        Seg::Prop("channelbags"),
        Seg::Index(0),
        Seg::Prop("fcurves"),
        Seg::Index(0),
        Seg::Prop("keyframe_points"),
        Seg::Index(1),
    ];
    let (loc, coll) = locate(&a, &path).expect("located");
    assert_eq!(coll, None);
    assert_eq!(loc, Loc::Key(0, 0, 0, 0, 1));
    let co = prop(&frap_spec_rna::KEYFRAME, "co");
    match action::get(&a, loc, co) {
        Ok(Field::Value(Value::FloatArray { data, .. })) => assert_eq!(&data[..], &[10.0, 5.0]),
        _ => panic!("co"),
    }
    let (fc, coll) = locate(&a, &path[..8]).expect("fcurve");
    assert_eq!(coll, None);
    let group = prop(&frap_spec_rna::F_CURVE, "group");
    let Ok(Field::Struct(g)) = action::get(&a, fc, group) else {
        panic!("group");
    };
    let name = prop(&frap_spec_rna::ACTION_GROUP, "name");
    assert!(matches!(g.get(name), Ok(Field::Value(Value::Str(n))) if n == "Object Transforms"));
    let (_, coll) = locate(&a, &path[..7]).expect("fcurves");
    assert_eq!(coll, Some("fcurves"));
    let slot = [Seg::Prop("slots"), Seg::Key("OBCube".into())];
    assert_eq!(locate(&a, &slot).map(|l| l.0), Some(Loc::Slot(0)));
}

#[test]
fn writes_with_side_effects() {
    let mut a = sample();
    let fc = Loc::FCurve(0, 0, 0, 0);
    // `extrapolation` recalculates the handles: no flat ends with LINEAR extrapolation.
    let ex = prop(&frap_spec_rna::F_CURVE, "extrapolation");
    assert!(matches!(
        action::set(&mut a, fc, ex, Value::Enum("LINEAR".into())),
        Ok(SetOutcome::Done)
    ));
    let k0 = &a.layers[0].strips[0];
    let crate::action::ActionStrip::Keyframe(k) = k0;
    assert_ne!(
        k.channelbags[0].fcurves[0].keyframe_points[0].handle_right[1],
        0.0
    );
    let co_ui = prop(&frap_spec_rna::KEYFRAME, "co_ui");
    let v = Value::FloatArray {
        data: [2.0_f32, 1.0].into_iter().collect(),
        shape: co_ui.array,
        tag: MathTag::None,
    };
    let before = {
        let crate::action::ActionStrip::Keyframe(k) = &a.layers[0].strips[0];
        k.channelbags[0].fcurves[0].keyframe_points[0].handle_left
    };
    action::set(&mut a, Loc::Key(0, 0, 0, 0, 0), co_ui, v).expect("co_ui");
    let crate::action::ActionStrip::Keyframe(k) = &a.layers[0].strips[0];
    let key = &k.channelbags[0].fcurves[0].keyframe_points[0];
    assert_eq!(key.co, [2.0, 1.0]);
    assert_eq!(key.handle_left, [before[0] + 1.0, before[1] + 1.0]);
    let name = prop(&frap_spec_rna::ACTION_SLOT, "identifier");
    assert!(matches!(
        action::set(&mut a, Loc::Slot(0), name, Value::Str("OBX".into())),
        Ok(SetOutcome::NeedsHook)
    ));
}

#[test]
fn nla_strip_setters_clamp_like_blender() {
    let mut adt = AnimData::default();
    let mut t = NlaTrack::new("NlaTrack");
    t.strips
        .push(NlaStrip::new_clip("A", None, 1.0, [1.0, 11.0]));
    t.strips
        .push(NlaStrip::new_clip("B", None, 20.0, [1.0, 11.0]));
    adt.nla_tracks.push(t);
    let p = |n| prop(&frap_spec_rna::NLA_STRIP, n);
    let loc = anim_data::AdtLoc::Strip(0, 0);
    // The start cannot pass the previous strip, the end not the next one.
    anim_data::set(&mut adt, loc, p("frame_end"), Value::Float(25.0)).expect("end");
    assert_eq!(adt.nla_tracks[0].strips[0].frame_end, 20.0);
    // Scaling pushes the next strip by whole frames.
    anim_data::set(&mut adt, loc, p("scale"), Value::Float(2.5)).expect("scale");
    assert_eq!(adt.nla_tracks[0].strips[0].frame_end, 26.0);
    assert_eq!(adt.nla_tracks[0].strips[1].frame_start, 26.0);
    anim_data::set(&mut adt, loc, p("blend_in"), Value::Float(100.0)).expect("blend in");
    assert_eq!(adt.nla_tracks[0].strips[0].blend_in, 25.0);
    let solo = prop(&frap_spec_rna::NLA_TRACK, "is_solo");
    adt.nla_tracks.push(NlaTrack::new("T2"));
    anim_data::set(
        &mut adt,
        anim_data::AdtLoc::Track(0),
        solo,
        Value::Bool(true),
    )
    .expect("solo");
    anim_data::set(
        &mut adt,
        anim_data::AdtLoc::Track(1),
        solo,
        Value::Bool(true),
    )
    .expect("solo");
    assert!(!adt.nla_tracks[0].is_solo && adt.nla_tracks[1].is_solo);
}

/// `rna_NlaStrip_transform_update` refits blends and revalidates auto-blends; the raw extents
/// store any value. Expected values from Blender 5.2.4 (an action keyed over 1..21, strip S
/// on the first track, strip U on a second one).
#[test]
fn nla_strip_transform_update_matches_blender() {
    let mut adt = AnimData::default();
    let mut t = NlaTrack::new("NlaTrack");
    t.strips
        .push(NlaStrip::new_clip("S", None, 1.0, [1.0, 21.0]));
    adt.nla_tracks.push(t);
    let p = |n| prop(&frap_spec_rna::NLA_STRIP, n);
    let (s, u) = (
        anim_data::AdtLoc::Strip(0, 0),
        anim_data::AdtLoc::Strip(1, 0),
    );
    let set = |adt: &mut AnimData, loc, n, v: f32| {
        anim_data::set(adt, loc, p(n), Value::Float(v)).expect(n);
    };
    let flag = |adt: &mut AnimData, loc, n, v: bool| {
        anim_data::set(adt, loc, p(n), Value::Bool(v)).expect(n);
    };
    let ext = |adt: &AnimData, loc| {
        let anim_data::AdtLoc::Strip(t, i) = loc else {
            unreachable!()
        };
        let x = &adt.nla_tracks[t].strips[i];
        [x.frame_start, x.frame_end, x.blend_in, x.blend_out]
    };
    set(&mut adt, s, "blend_in", 8.0);
    set(&mut adt, s, "blend_out", 8.0);
    assert_eq!(ext(&adt, s), [1.0, 21.0, 8.0, 8.0], "A");
    set(&mut adt, s, "scale", 0.25);
    assert_eq!(ext(&adt, s), [1.0, 6.0, 0.0, 5.0], "B");
    set(&mut adt, s, "frame_end", 3.0);
    assert_eq!(ext(&adt, s), [1.0, 3.0, 0.0, 2.0], "C");
    set(&mut adt, s, "frame_start_raw", 50.0);
    assert_eq!(ext(&adt, s), [50.0, 3.0, 0.0, -47.0], "D");
    set(&mut adt, s, "frame_end_raw", -10.0);
    assert_eq!(ext(&adt, s), [50.0, -10.0, 0.0, 0.0], "E");

    let mut t2 = NlaTrack::new("NlaTrack.001");
    t2.strips
        .push(NlaStrip::new_clip("U", None, 10.0, [1.0, 21.0]));
    adt.nla_tracks.push(t2);
    assert_eq!(ext(&adt, u), [10.0, 30.0, 0.0, 0.0], "U0");
    flag(&mut adt, u, "use_auto_blend", true);
    assert_eq!(ext(&adt, u), [10.0, 30.0, 0.0, 0.0], "U1");
    set(&mut adt, s, "frame_end_raw", 15.0);
    set(&mut adt, s, "frame_start_raw", 5.0);
    flag(&mut adt, s, "use_auto_blend", true);
    assert_eq!(ext(&adt, s), [5.0, 15.0, 0.0, 5.0], "S1");
    assert_eq!(ext(&adt, u), [10.0, 30.0, 5.0, 0.0], "U2");
    set(&mut adt, u, "frame_start_ui", 12.0);
    assert_eq!(ext(&adt, s), [5.0, 15.0, 0.0, 3.0], "S2");
    assert_eq!(ext(&adt, u), [12.0, 32.0, 3.0, 0.0], "U3");
    set(&mut adt, u, "scale", 0.5);
    assert_eq!(ext(&adt, s), [5.0, 15.0, 0.0, 3.0], "S3");
    assert_eq!(ext(&adt, u), [12.0, 22.0, 3.0, 0.0], "U4");
    set(&mut adt, s, "repeat", 3.0);
    assert_eq!(ext(&adt, s), [5.0, 20.0, 0.0, 8.0], "S4");
    assert_eq!(ext(&adt, u), [12.0, 22.0, 8.0, 0.0], "U5");
    set(&mut adt, u, "blend_in", 1.5);
    set(&mut adt, u, "frame_end", 40.0);
    assert_eq!(ext(&adt, u), [12.0, 40.0, 8.0, 0.0], "U6");
    flag(&mut adt, u, "use_animated_time_cyclic", true);
    assert_eq!(ext(&adt, u), [12.0, 40.0, 8.0, 0.0], "U7");
}
