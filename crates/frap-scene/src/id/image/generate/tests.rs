// SPDX-License-Identifier: GPL-3.0-or-later
//! Generated buffers and `Image.scale()` against Blender 5.2.4 dumps
//! (`testdata/blender_5_2_4.txt`, written by `bpy.ops.image.new` / `Image.scale` and read back
//! through `Image.pixels`).

use super::super::imbuf::{ImBuf, ImBufData};
use super::*;

/// One dumped buffer.
struct Ref {
    name: &'static str,
    w: usize,
    h: usize,
    float: bool,
    extra: &'static str,
    bytes: Vec<u8>,
}

impl Ref {
    fn floats(&self) -> Vec<f32> {
        self.bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }
    /// The dumped samples as floats (`Image.pixels` values).
    fn samples(&self) -> Vec<f32> {
        if self.float {
            self.floats()
        } else {
            self.bytes
                .iter()
                .map(|&b| f32::from(b) * (1.0 / 255.0))
                .collect()
        }
    }
}

fn refs() -> Vec<Ref> {
    include_str!("../testdata/blender_5_2_4.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            let hex = f[5];
            Ref {
                name: f[0],
                w: f[1].parse().unwrap(),
                h: f[2].parse().unwrap(),
                float: f[3] == "f",
                extra: f[4],
                bytes: (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}

fn get(name: &str) -> Ref {
    refs()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no reference {name}"))
}

/// Pixels of `buf` inside `crop` (`x0,y0,x1,y1`, or `-` for all), as `Image.pixels` reads them.
fn cropped(buf: &ImBuf, crop: &str) -> Vec<f32> {
    let px = buf.pixels();
    if crop == "-" {
        return px;
    }
    let c: Vec<usize> = crop.split(',').map(|v| v.parse().unwrap()).collect();
    let mut out = Vec::new();
    for y in c[1]..c[3] {
        for x in c[0]..c[2] {
            let i = (y * buf.x as usize + x) * 4;
            out.extend_from_slice(&px[i..i + 4]);
        }
    }
    out
}

fn compare(name: &str, buf: &ImBuf, r: &Ref, crop: &str) {
    assert_eq!((buf.x as usize, buf.y as usize), (r.w, r.h), "{name}: size");
    assert_eq!(buf.is_float(), r.float, "{name}: float");
    let got = cropped(buf, crop);
    let want = r.samples();
    assert_eq!(got.len(), want.len(), "{name}: sample count");
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        let ok = if r.float {
            // `powf` may differ by an ulp between C libraries; Blender's float sRGB → linear
            // goes through OCIO for some buffers.
            (g - w).abs() <= 2e-6 + 2e-6 * w.abs()
        } else {
            g == w
        };
        assert!(
            ok,
            "{name}: sample {i} (pixel {}, channel {}): frap {g} vs Blender {w}",
            i / 4,
            i % 4
        );
    }
}

fn op_new(kind: &str, w: i32, h: i32, float: bool, color: [f32; 4], alpha: bool) -> ImBuf {
    let mut color = color;
    if !alpha {
        color[3] = 1.0;
    }
    generate(&GenParams {
        kind,
        width: w,
        height: h,
        float,
        depth: if alpha { 32 } else { 24 },
        color,
        is_data: false,
    })
}

#[test]
fn uv_grid_matches_blender() {
    for (name, float) in [("uv_b", false), ("uv_f", true)] {
        let r = get(name);
        let buf = op_new("UV_GRID", r.w as i32, r.h as i32, float, [0.0; 4], true);
        compare(name, &buf, &r, "-");
    }
}

#[test]
fn color_grid_matches_blender_outside_the_labels() {
    for (name, float) in [
        ("cg_b", false),
        ("cg_f", true),
        ("cg_b_big", false),
        ("cg_b_big2", false),
    ] {
        let r = get(name);
        let buf = op_new("COLOR_GRID", r.w as i32, r.h as i32, float, [0.0; 4], true);
        compare(name, &buf, &r, r.extra);
    }
}

#[test]
fn blank_matches_blender() {
    let c = [0.5, 0.25, 0.1, 0.8];
    compare(
        "blank_b",
        &op_new("BLANK", 3, 2, false, c, true),
        &get("blank_b"),
        "-",
    );
    compare(
        "blank_f",
        &op_new("BLANK", 3, 2, true, c, true),
        &get("blank_f"),
        "-",
    );
    compare(
        "blank_f_noalpha",
        &op_new("BLANK", 3, 2, true, [0.5, 0.25, 1.5, 0.8], false),
        &get("blank_f_noalpha"),
        "-",
    );
    // `images.new(float_buffer=True, is_data=True)` + `generated_color`: not linearised.
    let data = generate(&GenParams {
        kind: "BLANK",
        width: 2,
        height: 1,
        float: true,
        depth: 24,
        color: [0.5, 0.25, 0.1, 0.8],
        is_data: true,
    });
    compare("data_f", &data, &get("data_f"), "-");
}

#[test]
fn box_scale_matches_blender() {
    for r in refs().into_iter().filter(|r| r.name.starts_with("sc_")) {
        let src: Vec<usize> = r.extra.split(',').map(|v| v.parse().unwrap()).collect();
        let (w, h) = (src[0] as u32, src[1] as u32);
        let mut buf = ImBuf::new(w, h, 32, r.float);
        let n = buf.len();
        // Python floats are doubles: `((i * 37) % 101) / 100.0` rounded to f32 on store.
        let values: Vec<f32> = (0..n)
            .map(|i| (((i * 37) % 101) as f64 / 100.0) as f32)
            .collect();
        buf.set_pixels(&values);
        assert!(buf.scale(r.w as u32, r.h as u32), "{}: scaled", r.name);
        compare(r.name, &buf, &r, "-");
    }
}

#[test]
fn scale_rejects_degenerate_sizes() {
    let mut b = ImBuf::new(2, 2, 32, false);
    assert!(!b.scale(2, 2));
    assert!(!b.scale(0, 3));
    assert_eq!((b.x, b.y), (2, 2));
    if let ImBufData::Byte(d) = &b.data {
        assert_eq!(d.len(), 16);
    }
}
