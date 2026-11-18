// SPDX-License-Identifier: GPL-3.0-or-later
//! CustomData: typed columns by element slot, and interpolation (`blenkernel/intern/
//! customdata.cc`, `bmesh/intern/bmesh_interp.cc`; ARCHITECTURE §6.2). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1.
//!
//! A layer is a column with one value per **pool slot** of its domain (free slots hold
//! garbage), stored as a `frap_mesh::AttrData` so conversion to and from `Mesh` attributes is a
//! gather/scatter. As in Blender, the layers of one domain are **sorted by custom-data type**
//! (`customData_add_layer__internal` inserts after the last layer of a lower or equal type),
//! which is the order `BM_mesh_bm_to_me` writes generic attributes back in.
//!
//! Per-type behaviour follows `LAYERTYPEINFO`: BMesh defaults (`set_default_value`) are white
//! for colours and identity for quaternions/matrices, zero otherwise; interpolation is a
//! weighted sum for float types, rounded for `INT`, "any true with weight > 0" for booleans,
//! rounded and clamped for byte colours, an exponential-map mix for quaternions, and absent
//! for `INT8`, `INT16_2D`, `INT32_2D`, `FLOAT4X4` and `STRING` (the destination keeps its
//! value). Vertex groups (`CD_MDEFORMVERT`) are a separate column on the vertex domain.

use std::sync::Arc;

use frap_mesh::{AttrData, DeformVert};
use smol_str::SmolStr;

use crate::query::geom::{self, V3};
use crate::types::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// One layer.
#[derive(Clone, Debug)]
pub struct CdLayer {
    /// Attribute name (`UVMap`, `crease_edge`, `.pn.UVMap`, …).
    pub name: SmolStr,
    /// Values by element slot.
    pub data: AttrData,
}

/// The layers of one domain.
#[derive(Clone, Debug, Default)]
pub struct CustomData {
    /// Layers sorted by custom-data type, then creation order.
    pub layers: Vec<CdLayer>,
    /// `CD_MDEFORMVERT` (vertex domain only): vertex-group weights by slot.
    pub deform: Option<Vec<DeformVert>>,
    /// Active `FLOAT2` layer (`CustomData_get_active_layer_name(ldata, CD_PROP_FLOAT2)`).
    pub uv_active: Option<SmolStr>,
    /// Render `FLOAT2` layer (`CustomData_get_render_layer_name`).
    pub uv_render: Option<SmolStr>,
}

/// The `eCustomDataType` number of a column (`CD_PROP_*`), the layer sort key.
pub fn cd_type(d: &AttrData) -> u32 {
    match d {
        AttrData::Float(_) => 10,
        AttrData::Int(_) => 11,
        AttrData::String(_) => 12,
        AttrData::ByteColor(_) => 17,
        AttrData::Float4x4(_) => 20,
        AttrData::Int16x2(_) => 22,
        AttrData::Float4(_) => 24,
        AttrData::Int8(_) => 45,
        AttrData::Int32x2(_) => 46,
        AttrData::FloatColor(_) => 47,
        AttrData::Float3(_) => 48,
        AttrData::Float2(_) => 49,
        AttrData::Bool(_) => 50,
        AttrData::Quaternion(_) => 52,
        _ => 99,
    }
}

const IDENT4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Applies `$body` with `$v: &mut Vec<T>` and `$def: T` (the BMesh default) to any column.
macro_rules! with_col_mut {
    ($data:expr, |$v:ident, $def:ident| $body:expr) => {
        match $data {
            AttrData::Float(c) => {
                let $v = Arc::make_mut(c);
                let $def = 0.0f32;
                $body
            }
            AttrData::Int(c) => {
                let $v = Arc::make_mut(c);
                let $def = 0i32;
                $body
            }
            AttrData::Int8(c) => {
                let $v = Arc::make_mut(c);
                let $def = 0i8;
                $body
            }
            AttrData::Bool(c) => {
                let $v = Arc::make_mut(c);
                let $def = false;
                $body
            }
            AttrData::Float2(c) => {
                let $v = Arc::make_mut(c);
                let $def = [0.0f32; 2];
                $body
            }
            AttrData::Float3(c) => {
                let $v = Arc::make_mut(c);
                let $def = [0.0f32; 3];
                $body
            }
            AttrData::Float4(c) => {
                let $v = Arc::make_mut(c);
                let $def = [0.0f32; 4];
                $body
            }
            AttrData::FloatColor(c) => {
                let $v = Arc::make_mut(c);
                let $def = [1.0f32; 4];
                $body
            }
            AttrData::ByteColor(c) => {
                let $v = Arc::make_mut(c);
                let $def = [255u8; 4];
                $body
            }
            AttrData::Int16x2(c) => {
                let $v = Arc::make_mut(c);
                let $def = [0i16; 2];
                $body
            }
            AttrData::Int32x2(c) => {
                let $v = Arc::make_mut(c);
                let $def = [0i32; 2];
                $body
            }
            AttrData::Quaternion(c) => {
                let $v = Arc::make_mut(c);
                let $def = [1.0f32, 0.0, 0.0, 0.0];
                $body
            }
            AttrData::Float4x4(c) => {
                let $v = Arc::make_mut(c);
                let $def = IDENT4;
                $body
            }
            AttrData::String(c) => {
                let $v = Arc::make_mut(c);
                let $def = SmolStr::default();
                $body
            }
            _ => {}
        }
    };
}

fn madd<const N: usize>(src: &[[f32; N]], w: &[f32]) -> [f32; N] {
    let mut r = [0.0f32; N];
    for (s, &wt) in src.iter().zip(w) {
        for k in 0..N {
            r[k] += s[k] * wt;
        }
    }
    r
}

/// `round_fl_to_uchar_clamp`.
fn round_fl_to_uchar_clamp(f: f32) -> u8 {
    if f <= 0.0 {
        0
    } else if f > 255.0 - 0.5 {
        255
    } else {
        (f + 0.5) as u8
    }
}

// Ported from blender/source/blender/blenkernel/intern/attribute_math.hh
// (quat_to_expmap / expmap_to_quat, the DefaultMixer of quaternions).
fn quat_to_expmap(q: [f32; 4]) -> V3 {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let q = if len > 0.0 {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };
    let (axis, l) = geom::normalize_v3([q[1], q[2], q[3]]);
    if l == 0.0 {
        return [0.0; 3];
    }
    let angle = 2.0 * libm::atan2f(l, q[0]);
    geom::scale(axis, angle)
}

fn expmap_to_quat(e: V3) -> [f32; 4] {
    let (axis, angle) = geom::normalize_v3(e);
    if angle == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let (s, c) = libm::sincosf(angle * 0.5);
    [c, axis[0] * s, axis[1] * s, axis[2] * s]
}

/// One column's interpolation: `dst = mix(src, w)`, or nothing for types without `interp`.
fn interp_col(data: &mut AttrData, src: &[u32], w: &[f32], dst: u32) {
    let d = dst as usize;
    match data {
        AttrData::Float(c) => {
            let mut r = 0.0f32;
            for (&s, &wt) in src.iter().zip(w) {
                r += c[s as usize] * wt;
            }
            Arc::make_mut(c)[d] = r;
        }
        AttrData::Int(c) => {
            let mut r = 0.0f32;
            for (&s, &wt) in src.iter().zip(w) {
                r += c[s as usize] as f32 * wt;
            }
            Arc::make_mut(c)[d] = (r as f64).round() as i32;
        }
        AttrData::Bool(c) => {
            let r = src.iter().zip(w).any(|(&s, &wt)| c[s as usize] && wt > 0.0);
            Arc::make_mut(c)[d] = r;
        }
        AttrData::Float2(c) => {
            let v: Vec<[f32; 2]> = src.iter().map(|&s| c[s as usize]).collect();
            Arc::make_mut(c)[d] = madd(&v, w);
        }
        AttrData::Float3(c) => {
            let v: Vec<[f32; 3]> = src.iter().map(|&s| c[s as usize]).collect();
            Arc::make_mut(c)[d] = madd(&v, w);
        }
        AttrData::Float4(c) | AttrData::FloatColor(c) => {
            let v: Vec<[f32; 4]> = src.iter().map(|&s| c[s as usize]).collect();
            Arc::make_mut(c)[d] = madd(&v, w);
        }
        AttrData::ByteColor(c) => {
            let mut col = [0.0f32; 4];
            for (&s, &wt) in src.iter().zip(w) {
                let px = c[s as usize];
                for k in 0..4 {
                    col[k] += px[k] as f32 * wt;
                }
            }
            Arc::make_mut(c)[d] = col.map(round_fl_to_uchar_clamp);
        }
        AttrData::Quaternion(c) => {
            let mut acc = [0.0f32; 3];
            let mut total = 0.0f32;
            for (&s, &wt) in src.iter().zip(w) {
                geom::madd_v3_v3fl(&mut acc, quat_to_expmap(c[s as usize]), wt);
                total += wt;
            }
            let q = if total > 0.0 {
                expmap_to_quat(geom::scale(acc, 1.0 / total))
            } else {
                [1.0, 0.0, 0.0, 0.0]
            };
            Arc::make_mut(c)[d] = q;
        }
        _ => {}
    }
}

// Ported from blender/source/blender/blenkernel/intern/customdata.cc (layerInterp_mdeformvert).
fn interp_deform(col: &mut [DeformVert], src: &[u32], w: &[f32], dst: u32) {
    // Unique groups, most recently found first (Blender prepends to a linked list).
    let mut list: Vec<(u32, f32)> = Vec::new();
    for (&s, &wt) in src.iter().zip(w) {
        for &(def_nr, weight) in &col[s as usize].weights {
            let weight = weight * wt;
            if weight == 0.0 {
                continue;
            }
            if let Some(n) = list.iter_mut().find(|n| n.0 == def_nr) {
                n.1 += weight;
            } else {
                list.insert(0, (def_nr, weight));
            }
        }
    }
    let out = &mut col[dst as usize];
    out.weights.clear();
    for (g, wt) in list {
        out.weights.push((g, wt.min(1.0)));
    }
}

impl CustomData {
    /// Position of the layer named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }
    /// The layer named `name`.
    pub fn get(&self, name: &str) -> Option<&CdLayer> {
        self.layers.iter().find(|l| l.name == name)
    }
    /// Names of the `FLOAT2` layers in order (the UV maps of the loop domain).
    pub fn float2_names(&self) -> Vec<SmolStr> {
        self.layers
            .iter()
            .filter(|l| matches!(l.data, AttrData::Float2(_)))
            .map(|l| l.name.clone())
            .collect()
    }
    /// `CustomData_add_layer_named`: inserts `layer` after the last layer whose type is not
    /// greater (Blender keeps layers sorted by type); returns its position. The column must
    /// already have one value per slot.
    pub fn add_layer(&mut self, layer: CdLayer) -> usize {
        let t = cd_type(&layer.data);
        let mut i = self.layers.len();
        while i > 0 && cd_type(&self.layers[i - 1].data) > t {
            i -= 1;
        }
        self.layers.insert(i, layer);
        i
    }
    /// Removes the layer named `name` (`CustomData_free_layer_named`).
    pub fn remove_layer(&mut self, name: &str) -> Option<CdLayer> {
        let i = self.find(name)?;
        Some(self.layers.remove(i))
    }
    /// Grows every column to `slots` values (new slots get the type's default).
    pub fn ensure_slots(&mut self, slots: usize) {
        for l in &mut self.layers {
            if l.data.len() < slots {
                with_col_mut!(&mut l.data, |v, def| v.resize(slots, def));
            }
        }
        if let Some(d) = &mut self.deform
            && d.len() < slots
        {
            d.resize(slots, DeformVert::default());
        }
    }
    /// `CustomData_bmesh_copy_data`: copies every layer's value from slot `src` to slot `dst`.
    pub fn copy_elem(&mut self, src: u32, dst: u32) {
        if src == dst {
            return;
        }
        let (s, d) = (src as usize, dst as usize);
        self.ensure_slots(s.max(d) + 1);
        for l in &mut self.layers {
            with_col_mut!(&mut l.data, |v, def| {
                let _ = def;
                #[allow(clippy::clone_on_copy)]
                let x = v[s].clone();
                v[d] = x;
            });
        }
        if let Some(dv) = &mut self.deform {
            dv[d] = dv[s].clone();
        }
    }
    /// `CustomData_bmesh_set_default`: the type defaults in slot `i`.
    pub fn set_default(&mut self, i: u32) {
        let i = i as usize;
        self.ensure_slots(i + 1);
        for l in &mut self.layers {
            with_col_mut!(&mut l.data, |v, def| v[i] = def);
        }
        if let Some(dv) = &mut self.deform {
            dv[i] = DeformVert::default();
        }
    }
    /// `CustomData_bmesh_interp`: slot `dst` = Σ `weights[k]` · slot `src[k]` (types that do
    /// not interpolate keep `dst`'s value). Empty `weights` means equal weights.
    pub fn interp(&mut self, src: &[u32], weights: &[f32], dst: u32) {
        if src.is_empty() {
            return;
        }
        let max = src.iter().copied().max().unwrap_or(0).max(dst) as usize;
        self.ensure_slots(max + 1);
        let default_w;
        let w = if weights.is_empty() {
            default_w = vec![1.0 / src.len() as f32; src.len()];
            &default_w[..]
        } else {
            weights
        };
        for l in &mut self.layers {
            interp_col(&mut l.data, src, w, dst);
        }
        if let Some(dv) = &mut self.deform {
            interp_deform(dv, src, w, dst);
        }
    }
    /// The value of a `FLOAT2` layer at `slot` (UV access for the walkers' delimit test).
    pub fn float2(&self, layer: usize, slot: u32) -> Option<[f32; 2]> {
        match &self.layers.get(layer)?.data {
            AttrData::Float2(c) => c.get(slot as usize).copied(),
            _ => None,
        }
    }
}

/// Flags `BM_elem_attrs_copy` leaves alone on the destination.
const VERT_EDGE_KEEP: ElemFlag = ElemFlag::SELECT;

impl BMesh {
    /// Copies vertex shape-key coordinates (`CD_SHAPEKEY`) from `src` to `dst`.
    pub(crate) fn shape_copy(&mut self, src: Option<VertId>, dst: VertId) {
        let d = dst.slot();
        for s in &mut self.shape_layers {
            if s.co.len() <= d {
                s.co.resize(d + 1, [0.0; 3]);
            }
            s.co[d] = match src {
                Some(v) => s.co.get(v.slot()).copied().unwrap_or([0.0; 3]),
                None => [0.0; 3],
            };
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc (BM_elem_attrs_copy).
    /// `BM_elem_attrs_copy` for vertices: custom data, flags (keeping `dst`'s selection) and
    /// the normal.
    pub fn vert_attrs_copy(&mut self, src: VertId, dst: VertId) {
        debug_assert_ne!(src, dst);
        self.vdata.copy_elem(src.0, dst.0);
        let (sf, sno) = {
            let s = self.vert(src);
            (s.flag, s.no)
        };
        let d = self.vert_mut(dst);
        d.flag = (d.flag & VERT_EDGE_KEEP) | (sf & !VERT_EDGE_KEEP);
        d.no = sno;
    }
    /// `BM_elem_attrs_copy` for edges.
    pub fn edge_attrs_copy(&mut self, src: EdgeId, dst: EdgeId) {
        debug_assert_ne!(src, dst);
        self.edata.copy_elem(src.0, dst.0);
        let sf = self.edge(src).flag;
        let d = self.edge_mut(dst);
        d.flag = (d.flag & VERT_EDGE_KEEP) | (sf & !VERT_EDGE_KEEP);
    }
    /// `BM_elem_attrs_copy` for faces: custom data, flags (keeping selection and UV
    /// selection), normal and material.
    pub fn face_attrs_copy(&mut self, src: FaceId, dst: FaceId) {
        debug_assert_ne!(src, dst);
        self.pdata.copy_elem(src.0, dst.0);
        let keep = ElemFlag::SELECT | ElemFlag::SELECT_UV;
        let (sf, sno, mat) = {
            let s = self.face(src);
            (s.flag, s.no, s.mat_nr)
        };
        let d = self.face_mut(dst);
        d.flag = (d.flag & keep) | (sf & !keep);
        d.no = sno;
        d.mat_nr = mat;
    }
    /// `BM_elem_attrs_copy` for loops.
    pub fn loop_attrs_copy(&mut self, src: LoopId, dst: LoopId) {
        debug_assert_ne!(src, dst);
        self.ldata.copy_elem(src.0, dst.0);
        let keep = ElemFlag::SELECT | ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE;
        let sf = self.lp(src).flag;
        let d = self.lp_mut(dst);
        d.flag = (d.flag & keep) | (sf & !keep);
    }

    /// Shape-key interpolation (`layerInterp_shapekey`) into vertex `dst`.
    fn shape_interp(&mut self, src: &[VertId], w: &[f32], dst: VertId) {
        let d = dst.slot();
        for s in &mut self.shape_layers {
            let mut co = [0.0f32; 3];
            for (v, &wt) in src.iter().zip(w) {
                let c = s.co.get(v.slot()).copied().unwrap_or([0.0; 3]);
                geom::madd_v3_v3fl(&mut co, c, wt);
            }
            if s.co.len() <= d {
                s.co.resize(d + 1, [0.0; 3]);
            }
            s.co[d] = co;
        }
    }

    /// `CustomData_bmesh_interp` on the vertex domain (custom data and shape keys).
    pub fn vert_data_interp(&mut self, src: &[VertId], w: &[f32], dst: VertId) {
        let s: Vec<u32> = src.iter().map(|v| v.0).collect();
        self.vdata.interp(&s, w, dst.0);
        if !self.shape_layers.is_empty() {
            self.shape_interp(src, w, dst);
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_interp.cc (BM_loop_interp_from_face).
    /// `BM_loop_interp_from_face`: the loop data of `l_dst` interpolated from `f_src`'s corners
    /// at `l_dst`'s position (mean-value coordinates, `do_vertex` also interpolates the vertex
    /// data).
    pub fn loop_interp_from_face(&mut self, l_dst: LoopId, f_src: FaceId, do_vertex: bool) {
        let fno = self.face(f_src).no;
        let axis = if fno != [0.0; 3] {
            fno
        } else {
            let t = self.face_calc_tangent_auto(f_src);
            geom::normalize_v3(geom::ortho_v3_v3(t)).0
        };
        let rows = geom::axis_dominant_v3_rows(axis);
        let mut cos = Vec::new();
        let mut blocks = Vec::new();
        let mut vblocks = Vec::new();
        for l in self.face_loops(f_src) {
            let v = self.lp(l).v;
            cos.push(geom::project_v2(&rows, self.vert(v).co));
            blocks.push(l.0);
            vblocks.push(v);
        }
        let v_dst = self.lp(l_dst).v;
        let co = geom::project_v2(&rows, self.vert(v_dst).co);
        let w = geom::interp_weights_poly_v2(&cos, co);
        self.ldata.interp(&blocks, &w, l_dst.0);
        if do_vertex {
            self.vert_data_interp(&vblocks, &w, v_dst);
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc
    // (BM_face_calc_tangent_auto; the triangle case uses the longest edge like n-gons).
    pub(crate) fn face_calc_tangent_auto(&self, f: FaceId) -> V3 {
        let ls = self.face_loops(f);
        let co = |l: LoopId| self.vert(self.lp(l).v).co;
        if ls.len() == 4 {
            let v: Vec<V3> = ls.iter().map(|&l| co(l)).collect();
            let mut t = geom::add(geom::sub(v[3], v[2]), geom::sub(v[0], v[1]));
            let vec = geom::add(geom::sub(v[0], v[3]), geom::sub(v[1], v[2]));
            if geom::len_squared_v3(t) < geom::len_squared_v3(vec) {
                t = vec;
            }
            return geom::normalize_v3(t).0;
        }
        let mut best = ls[0];
        let mut len_max = 0.0f32;
        for &l in &ls {
            let n = self.lp(l).next;
            let d = geom::len_squared_v3(geom::sub(co(l), co(n)));
            if d >= len_max {
                best = l;
                len_max = d;
            }
        }
        let n = self.lp(best).next;
        geom::normalize_v3(geom::sub(co(best), co(n))).0
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_interp.cc (bm_data_interp_from_elem).
    /// `BM_data_interp_from_verts`: `v_dst` data = mix of `v_src1`/`v_src2` by `fac`.
    pub fn data_interp_from_verts(
        &mut self,
        v_src1: VertId,
        v_src2: VertId,
        v_dst: VertId,
        fac: f32,
    ) {
        if fac <= 0.0 {
            if v_src1 != v_dst {
                self.vdata.copy_elem(v_src1.0, v_dst.0);
                if !self.shape_layers.is_empty() {
                    self.shape_copy(Some(v_src1), v_dst);
                }
            }
        } else if fac >= 1.0 {
            if v_src2 != v_dst {
                self.vdata.copy_elem(v_src2.0, v_dst.0);
                if !self.shape_layers.is_empty() {
                    self.shape_copy(Some(v_src2), v_dst);
                }
            }
        } else {
            self.vert_data_interp(&[v_src1, v_src2], &[1.0 - fac, fac], v_dst);
        }
    }

    /// `BM_data_interp_from_edges`: `e_dst` data = mix of `e_src1`/`e_src2` by `fac`.
    pub fn data_interp_from_edges(
        &mut self,
        e_src1: EdgeId,
        e_src2: EdgeId,
        e_dst: EdgeId,
        fac: f32,
    ) {
        if fac <= 0.0 {
            if e_src1 != e_dst {
                self.edata.copy_elem(e_src1.0, e_dst.0);
            }
        } else if fac >= 1.0 {
            if e_src2 != e_dst {
                self.edata.copy_elem(e_src2.0, e_dst.0);
            }
        } else {
            self.edata
                .interp(&[e_src1.0, e_src2.0], &[1.0 - fac, fac], e_dst.0);
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_interp.cc
    // (BM_data_interp_face_vert_edge).
    /// `BM_data_interp_face_vert_edge`: after splitting `e` at `v` (between `v_src_1` and the
    /// other end), the loop data of `v`'s new corners mixed from the corners of the two ends.
    pub fn data_interp_face_vert_edge(&mut self, v_src_1: VertId, v: VertId, e: EdgeId, fac: f32) {
        let Some(l_first) = self.edge(e).l else {
            return;
        };
        let w = [fac, 1.0 - fac];
        let mut l_iter = l_first;
        let (mut l_v1, mut l_v, mut l_v2) = (None, None, None);
        loop {
            let li = self.lp(l_iter).clone();
            if li.v == v_src_1 {
                l_v1 = Some(l_iter);
                let lv = li.next;
                l_v = Some(lv);
                l_v2 = Some(self.lp(lv).next);
            } else if li.v == v {
                l_v1 = Some(li.next);
                l_v = Some(l_iter);
                l_v2 = Some(li.prev);
            }
            let (Some(a), Some(b), Some(c)) = (l_v1, l_v, l_v2) else {
                return;
            };
            self.ldata.interp(&[a.0, c.0], &w, b.0);
            l_iter = self.lp(l_iter).radial_next;
            if l_iter == l_first {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str, data: AttrData) -> CdLayer {
        CdLayer {
            name: name.into(),
            data,
        }
    }

    #[test]
    fn layers_sort_by_type() {
        let mut cd = CustomData::default();
        cd.add_layer(layer("b", AttrData::Bool(Arc::new(vec![]))));
        cd.add_layer(layer("uv", AttrData::Float2(Arc::new(vec![]))));
        cd.add_layer(layer("f", AttrData::Float(Arc::new(vec![]))));
        cd.add_layer(layer("uv2", AttrData::Float2(Arc::new(vec![]))));
        let names: Vec<&str> = cd.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["f", "uv", "uv2", "b"]);
    }

    #[test]
    fn interp_rules() {
        let mut cd = CustomData::default();
        cd.add_layer(layer("i", AttrData::Int(Arc::new(vec![1, 2, 0]))));
        cd.add_layer(layer(
            "b",
            AttrData::Bool(Arc::new(vec![false, true, false])),
        ));
        cd.add_layer(layer(
            "c",
            AttrData::ByteColor(Arc::new(vec![[0, 0, 0, 0], [255, 255, 255, 255], [9; 4]])),
        ));
        cd.add_layer(layer("i8", AttrData::Int8(Arc::new(vec![1, 2, 7]))));
        cd.interp(&[0, 1], &[0.5, 0.5], 2);
        match &cd.layers[cd.find("i").unwrap()].data {
            AttrData::Int(v) => assert_eq!(v[2], 2), // round(1.5) = 2
            _ => unreachable!(),
        }
        match &cd.layers[cd.find("b").unwrap()].data {
            AttrData::Bool(v) => assert!(v[2]),
            _ => unreachable!(),
        }
        match &cd.layers[cd.find("c").unwrap()].data {
            AttrData::ByteColor(v) => assert_eq!(v[2], [128; 4]),
            _ => unreachable!(),
        }
        match &cd.layers[cd.find("i8").unwrap()].data {
            AttrData::Int8(v) => assert_eq!(v[2], 7),
            _ => unreachable!(),
        }
    }

    #[test]
    fn defaults_are_bmesh_defaults() {
        let mut cd = CustomData::default();
        cd.add_layer(layer("c", AttrData::FloatColor(Arc::new(vec![]))));
        cd.add_layer(layer("q", AttrData::Quaternion(Arc::new(vec![]))));
        cd.ensure_slots(1);
        match &cd.layers[0].data {
            AttrData::FloatColor(v) => assert_eq!(v[0], [1.0; 4]),
            _ => unreachable!(),
        }
        match &cd.layers[1].data {
            AttrData::Quaternion(v) => assert_eq!(v[0], [1.0, 0.0, 0.0, 0.0]),
            _ => unreachable!(),
        }
    }
}
