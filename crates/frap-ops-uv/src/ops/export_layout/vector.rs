// SPDX-License-Identifier: GPL-3.0-or-later
//! The SVG and EPS writers of `uv.export_layout`: byte for byte what the add-on writes, except
//! the header's generator line (SVG `<desc>`, EPS `%%Creator:`), which names frap.
//!
//! Ported from blender/scripts/addons_core/io_mesh_uv_layout/export_uv_svg.py and
//! export_uv_eps.py, Blender 5.2.4, GPL-2.0-or-later.

use super::py::{fmt_g, round_half_even, set_order};

/// One polygon to draw: its UVs (Python floats) and its colour (`get_polygon_color`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Poly {
    /// The corner UVs.
    pub uvs: Vec<[f64; 2]>,
    /// The material's `diffuse_color` RGB, or (0.8, 0.8, 0.8).
    pub color: [f64; 3],
}

/// The generator named in the headers.
pub(crate) const GENERATOR: &str = concat!("frap v", env!("CARGO_PKG_VERSION"));

/// `xml.sax.saxutils.escape`.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('>', "&gt;")
        .replace('<', "&lt;")
}

/// `export_uv_svg.get_color_string`.
fn svg_color(c: [f64; 3]) -> String {
    format!(
        "rgb({}, {}, {})",
        round_half_even(c[0] * 255.0),
        round_half_even(c[1] * 255.0),
        round_half_even(c[2] * 255.0)
    )
}

/// `export_uv_svg.get_file_parts`. `blend_name` is `basename(bpy.data.filepath)`.
pub(crate) fn svg(
    tile: (i64, i64),
    polys: &[Poly],
    width: i32,
    height: i32,
    opacity: f64,
    blend_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
    out.push_str("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \n");
    out.push_str("  \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">\n");
    out.push_str(&format!(
        "<svg width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"\n"
    ));
    out.push_str("     xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\">\n");
    let desc = format!("{blend_name}, ({GENERATOR})");
    out.push_str(&format!("<desc>{}</desc>\n", xml_escape(&desc)));
    let (w, h) = (f64::from(width), f64::from(height));
    for p in polys {
        out.push_str("<polygon stroke=\"black\" stroke-width=\"1\"");
        out.push_str(&format!(
            " fill=\"{}\" fill-opacity=\"{}\"",
            svg_color(p.color),
            fmt_g(opacity, 2)
        ));
        out.push_str(" points=\"");
        for uv in &p.uvs {
            let x = uv[0] - tile.0 as f64;
            let y = 1.0 - uv[1] + tile.1 as f64;
            out.push_str(&format!("{:.3},{:.3} ", x * w, y * h));
        }
        out.push_str("\" />\n");
    }
    out.push('\n');
    out.push_str("</svg>\n");
    out
}

/// `export_uv_eps.draw_polygon_path`.
fn eps_path(out: &mut String, tile: (i64, i64), uvs: &[[f64; 2]], w: f64, h: f64) {
    out.push_str("newpath\n");
    for (j, uv) in uvs.iter().enumerate() {
        let x = (uv[0] - tile.0 as f64) * w;
        let y = (uv[1] - tile.1 as f64) * h;
        let op = if j == 0 { "moveto" } else { "lineto" };
        out.push_str(&format!("{x:.5} {y:.5} {op}\n"));
    }
}

/// `export_uv_eps.get_file_parts`. The colours are named in the iteration order of the
/// add-on's `set` of polygon colours.
pub(crate) fn eps(
    tile: (i64, i64),
    polys: &[Poly],
    width: i32,
    height: i32,
    opacity: f64,
) -> String {
    let mut out = String::new();
    out.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    out.push_str(&format!("%%Creator: {GENERATOR}\n"));
    out.push_str("%%Pages: 1\n");
    out.push_str("%%Orientation: Portrait\n");
    out.push_str(&format!("%%BoundingBox: 0 0 {width} {height}\n"));
    out.push_str(&format!(
        "%%HiResBoundingBox: 0.0 0.0 {:.4} {:.4}\n",
        f64::from(width),
        f64::from(height)
    ));
    out.push_str("%%EndComments\n");
    out.push_str("%%Page: 1 1\n");
    out.push_str("0 0 translate\n");
    out.push_str("1.0 1.0 scale\n");
    out.push_str("0 0 0 setrgbcolor\n");
    out.push_str("[] 0 setdash\n");
    out.push_str("1 setlinewidth\n");
    out.push_str("1 setlinejoin\n");
    out.push_str("1 setlinecap\n");
    let (w, h) = (f64::from(width), f64::from(height));
    if opacity > 0.0 {
        let colors: Vec<[f64; 3]> = polys.iter().map(|p| p.color).collect();
        let order = set_order(&colors);
        let mut names: Vec<([f64; 3], String)> = Vec::with_capacity(order.len());
        for (i, &k) in order.iter().enumerate() {
            let c = colors[k];
            let name = format!("COLOR_{i}");
            out.push_str(&format!("/{name} {{"));
            out.push_str("gsave\n");
            out.push_str(&format!(
                "{} {} {} setrgbcolor\n",
                fmt_g(c[0], 3),
                fmt_g(c[1], 3),
                fmt_g(c[2], 3)
            ));
            out.push_str("fill\n");
            out.push_str("grestore\n");
            out.push_str("0 setgray\n");
            out.push_str("} def\n");
            names.push((c, name));
        }
        for p in polys {
            eps_path(&mut out, tile, &p.uvs, w, h);
            out.push_str("closepath\n");
            let name = names
                .iter()
                .find(|(c, _)| *c == p.color)
                .map_or("COLOR_0", |(_, n)| n.as_str());
            out.push_str(name);
            out.push('\n');
        }
    }
    for p in polys {
        eps_path(&mut out, tile, &p.uvs, w, h);
        out.push_str("closepath\n");
        out.push_str("stroke\n");
    }
    out.push_str("showpage\n");
    out.push_str("%%EOF\n");
    out
}
