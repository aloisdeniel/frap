// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.6 / WP 5.3 acceptance: every view transform × look (on every SDR display) within
//! ΔE < 1 of Blender 5.2.4 on the **held-out** colour set (`assets/luts/heldout.json`, written
//! by `tools/blender/bake_view_luts.py` from `Image.save_render`), CPU only (ARCHITECTURE §19.5).
//!
//! ΔE is CIEDE2000 on the display values decoded with the display's EOTF and primaries (the
//! gate); CIE76 is printed alongside. Exposure and gamma variants are included.

use frap_color::delta_e::{Lab, delta_e_76, delta_e_2000, xyz_to_lab};
use frap_color::spaces::srgb_to_linear;
use frap_color::{DisplayDevice, DisplayProcessor, DisplaySettings, ViewSettings, ViewTransform};
use serde_json::Value;

const HELDOUT: &str = include_str!("../../../assets/luts/heldout.json");

/// RGB → XYZ (D65) of the display primaries (`config.ocio`: the inverses of its XYZ → linear
/// matrices for Rec.709, DCI-P3 D65 and Rec.2020).
fn primaries_to_xyz(d: DisplayDevice) -> [[f64; 3]; 3] {
    let inv = |m: [[f64; 3]; 3]| {
        let [[a, b, c], [d, e, f], [g, h, i]] = m;
        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        [
            [
                (e * i - f * h) / det,
                (c * h - b * i) / det,
                (b * f - c * e) / det,
            ],
            [
                (f * g - d * i) / det,
                (a * i - c * g) / det,
                (c * d - a * f) / det,
            ],
            [
                (d * h - e * g) / det,
                (b * g - a * h) / det,
                (a * e - b * d) / det,
            ],
        ]
    };
    match d {
        DisplayDevice::DisplayP3 => inv([
            [2.4934969, -0.9313836, -0.4027108],
            [-0.8294890, 1.7626641, 0.0236247],
            [0.0358458, -0.0761724, 0.9568845],
        ]),
        DisplayDevice::Rec2020 => inv([
            [1.7166512, -0.3556708, -0.2533663],
            [-0.6666844, 1.6164812, 0.0157685],
            [0.0176399, -0.0427706, 0.9421031],
        ]),
        _ => inv([
            [3.2409699, -1.5373832, -0.4986108],
            [-0.9692436, 1.8759675, 0.0415551],
            [0.0556301, -0.2039770, 1.0569715],
        ]),
    }
}

/// Display-encoded RGB (clamped as a file stores it) → CIELAB.
fn display_to_lab(d: DisplayDevice, rgb: [f32; 3]) -> Lab {
    let lin = rgb.map(|v| {
        let v = v.clamp(0.0, 1.0);
        f64::from(match d {
            DisplayDevice::Srgb | DisplayDevice::DisplayP3 => srgb_to_linear(v),
            _ => libm::powf(v, 2.4),
        })
    });
    let m = primaries_to_xyz(d);
    xyz_to_lab([0, 1, 2].map(|i| m[i][0] * lin[0] + m[i][1] * lin[1] + m[i][2] * lin[2]))
}

struct Stats {
    max00: f64,
    mean00: f64,
    max76: f64,
    worst: String,
}

fn run_case(inputs: &[[f32; 3]], case: &Value) -> (String, ViewTransform, String, Stats) {
    let display = DisplayDevice::from_name(case["display_device"].as_str().unwrap()).unwrap();
    let view = ViewSettings {
        view_transform: ViewTransform::from_name(case["view_transform"].as_str().unwrap()).unwrap(),
        look: case["look"].as_str().unwrap().to_owned(),
        exposure: case["exposure"].as_f64().unwrap() as f32,
        gamma: case["gamma"].as_f64().unwrap() as f32,
    };
    let label = format!(
        "{} | {} | {} exposure={} gamma={}",
        display.name(),
        view.view_transform.name(),
        view.look,
        view.exposure,
        view.gamma
    );
    let p = DisplayProcessor::new(
        &view,
        &DisplaySettings {
            display_device: display,
        },
    )
    .unwrap_or_else(|e| panic!("{label}: {e}"));
    let outs = case["outputs_u16"].as_array().unwrap();
    assert_eq!(outs.len(), inputs.len());
    let mut s = Stats {
        max00: 0.0,
        mean00: 0.0,
        max76: 0.0,
        worst: String::new(),
    };
    for (inp, out) in inputs.iter().zip(outs) {
        let b: Vec<f32> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as f32 / 65535.0)
            .collect();
        let blender = [b[0], b[1], b[2]];
        let frap = p.apply_rgb(*inp).map(|v| v.clamp(0.0, 1.0));
        let (x, y) = (
            display_to_lab(display, frap),
            display_to_lab(display, blender),
        );
        let d00 = delta_e_2000(x, y);
        s.max76 = s.max76.max(delta_e_76(x, y));
        s.mean00 += d00;
        if d00 > s.max00 {
            s.max00 = d00;
            s.worst = format!("in {inp:?}: frap {frap:?} blender {blender:?}");
        }
    }
    s.mean00 /= inputs.len() as f64;
    (label, view.view_transform, view.look, s)
}

#[test]
fn every_view_transform_and_look_matches_blender_on_heldout_colours() {
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
    let mut seen = std::collections::HashSet::new();
    let mut failures = Vec::new();
    for case in doc["cases"].as_array().unwrap() {
        let (label, vt, look, s) = run_case(&inputs, case);
        println!(
            "{label}: ΔE00 max {:.4} mean {:.4}; ΔE76 max {:.4}; worst {}",
            s.max00, s.mean00, s.max76, s.worst
        );
        if case["display_device"] == "sRGB" {
            seen.insert((vt, look));
        }
        if s.max00 >= 1.0 {
            failures.push(format!("{label}: ΔE00 {:.4} ({})", s.max00, s.worst));
        }
    }
    // Coverage: every view transform × every look Blender offers for it, on sRGB.
    for vt in ViewTransform::ALL {
        for look in frap_color::looks::looks_for(vt) {
            assert!(
                seen.contains(&(vt, look.to_owned())),
                "held-out set lacks {} / {look}",
                vt.name()
            );
        }
    }
    assert!(failures.is_empty(), "ΔE ≥ 1:\n{}", failures.join("\n"));
}

#[test]
fn heldout_colours_are_off_the_agx_lattice() {
    // The colours are random HDR values: none lands on a lattice node of the AgX base LUT on
    // all three axes, so interpolation is exercised (nothing is fitted to them).
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
fn single_lut_agx_matches_the_pipeline() {
    // `lut::baked("AgX", "None", "sRGB")` (the M1 single-LUT path) and the op pipeline agree.
    let agx = frap_color::lut::baked("AgX", "None", "sRGB").unwrap();
    let p = DisplayProcessor::new(&ViewSettings::default(), &DisplaySettings::default()).unwrap();
    let doc: Value = serde_json::from_str(HELDOUT).unwrap();
    for c in doc["inputs"].as_array().unwrap() {
        let c = c.as_array().unwrap();
        let rgb = [0, 1, 2].map(|i| c[i].as_f64().unwrap() as f32);
        let (a, b) = (agx.apply(rgb), p.apply_rgb(rgb));
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() < 2e-4, "{rgb:?}: {a:?} vs {b:?}");
        }
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
