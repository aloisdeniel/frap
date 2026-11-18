// SPDX-License-Identifier: GPL-3.0-or-later
//! Contact-sheet compositor (`frap.render_contact_sheet`, ARCHITECTURE §19.6): rendered tiles
//! placed on the CPU by a [`SheetLayout`], labels drawn with the embedded bitmap font of
//! `assets/font/` ([`font`]). Reached through `RenderService::composite_sheet` (WP 2.3).
//!
//! The compositor is colour-space agnostic: the background and the films must be in the same
//! space. `frap.render_contact_sheet` passes display-referred, already 8-bit-quantised tiles
//! (each colour-managed and dithered on its own, so a tile is the same pixels as a render of
//! its view) and a display-referred background, and writes the sheet without a further
//! transform.
//!
//! Rules:
//! - the sheet starts as `layout.background` (straight RGBA);
//! - each film is composited "over" the sheet at its tile's rectangle (straight alpha:
//!   `a = sa + da (1 - sa)`, `rgb = (src sa + dst da (1 - sa)) / a`), so an opaque tile is
//!   copied exactly;
//! - a label is drawn in white over a black backdrop of alpha [`LABEL_BACKDROP_ALPHA`], at the
//!   tile's top-left corner, with glyph cells of [`label_scale`] pixels.

pub mod font;

use frap_render_scene::{Film, RenderError, SheetLayout};

/// Opacity of the black box behind a label.
pub const LABEL_BACKDROP_ALPHA: f32 = 0.6;

/// Pixels per font cell for a tile `w x h`: `clamp(min(w, h) / 160, 1, 8)`.
pub fn label_scale(w: u32, h: u32) -> u32 {
    (w.min(h) / 160).clamp(1, 8)
}

/// Straight-alpha "over".
fn over(src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    let sa = src[3].clamp(0.0, 1.0);
    if sa >= 1.0 {
        return src;
    }
    let da = dst[3].clamp(0.0, 1.0);
    let a = sa + da * (1.0 - sa);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let mix = |s: f32, d: f32| (s * sa + d * da * (1.0 - sa)) / a;
    [
        mix(src[0], dst[0]),
        mix(src[1], dst[1]),
        mix(src[2], dst[2]),
        a,
    ]
}

/// The label box of a tile: `[x, y, w, h]` (clipped to the tile) and the text origin.
pub fn label_box(rect: [u32; 4], text: &str) -> ([u32; 4], [u32; 2]) {
    let [x, y, w, h] = rect;
    let s = label_scale(w, h);
    let pad = s * 2;
    let bw = (font::text_width(text) * s + 2 * pad).min(w);
    let bh = (font::GLYPH_H * s + 2 * pad).min(h);
    ([x, y, bw, bh], [x + pad, y + pad])
}

/// Composites `films` on a sheet laid out by `layout` (one film per tile, the film the size of
/// its tile).
pub fn composite(films: &[Film], layout: &SheetLayout) -> Result<Film, RenderError> {
    let [sw, sh] = layout.size;
    if films.len() != layout.tiles.len() {
        return Err(RenderError::Unsupported(format!(
            "contact sheet: {} films for {} tiles",
            films.len(),
            layout.tiles.len()
        )));
    }
    if sw == 0 || sh == 0 {
        return Err(RenderError::Unsupported(
            "contact sheet: empty sheet".into(),
        ));
    }
    let mut rgba = vec![layout.background; sw as usize * sh as usize];
    for (film, tile) in films.iter().zip(&layout.tiles) {
        let [x, y, w, h] = tile.rect;
        if (film.width, film.height) != (w, h) {
            return Err(RenderError::Unsupported(format!(
                "contact sheet: a {}x{} film for a {w}x{h} tile",
                film.width, film.height
            )));
        }
        if u64::from(x) + u64::from(w) > u64::from(sw)
            || u64::from(y) + u64::from(h) > u64::from(sh)
        {
            return Err(RenderError::Unsupported(format!(
                "contact sheet: tile {:?} outside the {sw}x{sh} sheet",
                tile.rect
            )));
        }
        if film.rgba.len() != w as usize * h as usize {
            return Err(RenderError::Unsupported(
                "contact sheet: film pixel count mismatch".into(),
            ));
        }
        for row in 0..h {
            for col in 0..w {
                let s = film.rgba[(row * w + col) as usize];
                let d = &mut rgba[((y + row) * sw + x + col) as usize];
                *d = over(s, *d);
            }
        }
        if let Some(label) = &tile.label {
            draw_label(&mut rgba, sw, tile.rect, label);
        }
    }
    Ok(Film {
        width: sw,
        height: sh,
        rgba,
    })
}

fn draw_label(rgba: &mut [[f32; 4]], sheet_w: u32, rect: [u32; 4], text: &str) {
    let ([bx, by, bw, bh], [tx, ty]) = label_box(rect, text);
    let backdrop = [0.0, 0.0, 0.0, LABEL_BACKDROP_ALPHA];
    for row in by..by + bh {
        for col in bx..bx + bw {
            let d = &mut rgba[(row * sheet_w + col) as usize];
            *d = over(backdrop, *d);
        }
    }
    let s = label_scale(rect[2], rect[3]);
    let (x_end, y_end) = (rect[0] + rect[2], rect[1] + rect[3]);
    for (i, c) in text.chars().enumerate() {
        let g = font::glyph(c);
        let gx = tx + i as u32 * font::ADVANCE * s;
        for cy in 0..font::GLYPH_H {
            for cx in 0..font::GLYPH_W {
                if !font::ink(&g, cx, cy) {
                    continue;
                }
                for py in 0..s {
                    for px in 0..s {
                        let (x, y) = (gx + cx * s + px, ty + cy * s + py);
                        if x < x_end && y < y_end {
                            rgba[(y * sheet_w + x) as usize] = [1.0, 1.0, 1.0, 1.0];
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_render_scene::SheetTile;

    fn film(w: u32, h: u32, c: [f32; 4]) -> Film {
        Film {
            width: w,
            height: h,
            rgba: vec![c; (w * h) as usize],
        }
    }

    #[test]
    fn tiles_are_copied_and_the_background_fills_the_rest() {
        let bg = [0.2, 0.2, 0.2, 1.0];
        let layout = SheetLayout::new(
            [8, 4],
            bg,
            vec![
                SheetTile::new([0, 0, 4, 4], None),
                SheetTile::new([4, 0, 2, 2], None),
            ],
        );
        let a = film(4, 4, [1.0, 0.0, 0.0, 1.0]);
        let b = film(2, 2, [0.0, 0.0, 1.0, 0.5]);
        let out = composite(&[a, b], &layout).unwrap();
        assert_eq!((out.width, out.height), (8, 4));
        assert_eq!(out.rgba[0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(out.rgba[3 * 8 + 3], [1.0, 0.0, 0.0, 1.0]);
        // Half-transparent blue over the grey background.
        let p = out.rgba[4];
        assert!(
            (p[2] - 0.6).abs() < 1e-6 && (p[0] - 0.1).abs() < 1e-6 && p[3] == 1.0,
            "{p:?}"
        );
        assert_eq!(out.rgba[3 * 8 + 7], bg);
    }

    #[test]
    fn mismatches_are_errors() {
        let layout = SheetLayout::new([4, 4], [0.0; 4], vec![SheetTile::new([2, 2, 4, 4], None)]);
        assert!(composite(&[film(4, 4, [1.0; 4])], &layout).is_err());
        let layout = SheetLayout::new([4, 4], [0.0; 4], vec![SheetTile::new([0, 0, 4, 4], None)]);
        assert!(composite(&[film(2, 2, [1.0; 4])], &layout).is_err());
        assert!(composite(&[], &layout).is_err());
    }

    #[test]
    fn labels_are_white_on_a_dark_box() {
        let layout = SheetLayout::new(
            [64, 32],
            [0.5, 0.5, 0.5, 1.0],
            vec![SheetTile::new([0, 0, 64, 32], Some("TOP".into()))],
        );
        let out = composite(&[film(64, 32, [0.5, 0.5, 0.5, 1.0])], &layout).unwrap();
        let ([bx, by, bw, bh], [tx, ty]) = label_box([0, 0, 64, 32], "TOP");
        assert_eq!((bx, by, tx, ty), (0, 0, 2, 2));
        assert_eq!((bw, bh), (17 + 4, 7 + 4));
        // 'T' top row is ink, the box behind is darkened, outside the box is untouched.
        assert_eq!(out.rgba[(ty * 64 + tx) as usize], [1.0, 1.0, 1.0, 1.0]);
        let dark = out.rgba[(ty * 64 + tx + 1 + 64) as usize];
        assert!((dark[0] - 0.2).abs() < 1e-6, "{dark:?}");
        assert_eq!(out.rgba[(20 * 64 + 40) as usize], [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(label_scale(512, 512), 3);
        assert_eq!(label_scale(64, 64), 1);
        assert_eq!(label_scale(4096, 4096), 8);
    }
}
