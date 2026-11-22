// SPDX-License-Identifier: GPL-3.0-or-later
//! The SVG and EPS writers against Blender 5.2.4's add-on (`testdata/make.py` records every
//! writer call of Blender's export: its polygon data in `calls.json`, its file next to it):
//! frap's files are byte-equal except the generator line of the header.

use super::py::round_half_even;
use super::vector::{self, GENERATOR, Poly};
use super::*;

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ops/export_layout/testdata");

/// The lines of `text` without the header's generator line.
fn body(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("<desc>") && !l.starts_with("%%Creator:"))
        .collect()
}

fn polys(call: &serde_json::Value) -> Vec<Poly> {
    call["face_data"]
        .as_array()
        .expect("face_data")
        .iter()
        .map(|fd| Poly {
            uvs: fd[0]
                .as_array()
                .expect("uvs")
                .iter()
                .map(|uv| [uv[0].as_f64().expect("u"), uv[1].as_f64().expect("v")])
                .collect(),
            color: [
                fd[1][0].as_f64().expect("r"),
                fd[1][1].as_f64().expect("g"),
                fd[1][2].as_f64().expect("b"),
            ],
        })
        .collect()
}

#[test]
fn svg_and_eps_match_blender() {
    let calls: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{TESTDATA}/calls.json")).expect("calls.json"),
    )
    .expect("json");
    let calls = calls.as_array().expect("calls");
    assert_eq!(calls.len(), 8);
    for call in calls {
        let file = call["file"].as_str().expect("file");
        let tile = (
            call["tile"][0].as_i64().expect("tile"),
            call["tile"][1].as_i64().expect("tile"),
        );
        let (w, h) = (
            call["width"].as_i64().expect("w") as i32,
            call["height"].as_i64().expect("h") as i32,
        );
        let opacity = call["opacity"].as_f64().expect("opacity");
        let polys = polys(call);
        let ours = match call["kind"].as_str() {
            Some("SVG") => vector::svg(tile, &polys, w, h, opacity, ""),
            _ => vector::eps(tile, &polys, w, h, opacity),
        };
        let theirs = std::fs::read_to_string(format!("{TESTDATA}/{file}")).expect("reference");
        assert_eq!(body(&ours), body(&theirs), "{file}");
        assert_eq!(ours.ends_with('\n'), theirs.ends_with('\n'), "{file}");
        let header = ours
            .lines()
            .find(|l| l.starts_with("<desc>") || l.starts_with("%%Creator:"))
            .expect("generator line");
        assert!(header.contains(GENERATOR), "{file}: {header}");
    }
}

#[test]
fn tiles_and_file_names() {
    let poly = |uvs: Vec<[f64; 2]>| Poly {
        uvs,
        color: [0.8; 3],
    };
    let polys = [
        poly(vec![[0.2, 0.2], [1.0, 1.0], [0.5, 0.9]]),
        poly(vec![[1.2, 0.3], [1.9, 0.1], [2.0, 0.5]]),
        poly(vec![[-0.5, 3.5]]),
    ];
    assert_eq!(tiles_to_export(ExportTiles::None, &polys), vec![(0, 0)]);
    // (1.0, 1.0) and (2.0, 0.5) sit on the upper/right border of tiles (0, 0) and (1, 0).
    assert_eq!(tiles_to_export(ExportTiles::Udim, &polys), vec![(0, 0), (1, 0)]);
    assert_eq!(strip_name("cube.1001.svg", ExportTiles::Udim), "cube");
    assert_eq!(strip_name("cube.1001.svg", ExportTiles::None), "cube.1001");
    assert_eq!(strip_name("cube.u2_v1.png", ExportTiles::Uv), "cube");
    assert_eq!(strip_name("cube.png.png", ExportTiles::None), "cube.png");
    assert_eq!(strip_name("layout", ExportTiles::Udim), "layout");
    assert_eq!(ensure_ext("a.PNG", ".png"), "a.PNG");
    assert_eq!(ensure_ext("a", ".eps"), "a.eps");
    assert_eq!(round_half_even(0.8 * 255.0), 204);
}
