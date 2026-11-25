// SPDX-License-Identifier: GPL-3.0-or-later
//! `.cube` LUT loading and tetrahedral application over the log2 shaper (`assets/luts/*.cube`).
//! WP 1.6.
//!
//! A baked view transform ([`ViewLut`]) is, per pixel:
//!
//! ```text
//! scene-linear Rec.709 ─► matrix (optional) ─► log2 shaper ─► 3D LUT (tetrahedral) ─► post ─► display
//! ```
//!
//! The shaper is OCIO's `AllocationTransform {allocation: lg2, vars: [lo, hi]}`:
//! `s = (log2(x) - lo) / (hi - lo)`, clamped to the LUT domain `[0, 1]` (OCIO clamps the lattice
//! index the same way; `x <= 0` falls to `s = 0`). The interpolation is a port of OCIO's
//! `Lut3DTetrahedralRenderer` (`src/OpenColorIO/ops/lut3d/Lut3DOpCPU.cpp`, BSD-3-Clause). The
//! parameters are read from `# frap-*:` header comments written by
//! `tools/blender/bake_view_luts.py`, which measures the lattice through Blender itself.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::spaces::{linear_to_srgb, mat3_mul, rec1886_to_linear};

/// `.cube` parse failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubeError {
    /// 1-based line number (0 for whole-file errors).
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for CubeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ".cube line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for CubeError {}

fn err(line: usize, message: impl Into<String>) -> CubeError {
    CubeError {
        line,
        message: message.into(),
    }
}

/// A parsed Resolve/Adobe `.cube` 3D LUT, plus its `# key: value` comment metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct CubeFile {
    /// `TITLE`, if present.
    pub title: Option<String>,
    /// `DOMAIN_MIN`.
    pub domain_min: [f32; 3],
    /// `DOMAIN_MAX`.
    pub domain_max: [f32; 3],
    /// The lattice.
    pub lut: Lut3d,
    /// Comment lines of the form `# key: value` (e.g. `frap-shaper`).
    pub meta: BTreeMap<String, String>,
}

impl CubeFile {
    /// Parses `.cube` text (3D LUTs only; red varies fastest, as the format specifies).
    pub fn parse(text: &str) -> Result<CubeFile, CubeError> {
        let mut title = None;
        let mut domain_min = [0.0; 3];
        let mut domain_max = [1.0; 3];
        let mut size: Option<usize> = None;
        let mut data: Vec<[f32; 3]> = Vec::new();
        let mut meta = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let lineno = i + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                if let Some((k, v)) = comment.split_once(':') {
                    let k = k.trim();
                    if !k.is_empty() && !k.contains(' ') {
                        meta.insert(k.to_owned(), v.trim().to_owned());
                    }
                }
                continue;
            }
            let first = line.as_bytes()[0];
            if first.is_ascii_alphabetic() {
                let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
                let rest = rest.trim();
                match key {
                    "TITLE" => title = Some(rest.trim_matches('"').to_owned()),
                    "LUT_3D_SIZE" => {
                        let n: usize = rest
                            .parse()
                            .map_err(|_| err(lineno, format!("bad LUT_3D_SIZE {rest:?}")))?;
                        if !(2..=256).contains(&n) {
                            return Err(err(lineno, format!("LUT_3D_SIZE {n} out of range")));
                        }
                        size = Some(n);
                        data.reserve(n * n * n);
                    }
                    "DOMAIN_MIN" => domain_min = parse_triple(rest, lineno)?,
                    "DOMAIN_MAX" => domain_max = parse_triple(rest, lineno)?,
                    "LUT_1D_SIZE" => return Err(err(lineno, "1D LUTs are not supported")),
                    "LUT_1D_INPUT_RANGE" | "LUT_3D_INPUT_RANGE" => {
                        let v = parse_pair(rest, lineno)?;
                        domain_min = [v[0]; 3];
                        domain_max = [v[1]; 3];
                    }
                    other => return Err(err(lineno, format!("unknown keyword {other}"))),
                }
                continue;
            }
            data.push(parse_triple(line, lineno)?);
        }
        let size = size.ok_or_else(|| err(0, "missing LUT_3D_SIZE"))?;
        if data.len() != size * size * size {
            return Err(err(
                0,
                format!(
                    "expected {} entries, found {}",
                    size * size * size,
                    data.len()
                ),
            ));
        }
        Ok(CubeFile {
            title,
            domain_min,
            domain_max,
            lut: Lut3d { size, data },
            meta,
        })
    }
}

fn parse_floats<const N: usize>(s: &str, line: usize) -> Result<[f32; N], CubeError> {
    let mut out = [0.0; N];
    let mut it = s.split_whitespace();
    for o in &mut out {
        let tok = it
            .next()
            .ok_or_else(|| err(line, format!("expected {N} numbers")))?;
        *o = tok
            .parse()
            .map_err(|_| err(line, format!("bad number {tok:?}")))?;
    }
    if it.next().is_some() {
        return Err(err(line, format!("expected {N} numbers")));
    }
    Ok(out)
}

fn parse_triple(s: &str, line: usize) -> Result<[f32; 3], CubeError> {
    parse_floats::<3>(s, line)
}

fn parse_pair(s: &str, line: usize) -> Result<[f32; 2], CubeError> {
    parse_floats::<2>(s, line)
}

/// A 3D lattice of RGB values, red fastest: `data[r + size * (g + size * b)]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3d {
    /// Points per axis.
    pub size: usize,
    /// `size³` entries.
    pub data: Vec<[f32; 3]>,
}

impl Lut3d {
    #[inline]
    fn at(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        self.data[r + self.size * (g + self.size * b)]
    }

    /// Lattice coordinates of a domain-normalised input: base index and fraction per axis.
    #[inline]
    fn locate(&self, v: f32) -> (usize, usize, f32) {
        let max = (self.size - 1) as f32;
        // NaN maps to 0 like OCIO's clamp does.
        let x = if v.is_nan() {
            0.0
        } else {
            v.clamp(0.0, 1.0) * max
        };
        let i0 = (x.floor() as usize).min(self.size - 1);
        let i1 = (i0 + 1).min(self.size - 1);
        (i0, i1, x - i0 as f32)
    }

    /// Tetrahedral interpolation of an input in `[0, 1]³` (clamped), as OCIO's
    /// `Lut3DTetrahedralRenderer`.
    pub fn tetrahedral(&self, rgb: [f32; 3]) -> [f32; 3] {
        let (r0, r1, fr) = self.locate(rgb[0]);
        let (g0, g1, fg) = self.locate(rgb[1]);
        let (b0, b1, fb) = self.locate(rgb[2]);
        let c000 = self.at(r0, g0, b0);
        let c111 = self.at(r1, g1, b1);
        let mut out = [0.0; 3];
        // Six tetrahedra, chosen by the order of the fractions (OCIO's case split).
        if fr > fg {
            if fg > fb {
                let c100 = self.at(r1, g0, b0);
                let c110 = self.at(r1, g1, b0);
                for k in 0..3 {
                    out[k] = (1.0 - fr) * c000[k]
                        + (fr - fg) * c100[k]
                        + (fg - fb) * c110[k]
                        + fb * c111[k];
                }
            } else if fr > fb {
                let c100 = self.at(r1, g0, b0);
                let c101 = self.at(r1, g0, b1);
                for k in 0..3 {
                    out[k] = (1.0 - fr) * c000[k]
                        + (fr - fb) * c100[k]
                        + (fb - fg) * c101[k]
                        + fg * c111[k];
                }
            } else {
                let c001 = self.at(r0, g0, b1);
                let c101 = self.at(r1, g0, b1);
                for k in 0..3 {
                    out[k] = (1.0 - fb) * c000[k]
                        + (fb - fr) * c001[k]
                        + (fr - fg) * c101[k]
                        + fg * c111[k];
                }
            }
        } else if fb > fg {
            let c001 = self.at(r0, g0, b1);
            let c011 = self.at(r0, g1, b1);
            for k in 0..3 {
                out[k] =
                    (1.0 - fb) * c000[k] + (fb - fg) * c001[k] + (fg - fr) * c011[k] + fr * c111[k];
            }
        } else if fb > fr {
            let c010 = self.at(r0, g1, b0);
            let c011 = self.at(r0, g1, b1);
            for k in 0..3 {
                out[k] =
                    (1.0 - fg) * c000[k] + (fg - fb) * c010[k] + (fb - fr) * c011[k] + fr * c111[k];
            }
        } else {
            let c010 = self.at(r0, g1, b0);
            let c110 = self.at(r1, g1, b0);
            for k in 0..3 {
                out[k] =
                    (1.0 - fg) * c000[k] + (fg - fr) * c010[k] + (fr - fb) * c110[k] + fb * c111[k];
            }
        }
        out
    }

    /// Trilinear interpolation of an input in `[0, 1]³` (clamped).
    pub fn trilinear(&self, rgb: [f32; 3]) -> [f32; 3] {
        let (r0, r1, fr) = self.locate(rgb[0]);
        let (g0, g1, fg) = self.locate(rgb[1]);
        let (b0, b1, fb) = self.locate(rgb[2]);
        let lerp = |a: [f32; 3], b: [f32; 3], t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        let c00 = lerp(self.at(r0, g0, b0), self.at(r1, g0, b0), fr);
        let c10 = lerp(self.at(r0, g1, b0), self.at(r1, g1, b0), fr);
        let c01 = lerp(self.at(r0, g0, b1), self.at(r1, g0, b1), fr);
        let c11 = lerp(self.at(r0, g1, b1), self.at(r1, g1, b1), fr);
        lerp(lerp(c00, c10, fg), lerp(c01, c11, fg), fb)
    }
}

/// OCIO `AllocationTransform {allocation: lg2, vars: [lo, hi]}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Log2Shaper {
    /// log2 of the darkest value (maps to 0).
    pub lo: f32,
    /// log2 of the brightest value (maps to 1).
    pub hi: f32,
}

impl Log2Shaper {
    /// Forward: linear → `[0, 1]` (unclamped above/below; the LUT clamps).
    #[inline]
    pub fn apply(&self, x: f32) -> f32 {
        if x <= 0.0 || x.is_nan() {
            return 0.0;
        }
        (libm::log2f(x) - self.lo) / (self.hi - self.lo)
    }

    /// Inverse: `[0, 1]` → linear.
    #[inline]
    pub fn invert(&self, s: f32) -> f32 {
        libm::exp2f(self.lo + s * (self.hi - self.lo))
    }
}

/// Per-channel transfer applied after the LUT (`# frap-post:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostTransfer {
    /// The LUT holds display values.
    None,
    /// The LUT holds Rec.1886 (2.4) values shown on the sRGB display:
    /// `srgb_encode(max(v, 0)^2.4)` (Blender's `ColorSpaceTransform {src: Rec.1886, dst: sRGB}`).
    Rec1886ToSrgb,
}

impl PostTransfer {
    fn from_name(s: &str) -> Option<PostTransfer> {
        match s {
            "none" => Some(PostTransfer::None),
            "rec1886_to_srgb" => Some(PostTransfer::Rec1886ToSrgb),
            _ => None,
        }
    }

    #[inline]
    fn apply(self, v: f32) -> f32 {
        match self {
            PostTransfer::None => v,
            PostTransfer::Rec1886ToSrgb => linear_to_srgb(rec1886_to_linear(v)),
        }
    }
}

/// A view transform baked by `tools/blender/bake_view_luts.py`.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewLut {
    /// `view_settings.view_transform` it was baked from.
    pub view_transform: String,
    /// `view_settings.look` it was baked with.
    pub look: String,
    /// `display_settings.display_device` it was baked for.
    pub display: String,
    /// Scene-linear Rec.709 → shaper input space.
    pub matrix: Option<[[f32; 3]; 3]>,
    /// The log2 shaper.
    pub shaper: Log2Shaper,
    /// The lattice.
    pub lut: Lut3d,
    /// Transfer after interpolation.
    pub post: PostTransfer,
}

impl ViewLut {
    /// Builds a view LUT from a parsed `.cube` with `# frap-*:` metadata.
    pub fn from_cube(cube: CubeFile) -> Result<ViewLut, CubeError> {
        let get = |k: &str| {
            cube.meta
                .get(k)
                .cloned()
                .ok_or_else(|| err(0, format!("missing `# {k}:` header")))
        };
        let shaper = get("frap-shaper")?;
        let mut it = shaper.split_whitespace();
        if it.next() != Some("lg2") {
            return Err(err(0, format!("unsupported shaper {shaper:?}")));
        }
        let lo: f32 = it
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| err(0, "bad shaper lo"))?;
        let hi: f32 = it
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| err(0, "bad shaper hi"))?;
        if hi <= lo {
            return Err(err(0, "shaper hi must exceed lo"));
        }
        let matrix = match cube.meta.get("frap-matrix") {
            None => None,
            Some(m) => {
                let v = parse_floats::<9>(m, 0)?;
                Some([[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]])
            }
        };
        let interp = cube
            .meta
            .get("frap-interpolation")
            .map(String::as_str)
            .unwrap_or("tetrahedral");
        if interp != "tetrahedral" {
            return Err(err(0, format!("unsupported interpolation {interp:?}")));
        }
        let post_name = get("frap-post")?;
        let post = PostTransfer::from_name(&post_name)
            .ok_or_else(|| err(0, format!("unknown post transfer {post_name:?}")))?;
        if cube.domain_min != [0.0; 3] || cube.domain_max != [1.0; 3] {
            return Err(err(0, "frap view LUTs use the [0, 1] domain"));
        }
        Ok(ViewLut {
            view_transform: get("frap-view-transform")?,
            look: get("frap-look")?,
            display: get("frap-display")?,
            matrix,
            shaper: Log2Shaper { lo, hi },
            lut: cube.lut,
            post,
        })
    }

    /// Scene-linear Rec.709 → display-encoded RGB (no exposure/gamma; see `display`).
    #[inline]
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let v = match &self.matrix {
            Some(m) => mat3_mul(m, rgb),
            None => rgb,
        };
        let s = [
            self.shaper.apply(v[0]),
            self.shaper.apply(v[1]),
            self.shaper.apply(v[2]),
        ];
        let o = self.lut.tetrahedral(s);
        [
            self.post.apply(o[0]),
            self.post.apply(o[1]),
            self.post.apply(o[2]),
        ]
    }
}

/// One baked LUT: `(view transform, look, display, deflate-packed .cube file)`. Each
/// view-transform module (`agx`, `filmic`, `pbr_neutral`, `aces`, `false_color`; WP 5.3) lists
/// its own. The packed copies live in `src/generated/` (`cargo xtask codegen` packs
/// `assets/luts/`; decision log BIN-1); [`unpack`] restores the text.
pub type BakedLut = (&'static str, &'static str, &'static str, &'static [u8]);

/// The text of a deflate-packed LUT file from `src/generated/` (decision log BIN-1). Panics only
/// on a file `cargo xtask codegen` did not write (the LUT tests parse every one).
pub(crate) fn unpack(packed: &[u8]) -> String {
    use std::io::Read as _;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(packed)
        .read_to_string(&mut text)
        .expect("invariant: src/generated/*.deflate is deflated UTF-8 (cargo xtask codegen)");
    text
}

/// Every LUT checked in under `assets/luts/`, from the view-transform modules (M5+M6 W0: the
/// extension point WP 5.3 fills without editing this file).
fn all_baked() -> impl Iterator<Item = &'static BakedLut> {
    crate::agx::BAKED
        .iter()
        .chain(crate::filmic::BAKED)
        .chain(crate::pbr_neutral::BAKED)
        .chain(crate::aces::BAKED)
        .chain(crate::false_color::BAKED)
}

/// The baked LUT for `(view_transform, look, display)`, parsed on first use.
///
/// `None` when that combination has not been baked (M1 bakes AgX on the sRGB display; WP 5.3
/// adds every view transform × look through the modules' `BAKED` lists).
pub fn baked(view_transform: &str, look: &str, display: &str) -> Option<&'static ViewLut> {
    static PARSED: OnceLock<Vec<ViewLut>> = OnceLock::new();
    let all = PARSED.get_or_init(|| {
        all_baked()
            .map(|(v, l, d, packed)| {
                let lut = CubeFile::parse(&unpack(packed))
                    .and_then(ViewLut::from_cube)
                    .unwrap_or_else(|e| panic!("assets/luts for {v}/{l}/{d} is corrupt: {e}"));
                assert_eq!(
                    (
                        lut.view_transform.as_str(),
                        lut.look.as_str(),
                        lut.display.as_str()
                    ),
                    (*v, *l, *d),
                    "assets/luts header does not match the BAKED table"
                );
                lut
            })
            .collect()
    });
    all.iter()
        .find(|l| l.view_transform == view_transform && l.look == look && l.display == display)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_cube(n: usize) -> Lut3d {
        let mut data = Vec::new();
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let m = (n - 1) as f32;
                    data.push([r as f32 / m, g as f32 / m, b as f32 / m]);
                }
            }
        }
        Lut3d { size: n, data }
    }

    #[test]
    fn identity_lut_is_exact_for_both_interpolations() {
        let lut = identity_cube(5);
        for p in [
            [0.1, 0.7, 0.33],
            [0.9, 0.2, 0.5],
            [0.0, 1.0, 0.5],
            [0.3, 0.3, 0.3],
        ] {
            let t = lut.tetrahedral(p);
            let l = lut.trilinear(p);
            for k in 0..3 {
                assert!((t[k] - p[k]).abs() < 1e-6, "{p:?} {t:?}");
                assert!((l[k] - p[k]).abs() < 1e-6, "{p:?} {l:?}");
            }
        }
        // Clamped outside the domain, NaN → 0.
        assert_eq!(lut.tetrahedral([-1.0, 2.0, f32::NAN]), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn tetrahedral_hits_nodes_and_matches_ocio_cases() {
        // A non-linear LUT: every node holds (r², g·b, r+b) of its coordinate.
        let n = 4;
        let m = (n - 1) as f32;
        let f = |r: f32, g: f32, b: f32| [r * r, g * b, r + b];
        let mut data = Vec::new();
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    data.push(f(r as f32 / m, g as f32 / m, b as f32 / m));
                }
            }
        }
        let lut = Lut3d { size: n, data };
        // Nodes are reproduced exactly.
        let node = lut.tetrahedral([1.0 / 3.0, 2.0 / 3.0, 1.0]);
        let want = f(1.0 / 3.0, 2.0 / 3.0, 1.0);
        for k in 0..3 {
            assert!((node[k] - want[k]).abs() < 1e-6);
        }
        // Every one of the six tetrahedra: compare with an independent barycentric evaluation
        // (inside one cell [0, 1/3]³ the six orderings of the fractions select the six cases).
        let fr = [0.1, 0.2, 0.3];
        let orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for o in orders {
            let frac = [fr[o[0]], fr[o[1]], fr[o[2]]];
            let p = [frac[0] / m, frac[1] / m, frac[2] / m];
            let got = lut.tetrahedral(p);
            // Walk from c000 to c111 along the axes in decreasing fraction order.
            let mut idx: Vec<usize> = (0..3).collect();
            idx.sort_by(|a, b| frac[*b].partial_cmp(&frac[*a]).unwrap());
            let mut corner = [0usize; 3];
            let node = |c: [usize; 3]| f(c[0] as f32 / m, c[1] as f32 / m, c[2] as f32 / m);
            let mut want = [0.0f32; 3];
            let w0 = 1.0 - frac[idx[0]];
            for (acc, v) in want.iter_mut().zip(node(corner)) {
                *acc += w0 * v;
            }
            for (j, &axis) in idx.iter().enumerate() {
                corner[axis] = 1;
                let next = if j + 1 < 3 { frac[idx[j + 1]] } else { 0.0 };
                let w = frac[axis] - next;
                for (acc, v) in want.iter_mut().zip(node(corner)) {
                    *acc += w * v;
                }
            }
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() < 1e-6,
                    "{o:?}: {got:?} vs {want:?}"
                );
            }
        }
    }

    #[test]
    fn parse_cube_with_meta_and_errors() {
        let text = "TITLE \"t\"\n# frap-shaper: lg2 -2 2\n# just a comment\nLUT_3D_SIZE 2\n\
                    DOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let c = CubeFile::parse(text).unwrap();
        assert_eq!(c.title.as_deref(), Some("t"));
        assert_eq!(c.lut.size, 2);
        assert_eq!(
            c.meta.get("frap-shaper").map(String::as_str),
            Some("lg2 -2 2")
        );
        assert_eq!(c.lut.at(1, 0, 1), [1.0, 0.0, 1.0]);

        let short = "LUT_3D_SIZE 2\n0 0 0\n";
        assert!(
            CubeFile::parse(short)
                .unwrap_err()
                .message
                .contains("expected 8")
        );
        let bad = "LUT_3D_SIZE 2\n0 0 x\n";
        assert_eq!(CubeFile::parse(bad).unwrap_err().line, 2);
        assert!(CubeFile::parse("LUT_1D_SIZE 4\n").is_err());
        // Missing frap metadata.
        assert!(ViewLut::from_cube(c).is_err());
    }

    #[test]
    fn shaper_round_trip() {
        let s = Log2Shaper {
            lo: -12.47393,
            hi: 12.526_069,
        };
        for x in [1e-3f32, 0.18, 1.0, 16.0, 1000.0] {
            let back = s.invert(s.apply(x));
            assert!((back / x - 1.0).abs() < 1e-5, "{x} {back}");
        }
        assert_eq!(s.apply(0.0), 0.0);
        assert_eq!(s.apply(-1.0), 0.0);
    }

    #[test]
    fn baked_agx_loads() {
        let agx = baked("AgX", "None", "sRGB").expect("AgX baked");
        assert_eq!(agx.lut.size, 57);
        assert_eq!(agx.post, PostTransfer::Rec1886ToSrgb);
        assert!(agx.matrix.is_some());
        assert!(baked("AgX", "Punchy", "sRGB").is_none());
        assert!(baked("Filmic", "None", "sRGB").is_none());
        // Black stays black, white-ish grows monotonically along the neutral axis.
        let mut last = -1.0;
        for i in 0..40 {
            let x = libm::exp2f(-12.0 + i as f32 * 0.4);
            let o = agx.apply([x, x, x]);
            assert!(o[1] >= last, "AgX neutral axis must be monotonic");
            last = o[1];
        }
    }
}
