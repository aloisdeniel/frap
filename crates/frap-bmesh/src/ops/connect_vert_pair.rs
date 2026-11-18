// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_vert_pair` (Blender 5.2.4 `bmo_connect_vert_pair_exec`,
//! `operators/bmo_connect_pair.cc`), implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.connect_vert_pair(bmesh, verts=[], verts_exclude=[], faces_exclude=[])`.
//!
//! **Create/kill order**: the plane through both vertices (spanned by their direction and
//! their averaged normals) is walked across faces from the first vertex, shortest path
//! first (`BLI_heapsimple` of path states); when the second vertex is reached, the crossed
//! edges are split where the plane cuts them (`BM_edge_split` from `e->v1`, path order from
//! the end), and `connect_verts(check_degenerate=True)` joins the path's vertices.
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH`.

use crate::ops::connect_verts;
use crate::ops::contextual_create::bli::{self, V3};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, LoopId, VertId};

/// Input slots of `connect_vert_pair` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertPairIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `verts_exclude`.
    pub verts_exclude: Vec<VertId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
}

/// Output slots of `connect_vert_pair`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertPairOut {
    /// `edges.out` as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the edges of the connection (mesh order).
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_connect_vert_pair_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const CONNECT_EPS: f32 = 0.0001;
const VERT_OUT: u16 = 1;
const VERT_EXCLUDE: u16 = 2;
const FACE_EXCLUDE: u16 = 2;
const ELE_TOUCHED: u16 = 4;

/// `BLI_heapsimple` (`blenlib/intern/BLI_heap_simple.cc`).
#[derive(Default)]
struct HeapSimple {
    tree: Vec<(f32, usize)>,
}

impl HeapSimple {
    fn insert(&mut self, value: f32, ptr: usize) {
        let mut i = self.tree.len();
        self.tree.push((value, ptr));
        while i > 0 {
            let p = (i - 1) >> 1;
            if value >= self.tree[p].0 {
                break;
            }
            self.tree[i] = self.tree[p];
            i = p;
        }
        self.tree[i] = (value, ptr);
    }
    fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
    fn clear(&mut self) {
        self.tree.clear();
    }
    fn pop_min(&mut self) -> usize {
        let ptr = self.tree[0].1;
        let last = self.tree.pop().expect("non-empty");
        if !self.tree.is_empty() {
            // `heapsimple_down(heap, 0, &tree[size])`.
            let size = self.tree.len();
            let (active_val, active_ptr) = last;
            let mut i = 0usize;
            self.tree[i].0 = active_val;
            loop {
                let l = 2 * i + 1;
                let r = l + 1;
                let mut smallest = i;
                if l < size && self.tree[l].0 < active_val {
                    smallest = l;
                }
                if r < size && self.tree[r].0 < self.tree[smallest].0 {
                    smallest = r;
                }
                if smallest == i {
                    break;
                }
                self.tree[i] = self.tree[smallest];
                i = smallest;
                self.tree[i].0 = active_val;
            }
            self.tree[i].1 = active_ptr;
        }
        ptr
    }
}

#[derive(Clone, Debug)]
struct Link {
    next: Option<usize>,
    ele: BMElem,
    ele_from: Option<BMElem>,
}

#[derive(Clone, Debug)]
struct State {
    link_last: Option<usize>,
    dist: f32,
    co_prev: V3,
}

struct Pc {
    heap: HeapSimple,
    states: Vec<State>,
    links: Vec<Link>,
    /// The inverted orientation matrix (Blender's `m[col][row]` layout).
    matrix: [[f32; 3]; 3],
    axis_sep: f32,
}

#[derive(Copy, Clone)]
struct MinDistDir {
    dist_min: [f32; 2],
    dir: V3,
}

impl MinDistDir {
    fn new() -> Self {
        MinDistDir {
            dist_min: [f32::MAX, f32::MAX],
            dir: [0.0; 3],
        }
    }
    fn test(&self, dist_dir: V3, dist_sq: f32) -> i32 {
        if self.dist_min[0] == f32::MAX {
            return 0;
        }
        if bli::dot(dist_dir, self.dir) > 0.0 {
            if dist_sq < self.dist_min[0] {
                return 0;
            }
        } else if dist_sq < self.dist_min[1] {
            return 1;
        }
        -1
    }
    fn update(&mut self, dist_dir: V3) {
        if self.dist_min[0] == f32::MAX {
            self.dir = dist_dir;
        }
    }
}

impl Pc {
    fn row_x(&self, a: V3) -> f32 {
        self.matrix[0][0] * a[0] + self.matrix[1][0] * a[1] + self.matrix[2][0] * a[2]
    }
    fn isect_co_pair(&self, a: V3, b: V3) -> bool {
        let da = self.row_x(a) - self.axis_sep;
        let db = self.row_x(b) - self.axis_sep;
        let ta = if da.abs() < CONNECT_EPS { 0 } else if da < 0.0 { -1 } else { 1 };
        let tb = if db.abs() < CONNECT_EPS { 0 } else if db < 0.0 { -1 } else { 1 };
        ta != 0 && tb != 0 && ta != tb
    }
    fn isect_co_exact(&self, co: V3) -> bool {
        (self.row_x(co) - self.axis_sep).abs() <= CONNECT_EPS
    }
    fn co_pair_fac(&self, a: V3, b: V3) -> f32 {
        let da = (self.row_x(a) - self.axis_sep).abs();
        let db = (self.row_x(b) - self.axis_sep).abs();
        let tot = da + db;
        if tot > f32::EPSILON { da / tot } else { 0.5 }
    }
    fn co_pair(&self, a: V3, b: V3) -> V3 {
        bli::interp_v3_v3v3(a, b, self.co_pair_fac(a, b))
    }

    /// `state_link_add`.
    fn link_add(&mut self, bm: &mut BMesh, s: usize, ele: BMElem, ele_from: Option<BMElem>) {
        bm.oflag_enable(ele, ELE_TOUCHED);
        let co = match ele {
            BMElem::Vert(v) => bm.vert(v).co,
            BMElem::Edge(e) => {
                let [a, b] = bm.edge(e).v;
                self.co_pair(bm.vert(a).co, bm.vert(b).co)
            }
            BMElem::Face(_) => unreachable!("faces are never path links"),
        };
        let st = &mut self.states[s];
        if ele_from.is_some() {
            st.dist += bli::len_v3v3(st.co_prev, co);
        }
        st.co_prev = co;
        let li = self.links.len();
        self.links.push(Link {
            next: st.link_last,
            ele,
            ele_from,
        });
        st.link_last = Some(li);
    }

    /// `state_link_add_test`.
    fn link_add_test(
        &mut self,
        bm: &mut BMesh,
        mut s: usize,
        orig: &State,
        ele: BMElem,
        ele_from: BMElem,
    ) -> usize {
        let is_new = orig.link_last != self.states[s].link_last;
        if is_new {
            s = self.states.len();
            self.states.push(orig.clone());
        }
        self.link_add(bm, s, ele, Some(ele_from));
        if is_new {
            self.heap.insert(self.states[s].dist, s);
        }
        s
    }

    /// `state_step__face_edges`.
    fn step_face_edges(
        &mut self,
        bm: &mut BMesh,
        mut s: usize,
        orig: &State,
        l_start: LoopId,
        l_last: LoopId,
        mddir: &mut MinDistDir,
    ) -> usize {
        let mut best: [Option<LoopId>; 2] = [None, None];
        let mut l_iter = l_start;
        loop {
            let ll = bm.lp(l_iter).clone();
            let a = bm.vert(ll.v).co;
            let b = bm.vert(bm.lp(ll.next).v).co;
            if self.isect_co_pair(a, b) {
                let co_isect = self.co_pair(a, b);
                let dist_dir = bli::sub(co_isect, orig.co_prev);
                let dist_test = bli::len_squared_v3(dist_dir);
                let index = mddir.test(dist_dir, dist_test);
                if index != -1
                    && !bm.oflag_test(ll.f, FACE_EXCLUDE)
                    && !bm.oflag_test(ll.e, ELE_TOUCHED)
                {
                    mddir.update(dist_dir);
                    mddir.dist_min[index as usize] = dist_test;
                    best[index as usize] = Some(l_iter);
                }
            }
            l_iter = ll.next;
            if l_iter == l_last {
                break;
            }
        }
        for l in best.into_iter().flatten() {
            let ll = bm.lp(l).clone();
            s = self.link_add_test(bm, s, orig, BMElem::Edge(ll.e), BMElem::Face(ll.f));
        }
        s
    }

    /// `state_step__face_verts`.
    fn step_face_verts(
        &mut self,
        bm: &mut BMesh,
        mut s: usize,
        orig: &State,
        l_start: LoopId,
        l_last: LoopId,
        mddir: &mut MinDistDir,
    ) -> usize {
        let mut best: [Option<LoopId>; 2] = [None, None];
        let mut l_iter = l_start;
        loop {
            let ll = bm.lp(l_iter).clone();
            let co = bm.vert(ll.v).co;
            if self.isect_co_exact(co) {
                let dist_dir = bli::sub(co, orig.co_prev);
                let dist_test = bli::len_squared_v3(dist_dir);
                let index = mddir.test(dist_dir, dist_test);
                if index != -1
                    && !bm.oflag_test(ll.f, FACE_EXCLUDE)
                    && !bm.oflag_test(ll.v, ELE_TOUCHED)
                {
                    mddir.update(dist_dir);
                    mddir.dist_min[index as usize] = dist_test;
                    best[index as usize] = Some(l_iter);
                }
            }
            l_iter = ll.next;
            if l_iter == l_last {
                break;
            }
        }
        for l in best.into_iter().flatten() {
            let ll = bm.lp(l).clone();
            s = self.link_add_test(bm, s, orig, BMElem::Vert(ll.v), BMElem::Face(ll.f));
        }
        s
    }

    /// `state_step`: true when the state was extended.
    fn step(&mut self, bm: &mut BMesh, s0: usize) -> bool {
        let orig = self.states[s0].clone();
        let link = self.links[orig.link_last.expect("state has links")].clone();
        let mut s = s0;
        match link.ele {
            BMElem::Edge(e) => {
                for l_start in bm.edge_loops(e) {
                    let f = bm.lp(l_start).f;
                    if Some(BMElem::Face(f)) != link.ele_from && !bm.oflag_test(f, FACE_EXCLUDE) {
                        let mut mddir = MinDistDir::new();
                        let next = bm.lp(l_start).next;
                        s = self.step_face_edges(bm, s, &orig, next, l_start, &mut mddir);
                        let nn = bm.lp(next).next;
                        s = self.step_face_verts(bm, s, &orig, nn, l_start, &mut mddir);
                    }
                }
            }
            BMElem::Vert(v) => {
                for l_start in bm.vert_loops(v) {
                    let f = bm.lp(l_start).f;
                    if Some(BMElem::Face(f)) != link.ele_from && !bm.oflag_test(f, FACE_EXCLUDE) {
                        let mut mddir = MinDistDir::new();
                        let ll = bm.lp(l_start).clone();
                        s = self.step_face_edges(bm, s, &orig, ll.next, ll.prev, &mut mddir);
                        if bm.face(f).len > 3 {
                            let nn = bm.lp(ll.next).next;
                            s = self.step_face_verts(bm, s, &orig, nn, ll.prev, &mut mddir);
                        }
                    }
                }
                for e in bm.vert_edges(v) {
                    let v_other = bm.edge_other_vert(e, v);
                    if Some(BMElem::Edge(e)) != link.ele_from
                        && !bm.oflag_test(v_other, VERT_EXCLUDE)
                        && self.isect_co_exact(bm.vert(v_other).co)
                        && !bm.oflag_test(v_other, ELE_TOUCHED)
                    {
                        s = self.link_add_test(bm, s, &orig, BMElem::Vert(v_other), BMElem::Edge(e));
                    }
                }
            }
            BMElem::Face(_) => unreachable!("faces are never path links"),
        }
        orig.link_last != self.states[s].link_last
    }
}

// Ported from blender/source/blender/blenlib/intern/math_matrix_c.cc (invert_m3_m3,
// adjoint_m3_m3, determinant_m3_array).
fn invert_m3(m: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let [m00, m01, m02] = m[0];
    let [m10, m11, m12] = m[1];
    let [m20, m21, m22] = m[2];
    let mut r = [
        [m11 * m22 - m12 * m21, -m01 * m22 + m02 * m21, m01 * m12 - m02 * m11],
        [-m10 * m22 + m12 * m20, m00 * m22 - m02 * m20, -m00 * m12 + m02 * m10],
        [m10 * m21 - m11 * m20, -m00 * m21 + m01 * m20, m00 * m11 - m01 * m10],
    ];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
    if det == 0.0 {
        return None;
    }
    let d = 1.0 / det;
    for row in &mut r {
        for x in row.iter_mut() {
            *x *= d;
        }
    }
    Some(r)
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect_pair.cc
// (bm_vert_pair_to_matrix).
fn vert_pair_to_matrix(bm: &BMesh, v_pair: [VertId; 2]) -> [[f32; 3]; 3] {
    let eps = 1e-8f32;
    let basis_dir = bli::normalize_v3(bli::sub(bm.vert(v_pair[0]).co, bm.vert(v_pair[1]).co)).0;
    let nor_a = bli::project_plane_normalized_v3_v3v3(bm.vert(v_pair[0]).no, basis_dir);
    let mut nor_b = bli::project_plane_normalized_v3_v3v3(bm.vert(v_pair[1]).no, basis_dir);
    if bli::dot(nor_a, nor_b) < 0.0 {
        nor_b = bli::negate(nor_b);
    }
    let mut basis_nor = bli::normalize_v3(bli::add(nor_a, nor_b)).0;
    let (mut basis_tmp, l) = bli::normalize_v3(bli::cross(basis_dir, basis_nor));
    if l < eps {
        let mut axis_pair = [([0.0f32; 3], -f32::MAX); 2];
        for (i, &v) in v_pair.iter().enumerate() {
            for l in bm.vert_loops(v) {
                let fno = bm.face(bm.lp(l).f).no;
                let (proj, pl) = bli::normalize_v3(bli::project_plane_normalized_v3_v3v3(basis_dir, fno));
                if pl > eps {
                    let c = bli::dot(proj, basis_dir);
                    if c > axis_pair[i].1 {
                        axis_pair[i] = (proj, c);
                    }
                }
            }
        }
        basis_nor = axis_pair[(axis_pair[0].1 < axis_pair[1].1) as usize].0;
        basis_nor = bli::project_plane_normalized_v3_v3v3(basis_nor, basis_dir);
        let (t, l2) = bli::normalize_v3(bli::cross(basis_dir, basis_nor));
        basis_tmp = t;
        if l2 < eps {
            basis_nor = bli::normalize_v3(crate::query::geom::ortho_v3_v3(basis_dir)).0;
            basis_tmp = bli::normalize_v3(bli::cross(basis_dir, basis_nor)).0;
        }
    }
    let m = [basis_tmp, basis_dir, basis_nor];
    invert_m3(m).unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
}

/// Runs `connect_vert_pair` on `bm`.
pub fn connect_vert_pair(
    bm: &mut BMesh,
    i: &ConnectVertPairIn,
) -> Result<ConnectVertPairOut, BmError> {
    if let Some(dead) = i.verts.iter().find(|&&v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!(
            "connect_vert_pair: dead vertex {dead:?}"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect_pair.cc
// (bmo_connect_vert_pair_exec).
fn exec(bm: &mut BMesh, i: &ConnectVertPairIn) -> Result<ConnectVertPairOut, BmError> {
    if i.verts.len() != 2 {
        return Ok(ConnectVertPairOut::default());
    }
    let v_pair = [i.verts[0], i.verts[1]];
    for &f in &i.faces_exclude {
        bm.oflag_enable(f, FACE_EXCLUDE);
    }
    for &v in &i.verts_exclude {
        bm.oflag_enable(v, VERT_EXCLUDE);
    }
    let matrix = vert_pair_to_matrix(bm, v_pair);
    let mut pc = Pc {
        heap: HeapSimple::default(),
        states: Vec::new(),
        links: Vec::new(),
        matrix,
        axis_sep: 0.0,
    };
    pc.axis_sep = pc.row_x(bm.vert(v_pair[0]).co);
    pc.states.push(State {
        link_last: None,
        dist: 0.0,
        co_prev: [0.0; 3],
    });
    pc.link_add(bm, 0, BMElem::Vert(v_pair[0]), None);
    pc.heap.insert(pc.states[0].dist, 0);
    let mut best: Option<State> = None;
    while !pc.heap.is_empty() {
        frap_math::budget::check();
        let s = pc.heap.pop_min();
        let last = pc.states[s].link_last.expect("links");
        if pc.links[last].ele == BMElem::Vert(v_pair[1]) {
            best = Some(pc.states[s].clone());
            pc.heap.clear();
        } else if pc.step(bm, s) {
            pc.heap.insert(pc.states[s].dist, s);
        }
    }
    if let Some(best) = &best {
        let mut link = best.link_last;
        while let Some(li) = link {
            match pc.links[li].ele {
                BMElem::Edge(e) => {
                    let [a, b] = bm.edge(e).v;
                    let fac = pc.co_pair_fac(bm.vert(a).co, bm.vert(b).co);
                    let (v_new, _) = bm.edge_split(e, a, fac);
                    for l in bm.edge_loops(e) {
                        let f = bm.lp(l).f;
                        bm.face_normal_update(f);
                    }
                    bm.oflag_enable(v_new, VERT_OUT);
                }
                BMElem::Vert(v) => bm.oflag_enable(v, VERT_OUT),
                BMElem::Face(_) => {}
            }
            link = pc.links[li].next;
        }
    }
    bm.oflag_enable(v_pair[0], VERT_OUT);
    bm.oflag_enable(v_pair[1], VERT_OUT);
    if best.is_none() {
        return Ok(ConnectVertPairOut::default());
    }
    let verts: Vec<VertId> = bm
        .vert_ids()
        .into_iter()
        .filter(|&v| bm.oflag_test(v, VERT_OUT))
        .collect();
    let r = connect_verts::connect_verts(
        bm,
        &connect_verts::ConnectVertsIn {
            verts,
            faces_exclude: i.faces_exclude.clone(),
            check_degenerate: true,
        },
    )?;
    Ok(ConnectVertPairOut {
        geom: r.geom,
        edges: r.edges,
    })
}
