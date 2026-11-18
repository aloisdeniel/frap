// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh-building helpers shared by the constructive modifiers of WP 4.2 (MIRROR, ARRAY,
//! SOLIDIFY, WELD): rebuilding a mesh with new topology while every other attribute is copied
//! (`LegacyMeshInterpolator::copy`, `attribute_math::gather`) or mixed by groups
//! (`attribute_math::mix_groups`), and the small vector helpers Blender's `BLI_math` provides.
//!
//! Lives under `types/weld/` (a submodule of a file WP 4.2 owns, ARCHITECTURE §22.1); the other
//! type files reach it as `super::weld::util`.

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use smallvec::SmallVec;

/// The topology attributes a rebuild writes itself.
pub(crate) const POSITION: &str = "position";
/// See [`POSITION`].
pub(crate) const EDGE_VERTS: &str = ".edge_verts";
/// See [`POSITION`].
pub(crate) const CORNER_VERT: &str = ".corner_vert";
/// See [`POSITION`].
pub(crate) const CORNER_EDGE: &str = ".corner_edge";

/// True for the four attributes that hold the topology (never copied generically).
pub(crate) fn is_topology(name: &str) -> bool {
    matches!(name, POSITION | EDGE_VERTS | CORNER_VERT | CORNER_EDGE)
}

/// New topology of a rebuilt mesh.
#[derive(Clone, Debug, Default)]
pub(crate) struct Topology {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// `.edge_verts`
    pub edges: Vec<[i32; 2]>,
    /// Face offsets (`faces + 1` entries, starting at 0).
    pub face_offsets: Vec<i32>,
    /// `.corner_vert`
    pub corner_verts: Vec<i32>,
    /// `.corner_edge`
    pub corner_edges: Vec<i32>,
}

impl Topology {
    /// Number of faces.
    pub fn faces_num(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }
}

/// How the attributes of one domain are carried to the rebuilt mesh.
#[derive(Clone, Debug)]
pub(crate) enum Carry {
    /// `dst[i] = src[map[i]]`, defaults where `map[i]` is `None`.
    Gather(Vec<Option<usize>>),
    /// `dst[i] = mix(src[groups[i]])` (`attribute_math::mix_groups`); an empty group gives the
    /// default value.
    Mix(Vec<SmallVec<[usize; 2]>>),
}

impl Carry {
    /// A plain gather without defaults.
    pub fn gather(map: impl IntoIterator<Item = usize>) -> Carry {
        Carry::Gather(map.into_iter().map(Some).collect())
    }
    fn len(&self) -> usize {
        match self {
            Carry::Gather(m) => m.len(),
            Carry::Mix(g) => g.len(),
        }
    }
}

/// Per-domain carry rules of a rebuild.
#[derive(Clone, Debug)]
pub(crate) struct DomainMaps {
    /// Vertices (also `deform_verts`).
    pub vert: Carry,
    /// Edges.
    pub edge: Carry,
    /// Faces.
    pub face: Carry,
    /// Corners.
    pub corner: Carry,
}

/// Rebuilds `src` with `topo`: every non-topology attribute is carried by its domain's rule
/// (string attributes are dropped by a mix, as `mix_attributes` skips them), vertex-group
/// weights follow the vertex rule, and the mesh parameters (materials, UV map names, …) are
/// kept (`BKE_mesh_copy_parameters_for_eval`).
pub(crate) fn rebuild(src: &Mesh, topo: Topology, maps: &DomainMaps) -> Mesh {
    debug_assert_eq!(maps.vert.len(), topo.positions.len());
    debug_assert_eq!(maps.edge.len(), topo.edges.len());
    debug_assert_eq!(maps.face.len(), topo.faces_num());
    debug_assert_eq!(maps.corner.len(), topo.corner_verts.len());
    let mut out = src.clone();
    let Topology {
        positions,
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
    } = topo;
    out.set_topology(face_offsets, corner_verts, edges);
    out.attributes_mut().insert(Attribute {
        name: CORNER_EDGE.into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_edges)),
    });
    out.set_positions(positions);
    let names: Vec<(smol_str::SmolStr, Domain)> = src
        .attributes()
        .iter()
        .filter(|a| !is_topology(&a.name))
        .map(|a| (a.name.clone(), a.domain))
        .collect();
    for (name, domain) in names {
        let Some(a) = src.attributes().get(&name) else {
            continue;
        };
        let carry = match domain {
            Domain::Point => &maps.vert,
            Domain::Edge => &maps.edge,
            Domain::Face => &maps.face,
            Domain::Corner => &maps.corner,
        };
        match carry_data(&a.data, carry) {
            Some(data) => out.attributes_mut().insert(Attribute {
                name: name.clone(),
                domain,
                data,
            }),
            None => {
                out.attributes_mut().remove(&name);
            }
        }
    }
    if let Some(dv) = src.deform_verts() {
        let new = match &maps.vert {
            Carry::Gather(m) => m
                .iter()
                .map(|i| i.map(|i| dv[i].clone()).unwrap_or_default())
                .collect(),
            Carry::Mix(g) => g.iter().map(|grp| mix_deform_verts(dv, grp)).collect(),
        };
        *out.deform_verts_mut() = new;
    }
    out
}

/// Carries one column (`None`: the attribute is dropped, a STRING column under a mix).
pub(crate) fn carry_data(data: &AttrData, carry: &Carry) -> Option<AttrData> {
    match carry {
        Carry::Gather(map) => Some(gather_opt(data, map)),
        Carry::Mix(groups) => mix_groups(data, groups),
    }
}

/// `dst[i] = src[map[i]]`, the type's default where `map[i]` is `None`.
pub(crate) fn gather_opt(data: &AttrData, map: &[Option<usize>]) -> AttrData {
    let src: Vec<usize> = map.iter().map(|i| i.unwrap_or(usize::MAX)).collect();
    if map.iter().all(Option::is_some) {
        return data.gather_with_default(&src, src.len());
    }
    // Gather the present entries, then patch in defaults one by one.
    let def = data.default_of_len(1);
    let present: Vec<usize> = map.iter().map(|i| i.unwrap_or(0)).collect();
    let mut out = if data.is_empty() {
        data.default_of_len(map.len())
    } else {
        data.gather_with_default(&present, present.len())
    };
    for (i, m) in map.iter().enumerate() {
        if m.is_none() {
            set_from(&mut out, i, &def, 0);
        }
    }
    out
}

/// `dst[di] = src[si]` for two columns of the same type.
pub(crate) fn set_from(dst: &mut AttrData, di: usize, src: &AttrData, si: usize) {
    macro_rules! arm {
        ($($v:ident),*) => {
            match (dst, src) {
                $((AttrData::$v(d), AttrData::$v(s)) => Arc::make_mut(d)[di] = s[si].clone(),)*
                _ => {}
            }
        };
    }
    arm!(
        Float, Int, Int8, Bool, Float2, Float3, Float4, FloatColor, ByteColor, Int16x2, Int32x2,
        Quaternion, Float4x4, String
    );
}

fn rcp(n: usize) -> f32 {
    1.0 / n as f32
}

fn mix_f<const N: usize>(v: &[[f32; N]], g: &[usize]) -> [f32; N] {
    let w = rcp(g.len());
    let mut acc = [0.0f32; N];
    for &i in g {
        for k in 0..N {
            acc[k] += v[i][k] * w;
        }
    }
    acc
}

/// `std::round` (half away from zero) of a double, as `int`.
fn round_i32(x: f64) -> i32 {
    x.round() as i32
}

// Ported from blender/source/blender/blenkernel/BKE_attribute_math.hh (mix_indices, the
// DefaultMixer accumulation types).
/// `attribute_math::mix_groups` over one column; `None` for STRING (skipped by
/// `mix_attributes`). An empty group gives the type's default.
pub(crate) fn mix_groups(data: &AttrData, groups: &[SmallVec<[usize; 2]>]) -> Option<AttrData> {
    macro_rules! each {
        ($v:expr, |$g:ident| $body:expr) => {
            Arc::new(
                groups
                    .iter()
                    .map(|$g| {
                        if $g.is_empty() {
                            Default::default()
                        } else {
                            $body
                        }
                    })
                    .collect(),
            )
        };
    }
    Some(match data {
        AttrData::Float(v) => AttrData::Float(each!(v, |g| {
            let w = rcp(g.len());
            g.iter().fold(0.0f32, |a, &i| a + v[i] * w)
        })),
        AttrData::Float2(v) => AttrData::Float2(each!(v, |g| mix_f(v, g))),
        AttrData::Float3(v) => AttrData::Float3(each!(v, |g| mix_f(v, g))),
        AttrData::Float4(v) => AttrData::Float4(each!(v, |g| mix_f(v, g))),
        AttrData::FloatColor(v) => AttrData::FloatColor(each!(v, |g| mix_f(v, g))),
        AttrData::Int(v) => AttrData::Int(each!(v, |g| {
            let w = f64::from(rcp(g.len()));
            round_i32(g.iter().fold(0.0f64, |a, &i| a + f64::from(v[i]) * w))
        })),
        AttrData::Int8(v) => AttrData::Int8(each!(v, |g| {
            let w = rcp(g.len());
            g.iter()
                .fold(0.0f32, |a, &i| a + f32::from(v[i]) * w)
                .round() as i8
        })),
        AttrData::Bool(v) => AttrData::Bool(each!(v, |g| g.iter().any(|&i| v[i]))),
        AttrData::ByteColor(v) => AttrData::ByteColor(each!(v, |g| {
            let w = rcp(g.len());
            let mut acc = [0.0f32; 4];
            for &i in g {
                for k in 0..4 {
                    acc[k] += f32::from(v[i][k]) * w;
                }
            }
            // `ColorGeometry4b(float, …)`: an implicit float → uint8 conversion (truncation).
            acc.map(|x| x as u8)
        })),
        AttrData::Int16x2(v) => AttrData::Int16x2(each!(v, |g| {
            let w = rcp(g.len());
            let mut acc = [0.0f32; 2];
            for &i in g {
                acc[0] += f32::from(v[i][0]) * w;
                acc[1] += f32::from(v[i][1]) * w;
            }
            acc.map(|x| x.round() as i16)
        })),
        AttrData::Int32x2(v) => AttrData::Int32x2(each!(v, |g| {
            let w = f64::from(rcp(g.len()));
            let mut acc = [0.0f64; 2];
            for &i in g {
                acc[0] += f64::from(v[i][0]) * w;
                acc[1] += f64::from(v[i][1]) * w;
            }
            acc.map(round_i32)
        })),
        AttrData::Quaternion(v) => AttrData::Quaternion(Arc::new(
            groups
                .iter()
                .map(|g| {
                    if g.is_empty() {
                        [1.0, 0.0, 0.0, 0.0]
                    } else {
                        mix_quaternions(v, g)
                    }
                })
                .collect(),
        )),
        AttrData::Float4x4(v) => {
            let ident = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            // frap: the first matrix of the group (Blender mixes location, rotation and scale).
            AttrData::Float4x4(Arc::new(
                groups
                    .iter()
                    .map(|g| g.first().map_or(ident, |&i| v[i]))
                    .collect(),
            ))
        }
        AttrData::String(_) => return None,
        other => {
            let first: Vec<usize> = groups.iter().map(|g| g.first().copied().unwrap_or(0)).collect();
            other.gather_with_default(&first, first.len())
        }
    })
}

// Ported from blender/source/blender/blenlib/BLI_math_quaternion.hh (Quaternion::expmap,
// Quaternion::expmap(float3)).
fn mix_quaternions(v: &[[f32; 4]], g: &[usize]) -> [f32; 4] {
    let w = rcp(g.len());
    let mut acc = [0.0f32; 3];
    for &i in g {
        let e = quat_to_expmap(v[i]);
        for k in 0..3 {
            acc[k] += e[k] * w;
        }
    }
    expmap_to_quat(acc)
}

fn quat_to_expmap(q: [f32; 4]) -> [f32; 3] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len == 0.0 {
        return [0.0; 3];
    }
    let q = q.map(|x| x / len);
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    let s = (x * x + y * y + z * z).sqrt();
    if s < 1e-8 {
        return [0.0; 3];
    }
    let angle = 2.0 * libm::atan2f(s, w);
    [x / s * angle, y / s * angle, z / s * angle]
}

fn expmap_to_quat(e: [f32; 3]) -> [f32; 4] {
    let angle = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
    if angle < 1e-8 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let (s, c) = libm::sincosf(angle * 0.5);
    let f = s / angle;
    [c, e[0] * f, e[1] * f, e[2] * f]
}

// Ported from blender/source/blender/blenkernel/intern/deform.cc (bke::mix_deform_verts).
/// `mix_deform_verts` without weights: one source is copied; several are averaged per group
/// (sorted by group index).
pub(crate) fn mix_deform_verts(src: &[DeformVert], g: &[usize]) -> DeformVert {
    match g {
        [] => DeformVert::default(),
        [i] => src[*i].clone(),
        _ => {
            let inv = rcp(g.len());
            let mut out: SmallVec<[(u32, f32); 4]> = SmallVec::new();
            for &i in g {
                for &(def_nr, w) in &src[i].weights {
                    match out.iter_mut().find(|(d, _)| *d == def_nr) {
                        Some(e) => e.1 += w * inv,
                        None => out.push((def_nr, w * inv)),
                    }
                }
            }
            out.sort_by_key(|e| e.0);
            DeformVert { weights: out }
        }
    }
}

/// `BKE_defvert_find_weight`.
pub(crate) fn defvert_find_weight(dv: &DeformVert, index: u32) -> f32 {
    dv.weights
        .iter()
        .find(|(d, _)| *d == index)
        .map_or(0.0, |e| e.1)
}

/// `BKE_id_defgroup_name_index` on a mesh: the group index of `name`, `None` for an empty or
/// unknown name.
pub(crate) fn defgroup_index(mesh: &Mesh, name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    mesh.vertex_group_names
        .iter()
        .position(|n| n == name)
        .map(|i| i as u32)
}

/// The UV maps (FLOAT2 corner attributes that are not internal), by name.
pub(crate) fn uv_map_names(mesh: &Mesh) -> Vec<smol_str::SmolStr> {
    mesh.uv_maps()
        .into_iter()
        .map(|(n, _)| smol_str::SmolStr::new(n))
        .collect()
}

/// Mutable FLOAT2 corner column `name`.
pub(crate) fn float2_mut<'a>(mesh: &'a mut Mesh, name: &str) -> Option<&'a mut Vec<[f32; 2]>> {
    match mesh.attributes_mut().get_mut(name).map(|a| &mut a.data) {
        Some(AttrData::Float2(v)) => Some(Arc::make_mut(v)),
        _ => None,
    }
}

/// A typed read of an INT column on `domain` (`material_index`, …).
pub(crate) fn int_attr<'a>(mesh: &'a Mesh, name: &str, domain: Domain) -> Option<&'a [i32]> {
    match mesh.attributes().get(name) {
        Some(Attribute {
            domain: d,
            data: AttrData::Int(v),
            ..
        }) if *d == domain => Some(v.as_slice()),
        _ => None,
    }
}

/// A typed read of a FLOAT column on `domain` (`bevel_weight_*`, `crease_*`).
pub(crate) fn float_attr<'a>(mesh: &'a Mesh, name: &str, domain: Domain) -> Option<&'a [f32]> {
    match mesh.attributes().get(name) {
        Some(Attribute {
            domain: d,
            data: AttrData::Float(v),
            ..
        }) if *d == domain => Some(v.as_slice()),
        _ => None,
    }
}

// ------------------------------------------------------------------------------- BLI_math

/// A 4×4 matrix in Blender's `float[4][4]` layout (`m[col][row]`, `m[3]` = translation).
pub(crate) type M4 = [[f32; 4]; 4];

/// `unit_m4`.
pub(crate) fn unit_m4() -> M4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `glam::Mat4` → `float[4][4]`.
pub(crate) fn m4_from_glam(m: glam::Mat4) -> M4 {
    m.to_cols_array_2d()
}

/// `float[4][4]` → `glam::Mat4`.
pub(crate) fn m4_to_glam(m: &M4) -> glam::Mat4 {
    glam::Mat4::from_cols_array_2d(m)
}

// Ported from blender/source/blender/blenlib/intern/math_matrix.cc (mul_m4_m4m4).
/// `mul_m4_m4m4(R, A, B)` = A·B, with the SSE2 path's evaluation order (Blender's builds,
/// arm64 through sse2neon): `(B0·A0 + B1·A1) + (B2·A2 + B3·A3)`.
pub(crate) fn mul_m4_m4m4(a: &M4, b: &M4) -> M4 {
    let mut r = [[0.0f32; 4]; 4];
    for (j, rj) in r.iter_mut().enumerate() {
        for (k, rjk) in rj.iter_mut().enumerate() {
            *rjk = (b[j][0] * a[0][k] + b[j][1] * a[1][k]) + (b[j][2] * a[2][k] + b[j][3] * a[3][k]);
        }
    }
    r
}

/// `mul_m4_v3`: a point.
pub(crate) fn mul_m4_v3(m: &M4, v: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = v;
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * z + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * z + m[3][2],
    ]
}

/// `mul_mat3_m4_v3`: a direction (upper 3×3).
pub(crate) fn mul_mat3_m4_v3(m: &M4, v: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = v;
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z,
        x * m[0][1] + y * m[1][1] + m[2][1] * z,
        x * m[0][2] + y * m[1][2] + m[2][2] * z,
    ]
}

/// `invert_m4_m4` (glam's inverse; a singular matrix gives the identity, as Blender's
/// `invert_m4_m4` leaves `r` as the unit matrix when it fails).
pub(crate) fn invert_m4(m: &M4) -> M4 {
    let g = m4_to_glam(m);
    if g.determinant() == 0.0 {
        return unit_m4();
    }
    m4_from_glam(g.inverse())
}

/// `len_squared_v3v3`.
pub(crate) fn len_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// `compare_len_v3v3(a, b, limit)`: `len_squared_v3v3(a, b) <= limit²`.
pub(crate) fn compare_len(a: [f32; 3], b: [f32; 3], limit: f32) -> bool {
    len_sq(a, b) <= limit * limit
}

/// `normalize_v3` (returns the vector and its original length; a zero vector stays zero).
pub(crate) fn normalize(v: [f32; 3]) -> ([f32; 3], f32) {
    let d = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d > 1.0e-35 {
        let l = d.sqrt();
        let f = 1.0 / l;
        ([v[0] * f, v[1] * f, v[2] * f], l)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `dot_v3v3`.
pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
