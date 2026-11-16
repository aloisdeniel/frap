// SPDX-License-Identifier: GPL-3.0-or-later
//! Colour differences for colour goldens (ARCHITECTURE §19.5, §22.7: "ΔE < 1").
//!
//! Inputs are **sRGB display-encoded** values (what lands in a PNG). They are decoded with the
//! display's transfer function, converted to CIE XYZ (D65) and CIELAB (D65 white), and compared
//! with CIEDE2000 ([`delta_e_2000`], the gating metric) or CIE76 ([`delta_e_76`], reported).
//! Computed in f64; the formulas follow Sharma, Wu & Dalal, "The CIEDE2000 Color-Difference
//! Formula: Implementation Notes, Supplementary Test Data, and Mathematical Observations" (2005).

use crate::spaces::{REC709_TO_XYZ, srgb_to_linear};

/// CIELAB coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    /// Lightness.
    pub l: f64,
    /// Green–red.
    pub a: f64,
    /// Blue–yellow.
    pub b: f64,
}

/// D65 reference white of `REC709_TO_XYZ` (Y = 1).
const WHITE: [f64; 3] = [0.950_455_9, 1.0, 1.089_057_8];

fn lab_f(t: f64) -> f64 {
    const D: f64 = 6.0 / 29.0;
    if t > D * D * D {
        libm::cbrt(t)
    } else {
        t / (3.0 * D * D) + 4.0 / 29.0
    }
}

/// CIE XYZ (D65, Y of white = 1) → CIELAB.
pub fn xyz_to_lab(xyz: [f64; 3]) -> Lab {
    let fx = lab_f(xyz[0] / WHITE[0]);
    let fy = lab_f(xyz[1] / WHITE[1]);
    let fz = lab_f(xyz[2] / WHITE[2]);
    Lab {
        l: 116.0 * fy - 16.0,
        a: 500.0 * (fx - fy),
        b: 200.0 * (fy - fz),
    }
}

/// sRGB display-encoded RGB (clamped to `[0, 1]`, as written to a file) → CIELAB.
pub fn srgb_to_lab(rgb: [f32; 3]) -> Lab {
    let lin = rgb.map(|v| f64::from(srgb_to_linear(v.clamp(0.0, 1.0))));
    let m = REC709_TO_XYZ;
    let xyz = [0, 1, 2].map(|i| {
        f64::from(m[i][0]) * lin[0] + f64::from(m[i][1]) * lin[1] + f64::from(m[i][2]) * lin[2]
    });
    xyz_to_lab(xyz)
}

/// CIE76 ΔE*ab (Euclidean distance in CIELAB).
pub fn delta_e_76(x: Lab, y: Lab) -> f64 {
    let (dl, da, db) = (x.l - y.l, x.a - y.a, x.b - y.b);
    libm::sqrt(dl * dl + da * da + db * db)
}

/// CIEDE2000 ΔE00 with `kL = kC = kH = 1`.
pub fn delta_e_2000(x: Lab, y: Lab) -> f64 {
    use std::f64::consts::PI;
    let deg = |r: f64| r * 180.0 / PI;
    let rad = |d: f64| d * PI / 180.0;
    let pow7 = |v: f64| v * v * v * v * v * v * v;
    let hypot = |a: f64, b: f64| libm::sqrt(a * a + b * b);

    let c1 = hypot(x.a, x.b);
    let c2 = hypot(y.a, y.b);
    let cbar = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - libm::sqrt(pow7(cbar) / (pow7(cbar) + pow7(25.0))));
    let a1 = (1.0 + g) * x.a;
    let a2 = (1.0 + g) * y.a;
    let c1p = hypot(a1, x.b);
    let c2p = hypot(a2, y.b);
    let hue = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            let h = deg(libm::atan2(b, a));
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let h1p = hue(x.b, a1);
    let h2p = hue(y.b, a2);

    let dlp = y.l - x.l;
    let dcp = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhp_big = 2.0 * libm::sqrt(c1p * c2p) * libm::sin(rad(dhp / 2.0));

    let lbarp = (x.l + y.l) / 2.0;
    let cbarp = (c1p + c2p) / 2.0;
    let hbarp = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * libm::cos(rad(hbarp - 30.0))
        + 0.24 * libm::cos(rad(2.0 * hbarp))
        + 0.32 * libm::cos(rad(3.0 * hbarp + 6.0))
        - 0.20 * libm::cos(rad(4.0 * hbarp - 63.0));
    let dtheta = 30.0 * libm::exp(-((hbarp - 275.0) / 25.0) * ((hbarp - 275.0) / 25.0));
    let rc = 2.0 * libm::sqrt(pow7(cbarp) / (pow7(cbarp) + pow7(25.0)));
    let l50 = (lbarp - 50.0) * (lbarp - 50.0);
    let sl = 1.0 + 0.015 * l50 / libm::sqrt(20.0 + l50);
    let sc = 1.0 + 0.045 * cbarp;
    let sh = 1.0 + 0.015 * cbarp * t;
    let rt = -libm::sin(rad(2.0 * dtheta)) * rc;
    let (tl, tc, th) = (dlp / sl, dcp / sc, dhp_big / sh);
    libm::sqrt(tl * tl + tc * tc + th * th + rt * tc * th)
}

/// CIEDE2000 between two sRGB display-encoded colours.
pub fn delta_e_srgb(x: [f32; 3], y: [f32; 3]) -> f64 {
    delta_e_2000(srgb_to_lab(x), srgb_to_lab(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab(l: f64, a: f64, b: f64) -> Lab {
        Lab { l, a, b }
    }

    #[test]
    fn ciede2000_sharma_reference_pairs() {
        // Sharma et al. (2005), Table 1: pairs 1, 7, 13, 17, 19, 25, 34.
        let cases = [
            (
                lab(50.0, 2.6772, -79.7751),
                lab(50.0, 0.0, -82.7485),
                2.0425,
            ),
            (lab(50.0, 0.0, 0.0), lab(50.0, -1.0, 2.0), 2.3669),
            (lab(50.0, 2.5, 0.0), lab(56.0, -27.0, -3.0), 31.9030),
            (lab(50.0, 2.5, 0.0), lab(50.0, 3.2592, 0.3350), 1.0000),
            (lab(50.0, 2.5, 0.0), lab(50.0, 3.1736, 0.5854), 1.0000),
            (
                lab(60.2574, -34.0099, 36.2677),
                lab(60.4626, -34.1751, 39.4387),
                1.2644,
            ),
            (
                lab(2.0776, 0.0795, -1.1350),
                lab(0.9033, -0.0636, -0.5514),
                0.9082,
            ),
        ];
        for (a, b, want) in cases {
            let got = delta_e_2000(a, b);
            assert!((got - want).abs() < 1e-4, "{a:?} {b:?}: {got} vs {want}");
            // Symmetric.
            assert!((delta_e_2000(b, a) - got).abs() < 1e-9);
        }
    }

    #[test]
    fn lab_of_srgb_white_and_black() {
        let w = srgb_to_lab([1.0, 1.0, 1.0]);
        assert!(
            (w.l - 100.0).abs() < 1e-3 && w.a.abs() < 1e-2 && w.b.abs() < 1e-2,
            "{w:?}"
        );
        let k = srgb_to_lab([0.0, 0.0, 0.0]);
        assert!(k.l.abs() < 1e-9);
        assert_eq!(delta_e_srgb([0.2, 0.4, 0.6], [0.2, 0.4, 0.6]), 0.0);
        assert!(delta_e_76(w, k) > 99.0);
    }
}
