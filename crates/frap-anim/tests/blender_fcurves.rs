// SPDX-License-Identifier: GPL-3.0-or-later
//! F-curve sampling and handle recalculation against Blender 5.2.4 (WP 6.1 acceptance: every
//! interpolation, easing, handle type, extrapolation and smoothing mode).
//!
//! `fixtures/blender_fcurves.json` is written by `blender/dump_fcurves.py`: each curve's spec
//! (keys, curve and key properties, in the order the script applies them), the keys Blender
//! ended up with, and the value sampled every quarter frame. This test builds the same curves
//! with frap-anim (`keys::insert_vert`, the property writes, `keys::update`) and compares.

use frap_anim::eval::evaluate_fcurve;
use frap_anim::fcurve::keys::{self, InsertFlags, KeyframeSettings};
use frap_anim::{Easing, Extrapolation, FCurve, HandleType, Interpolation};
use serde_json::Value as J;

fn f(v: &J) -> f32 {
    v.as_f64().expect("number") as f32
}

fn pair(v: &J) -> [f32; 2] {
    [f(&v[0]), f(&v[1])]
}

/// Absolute below 1, relative above (the golden harness's `tol`).
fn close(a: f32, b: f32, tol: f32) -> bool {
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= tol * scale
}

fn build(case: &J) -> FCurve {
    let mut fc = FCurve::new("location", 0);
    if let Some(s) = case["auto_smoothing"].as_str() {
        fc.auto_smoothing = s.into();
        keys::update(&mut fc);
    }
    if let Some(e) = case["extrapolation"].as_str() {
        fc.extrapolation = Extrapolation::from_identifier(e).expect("extrapolation");
        keys::update(&mut fc);
    }
    let insert = |fc: &mut FCurve, k: &J| {
        keys::insert_vert(
            fc,
            pair(k),
            &KeyframeSettings::default(),
            InsertFlags::default(),
        );
    };
    for k in case["keys"].as_array().expect("keys") {
        insert(&mut fc, k);
    }
    let props = case["key_props"].as_array().cloned().unwrap_or_default();
    for (key, p) in fc.keyframe_points.iter_mut().zip(&props) {
        for (name, v) in p.as_object().expect("props") {
            let s = || v.as_str().expect("enum");
            match name.as_str() {
                "interpolation" => {
                    key.interpolation = Interpolation::from_identifier(s()).expect("ipo")
                }
                "easing" => key.easing = Easing::from_identifier(s()).expect("easing"),
                "handle_left_type" => {
                    key.handle_left_type = HandleType::from_identifier(s()).expect("h")
                }
                "handle_right_type" => {
                    key.handle_right_type = HandleType::from_identifier(s()).expect("h")
                }
                "back" => key.back = f(v),
                "amplitude" => key.amplitude = f(v),
                "period" => key.period = f(v),
                "handle_left" => key.handle_left = pair(v),
                "handle_right" => key.handle_right = pair(v),
                other => panic!("unknown key property {other}"),
            }
        }
    }
    if !props.is_empty() {
        keys::update(&mut fc);
    }
    for k in case["then_insert"].as_array().cloned().unwrap_or_default() {
        insert(&mut fc, &k);
    }
    fc
}

#[test]
fn fcurves_match_blender() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/blender_fcurves.json"
    ))
    .expect("fixture");
    let doc: J = serde_json::from_str(&text).expect("json");
    let tol = 1e-5;
    let mut failures = Vec::new();
    let curves = doc["curves"].as_array().expect("curves");
    assert!(curves.len() > 80, "the fixture covers every case");
    for c in curves {
        let case = &c["case"];
        let name = case["name"].as_str().expect("name");
        let fc = build(case);
        let want = c["keys"].as_array().expect("keys");
        if want.len() != fc.keyframe_points.len() {
            failures.push(format!(
                "{name}: {} keys, Blender {}",
                fc.keyframe_points.len(),
                want.len()
            ));
            continue;
        }
        for (i, (k, w)) in fc.keyframe_points.iter().zip(want).enumerate() {
            for (what, got, exp) in [
                ("co", k.co, pair(&w["co"])),
                ("handle_left", k.handle_left, pair(&w["handle_left"])),
                ("handle_right", k.handle_right, pair(&w["handle_right"])),
            ] {
                if !(close(got[0], exp[0], tol) && close(got[1], exp[1], tol)) {
                    failures.push(format!("{name}: key {i} {what} {got:?}, Blender {exp:?}"));
                }
            }
            for (what, got, exp) in [
                (
                    "handle_left_type",
                    k.handle_left_type.identifier(),
                    &w["handle_left_type"],
                ),
                (
                    "handle_right_type",
                    k.handle_right_type.identifier(),
                    &w["handle_right_type"],
                ),
                (
                    "interpolation",
                    k.interpolation.identifier(),
                    &w["interpolation"],
                ),
            ] {
                if Some(got) != exp.as_str() {
                    failures.push(format!("{name}: key {i} {what} {got}, Blender {exp}"));
                }
            }
        }
        for s in c["samples"].as_array().expect("samples") {
            let (t, v) = (f(&s[0]), f(&s[1]));
            let got = evaluate_fcurve(&fc, t);
            if !close(got, v, tol) {
                failures.push(format!("{name}: f({t}) = {got}, Blender {v}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures[..failures.len().min(60)].join("\n")
    );
}
