// SPDX-License-Identifier: GPL-3.0-or-later
//! Bevel data structures (`NewVert`, `EdgeHalf`, `Profile`, `ProfileSpacing`, `BoundVert`,
//! `VMesh`, `BevVert`, `BevelParams`). Ported from
//! blender/source/blender/bmesh/tools/bmesh_bevel.cc (Blender 5.2.4, GPL-2.0-or-later).
//!
//! Blender links these structures with pointers into a memory arena; here they live in
//! vectors owned by [`Bevel`] and refer to each other by index ([`Eh`], [`Bnd`], [`Vm`],
//! [`Bev`]). Arena memory is zeroed (`BLI_memarena_use_calloc`), which `Default` mirrors.

use std::collections::BTreeMap;

use rustc_hash::FxHashMap;

use super::math::V3;
use crate::types::{EdgeId, FaceId, LoopId, VertId};

/// An `EdgeHalf` (index into [`Bevel::ehs`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Eh(pub(crate) u32);
/// A `BoundVert` (index into [`Bevel::bnds`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Bnd(pub(crate) u32);
/// A `VMesh` (index into [`Bevel::vms`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Vm(pub(crate) u32);
/// A `BevVert` (index into [`Bevel::bevs`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Bev(pub(crate) u32);

pub(crate) const BEVEL_EPSILON_D: f64 = 1e-6;
pub(crate) const BEVEL_EPSILON: f32 = 1e-6;
pub(crate) const BEVEL_EPSILON_SQ: f32 = 1e-12;
pub(crate) const BEVEL_EPSILON_BIG: f32 = 1e-4;
pub(crate) const BEVEL_MATCH_SPEC_WEIGHT: f64 = 0.2;
/// `BEVEL_GOOD_ANGLE`.
pub(crate) const BEVEL_GOOD_ANGLE: f32 = 0.1;

/// `BEVEL_EPSILON_ANG` (`DEG2RADF(2.0f)`).
pub(crate) fn bevel_epsilon_ang() -> f32 {
    super::math::deg2radf(2.0)
}
/// `BEVEL_SMALL_ANG` (`DEG2RADF(10.0f)`).
pub(crate) fn bevel_small_ang() -> f32 {
    super::math::deg2radf(10.0)
}
/// `BEVEL_EPSILON_ANG_DOT`.
pub(crate) fn bevel_epsilon_ang_dot() -> f32 {
    1.0 - libm::cosf(bevel_epsilon_ang())
}

pub(crate) const PRO_SQUARE_R: f32 = 1e4;
pub(crate) const PRO_CIRCLE_R: f32 = 2.0;
pub(crate) const PRO_LINE_R: f32 = 1.0;
pub(crate) const PRO_SQUARE_IN_R: f32 = 0.0;

/// Tool flag on output edges (`EDGE_OUT`).
pub(crate) const EDGE_OUT: u16 = 4;
/// Tool flag on output vertices (`VERT_OUT`).
pub(crate) const VERT_OUT: u16 = 8;

/// `BEVEL_AMT_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OffsetType {
    Offset,
    Width,
    Depth,
    Percent,
    Absolute,
}

/// `BEVEL_AFFECT_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Affect {
    Vertices,
    Edges,
}

/// `BEVEL_MITER_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Miter {
    Sharp,
    Patch,
    Arc,
}

/// `BEVEL_VMESH_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum VmeshMethod {
    Adj,
    Cutoff,
}

/// `BEVEL_FACE_STRENGTH_*` (ordered: `NONE < NEW < AFFECTED < ALL`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FaceStrength {
    None,
    New,
    Affected,
    All,
}

/// A constructed vertex, sometimes later instantiated as a `BMVert`.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct NewVert {
    pub(crate) v: Option<VertId>,
    pub(crate) co: V3,
}

/// Data for one end of an edge involved in a bevel.
#[derive(Clone, Debug, Default)]
pub(crate) struct EdgeHalf {
    /// Other EdgeHalves connected to the same BevVert, in CCW order.
    pub(crate) next: Eh,
    pub(crate) prev: Eh,
    /// Original mesh edge (unset only while the edge order is being found).
    pub(crate) e: Option<EdgeId>,
    /// Face between this edge and previous, if any.
    pub(crate) fprev: Option<FaceId>,
    /// Face between this edge and next, if any.
    pub(crate) fnext: Option<FaceId>,
    /// Left boundary vert (looking along edge to end).
    pub(crate) leftv: Option<Bnd>,
    /// Right boundary vert, if beveled.
    pub(crate) rightv: Option<Bnd>,
    /// Offset into profile to attach non-beveled edge.
    pub(crate) profile_index: i32,
    /// How many segments for the bevel.
    pub(crate) seg: i32,
    pub(crate) offset_l: f32,
    pub(crate) offset_r: f32,
    pub(crate) offset_l_spec: f32,
    pub(crate) offset_r_spec: f32,
    pub(crate) is_bev: bool,
    /// Is `e->v2` the vertex at this end?
    pub(crate) is_rev: bool,
    /// Is `e` a seam for custom loop data (UVs)?
    pub(crate) is_seam: bool,
}

/// Profile specification (see `bmesh_bevel.cc`): start, middle and end control points
/// projected onto a plane along `proj_dir`; `prof_co` holds the `seg + 1` points and
/// `prof_co_2` the points for the power of two `>= seg`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Profile {
    pub(crate) super_r: f32,
    pub(crate) height: f32,
    pub(crate) start: V3,
    pub(crate) middle: V3,
    pub(crate) end: V3,
    pub(crate) plane_no: V3,
    pub(crate) plane_co: V3,
    pub(crate) proj_dir: V3,
    pub(crate) prof_co: Vec<V3>,
    /// Empty when `prof_co_2` is `prof_co` (Blender aliases the pointers).
    pub(crate) prof_co_2: Vec<V3>,
    pub(crate) special_params: bool,
}

impl Profile {
    /// `pro->prof_co_2` (aliasing `prof_co` when `seg` is a power of two).
    pub(crate) fn co2(&self) -> &[V3] {
        if self.prof_co_2.is_empty() {
            &self.prof_co
        } else {
            &self.prof_co_2
        }
    }
}

/// Un-transformed 2D profile vertex locations for `seg` and `seg_2` segments.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProfileSpacing {
    pub(crate) xvals: Vec<f64>,
    pub(crate) yvals: Vec<f64>,
    /// Empty when `seg_2 == seg` (aliases `xvals`/`yvals`).
    pub(crate) xvals_2: Vec<f64>,
    pub(crate) yvals_2: Vec<f64>,
    pub(crate) seg_2: i32,
    pub(crate) fullness: f32,
}

impl ProfileSpacing {
    pub(crate) fn x2(&self) -> &[f64] {
        if self.xvals_2.is_empty() {
            &self.xvals
        } else {
            &self.xvals_2
        }
    }
    pub(crate) fn y2(&self) -> &[f64] {
        if self.yvals_2.is_empty() {
            &self.yvals
        } else {
            &self.yvals_2
        }
    }
}

/// Loop layers "with math" (UVs, colours) and, for odd segment counts, the connected
/// component (in UV space) of every face by face index.
#[derive(Clone, Debug, Default)]
pub(crate) struct MathLayerInfo {
    pub(crate) face_component: Option<Vec<i32>>,
    pub(crate) has_math_layers: bool,
}

/// An element in the cyclic boundary of a vertex mesh.
#[derive(Clone, Debug, Default)]
pub(crate) struct BoundVert {
    pub(crate) next: Bnd,
    pub(crate) prev: Bnd,
    pub(crate) nv: NewVert,
    /// First of edges attached here, in CCW order.
    pub(crate) efirst: Option<Eh>,
    pub(crate) elast: Option<Eh>,
    /// The "edge between" this boundvert is on (`offset_on_edge_between`).
    pub(crate) eon: Option<Eh>,
    /// Beveled edge whose left side is attached here, if any.
    pub(crate) ebev: Option<Eh>,
    pub(crate) index: i32,
    pub(crate) sinratio: f32,
    pub(crate) adjchain: Option<Bnd>,
    /// Edge profile between this and the next BoundVert.
    pub(crate) profile: Profile,
    pub(crate) any_seam: bool,
    pub(crate) visited: bool,
    pub(crate) is_arc_start: bool,
    pub(crate) is_patch_start: bool,
    pub(crate) is_profile_start: bool,
    pub(crate) seam_len: i32,
    pub(crate) sharp_len: i32,
}

/// `MeshKind`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum MeshKind {
    #[default]
    None,
    Poly,
    Adj,
    TriFan,
    Cutoff,
}

/// The mesh structure replacing a vertex.
#[derive(Clone, Debug, Default)]
pub(crate) struct VMesh {
    pub(crate) mesh: Vec<NewVert>,
    pub(crate) boundstart: Option<Bnd>,
    pub(crate) count: i32,
    pub(crate) seg: i32,
    pub(crate) mesh_kind: MeshKind,
}

impl VMesh {
    /// `mesh_vert(vm, i, j, k)` as an index into `mesh`.
    #[inline]
    pub(crate) fn idx(&self, i: i32, j: i32, k: i32) -> usize {
        let nj = (self.seg / 2) + 1;
        let nk = self.seg + 1;
        (i * nk * nj + j * nk + k) as usize
    }
    /// `mesh_vert(vm, i, j, k)`.
    #[inline]
    pub(crate) fn mv(&self, i: i32, j: i32, k: i32) -> &NewVert {
        &self.mesh[self.idx(i, j, k)]
    }
    /// `mesh_vert(vm, i, j, k)`, mutably.
    #[inline]
    pub(crate) fn mv_mut(&mut self, i: i32, j: i32, k: i32) -> &mut NewVert {
        let x = self.idx(i, j, k);
        &mut self.mesh[x]
    }
    /// Allocates the zeroed `mesh` array for `count` boundverts and `seg` segments.
    pub(crate) fn alloc_mesh(&mut self) {
        let n = (self.count * (1 + self.seg / 2) * (1 + self.seg)) as usize;
        self.mesh = vec![NewVert::default(); n];
    }

    /// `mesh_vert_canon`: the canonical index for any `(i, j, k)`.
    pub(crate) fn canon(&self, i: i32, j: i32, k: i32) -> usize {
        let n = self.count;
        let ns = self.seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        debug_assert!(0 <= i && i <= n && 0 <= j && j <= ns && 0 <= k && k <= ns);
        if odd == 0 && j == ns2 && k == ns2 {
            return self.idx(0, j, k);
        }
        if j < ns2 + odd && k <= ns2 {
            return self.idx(i, j, k);
        }
        if k <= ns2 {
            return self.idx((i + n - 1) % n, k, ns - j);
        }
        self.idx((i + 1) % n, ns - k, j)
    }

    /// `is_canon`.
    pub(crate) fn is_canon(&self, i: i32, j: i32, k: i32) -> bool {
        let ns2 = self.seg / 2;
        if self.seg % 2 == 1 {
            return j <= ns2 && k <= ns2;
        }
        (j < ns2 && k <= ns2) || (j == ns2 && k == ns2 && i == 0)
    }

    /// `vmesh_copy_equiv_verts`.
    pub(crate) fn copy_equiv_verts(&mut self) {
        let n = self.count;
        let ns = self.seg;
        let ns2 = ns / 2;
        for i in 0..n {
            for j in 0..=ns2 {
                for k in 0..=ns {
                    if self.is_canon(i, j, k) {
                        continue;
                    }
                    let src = self.mesh[self.canon(i, j, k)];
                    let dst = self.idx(i, j, k);
                    self.mesh[dst] = src;
                }
            }
        }
    }

    /// `copy_mesh_vert`.
    pub(crate) fn copy_mesh_vert(
        &mut self,
        ito: i32,
        jto: i32,
        kto: i32,
        ifr: i32,
        jfr: i32,
        kfr: i32,
    ) {
        let src = *self.mv(ifr, jfr, kfr);
        let d = self.mv_mut(ito, jto, kto);
        d.v = src.v;
        d.co = src.co;
    }
}

/// Data for a vertex involved in a bevel.
#[derive(Clone, Debug)]
pub(crate) struct BevVert {
    pub(crate) v: VertId,
    /// Total number of edges around the vertex (excluding wire edges if edge beveling).
    pub(crate) edgecount: i32,
    /// Number of selected edges around the vertex.
    pub(crate) selcount: i32,
    pub(crate) wirecount: i32,
    pub(crate) offset: f32,
    pub(crate) any_seam: bool,
    /// First `EdgeHalf` (the `edgecount` halves are consecutive in [`Bevel::ehs`]), CCW order
    /// from the vertex normal side.
    pub(crate) edges: u32,
    pub(crate) wire_edges: Vec<EdgeId>,
    pub(crate) vmesh: Vm,
}

impl BevVert {
    /// `&bv->edges[i]`.
    #[inline]
    pub(crate) fn edge(&self, i: i32) -> Eh {
        Eh(self.edges + i as u32)
    }
}

/// Face classification (`FKind`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FKind {
    Orig,
    Vert,
    Edge,
    Recon,
}

/// Angle classification (`AngleKind`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AngleKind {
    Smaller,
    Straight,
    Larger,
}

/// UV vert buckets of one vertex (each bucket is a set of loops, in insertion order).
pub(crate) type UvBuckets = Vec<Vec<LoopId>>;

/// Bevel parameters and state (`BevelParams` plus the arenas).
#[derive(Debug)]
pub(crate) struct Bevel {
    pub(crate) vert_hash: FxHashMap<VertId, Bev>,
    pub(crate) face_hash: FxHashMap<FaceId, FKind>,
    /// `uv_face_hash`: new face → its `attached_frep`.
    pub(crate) uv_face_hash: FxHashMap<FaceId, Option<FaceId>>,
    /// One map per UV layer (deterministic order, ARCHITECTURE §21.4).
    pub(crate) uv_vert_maps: Vec<BTreeMap<VertId, UvBuckets>>,
    pub(crate) pro_spacing: ProfileSpacing,
    pub(crate) pro_spacing_miter: ProfileSpacing,
    pub(crate) math_layer_info: MathLayerInfo,
    pub(crate) offset: f32,
    pub(crate) offset_type: OffsetType,
    pub(crate) affect_type: Affect,
    pub(crate) affect_vertices_odd: bool,
    pub(crate) seg: i32,
    pub(crate) profile: f32,
    pub(crate) pro_super_r: f32,
    pub(crate) use_weights: bool,
    /// Layer index of `bevel_weight_vert` (`bweight_offset_vert`), if any.
    pub(crate) bweight_vert: Option<usize>,
    /// Layer index of `bevel_weight_edge`, if any.
    pub(crate) bweight_edge: Option<usize>,
    /// `dvert` + `vertex_group` (the BEVEL modifier's vertex group, WP 4.5): the group weight
    /// of every vertex by slot (`BKE_defvert_find_weight`), scaling vertex-bevel offsets.
    pub(crate) vertex_group_weights: Option<Vec<f32>>,
    pub(crate) loop_slide: bool,
    pub(crate) limit_offset: bool,
    pub(crate) offset_adjust: bool,
    pub(crate) mark_seam: bool,
    pub(crate) mark_sharp: bool,
    pub(crate) mat_nr: i32,
    pub(crate) face_strength_mode: FaceStrength,
    pub(crate) miter_outer: Miter,
    pub(crate) miter_inner: Miter,
    pub(crate) vmesh_method: VmeshMethod,
    pub(crate) spread: f32,
    // Arenas.
    pub(crate) ehs: Vec<EdgeHalf>,
    pub(crate) bnds: Vec<BoundVert>,
    pub(crate) vms: Vec<VMesh>,
    pub(crate) bevs: Vec<BevVert>,
    /// `BM_BEVEL_EDGE_TAG` (`_FLAG_OVERLAP` API flag) by edge slot.
    pub(crate) edge_tag: Vec<bool>,
}

impl Bevel {
    #[inline]
    pub(crate) fn eh(&self, e: Eh) -> &EdgeHalf {
        &self.ehs[e.0 as usize]
    }
    #[inline]
    pub(crate) fn eh_mut(&mut self, e: Eh) -> &mut EdgeHalf {
        &mut self.ehs[e.0 as usize]
    }
    /// `e->e` (always set once the edge order is known).
    #[inline]
    #[track_caller]
    pub(crate) fn ehe(&self, e: Eh) -> EdgeId {
        self.ehs[e.0 as usize].e.expect("EdgeHalf without edge")
    }
    #[inline]
    pub(crate) fn bnd(&self, b: Bnd) -> &BoundVert {
        &self.bnds[b.0 as usize]
    }
    #[inline]
    pub(crate) fn bnd_mut(&mut self, b: Bnd) -> &mut BoundVert {
        &mut self.bnds[b.0 as usize]
    }
    #[inline]
    pub(crate) fn vm(&self, v: Vm) -> &VMesh {
        &self.vms[v.0 as usize]
    }
    #[inline]
    pub(crate) fn vm_mut(&mut self, v: Vm) -> &mut VMesh {
        &mut self.vms[v.0 as usize]
    }
    #[inline]
    pub(crate) fn bev(&self, b: Bev) -> &BevVert {
        &self.bevs[b.0 as usize]
    }
    #[inline]
    pub(crate) fn bev_mut(&mut self, b: Bev) -> &mut BevVert {
        &mut self.bevs[b.0 as usize]
    }
    /// The vmesh of a BevVert.
    #[inline]
    pub(crate) fn bvm(&self, b: Bev) -> Vm {
        self.bevs[b.0 as usize].vmesh
    }
    /// `vm->boundstart` (set once the boundary is built).
    #[inline]
    #[track_caller]
    pub(crate) fn bstart(&self, v: Vm) -> Bnd {
        self.vms[v.0 as usize]
            .boundstart
            .expect("VMesh without boundary")
    }
    /// `bndv->next`.
    #[inline]
    pub(crate) fn bnext(&self, b: Bnd) -> Bnd {
        self.bnds[b.0 as usize].next
    }
    /// `bndv->prev`.
    #[inline]
    pub(crate) fn bprev(&self, b: Bnd) -> Bnd {
        self.bnds[b.0 as usize].prev
    }
    /// `e->next`.
    #[inline]
    pub(crate) fn enext(&self, e: Eh) -> Eh {
        self.ehs[e.0 as usize].next
    }
    /// `e->prev`.
    #[inline]
    pub(crate) fn eprev(&self, e: Eh) -> Eh {
        self.ehs[e.0 as usize].prev
    }
    /// Every BoundVert of a vmesh in boundary order, from `boundstart`.
    pub(crate) fn bounds(&self, v: Vm) -> Vec<Bnd> {
        let start = self.bstart(v);
        let mut out = vec![start];
        let mut b = self.bnext(start);
        while b != start {
            out.push(b);
            b = self.bnext(b);
        }
        out
    }

    /// `add_new_bound_vert`: a new BoundVert at the end of `vm`'s circular list.
    pub(crate) fn add_new_bound_vert(&mut self, vm: Vm, co: V3) -> Bnd {
        let ans = Bnd(self.bnds.len() as u32);
        let mut bv = BoundVert {
            nv: NewVert { v: None, co },
            ..BoundVert::default()
        };
        match self.vm(vm).boundstart {
            None => {
                bv.index = 0;
                bv.next = ans;
                bv.prev = ans;
                self.bnds.push(bv);
                self.vm_mut(vm).boundstart = Some(ans);
            }
            Some(start) => {
                let tail = self.bprev(start);
                bv.index = self.bnd(tail).index + 1;
                bv.prev = tail;
                bv.next = start;
                self.bnds.push(bv);
                self.bnd_mut(tail).next = ans;
                self.bnd_mut(start).prev = ans;
            }
        }
        let b = self.bnd_mut(ans);
        b.profile.super_r = PRO_LINE_R;
        b.adjchain = None;
        b.sinratio = 1.0;
        b.visited = false;
        b.any_seam = false;
        b.is_arc_start = false;
        b.is_patch_start = false;
        b.is_profile_start = false;
        self.vm_mut(vm).count += 1;
        ans
    }

    /// `new_adj_vmesh`.
    pub(crate) fn new_adj_vmesh(&mut self, count: i32, seg: i32, bounds: Option<Bnd>) -> Vm {
        let mut vm = VMesh {
            mesh: Vec::new(),
            boundstart: bounds,
            count,
            seg,
            mesh_kind: MeshKind::Adj,
        };
        vm.alloc_mesh();
        self.vms.push(vm);
        Vm(self.vms.len() as u32 - 1)
    }

    /// `find_edge_half`.
    pub(crate) fn find_edge_half(&self, bv: Bev, bme: EdgeId) -> Option<Eh> {
        let b = self.bev(bv);
        (0..b.edgecount)
            .map(|i| b.edge(i))
            .find(|&e| self.eh(e).e == Some(bme))
    }

    /// `find_bevvert`.
    pub(crate) fn find_bevvert(&self, v: VertId) -> Option<Bev> {
        self.vert_hash.get(&v).copied()
    }

    /// `record_face_kind`.
    pub(crate) fn record_face_kind(&mut self, f: Option<FaceId>, k: FKind) {
        if let Some(f) = f {
            self.face_hash.insert(f, k);
        }
    }

    /// `get_face_kind`.
    pub(crate) fn get_face_kind(&self, f: FaceId) -> FKind {
        self.face_hash.get(&f).copied().unwrap_or(FKind::Orig)
    }

    /// `next_bev`: the first beveled EdgeHalf at or after `from` (CCW), starting from the last
    /// edge when `from` is `None`.
    pub(crate) fn next_bev(&self, bv: Bev, from: Option<Eh>) -> Option<Eh> {
        let b = self.bev(bv);
        let from_e = from.unwrap_or(b.edge(b.edgecount - 1));
        let mut e = from_e;
        loop {
            if self.eh(e).is_bev {
                return Some(e);
            }
            e = self.enext(e);
            if e == from_e {
                return None;
            }
        }
    }

    /// `count_ccw_edges_between`.
    pub(crate) fn count_ccw_edges_between(&self, e1: Eh, e2: Eh) -> i32 {
        let mut count = 0;
        let mut e = e1;
        loop {
            if e == e2 {
                break;
            }
            e = self.enext(e);
            count += 1;
            if e == e1 {
                break;
            }
        }
        count
    }
}
