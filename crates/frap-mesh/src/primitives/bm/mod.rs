// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal BMesh used by the primitive generators to reproduce Blender's element creation
//! order: `BLI_mempool`-style pools (a freed slot is reused first, LIFO), `BM_vert_create`,
//! `BM_edge_create(…, BM_CREATE_NO_DOUBLE)`, `BM_face_create_verts(…, create_edges=true)`
//! (edges `(v[i-1], v[i])` in loop order), kills, and the conversion `BM_mesh_bm_to_me` performs
//! for a freshly added primitive (pool order, attributes in creation order, everything
//! selected, flat faces). The full BMesh (`frap-bmesh`, M2) depends on `frap-mesh`, so the
//! generators cannot use it.

pub(crate) mod math;
pub(crate) mod subdivide;

use std::collections::HashMap;
use std::sync::Arc;

use crate::mesh::{
    AttrData, Attribute, AttributeSet, Domain, Mesh, SELECT_EDGE, SELECT_POLY, SELECT_VERT,
    SHARP_FACE, UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT, bool_attr,
};
use crate::primitives::UV_MAP_NAME;

pub(crate) type V3 = [f32; 3];

/// A face: its loops from `l_first`, with one UV per loop.
#[derive(Clone, Debug, Default)]
pub(crate) struct Face {
    pub verts: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
}

/// A pool with Blender's slot reuse: `alloc` pops the most recently freed slot, else appends.
#[derive(Clone, Debug)]
pub(crate) struct Pool<T> {
    pub slots: Vec<Option<T>>,
    free: Vec<u32>,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Pool<T> {
    pub fn alloc(&mut self, v: T) -> u32 {
        if let Some(i) = self.free.pop() {
            self.slots[i as usize] = Some(v);
            i
        } else {
            self.slots.push(Some(v));
            u32::try_from(self.slots.len() - 1)
                .expect("primitive element count exceeds u32 (callers check `counts()` first)")
        }
    }
    pub fn free(&mut self, i: u32) {
        if self.slots[i as usize].take().is_some() {
            self.free.push(i);
        }
    }
    pub fn get(&self, i: u32) -> Option<&T> {
        self.slots.get(i as usize).and_then(|s| s.as_ref())
    }
    pub fn get_mut(&mut self, i: u32) -> Option<&mut T> {
        self.slots.get_mut(i as usize).and_then(|s| s.as_mut())
    }
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|v| (i as u32, v)))
    }
    pub fn live(&self) -> usize {
        self.slots.len() - self.free.len()
    }
}

#[inline]
pub(crate) fn ekey(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

/// The primitive builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct Bm {
    pub verts: Pool<V3>,
    pub edges: Pool<[u32; 2]>,
    pub faces: Pool<Face>,
    emap: HashMap<(u32, u32), u32>,
}

impl Bm {
    pub fn new() -> Bm {
        Bm::default()
    }

    /// `BM_vert_create`.
    pub fn vert_create(&mut self, co: V3) -> u32 {
        frap_math::budget::check();
        self.verts.alloc(co)
    }

    pub fn co(&self, v: u32) -> V3 {
        *self.verts.get(v).expect("live vertex")
    }

    pub fn co_mut(&mut self, v: u32) -> &mut V3 {
        self.verts.get_mut(v).expect("live vertex")
    }

    /// `BM_edge_exists`.
    pub fn edge_exists(&self, a: u32, b: u32) -> Option<u32> {
        self.emap.get(&ekey(a, b)).copied()
    }

    /// `BM_edge_create(bm, a, b, nullptr, BM_CREATE_NO_DOUBLE)`.
    pub fn edge_create(&mut self, a: u32, b: u32) -> u32 {
        if let Some(e) = self.edge_exists(a, b) {
            return e;
        }
        self.edge_create_always(a, b)
    }

    /// `BM_edge_create(bm, a, b, nullptr, BM_CREATE_NOP)` (the caller knows it is new).
    pub fn edge_create_always(&mut self, a: u32, b: u32) -> u32 {
        let e = self.edges.alloc([a, b]);
        self.emap.insert(ekey(a, b), e);
        e
    }

    /// `BM_face_create_verts(…, create_edges=true)` / `BM_face_create_quad_tri`: edges
    /// `(v[n-1], v[0]), (v[0], v[1]), …` ensured in that order; UVs zero.
    pub fn face_create(&mut self, verts: &[u32]) -> u32 {
        frap_math::budget::check();
        let n = verts.len();
        let mut prev = n - 1;
        for i in 0..n {
            self.edge_create(verts[prev], verts[i]);
            prev = i;
        }
        self.faces.alloc(Face {
            verts: verts.to_vec(),
            uvs: vec![[0.0; 2]; n],
        })
    }

    /// `BM_face_kill` (loops only; edges and verts stay).
    pub fn face_kill(&mut self, f: u32) {
        self.faces.free(f);
    }

    /// `bm_kill_only_edge`.
    pub fn edge_kill_only(&mut self, e: u32) {
        if let Some([a, b]) = self.edges.get(e).copied() {
            if self.emap.get(&ekey(a, b)) == Some(&e) {
                self.emap.remove(&ekey(a, b));
            }
            self.edges.free(e);
        }
    }

    /// `bm_kill_only_vert`.
    pub fn vert_kill_only(&mut self, v: u32) {
        self.verts.free(v);
    }

    /// Re-keys an edge after its endpoints changed.
    pub fn edge_set_verts(&mut self, e: u32, a: u32, b: u32) {
        if let Some(old) = self.edges.get(e).copied()
            && self.emap.get(&ekey(old[0], old[1])) == Some(&e)
        {
            self.emap.remove(&ekey(old[0], old[1]));
        }
        *self.edges.get_mut(e).expect("live edge") = [a, b];
        self.emap.insert(ekey(a, b), e);
    }

    /// Converts to a `Mesh` the way `BM_mesh_bm_to_me` does after `mesh.primitive_*_add`:
    /// elements in pool order, every element selected, every face flat, attributes in Blender's
    /// creation order (`UVMap`, `.select_vert`, `.select_edge`, `.select_poly`, `sharp_face`,
    /// `.uv_select_vert`, `.uv_select_edge`, `.uv_select_face`, `position`, `.edge_verts`,
    /// `.corner_vert`, `.corner_edge`). The `.select_*`/`sharp_face` flags exist only when
    /// their domain is non-empty; the UV selection attributes exist (all false) with a UV map.
    pub fn to_mesh(&self, name: &str, calc_uvs: bool) -> Mesh {
        let mut vindex = vec![u32::MAX; self.verts.slots.len()];
        let mut positions = Vec::with_capacity(self.verts.live());
        for (i, co) in self.verts.iter() {
            vindex[i as usize] = positions.len() as u32;
            positions.push(*co);
        }
        let mut eindex: HashMap<(u32, u32), i32> = HashMap::with_capacity(self.edges.live());
        let mut edges = Vec::with_capacity(self.edges.live());
        for (_, &[a, b]) in self.edges.iter() {
            let (a, b) = (vindex[a as usize], vindex[b as usize]);
            eindex.insert(ekey(a, b), edges.len() as i32);
            edges.push([a as i32, b as i32]);
        }
        let mut offsets = vec![0i32];
        let mut corner_verts = Vec::new();
        let mut corner_edges = Vec::new();
        let mut uvs = Vec::new();
        for (_, f) in self.faces.iter() {
            frap_math::budget::check();
            let n = f.verts.len();
            for i in 0..n {
                let a = vindex[f.verts[i] as usize];
                let b = vindex[f.verts[(i + 1) % n] as usize];
                corner_verts.push(a as i32);
                corner_edges.push(*eindex.get(&ekey(a, b)).expect("face edge exists"));
            }
            uvs.extend_from_slice(&f.uvs);
            offsets.push(corner_verts.len() as i32);
        }
        let (nv, ne, nf, nc) = (
            positions.len(),
            edges.len(),
            offsets.len() - 1,
            corner_verts.len(),
        );
        let mut attrs = AttributeSet::default();
        if calc_uvs {
            attrs.insert(Attribute {
                name: UV_MAP_NAME.into(),
                domain: Domain::Corner,
                data: AttrData::Float2(Arc::new(uvs)),
            });
        }
        if nv > 0 {
            attrs.insert(bool_attr(SELECT_VERT, Domain::Point, vec![true; nv]));
        }
        if ne > 0 {
            attrs.insert(bool_attr(SELECT_EDGE, Domain::Edge, vec![true; ne]));
        }
        if nf > 0 {
            attrs.insert(bool_attr(SELECT_POLY, Domain::Face, vec![true; nf]));
            attrs.insert(bool_attr(SHARP_FACE, Domain::Face, vec![true; nf]));
        }
        if calc_uvs {
            attrs.insert(bool_attr(UV_SELECT_VERT, Domain::Corner, vec![false; nc]));
            attrs.insert(bool_attr(UV_SELECT_EDGE, Domain::Corner, vec![false; nc]));
            attrs.insert(bool_attr(UV_SELECT_FACE, Domain::Face, vec![false; nf]));
        }
        attrs.insert(Attribute {
            name: "position".into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(positions)),
        });
        attrs.insert(Attribute {
            name: ".edge_verts".into(),
            domain: Domain::Edge,
            data: AttrData::Int32x2(Arc::new(edges)),
        });
        attrs.insert(Attribute {
            name: ".corner_vert".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_verts)),
        });
        attrs.insert(Attribute {
            name: ".corner_edge".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_edges)),
        });
        let mut mesh = Mesh::from_parts(name, offsets, attrs);
        if calc_uvs {
            mesh.uv_active = Some(UV_MAP_NAME.into());
            mesh.uv_active_render = Some(UV_MAP_NAME.into());
        }
        mesh
    }
}
