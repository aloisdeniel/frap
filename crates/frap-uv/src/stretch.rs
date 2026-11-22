// SPDX-License-Identifier: GPL-3.0-or-later
//! Stretch minimisation (`uv.minimize_stretch`: `uv_parametrizer_stretch_begin/blend/iter/end`
//! and `p_chart_stretch_minimize` of `uv_parametrizer.cc`): with the chart boundaries pinned
//! (user pins are ignored), every selected interior vertex moves in a random direction (Blender's
//! `BLI_rng` seeded with 31415926, so runs are repeatable) by the binary-searched step that
//! lowers the Sander texture-stretch metric of its faces, each iteration once per vertex. Flushing
//! blends the result with the original UVs (`blend`).
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc and
//! blender/source/blender/blenlib/BLI_rand.hh (Blender 5.2.4).
//!
//! Known source of ulp divergence: the random direction uses `libm::cosf`/`sinf` (rule
//! 2.2.12, portable transcendentals), while Blender's macOS build calls Apple's libm, whose
//! `cosf` differs in the last bit on about 1.3% of the RNG's angles. A one-ulp different
//! direction can change a later binary-search step of `p_chart_stretch_minimize` on some
//! inputs; no golden or probe has shown a difference above the 1e-4 tolerance so far.

use crate::param::bli;
use crate::param::{NIL, PFACE_FILLED, PVERT_PIN, PVERT_SELECT, ParamMesh, PhState};

/// `P_STRETCH_ITER`.
const P_STRETCH_ITER: usize = 20;

/// `RandomNumberGenerator` (`BLI_rng`): the 48-bit `drand48` congruential generator.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rng {
    x: u64,
}

impl Rng {
    /// `BLI_rng_new(seed)` (`seed`).
    pub(crate) fn new(seed: u32) -> Rng {
        const LOWSEED: u64 = 0x330E;
        Rng {
            x: (u64::from(seed) << 16) | LOWSEED,
        }
    }

    fn step(&mut self) {
        const MULTIPLIER: u64 = 0x5_DEEC_E66D;
        const ADDEND: u64 = 0xB;
        const MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
        self.x = (MULTIPLIER.wrapping_mul(self.x).wrapping_add(ADDEND)) & MASK;
    }

    /// `get_int32`.
    fn get_int32(&mut self) -> i32 {
        self.step();
        (self.x >> 17) as i32
    }

    /// `BLI_rng_get_float`: in `[0, 1)`.
    pub(crate) fn get_float(&mut self) -> f32 {
        self.get_int32() as f32 / 2_147_483_648.0f32
    }
}

impl ParamMesh {
    /// `p_edge_uv_length`.
    fn edge_uv_length(&self, e: u32) -> f32 {
        let ed = &self.edges[e as usize];
        let a = self.verts[ed.vert as usize].uv;
        let b = self.verts[self.edges[ed.next as usize].vert as usize].uv;
        bli::len_v2v2(a, b)
    }

    /// `p_face_stretch`.
    fn face_stretch(&self, f: u32) -> f32 {
        let [e1, e2, e3] = self.face_edges(f);
        let [v1, v2, v3] = self.face_verts(f);
        let (p1, p2, p3) = (
            &self.verts[v1 as usize],
            &self.verts[v2 as usize],
            &self.verts[v3 as usize],
        );
        let area = self.face_uv_area_signed(f);
        if area <= 0.0 {
            // A flipped face: a large penalty, with a slight gradient to unflip it.
            return 1e8f32
                * (1.0
                    + self.edge_uv_length(e1)
                    + self.edge_uv_length(e2)
                    + self.edge_uv_length(e3));
        }
        let w = 1.0f32 / (2.0 * area);

        let mul = |a: [f32; 3], s: f32| [a[0] * s, a[1] * s, a[2] * s];
        let add = |a: [f32; 3], b: [f32; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];

        // Derivatives.
        let mut ps = mul(p1.co, p2.uv[1] - p3.uv[1]);
        ps = add(ps, mul(p2.co, p3.uv[1] - p1.uv[1]));
        ps = add(ps, mul(p3.co, p1.uv[1] - p2.uv[1]));
        ps = mul(ps, w);

        let mut pt = mul(p1.co, p3.uv[0] - p2.uv[0]);
        pt = add(pt, mul(p2.co, p1.uv[0] - p3.uv[0]));
        pt = add(pt, mul(p3.co, p2.uv[0] - p1.uv[0]));
        pt = mul(pt, w);

        // Sander tensor.
        let a = bli::dot3(ps, ps);
        let c = bli::dot3(pt, pt);
        let mut t = (0.5f32 * (a + c)).sqrt();
        if self.faces[f as usize].flag & PFACE_FILLED != 0 {
            t *= 0.2;
        }
        t
    }

    /// `p_stretch_compute_vertex`.
    fn stretch_compute_vertex(&self, v: u32) -> f32 {
        let start = self.verts[v as usize].edge;
        let mut e = start;
        let mut sum = 0.0f32;
        loop {
            sum += self.face_stretch(self.edges[e as usize].face);
            e = self.wheel_edge_next(e);
            if e == NIL || e == start {
                break;
            }
        }
        sum
    }

    /// `p_chart_stretch_minimize`.
    fn chart_stretch_minimize(&mut self, c: usize) {
        for v in self.chart_vert_list(c) {
            frap_math::budget::check();
            let flag = self.verts[v as usize].flag;
            if flag & PVERT_PIN != 0 || flag & PVERT_SELECT == 0 {
                continue;
            }
            let orig_stretch = self.stretch_compute_vertex(v);
            let orig_uv = self.verts[v as usize].uv;

            // Move the vertex in a random direction.
            let mut trusted_radius = 0.0f32;
            let mut nedges = 0i32;
            let start = self.verts[v as usize].edge;
            let mut e = start;
            loop {
                trusted_radius += self.edge_uv_length(e);
                nedges += 1;
                e = self.wheel_edge_next(e);
                if e == NIL || e == start {
                    break;
                }
            }
            trusted_radius /= (2 * nedges) as f32;

            let random_angle = self.rng.get_float() * 2.0f32 * core::f32::consts::PI;
            let dir = [
                trusted_radius * libm::cosf(random_angle),
                trusted_radius * libm::sinf(random_angle),
            ];

            // Old and new stretch.
            let mut low = 0.0f32;
            let mut stretch_low = orig_stretch;
            self.verts[v as usize].uv = [orig_uv[0] + dir[0], orig_uv[1] + dir[1]];
            let mut high = 1.0f32;
            let mut stretch = self.stretch_compute_vertex(v);
            let mut stretch_high = stretch;

            // Binary search for the lowest stretch position.
            for _ in 0..P_STRETCH_ITER {
                let mid = 0.5f32 * (low + high);
                self.verts[v as usize].uv = [orig_uv[0] + mid * dir[0], orig_uv[1] + mid * dir[1]];
                stretch = self.stretch_compute_vertex(v);
                if stretch_low < stretch_high {
                    high = mid;
                    stretch_high = stretch;
                } else {
                    low = mid;
                    stretch_low = stretch;
                }
            }

            // No luck, the stretch increased: back to the old values.
            if stretch >= orig_stretch {
                self.verts[v as usize].uv = orig_uv;
            }
        }
    }

    /// `uv_parametrizer_stretch_begin`.
    pub fn stretch_begin(&mut self) {
        assert_eq!(self.state, PhState::Constructed);
        self.state = PhState::Stretch;
        self.rng = Rng::new(31_415_926);
        self.blend = 0.0;
        for c in 0..self.charts.len() {
            for v in self.chart_vert_list(c) {
                // Don't use user-defined pins.
                self.verts[v as usize].flag &= !PVERT_PIN;
            }
            // `p_stretch_pin_boundary`.
            for v in self.chart_vert_list(c) {
                let e = self.verts[v as usize].edge;
                if self.edges[e as usize].pair == NIL {
                    self.verts[v as usize].flag |= PVERT_PIN;
                } else {
                    self.verts[v as usize].flag &= !PVERT_PIN;
                }
            }
            for f in self.chart_face_list(c) {
                self.face_backup_uvs(f);
                self.faces[f as usize].area3d = self.face_area(f);
            }
        }
    }

    /// `uv_parametrizer_stretch_blend`: the blend factor of the next flushes.
    pub fn stretch_blend(&mut self, blend: f32) {
        assert_eq!(self.state, PhState::Stretch);
        self.blend = blend;
    }

    /// `uv_parametrizer_stretch_iter`: one iteration over every chart.
    pub fn stretch_iter(&mut self) {
        assert_eq!(self.state, PhState::Stretch);
        for c in 0..self.charts.len() {
            self.chart_stretch_minimize(c);
        }
    }

    /// `uv_parametrizer_stretch_end`.
    pub fn stretch_end(&mut self) {
        assert_eq!(self.state, PhState::Stretch);
        self.state = PhState::Constructed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::{ConstructOpts, ParamFace};

    #[test]
    fn rng_matches_blender() {
        // `BLI_rng_new(31415926)`: x = seed << 16 | 0x330E, then drand48 steps.
        let mut r = Rng::new(31_415_926);
        let mut x: u64 = (31_415_926u64 << 16) | 0x330E;
        for _ in 0..5 {
            x = (0x5DEE_CE66Du64.wrapping_mul(x) + 0xB) & 0xFFFF_FFFF_FFFF;
            let expect = ((x >> 17) as i32) as f32 / 2_147_483_648.0;
            assert_eq!(r.get_float(), expect);
        }
        let mut r = Rng::new(31_415_926);
        let v = r.get_float();
        assert!((0.0..1.0).contains(&v));
    }

    /// A 3 × 3 vertex grid with its centre vertex displaced in UV: stretch minimisation moves
    /// it back towards the centre and keeps the boundary.
    #[test]
    fn interior_vertex_relaxes() {
        let mut pm = ParamMesh::new();
        let n = 3usize;
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut slot = 0u32;
        for i in 0..n - 1 {
            for j in 0..n - 1 {
                let ids = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let vkeys: Vec<u64> = ids.iter().map(|&(a, b)| (a * n + b) as u64).collect();
                let cos: Vec<[f32; 3]> = ids
                    .iter()
                    .map(|&(a, b)| [a as f32 * 0.5, b as f32 * 0.5, 0.0])
                    .collect();
                let uv: Vec<[f32; 2]> = ids
                    .iter()
                    .map(|&(a, b)| {
                        if (a, b) == (1, 1) {
                            [0.65, 0.6]
                        } else {
                            [a as f32 * 0.5, b as f32 * 0.5]
                        }
                    })
                    .collect();
                let uvi: Vec<u32> = (0..4).map(|k| slot + k).collect();
                slot += 4;
                uvs.extend_from_slice(&uv);
                pm.face_add(&ParamFace::new(
                    (i * n + j) as u64,
                    &vkeys,
                    &cos,
                    &uvi,
                    &uv,
                    &[false; 4],
                    &[true; 4],
                ));
            }
        }
        pm.construct_end(ConstructOpts::new(true, true));
        pm.stretch_begin();
        for _ in 0..50 {
            pm.stretch_iter();
        }
        let mut out = uvs.clone();
        pm.flush(&mut out);
        pm.stretch_end();
        for (k, (a, b)) in uvs.iter().zip(&out).enumerate() {
            if *a == [0.65, 0.6] {
                let d = ((b[0] - 0.5) * (b[0] - 0.5) + (b[1] - 0.5) * (b[1] - 0.5)).sqrt();
                assert!(d < 0.05, "corner {k}: {b:?}");
            } else {
                assert_eq!(a, b, "boundary corner {k} moved");
            }
        }
    }
}
