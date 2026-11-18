// SPDX-License-Identifier: GPL-3.0-or-later
//! The embedded 5 x 7 bitmap font of contact-sheet labels (`assets/font/frap5x7.txt`, drawn for
//! frap; format in `assets/font/README.md`).

use std::collections::HashMap;
use std::sync::OnceLock;

/// Glyph width in cells.
pub const GLYPH_W: u32 = 5;
/// Glyph height in cells.
pub const GLYPH_H: u32 = 7;
/// Horizontal advance in cells (one blank column).
pub const ADVANCE: u32 = 6;
/// Line height in cells.
pub const LINE_H: u32 = 9;

const SOURCE: &str = include_str!("../../../../assets/font/frap5x7.txt");

/// A glyph: 7 rows of 5 bits (bit 4 = leftmost cell).
pub type Glyph = [u8; GLYPH_H as usize];

fn parse(text: &str) -> Result<HashMap<char, Glyph>, String> {
    let mut glyphs = HashMap::new();
    let mut lines = text.lines().skip_while(|l| !l.starts_with("glyph "));
    while let Some(head) = lines.next() {
        let name = head
            .strip_prefix("glyph ")
            .ok_or_else(|| format!("expected a `glyph` line, got {head:?}"))?;
        let c = match name {
            "space" => ' ',
            s if s.chars().count() == 1 => s.chars().next().expect("one char"),
            s => return Err(format!("bad glyph name {s:?}")),
        };
        let mut g: Glyph = [0; GLYPH_H as usize];
        for row in g.iter_mut() {
            let l = lines
                .next()
                .ok_or_else(|| format!("glyph {c:?}: missing rows"))?;
            if l.len() != GLYPH_W as usize || !l.chars().all(|ch| ch == '#' || ch == '.') {
                return Err(format!("glyph {c:?}: bad row {l:?}"));
            }
            *row = l
                .chars()
                .fold(0u8, |acc, ch| (acc << 1) | u8::from(ch == '#'));
        }
        if glyphs.insert(c, g).is_some() {
            return Err(format!("glyph {c:?} defined twice"));
        }
    }
    Ok(glyphs)
}

fn glyphs() -> &'static HashMap<char, Glyph> {
    static GLYPHS: OnceLock<HashMap<char, Glyph>> = OnceLock::new();
    GLYPHS.get_or_init(|| parse(SOURCE).expect("invariant: the embedded font parses"))
}

/// The glyph drawn for `c` (lowercase as uppercase, unknown characters as `?`).
pub fn glyph(c: char) -> Glyph {
    let g = glyphs();
    g.get(&c)
        .or_else(|| g.get(&c.to_ascii_uppercase()))
        .or_else(|| g.get(&'?'))
        .copied()
        .expect("invariant: the font has '?'")
}

/// Whether cell `(x, y)` of `g` is ink.
pub fn ink(g: &Glyph, x: u32, y: u32) -> bool {
    x < GLYPH_W && y < GLYPH_H && (g[y as usize] >> (GLYPH_W - 1 - x)) & 1 == 1
}

/// Width in cells of a one-line `text` (no trailing blank column).
pub fn text_width(text: &str) -> u32 {
    let n = text.chars().count() as u32;
    if n == 0 { 0 } else { n * ADVANCE - 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_covers_view_names() {
        let g = glyphs();
        for c in ('A'..='Z').chain('0'..='9').chain(" -_.,:()/+=#'?".chars()) {
            assert!(g.contains_key(&c), "{c:?}");
        }
        // 'T': a full top row, then the centre column.
        let t = glyph('T');
        assert_eq!(t[0], 0b11111);
        assert!((1..7).all(|y| t[y] == 0b00100));
        assert_eq!(glyph('t'), t);
        assert_eq!(glyph('é'), glyph('?'));
        assert!(ink(&t, 0, 0) && !ink(&t, 0, 1) && ink(&t, 2, 6));
        assert!(!ink(&t, 5, 0) && !ink(&t, 0, 7));
        assert_eq!(text_width("ISO"), 17);
        assert_eq!(text_width(""), 0);
    }

    #[test]
    fn parser_rejects_bad_glyphs() {
        assert!(parse("glyph A\n#####\n").is_err());
        assert!(parse("glyph A\n####\n.....\n.....\n.....\n.....\n.....\n.....\n").is_err());
        assert!(parse("glyph AB\n").is_err());
    }
}
