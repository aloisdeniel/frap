// SPDX-License-Identifier: GPL-3.0-or-later
//! T3 invariants of the island packer (WP 3.3 acceptance, ARCHITECTURE §20 "T3 invariant"):
//! for random islands (convex polygons of random size, position and orientation) and random
//! AABB-packer parameters, every packed UV lies in the target square, no two islands overlap,
//! and every island keeps its shape and the common scale (area ratios are preserved).

use frap_uv::pack::{self, MarginMethod, PackIsland, PackParams, RotationMethod, ShapeMethod};
use proptest::prelude::*;

type Poly = Vec<[f32; 2]>;

/// A convex polygon: `n` points on an ellipse, rotated and translated.
fn polygon() -> impl Strategy<Value = Poly> {
    (
        3usize..7,
        0.05f32..3.0,
        0.05f32..3.0,
        -3.2f32..3.2,
        -5.0f32..5.0,
        -5.0f32..5.0,
    )
        .prop_map(|(n, rx, ry, rot, tx, ty)| {
            let (s, c) = (libm::sinf(rot), libm::cosf(rot));
            (0..n)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / n as f32;
                    let (x, y) = (rx * libm::cosf(a), ry * libm::sinf(a));
                    [c * x - s * y + tx, s * x + c * y + ty]
                })
                .collect()
        })
}

fn params() -> impl Strategy<Value = PackParams> {
    (
        prop_oneof![
            Just(RotationMethod::None),
            Just(RotationMethod::Any),
            Just(RotationMethod::Cardinal),
            Just(RotationMethod::AxisAligned),
            Just(RotationMethod::AxisAlignedX),
            Just(RotationMethod::AxisAlignedY),
        ],
        prop_oneof![
            Just(MarginMethod::Scaled),
            Just(MarginMethod::Add),
            Just(MarginMethod::Fraction)
        ],
        0.0f32..0.05,
    )
        .prop_map(|(rotate_method, margin_method, margin)| {
            let mut p = PackParams::default();
            p.rotate_method = rotate_method;
            p.margin_method = margin_method;
            p.margin = margin;
            p.shape_method = ShapeMethod::Aabb;
            p
        })
}

fn area(p: &[[f32; 2]]) -> f64 {
    let mut a = 0.0f64;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += f64::from(u[0]) * f64::from(v[1]) - f64::from(v[0]) * f64::from(u[1]);
    }
    a.abs() * 0.5
}

fn bounds(p: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for q in p {
        for k in 0..2 {
            min[k] = min[k].min(q[k]);
            max[k] = max[k].max(q[k]);
        }
    }
    (min, max)
}

/// Strict overlap of two convex polygons (separating axes).
fn convex_overlap(a: &[[f32; 2]], b: &[[f32; 2]], eps: f32) -> bool {
    for (t, o) in [(a, b), (b, a)] {
        for i in 0..t.len() {
            let (p, q) = (t[i], t[(i + 1) % t.len()]);
            let n = [q[1] - p[1], p[0] - q[0]];
            let len = (n[0] * n[0] + n[1] * n[1]).sqrt();
            if len == 0.0 {
                continue;
            }
            let proj = |x: [f32; 2]| (n[0] * x[0] + n[1] * x[1]) / len;
            let (amin, amax) = t.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| {
                (lo.min(proj(x)), hi.max(proj(x)))
            });
            let (bmin, bmax) = o.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| {
                (lo.min(proj(x)), hi.max(proj(x)))
            });
            if amax <= bmin + eps || bmax <= amin + eps {
                return false;
            }
        }
    }
    true
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn packed_islands_fit_without_overlap(
        polys in prop::collection::vec(polygon(), 1..10),
        params in params(),
    ) {
        let mut islands: Vec<PackIsland> = polys
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut island = PackIsland::new();
                island.caller_index = i as i32;
                island.add_polygon(p);
                island
            })
            .collect();
        let scale = pack::pack_islands(&mut islands, &params);
        prop_assert!(scale > 0.0 && scale.is_finite());
        let packed: Vec<Poly> = islands
            .iter()
            .map(|island| {
                let mut uvs = polys[island.caller_index as usize].clone();
                pack::apply_to_uvs(island, scale, &params, &mut uvs);
                uvs
            })
            .collect();
        let tol = 1e-4f32;
        for (i, p) in packed.iter().enumerate() {
            let (min, max) = bounds(p);
            prop_assert!(
                min[0] >= -tol && min[1] >= -tol && max[0] <= 1.0 + tol && max[1] <= 1.0 + tol,
                "island {} outside the unit square: {:?} {:?}", i, min, max
            );
        }
        for a in 0..packed.len() {
            for b in a + 1..packed.len() {
                prop_assert!(
                    !convex_overlap(&packed[a], &packed[b], tol),
                    "islands {} and {} overlap", a, b
                );
            }
        }
        // One common scale: the area ratio is the same for every island.
        let ratio = |i: usize| area(&packed[i]) / area(&polys[i]);
        let r0 = ratio(0);
        for i in 1..packed.len() {
            let ri = ratio(i);
            prop_assert!((ri - r0).abs() <= 1e-3 * r0.max(1e-12), "area ratio {} vs {}", ri, r0);
        }
    }

    #[test]
    fn without_scale_to_fit_islands_keep_their_size(
        polys in prop::collection::vec(polygon(), 1..6),
        mut params in params(),
    ) {
        params.scale_to_fit = false;
        let mut islands: Vec<PackIsland> = polys
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut island = PackIsland::new();
                island.caller_index = i as i32;
                island.add_polygon(p);
                island
            })
            .collect();
        let scale = pack::pack_islands(&mut islands, &params);
        prop_assert_eq!(scale, 1.0);
        for island in &islands {
            let p = &polys[island.caller_index as usize];
            let mut uvs = p.clone();
            pack::apply_to_uvs(island, scale, &params, &mut uvs);
            let (a, b) = (area(p), area(&uvs));
            prop_assert!((a - b).abs() <= 1e-3 * a, "area {} became {}", a, b);
        }
    }
}
