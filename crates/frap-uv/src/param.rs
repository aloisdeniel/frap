// SPDX-License-Identifier: GPL-3.0-or-later
//! [`ParamMesh`]: the port of Blender's `ParamHandle` and its chart construction
//! (`uv_parametrizer_*`, `blender/source/blender/geometry/intern/uv_parametrizer.cc`).
//!
//! Life cycle, as in Blender: [`ParamMesh::new`] → [`ParamMesh::face_add`] /
//! [`ParamMesh::edge_set_seam`] → [`ParamMesh::construct_end`] (splits into charts along the
//! seams) → a solver ([`ParamMesh::lscm_begin`] / [`ParamMesh::lscm_solve`] /
//! [`ParamMesh::lscm_end`], [`ParamMesh::average`], [`ParamMesh::pack`], …) →
//! [`ParamMesh::flush`] writes every chart's UVs into the caller's corner UV array at the
//! indices given to `face_add`.
//!
//! The element storage mirrors Blender's linked lists (`nextlink`) and its special purpose hash
//! (`PHash`, which keeps every element in one list in a hash-dependent order): vertex, edge and
//! face *order* decides tie-breaks (extrema pins, convex hull starts), so it is ported as is.
//! Pointers are `u32` indices into arenas ([`NIL`] for `nullptr`). Blender's `orig_uv`
//! pointers into the caller's UV array become slots of an internal store that mirrors the
//! caller's values (flushes write both).
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).
//! ABF++ (`abf=true`) and SLIM are M8 (WP 8.1): until then `lscm_begin(abf=true)` solves with
//! the LSCM angles, which is what Blender itself does when ABF fails.

pub(crate) mod beautify;
pub mod bli;
pub(crate) mod heap;
mod hull;
mod phash;

use core::cell::Cell;

use rustc_hash::FxHashMap;

use self::bli::{V2, V3};
use self::phash::{HashLink, PHash};
use crate::pack::{PackIsland, PackParams, PinMethod, mul_v2_m2_add_v2v2};

/// Key identifying a vertex or a face (`ParamKey`: Blender passes BMesh element indices).
pub type ParamKey = u64;

/// `PARAM_KEY_MAX`.
pub const PARAM_KEY_MAX: ParamKey = u64::MAX;

/// `nullptr` of the element arenas.
pub(crate) const NIL: u32 = u32::MAX;

/// `PHandleState`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PhState {
    /// `PHANDLE_STATE_ALLOCATED`: faces and seams may be added.
    #[default]
    Allocated,
    /// `PHANDLE_STATE_CONSTRUCTED`: charts built.
    Constructed,
    /// `PHANDLE_STATE_LSCM`: between `lscm_begin` and `lscm_end`.
    Lscm,
    /// `PHANDLE_STATE_STRETCH`: between `stretch_begin` and `stretch_end`.
    Stretch,
}

/// `uv_parametrizer_construct_end` options.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConstructOpts {
    /// `fill_holes` (unwrap's `fill_holes`).
    pub fill_holes: bool,
    /// `topology_from_uvs` (charts from existing UV islands instead of seams).
    pub topology_from_uvs: bool,
}

impl ConstructOpts {
    /// Options with the given flags.
    pub fn new(fill_holes: bool, topology_from_uvs: bool) -> ConstructOpts {
        ConstructOpts {
            fill_holes,
            topology_from_uvs,
        }
    }
}

/// Changed / failed chart counts of a solve (`count_changed`, `count_failed`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SolveCount {
    /// Charts solved.
    pub changed: u32,
    /// Charts whose solve failed (Blender keeps their UVs).
    pub failed: u32,
}

/// One face handed to [`ParamMesh::face_add`] (`uv_parametrizer_face_add`'s arguments).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ParamFace<'a> {
    /// Face key.
    pub key: ParamKey,
    /// Vertex keys, in corner order.
    pub vkeys: &'a [ParamKey],
    /// Vertex coordinates per corner.
    pub co: &'a [[f32; 3]],
    /// Index into the caller's corner UV array per corner (where `flush` writes).
    pub uv: &'a [u32],
    /// Initial UV per corner (read for pins and `topology_from_uvs`).
    pub uv_init: &'a [[f32; 2]],
    /// Optional per-corner weight (SLIM `weight_influence`).
    pub weight: Option<&'a [f32]>,
    /// Pinned per corner.
    pub pin: &'a [bool],
    /// Selected per corner.
    pub select: &'a [bool],
}

impl<'a> ParamFace<'a> {
    /// A face with all per-corner arrays (`weight` unset).
    pub fn new(
        key: ParamKey,
        vkeys: &'a [ParamKey],
        co: &'a [[f32; 3]],
        uv: &'a [u32],
        uv_init: &'a [[f32; 2]],
        pin: &'a [bool],
        select: &'a [bool],
    ) -> ParamFace<'a> {
        ParamFace {
            key,
            vkeys,
            co,
            uv,
            uv_init,
            weight: None,
            pin,
            select,
        }
    }
}

// `PVertFlag`.
pub(crate) const PVERT_PIN: u32 = 1;
pub(crate) const PVERT_SELECT: u32 = 2;
pub(crate) const PVERT_SPLIT: u32 = 16;
// `PEdgeFlag`.
pub(crate) const PEDGE_SEAM: u32 = 1;
pub(crate) const PEDGE_VERTEX_SPLIT: u32 = 2;
pub(crate) const PEDGE_PIN: u32 = 4;
pub(crate) const PEDGE_SELECT: u32 = 8;
pub(crate) const PEDGE_DONE: u32 = 16;
pub(crate) const PEDGE_FILLED: u32 = 32;
/// `PEDGE_VERTEX_FLAGS`: flags that follow the vertex when a face is flipped.
const PEDGE_VERTEX_FLAGS: u32 = PEDGE_PIN;
// `PFaceFlag`.
pub(crate) const PFACE_CONNECTED: u32 = 1;
pub(crate) const PFACE_FILLED: u32 = 2;

/// `PVert`.
#[derive(Clone, Debug)]
pub(crate) struct PVert {
    pub(crate) nextlink: u32,
    /// `u.key` (construction).
    pub(crate) key: ParamKey,
    /// `u.id` (matrix index).
    pub(crate) id: i32,
    pub(crate) edge: u32,
    pub(crate) co: V3,
    pub(crate) uv: V2,
    pub(crate) flag: u32,
    /// Read by SLIM (`weight_influence`, M8, WP 8.1).
    #[allow(dead_code)]
    pub(crate) weight: f32,
}

/// `PEdge`.
#[derive(Clone, Debug)]
pub(crate) struct PEdge {
    pub(crate) nextlink: u32,
    /// `u.key` (construction).
    pub(crate) key: ParamKey,
    /// `u.heaplink` (fill holes).
    pub(crate) heaplink: Option<heap::NodeId>,
    pub(crate) vert: u32,
    pub(crate) pair: u32,
    pub(crate) next: u32,
    pub(crate) face: u32,
    /// `orig_uv`: slot of the UV store ([`NIL`] for fill faces).
    pub(crate) orig_uv: u32,
    pub(crate) old_uv: V2,
    pub(crate) flag: u32,
}

/// `PFace`.
#[derive(Clone, Debug)]
pub(crate) struct PFace {
    pub(crate) nextlink: u32,
    /// `u.key` (construction).
    pub(crate) key: ParamKey,
    /// `u.chart` (construction splitting).
    pub(crate) chart: i32,
    pub(crate) edge: u32,
    pub(crate) flag: u32,
}

#[allow(clippy::misnamed_getters)]
impl HashLink for PVert {
    fn next(&self) -> u32 {
        self.nextlink
    }
    fn set_next(&mut self, n: u32) {
        self.nextlink = n;
    }
    fn key(&self) -> ParamKey {
        self.key
    }
}
#[allow(clippy::misnamed_getters)]
impl HashLink for PEdge {
    fn next(&self) -> u32 {
        self.nextlink
    }
    fn set_next(&mut self, n: u32) {
        self.nextlink = n;
    }
    fn key(&self) -> ParamKey {
        self.key
    }
}
#[allow(clippy::misnamed_getters)]
impl HashLink for PFace {
    fn next(&self) -> u32 {
        self.nextlink
    }
    fn set_next(&mut self, n: u32) {
        self.nextlink = n;
    }
    fn key(&self) -> ParamKey {
        self.key
    }
}

/// `PChartOrigBounds`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct OrigBounds {
    pub(crate) min: V2,
    pub(crate) max: V2,
    pub(crate) angle: f32,
}

/// `PChart`.
#[derive(Debug)]
pub(crate) struct PChart {
    pub(crate) verts: u32,
    pub(crate) edges: u32,
    pub(crate) faces: u32,
    pub(crate) nverts: i32,
    pub(crate) nedges: i32,
    pub(crate) nfaces: i32,
    pub(crate) nboundaries: i32,
    pub(crate) area_uv: f32,
    pub(crate) area_3d: f32,
    pub(crate) origin: V2,
    pub(crate) orig_bounds: Option<OrigBounds>,
    pub(crate) context: Option<crate::lscm::LinearSolver>,
    pub(crate) abf_alpha: Option<Vec<f32>>,
    pub(crate) pin1: u32,
    pub(crate) pin2: u32,
    pub(crate) single_pin: u32,
    pub(crate) has_pins: bool,
    pub(crate) skip_flush: bool,
}

impl Default for PChart {
    fn default() -> Self {
        PChart {
            verts: NIL,
            edges: NIL,
            faces: NIL,
            nverts: 0,
            nedges: 0,
            nfaces: 0,
            nboundaries: 0,
            area_uv: 0.0,
            area_3d: 0.0,
            origin: [0.0; 2],
            orig_bounds: None,
            context: None,
            abf_alpha: None,
            pin1: NIL,
            pin2: NIL,
            single_pin: NIL,
            has_pins: false,
            skip_flush: false,
        }
    }
}

/// `PHASH_edge(v1, v2)`.
#[inline]
fn phash_edge(v1: ParamKey, v2: ParamKey) -> ParamKey {
    if v1 < v2 {
        v1.wrapping_mul(39) ^ v2.wrapping_mul(31)
    } else {
        v1.wrapping_mul(31) ^ v2.wrapping_mul(39)
    }
}

/// `GeoUVPinIndex`.
#[derive(Clone, Debug)]
struct PinIndex {
    uv: V2,
    reindex: ParamKey,
}

/// Blender's `ParamHandle`.
#[derive(Debug)]
pub struct ParamMesh {
    /// Construction state.
    pub state: PhState,
    /// `aspect_y` (`uv_parametrizer_aspect_ratio`).
    pub aspect_y: f32,
    pub(crate) verts: Vec<PVert>,
    pub(crate) edges: Vec<PEdge>,
    pub(crate) faces: Vec<PFace>,
    construction: PChart,
    hash_verts: PHash,
    hash_edges: PHash,
    hash_faces: PHash,
    pin_hash: Option<FxHashMap<i32, Vec<PinIndex>>>,
    unique_pin_count: u64,
    pub(crate) charts: Vec<PChart>,
    /// `blend` (stretch).
    pub(crate) blend: f32,
    /// The `orig_uv` targets: current value per slot (Blender reads them through the pointer).
    uv_store: Vec<Cell<V2>>,
    /// Caller corner index per slot.
    uv_target: Vec<u32>,
    uv_slot_of: FxHashMap<u32, u32>,
}

impl Default for ParamMesh {
    fn default() -> Self {
        ParamMesh::new()
    }
}

impl ParamMesh {
    /// `ParamHandle::ParamHandle()`.
    pub fn new() -> ParamMesh {
        ParamMesh {
            state: PhState::Allocated,
            aspect_y: 1.0,
            verts: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            construction: PChart::default(),
            hash_verts: PHash::new(1),
            hash_edges: PHash::new(1),
            hash_faces: PHash::new(1),
            pin_hash: None,
            unique_pin_count: 0,
            charts: Vec::new(),
            blend: 0.0,
            uv_store: Vec::new(),
            uv_target: Vec::new(),
            uv_slot_of: FxHashMap::default(),
        }
    }

    /// `uv_parametrizer_aspect_ratio`.
    pub fn set_aspect_ratio(&mut self, aspect_y: f32) {
        debug_assert!(aspect_y > 0.0);
        self.aspect_y = aspect_y;
    }

    // ------------------------------------------------------------------ pin indices

    /// `uv_prepare_pin_index`: registers a pinned UV of the vertex `bmvertindex`, so pinned
    /// corners of one vertex with different UVs get distinct keys.
    pub fn prepare_pin_index(&mut self, bmvertindex: i32, uv: [f32; 2]) {
        let count = &mut self.unique_pin_count;
        let list = self
            .pin_hash
            .get_or_insert_with(FxHashMap::default)
            .entry(bmvertindex)
            .or_default();
        if list.iter().any(|p| p.uv == uv) {
            return;
        }
        let reindex = PARAM_KEY_MAX - *count;
        *count += 1;
        list.push(PinIndex { uv, reindex });
    }

    /// `uv_find_pin_index`: the key of the vertex `bmvertindex` for a corner with UV `uv`.
    pub fn find_pin_index(&self, bmvertindex: i32, uv: [f32; 2]) -> ParamKey {
        let Some(hash) = &self.pin_hash else {
            return bmvertindex as ParamKey;
        };
        let Some(list) = hash.get(&bmvertindex) else {
            return bmvertindex as ParamKey;
        };
        let mut best = bli::len_squared_v2v2(list[0].uv, uv);
        let mut key = list[0].reindex;
        for p in &list[1..] {
            let d = bli::len_squared_v2v2(p.uv, uv);
            if best > d {
                best = d;
                key = p.reindex;
            }
        }
        key
    }

    // ------------------------------------------------------------------ UV store

    fn uv_slot(&mut self, caller: u32, init: V2) -> u32 {
        if let Some(&s) = self.uv_slot_of.get(&caller) {
            return s;
        }
        let s = self.uv_store.len() as u32;
        self.uv_store.push(Cell::new(init));
        self.uv_target.push(caller);
        self.uv_slot_of.insert(caller, s);
        s
    }

    #[inline]
    pub(crate) fn orig_uv(&self, slot: u32) -> V2 {
        self.uv_store[slot as usize].get()
    }

    fn write_orig_uv(&self, slot: u32, uv: V2, out: &mut [[f32; 2]]) {
        self.uv_store[slot as usize].set(uv);
        if let Some(dst) = out.get_mut(self.uv_target[slot as usize] as usize) {
            *dst = uv;
        }
    }

    // ------------------------------------------------------------------ topology helpers

    /// `p_wheel_edge_next`.
    #[inline]
    pub(crate) fn wheel_edge_next(&self, e: u32) -> u32 {
        let n = self.edges[e as usize].next;
        let nn = self.edges[n as usize].next;
        self.edges[nn as usize].pair
    }

    /// `p_wheel_edge_prev`.
    #[inline]
    pub(crate) fn wheel_edge_prev(&self, e: u32) -> u32 {
        let pair = self.edges[e as usize].pair;
        if pair == NIL {
            NIL
        } else {
            self.edges[pair as usize].next
        }
    }

    /// `p_boundary_edge_next`.
    #[inline]
    pub(crate) fn boundary_edge_next(&self, e: u32) -> u32 {
        let n = self.edges[e as usize].next;
        let v = self.edges[n as usize].vert;
        self.verts[v as usize].edge
    }

    /// `p_boundary_edge_prev`.
    pub(crate) fn boundary_edge_prev(&self, e: u32) -> u32 {
        let mut we = e;
        let mut last;
        loop {
            last = we;
            we = self.wheel_edge_next(we);
            if we == NIL || we == e {
                break;
            }
        }
        let n = self.edges[last as usize].next;
        self.edges[n as usize].next
    }

    /// The three vertices of a face (`f->edge`, `->next`, `->next->next`).
    #[inline]
    pub(crate) fn face_verts(&self, f: u32) -> [u32; 3] {
        let e1 = self.faces[f as usize].edge;
        let e2 = self.edges[e1 as usize].next;
        let e3 = self.edges[e2 as usize].next;
        [
            self.edges[e1 as usize].vert,
            self.edges[e2 as usize].vert,
            self.edges[e3 as usize].vert,
        ]
    }

    /// The three edges of a face.
    #[inline]
    pub(crate) fn face_edges(&self, f: u32) -> [u32; 3] {
        let e1 = self.faces[f as usize].edge;
        let e2 = self.edges[e1 as usize].next;
        let e3 = self.edges[e2 as usize].next;
        [e1, e2, e3]
    }

    /// `p_edge_length`.
    #[inline]
    pub(crate) fn edge_length(&self, e: u32) -> f32 {
        let ed = &self.edges[e as usize];
        let v1 = self.verts[ed.vert as usize].co;
        let n = self.edges[ed.next as usize].vert;
        bli::len_v3v3(v1, self.verts[n as usize].co)
    }

    /// `p_face_area`.
    pub(crate) fn face_area(&self, f: u32) -> f32 {
        let [a, b, c] = self.face_verts(f);
        bli::area_tri_v3(
            self.verts[a as usize].co,
            self.verts[b as usize].co,
            self.verts[c as usize].co,
        )
    }

    /// `p_face_uv_area_signed`.
    pub(crate) fn face_uv_area_signed(&self, f: u32) -> f32 {
        let [a, b, c] = self.face_verts(f);
        let (v1, v2, v3) = (
            self.verts[a as usize].uv,
            self.verts[b as usize].uv,
            self.verts[c as usize].uv,
        );
        0.5 * (((v2[0] - v1[0]) * (v3[1] - v1[1])) - ((v3[0] - v1[0]) * (v2[1] - v1[1])))
    }

    /// Iterates a `nextlink` list of vertices.
    pub(crate) fn chart_vert_list(&self, c: usize) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.charts[c].nverts.max(0) as usize);
        let mut v = self.charts[c].verts;
        while v != NIL {
            out.push(v);
            v = self.verts[v as usize].nextlink;
        }
        out
    }

    /// Iterates a `nextlink` list of faces.
    pub(crate) fn chart_face_list(&self, c: usize) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.charts[c].nfaces.max(0) as usize);
        let mut f = self.charts[c].faces;
        while f != NIL {
            out.push(f);
            f = self.faces[f as usize].nextlink;
        }
        out
    }

    /// Iterates a `nextlink` list of edges.
    pub(crate) fn chart_edge_list(&self, c: usize) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.charts[c].nedges.max(0) as usize);
        let mut e = self.charts[c].edges;
        while e != NIL {
            out.push(e);
            e = self.edges[e as usize].nextlink;
        }
        out
    }

    /// `p_chart_uv_bbox`.
    pub(crate) fn chart_uv_bbox(&self, c: usize) -> (V2, V2) {
        let mut min = [f32::MAX; 2];
        let mut max = [-f32::MAX; 2];
        let mut v = self.charts[c].verts;
        while v != NIL {
            let uv = self.verts[v as usize].uv;
            for k in 0..2 {
                if uv[k] < min[k] {
                    min[k] = uv[k];
                }
                if uv[k] > max[k] {
                    max[k] = uv[k];
                }
            }
            v = self.verts[v as usize].nextlink;
        }
        (min, max)
    }

    /// `p_chart_uv_area`.
    pub(crate) fn chart_uv_area(&self, c: usize) -> f32 {
        let mut area = 0.0f32;
        let mut f = self.charts[c].faces;
        while f != NIL {
            area += self.face_uv_area_signed(f).abs();
            f = self.faces[f as usize].nextlink;
        }
        area
    }

    /// Applies `op` to every vertex UV of chart `c` in list order.
    pub(crate) fn chart_uv_map(&mut self, c: usize, mut op: impl FnMut(V2) -> V2) {
        let mut v = self.charts[c].verts;
        while v != NIL {
            let vert = &mut self.verts[v as usize];
            vert.uv = op(vert.uv);
            v = vert.nextlink;
        }
    }

    /// `p_chart_uv_scale`.
    pub(crate) fn chart_uv_scale(&mut self, c: usize, scale: f32) {
        if scale == 1.0 {
            return;
        }
        self.chart_uv_map(c, |uv| [uv[0] * scale, uv[1] * scale]);
    }

    /// `p_chart_uv_translate`.
    pub(crate) fn chart_uv_translate(&mut self, c: usize, t: V2) {
        self.chart_uv_map(c, |uv| [uv[0] + t[0], uv[1] + t[1]]);
    }

    /// `p_chart_uv_transform`.
    pub(crate) fn chart_uv_transform(&mut self, c: usize, m: &bli::M2) {
        self.chart_uv_map(c, |uv| bli::mul_m2_v2(m, uv));
    }

    /// `p_chart_uv_rotate` (around `chart.origin`).
    pub(crate) fn chart_uv_rotate(&mut self, c: usize, angle: f32) {
        if angle == 0.0 {
            return;
        }
        let ca = libm::cosf(angle);
        let sa = libm::sinf(angle);
        let o = self.charts[c].origin;
        self.chart_uv_map(c, |uv| {
            let x = uv[0] - o[0];
            let y = uv[1] - o[1];
            [ca * x - sa * y + o[0], sa * x + ca * y + o[1]]
        });
    }

    /// `uv_parametrizer_scale_x`.
    fn scale_x(&mut self, scale_x: f32) {
        if scale_x == 1.0 {
            return;
        }
        for c in 0..self.charts.len() {
            self.chart_uv_map(c, |uv| [uv[0] * scale_x, uv[1]]);
        }
    }

    // ------------------------------------------------------------------ construction

    /// `p_vert_add`.
    fn vert_add(&mut self, key: ParamKey, co: V3, weight: f32, e: u32) -> u32 {
        let mut co = co;
        for c in &mut co {
            if !c.is_finite() {
                *c = 0.0;
            }
        }
        let v = self.verts.len() as u32;
        self.verts.push(PVert {
            nextlink: NIL,
            key,
            id: 0,
            edge: e,
            co,
            uv: [0.0; 2],
            flag: 0,
            weight,
        });
        self.hash_verts
            .insert(&mut self.verts, &mut self.construction.verts, v);
        v
    }

    /// `p_vert_lookup`.
    fn vert_lookup(&mut self, key: ParamKey, co: V3, weight: f32, e: u32) -> u32 {
        let v = self.hash_verts.lookup(&self.verts, key);
        if v != NIL {
            return v;
        }
        self.vert_add(key, co, weight, e)
    }

    /// `p_vert_copy`.
    fn vert_copy(&mut self, v: u32) -> u32 {
        let mut nv = self.verts[v as usize].clone();
        nv.nextlink = NIL;
        let id = self.verts.len() as u32;
        self.verts.push(nv);
        id
    }

    /// `p_edge_lookup`.
    fn edge_lookup(&self, vkeys: [ParamKey; 2]) -> u32 {
        let key = phash_edge(vkeys[0], vkeys[1]);
        let mut e = self.hash_edges.lookup(&self.edges, key);
        while e != NIL {
            let k0 = self.verts[self.edges[e as usize].vert as usize].key;
            let n = self.edges[e as usize].next;
            let k1 = self.verts[self.edges[n as usize].vert as usize].key;
            if (k0 == vkeys[0] && k1 == vkeys[1]) || (k0 == vkeys[1] && k1 == vkeys[0]) {
                return e;
            }
            e = self.hash_edges.next(&self.edges, key, e);
        }
        NIL
    }

    /// `p_face_exists`.
    fn face_exists(&self, vkeys: &[ParamKey], i1: usize, i2: usize, i3: usize) -> bool {
        let key = phash_edge(vkeys[i1], vkeys[i2]);
        let mut e = self.hash_edges.lookup(&self.edges, key);
        while e != NIL {
            let ed = &self.edges[e as usize];
            let k0 = self.verts[ed.vert as usize].key;
            let n = &self.edges[ed.next as usize];
            let k1 = self.verts[n.vert as usize].key;
            let k2 = self.verts[self.edges[n.next as usize].vert as usize].key;
            if k0 == vkeys[i1] && k1 == vkeys[i2] {
                if k2 == vkeys[i3] {
                    return true;
                }
            } else if k0 == vkeys[i2] && k1 == vkeys[i1] && k2 == vkeys[i3] {
                return true;
            }
            e = self.hash_edges.next(&self.edges, key, e);
        }
        false
    }

    /// `p_face_add`: a face with three linked edges (vertices unset).
    fn face_alloc(&mut self) -> u32 {
        let f = self.faces.len() as u32;
        let e1 = self.edges.len() as u32;
        let (e2, e3) = (e1 + 1, e1 + 2);
        self.faces.push(PFace {
            nextlink: NIL,
            key: 0,
            chart: 0,
            edge: e1,
            flag: 0,
        });
        for (e, next) in [(e1, e2), (e2, e3), (e3, e1)] {
            let _ = e;
            self.edges.push(PEdge {
                nextlink: NIL,
                key: 0,
                heaplink: None,
                vert: NIL,
                pair: NIL,
                next,
                face: f,
                orig_uv: NIL,
                old_uv: [0.0; 2],
                flag: 0,
            });
        }
        f
    }

    /// `p_face_add_construct`.
    fn face_add_construct(&mut self, face: &ParamFace<'_>, i1: usize, i2: usize, i3: usize) -> u32 {
        let f = self.face_alloc();
        let [e1, e2, e3] = self.face_edges(f);
        let w = |i: usize| face.weight.map_or(1.0, |w| w[i]);
        for (e, i) in [(e1, i1), (e2, i2), (e3, i3)] {
            let v = self.vert_lookup(face.vkeys[i], face.co[i], w(i), e);
            self.edges[e as usize].vert = v;
        }
        for (e, i) in [(e1, i1), (e2, i2), (e3, i3)] {
            let slot = self.uv_slot(face.uv[i], face.uv_init[i]);
            let ed = &mut self.edges[e as usize];
            ed.orig_uv = slot;
            if face.pin.get(i).copied().unwrap_or(false) {
                ed.flag |= PEDGE_PIN;
            }
            if face.select.get(i).copied().unwrap_or(false) {
                ed.flag |= PEDGE_SELECT;
            }
        }
        self.faces[f as usize].key = face.key;
        self.hash_faces
            .insert(&mut self.faces, &mut self.construction.faces, f);

        self.edges[e1 as usize].key = phash_edge(face.vkeys[i1], face.vkeys[i2]);
        self.edges[e2 as usize].key = phash_edge(face.vkeys[i2], face.vkeys[i3]);
        self.edges[e3 as usize].key = phash_edge(face.vkeys[i3], face.vkeys[i1]);
        for e in [e1, e2, e3] {
            self.hash_edges
                .insert(&mut self.edges, &mut self.construction.edges, e);
        }
        f
    }

    /// `p_add_ngon`: triangulates with `BLI_polyfill_calc` (flipped Newell normal) + beautify.
    fn add_ngon(&mut self, face: &ParamFace<'_>) {
        let n = face.vkeys.len();
        let mut normal = [0.0f32; 3];
        let mut co_prev = face.co[n - 1];
        for &co_curr in face.co {
            bli::add_newell_cross(&mut normal, co_prev, co_curr);
            co_prev = co_curr;
        }
        let (mut normal, len) = bli::normalize3(normal);
        if len == 0.0 {
            normal[2] = 1.0;
        }
        let axis_mat = bli::axis_dominant_v3_to_m3_negate(normal);
        let projverts: Vec<V2> = face
            .co
            .iter()
            .map(|&c| bli::mul_v2_m3v3(&axis_mat, c))
            .collect();
        let mut tris = frap_geom::polyfill::polyfill_calc(&projverts, 1);
        beautify::polyfill_beautify(&projverts, &mut tris);
        for t in tris {
            let idx = [t[0] as usize, t[1] as usize, t[2] as usize];
            let vkeys = idx.map(|i| face.vkeys[i]);
            let co = idx.map(|i| face.co[i]);
            let uv = idx.map(|i| face.uv[i]);
            let uv_init = idx.map(|i| face.uv_init[i]);
            let weight = face.weight.map(|w| idx.map(|i| w[i]));
            let pin = idx.map(|i| face.pin.get(i).copied().unwrap_or(false));
            let select = idx.map(|i| face.select.get(i).copied().unwrap_or(false));
            let tri = ParamFace {
                key: face.key,
                vkeys: &vkeys,
                co: &co,
                uv: &uv,
                uv_init: &uv_init,
                weight: weight.as_ref().map(|w| &w[..]),
                pin: &pin,
                select: &select,
            };
            self.face_add(&tri);
        }
    }

    /// `uv_parametrizer_face_add` (n-gons are triangulated with Blender's polyfill + beautify).
    pub fn face_add(&mut self, face: &ParamFace<'_>) {
        let nverts = face.vkeys.len();
        assert!(
            nverts >= 3,
            "uv_parametrizer_face_add: a face needs 3 corners"
        );
        assert_eq!(self.state, PhState::Allocated);
        if nverts > 3 {
            // Protect against (manifold) geometry with a non-manifold triangulation (#102543).
            let mut permute: Vec<usize> = (0..nverts).collect();
            let mut i = nverts as isize - 1;
            while i >= 0 {
                let pm = permute.len();
                let iu = i as usize;
                let i0 = permute[iu];
                let i1 = permute[(iu + 1) % pm];
                let i2 = permute[(iu + 2) % pm];
                if !self.face_exists(face.vkeys, i0, i1, i2) {
                    i -= 1;
                    continue;
                }
                let iprev = permute[(iu + pm - 1) % pm];
                self.face_add_construct(face, iprev, i0, i1);
                permute.remove(iu);
                if permute.len() == 3 {
                    break;
                }
                i -= 1;
            }
            if permute.len() != nverts {
                let vkeys: Vec<ParamKey> = permute.iter().map(|&j| face.vkeys[j]).collect();
                let co: Vec<V3> = permute.iter().map(|&j| face.co[j]).collect();
                let uv: Vec<u32> = permute.iter().map(|&j| face.uv[j]).collect();
                let uv_init: Vec<V2> = permute.iter().map(|&j| face.uv_init[j]).collect();
                let weight: Option<Vec<f32>> =
                    face.weight.map(|w| permute.iter().map(|&j| w[j]).collect());
                let pin: Vec<bool> = permute
                    .iter()
                    .map(|&j| face.pin.get(j).copied().unwrap_or(false))
                    .collect();
                let select: Vec<bool> = permute
                    .iter()
                    .map(|&j| face.select.get(j).copied().unwrap_or(false))
                    .collect();
                let sub = ParamFace {
                    key: face.key,
                    vkeys: &vkeys,
                    co: &co,
                    uv: &uv,
                    uv_init: &uv_init,
                    weight: weight.as_deref(),
                    pin: &pin,
                    select: &select,
                };
                self.add_ngon(&sub);
                return;
            }
        }
        if nverts > 3 {
            self.add_ngon(face);
        } else if !self.face_exists(face.vkeys, 0, 1, 2) {
            self.face_add_construct(face, 0, 1, 2);
        }
    }

    /// `uv_parametrizer_edge_set_seam`.
    pub fn edge_set_seam(&mut self, vkeys: [ParamKey; 2]) {
        assert_eq!(self.state, PhState::Allocated);
        let e = self.edge_lookup(vkeys);
        if e != NIL {
            self.edges[e as usize].flag |= PEDGE_SEAM;
        }
    }

    /// `p_face_flip`.
    fn face_flip(&mut self, f: u32) {
        let [e1, e2, e3] = self.face_edges(f);
        let (v1, v2, v3) = (
            self.edges[e1 as usize].vert,
            self.edges[e2 as usize].vert,
            self.edges[e3 as usize].vert,
        );
        let (f1, f2, f3) = (
            self.edges[e1 as usize].flag,
            self.edges[e2 as usize].flag,
            self.edges[e3 as usize].flag,
        );
        let (o1, o2, o3) = (
            self.edges[e1 as usize].orig_uv,
            self.edges[e2 as usize].orig_uv,
            self.edges[e3 as usize].orig_uv,
        );
        {
            let e = &mut self.edges[e1 as usize];
            e.vert = v2;
            e.next = e3;
            e.orig_uv = o2;
            e.flag = (f1 & !PEDGE_VERTEX_FLAGS) | (f2 & PEDGE_VERTEX_FLAGS);
        }
        {
            let e = &mut self.edges[e2 as usize];
            e.vert = v3;
            e.next = e1;
            e.orig_uv = o3;
            e.flag = (f2 & !PEDGE_VERTEX_FLAGS) | (f3 & PEDGE_VERTEX_FLAGS);
        }
        {
            let e = &mut self.edges[e3 as usize];
            e.vert = v1;
            e.next = e2;
            e.orig_uv = o1;
            e.flag = (f3 & !PEDGE_VERTEX_FLAGS) | (f1 & PEDGE_VERTEX_FLAGS);
        }
    }

    /// `p_edge_implicit_seam`.
    fn edge_implicit_seam(&mut self, e: u32, ep: u32) -> bool {
        let limit = 0.00001f32;
        let ed = &self.edges[e as usize];
        let uv1 = self.orig_uv(ed.orig_uv);
        let uv2 = self.orig_uv(self.edges[ed.next as usize].orig_uv);
        let epd = &self.edges[ep as usize];
        let (uvp1, uvp2) = if self.verts[ed.vert as usize].key == self.verts[epd.vert as usize].key
        {
            (
                self.orig_uv(epd.orig_uv),
                self.orig_uv(self.edges[epd.next as usize].orig_uv),
            )
        } else {
            (
                self.orig_uv(self.edges[epd.next as usize].orig_uv),
                self.orig_uv(epd.orig_uv),
            )
        };
        if (uv1[0] - uvp1[0]).abs() > limit
            || (uv1[1] - uvp1[1]).abs() > limit
            || (uv2[0] - uvp2[0]).abs() > limit
            || (uv2[1] - uvp2[1]).abs() > limit
        {
            self.edges[e as usize].flag |= PEDGE_SEAM;
            self.edges[ep as usize].flag |= PEDGE_SEAM;
            return true;
        }
        false
    }

    /// `p_edge_has_pair`.
    fn edge_has_pair(&mut self, e: u32, topology_from_uvs: bool) -> u32 {
        if self.edges[e as usize].flag & PEDGE_SEAM != 0 {
            return NIL;
        }
        let key1 = self.verts[self.edges[e as usize].vert as usize].key;
        let n = self.edges[e as usize].next;
        let key2 = self.verts[self.edges[n as usize].vert as usize].key;
        let key = phash_edge(key1, key2);
        let mut pe = self.hash_edges.lookup(&self.edges, key);
        let mut pair = NIL;
        while pe != NIL {
            if pe != e {
                let k1 = self.verts[self.edges[pe as usize].vert as usize].key;
                let pn = self.edges[pe as usize].next;
                let k2 = self.verts[self.edges[pn as usize].vert as usize].key;
                if (k1 == key1 && k2 == key2) || (k1 == key2 && k2 == key1) {
                    // Don't connect seams and t-junctions.
                    if self.edges[pe as usize].flag & PEDGE_SEAM != 0
                        || pair != NIL
                        || (topology_from_uvs && self.edge_implicit_seam(e, pe))
                    {
                        return NIL;
                    }
                    pair = pe;
                }
            }
            pe = self.hash_edges.next(&self.edges, key, pe);
        }
        if pair != NIL && self.edges[e as usize].vert == self.edges[pair as usize].vert {
            let pn = self.edges[pair as usize].next;
            let pnn = self.edges[pn as usize].next;
            if self.edges[pn as usize].pair != NIL || self.edges[pnn as usize].pair != NIL {
                // Non unfoldable, maybe a mobius ring or a klein bottle.
                return NIL;
            }
        }
        pair
    }

    /// `p_edge_connect_pair`.
    fn edge_connect_pair(&mut self, e: u32, topology_from_uvs: bool, stack: &mut Vec<u32>) -> bool {
        if self.edges[e as usize].pair == NIL {
            let pair = self.edge_has_pair(e, topology_from_uvs);
            if pair != NIL {
                if self.edges[e as usize].vert == self.edges[pair as usize].vert {
                    let pf = self.edges[pair as usize].face;
                    self.face_flip(pf);
                }
                self.edges[e as usize].pair = pair;
                self.edges[pair as usize].pair = e;
                let pf = self.edges[pair as usize].face;
                if self.faces[pf as usize].flag & PFACE_CONNECTED == 0 {
                    stack.push(pair);
                }
            }
        }
        self.edges[e as usize].pair != NIL
    }

    /// `p_connect_pairs`: returns the number of charts.
    fn connect_pairs(&mut self, topology_from_uvs: bool) -> i32 {
        let mut stack: Vec<u32> = Vec::new();
        let mut ncharts = 0i32;
        let mut first = self.construction.faces;
        while first != NIL {
            frap_math::budget::check();
            if self.faces[first as usize].flag & PFACE_CONNECTED != 0 {
                first = self.faces[first as usize].nextlink;
                continue;
            }
            stack.push(self.faces[first as usize].edge);
            while let Some(e) = stack.pop() {
                let e1 = self.edges[e as usize].next;
                let e2 = self.edges[e1 as usize].next;
                let f = self.edges[e as usize].face;
                self.faces[f as usize].flag |= PFACE_CONNECTED;
                self.faces[f as usize].chart = ncharts;
                for x in [e, e1, e2] {
                    if !self.edge_connect_pair(x, topology_from_uvs, &mut stack) {
                        let v = self.edges[x as usize].vert;
                        self.verts[v as usize].edge = x;
                    }
                }
            }
            ncharts += 1;
            first = self.faces[first as usize].nextlink;
        }
        ncharts
    }

    /// `p_split_vert`.
    fn split_vert(&mut self, c: usize, e: u32) {
        let mut v = self.edges[e as usize].vert;
        let mut copy = true;
        if self.edges[e as usize].flag & PEDGE_PIN != 0 {
            self.charts[c].has_pins = true;
        }
        if self.edges[e as usize].flag & PEDGE_VERTEX_SPLIT != 0 {
            return;
        }
        // Rewind to start.
        let mut lastwe = e;
        let mut we = self.wheel_edge_prev(e);
        while we != NIL && we != e {
            lastwe = we;
            we = self.wheel_edge_prev(we);
        }
        // Go over all edges in the wheel.
        let mut we = lastwe;
        while we != NIL {
            if self.edges[we as usize].flag & PEDGE_VERTEX_SPLIT != 0 {
                break;
            }
            self.edges[we as usize].flag |= PEDGE_VERTEX_SPLIT;
            if we == self.verts[v as usize].edge {
                // Found it, no need to copy.
                copy = false;
                self.verts[v as usize].nextlink = self.charts[c].verts;
                self.charts[c].verts = v;
                self.charts[c].nverts += 1;
            }
            we = self.wheel_edge_next(we);
        }
        if copy {
            self.verts[v as usize].flag |= PVERT_SPLIT;
            v = self.vert_copy(v);
            self.verts[v as usize].flag |= PVERT_SPLIT;
            self.verts[v as usize].nextlink = self.charts[c].verts;
            self.charts[c].verts = v;
            self.charts[c].nverts += 1;
            self.verts[v as usize].edge = lastwe;
            let mut we = lastwe;
            loop {
                self.edges[we as usize].vert = v;
                we = self.wheel_edge_next(we);
                if we == NIL || we == lastwe {
                    break;
                }
            }
        }
    }

    /// `p_split_charts`.
    fn split_charts(&mut self, ncharts: i32) {
        self.charts = (0..ncharts).map(|_| PChart::default()).collect();
        let mut f = self.construction.faces;
        while f != NIL {
            let [e1, e2, e3] = self.face_edges(f);
            let nextf = self.faces[f as usize].nextlink;
            let c = self.faces[f as usize].chart as usize;
            self.faces[f as usize].nextlink = self.charts[c].faces;
            self.charts[c].faces = f;
            for e in [e1, e2, e3] {
                self.edges[e as usize].nextlink = self.charts[c].edges;
                self.charts[c].edges = e;
            }
            self.charts[c].nfaces += 1;
            self.charts[c].nedges += 3;
            self.split_vert(c, e1);
            self.split_vert(c, e2);
            self.split_vert(c, e3);
            f = nextf;
        }
    }

    /// `p_chart_boundaries`: the number of boundaries and the longest one (`outer`).
    pub(crate) fn chart_boundaries(&mut self, c: usize) -> u32 {
        let mut maxlen = -1.0f32;
        let mut outer = NIL;
        self.charts[c].nboundaries = 0;
        let edges = self.chart_edge_list(c);
        for &e in &edges {
            let ed = &self.edges[e as usize];
            if ed.pair != NIL || ed.flag & PEDGE_DONE != 0 {
                continue;
            }
            self.charts[c].nboundaries += 1;
            let mut len = 0.0f32;
            let mut be = e;
            loop {
                self.edges[be as usize].flag |= PEDGE_DONE;
                len += self.edge_length(be);
                be = self.boundary_edge_next(be);
                if be == e {
                    break;
                }
            }
            if len > maxlen {
                outer = e;
                maxlen = len;
            }
        }
        for &e in &edges {
            self.edges[e as usize].flag &= !PEDGE_DONE;
        }
        outer
    }

    /// `p_edge_boundary_angle`.
    fn edge_boundary_angle(&self, e: u32) -> f32 {
        let v = self.edges[e as usize].vert;
        let vco = self.verts[v as usize].co;
        let mut angle = core::f32::consts::PI;
        let first = self.verts[v as usize].edge;
        let mut we = first;
        loop {
            let n = self.edges[we as usize].next;
            let nn = self.edges[n as usize].next;
            let v1 = self.verts[self.edges[n as usize].vert as usize].co;
            let v2 = self.verts[self.edges[nn as usize].vert as usize].co;
            angle -= bli::angle_v3v3v3(v1, vco, v2);
            we = self.edges[nn as usize].pair;
            if we == NIL || we == first {
                break;
            }
        }
        angle
    }

    /// `p_face_add_fill`.
    fn face_add_fill(&mut self, c: usize, v1: u32, v2: u32, v3: u32) -> u32 {
        let f = self.face_alloc();
        let [e1, e2, e3] = self.face_edges(f);
        self.edges[e1 as usize].vert = v1;
        self.edges[e2 as usize].vert = v2;
        self.edges[e3 as usize].vert = v3;
        self.faces[f as usize].nextlink = self.charts[c].faces;
        self.charts[c].faces = f;
        for e in [e1, e2, e3] {
            self.edges[e as usize].nextlink = self.charts[c].edges;
            self.charts[c].edges = e;
        }
        self.charts[c].nfaces += 1;
        self.charts[c].nedges += 3;
        f
    }

    /// `p_chart_fill_boundary`.
    fn chart_fill_boundary(&mut self, c: usize, be: u32, mut nedges: i32) {
        let mut heap: heap::Heap<u32> = heap::Heap::new();
        let mut e = be;
        loop {
            let angle = self.edge_boundary_angle(e);
            self.edges[e as usize].heaplink = Some(heap.insert(angle, e));
            e = self.boundary_edge_next(e);
            if e == be {
                break;
            }
        }
        if nedges == 2 {
            // No real boundary, but an isolated seam.
            let n = self.edges[be as usize].next;
            let e = self.verts[self.edges[n as usize].vert as usize].edge;
            self.edges[e as usize].pair = be;
            self.edges[be as usize].pair = e;
            if let Some(h) = self.edges[e as usize].heaplink {
                heap.remove(h);
            }
            if let Some(h) = self.edges[be as usize].heaplink {
                heap.remove(h);
            }
        } else {
            while nedges > 2 {
                frap_math::budget::check();
                let e = heap.pop_min();
                let e1 = self.boundary_edge_prev(e);
                let e2 = self.boundary_edge_next(e);
                if let Some(h) = self.edges[e1 as usize].heaplink {
                    heap.remove(h);
                }
                if let Some(h) = self.edges[e2 as usize].heaplink {
                    heap.remove(h);
                }
                self.edges[e as usize].heaplink = None;
                self.edges[e1 as usize].heaplink = None;
                self.edges[e2 as usize].heaplink = None;
                self.edges[e as usize].flag |= PEDGE_FILLED;
                self.edges[e1 as usize].flag |= PEDGE_FILLED;

                let (ev, e1v, e2v) = (
                    self.edges[e as usize].vert,
                    self.edges[e1 as usize].vert,
                    self.edges[e2 as usize].vert,
                );
                let f = self.face_add_fill(c, ev, e1v, e2v);
                self.faces[f as usize].flag |= PFACE_FILLED;
                let fe = self.faces[f as usize].edge;
                let ne1 = fe;
                let ne2 = self.edges[fe as usize].next;
                let ne = self.edges[ne2 as usize].next;
                for x in [ne, ne1, ne2] {
                    self.edges[x as usize].flag = PEDGE_FILLED;
                }
                self.edges[e as usize].pair = ne;
                self.edges[ne as usize].pair = e;
                self.edges[e1 as usize].pair = ne1;
                self.edges[ne1 as usize].pair = e1;
                self.edges[ne as usize].vert = e2v;
                self.edges[ne1 as usize].vert = ev;
                self.edges[ne2 as usize].vert = e1v;
                if nedges == 3 {
                    self.edges[e2 as usize].pair = ne2;
                    self.edges[ne2 as usize].pair = e2;
                } else {
                    self.verts[e1v as usize].edge = ne2;
                    let a = self.edge_boundary_angle(ne2);
                    self.edges[ne2 as usize].heaplink = Some(heap.insert(a, ne2));
                    let a = self.edge_boundary_angle(e2);
                    self.edges[e2 as usize].heaplink = Some(heap.insert(a, e2));
                }
                nedges -= 1;
            }
        }
        // `BLI_heap_free`: the remaining links dangle, as in Blender.
        let _ = heap;
    }

    /// `p_chart_fill_boundaries`.
    fn chart_fill_boundaries(&mut self, c: usize, outer: u32) {
        let mut e = self.charts[c].edges;
        while e != NIL {
            let ed = &self.edges[e as usize];
            if ed.pair != NIL || ed.flag & PEDGE_FILLED != 0 {
                e = ed.nextlink;
                continue;
            }
            let mut nedges = 0;
            let mut be = e;
            loop {
                self.edges[be as usize].flag |= PEDGE_FILLED;
                be = self.boundary_edge_next(be);
                nedges += 1;
                if be == e {
                    break;
                }
            }
            if e != outer {
                self.chart_fill_boundary(c, e, nedges);
            }
            e = self.edges[e as usize].nextlink;
        }
    }

    /// `p_vert_load_pin_select_uvs`.
    pub(crate) fn vert_load_pin_select_uvs(&mut self, v: u32) {
        let mut nedges = 0;
        let mut npins = 0;
        let mut uv = [0.0f32; 2];
        let mut pinuv = [0.0f32; 2];
        let first = self.verts[v as usize].edge;
        let mut e = first;
        let mut flag = self.verts[v as usize].flag;
        loop {
            let ed = &self.edges[e as usize];
            if ed.orig_uv != NIL {
                let o = self.orig_uv(ed.orig_uv);
                if ed.flag & PEDGE_SELECT != 0 {
                    flag |= PVERT_SELECT;
                }
                if ed.flag & PEDGE_PIN != 0 {
                    pinuv[0] += o[0] * self.aspect_y;
                    pinuv[1] += o[1];
                    npins += 1;
                } else {
                    uv[0] += o[0] * self.aspect_y;
                    uv[1] += o[1];
                }
                nedges += 1;
            }
            e = self.wheel_edge_next(e);
            if e == NIL || e == first {
                break;
            }
        }
        if npins > 0 {
            uv = [pinuv[0] / npins as f32, pinuv[1] / npins as f32];
            flag |= PVERT_PIN;
        } else if nedges > 0 {
            uv = [uv[0] / nedges as f32, uv[1] / nedges as f32];
        }
        let vert = &mut self.verts[v as usize];
        vert.uv = uv;
        vert.flag = flag;
    }

    /// `uv_parametrizer_construct_end`: returns the number of charts that failed
    /// (`count_fail`: closed charts without a boundary, unless `topology_from_uvs`).
    pub fn construct_end(&mut self, opts: ConstructOpts) -> u32 {
        assert_eq!(self.state, PhState::Allocated);
        let ncharts = self.connect_pairs(opts.topology_from_uvs);
        self.split_charts(ncharts);
        self.construction = PChart::default();
        self.hash_verts = PHash::new(1);
        self.hash_edges = PHash::new(1);
        self.hash_faces = PHash::new(1);

        let mut failed = 0u32;
        let charts = core::mem::take(&mut self.charts);
        let mut kept = Vec::with_capacity(charts.len());
        for chart in charts {
            self.charts.push(chart);
            let c = self.charts.len() - 1;
            let outer = self.chart_boundaries(c);
            if !opts.topology_from_uvs && self.charts[c].nboundaries == 0 {
                self.charts.pop();
                failed += 1;
                continue;
            }
            if opts.fill_holes && self.charts[c].nboundaries > 1 {
                self.chart_fill_boundaries(c, outer);
            }
            for v in self.chart_vert_list(c) {
                self.vert_load_pin_select_uvs(v);
            }
            kept.push(self.charts.pop().expect("chart"));
        }
        self.charts = kept;
        self.state = PhState::Constructed;
        failed
    }

    /// Number of charts after construction.
    pub fn chart_count(&self) -> usize {
        self.charts.len()
    }

    // ------------------------------------------------------------------ flushing

    /// `p_face_backup_uvs`.
    pub(crate) fn face_backup_uvs(&mut self, f: u32) {
        for e in self.face_edges(f) {
            let slot = self.edges[e as usize].orig_uv;
            if slot != NIL {
                self.edges[e as usize].old_uv = self.orig_uv(slot);
            }
        }
    }

    /// `p_flush_uvs`.
    fn flush_chart(&self, c: usize, out: &mut [[f32; 2]]) {
        let blend = self.blend;
        let invblend = 1.0 - blend;
        let invblend_x = invblend / self.aspect_y;
        let mut e = self.charts[c].edges;
        while e != NIL {
            let ed = &self.edges[e as usize];
            if ed.orig_uv != NIL {
                let vuv = self.verts[ed.vert as usize].uv;
                let uv = [
                    blend * ed.old_uv[0] + invblend_x * vuv[0],
                    blend * ed.old_uv[1] + invblend * vuv[1],
                ];
                self.write_orig_uv(ed.orig_uv, uv, out);
            }
            e = ed.nextlink;
        }
    }

    /// `uv_parametrizer_flush`: writes every chart's UVs into `uvs` at the indices given to
    /// [`ParamMesh::face_add`].
    pub fn flush(&self, uvs: &mut [[f32; 2]]) {
        for c in 0..self.charts.len() {
            if self.charts[c].skip_flush {
                continue;
            }
            self.flush_chart(c, uvs);
        }
    }

    /// The number of charts (not skipped by [`ParamMesh::flush`]) holding a non-finite UV.
    /// Like Blender, the solvers work in single precision and write NaN UVs without failing
    /// when the coordinates are too large (a 1e15 quad already overflows the area sums of
    /// `average`; Blender 5.2.4's `uv.unwrap` returns FINISHED with NaN UVs). Not part of
    /// Blender: a frap extra so that callers can report the NaNs instead of writing them
    /// silently.
    pub fn non_finite_charts(&self) -> usize {
        (0..self.charts.len())
            .filter(|&c| {
                !self.charts[c].skip_flush
                    && self.chart_vert_list(c).iter().any(|&v| {
                        let uv = self.verts[v as usize].uv;
                        !(uv[0].is_finite() && uv[1].is_finite())
                    })
            })
            .count()
    }

    /// `uv_parametrizer_flush_restore`: writes back the UVs read at construction.
    pub fn flush_restore(&self, uvs: &mut [[f32; 2]]) {
        for c in 0..self.charts.len() {
            let mut f = self.charts[c].faces;
            while f != NIL {
                for e in self.face_edges(f) {
                    let ed = &self.edges[e as usize];
                    if ed.orig_uv != NIL {
                        self.write_orig_uv(ed.orig_uv, ed.old_uv, uvs);
                    }
                }
                f = self.faces[f as usize].nextlink;
            }
        }
    }

    // ------------------------------------------------------------------ packing

    /// `uv_parametrizer_pack`: packs the charts with [`crate::pack::pack_islands`].
    pub fn pack(&mut self, params: &PackParams) {
        if self.charts.is_empty() {
            return;
        }
        self.scale_x(1.0 / self.aspect_y);
        let mut islands: Vec<PackIsland> = Vec::new();
        for c in 0..self.charts.len() {
            if params.pin_method == PinMethod::None && self.charts[c].has_pins {
                continue;
            }
            let mut island = PackIsland::new();
            island.caller_index = c as i32;
            island.aspect_y = self.aspect_y;
            island.pinned = self.charts[c].has_pins;
            for f in self.chart_face_list(c) {
                let [a, b, cc] = self.face_verts(f);
                island.add_triangle(
                    self.verts[a as usize].uv,
                    self.verts[b as usize].uv,
                    self.verts[cc as usize].uv,
                );
            }
            islands.push(island);
        }
        let scale = crate::pack::pack_islands(&mut islands, params);
        for island in &islands {
            let island_scale = if island.can_scale(params) { scale } else { 1.0 };
            let c = island.caller_index as usize;
            let m = island.build_transformation(island_scale, f64::from(island.angle));
            let pre = island.pre_translate;
            self.chart_uv_map(c, |uv| mul_v2_m2_add_v2v2(&m, uv, pre));
            self.chart_uv_translate(c, params.udim_base_offset);
        }
        self.scale_x(self.aspect_y);
    }

    /// `uv_parametrizer_original_bounds`: fits every fully selected chart back into the
    /// bounds and orientation it had before the solve (`use_original_bounds`).
    pub fn original_bounds(&mut self) {
        for c in 0..self.charts.len() {
            let all_selected = self
                .chart_vert_list(c)
                .iter()
                .all(|&v| self.verts[v as usize].flag & PVERT_SELECT != 0);
            if !all_selected {
                continue;
            }
            let Some(ob) = self.charts[c].orig_bounds else {
                continue;
            };
            let new_angle = self.chart_minimum_area_angle(c);
            self.chart_uv_rotate(c, ob.angle - new_angle);
            let (minv, maxv) = self.chart_uv_bbox(c);
            let new_size = [maxv[0] - minv[0], maxv[1] - minv[1]];
            let orig_size = [ob.max[0] - ob.min[0], ob.max[1] - ob.min[1]];
            let size = orig_size[0].max(orig_size[1]);
            let denom = new_size[0].max(new_size[1]);
            let scale = if denom != 0.0 { size / denom } else { 0.0 };
            self.chart_uv_scale(c, scale);
            let (minv, maxv) = self.chart_uv_bbox(c);
            let mid = [(minv[0] + maxv[0]) * 0.5, (minv[1] + maxv[1]) * 0.5];
            let center = [(ob.min[0] + ob.max[0]) * 0.5, (ob.min[1] + ob.max[1]) * 0.5];
            self.chart_uv_translate(c, [center[0] - mid[0], center[1] - mid[1]]);
        }
    }

    /// Whether chart `c` has pinned vertices (`chart->has_pins`).
    pub fn chart_has_pins(&self, c: usize) -> bool {
        self.charts[c].has_pins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit quad split in two triangles, plus a third triangle sharing an edge.
    fn quad_mesh(pm: &mut ParamMesh) {
        let co = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let keys = [0u64, 1, 2, 3];
        let uv_init = [[0.0, 0.0]; 4];
        let uv = [0u32, 1, 2, 3];
        let flags = [false; 4];
        let face = ParamFace::new(0, &keys, &co, &uv, &uv_init, &flags, &flags);
        pm.face_add(&face);
    }

    #[test]
    fn quad_is_one_chart_with_four_verts() {
        let mut pm = ParamMesh::new();
        quad_mesh(&mut pm);
        assert_eq!(pm.construct_end(ConstructOpts::default()), 0);
        assert_eq!(pm.chart_count(), 1);
        assert_eq!(pm.charts[0].nverts, 4);
        assert_eq!(pm.charts[0].nfaces, 2);
        assert_eq!(pm.charts[0].nboundaries, 1);
    }

    #[test]
    fn seam_splits_and_closed_mesh_fails() {
        // A tetrahedron is closed: one chart without a boundary fails.
        let co = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let tris = [[0usize, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        let build = |seams: &[[u64; 2]]| {
            let mut pm = ParamMesh::new();
            for (fi, t) in tris.iter().enumerate() {
                let keys = t.map(|i| i as u64);
                let c = t.map(|i| co[i]);
                let uv = [fi as u32 * 3, fi as u32 * 3 + 1, fi as u32 * 3 + 2];
                let init = [[0.0, 0.0]; 3];
                let fl = [false; 3];
                pm.face_add(&ParamFace::new(fi as u64, &keys, &c, &uv, &init, &fl, &fl));
            }
            for s in seams {
                pm.edge_set_seam(*s);
            }
            pm
        };
        let mut closed = build(&[]);
        assert_eq!(closed.construct_end(ConstructOpts::default()), 1);
        assert_eq!(closed.chart_count(), 0);
        let mut cut = build(&[[0, 3], [1, 3]]);
        assert_eq!(cut.construct_end(ConstructOpts::default()), 0);
        assert_eq!(cut.chart_count(), 1);
    }

    /// Huge coordinates: like Blender 5.2.4 (`uv.unwrap` FINISHED, every UV NaN), the unwrap
    /// does not fail, and `non_finite_charts` reports the chart.
    #[test]
    fn huge_coordinates_give_nan_uvs_and_are_reported() {
        for (s, want) in [(1.0f32, 0usize), (1e30, 1)] {
            let mut pm = ParamMesh::new();
            let co = [[0.0, 0.0, 0.0], [s, 0.0, 0.0], [s, s, 0.0], [0.0, s, 0.0]];
            let keys = [0u64, 1, 2, 3];
            let fl = [false; 4];
            let init = [[0.0, 0.0]; 4];
            pm.face_add(&ParamFace::new(
                0,
                &keys,
                &co,
                &[0, 1, 2, 3],
                &init,
                &fl,
                &fl,
            ));
            assert_eq!(pm.construct_end(ConstructOpts::default()), 0);
            pm.lscm_begin(false, false, false);
            assert_eq!(pm.lscm_solve().failed, 0);
            pm.lscm_end();
            pm.average(true, false, false);
            assert_eq!(pm.non_finite_charts(), want, "size {s}");
        }
    }

    #[test]
    fn pin_index_reuses_equal_uvs() {
        let mut pm = ParamMesh::new();
        assert_eq!(pm.find_pin_index(5, [0.0, 0.0]), 5);
        pm.prepare_pin_index(5, [0.0, 0.0]);
        pm.prepare_pin_index(5, [0.0, 0.0]);
        pm.prepare_pin_index(5, [1.0, 0.0]);
        assert_eq!(pm.find_pin_index(5, [0.1, 0.0]), PARAM_KEY_MAX);
        assert_eq!(pm.find_pin_index(5, [0.9, 0.0]), PARAM_KEY_MAX - 1);
        assert_eq!(pm.find_pin_index(6, [0.9, 0.0]), 6);
    }
}
