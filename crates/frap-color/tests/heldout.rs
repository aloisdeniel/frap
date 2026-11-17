// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.6 acceptance: Standard and AgX within ΔE < 1 of Blender 5.2.4 on the **held-out** colour
//! set (`assets/luts/heldout.json`, written by `tools/blender/bake_view_luts.py`), never on the
//! lattice the LUT was fitted to (ARCHITECTURE §19.5). CPU only.
//!
//! ΔE is CIEDE2000 on the sRGB display values (the gate); CIE76 is printed alongside.

use frap_color::delta_e::{delta_e_76, delta_e_2000, srgb_to_lab};
use frap_color::{DisplayProcessor, DisplaySettings, ViewSettings, ViewTransform};
use serde_json::Value;

const HELDOUT: &str = include_str!("../../../assets/luts/heldout.json");

struct Stats {
    max00: f64,
    mean00: f64,
    max76: f64,
    worst: String,
}

fn run_case(inputs: &[[f32; 3]], case: &Value) -> (String, Stats) {
    let view = ViewSettings {
        view_transform: ViewTransform::from_name(case["view_transform"].as_str().unwrap()).unwrap(),
        look: case["look"].as_str().unwrap().to_owned(),
        exposure: case["exposure"].as_f64().unwrap() as f32,
        gamma: case["gamma"].as_f64().unwrap() as f32,
    };
    assert_eq!(case["display_device"], "sRGB");
    let label = format!(
        "{} look={} exposure={} gamma={}",
        view.view_transform.name(),
        view.look,
        view.exposure,
        view.gamma
    );
    let p = DisplayProcessor::new(&view, &DisplaySettings::default()).unwrap();
    let outs = case["outputs_u16"].as_array().unwrap();
    assert_eq!(outs.len(), inputs.len());
    let mut s = Stats {
        max00: 0.0,
        mean00: 0.0,
        max76: 0.0,
        worst: String::new(),
    };
    for (inp, out) in inputs.iter().zip(outs) {
        let blender: Vec<f32> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as f32 / 65535.0)
            .collect();
        let blender = [blender[0], blender[1], blender[2]];
        // Files clamp to [0, 1]; compare what would be written.
        let frap = p.apply_rgb(*inp).map(|v| v.clamp(0.0, 1.0));
        let (a, b) = (srgb_to_lab(frap), srgb_to_lab(blender));
        let d00 = delta_e_2000(a, b);
        s.max76 = s.max76.max(delta_e_76(a, b));
        s.mean00 += d00;
        if d00 > s.max00 {
            s.max00 = d00;
            s.worst = format!("in {inp:?}: frap {frap:?} blender {blender:?}");
        }
    }
    s.mean00 /= inputs.len() as f64;
    (label, s)
}

#[test]
fn view_transforms_match_blender_on_heldout_colours() {
    let doc: Value = serde_json::from_str(HELDOUT).unwrap();
    let inputs: Vec<[f32; 3]> = doc["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let c = c.as_array().unwrap();
            [0, 1, 2].map(|i| c[i].as_f64().unwrap() as f32)
        })
        .collect();
    assert!(inputs.len() >= 300, "held-out set too small");
    let cases = doc["cases"].as_array().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut failures = Vec::new();
    for case in cases {
        let (label, s) = run_case(&inputs, case);
        println!(
            "{label}: ΔE00 max {:.4} mean {:.4}; ΔE76 max {:.4}; worst {}",
            s.max00, s.mean00, s.max76, s.worst
        );
        seen.insert(case["view_transform"].as_str().unwrap().to_owned());
        if s.max00 >= 1.0 {
            failures.push(format!("{label}: ΔE00 {:.4} ({})", s.max00, s.worst));
        }
    }
    for required in ["Standard", "AgX"] {
        assert!(seen.contains(required), "held-out set lacks {required}");
    }
    assert!(failures.is_empty(), "ΔE ≥ 1:\n{}", failures.join("\n"));
}

#[test]
fn heldout_colours_are_off_the_lattice() {
    // The gate must not be measured on fitted points: no held-out input maps to a lattice node
    // of the baked AgX LUT on all three axes.
    let agx = frap_color::lut::baked("AgX", "None", "sRGB").unwrap();
    let n = (agx.lut.size - 1) as f32;
    let doc: Value = serde_json::from_str(HELDOUT).unwrap();
    for c in doc["inputs"].as_array().unwrap() {
        let c: Vec<f32> = c
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        let m = agx.matrix.unwrap();
        let v = frap_color::spaces::mat3_mul(&m, [c[0], c[1], c[2]]);
        // Unclamped: inputs below the shaper floor are clamped by Blender too, but they are not
        // lattice inputs the bake rendered.
        let on_node = v.iter().all(|x| {
            if *x <= 0.0 {
                return false;
            }
            let s = agx.shaper.apply(*x) * n;
            (s - s.round()).abs() < 1e-3
        });
        assert!(!on_node, "held-out colour {c:?} lies on the lattice");
    }
}

#[test]
fn dither_matches_blender_render_output() {
    // A flat Workbench render saved by Blender as an undithered 16-bit PNG and as a dithered
    // 8-bit PNG (render.dither_intensity = 1). frap dithers the 16-bit display values and must
    // produce Blender's bytes, except where the 16-bit rounding of the display value (±0.5/65535)
    // straddles an 8-bit rounding boundary.
    let doc: Value = serde_json::from_str(HELDOUT).unwrap();
    let d = &doc["dither"];
    let w = d["width"].as_u64().unwrap() as u32;
    let h = d["height"].as_u64().unwrap() as u32;
    let intensity = d["intensity"].as_f64().unwrap() as f32;
    let display: Vec<f32> = d["display_u16"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as f32 / 65535.0)
        .collect();
    let blender: Vec<u8> = d["outputs_u8"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    // Blender stores the bottom row first; frap images are top row first.
    let mut rgba = Vec::new();
    for row in (0..h as usize).rev() {
        for x in 0..w as usize {
            let v = display[row * w as usize + x];
            rgba.push([v, v, v, 1.0]);
        }
    }
    let img = frap_color::DisplayImage {
        width: w,
        height: h,
        rgba,
    };
    let frap = img.to_rgba8(intensity);
    let undithered = img.to_rgba8(0.0);
    let mut mismatches = 0;
    let mut flipped = 0;
    for row in 0..h as usize {
        for x in 0..w as usize {
            let i = row * w as usize + x;
            let b = blender[(h as usize - 1 - row) * w as usize + x];
            let f = frap[4 * i];
            if f != undithered[4 * i] {
                flipped += 1;
            }
            if f != b {
                // Tolerated only next to a rounding boundary.
                let v = img.rgba[i][0];
                let noise = frap_color::display::dither_random_value(
                    x as i32,
                    (h as usize - 1 - row) as i32,
                ) * 0.0033
                    * intensity;
                let code = (v + noise) * 255.0 + 0.5;
                let dist = (code - code.round()).abs() / 255.0;
                assert!(
                    dist < 1.0 / 65535.0,
                    "pixel ({x}, {row}): frap {f} blender {b} (distance to boundary {dist})"
                );
                mismatches += 1;
            }
        }
    }
    println!("dither: {flipped} pixels changed by dither, {mismatches} boundary mismatches");
    assert!(
        flipped > (w * h / 10) as usize,
        "the case must exercise the dither"
    );
}
