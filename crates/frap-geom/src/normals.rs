// SPDX-License-Identifier: GPL-3.0-or-later
//! Face, vertex and corner normals: a port of
//! `blender/source/blender/blenkernel/intern/mesh_normals.cc` (Blender 5.2) plus the BLI vector
//! helpers it relies on. Everything works on plain slices (positions, face offsets, corner
//! vertices/edges) so `frap-mesh`, `frap-bmesh` and the exporters share one implementation.
//!
//! Float rules (ARCHITECTURE §21.3): every operation is written in Blender's order, without
//! `mul_add`, so results match Blender bit for bit wherever Blender's build does not contract
//! to FMA. `sqrt` is IEEE-exact; the only transcendental (`acos`) is Blender's own polynomial
//! approximation ([`safe_acos_approx`]), so no `libm` call is needed.

/// 3D vector as Blender stores it (`float[3]`).
pub type V3 = [f32; 3];

/// `sub_v3_v3v3`: `a - b`.
#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `add_v3_v3v3`: `a + b`.
#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `mul_v3_v3fl`: `a * f`.
#[inline]
pub fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `dot_v3v3`: `a[0]*b[0] + a[1]*b[1] + a[2]*b[2]` (left to right).
#[inline]
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `cross_v3_v3v3` / `math::cross`.
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `len_squared_v3v3(a, b)`: squared distance (`b - a`, then dot).
#[inline]
pub fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    let d = sub(b, a);
    dot(d, d)
}

/// `len_v3v3(a, b)`.
#[inline]
pub fn len_v3v3(a: V3, b: V3) -> f32 {
    len_squared_v3v3(a, b).sqrt()
}

// Ported from blender/source/blender/blenlib/intern/math_vector_inline.cc
// (normalize_v3_v3_length): multiplies by the reciprocal of the length.
/// C-style `normalize_v3`: returns the normalised vector (zero when the squared length is
/// `<= 1e-35`) and the original length (0 in that case).
#[inline]
pub fn normalize_v3(a: V3) -> (V3, f32) {
    normalize_v3_length(a, 1.0)
}

/// C-style `normalize_v3_v3_length(r, a, unit_length)`.
#[inline]
pub fn normalize_v3_length(a: V3, unit_length: f32) -> (V3, f32) {
    let d = dot(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        (scale(a, unit_length / d), d)
    } else {
        ([0.0; 3], 0.0)
    }
}

// Ported from blender/source/blender/blenlib/BLI_math_vector.hh (normalize_and_get_length):
// divides each component by the length.
/// C++-style `math::normalize`: zero vector when the squared length is `<= 1e-35`.
#[inline]
pub fn normalize(v: V3) -> V3 {
    let len_sq = dot(v, v);
    if len_sq > 1.0e-35 {
        let len = len_sq.sqrt();
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0; 3]
    }
}

// Ported from blender/source/blender/blenlib/BLI_math_base.hh (safe_acos_approx).
/// Blender's polynomial `acos` approximation with clamping (max error 4.5e-5 rad), used for
/// every angle weight of the normal calculation.
#[inline]
pub fn safe_acos_approx(x: f32) -> f32 {
    let f = x.abs();
    // Clamp and crush denormals. Blender's literal `1.5707963267f` is exactly FRAC_PI_2 in f32.
    let m = if f < 1.0 { 1.0 - (1.0 - f) } else { 1.0 };
    let a = (1.0 - m).sqrt()
        * (std::f32::consts::FRAC_PI_2
            + m * (-0.213_300_99 + m * (0.077_980_48 + m * -0.021_640_95)));
    if x < 0.0 { std::f32::consts::PI - a } else { a }
}

// Ported from blender/source/blender/blenlib/intern/math_vector_inline.cc
// (add_newell_cross_v3_v3v3).
#[inline]
fn add_newell_cross(n: &mut V3, v_prev: V3, v_curr: V3) {
    n[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
    n[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
    n[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
}

/// Unnormalised Newell normal of a polygon given by vertex indices.
pub fn newell_cross(positions: &[V3], face_verts: &[i32]) -> V3 {
    let mut n = [0.0f32; 3];
    let Some(&last) = face_verts.last() else {
        return n;
    };
    let mut v_prev = positions[last as usize];
    for &v in face_verts {
        let v_curr = positions[v as usize];
        add_newell_cross(&mut n, v_prev, v_curr);
        v_prev = v_curr;
    }
    n
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (normal_calc_ngon).
/// Face normal by Newell's method for any polygon size; `(0, 0, 1)` for degenerate faces.
/// This is what `Mesh.polygon_normals` uses for every face size in Blender 5.2.
pub fn face_normal_newell(positions: &[V3], face_verts: &[i32]) -> V3 {
    let (mut n, len) = normalize_v3(newell_cross(positions, face_verts));
    if len == 0.0 {
        n[2] = 1.0;
    }
    n
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (face_normal_calc), with
// math_geom.hh normal_tri/normal_quad.
/// Face normal with the triangle/quad special cases of `face_normal_calc` (used by BMesh code);
/// `(0, 0, 1)` when degenerate.
pub fn face_normal_calc(positions: &[V3], face_verts: &[i32]) -> V3 {
    let p = |i: usize| positions[face_verts[i] as usize];
    let mut n = match face_verts.len() {
        4 => normalize(cross(sub(p(0), p(2)), sub(p(1), p(3)))),
        3 => normalize(cross(sub(p(0), p(1)), sub(p(1), p(2)))),
        _ => face_normal_newell(positions, face_verts),
    };
    if n == [0.0; 3] {
        n[2] = 1.0;
    }
    n
}

/// `normals_calc_faces`: Newell normal per face.
pub fn normals_calc_faces(positions: &[V3], face_offsets: &[i32], corner_verts: &[i32]) -> Vec<V3> {
    face_offsets
        .windows(2)
        .map(|w| {
            frap_math::budget::check();
            face_normal_newell(positions, &corner_verts[w[0] as usize..w[1] as usize])
        })
        .collect()
}

/// Vertex → faces map (`build_vert_to_face_map`): faces in ascending order per vertex; a face
/// that uses a vertex twice is listed twice.
#[derive(Clone, Debug, Default)]
pub struct VertToFaceMap {
    /// `len = verts + 1`.
    pub offsets: Vec<u32>,
    /// Face indices grouped by vertex.
    pub faces: Vec<u32>,
}

impl VertToFaceMap {
    /// Builds the map.
    pub fn new(verts_num: usize, face_offsets: &[i32], corner_verts: &[i32]) -> VertToFaceMap {
        let mut counts = vec![0u32; verts_num + 1];
        for &v in corner_verts {
            counts[v as usize] += 1;
        }
        let mut offsets = Vec::with_capacity(verts_num + 1);
        let mut acc = 0u32;
        for &c in &counts[..verts_num] {
            offsets.push(acc);
            acc += c;
        }
        offsets.push(acc);
        let mut fill = offsets.clone();
        let mut faces = vec![0u32; acc as usize];
        for (f, w) in face_offsets.windows(2).enumerate() {
            for &v in &corner_verts[w[0] as usize..w[1] as usize] {
                let slot = &mut fill[v as usize];
                faces[*slot as usize] = f as u32;
                *slot += 1;
            }
        }
        VertToFaceMap { offsets, faces }
    }
    /// Faces of vertex `v`.
    pub fn get(&self, v: usize) -> &[u32] {
        &self.faces[self.offsets[v] as usize..self.offsets[v + 1] as usize]
    }
}

#[inline]
fn face_range(face_offsets: &[i32], f: usize) -> (usize, usize) {
    (face_offsets[f] as usize, face_offsets[f + 1] as usize)
}

/// `face_find_corner_from_vert`: first corner of the face that uses `vert`.
#[inline]
fn corner_of_vert(face_offsets: &[i32], corner_verts: &[i32], f: usize, vert: usize) -> usize {
    let (s, e) = face_range(face_offsets, f);
    (s..e)
        .find(|&c| corner_verts[c] as usize == vert)
        .expect("vertex-to-face map is consistent")
}

#[inline]
fn corner_prev(s: usize, e: usize, c: usize) -> usize {
    if c == s { e - 1 } else { c - 1 }
}
#[inline]
fn corner_next(s: usize, e: usize, c: usize) -> usize {
    if c + 1 == e { s } else { c + 1 }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (normals_calc_verts).
/// Angle-weighted vertex normals; a vertex without faces gets its normalised position.
pub fn normals_calc_verts(
    positions: &[V3],
    face_offsets: &[i32],
    corner_verts: &[i32],
    vert_to_face: &VertToFaceMap,
    face_normals: &[V3],
) -> Vec<V3> {
    let mut out = Vec::with_capacity(positions.len());
    for (vert, &co) in positions.iter().enumerate() {
        let vert_faces = vert_to_face.get(vert);
        if vert_faces.is_empty() {
            out.push(normalize(co));
            continue;
        }
        let mut n = [0.0f32; 3];
        for &face in vert_faces {
            let face = face as usize;
            let (s, e) = face_range(face_offsets, face);
            let c = corner_of_vert(face_offsets, corner_verts, face, vert);
            let prev = positions[corner_verts[corner_prev(s, e, c)] as usize];
            let next = positions[corner_verts[corner_next(s, e, c)] as usize];
            let dir_prev = normalize(sub(prev, co));
            let dir_next = normalize(sub(next, co));
            let factor = safe_acos_approx(dot(dir_prev, dir_next));
            n = add(n, scale(face_normals[face], factor));
        }
        out.push(normalize(n));
    }
    out
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (mix_normals_corner_to_vert).
/// Vertex normals mixed from per-corner normals (custom normals on the corner domain).
pub fn mix_normals_corner_to_vert(
    positions: &[V3],
    face_offsets: &[i32],
    corner_verts: &[i32],
    vert_to_face: &VertToFaceMap,
    corner_normals: &[V3],
) -> Vec<V3> {
    let mut out = Vec::with_capacity(positions.len());
    for (vert, &co) in positions.iter().enumerate() {
        let vert_faces = vert_to_face.get(vert);
        if vert_faces.is_empty() {
            out.push(normalize(co));
            continue;
        }
        let mut n = [0.0f32; 3];
        for &face in vert_faces {
            let face = face as usize;
            let (s, e) = face_range(face_offsets, face);
            let c = corner_of_vert(face_offsets, corner_verts, face, vert);
            let prev = positions[corner_verts[corner_prev(s, e, c)] as usize];
            let next = positions[corner_verts[corner_next(s, e, c)] as usize];
            let dir_prev = normalize(sub(prev, co));
            let dir_next = normalize(sub(next, co));
            let factor = safe_acos_approx(dot(dir_prev, dir_next));
            n = add(n, scale(corner_normals[c], factor));
        }
        out.push(normalize(n));
    }
    out
}

/// `mix_normals_vert_to_face`: face normal = normalised sum of its vertex normals.
pub fn mix_normals_vert_to_face(
    face_offsets: &[i32],
    corner_verts: &[i32],
    vert_normals: &[V3],
) -> Vec<V3> {
    face_offsets
        .windows(2)
        .map(|w| {
            let mut sum = [0.0f32; 3];
            for &v in &corner_verts[w[0] as usize..w[1] as usize] {
                sum = add(sum, vert_normals[v as usize]);
            }
            normalize(sum)
        })
        .collect()
}

/// `mix_normals_corner_to_face`: face normal = normalised sum of its corner normals.
pub fn mix_normals_corner_to_face(face_offsets: &[i32], corner_normals: &[V3]) -> Vec<V3> {
    face_offsets
        .windows(2)
        .map(|w| {
            let mut sum = [0.0f32; 3];
            for &n in &corner_normals[w[0] as usize..w[1] as usize] {
                sum = add(sum, n);
            }
            normalize(sum)
        })
        .collect()
}

// ------------------------------------------------------------------------- corner normals

/// `LNOR_SPACE_TRIGO_THRESHOLD`.
const LNOR_SPACE_TRIGO_THRESHOLD: f32 = 1.0 - 1e-4;

/// `CornerNormalSpace`: the per-fan space custom normals are encoded in.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CornerNormalSpace {
    /// Automatic (fan) normal.
    pub vec_lnor: V3,
    /// Reference edge direction projected on the normal's plane.
    pub vec_ref: V3,
    /// `vec_lnor × vec_ref`.
    pub vec_ortho: V3,
    /// Average angle of the fan edges to the normal (0 = invalid space).
    pub ref_alpha: f32,
    /// Angle between the first and last fan edge around the normal (0 = invalid space).
    pub ref_beta: f32,
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (corner_fan_space_define).
fn corner_fan_space_define(
    lnor: V3,
    vec_ref: V3,
    vec_other: V3,
    edge_vectors: &[V3],
) -> CornerNormalSpace {
    let mut space = CornerNormalSpace::default();
    let pi2 = std::f32::consts::PI * 2.0;
    let dtp_ref = dot(vec_ref, lnor);
    let dtp_other = dot(vec_other, lnor);
    if dtp_ref.abs() >= LNOR_SPACE_TRIGO_THRESHOLD || dtp_other.abs() >= LNOR_SPACE_TRIGO_THRESHOLD
    {
        return space;
    }
    space.vec_lnor = lnor;
    if !edge_vectors.is_empty() {
        let mut alpha = 0.0f32;
        for &v in edge_vectors {
            alpha += safe_acos_approx(dot(v, lnor));
        }
        space.ref_alpha = alpha / edge_vectors.len() as f32;
    } else {
        space.ref_alpha =
            (safe_acos_approx(dot(vec_ref, lnor)) + safe_acos_approx(dot(vec_other, lnor))) / 2.0;
    }
    space.vec_ref = normalize(sub(vec_ref, scale(lnor, dtp_ref)));
    space.vec_ortho = normalize(cross(lnor, space.vec_ref));
    let vec_other_proj = normalize(sub(vec_other, scale(lnor, dtp_other)));
    let dtp = dot(space.vec_ref, vec_other_proj);
    if dtp < LNOR_SPACE_TRIGO_THRESHOLD {
        let beta = safe_acos_approx(dtp);
        space.ref_beta = if dot(space.vec_ortho, vec_other_proj) < 0.0 {
            pi2 - beta
        } else {
            beta
        };
    } else {
        space.ref_beta = pi2;
    }
    space
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (corner_space_custom_data_to_normal, unit_short_to_float).
/// Decodes an `INT16_2D` custom normal in its fan space.
pub fn corner_space_custom_data_to_normal(space: &CornerNormalSpace, clnor: [i16; 2]) -> V3 {
    if clnor[0] == 0 || space.ref_alpha == 0.0 || space.ref_beta == 0.0 {
        return space.vec_lnor;
    }
    let pi2 = (std::f64::consts::PI * 2.0) as f32;
    let alphafac = clnor[0] as f32 / i16::MAX as f32;
    let alpha = (if alphafac > 0.0 {
        space.ref_alpha
    } else {
        pi2 - space.ref_alpha
    }) * alphafac;
    let betafac = clnor[1] as f32 / i16::MAX as f32;
    let mut r = scale(space.vec_lnor, libm::cosf(alpha));
    if betafac == 0.0 {
        r = add(r, scale(space.vec_ref, libm::sinf(alpha)));
    } else {
        let sinalpha = libm::sinf(alpha);
        let beta = (if betafac > 0.0 {
            space.ref_beta
        } else {
            pi2 - space.ref_beta
        }) * betafac;
        r = add(r, scale(space.vec_ref, sinalpha * libm::cosf(beta)));
        r = add(r, scale(space.vec_ortho, sinalpha * libm::sinf(beta)));
    }
    r
}

#[derive(Copy, Clone, Debug)]
struct VertCornerInfo {
    face: usize,
    corner: usize,
    corner_prev: usize,
    corner_next: usize,
    vert_prev: usize,
    vert_next: usize,
    local_edge_prev: usize,
    local_edge_next: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum EdgeInfo {
    Uninitialized,
    OneCorner {
        local_corner_1: usize,
        winding_towards_vert: bool,
    },
    TwoCorners {
        local_corner_1: usize,
        local_corner_2: usize,
    },
    Sharp,
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (add_corner_to_edge).
fn add_corner_to_edge(
    corner_edges: &[i32],
    sharp_edges: Option<&[bool]>,
    local_corner: usize,
    corner: usize,
    other_corner: usize,
    winding_towards_vert: bool,
    info: &mut EdgeInfo,
) {
    match *info {
        EdgeInfo::Uninitialized => {
            if let Some(sharp) = sharp_edges {
                let c = if winding_towards_vert {
                    other_corner
                } else {
                    corner
                };
                if sharp[corner_edges[c] as usize] {
                    *info = EdgeInfo::Sharp;
                    return;
                }
            }
            *info = EdgeInfo::OneCorner {
                local_corner_1: local_corner,
                winding_towards_vert,
            };
        }
        EdgeInfo::OneCorner {
            local_corner_1,
            winding_towards_vert: w,
        } => {
            if w == winding_towards_vert {
                *info = EdgeInfo::Sharp;
                return;
            }
            *info = EdgeInfo::TwoCorners {
                local_corner_1,
                local_corner_2: local_corner,
            };
        }
        _ => *info = EdgeInfo::Sharp,
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (traverse_fan_local_corners).
fn traverse_fan(infos: &[VertCornerInfo], edges: &[EdgeInfo], start: usize, fan: &mut Vec<usize>) {
    fan.push(start);
    let mut current = start;
    let mut local_edge = infos[current].local_edge_next;
    let mut cyclic = false;
    while let EdgeInfo::TwoCorners {
        local_corner_1,
        local_corner_2,
    } = edges[local_edge]
    {
        current = if current == local_corner_1 {
            local_corner_2
        } else {
            local_corner_1
        };
        if current == start {
            cyclic = true;
            break;
        }
        fan.push(current);
        local_edge = infos[current].local_edge_next;
    }
    fan.reverse();
    if cyclic {
        // Rotate so that the smallest face corner index comes first (std::min_element: first
        // minimum).
        let mut best = 0;
        for i in 1..fan.len() {
            if infos[fan[i]].corner < infos[fan[best]].corner {
                best = i;
            }
        }
        fan.rotate_left(best);
        return;
    }
    let mut current = start;
    let mut local_edge = infos[current].local_edge_prev;
    while let EdgeInfo::TwoCorners {
        local_corner_1,
        local_corner_2,
    } = edges[local_edge]
    {
        current = if current == local_corner_1 {
            local_corner_2
        } else {
            local_corner_1
        };
        fan.push(current);
        local_edge = infos[current].local_edge_prev;
    }
}

/// Inputs of [`normals_calc_corners`].
#[derive(Copy, Clone, Debug)]
pub struct CornerNormalsInput<'a> {
    /// Vertex positions.
    pub positions: &'a [V3],
    /// Face offsets (`len = faces + 1`).
    pub face_offsets: &'a [i32],
    /// Corner → vertex.
    pub corner_verts: &'a [i32],
    /// Corner → edge.
    pub corner_edges: &'a [i32],
    /// Vertex → faces.
    pub vert_to_face: &'a VertToFaceMap,
    /// "True" face normals ([`normals_calc_faces`]).
    pub face_normals: &'a [V3],
    /// `sharp_edge`, when present.
    pub sharp_edges: Option<&'a [bool]>,
    /// `sharp_face`, when present.
    pub sharp_faces: Option<&'a [bool]>,
    /// `custom_normal` (INT16_2D, CORNER), when present.
    pub custom_normals: Option<&'a [[i16; 2]]>,
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (normals_calc_corners,
// collect_corner_info, calc_local_edge_indices, calc_connecting_edge_info, accumulate_fan_normal,
// handle_fan_result_and_custom_normals).
/// Corner normals: smooth fans around each vertex, split at sharp edges, sharp-face boundaries,
/// non-manifold edges and winding flips; angle-weighted face normals per fan; custom normals
/// decoded in each fan's space.
pub fn normals_calc_corners(inp: &CornerNormalsInput<'_>) -> Vec<V3> {
    let CornerNormalsInput {
        positions,
        face_offsets,
        corner_verts,
        corner_edges,
        vert_to_face,
        face_normals,
        sharp_edges,
        sharp_faces,
        custom_normals,
    } = *inp;
    let mut out = vec![[0.0f32; 3]; corner_verts.len()];
    if corner_verts.is_empty() {
        return out;
    }
    let mut infos: Vec<VertCornerInfo> = Vec::new();
    let mut local_edge_verts: Vec<usize> = Vec::new();
    let mut edge_infos: Vec<EdgeInfo> = Vec::new();
    let mut edge_dirs: Vec<V3> = Vec::new();
    let mut visited: Vec<bool> = Vec::new();
    let mut fan: Vec<usize> = Vec::new();
    for (vert, &vert_co) in positions.iter().enumerate() {
        frap_math::budget::check();
        let vert_faces = vert_to_face.get(vert);
        if vert_faces.is_empty() {
            continue;
        }
        infos.clear();
        for &face in vert_faces {
            let face = face as usize;
            let (s, e) = face_range(face_offsets, face);
            let corner = corner_of_vert(face_offsets, corner_verts, face, vert);
            let cp = corner_prev(s, e, corner);
            let cn = corner_next(s, e, corner);
            infos.push(VertCornerInfo {
                face,
                corner,
                corner_prev: cp,
                corner_next: cn,
                vert_prev: corner_verts[cp] as usize,
                vert_next: corner_verts[cn] as usize,
                local_edge_prev: 0,
                local_edge_next: 0,
            });
        }
        // calc_local_edge_indices: VectorSet index_of_or_add (insertion order).
        local_edge_verts.clear();
        let index_of_or_add = |list: &mut Vec<usize>, v: usize| -> usize {
            if let Some(i) = list.iter().position(|&x| x == v) {
                i
            } else {
                list.push(v);
                list.len() - 1
            }
        };
        for info in infos.iter_mut() {
            info.local_edge_prev = index_of_or_add(&mut local_edge_verts, info.vert_prev);
            info.local_edge_next = index_of_or_add(&mut local_edge_verts, info.vert_next);
        }
        edge_infos.clear();
        edge_infos.resize(local_edge_verts.len(), EdgeInfo::Uninitialized);
        for (local_corner, info) in infos.iter().enumerate() {
            if sharp_faces.is_some_and(|s| s[info.face]) {
                edge_infos[info.local_edge_prev] = EdgeInfo::Sharp;
                edge_infos[info.local_edge_next] = EdgeInfo::Sharp;
                continue;
            }
            add_corner_to_edge(
                corner_edges,
                sharp_edges,
                local_corner,
                info.corner,
                info.corner_prev,
                true,
                &mut edge_infos[info.local_edge_prev],
            );
            add_corner_to_edge(
                corner_edges,
                sharp_edges,
                local_corner,
                info.corner,
                info.corner_next,
                false,
                &mut edge_infos[info.local_edge_next],
            );
        }
        edge_dirs.clear();
        edge_dirs.extend(
            local_edge_verts
                .iter()
                .map(|&v| normalize(sub(positions[v], vert_co))),
        );

        let mut visited_count = 0usize;
        visited.clear();
        visited.resize(infos.len(), false);
        let mut start = 0usize;
        loop {
            fan.clear();
            traverse_fan(&infos, &edge_infos, start, &mut fan);
            let mut fan_normal = if fan.len() == 1 {
                face_normals[infos[fan[0]].face]
            } else {
                let mut n = [0.0f32; 3];
                for &lc in &fan {
                    let info = &infos[lc];
                    let factor = safe_acos_approx(dot(
                        edge_dirs[info.local_edge_prev],
                        edge_dirs[info.local_edge_next],
                    ));
                    n = add(n, scale(face_normals[info.face], factor));
                }
                normalize(n)
            };
            if let Some(custom) = custom_normals {
                let local_edge_first = infos[fan[0]].local_edge_next;
                let local_edge_last = infos[*fan.last().expect("fan is non-empty")].local_edge_prev;
                let mut fan_edge_dirs: Vec<V3> = Vec::new();
                if fan.len() > 1 {
                    for &lc in &fan {
                        fan_edge_dirs.push(edge_dirs[infos[lc].local_edge_next]);
                    }
                    if local_edge_last != local_edge_first {
                        fan_edge_dirs.push(edge_dirs[local_edge_last]);
                    }
                }
                let space = corner_fan_space_define(
                    fan_normal,
                    edge_dirs[local_edge_first],
                    edge_dirs[local_edge_last],
                    &fan_edge_dirs,
                );
                let mut avg = [0i32; 2];
                for &lc in &fan {
                    let c = custom[infos[lc].corner];
                    avg[0] += c[0] as i32;
                    avg[1] += c[1] as i32;
                }
                let n = fan.len() as i32;
                let avg = [(avg[0] / n) as i16, (avg[1] / n) as i16];
                fan_normal = corner_space_custom_data_to_normal(&space, avg);
            }
            for &lc in &fan {
                out[infos[lc].corner] = fan_normal;
            }
            visited_count += fan.len();
            if visited_count == infos.len() {
                break;
            }
            for &lc in &fan {
                visited[lc] = true;
            }
            while visited[start] {
                start += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acos_approx_endpoints() {
        assert!((safe_acos_approx(1.0)).abs() < 1e-6);
        assert!((safe_acos_approx(-1.0) - std::f32::consts::PI).abs() < 1e-6);
        assert!((safe_acos_approx(0.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!((safe_acos_approx(2.0)).abs() < 1e-6);
    }

    #[test]
    fn newell_degenerate_is_z() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert_eq!(face_normal_newell(&p, &[0, 1, 2]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn flat_quad_corner_normals() {
        // Two coplanar quads sharing an edge: one smooth fan per shared vertex.
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        let offs = [0, 4, 8];
        let cv = [0, 1, 2, 3, 1, 4, 5, 2];
        // edges: (0,1)=0 (1,2)=1 (2,3)=2 (3,0)=3 (1,4)=4 (4,5)=5 (5,2)=6
        let ce = [0, 1, 2, 3, 4, 5, 6, 1];
        let fnor = normals_calc_faces(&p, &offs, &cv);
        let map = VertToFaceMap::new(p.len(), &offs, &cv);
        let cn = normals_calc_corners(&CornerNormalsInput {
            positions: &p,
            face_offsets: &offs,
            corner_verts: &cv,
            corner_edges: &ce,
            vert_to_face: &map,
            face_normals: &fnor,
            sharp_edges: None,
            sharp_faces: None,
            custom_normals: None,
        });
        for n in cn {
            assert_eq!(n, [0.0, 0.0, 1.0]);
        }
    }
}
