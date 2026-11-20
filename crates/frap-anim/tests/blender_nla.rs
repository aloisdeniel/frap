// SPDX-License-Identifier: GPL-3.0-or-later
//! NLA blending against Blender 5.2.4 (WP 6.1 acceptance): `fixtures/blender_nla.json`
//! (written by `blender/dump_nla.py`) holds scenarios of NLA tracks and strips on the default
//! Cube with the values Blender evaluated at whole and fractional frames. This test rebuilds
//! each scenario's actions and `AnimData` and compares `evaluate_anim_for` channel by channel;
//! every sampled channel frap does not write must have kept the object's base value (which
//! differs from the RNA default) in Blender too.

use frap_anim::eval::props::PathSeg;
use frap_anim::eval::{ActionLookup, evaluate_anim, evaluate_anim_for, evaluate_anim_refined};
use frap_anim::{
    Action, ActionGroup, ActionLayer, ActionSlot, ActionStrip, AnimData, Channelbag, Easing,
    Extrapolation, FCurve, Interpolation, Keyframe, KeyframeStrip, NlaStrip, NlaTrack,
};
use frap_rna::{ActionId, IdKey, IdRef};
use serde_json::Value as J;

struct Actions(Vec<(IdRef<ActionId>, Action)>);

impl ActionLookup for Actions {
    fn action(&self, r: IdRef<ActionId>) -> Option<&Action> {
        self.0.iter().find(|(k, _)| *k == r).map(|(_, a)| a)
    }
}

impl Actions {
    fn by_name(&self, name: &J) -> Option<IdRef<ActionId>> {
        let name = name.as_str()?;
        self.0
            .iter()
            .find(|(_, a)| a.header.name == name)
            .map(|(k, _)| *k)
    }
}

fn f(v: &J) -> f32 {
    v.as_f64().expect("number") as f32
}

fn s(v: &J) -> &str {
    v.as_str().expect("string")
}

fn pair(v: &J) -> [f32; 2] {
    [f(&v[0]), f(&v[1])]
}

fn action(d: &J) -> Action {
    let mut a = Action::new(s(&d["name"]));
    for sl in d["slots"].as_array().expect("slots") {
        a.slots.push(ActionSlot::new(
            sl["handle"].as_i64().expect("handle") as i32,
            s(&sl["identifier"]),
            s(&sl["target_id_type"]),
        ));
    }
    let mut strip = KeyframeStrip::default();
    for b in d["channelbags"].as_array().expect("bags") {
        let mut bag = Channelbag {
            slot_handle: b["slot_handle"].as_i64().expect("slot") as i32,
            ..Channelbag::default()
        };
        for c in b["fcurves"].as_array().expect("fcurves") {
            let mut fc = FCurve::new(
                s(&c["data_path"]),
                c["array_index"].as_i64().expect("i") as i32,
            );
            fc.extrapolation = Extrapolation::from_identifier(s(&c["extrapolation"])).expect("ex");
            fc.mute = c["mute"].as_bool().expect("mute");
            fc.group = c["group"].as_str().map(Into::into);
            for k in c["keys"].as_array().expect("keys") {
                let co = pair(&k["co"]);
                fc.keyframe_points.push(Keyframe {
                    handle_left: pair(&k["handle_left"]),
                    handle_right: pair(&k["handle_right"]),
                    interpolation: Interpolation::from_identifier(s(&k["interpolation"]))
                        .expect("ipo"),
                    easing: Easing::from_identifier(s(&k["easing"])).expect("easing"),
                    ..Keyframe::new(co[0], co[1])
                });
            }
            bag.fcurves.push(fc);
        }
        for g in b["groups"].as_array().expect("groups") {
            let mut grp = ActionGroup::new(s(&g["name"]));
            grp.mute = g["mute"].as_bool().expect("mute");
            bag.groups.push(grp);
        }
        strip.channelbags.push(bag);
    }
    let mut layer = ActionLayer::new("Layer");
    layer.strips.push(ActionStrip::Keyframe(strip));
    a.layers.push(layer);
    a
}

fn anim_data(d: &J, actions: &Actions) -> AnimData {
    let mut adt = AnimData {
        action: actions.by_name(&d["action"]),
        action_slot_handle: d["action_slot_handle"].as_i64().expect("h") as i32,
        use_nla: d["use_nla"].as_bool().expect("use_nla"),
        action_influence: f(&d["action_influence"]),
        action_blend_type: s(&d["action_blend_type"]).into(),
        action_extrapolation: s(&d["action_extrapolation"]).into(),
        ..AnimData::default()
    };
    for t in d["tracks"].as_array().expect("tracks") {
        let mut track = NlaTrack::new(s(&t["name"]));
        track.mute = t["mute"].as_bool().expect("mute");
        track.is_solo = t["is_solo"].as_bool().expect("solo");
        for x in t["strips"].as_array().expect("strips") {
            let mut st = NlaStrip::new_clip(
                s(&x["name"]),
                actions.by_name(&x["action"]),
                0.0,
                [0.0, 1.0],
            );
            st.r#type = s(&x["type"]).into();
            st.extrapolation = s(&x["extrapolation"]).into();
            st.blend_type = s(&x["blend_type"]).into();
            st.frame_start = f(&x["frame_start"]);
            st.frame_end = f(&x["frame_end"]);
            st.action_frame_start = f(&x["action_frame_start"]);
            st.action_frame_end = f(&x["action_frame_end"]);
            st.scale = f(&x["scale"]);
            st.repeat = f(&x["repeat"]);
            st.blend_in = f(&x["blend_in"]);
            st.blend_out = f(&x["blend_out"]);
            st.influence = f(&x["influence"]);
            st.use_animated_influence = x["use_animated_influence"].as_bool().expect("b");
            st.strip_time = f(&x["strip_time"]);
            st.use_animated_time = x["use_animated_time"].as_bool().expect("b");
            st.mute = x["mute"].as_bool().expect("b");
            st.use_reverse = x["use_reverse"].as_bool().expect("b");
            st.action_slot_handle = x["action_slot_handle"].as_i64().expect("h") as i32;
            track.strips.push(st);
        }
        adt.nla_tracks.push(track);
    }
    adt
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn nla_matches_blender() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/blender_nla.json"
    ))
    .expect("fixture");
    let doc: J = serde_json::from_str(&text).expect("json");
    let mut failures = Vec::new();
    let mut compared = 0;
    let mut untouched = 0;
    for sc in doc["scenarios"].as_array().expect("scenarios") {
        let name = s(&sc["name"]);
        let actions = Actions(
            sc["actions"]
                .as_array()
                .expect("actions")
                .iter()
                .map(|a| (IdRef::from_key(IdKey::next()), action(a)))
                .collect(),
        );
        let adt = anim_data(&sc["animation_data"], &actions);
        for smp in sc["samples"].as_array().expect("samples") {
            let frame = f(&smp[0]);
            let got = evaluate_anim_for(&adt, &actions, frame, Some(&frap_spec_rna::OBJECT));
            assert_eq!(
                got,
                evaluate_anim(&adt, &actions, frame),
                "{name}: root from the slots"
            );
            if got.is_empty() {
                failures.push(format!("{name} @ {frame}: nothing evaluated"));
            }
            for ch in &got {
                let want = &smp[1][ch.data_path.as_str()][ch.array_index as usize];
                let w = f(want);
                compared += 1;
                if !close(ch.value, w) {
                    failures.push(format!(
                        "{name} @ {frame}: {}[{}] = {}, Blender {w}",
                        ch.data_path, ch.array_index, ch.value
                    ));
                }
            }
            // Channels frap leaves alone: Blender must not have written them either.
            for (path, values) in smp[1].as_object().expect("channels") {
                for (i, v) in values.as_array().expect("values").iter().enumerate() {
                    if got
                        .iter()
                        .any(|c| c.data_path == path.as_str() && c.array_index as usize == i)
                    {
                        continue;
                    }
                    let base = f(&sc["base"][path.as_str()][i]);
                    untouched += 1;
                    if !close(f(v), base) {
                        failures.push(format!(
                            "{name} @ {frame}: {path}[{i}] not written by frap, Blender {} \
                             (base {base})",
                            f(v)
                        ));
                    }
                }
            }
        }
    }
    assert!(compared > 2000, "only {compared} values compared");
    assert!(
        untouched > 2000,
        "only {untouched} untouched values checked"
    );
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures[..failures.len().min(60)].join("\n")
    );
}

/// A channel under a refined item (`modifiers["Bevel"]` is a `BevelModifier`) gets its real RNA
/// default where no strip writes it. Blender 5.2.4: a Bevel modifier (width 0.4) whose `width`
/// (keys 1: 0.7, 11: 0.9) and `segments` (1: 3) are animated only by a strip at frame 10 with
/// extrapolation NOTHING reads width 0.1 / segments 1 at frames 5 and 25, 0.7 / 3 at 10 and
/// 0.8 / 3 at 15.
#[test]
fn nla_refined_item_defaults_match_blender() {
    let key = IdRef::from_key(IdKey::next());
    let mut a = Action::new("A");
    a.slots.push(ActionSlot::new(1, "OBCube", "OBJECT"));
    let mut bag = Channelbag {
        slot_handle: 1,
        ..Channelbag::default()
    };
    let mut width = FCurve::new("modifiers[\"Bevel\"].width", 0);
    width.keyframe_points.push(Keyframe::new(1.0, 0.7));
    width.keyframe_points.push(Keyframe::new(11.0, 0.9));
    frap_anim::eval::recalc_handles(&mut width);
    let mut segments = FCurve::new("modifiers[\"Bevel\"].segments", 0);
    segments.keyframe_points.push(Keyframe::new(1.0, 3.0));
    bag.fcurves.extend([width, segments]);
    let mut strip = KeyframeStrip::default();
    strip.channelbags.push(bag);
    let mut layer = ActionLayer::new("L");
    layer.strips.push(ActionStrip::Keyframe(strip));
    a.layers.push(layer);
    let actions = Actions(vec![(key, a)]);
    let mut adt = AnimData::default();
    let mut track = NlaTrack::new("NlaTrack");
    let mut st = NlaStrip::new_clip("S", Some(key), 10.0, [1.0, 11.0]);
    st.extrapolation = "NOTHING".into();
    st.action_slot_handle = 1;
    track.strips.push(st);
    adt.nla_tracks.push(track);
    let refine = |prefix: &[PathSeg], base: &'static frap_rna::StructDef| {
        (base.identifier == "Modifier" && prefix.last() == Some(&PathSeg::Key("Bevel".into())))
            .then(|| frap_spec_rna::find_struct("BevelModifier"))
            .flatten()
    };
    for (frame, w, seg) in [
        (5.0, 0.1, 1.0),
        (10.0, 0.7, 3.0),
        (15.0, 0.8, 3.0),
        (25.0, 0.1, 1.0),
    ] {
        let got = evaluate_anim_refined(
            &adt,
            &actions,
            frame,
            Some(&frap_spec_rna::OBJECT),
            Some(&refine),
        );
        let val = |p: &str| {
            got.iter()
                .find(|c| c.data_path.ends_with(p))
                .unwrap_or_else(|| panic!("{p} @ {frame}: {got:?}"))
                .value
        };
        assert!(close(val(".width"), w), "width @ {frame}: {got:?}");
        assert!(
            close(val(".segments").round(), seg),
            "segments @ {frame}: {got:?}"
        );
    }
}
