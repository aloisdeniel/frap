// SPDX-License-Identifier: GPL-3.0-or-later
//! The Gregory end caps of irregular patches: `Far::SourcePatch` (the source points around an
//! irregular face), `SparseMatrix<float>` and `GregoryConverter<float>` with the limit
//! weights of `CatmarkLimits<float>` (computed in `double`, stored in `float`), producing the
//! 20 Gregory basis points as rows over the source points.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/{patchBuilder,catmarkPatchBuilder,
//! sparseMatrix}.{h,cpp}` (Pixar, TOST / Apache-2.0 modified). The float expressions follow
//! clang's contraction of `a * b + c` within one expression (`f32::mul_add` / `f64::mul_add`);
//! the trigonometry is the C library's (`cos`, `sin`, `cosf`, `sinf`) as in Blender's build.

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::trig;

/// `SourcePatch::Corner`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Corner {
    pub num_faces: i32,
    pub patch_face: i32,
    pub boundary: bool,
    pub sharp: bool,
    pub dart: bool,
    pub shares_with_prev: bool,
    pub shares_with_next: bool,
    pub val2_interior: bool,
    pub val2_adjacent: bool,
}

/// `Far::SourcePatch`
#[derive(Clone, Debug, Default)]
pub(crate) struct SourcePatch {
    pub corners: [Corner; 4],
    pub num_corners: usize,
    pub num_source_points: i32,
    pub max_valence: i32,
    pub max_ring_size: i32,
    pub ring_sizes: [i32; 4],
    pub local_ring_sizes: [i32; 4],
    pub local_ring_offsets: [i32; 4],
}

fn fast_mod_n(x: usize, n: usize) -> usize {
    if x < n { x } else { x - n }
}

impl SourcePatch {
    /// `SourcePatch::Finalize`
    pub(crate) fn finalize(&mut self, size: usize) {
        let is_quad = size == 4;
        let q = is_quad as usize;
        self.num_corners = size;
        self.max_valence = 0;
        self.max_ring_size = 0;
        self.num_source_points = size as i32;
        for c in 0..size {
            let c_prev = fast_mod_n(c + 2 + q, size);
            let c_next = fast_mod_n(c + 1, size);
            let val2 = |k: usize| self.corners[k].num_faces == 2 && !self.corners[k].boundary;
            let prev_v2 = val2(c_prev);
            let this_v2 = val2(c);
            let next_v2 = val2(c_next);
            let (cp_boundary, cp_patch_face) = (
                self.corners[c_prev].boundary,
                self.corners[c_prev].patch_face,
            );
            let (cn_boundary, cn_patch_face, cn_faces) = (
                self.corners[c_next].boundary,
                self.corners[c_next].patch_face,
                self.corners[c_next].num_faces,
            );
            let corner = &mut self.corners[c];
            corner.val2_interior = this_v2;
            corner.val2_adjacent = prev_v2 || next_v2;
            if corner.num_faces + corner.boundary as i32 > 2 {
                if corner.boundary {
                    corner.shares_with_prev = is_quad && corner.patch_face != corner.num_faces - 1;
                    corner.shares_with_next = corner.patch_face != 0;
                } else if corner.dart {
                    let prev_on_dart = cp_boundary && cp_patch_face == 0;
                    let next_on_dart = cn_boundary && cn_patch_face == cn_faces - 1;
                    corner.shares_with_prev = is_quad && !prev_on_dart;
                    corner.shares_with_next = !next_on_dart;
                } else {
                    corner.shares_with_prev = is_quad;
                    corner.shares_with_next = true;
                }
                self.ring_sizes[c] = corner.num_faces * (1 + q as i32) + corner.boundary as i32;
                self.local_ring_sizes[c] = self.ring_sizes[c]
                    - (size as i32 - 1)
                    - corner.shares_with_prev as i32
                    - corner.shares_with_next as i32;
                if corner.val2_adjacent && !corner.boundary {
                    self.local_ring_sizes[c] -= prev_v2 as i32;
                    self.local_ring_sizes[c] -= (next_v2 && is_quad) as i32;
                }
            } else {
                corner.shares_with_prev = false;
                corner.shares_with_next = false;
                if corner.num_faces == 1 {
                    self.ring_sizes[c] = size as i32 - 1;
                    self.local_ring_sizes[c] = 0;
                } else {
                    self.ring_sizes[c] = 2 * (1 + q as i32);
                    self.local_ring_sizes[c] = q as i32;
                }
            }
            let corner = self.corners[c];
            self.local_ring_offsets[c] = self.num_source_points;
            self.max_valence = self
                .max_valence
                .max(corner.num_faces + corner.boundary as i32);
            self.max_ring_size = self.max_ring_size.max(self.ring_sizes[c]);
            self.num_source_points += self.local_ring_sizes[c];
        }
    }

    /// `SourcePatch::GetCornerRingPoints`
    pub(crate) fn corner_ring_points(&self, corner: usize, ring: &mut Vec<i32>) -> usize {
        ring.clear();
        let n = self.num_corners;
        let is_quad = n == 4;
        let q = is_quad as usize;
        let c_next = fast_mod_n(corner + 1, n);
        let c_opp = fast_mod_n(corner + 1 + q, n);
        let c_prev = fast_mod_n(corner + 2 + q, n);
        let cc = &self.corners;
        ring.push(c_next as i32);
        if is_quad {
            ring.push(c_opp as i32);
        }
        ring.push(c_prev as i32);
        if cc[c_prev].val2_interior && !cc[corner].boundary {
            ring.push(if is_quad { c_opp } else { c_next } as i32);
        }
        if cc[corner].shares_with_prev {
            ring.push(self.local_ring_offsets[c_prev] + self.local_ring_sizes[c_prev] - 1);
        }
        for i in 0..self.local_ring_sizes[corner] {
            ring.push(self.local_ring_offsets[corner] + i);
        }
        if is_quad {
            if cc[corner].shares_with_next {
                ring.push(self.local_ring_offsets[c_next]);
            }
            if cc[c_next].val2_interior && !cc[corner].boundary {
                ring.push(c_opp as i32);
            }
        } else if cc[corner].shares_with_next {
            if cc[c_next].val2_interior && !cc[corner].boundary {
                ring.push(c_prev as i32);
            } else if self.local_ring_sizes[c_next] == 0 {
                ring.push(self.local_ring_offsets[c_prev]);
            } else {
                ring.push(self.local_ring_offsets[c_next]);
            }
        }
        let size = ring.len();
        if cc[corner].patch_face != 0 {
            let rot = size as i32 - (1 + q as i32) * cc[corner].patch_face;
            ring.rotate_left(rot.rem_euclid(size as i32) as usize);
        }
        size
    }
}

/// `Far::SparseMatrix<float>`: rows of `(column, weight)` pairs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SparseMatrix {
    pub num_rows: usize,
    pub num_columns: usize,
    pub row_offsets: Vec<usize>,
    pub columns: Vec<i32>,
    pub elements: Vec<f32>,
}

impl SparseMatrix {
    fn with_row_sizes(num_columns: usize, sizes: &[usize]) -> SparseMatrix {
        let mut offsets = vec![0usize];
        for &s in sizes {
            offsets.push(offsets.last().copied().unwrap_or(0) + s);
        }
        let n = *offsets.last().unwrap_or(&0);
        SparseMatrix {
            num_rows: sizes.len(),
            num_columns,
            row_offsets: offsets,
            // One spare element: `computeIrregularBoundaryEdgePoints` writes one past its `em`
            // row (into the next row, overwritten later) as OpenSubdiv does.
            columns: vec![0; n + 1],
            elements: vec![0.0; n + 1],
        }
    }

    pub(crate) fn row_size(&self, r: usize) -> usize {
        self.row_offsets[r + 1] - self.row_offsets[r]
    }

    pub(crate) fn row_columns(&self, r: usize) -> &[i32] {
        &self.columns[self.row_offsets[r]..self.row_offsets[r + 1]]
    }

    pub(crate) fn row_elements(&self, r: usize) -> &[f32] {
        &self.elements[self.row_offsets[r]..self.row_offsets[r + 1]]
    }

    fn assign(&mut self, r: usize, k: usize, column: i32, weight: f32) {
        let i = self.row_offsets[r] + k;
        self.columns[i] = column;
        self.elements[i] = weight;
    }

    fn copy_row(&mut self, dst: usize, src: usize) {
        let n = self.row_size(src);
        let (s, d) = (self.row_offsets[src], self.row_offsets[dst]);
        for k in 0..n {
            self.columns[d + k] = self.columns[s + k];
            self.elements[d + k] = self.elements[s + k];
        }
    }
}

/// `CatmarkLimits<REAL>::computeCoefficient`
fn compute_coefficient(valence: i32) -> f64 {
    // OpenSubdiv's literal table (more digits than an `f64` holds; the same values).
    #[allow(clippy::excessive_precision)]
    const EF: [f64; 30] = [
        0.0,
        0.0,
        0.0,
        8.1281572906372312e-01,
        0.5,
        3.6364406329142801e-01,
        2.8751379706077085e-01,
        2.3868786685851678e-01,
        2.0454364190756097e-01,
        1.7922903958061159e-01,
        1.5965737079986253e-01,
        1.4404233443011302e-01,
        1.3127568415883017e-01,
        1.2063172212675841e-01,
        1.1161437506676930e-01,
        1.0387245516114274e-01,
        9.7150019090724835e-02,
        9.1255917505950648e-02,
        8.6044378511602668e-02,
        8.1402211336798411e-02,
        7.7240129516184072e-02,
        7.3486719751997026e-02,
        7.0084157479797987e-02,
        6.6985104030725440e-02,
        6.4150420569810074e-02,
        6.1547457638637268e-02,
        5.9148757447233989e-02,
        5.6931056818776957e-02,
        5.4874512279256417e-02,
        5.2962091433796134e-02,
    ];
    if valence < 30 {
        return EF[valence as usize];
    }
    let inv = 1.0 / valence as f64;
    let cos_t = trig::cos(2.0 * std::f64::consts::PI * inv);
    let divisor = (cos_t + 5.0) + ((cos_t + 9.0) * (cos_t + 1.0)).sqrt();
    16.0 * inv / divisor
}

/// `CatmarkLimits<float>::ComputeInteriorPointWeights` (with edge points).
fn interior_point_weights(
    valence: i32,
    face_in_ring: i32,
    p: &mut [f32],
    ep: &mut [f32],
    em: &mut [f32],
) {
    let fv = valence as f64;
    let one_over_valence = 1.0 / fv;
    let one_over_val_plus5 = 1.0 / (fv + 5.0);
    let p_coeff = one_over_valence * one_over_val_plus5;
    let tan_coeff = compute_coefficient(valence) * 0.5 * one_over_val_plus5;
    let face_angle = 2.0 * std::f64::consts::PI * one_over_valence;
    let width = (1 + 2 * valence) as usize;
    let mut tan = vec![0.0f32; width];
    p[0] = (fv * one_over_val_plus5) as f32;
    let n = valence as usize;
    for i in 0..n {
        p[1 + 2 * i] = (p_coeff * 4.0) as f32;
        p[1 + 2 * i + 1] = p_coeff as f32;
        let i_prev = (i + n - 1) % n;
        let i_next = (i + 1) % n;
        let cos_i = tan_coeff * trig::cos(face_angle * i as f64);
        tan[1 + 2 * i_prev] += (cos_i * 2.0) as f32;
        tan[1 + 2 * i_prev + 1] += cos_i as f32;
        tan[1 + 2 * i] += (cos_i * 4.0) as f32;
        tan[1 + 2 * i + 1] += cos_i as f32;
        tan[1 + 2 * i_next] += (cos_i * 2.0) as f32;
    }
    let ep_off = 2 * ((valence - face_in_ring) % valence) as usize;
    let em_off = 2 * ((valence - face_in_ring - 1 + valence) % valence) as usize;
    ep[0] = p[0];
    em[0] = p[0];
    for i in 1..width {
        let mut ip = i + ep_off;
        if ip >= width {
            ip -= width - 1;
        }
        let mut im = i + em_off;
        if im >= width {
            im -= width - 1;
        }
        ep[i] = p[i] + tan[ip];
        em[i] = p[i] + tan[im];
    }
}

/// `CatmarkLimits<float>::ComputeBoundaryPointWeights` (with edge points).
fn boundary_point_weights(
    valence: i32,
    face_in_ring: i32,
    p: &mut [f32],
    ep: &mut [f32],
    em: &mut [f32],
) {
    let num_faces = valence - 1;
    let face_angle = std::f64::consts::PI / num_faces as f64;
    let width = (2 * valence) as usize;
    let n = width - 1;
    for x in p[..width].iter_mut() {
        *x = 0.0;
    }
    p[0] = (4.0f64 / 6.0) as f32;
    p[1] = (1.0f64 / 6.0) as f32;
    p[n] = (1.0f64 / 6.0) as f32;
    let t1 = 1.0f64 / 6.0;
    let tn = -1.0f64 / 6.0;
    let mut tan = vec![0.0f32; width];
    {
        let k = num_faces as f64;
        let theta = face_angle;
        let (s, c) = trig::sincos(theta);
        let div3 = 1.0f64 / 3.0;
        let div3kc = 1.0 / 3.0f64.mul_add(k, c);
        let gamma = -4.0 * s * div3kc;
        let alpha_0k = -(2.0f64.mul_add(c, 1.0) * (1.0 + c).sqrt()) * div3kc / (1.0 - c).sqrt();
        let beta_0 = s * div3kc;
        tan[0] = (gamma * div3) as f32;
        tan[1] = (alpha_0k * div3) as f32;
        tan[2] = (beta_0 * div3) as f32;
        tan[n] = (alpha_0k * div3) as f32;
        for i in 1..(valence - 1) as usize {
            let sin_i = trig::sin(theta * i as f64);
            let sin_i1 = trig::sin(theta * (i + 1) as f64);
            let alpha = 4.0 * sin_i * div3kc;
            let beta = (sin_i + sin_i1) * div3kc;
            tan[1 + 2 * i] = (alpha * div3) as f32;
            tan[1 + 2 * i + 1] = (beta * div3) as f32;
        }
    }
    if face_in_ring == 0 {
        for x in ep[..width].iter_mut() {
            *x = 0.0;
        }
        ep[0] = (2.0f64 / 3.0) as f32;
        ep[1] = (1.0f64 / 3.0) as f32;
    } else {
        let (sa, ca) = trig::sincos(face_angle * face_in_ring as f64);
        for i in 0..width {
            ep[i] = (tan[i] as f64 * sa) as f32;
        }
        ep[0] += p[0];
        ep[1] += p[1] + (t1 * ca) as f32;
        ep[n] += p[n] + (tn * ca) as f32;
    }
    if face_in_ring == num_faces - 1 {
        for x in em[..width].iter_mut() {
            *x = 0.0;
        }
        em[0] = (2.0f64 / 3.0) as f32;
        em[n] = (1.0f64 / 3.0) as f32;
    } else {
        let prev = (face_in_ring + 1) % valence;
        let (sa, ca) = trig::sincos(face_angle * prev as f64);
        for i in 0..width {
            em[i] = (tan[i] as f64 * sa) as f32;
        }
        em[0] += p[0];
        em[1] += p[1] + (t1 * ca) as f32;
        em[n] += p[n] + (tn * ca) as f32;
    }
}

/// `GregoryConverter::CornerTopology`
#[derive(Clone, Debug, Default)]
struct CornerTopology {
    is_boundary: bool,
    is_sharp: bool,
    is_regular: bool,
    is_val2_int: bool,
    ep_on_boundary: bool,
    em_on_boundary: bool,
    fp_is_regular: bool,
    fm_is_regular: bool,
    fp_is_copied: bool,
    fm_is_copied: bool,
    valence: i32,
    num_faces: i32,
    face_in_ring: i32,
    cos_face_angle: f32,
    ring: Vec<i32>,
}

/// `GregoryConverter<float>`
struct GregoryConverter {
    num_source_points: usize,
    max_valence: i32,
    isolated_interior: bool,
    has_val2_interior: bool,
    isolated_corner: usize,
    isolated_valence: i32,
    corners: [CornerTopology; 4],
}

/// `GregoryConverter<float>(sourcePatch, matrix)`: the 20 x `numSourcePoints` conversion.
pub(crate) fn convert(sp: &SourcePatch) -> SparseMatrix {
    let g = GregoryConverter::new(sp);
    g.convert()
}

impl GregoryConverter {
    fn new(sp: &SourcePatch) -> GregoryConverter {
        let mut g = GregoryConverter {
            num_source_points: sp.num_source_points as usize,
            max_valence: sp.max_valence,
            isolated_interior: false,
            has_val2_interior: false,
            isolated_corner: 0,
            isolated_valence: 0,
            corners: Default::default(),
        };
        let (mut boundary_n, mut irregular_n, mut sharp_n, mut val2_n) = (0, 0, 0, 0);
        let mut irregular_corner = 0usize;
        let mut irregular_valence = -1;
        for c in 0..4 {
            let src = sp.corners[c];
            let ct = &mut g.corners[c];
            ct.is_boundary = src.boundary;
            ct.is_sharp = src.sharp;
            ct.num_faces = src.num_faces;
            ct.face_in_ring = src.patch_face;
            ct.is_val2_int = src.val2_interior;
            ct.valence = ct.num_faces + ct.is_boundary as i32;
            ct.is_regular = (ct.num_faces << ct.is_boundary as i32) == 4 && !ct.is_sharp;
            if ct.is_regular {
                ct.cos_face_angle = 0.0;
            } else {
                let num = if ct.is_boundary {
                    std::f32::consts::PI
                } else {
                    2.0f32 * std::f32::consts::PI
                };
                let angle = num / ct.num_faces as f32;
                ct.cos_face_angle = trig::cosf(angle);
            }
            sp.corner_ring_points(c, &mut ct.ring);
            boundary_n += ct.is_boundary as i32;
            if !ct.is_regular {
                irregular_n += 1;
                irregular_corner = c;
                irregular_valence = ct.valence;
            }
            sharp_n += ct.is_sharp as i32;
            val2_n += ct.is_val2_int as i32;
        }
        for c in 0..4 {
            let c_next = (c + 1) & 3;
            let c_prev = (c + 3) & 3;
            let next_regular = g.corners[c_next].is_regular;
            let prev_regular = g.corners[c_prev].is_regular;
            let ct = &mut g.corners[c];
            ct.ep_on_boundary = false;
            ct.em_on_boundary = false;
            ct.fp_is_regular = ct.is_regular && next_regular;
            ct.fm_is_regular = ct.is_regular && prev_regular;
            ct.fp_is_copied = false;
            ct.fm_is_copied = false;
            if ct.is_boundary {
                ct.ep_on_boundary = ct.face_in_ring == 0;
                ct.em_on_boundary = ct.face_in_ring == ct.num_faces - 1;
                if ct.num_faces > 1 {
                    if ct.ep_on_boundary {
                        ct.fp_is_regular = ct.fm_is_regular;
                        ct.fp_is_copied = !ct.fp_is_regular;
                    }
                    if ct.em_on_boundary {
                        ct.fm_is_regular = ct.fp_is_regular;
                        ct.fm_is_copied = !ct.fm_is_regular;
                    }
                } else {
                    ct.fp_is_regular = true;
                    ct.fm_is_regular = true;
                }
            }
        }
        g.isolated_interior =
            irregular_n == 1 && boundary_n == 0 && irregular_valence > 2 && sharp_n == 0;
        if g.isolated_interior {
            g.isolated_corner = irregular_corner;
            g.isolated_valence = irregular_valence;
        }
        g.has_val2_interior = val2_n > 0;
        g
    }

    fn convert(&self) -> SparseMatrix {
        let mut m = if self.isolated_interior {
            self.resize_isolated_irregular()
        } else {
            self.resize_unisolated()
        };
        let max_ring = (1 + 2 * self.max_valence) as usize;
        let buf_size = (3 * max_ring).max(2 * self.num_source_points);
        let mut weights = vec![0.0f32; buf_size];
        let mut mask = vec![0i32; buf_size];
        for c in 0..4 {
            if self.corners[c].is_regular {
                self.assign_regular_edge_points(c, &mut m);
            } else {
                self.compute_irregular_edge_points(c, &mut m, &mut weights);
            }
        }
        for c in 0..4 {
            let ct = &self.corners[c];
            if ct.fp_is_regular || ct.fm_is_regular {
                self.assign_regular_face_points(c, &mut m);
            }
            if !ct.fp_is_regular || !ct.fm_is_regular {
                self.compute_irregular_face_points(c, &mut m, &mut weights, &mut mask);
            }
        }
        if self.has_val2_interior {
            remove_valence2_duplicates(&mut m);
        }
        m
    }

    fn resize_isolated_irregular(&self) -> SparseMatrix {
        let ring = (1 + 2 * self.isolated_valence) as usize;
        let c = self.isolated_corner;
        let plus = (c + 1) & 3;
        let opp = (c + 2) & 3;
        let minus = (c + 3) & 3;
        let mut sizes = [0usize; 20];
        sizes[c * 5..c * 5 + 5].copy_from_slice(&[ring; 5]);
        sizes[plus * 5..plus * 5 + 5].copy_from_slice(&[9, 6, 6, 4, 3 + ring]);
        sizes[opp * 5..opp * 5 + 5].copy_from_slice(&[9, 6, 6, 4, 4]);
        sizes[minus * 5..minus * 5 + 5].copy_from_slice(&[9, 6, 6, 3 + ring, 4]);
        SparseMatrix::with_row_sizes(self.num_source_points, &sizes)
    }

    fn resize_unisolated(&self) -> SparseMatrix {
        let mut sizes = [0usize; 20];
        for c in 0..4 {
            let ct = &self.corners[c];
            let rs = &mut sizes[c * 5..c * 5 + 5];
            if ct.is_regular {
                if !ct.is_boundary {
                    rs[0] = 9;
                    rs[1] = 6;
                    rs[2] = 6;
                } else {
                    rs[0] = 3;
                    rs[1] = if ct.ep_on_boundary { 2 } else { 6 };
                    rs[2] = if ct.em_on_boundary { 2 } else { 6 };
                }
            } else if ct.is_sharp {
                rs[0] = 1;
                rs[1] = 2;
                rs[2] = 2;
            } else if !ct.is_boundary {
                let ring = (1 + 2 * ct.valence) as usize;
                rs[0] = ring;
                rs[1] = ring;
                rs[2] = ring;
            } else if ct.num_faces > 1 {
                let ring = (1 + ct.valence + ct.num_faces) as usize;
                rs[0] = 3;
                rs[1] = if ct.ep_on_boundary { 2 } else { ring };
                rs[2] = if ct.em_on_boundary { 2 } else { ring };
            } else {
                rs[0] = 3;
                rs[1] = 2;
                rs[2] = 2;
            }
            rs[3] = 4;
            rs[4] = 4;
            if !ct.fp_is_regular || !ct.fm_is_regular {
                let c_next = (c + 1) & 3;
                let c_prev = (c + 3) & 3;
                if !ct.fp_is_regular {
                    rs[3] = self.irregular_face_point_size(
                        c,
                        if ct.fp_is_copied { c_prev } else { c_next },
                    );
                }
                if !ct.fm_is_regular {
                    rs[4] = self.irregular_face_point_size(
                        c,
                        if ct.fm_is_copied { c_next } else { c_prev },
                    );
                }
            }
        }
        SparseMatrix::with_row_sizes(self.num_source_points, &sizes)
    }

    fn assign_regular_edge_points(&self, c: usize, m: &mut SparseMatrix) {
        let (p, ep, em) = (5 * c, 5 * c + 1, 5 * c + 2);
        let ct = &self.corners[c];
        let r = &ct.ring;
        let ci = c as i32;
        let w = |x: f64| x as f32;
        if !ct.is_boundary {
            m.assign(p, 0, ci, w(4.0 / 9.0));
            m.assign(p, 1, r[0], w(1.0 / 9.0));
            m.assign(p, 2, r[2], w(1.0 / 9.0));
            m.assign(p, 3, r[4], w(1.0 / 9.0));
            m.assign(p, 4, r[6], w(1.0 / 9.0));
            m.assign(p, 5, r[1], w(1.0 / 36.0));
            m.assign(p, 6, r[3], w(1.0 / 36.0));
            m.assign(p, 7, r[5], w(1.0 / 36.0));
            m.assign(p, 8, r[7], w(1.0 / 36.0));
            let fr = ct.face_in_ring as usize;
            let i_ep = 2 * fr;
            let i_em = 2 * ((fr + 1) & 3);
            let i_op = 2 * ((fr + 2) & 3);
            let i_om = 2 * ((fr + 3) & 3);
            m.assign(ep, 0, ci, w(4.0 / 9.0));
            m.assign(ep, 1, r[i_ep], w(2.0 / 9.0));
            m.assign(ep, 2, r[i_em], w(1.0 / 9.0));
            m.assign(ep, 3, r[i_om], w(1.0 / 9.0));
            m.assign(ep, 4, r[i_ep + 1], w(1.0 / 18.0));
            m.assign(ep, 5, r[i_om + 1], w(1.0 / 18.0));
            m.assign(em, 0, ci, w(4.0 / 9.0));
            m.assign(em, 1, r[i_em], w(2.0 / 9.0));
            m.assign(em, 2, r[i_ep], w(1.0 / 9.0));
            m.assign(em, 3, r[i_op], w(1.0 / 9.0));
            m.assign(em, 4, r[i_ep + 1], w(1.0 / 18.0));
            m.assign(em, 5, r[i_em + 1], w(1.0 / 18.0));
        } else {
            let (eb, ei) = if ct.ep_on_boundary {
                (ep, em)
            } else {
                (em, ep)
            };
            let ib = if ct.ep_on_boundary { 0 } else { 4 };
            m.assign(p, 0, ci, w(2.0 / 3.0));
            m.assign(p, 1, r[0], w(1.0 / 6.0));
            m.assign(p, 2, r[4], w(1.0 / 6.0));
            m.assign(eb, 0, ci, w(2.0 / 3.0));
            m.assign(eb, 1, r[ib], w(1.0 / 3.0));
            m.assign(ei, 0, ci, w(4.0 / 9.0));
            m.assign(ei, 1, r[2], w(2.0 / 9.0));
            m.assign(ei, 2, r[0], w(1.0 / 9.0));
            m.assign(ei, 3, r[4], w(1.0 / 9.0));
            m.assign(ei, 4, r[1], w(1.0 / 18.0));
            m.assign(ei, 5, r[3], w(1.0 / 18.0));
        }
    }

    fn compute_irregular_edge_points(&self, c: usize, m: &mut SparseMatrix, buf: &mut [f32]) {
        let (p, ep, em) = (5 * c, 5 * c + 1, 5 * c + 2);
        let ct = &self.corners[c];
        let ci = c as i32;
        let next = ((c + 1) & 3) as i32;
        let prev = ((c + 3) & 3) as i32;
        if ct.is_sharp {
            m.assign(p, 0, ci, 1.0);
            m.assign(ep, 0, ci, (2.0f64 / 3.0) as f32);
            m.assign(ep, 1, next, (1.0f64 / 3.0) as f32);
            m.assign(em, 0, ci, (2.0f64 / 3.0) as f32);
            m.assign(em, 1, prev, (1.0f64 / 3.0) as f32);
        } else if !ct.is_boundary {
            let valence = ct.valence;
            let width = (1 + 2 * valence) as usize;
            let (pw, rest) = buf.split_at_mut(width);
            let (epw, rest) = rest.split_at_mut(width);
            let emw = &mut rest[..width];
            interior_point_weights(valence, ct.face_in_ring, pw, epw, emw);
            m.assign(p, 0, ci, pw[0]);
            m.assign(ep, 0, ci, epw[0]);
            m.assign(em, 0, ci, emw[0]);
            for i in 1..width {
                let rp = ct.ring[i - 1];
                m.assign(p, i, rp, pw[i]);
                m.assign(ep, i, rp, epw[i]);
                m.assign(em, i, rp, emw[i]);
            }
        } else if ct.num_faces > 1 {
            let valence = ct.valence;
            let width = (1 + ct.valence + ct.num_faces) as usize;
            let (pw, rest) = buf.split_at_mut(width);
            let (epw, rest) = rest.split_at_mut(width);
            let emw = &mut rest[..width];
            boundary_point_weights(valence, ct.face_in_ring, pw, epw, emw);
            let n = width - 1;
            let p1 = ct.ring[0];
            let pn = ct.ring[2 * (valence as usize - 1)];
            m.assign(p, 0, ci, pw[0]);
            m.assign(p, 1, p1, pw[1]);
            m.assign(p, 2, pn, pw[n]);
            m.assign(ep, 0, ci, epw[0]);
            if ct.ep_on_boundary {
                m.assign(ep, 1, p1, epw[1]);
            } else {
                for i in 1..width {
                    m.assign(ep, i, ct.ring[i - 1], epw[i]);
                }
            }
            m.assign(em, 0, ci, emw[0]);
            if ct.em_on_boundary {
                m.assign(em, 1, pn, emw[n]);
            } else {
                for i in 1..width {
                    m.assign(em, i, ct.ring[i - 1], emw[i]);
                }
            }
        } else {
            m.assign(p, 0, ci, (4.0f64 / 6.0) as f32);
            m.assign(p, 1, next, (1.0f64 / 6.0) as f32);
            m.assign(p, 2, prev, (1.0f64 / 6.0) as f32);
            m.assign(ep, 0, ci, (2.0f64 / 3.0) as f32);
            m.assign(ep, 1, next, (1.0f64 / 3.0) as f32);
            m.assign(em, 0, ci, (2.0f64 / 3.0) as f32);
            m.assign(em, 1, prev, (1.0f64 / 3.0) as f32);
        }
    }

    fn irregular_face_point_size(&self, near: usize, far: usize) -> usize {
        let c = &self.corners[near];
        let a = &self.corners[far];
        if c.is_sharp && a.is_sharp {
            return 2;
        }
        let this_size = if c.is_sharp {
            6
        } else {
            1 + c.ring.len() as i32
        };
        let adj_size = if a.is_regular || a.is_sharp {
            0
        } else {
            1 + a.ring.len() as i32 - 6
        };
        (this_size + adj_size).max(0) as usize
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_irregular_face_point(
        &self,
        near: usize,
        edge_in_near_ring: usize,
        far: usize,
        rows: [usize; 4],
        sign: f32,
        m: &mut SparseMatrix,
        row_weights: &mut [f32],
        column_mask: &mut [i32],
    ) {
        let [p, e_near, e_far, f_near] = rows;
        let cn = &self.corners[near];
        let cf = &self.corners[far];
        let valence = cn.valence as usize;
        let cos_near = cn.cos_face_angle;
        let cos_far = cf.cos_face_angle;
        let p_coeff = cos_far / 3.0f32;
        let e_near_coeff = ((-2.0f32).mul_add(cos_near, 3.0) - cos_far) / 3.0f32;
        let e_far_coeff = 2.0f32 * cos_near / 3.0f32;
        let full = self.num_source_points;
        for x in column_mask[..full].iter_mut() {
            *x = 0;
        }
        for x in row_weights[..full].iter_mut() {
            *x = 0.0;
        }
        for (row, s) in [(p, p_coeff), (e_near, e_near_coeff), (e_far, e_far_coeff)] {
            let cols = m.row_columns(row);
            let ws = m.row_elements(row);
            for i in 0..cols.len() {
                let idx = cols[i] as usize;
                row_weights[idx] = s.mul_add(ws[i], row_weights[idx]);
                column_mask[idx] = 1 + idx as i32;
            }
        }
        let i_int = edge_in_near_ring;
        let i_prev = (i_int + valence - 1) % valence;
        let i_next = (i_int + 1) % valence;
        let r = &cn.ring;
        row_weights[r[2 * i_prev] as usize] += -sign / 9.0f32;
        row_weights[r[2 * i_prev + 1] as usize] += -sign / 18.0f32;
        row_weights[r[2 * i_int + 1] as usize] += sign / 18.0f32;
        row_weights[r[2 * i_next] as usize] += sign / 9.0f32;
        let size = m.row_size(f_near);
        let mut n = 0;
        for i in 0..full {
            if column_mask[i] != 0 && n < size {
                m.assign(f_near, n, column_mask[i] - 1, row_weights[i]);
                n += 1;
            }
        }
        if self.has_val2_interior {
            while n < size {
                m.assign(f_near, n, near as i32, 0.0);
                n += 1;
            }
        }
    }

    fn assign_regular_face_points(&self, c: usize, m: &mut SparseMatrix) {
        let (fp, fm) = (5 * c + 3, 5 * c + 4);
        let ct = &self.corners[c];
        let next = ((c + 1) & 3) as i32;
        let opp = ((c + 2) & 3) as i32;
        let prev = ((c + 3) & 3) as i32;
        let ci = c as i32;
        for (row, on) in [(fp, ct.fp_is_regular), (fm, ct.fm_is_regular)] {
            if on {
                m.assign(row, 0, ci, (4.0f64 / 9.0) as f32);
                m.assign(row, 1, prev, (2.0f64 / 9.0) as f32);
                m.assign(row, 2, next, (2.0f64 / 9.0) as f32);
                m.assign(row, 3, opp, (1.0f64 / 9.0) as f32);
            }
        }
    }

    fn compute_irregular_face_points(
        &self,
        c: usize,
        m: &mut SparseMatrix,
        weights: &mut [f32],
        mask: &mut [i32],
    ) {
        let ct = &self.corners[c];
        let next = (c + 1) & 3;
        let prev = (c + 3) & 3;
        let ep_prev = 5 * prev + 1;
        let em = 5 * c + 2;
        let p = 5 * c;
        let ep = 5 * c + 1;
        let em_next = 5 * next + 2;
        let fp = 5 * c + 3;
        let fm = 5 * c + 4;
        if !ct.fp_is_regular && !ct.fp_is_copied {
            let ie = ct.face_in_ring as usize;
            self.compute_irregular_face_point(
                c,
                ie,
                next,
                [p, ep, em_next, fp],
                1.0,
                m,
                weights,
                mask,
            );
        }
        if !ct.fm_is_regular && !ct.fm_is_copied {
            let ie = ((ct.face_in_ring + 1) % ct.valence) as usize;
            self.compute_irregular_face_point(
                c,
                ie,
                prev,
                [p, em, ep_prev, fm],
                -1.0,
                m,
                weights,
                mask,
            );
        }
        if ct.fp_is_copied {
            m.copy_row(fp, fm);
        }
        if ct.fm_is_copied {
            m.copy_row(fm, fp);
        }
    }
}

/// `_removeValence2Duplicates`
fn remove_valence2_duplicates(m: &mut SparseMatrix) {
    let mut sizes = Vec::with_capacity(m.num_rows);
    let mut cols: Vec<Vec<i32>> = Vec::with_capacity(m.num_rows);
    let mut ws: Vec<Vec<f32>> = Vec::with_capacity(m.num_rows);
    for row in 0..m.num_rows {
        let sc = m.row_columns(row);
        let sw = m.row_elements(row);
        let mut out_c: Vec<i32> = Vec::with_capacity(sc.len());
        let mut out_w: Vec<f32> = Vec::with_capacity(sc.len());
        let mut dst_of_corner: [Option<usize>; 4] = [None; 4];
        for i in 0..sc.len() {
            let idx = sc[i];
            let w = sw[i];
            if idx < 4 {
                if let Some(d) = dst_of_corner[idx as usize] {
                    out_w[d] += w;
                    continue;
                }
                dst_of_corner[idx as usize] = Some(out_c.len());
            }
            out_c.push(idx);
            out_w.push(w);
        }
        sizes.push(out_c.len());
        cols.push(out_c);
        ws.push(out_w);
    }
    let mut t = SparseMatrix::with_row_sizes(m.num_columns, &sizes);
    for row in 0..t.num_rows {
        for k in 0..sizes[row] {
            t.assign(row, k, cols[row][k], ws[row][k]);
        }
    }
    *m = t;
}
