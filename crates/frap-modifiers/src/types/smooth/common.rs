// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the modifier types of WP 4.3 (SMOOTH, CAST, SIMPLE_DEFORM,
//! LAPLACIANSMOOTH, WIREFRAME, MASK, BUILD, SCREW, WEIGHTED_NORMAL, NORMAL_EDIT, TRIANGULATE):
//! vertex-group lookups (`MOD_get_vgroup`, `BKE_defvert_find_weight`), a position-only deform
//! wrapper, and the mesh rebuild that carries every attribute of the input through per-domain
//! source maps (Blender's `BKE_mesh_new_nomain_from_template` followed by
//! `LegacyMeshInterpolator::copy` / `mix`).
//!
//! Lives under `types/smooth/` (a submodule of a file WP 4.3 owns, ARCHITECTURE §22.1); the other
//! type files reach it as `super::smooth::common`.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_util.cc (MOD_get_vgroup),
//! blender/source/blender/blenkernel/intern/deform.cc (BKE_defvert_find_weight),
//! blender/source/blender/blenkernel/BKE_attribute_math.hh (mix_indices with weights) and
//! blender/source/blender/blenkernel/intern/customdata.cc (layerInterp_mdeformvert).

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use smallvec::SmallVec;

/// `position`.
pub(crate) const POSITION: &str = "position";
/// `.edge_verts`.
pub(crate) const EDGE_VERTS: &str = ".edge_verts";
/// `.corner_vert`.
pub(crate) const CORNER_VERT: &str = ".corner_vert";
/// `.corner_edge`.
pub(crate) const CORNER_EDGE: &str = ".corner_edge";

/// `BKE_id_defgroup_name_index` on the mesh: the vertex group's index, `None` when the name is
/// empty or unknown.
pub(crate) fn defgroup_index(mesh: &Mesh, name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    mesh.vertex_group_names
        .iter()
        .position(|n| n == name)
        .map(|i| i as u32)
}

/// `MOD_get_vgroup`: the deform weights and the group index, when the group exists and the
/// mesh has weights (`mesh->deform_verts().data()` is null otherwise).
pub(crate) fn get_vgroup<'a>(mesh: &'a Mesh, name: &str) -> Option<(&'a [DeformVert], u32)> {
    let index = defgroup_index(mesh, name)?;
    let dverts = mesh.deform_verts().filter(|d| !d.is_empty())?;
    Some((dverts, index))
}

/// `BKE_defvert_find_weight`: the weight of group `index`, 0 when unassigned.
pub(crate) fn find_weight(dv: &DeformVert, index: u32) -> f32 {
    dv.weights
        .iter()
        .find(|(g, _)| *g == index)
        .map_or(0.0, |(_, w)| *w)
}

/// `BKE_defvert_find_index(dv, index) != nullptr`.
pub(crate) fn has_group(dv: &DeformVert, index: u32) -> bool {
    dv.weights.iter().any(|(g, _)| *g == index)
}

/// The raw inputs of `BKE_defvert_array_find_weight_safe`: the group index (`None` = -1) and
/// the mesh's weights (`None` = null when the mesh has none).
pub(crate) fn vgroup_raw<'a>(
    mesh: &'a Mesh,
    name: &str,
) -> (Option<u32>, Option<&'a [DeformVert]>) {
    let index = defgroup_index(mesh, name);
    let dverts = index.and(mesh.deform_verts().filter(|d| !d.is_empty()));
    (index, dverts)
}

/// `BKE_defvert_array_find_weight_safe(dvert, i, index, invert)`: 1 when the group does not
/// exist; 0 (1 inverted) when it exists but the mesh has no weights; else the weight.
pub(crate) fn weight_safe(
    raw: (Option<u32>, Option<&[DeformVert]>),
    i: usize,
    invert: bool,
) -> f32 {
    let Some(index) = raw.0 else {
        return 1.0;
    };
    let Some(dv) = raw.1 else {
        return if invert { 1.0 } else { 0.0 };
    };
    let w = find_weight(&dv[i], index);
    if invert { 1.0 - w } else { w }
}

/// Runs a `deform_verts` callback on a copy of the positions and writes them back (an
/// `OnlyDeform` modifier changes nothing else).
pub(crate) fn deform(mut mesh: Mesh, f: impl FnOnce(&Mesh, &mut [[f32; 3]])) -> Mesh {
    let mut positions = mesh.positions().to_vec();
    f(&mesh, &mut positions);
    mesh.set_positions(positions);
    mesh
}

/// Where one element of a rebuilt mesh takes its attribute values from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Src {
    /// `LegacyMeshInterpolator::copy(i, dst, 1)`.
    Copy(usize),
    /// `LegacyMeshInterpolator::mix(indices, weights, dst)`.
    Mix(SmallVec<[(usize, f32); 2]>),
    /// Left at the attribute's default (`CD_SET_DEFAULT` of the template).
    Default,
}

/// The new topology of a rebuilt mesh (positions follow the vertex sources unless set).
#[derive(Clone, Debug, Default)]
pub(crate) struct Topology {
    /// `.edge_verts`.
    pub edges: Vec<[i32; 2]>,
    /// Face offsets (`faces + 1` entries).
    pub face_offsets: Vec<i32>,
    /// `.corner_vert`.
    pub corner_verts: Vec<i32>,
    /// `.corner_edge`.
    pub corner_edges: Vec<i32>,
}

/// Per-domain sources of a rebuild.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sources {
    /// Vertices (positions and vertex-group weights included).
    pub vert: Vec<Src>,
    /// Edges.
    pub edge: Vec<Src>,
    /// Faces.
    pub face: Vec<Src>,
    /// Corners.
    pub corner: Vec<Src>,
}

fn is_topology(name: &str) -> bool {
    matches!(name, EDGE_VERTS | CORNER_VERT | CORNER_EDGE)
}

/// `BKE_mesh_new_nomain_from_template(src, …)` filled through `src`'s attributes: every
/// attribute of `src` (positions included) is carried to the new elements by its domain's
/// sources, vertex-group weights follow the vertex sources, and the mesh parameters
/// (materials, vertex-group names, active layers, …) are kept.
pub(crate) fn rebuild(src: &Mesh, topo: Topology, s: &Sources) -> Mesh {
    debug_assert_eq!(s.edge.len(), topo.edges.len());
    debug_assert_eq!(s.face.len() + 1, topo.face_offsets.len().max(1));
    debug_assert_eq!(s.corner.len(), topo.corner_verts.len());
    let mut out = src.clone();
    let Topology {
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
    } = topo;
    let face_offsets = if face_offsets.is_empty() {
        vec![0]
    } else {
        face_offsets
    };
    out.set_topology(face_offsets, corner_verts, edges);
    out.attributes_mut().insert(Attribute {
        name: CORNER_EDGE.into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_edges)),
    });
    let carried: Vec<Attribute> = src
        .attributes()
        .iter()
        .filter(|a| !is_topology(&a.name))
        .map(|a| {
            let srcs = match a.domain {
                Domain::Point => &s.vert,
                Domain::Edge => &s.edge,
                Domain::Face => &s.face,
                Domain::Corner => &s.corner,
            };
            Attribute {
                name: a.name.clone(),
                domain: a.domain,
                data: carry(&a.data, srcs),
            }
        })
        .collect();
    for a in carried {
        out.attributes_mut().insert(a);
    }
    if !src.attributes().iter().any(|a| a.name == POSITION) {
        out.set_positions(vec![[0.0; 3]; s.vert.len()]);
    }
    if let Some(dv) = src.deform_verts() {
        let new: Vec<DeformVert> = s
            .vert
            .iter()
            .map(|x| match x {
                Src::Copy(i) => dv[*i].clone(),
                Src::Mix(w) => interp_deform_vert(dv, w),
                Src::Default => DeformVert::default(),
            })
            .collect();
        *out.deform_verts_mut() = new;
    }
    out
}

/// `layerInterp_mdeformvert`: weighted sum per group (new groups prepended, as Blender's
/// inline linked list does), clamped to 1.
pub(crate) fn interp_deform_vert(dv: &[DeformVert], w: &[(usize, f32)]) -> DeformVert {
    let mut list: Vec<(u32, f32)> = Vec::new();
    for &(i, iw) in w {
        for &(g, gw) in &dv[i].weights {
            let weight = gw * iw;
            if weight == 0.0 {
                continue;
            }
            if let Some(n) = list.iter_mut().find(|(lg, _)| *lg == g) {
                n.1 += weight;
            } else {
                list.insert(0, (g, weight));
            }
        }
    }
    DeformVert {
        weights: list.into_iter().map(|(g, w)| (g, w.min(1.0))).collect(),
    }
}

fn safe_rcp(x: f32) -> f32 {
    if x != 0.0 { 1.0 / x } else { 0.0 }
}

/// `std::round` (half away from zero).
fn round_f64(x: f64) -> f64 {
    x.round()
}

macro_rules! mixv {
    ($v:expr, $w:expr, $n:literal) => {{
        let mut acc = [0.0f32; $n];
        let mut wsum = 0.0f32;
        for &(i, wi) in $w {
            for k in 0..$n {
                acc[k] += $v[i][k] * wi;
            }
            wsum += wi;
        }
        let r = safe_rcp(wsum);
        let mut out = [0.0f32; $n];
        for k in 0..$n {
            out[k] = acc[k] * r;
        }
        out
    }};
}

/// Carries one column through `srcs`.
pub(crate) fn carry(data: &AttrData, srcs: &[Src]) -> AttrData {
    let def = data.default_of_len(srcs.len());
    macro_rules! build {
        ($v:ident, $variant:ident, $def:ident, $mix:expr) => {{
            let mut out = match $def {
                AttrData::$variant(d) => (*d).clone(),
                _ => unreachable!("default_of_len keeps the type"),
            };
            for (k, s) in srcs.iter().enumerate() {
                match s {
                    Src::Copy(i) => out[k] = $v[*i].clone(),
                    Src::Mix(w) =>
                    {
                        #[allow(clippy::redundant_closure_call)]
                        if let Some(x) = ($mix)(&**$v, &w[..]) {
                            out[k] = x;
                        }
                    }
                    Src::Default => {}
                }
            }
            AttrData::$variant(Arc::new(out))
        }};
    }
    match data {
        AttrData::Float(v) => build!(v, Float, def, |v: &[f32], w: &[(usize, f32)]| {
            let mut acc = 0.0f32;
            let mut ws = 0.0f32;
            for &(i, wi) in w {
                acc += v[i] * wi;
                ws += wi;
            }
            Some(acc * safe_rcp(ws))
        }),
        AttrData::Int(v) => build!(v, Int, def, |v: &[i32], w: &[(usize, f32)]| {
            let mut acc = 0.0f64;
            let mut ws = 0.0f32;
            for &(i, wi) in w {
                acc += v[i] as f64 * wi as f64;
                ws += wi;
            }
            Some(round_f64(acc * safe_rcp(ws) as f64) as i32)
        }),
        AttrData::Int8(v) => build!(v, Int8, def, |v: &[i8], w: &[(usize, f32)]| {
            let mut acc = 0.0f32;
            let mut ws = 0.0f32;
            for &(i, wi) in w {
                acc += v[i] as f32 * wi;
                ws += wi;
            }
            Some((acc * safe_rcp(ws)).round() as i8)
        }),
        AttrData::Bool(v) => build!(v, Bool, def, |v: &[bool], w: &[(usize, f32)]| {
            Some(w.iter().any(|&(i, wi)| v[i] && wi > 0.0))
        }),
        AttrData::Float2(v) => build!(v, Float2, def, |v: &[[f32; 2]], w: &[(usize, f32)]| {
            Some(mixv!(v, w, 2))
        }),
        AttrData::Float3(v) => build!(v, Float3, def, |v: &[[f32; 3]], w: &[(usize, f32)]| {
            Some(mixv!(v, w, 3))
        }),
        AttrData::Float4(v) => build!(v, Float4, def, |v: &[[f32; 4]], w: &[(usize, f32)]| {
            Some(mixv!(v, w, 4))
        }),
        AttrData::FloatColor(v) => {
            build!(v, FloatColor, def, |v: &[[f32; 4]], w: &[(usize, f32)]| {
                Some(mixv!(v, w, 4))
            })
        }
        AttrData::ByteColor(v) => {
            build!(v, ByteColor, def, |v: &[[u8; 4]], w: &[(usize, f32)]| {
                let f: Vec<[f32; 4]> = w.iter().map(|&(i, _)| v[i].map(|c| c as f32)).collect();
                let idx: Vec<(usize, f32)> =
                    w.iter().enumerate().map(|(k, &(_, wi))| (k, wi)).collect();
                let m = mixv!(f, &idx, 4);
                Some(m.map(|c| c as u8))
            })
        }
        AttrData::Int16x2(v) => build!(v, Int16x2, def, |v: &[[i16; 2]], w: &[(usize, f32)]| {
            let f: Vec<[f32; 2]> = w.iter().map(|&(i, _)| v[i].map(|c| c as f32)).collect();
            let idx: Vec<(usize, f32)> =
                w.iter().enumerate().map(|(k, &(_, wi))| (k, wi)).collect();
            let m = mixv!(f, &idx, 2);
            Some(m.map(|c| c.round() as i16))
        }),
        AttrData::Int32x2(v) => build!(v, Int32x2, def, |v: &[[i32; 2]], w: &[(usize, f32)]| {
            let mut acc = [0.0f64; 2];
            let mut ws = 0.0f32;
            for &(i, wi) in w {
                acc[0] += v[i][0] as f64 * wi as f64;
                acc[1] += v[i][1] as f64 * wi as f64;
                ws += wi;
            }
            let r = safe_rcp(ws) as f64;
            Some([round_f64(acc[0] * r) as i32, round_f64(acc[1] * r) as i32])
        }),
        AttrData::Quaternion(v) => {
            build!(v, Quaternion, def, |v: &[[f32; 4]], w: &[(usize, f32)]| {
                let e: Vec<[f32; 3]> = w.iter().map(|&(i, _)| quat_to_expmap(v[i])).collect();
                let idx: Vec<(usize, f32)> =
                    w.iter().enumerate().map(|(k, &(_, wi))| (k, wi)).collect();
                Some(expmap_to_quat(mixv!(e, &idx, 3)))
            })
        }
        AttrData::Float4x4(v) => {
            build!(
                v,
                Float4x4,
                def,
                |v: &[[[f32; 4]; 4]], w: &[(usize, f32)]| {
                    let flat: Vec<[f32; 16]> = w
                        .iter()
                        .map(|&(i, _)| {
                            let m = v[i];
                            let mut f = [0.0; 16];
                            for r in 0..4 {
                                f[r * 4..r * 4 + 4].copy_from_slice(&m[r]);
                            }
                            f
                        })
                        .collect();
                    let idx: Vec<(usize, f32)> =
                        w.iter().enumerate().map(|(k, &(_, wi))| (k, wi)).collect();
                    let f = mixv!(flat, &idx, 16);
                    let mut m = [[0.0; 4]; 4];
                    for r in 0..4 {
                        m[r].copy_from_slice(&f[r * 4..r * 4 + 4]);
                    }
                    Some(m)
                }
            )
        }
        // Strings have no mixer (`DefaultMixer<std::string>` is void): mixed elements keep the
        // default.
        AttrData::String(v) => {
            build!(
                v,
                String,
                def,
                |_v: &[smol_str::SmolStr], _w: &[(usize, f32)]| None
            )
        }
        _ => data.default_of_len(srcs.len()),
    }
}

/// `math::Quaternion::expmap` (w-first storage).
fn quat_to_expmap(q: [f32; 4]) -> [f32; 3] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len == 0.0 {
        return [0.0; 3];
    }
    let (w, x, y, z) = (q[0] / len, q[1] / len, q[2] / len, q[3] / len);
    let s = (x * x + y * y + z * z).sqrt();
    if s == 0.0 {
        return [0.0; 3];
    }
    let angle = 2.0 * libm::atan2f(s, w);
    [x / s * angle, y / s * angle, z / s * angle]
}

/// `math::Quaternion::expmap(v)` (w-first storage).
fn expmap_to_quat(e: [f32; 3]) -> [f32; 4] {
    let angle = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
    if angle == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let half = angle * 0.5;
    let s = libm::sinf(half) / angle;
    [libm::cosf(half), e[0] * s, e[1] * s, e[2] * s]
}

/// Test support: a [`crate::ModEvalCx`] over a plain `Main`, and Blender dumps of evaluated
/// meshes (the golden fixtures of `tests/golden/fixtures/modifiers/b/`).
#[cfg(test)]
pub(crate) mod testing {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use frap_boolean::BooleanSolver;
    use frap_imbuf::DecodedImage;
    use frap_mesh::{DeformVert, Mesh};
    use frap_rna::{IdRef, ImageId, ObjectId, Report};
    use frap_scene::{EvalMode, EvaluatedPose, Main};
    use frap_subdiv::Subdivider;
    use glam::Mat4;
    use serde_json::Value as Json;

    use crate::{ModError, ModEvalCx, Object};

    /// A context for one object; other objects' matrices come from `matrices`.
    pub(crate) struct TestCx {
        pub main: Main,
        pub ob: IdRef<ObjectId>,
        pub matrices: HashMap<IdRef<ObjectId>, Mat4>,
        pub frame: f32,
        pub mode: EvalMode,
        pub reports: Vec<Report>,
    }

    impl TestCx {
        /// A `Main` with one object "Object" (no data).
        pub fn new() -> TestCx {
            let mut main = Main::default();
            let mut log = frap_scene::ChangeLog::default();
            let ob = main
                .objects
                .insert("Object", Object::new("Object", None), &mut log);
            TestCx {
                main,
                ob,
                matrices: HashMap::new(),
                frame: 1.0,
                mode: EvalMode::Viewport,
                reports: Vec::new(),
            }
        }
        /// Adds another object (an empty) with the given world matrix.
        pub fn add_object(&mut self, name: &str, world: Mat4) -> IdRef<ObjectId> {
            let mut log = frap_scene::ChangeLog::default();
            let r = self
                .main
                .objects
                .insert(name, Object::new(name, None), &mut log);
            self.matrices.insert(r, world);
            r
        }
    }

    impl ModEvalCx for TestCx {
        fn main(&self) -> &Main {
            &self.main
        }
        fn object(&self) -> &Object {
            self.main.objects.get(self.ob).expect("test object")
        }
        fn object_ref(&self) -> IdRef<ObjectId> {
            self.ob
        }
        fn evaluated_mesh(&mut self, _ob: IdRef<ObjectId>) -> Result<Arc<Mesh>, ModError> {
            Err(ModError::Dependency("no evaluated meshes in tests".into()))
        }
        fn object_matrix_world(&mut self, ob: IdRef<ObjectId>) -> Mat4 {
            self.matrices.get(&ob).copied().unwrap_or(Mat4::IDENTITY)
        }
        fn pose(&mut self, _arm: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError> {
            Err(ModError::Dependency("no poses in tests".into()))
        }
        fn image(&mut self, _im: IdRef<ImageId>) -> Option<Arc<DecodedImage>> {
            None
        }
        fn mode(&self) -> EvalMode {
            self.mode
        }
        fn frame(&self) -> f32 {
            self.frame
        }
        fn report(&mut self, r: Report) {
            self.reports.push(r);
        }
        fn subdivider(&self) -> &dyn Subdivider {
            &frap_subdiv::catmull_clark::CatmullClark
        }
        fn boolean_solver(&self) -> &dyn BooleanSolver {
            frap_boolean::default_solver()
        }
    }

    /// A modifier named "M" with `settings` and Blender's default flags.
    pub(crate) fn modifier(settings: crate::ModifierSettings) -> crate::Modifier {
        crate::Modifier {
            name: "M".into(),
            show_viewport: true,
            show_render: true,
            show_in_editmode: true,
            show_on_cage: false,
            show_expanded: true,
            is_active: true,
            use_pin_to_last: false,
            persistent_uid: 1,
            settings,
        }
    }

    /// `tests/golden/fixtures/modifiers/b/<rel>`.
    pub(crate) fn fixture_path(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/fixtures/modifiers/b")
            .join(rel)
    }

    /// Loads a JSON fixture.
    pub(crate) fn load(rel: &str) -> Json {
        let p = fixture_path(rel);
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
        serde_json::from_str(&text).expect("fixture JSON")
    }

    fn f3(v: &Json) -> [f32; 3] {
        let a = v.as_array().expect("vec3");
        [
            a[0].as_f64().unwrap() as f32,
            a[1].as_f64().unwrap() as f32,
            a[2].as_f64().unwrap() as f32,
        ]
    }

    /// A mesh from a dump `{positions, edges, faces, groups?, weights?, sharp_face?, uv?}`
    /// with Blender's exact element order.
    pub(crate) fn mesh_from_dump(d: &Json) -> Mesh {
        let positions: Vec<[f32; 3]> = d["positions"].as_array().unwrap().iter().map(f3).collect();
        let edges: Vec<[i32; 2]> = d["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| [e[0].as_i64().unwrap() as i32, e[1].as_i64().unwrap() as i32])
            .collect();
        let mut offsets = vec![0i32];
        let mut corner_verts = Vec::new();
        for f in d["faces"].as_array().unwrap() {
            for v in f.as_array().unwrap() {
                corner_verts.push(v.as_i64().unwrap() as i32);
            }
            offsets.push(corner_verts.len() as i32);
        }
        let mut m = Mesh::new("Mesh");
        m.set_positions(positions);
        m.set_topology(offsets, corner_verts, edges);
        if let Some(groups) = d.get("groups").and_then(Json::as_array) {
            m.vertex_group_names = groups.iter().map(|g| g.as_str().unwrap().into()).collect();
            let dv: Vec<DeformVert> = d["weights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| DeformVert {
                    weights: v
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| (p[0].as_u64().unwrap() as u32, p[1].as_f64().unwrap() as f32))
                        .collect(),
                })
                .collect();
            // Blender creates the weight layer on the first assignment only.
            if dv.iter().any(|d| !d.weights.is_empty()) {
                *m.deform_verts_mut() = dv;
            }
        }
        if let Some(uv) = d.get("uv").and_then(Json::as_array) {
            let uv: Vec<[f32; 2]> = uv
                .iter()
                .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
                .collect();
            m.attributes_mut().insert(frap_mesh::Attribute {
                name: "UVMap".into(),
                domain: frap_mesh::Domain::Corner,
                data: frap_mesh::AttrData::Float2(Arc::new(uv)),
            });
            m.uv_active = Some("UVMap".into());
            m.uv_active_render = Some("UVMap".into());
        }
        if let Some(se) = d.get("sharp_edge_flags").and_then(Json::as_array) {
            let v: Vec<bool> = se.iter().map(|b| b.as_bool().unwrap()).collect();
            m.set_bool_attribute(frap_mesh::SHARP_EDGE, frap_mesh::Domain::Edge, v);
        }
        if let Some(sf) = d.get("sharp_face").and_then(Json::as_array) {
            let v: Vec<bool> = sf.iter().map(|b| b.as_bool().unwrap()).collect();
            m.set_bool_attribute(frap_mesh::SHARP_FACE, frap_mesh::Domain::Face, v);
        }
        m
    }

    /// Asserts `got`'s positions equal the dump's within `tol`, index by index.
    pub(crate) fn assert_positions(got: &Mesh, want: &Json, tol: f32) {
        let w: Vec<[f32; 3]> = want["positions"]
            .as_array()
            .unwrap()
            .iter()
            .map(f3)
            .collect();
        assert_eq!(got.verts_num(), w.len(), "vertex count");
        for (i, (a, b)) in got.positions().iter().zip(&w).enumerate() {
            for k in 0..3 {
                assert!(
                    (a[k] - b[k]).abs() <= tol * b[k].abs().max(1.0),
                    "vertex {i}: {a:?} vs Blender {b:?}"
                );
            }
        }
    }

    /// Asserts the topology (edges, faces) equals the dump's exactly.
    pub(crate) fn assert_topology(got: &Mesh, want: &Json) {
        let edges: Vec<[i32; 2]> = want["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| [e[0].as_i64().unwrap() as i32, e[1].as_i64().unwrap() as i32])
            .collect();
        assert_eq!(got.edges(), &edges[..], "edges");
        let faces: Vec<Vec<i32>> = want["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect()
            })
            .collect();
        let got_faces: Vec<Vec<i32>> = (0..got.faces_num())
            .map(|f| got.face(f).map(|c| got.corner_verts()[c]).collect())
            .collect();
        assert_eq!(got_faces, faces, "faces");
    }

    /// Asserts the first UV map equals the dump's `uv` within `tol` (when the dump has one).
    pub(crate) fn assert_uv(got: &Mesh, want: &Json, tol: f32) {
        let Some(w) = want.get("uv").and_then(Json::as_array) else {
            return;
        };
        let maps = got.uv_maps();
        let (_, g) = maps.first().expect("a UV map");
        assert_eq!(g.len(), w.len(), "UV count");
        for (i, (a, b)) in g.iter().zip(w).enumerate() {
            let b = [b[0].as_f64().unwrap() as f32, b[1].as_f64().unwrap() as f32];
            assert!(
                (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol,
                "corner {i}: uv {a:?} vs Blender {b:?}"
            );
        }
    }

    /// Asserts a dumped attribute (`{domain, data_type, data}` under `name`, or null) equals
    /// frap's, values within `tol`.
    pub(crate) fn assert_attribute(got: &Mesh, want: &Json, name: &str, tol: f32) {
        let w = &want[name];
        let g = got.attributes().get(name);
        if w.is_null() {
            assert!(g.is_none(), "{name}: Blender has no such attribute");
            return;
        }
        let g = g.unwrap_or_else(|| panic!("{name}: missing in frap"));
        assert_eq!(
            g.domain.identifier(),
            w["domain"].as_str().unwrap(),
            "{name}"
        );
        let data = w["data"].as_array().unwrap();
        let num = |v: &Json| -> Vec<f64> {
            match v {
                Json::Bool(b) => vec![f64::from(u8::from(*b))],
                Json::Number(n) => vec![n.as_f64().unwrap()],
                Json::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
                _ => panic!("unexpected value {v}"),
            }
        };
        let got_vals: Vec<Vec<f64>> = match &g.data {
            frap_mesh::AttrData::Bool(v) => {
                v.iter().map(|b| vec![f64::from(u8::from(*b))]).collect()
            }
            frap_mesh::AttrData::Int(v) => v.iter().map(|x| vec![*x as f64]).collect(),
            frap_mesh::AttrData::Float(v) => v.iter().map(|x| vec![*x as f64]).collect(),
            frap_mesh::AttrData::Int16x2(v) => {
                v.iter().map(|x| vec![x[0] as f64, x[1] as f64]).collect()
            }
            frap_mesh::AttrData::Float2(v) => {
                v.iter().map(|x| vec![x[0] as f64, x[1] as f64]).collect()
            }
            frap_mesh::AttrData::Float3(v) => v
                .iter()
                .map(|x| vec![x[0] as f64, x[1] as f64, x[2] as f64])
                .collect(),
            other => panic!("{name}: unsupported type {}", other.data_type()),
        };
        assert_eq!(got_vals.len(), data.len(), "{name}: length");
        for (i, (a, b)) in got_vals.iter().zip(data).enumerate() {
            let b = num(b);
            for (x, y) in a.iter().zip(&b) {
                assert!(
                    (x - y).abs() <= tol as f64 * y.abs().max(1.0),
                    "{name}[{i}]: {a:?} vs Blender {b:?}"
                );
            }
        }
    }

    /// Asserts corner normals equal the dump's within `tol`.
    pub(crate) fn assert_corner_normals(got: &Mesh, want: &Json, tol: f32) {
        let w: Vec<[f32; 3]> = want["corner_normals"]
            .as_array()
            .unwrap()
            .iter()
            .map(f3)
            .collect();
        let g = got.corner_normals();
        assert_eq!(g.len(), w.len(), "corner count");
        for (i, (a, b)) in g.iter().zip(&w).enumerate() {
            for k in 0..3 {
                assert!(
                    (a[k] - b[k]).abs() <= tol,
                    "corner {i}: {a:?} vs Blender {b:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deform_vert_interpolation_prepends_and_clamps() {
        let dv = vec![
            DeformVert {
                weights: [(0, 1.0), (1, 0.5)].into_iter().collect(),
            },
            DeformVert {
                weights: [(1, 1.0), (2, 0.25)].into_iter().collect(),
            },
        ];
        let r = interp_deform_vert(&dv, &[(0, 0.5), (1, 0.5)]);
        assert_eq!(r.weights.as_slice(), &[(2, 0.125), (1, 0.75), (0, 0.5)]);
        let r = interp_deform_vert(&dv, &[(0, 1.0), (1, 1.0)]);
        assert_eq!(r.weights.as_slice(), &[(2, 0.25), (1, 1.0), (0, 1.0)]);
    }

    #[test]
    fn carry_copies_mixes_and_defaults() {
        let d = AttrData::Float(Arc::new(vec![1.0, 3.0]));
        let out = carry(
            &d,
            &[
                Src::Copy(1),
                Src::Mix([(0, 0.25), (1, 0.75)].into_iter().collect()),
                Src::Default,
            ],
        );
        match out {
            AttrData::Float(v) => assert_eq!(v.as_slice(), &[3.0, 2.5, 0.0]),
            _ => panic!(),
        }
        let b = AttrData::Bool(Arc::new(vec![false, true]));
        match carry(&b, &[Src::Mix([(0, 1.0), (1, 0.0)].into_iter().collect())]) {
            AttrData::Bool(v) => assert_eq!(v.as_slice(), &[false]),
            _ => panic!(),
        }
        let i = AttrData::Int(Arc::new(vec![1, 2]));
        match carry(&i, &[Src::Mix([(0, 0.5), (1, 0.5)].into_iter().collect())]) {
            AttrData::Int(v) => assert_eq!(v.as_slice(), &[2]),
            _ => panic!(),
        }
    }

    #[test]
    fn quaternion_expmap_round_trip() {
        let q = [0.9238795, 0.0, 0.0, 0.38268343]; // 45° about Z
        let e = quat_to_expmap(q);
        assert!((e[2] - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        let back = expmap_to_quat(e);
        for k in 0..4 {
            assert!((back[k] - q[k]).abs() < 1e-5);
        }
    }
}
