// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bisect_plane` (Blender 5.2.4 `bmo_bisect_plane_exec`,
//! `operators/bmo_bisect_plane.cc`) and `BM_mesh_bisect_plane` (`tools/bmesh_bisect_plane.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bisect_plane(bmesh, geom=[], dist=0.0, plane_co=Vector(), plane_no=Vector(), use_snap_center=False, clear_outer=False, clear_inner=False)`.
//!
//! **Create/kill order**: input edges crossing the plane are split in edge order
//! (`BM_edge_split` from `e->v1` at the crossing factor), their faces pushed on a stack; faces
//! touching a vertex on the plane are pushed too. Faces are then popped (last first) and cut
//! between their on-plane vertices (`BM_face_split`; several cuts are sorted across the face).
//! With `clear_inner`/`clear_outer` the input vertices on that side are killed afterwards
//! (`BM_vert_kill`, last input vertex first).
//!
//! Errors: `Zero normal given` (`BMO_ERROR_CANCEL`).
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ops::connect_verts::loop_is_adjacent;
use crate::ops::contextual_create::{bli, construct};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};

/// Input slots of `bisect_plane` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BisectPlaneIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dist`.
    pub dist: f32,
    /// `plane_co`.
    pub plane_co: [f32; 3],
    /// `plane_no`.
    pub plane_no: [f32; 3],
    /// `use_snap_center`.
    pub use_snap_center: bool,
    /// `clear_outer`.
    pub clear_outer: bool,
    /// `clear_inner`.
    pub clear_inner: bool,
}

/// Output slots of `bisect_plane`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BisectPlaneOut {
    /// `geom.out`: new and input elements still alive (vertices, edges, faces; mesh order).
    pub geom: Vec<BMElem>,
    /// `geom_cut.out`: the vertices and edges on the plane (mesh order).
    pub geom_cut: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_bisect_plane_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const ELE_NEW: u16 = 1;
const ELE_CUT: u16 = 2;
const ELE_INPUT: u16 = 4;

/// `plane_point_side_v3`.
fn plane_point_side_v3(plane: [f32; 4], co: [f32; 3]) -> f32 {
    bli::dot([plane[0], plane[1], plane[2]], co) + plane[3]
}

#[derive(Copy, Clone, Default)]
struct VInfo {
    dir: i16,
    skip: bool,
    dist: f32,
    sortval: f32,
    loopindex: u32,
}

struct Bisect {
    info: FxHashMap<VertId, VInfo>,
}

impl Bisect {
    fn get(&self, v: VertId) -> VInfo {
        self.info.get(&v).copied().unwrap_or_default()
    }
    fn get_mut(&mut self, v: VertId) -> &mut VInfo {
        self.info.entry(v).or_default()
    }
}

fn is_center(bm: &BMesh, v: VertId) -> bool {
    bm.vert(v).flag.contains(ElemFlag::TAG)
}

/// Faces are "in the stack" when their tag is *cleared* (`face_in_stack_test`).
fn face_in_stack(bm: &BMesh, f: FaceId) -> bool {
    !bm.face(f).flag.contains(ElemFlag::TAG)
}

// Ported from blender/source/blender/bmesh/tools/bmesh_bisect_plane.cc (bm_face_bisect_verts).
fn face_bisect_verts(bm: &mut BMesh, st: &mut Bisect, f: FaceId, plane: [f32; 4], oflag_center: u16, oflag_new: u16) {
    let f_len_orig = bm.face(f).len;
    let mut vert_split: Vec<VertId> = Vec::new();
    let mut use_dirs = [false; 3];
    let mut is_inside = false;
    let mut face_has_center_edge = false;
    let loops = bm.face_loops(f);
    for &l in &loops {
        let ll = bm.lp(l).clone();
        if is_center(bm, ll.v) {
            let dp = st.get(bm.lp(ll.prev).v).dir;
            let dn = st.get(bm.lp(ll.next).v).dir;
            st.get_mut(ll.v).skip = (dp ^ dn) == 0;
            vert_split.push(ll.v);
            if !face_has_center_edge && is_center(bm, bm.lp(ll.prev).v) {
                face_has_center_edge = true;
            }
        }
        use_dirs[(st.get(ll.v).dir + 1) as usize] = true;
    }
    if !(vert_split.len() > 1 && use_dirs[0] && use_dirs[2]) {
        return;
    }
    let flag_edge_face = |bm: &mut BMesh, l_new: crate::LoopId| {
        if oflag_center | oflag_new != 0 {
            let e = bm.lp(l_new).e;
            bm.oflag_enable(e, oflag_center | oflag_new);
        }
        if oflag_new != 0 {
            let fnew = bm.lp(l_new).f;
            bm.oflag_enable(fnew, oflag_new);
        }
    };
    if vert_split.len() == 2 {
        let l_a = bm.face_vert_share_loop(f, vert_split[0]);
        let l_b = bm.face_vert_share_loop(f, vert_split[1]);
        if let (Some(a), Some(b)) = (l_a, l_b)
            && let Some((_, l_new)) = bm.face_split(f, a, b, None, true)
        {
            flag_edge_face(bm, l_new);
        }
        return;
    }
    if face_has_center_edge {
        for (i, &l) in loops.iter().enumerate() {
            let v = bm.lp(l).v;
            st.get_mut(v).loopindex = i as u32;
        }
        let mut l_first_nc = loops[0];
        while is_center(bm, bm.lp(l_first_nc).v) {
            l_first_nc = bm.lp(l_first_nc).next;
        }
        let mut l_iter = l_first_nc;
        loop {
            let v = bm.lp(l_iter).v;
            let skip_this = st.get(v).skip
                || !(is_center(bm, v) && is_center(bm, bm.lp(bm.lp(l_iter).next).v));
            if !skip_this {
                let l_prev = bm.lp(l_iter).prev;
                let mut l_next = bm.lp(bm.lp(l_iter).next).next;
                while is_center(bm, bm.lp(l_next).v) {
                    l_next = bm.lp(l_next).next;
                }
                if (st.get(bm.lp(l_prev).v).dir ^ st.get(bm.lp(l_next).v).dir) == 0 {
                    let mut it = bm.lp(l_prev).next;
                    while it != l_next {
                        let vv = bm.lp(it).v;
                        st.get_mut(vv).skip = true;
                        it = bm.lp(it).next;
                    }
                }
                l_iter = bm.lp(l_next).prev;
            }
            l_iter = bm.lp(l_iter).next;
            if l_iter == l_first_nc {
                break;
            }
        }
    }
    let fno = bm.face(f).no;
    let (mut sort_dir, l) = bli::normalize_v3(bli::cross(fno, [plane[0], plane[1], plane[2]]));
    if l == 0.0 {
        let mut i = 0;
        while i < vert_split.len() {
            let (c0, ci) = (bm.vert(vert_split[0]).co, bm.vert(vert_split[i]).co);
            if c0 != ci {
                sort_dir = bli::normalize_v3(bli::sub(c0, ci)).0;
            }
            i += 1;
        }
        if i == vert_split.len() {
            return;
        }
    }
    for &v in &vert_split {
        let s = bli::dot(sort_dir, bm.vert(v).co);
        st.get_mut(v).sortval = s;
    }
    vert_split.sort_by(|&a, &b| st.get(a).sortval.total_cmp(&st.get(b).sortval));
    let mut face_split_arr: Vec<FaceId> = vec![f];
    for i in 0..vert_split.len() - 1 {
        let (v_a, v_b) = (vert_split[i], vert_split[i + 1]);
        if face_has_center_edge {
            let d = (st.get(v_a).loopindex as i64 - st.get(v_b).loopindex as i64).unsigned_abs() as u32;
            if d == 1 || d == f_len_orig - 1 {
                continue;
            }
        }
        if !st.get(v_a).skip {
            is_inside = !is_inside;
        }
        if !is_inside {
            continue;
        }
        let mut found = None;
        for (j, &fs) in face_split_arr.iter().enumerate() {
            if let (Some(a), Some(b)) = (bm.face_vert_share_loop(fs, v_a), bm.face_vert_share_loop(fs, v_b)) {
                found = Some((j, a, b));
                break;
            }
        }
        if let Some((j, l_a, l_b)) = found
            && !loop_is_adjacent(bm, l_a, l_b)
        {
            let fj = face_split_arr[j];
            if let Some((f_tmp, l_new)) = bm.face_split(fj, l_a, l_b, None, true) {
                flag_edge_face(bm, l_new);
                if f_tmp != fj {
                    face_split_arr.push(f_tmp);
                }
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/tools/bmesh_bisect_plane.cc (BM_mesh_bisect_plane).
/// `BM_mesh_bisect_plane(plane, use_snap_center, use_tag = true, oflag_center, oflag_new, eps)`:
/// cuts the tagged edges and faces (`BM_ELEM_TAG`) along `plane` (`a·x + d`, not normalised).
pub fn mesh_bisect_plane(
    bm: &mut BMesh,
    plane: [f32; 4],
    use_snap_center: bool,
    oflag_center: u16,
    oflag_new: u16,
    eps: f32,
) {
    let mut st = Bisect {
        info: FxHashMap::default(),
    };
    construct::verts_flag_disable_all(bm, ElemFlag::TAG);
    let mut edges_arr: Vec<EdgeId> = Vec::new();
    for e in bm.edge_ids() {
        if bm.edge(e).flag.contains(ElemFlag::TAG) {
            edges_arr.push(e);
            let [a, b] = bm.edge(e).v;
            bm.vert_mut(a).flag.insert(ElemFlag::TAG);
            bm.vert_mut(b).flag.insert(ElemFlag::TAG);
        }
    }
    let mut deferred: FxHashSet<FaceId> = FxHashSet::default();
    for v in bm.vert_ids() {
        let tagged = bm.vert(v).flag.contains(ElemFlag::TAG);
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        if !tagged {
            *st.get_mut(v) = VInfo::default();
            continue;
        }
        let co = bm.vert(v).co;
        let f = plane_point_side_v3(plane, co);
        let dir: i16 = if f <= -eps {
            -1
        } else if f >= eps {
            1
        } else {
            0
        };
        {
            let vi = st.get_mut(v);
            vi.dir = dir;
            vi.dist = f;
        }
        if dir == 0 {
            if oflag_center != 0 {
                bm.oflag_enable(v, oflag_center);
            }
            if use_snap_center {
                // `closest_to_plane_v3`.
                let pn = [plane[0], plane[1], plane[2]];
                let len_sq = bli::len_squared_v3(pn);
                let side = plane_point_side_v3(plane, co);
                let c = bli::madd_v3_v3v3fl(co, pn, -side / len_sq);
                if c != co {
                    bm.vert_mut(v).co = c;
                    for fv in bm.vert_faces(v) {
                        deferred.insert(fv);
                    }
                }
            }
        }
    }
    for f in deferred {
        bm.face_normal_update(f);
    }
    let mut face_stack: Vec<FaceId> = Vec::new();
    for e in edges_arr {
        let [v1, v2] = bm.edge(e).v;
        let side = [st.get(v1).dir, st.get(v2).dir];
        let dist = [st.get(v1).dist, st.get(v2).dist];
        if side[0] != 0 && side[1] != 0 && side[0] != side[1] {
            let e_fac = dist[0] / (dist[0] - dist[1]);
            for l in bm.edge_loops(e) {
                let f = bm.lp(l).f;
                if !face_in_stack(bm, f) {
                    bm.face_mut(f).flag.remove(ElemFlag::TAG);
                    face_stack.push(f);
                }
            }
            let (v_new, e_new) = bm.edge_split(e, v1, e_fac);
            if oflag_new != 0 {
                bm.oflag_enable(e_new, oflag_new);
            }
            bm.vert_mut(v_new).flag.insert(ElemFlag::TAG);
            if oflag_new | oflag_center != 0 {
                bm.oflag_enable(v_new, oflag_new | oflag_center);
            }
            let vi = st.get_mut(v_new);
            vi.dir = 0;
            vi.dist = 0.0;
        } else if side[0] == 0 || side[1] == 0 {
            for (j, v) in [v1, v2].into_iter().enumerate() {
                if side[j] == 0 && !is_center(bm, v) {
                    bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                    for l in bm.vert_loops(v) {
                        let f = bm.lp(l).f;
                        if !face_in_stack(bm, f) {
                            bm.face_mut(f).flag.remove(ElemFlag::TAG);
                            face_stack.push(f);
                        }
                    }
                }
            }
            if oflag_center != 0 && side[0] == 0 && side[1] == 0 {
                bm.oflag_enable(e, oflag_center);
            }
        }
    }
    while let Some(f) = face_stack.pop() {
        frap_math::budget::check();
        face_bisect_verts(bm, &mut st, f, plane, oflag_center, oflag_new);
    }
}

/// Runs `bisect_plane` on `bm`. `Zero normal given` is a [`BmError::Cancel`].
pub fn bisect_plane(bm: &mut BMesh, i: &BisectPlaneIn) -> Result<BisectPlaneOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!("bisect_plane: dead element {dead:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

// Ported from blender/source/blender/bmesh/operators/bmo_bisect_plane.cc (bmo_bisect_plane_exec).
fn exec(bm: &mut BMesh, i: &BisectPlaneIn) -> Result<BisectPlaneOut, BmError> {
    if bli::is_zero_v3(i.plane_no) {
        return Err(BmError::Cancel("Zero normal given".into()));
    }
    let n = i.plane_no;
    let plane = [n[0], n[1], n[2], -bli::dot(n, i.plane_co)];
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    construct::faces_flag_disable_all(bm, ElemFlag::TAG);
    for &g in &i.geom {
        match g {
            BMElem::Edge(e) => bm.edge_mut(e).flag.insert(ElemFlag::TAG),
            BMElem::Face(f) => bm.face_mut(f).flag.insert(ElemFlag::TAG),
            BMElem::Vert(_) => {}
        }
        bm.oflag_enable(g, ELE_INPUT);
    }
    mesh_bisect_plane(bm, plane, i.use_snap_center, ELE_CUT, ELE_NEW, i.dist);
    if i.clear_outer || i.clear_inner {
        let plane_outer = [plane[0], plane[1], plane[2], plane[3] - i.dist];
        let plane_inner = [plane[0], plane[1], plane[2], plane[3] + i.dist];
        let mut vert_arr: Vec<VertId> = Vec::new();
        for &g in &i.geom {
            if let BMElem::Vert(v) = g
                && bm.verts.contains(v.0)
            {
                let co = bm.vert(v).co;
                if (i.clear_outer && plane_point_side_v3(plane_outer, co) > 0.0)
                    || (i.clear_inner && plane_point_side_v3(plane_inner, co) < 0.0)
                {
                    vert_arr.push(v);
                }
            }
        }
        while let Some(v) = vert_arr.pop() {
            if bm.verts.contains(v.0) {
                bm.vert_kill(v);
            }
        }
    }
    let flags = ELE_NEW | ELE_INPUT;
    let mut geom: Vec<BMElem> = Vec::new();
    geom.extend(bm.vert_ids().into_iter().filter(|&v| bm.oflag_test(v, flags)).map(BMElem::Vert));
    geom.extend(bm.edge_ids().into_iter().filter(|&e| bm.oflag_test(e, flags)).map(BMElem::Edge));
    geom.extend(bm.face_ids().into_iter().filter(|&f| bm.oflag_test(f, flags)).map(BMElem::Face));
    let mut geom_cut: Vec<BMElem> = Vec::new();
    geom_cut.extend(bm.vert_ids().into_iter().filter(|&v| bm.oflag_test(v, ELE_CUT)).map(BMElem::Vert));
    geom_cut.extend(bm.edge_ids().into_iter().filter(|&e| bm.oflag_test(e, ELE_CUT)).map(BMElem::Edge));
    Ok(BisectPlaneOut { geom, geom_cut })
}
