// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_box_pack_2d`: packs rectangles towards the origin by trying every free corner of the
//! boxes placed so far (sorted by how little the bounds would grow), with
//! `USE_MERGE`, `USE_FREE_STRIP` and `USE_PACK_BIAS`.
//!
//! Ported from blender/source/blender/blenlib/intern/boxpack_2d.cc (Blender 5.2.4). Pointers
//! to boxes and vertices are indices; the vertex sort is Blender's unstable `BLI_qsort_r`
//! ([`super::qsort::qsort_r`]) because the order of equal keys decides the layout.

use core::cmp::Ordering;

use super::qsort::qsort_r;

/// `BoxPack` (input `w`, `h`; output `x`, `y`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct BoxPack {
    /// Output: left.
    pub x: f32,
    /// Output: bottom.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
    v: [usize; 4],
}

impl BoxPack {
    /// A box of the given size.
    pub fn new(w: f32, h: f32) -> BoxPack {
        BoxPack {
            w,
            h,
            ..BoxPack::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct BoxVert {
    x: f32,
    y: f32,
    free: i32,
    used: bool,
    index: usize,
    bias: f32,
    trb: Option<usize>,
    blb: Option<usize>,
    brb: Option<usize>,
    tlb: Option<usize>,
    isect_cache: [Option<usize>; 4],
}

const EPSILON: f32 = 0.000_000_1;
const EPSILON_MERGE: f32 = 0.000_01;
const EPSILON_BIAS: f32 = 0.000_001;
const BLF: i32 = 1;
const TRF: i32 = 2;
const TLF: i32 = 4;
const BRF: i32 = 8;
const CORNERFLAGS: i32 = BLF | TRF | TLF | BRF;
const BL: usize = 0;
const TR: usize = 1;
const TL: usize = 2;
const BR: usize = 3;

#[inline]
fn quad_flag(q: usize) -> i32 {
    1 << q
}

struct Packer {
    boxes: Vec<BoxPack>,
    verts: Vec<BoxVert>,
}

impl Packer {
    fn xmin(&self, b: usize) -> f32 {
        self.verts[self.boxes[b].v[BL]].x
    }
    fn xmax(&self, b: usize) -> f32 {
        self.verts[self.boxes[b].v[TR]].x
    }
    fn ymin(&self, b: usize) -> f32 {
        self.verts[self.boxes[b].v[BL]].y
    }
    fn ymax(&self, b: usize) -> f32 {
        self.verts[self.boxes[b].v[TR]].y
    }
    fn v34x_update(&mut self, b: usize) {
        let v = self.boxes[b].v;
        self.verts[v[TL]].x = self.verts[v[BL]].x;
        self.verts[v[BR]].x = self.verts[v[TR]].x;
    }
    fn v34y_update(&mut self, b: usize) {
        let v = self.boxes[b].v;
        self.verts[v[TL]].y = self.verts[v[TR]].y;
        self.verts[v[BR]].y = self.verts[v[BL]].y;
    }
    fn xmin_set(&mut self, b: usize, f: f32) {
        let (v, w) = (self.boxes[b].v, self.boxes[b].w);
        self.verts[v[TR]].x = f + w;
        self.verts[v[BL]].x = f;
        self.v34x_update(b);
    }
    fn xmax_set(&mut self, b: usize, f: f32) {
        let (v, w) = (self.boxes[b].v, self.boxes[b].w);
        self.verts[v[BL]].x = f - w;
        self.verts[v[TR]].x = f;
        self.v34x_update(b);
    }
    fn ymin_set(&mut self, b: usize, f: f32) {
        let (v, h) = (self.boxes[b].v, self.boxes[b].h);
        self.verts[v[TR]].y = f + h;
        self.verts[v[BL]].y = f;
        self.v34y_update(b);
    }
    fn ymax_set(&mut self, b: usize, f: f32) {
        let (v, h) = (self.boxes[b].v, self.boxes[b].h);
        self.verts[v[BL]].y = f - h;
        self.verts[v[TR]].y = f;
        self.v34y_update(b);
    }
    fn isect(&self, a: usize, b: usize) -> bool {
        !(self.xmin(a) + EPSILON >= self.xmax(b)
            || self.ymin(a) + EPSILON >= self.ymax(b)
            || self.xmax(a) - EPSILON <= self.xmin(b)
            || self.ymax(a) - EPSILON <= self.ymin(b))
    }
    fn bias_update(&mut self, v: usize) {
        let vert = &mut self.verts[v];
        vert.bias = (vert.x * vert.y) * EPSILON_BIAS;
    }

    /// The `USE_MERGE` block: `A`/`B` are box vertex slots `(box, corner)`, one of them used.
    fn merge(&mut self, a: (usize, usize), b: (usize, usize), mask: i32) {
        let va = self.boxes[a.0].v[a.1];
        let vb = self.boxes[b.0].v[b.1];
        if self.verts[va].used {
            let fb = self.verts[vb].free;
            self.verts[va].free &= fb & !mask;
            self.boxes[b.0].v[b.1] = va;
        } else {
            let fa = self.verts[va].free;
            self.verts[vb].free &= fa & !mask;
            self.boxes[a.0].v[a.1] = vb;
        }
    }
}

/// `BLI_box_pack_2d(boxarray, len, sort_boxes = false, &tot_x, &tot_y)`: places `boxes` in the
/// given order and returns the packed extent.
pub fn box_pack_2d(boxes: &mut [BoxPack]) -> (f32, f32) {
    let len = boxes.len();
    if len == 0 {
        return (0.0, 0.0);
    }
    let mut p = Packer {
        boxes: boxes.to_vec(),
        verts: vec![BoxVert::default(); 4 * len],
    };
    let mut vertex_pack_indices: Vec<usize> = vec![0; 3 * len];

    let mut i = 0usize;
    for b in 0..len {
        let base = 4 * b;
        let flags = [
            CORNERFLAGS & !TRF,
            CORNERFLAGS & !BLF,
            CORNERFLAGS & !BRF,
            CORNERFLAGS & !TLF,
        ];
        for (k, &free) in flags.iter().enumerate() {
            let v = &mut p.verts[base + k];
            *v = BoxVert::default();
            v.free = free;
            v.index = i;
            i += 1;
        }
        p.verts[base + BL].trb = Some(b);
        p.verts[base + TR].blb = Some(b);
        p.verts[base + TL].brb = Some(b);
        p.verts[base + BR].tlb = Some(b);
        p.boxes[b].v = [base + BL, base + TR, base + TL, base + BR];
    }

    // Pack the first box.
    {
        let v = p.boxes[0].v;
        p.verts[v[BL]].free = 0;
        p.verts[v[BR]].free &= !(BLF | BRF);
        p.verts[v[TL]].free &= !(BLF | TLF);
    }
    let mut tot_x = p.boxes[0].w;
    let mut tot_y = p.boxes[0].h;
    p.xmin_set(0, 0.0);
    p.ymin_set(0, 0.0);
    p.boxes[0].x = 0.0;
    p.boxes[0].y = 0.0;
    for k in 0..4 {
        let v = p.boxes[0].v[k];
        p.verts[v].used = true;
        p.bias_update(v);
    }
    for (k, slot) in vertex_pack_indices.iter_mut().take(3).enumerate() {
        *slot = p.verts[p.boxes[0].v[k + 1]].index;
    }
    let mut verts_pack_len = 3usize;

    for b in 1..len {
        frap_math::budget::check();
        let (box_width, box_height) = (p.boxes[b].w, p.boxes[b].h);
        {
            let verts = &p.verts;
            qsort_r(
                &mut vertex_pack_indices[..verts_pack_len],
                &mut |&i1: &usize, &i2: &usize| {
                    let (v1, v2) = (&verts[i1], &verts[i2]);
                    // Push free verts to the end so we can strip.
                    if v1.free == 0 && v2.free == 0 {
                        return Ordering::Equal;
                    }
                    if v1.free == 0 {
                        return Ordering::Greater;
                    }
                    if v2.free == 0 {
                        return Ordering::Less;
                    }
                    let a1 = (v1.x + box_width).max(v1.y + box_height) + v1.bias;
                    let a2 = (v2.x + box_width).max(v2.y + box_height) + v2.bias;
                    if a1 > a2 {
                        Ordering::Greater
                    } else if a1 < a2 {
                        Ordering::Less
                    } else {
                        Ordering::Equal
                    }
                },
            );
        }
        // Strip free vertices.
        let mut k = verts_pack_len - 1;
        while k != 0 && p.verts[vertex_pack_indices[k]].free == 0 {
            k -= 1;
        }
        verts_pack_len = k + 1;

        let mut isect = true;
        let mut vi = 0usize;
        while vi < verts_pack_len && isect {
            let vert = vertex_pack_indices[vi];
            let mut j = 0usize;
            while j < 4 && isect {
                if p.verts[vert].free & quad_flag(j) != 0 {
                    let (vx, vy) = (p.verts[vert].x, p.verts[vert].y);
                    match j {
                        BL => {
                            p.xmax_set(b, vx);
                            p.ymax_set(b, vy);
                        }
                        TR => {
                            p.xmin_set(b, vx);
                            p.ymin_set(b, vy);
                        }
                        TL => {
                            p.xmax_set(b, vx);
                            p.ymin_set(b, vy);
                        }
                        _ => {
                            p.xmin_set(b, vx);
                            p.ymax_set(b, vy);
                        }
                    }
                    isect = false;
                    if p.xmin(b) < 0.0
                        || p.ymin(b) < 0.0
                        || p.verts[vert].isect_cache[j].is_some_and(|c| p.isect(b, c))
                    {
                        isect = true;
                    } else {
                        for box_test in 0..b {
                            if p.isect(b, box_test) {
                                p.verts[vert].isect_cache[j] = Some(box_test);
                                isect = true;
                                break;
                            }
                        }
                    }

                    if !isect {
                        tot_x = p.xmax(b).max(tot_x);
                        tot_y = p.ymax(b).max(tot_y);

                        p.verts[vert].free &= !quad_flag(j);
                        match j {
                            TR => {
                                p.boxes[b].v[BL] = vert;
                                p.verts[vert].trb = Some(b);
                            }
                            TL => {
                                p.boxes[b].v[BR] = vert;
                                p.verts[vert].tlb = Some(b);
                            }
                            BR => {
                                p.boxes[b].v[TL] = vert;
                                p.verts[vert].brb = Some(b);
                            }
                            _ => {
                                p.boxes[b].v[TR] = vert;
                                p.verts[vert].blb = Some(b);
                            }
                        }

                        // Mask free flags for verts on the bottom or side.
                        if p.xmin(b) <= 0.0 {
                            let v = p.boxes[b].v;
                            p.verts[v[TL]].free &= !(TLF | BLF);
                            p.verts[v[BL]].free &= !(TLF | BLF);
                        } else if p.ymin(b) <= 0.0 {
                            let v = p.boxes[b].v;
                            p.verts[v[BL]].free &= !(BRF | BLF);
                            p.verts[v[BR]].free &= !(BRF | BLF);
                        }

                        let vx = p.verts[vert];
                        // Vertical.
                        if let (Some(tlb), Some(trb)) = (vx.tlb, vx.trb)
                            && (b == tlb || b == trb)
                        {
                            if (p.boxes[tlb].h - p.boxes[trb].h).abs() < EPSILON_MERGE {
                                p.merge((trb, TL), (tlb, TR), BLF | BRF);
                            } else if p.boxes[tlb].h > p.boxes[trb].h {
                                let v = p.boxes[trb].v[TL];
                                p.verts[v].free &= !(TLF | BLF);
                            } else {
                                let v = p.boxes[tlb].v[TR];
                                p.verts[v].free &= !(TRF | BRF);
                            }
                        } else if let (Some(blb), Some(brb)) = (vx.blb, vx.brb)
                            && (b == blb || b == brb)
                        {
                            if (p.boxes[blb].h - p.boxes[brb].h).abs() < EPSILON_MERGE {
                                p.merge((blb, BR), (brb, BL), TRF | TLF);
                            } else if p.boxes[blb].h > p.boxes[brb].h {
                                let v = p.boxes[brb].v[BL];
                                p.verts[v].free &= !(TLF | BLF);
                            } else {
                                let v = p.boxes[blb].v[BR];
                                p.verts[v].free &= !(TRF | BRF);
                            }
                        }
                        // Horizontal.
                        if let (Some(tlb), Some(blb)) = (vx.tlb, vx.blb)
                            && (b == tlb || b == blb)
                        {
                            if (p.boxes[tlb].w - p.boxes[blb].w).abs() < EPSILON_MERGE {
                                p.merge((blb, TL), (tlb, BL), TRF | BRF);
                            } else if p.boxes[tlb].w > p.boxes[blb].w {
                                let v = p.boxes[blb].v[TL];
                                p.verts[v].free &= !(TLF | TRF);
                            } else {
                                let v = p.boxes[tlb].v[BL];
                                p.verts[v].free &= !(BLF | BRF);
                            }
                        } else if let (Some(trb), Some(brb)) = (vx.trb, vx.brb)
                            && (b == trb || b == brb)
                        {
                            if (p.boxes[trb].w - p.boxes[brb].w).abs() < EPSILON_MERGE {
                                p.merge((brb, TR), (trb, BR), TLF | BLF);
                            } else if p.boxes[trb].w > p.boxes[brb].w {
                                let v = p.boxes[brb].v[TR];
                                p.verts[v].free &= !(TLF | TRF);
                            } else {
                                let v = p.boxes[trb].v[BR];
                                p.verts[v].free &= !(BLF | BRF);
                            }
                        }

                        for k in 0..4 {
                            let v = p.boxes[b].v[k];
                            if !p.verts[v].used {
                                p.verts[v].used = true;
                                p.bias_update(v);
                                vertex_pack_indices[verts_pack_len] = p.verts[v].index;
                                verts_pack_len += 1;
                            }
                        }
                        p.boxes[b].x = p.xmin(b);
                        p.boxes[b].y = p.ymin(b);
                    }
                }
                j += 1;
            }
            vi += 1;
        }
    }

    for (dst, src) in boxes.iter_mut().zip(&p.boxes) {
        dst.x = src.x;
        dst.y = src.y;
    }
    (tot_x, tot_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &BoxPack, b: &BoxPack) -> bool {
        a.x < b.x + b.w - 1e-6
            && b.x < a.x + a.w - 1e-6
            && a.y < b.y + b.h - 1e-6
            && b.y < a.y + a.h - 1e-6
    }

    #[test]
    fn packs_without_overlap() {
        let sizes = [
            (3.0, 2.0),
            (2.0, 2.0),
            (1.0, 3.0),
            (1.0, 1.0),
            (2.0, 1.0),
            (0.5, 0.5),
        ];
        let mut boxes: Vec<BoxPack> = sizes.iter().map(|&(w, h)| BoxPack::new(w, h)).collect();
        let (tx, ty) = box_pack_2d(&mut boxes);
        for (i, a) in boxes.iter().enumerate() {
            assert!(a.x >= 0.0 && a.y >= 0.0);
            assert!(a.x + a.w <= tx + 1e-6 && a.y + a.h <= ty + 1e-6);
            for b in &boxes[i + 1..] {
                assert!(!overlap(a, b), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn equal_squares_pack_in_a_grid() {
        let mut boxes = vec![BoxPack::new(1.0, 1.0); 4];
        let (tx, ty) = box_pack_2d(&mut boxes);
        assert_eq!((tx, ty), (2.0, 2.0));
    }
}
