// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.weld_verts` (Blender 5.2.4 `bmo_weld_verts_exec`, `operators/bmo_removedoubles.cc`),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.weld_verts(bmesh, targetmap={}, use_centroid=False, average_vert_data=False)`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/
//! bmo_removedoubles.cc): every mapped vertex is flagged for deletion (its flags merged into
//! its target, `HIDDEN` only when both are hidden); with `use_centroid` each target moves to
//! the centroid of its cluster, and with `average_vert_data` (implied by `use_centroid`) its
//! vertex data becomes the cluster average. Faces whose own corners would merge are split
//! first (`BM_face_split`, recursively). Then, in edge order, every edge touching a mapped
//! vertex is flagged; when it does not collapse to a point, the edge between the targets is
//! looked up or **created** (`BM_edge_create` with the old edge as example). Then, in face
//! order, every face with a mapped vertex is rebuilt: the new face is created over the target
//! vertices (or an existing identical face reused), its loops take the old corners' data, and
//! when it is new it **swaps its geometry into the old face's slot** and is freed at once
//! (so the face keeps its index). The selection history follows the map. Finally the flagged
//! faces, edges and vertices are killed in that order (`DEL_ONLYTAGGED`), so the freed slots
//! are reused LIFO by later creations ([MESH App. B.6]).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use rustc_hash::FxHashMap;

use crate::kernel::DeleteContext;
use crate::ops::dissolve_verts::mods;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, CreateFlag, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `weld_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeldVertsIn {
    /// `targetmap` (vertex -> target vertex).
    pub targetmap: Vec<(VertId, VertId)>,
    /// `use_centroid`.
    pub use_centroid: bool,
    /// `average_vert_data`.
    pub average_vert_data: bool,
}

/// Output slots of `weld_verts` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct WeldVertsOut {
    /// Always empty (`bmesh.ops.weld_verts` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_weld_verts_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const ELE_DEL: u16 = 1;
const EDGE_COL: u16 = 2;
const VERT_IN_FACE: u16 = 4;

/// The target map as Blender's slot map (`GHash`: a later entry for the same key replaces
/// the earlier one).
pub(crate) type TargetMap = FxHashMap<VertId, VertId>;

/// Runs `weld_verts` on `bm`.
pub fn weld_verts(bm: &mut BMesh, i: &WeldVertsIn) -> Result<WeldVertsOut, BmError> {
    for &(a, b) in &i.targetmap {
        if !bm.verts.contains(a.0) || !bm.verts.contains(b.0) {
            return Err(BmError::Invalid(format!(
                "weld_verts: dead vertex in targetmap ({a:?} -> {b:?})"
            )));
        }
    }
    let map: TargetMap = i.targetmap.iter().copied().collect();
    bm.bmo_exec(TYPE_FLAG, |bm| {
        weld_exec(bm, &map, i.use_centroid, i.average_vert_data);
    });
    Ok(WeldVertsOut::default())
}

/// `remdoubles_splitface`: splits `f` while two of its non-adjacent corners map to each
/// other.
fn remdoubles_splitface(bm: &mut BMesh, f: FaceId, map: &TargetMap) {
    let mut split: Option<(LoopId, LoopId)> = None;
    for l in bm.face_loops(f) {
        let Some(&v_tar) = map.get(&bm.lp(l).v) else {
            continue;
        };
        if let Some(l_tar) = bm.face_vert_share_loop(f, v_tar)
            && l_tar != l
            && !mods::loop_is_adjacent(bm, l_tar, l)
        {
            split = Some((l, l_tar));
            break;
        }
    }
    if let Some((l_double, l_tar)) = split
        && let Some((f_new, _)) = bm.face_split(f, l_double, l_tar, None, false)
    {
        remdoubles_splitface(bm, f, map);
        remdoubles_splitface(bm, f_new, map);
    }
}

/// `remdoubles_createface`: the face `f` becomes over the mapped vertices; `(face, created)`.
fn remdoubles_createface(bm: &mut BMesh, f: FaceId, map: &TargetMap) -> Option<(FaceId, bool)> {
    let map_vert = |bm: &BMesh, l: LoopId| -> (VertId, bool) {
        let v = bm.lp(l).v;
        let is_del = bm.oflag_test(v, ELE_DEL);
        (if is_del { map[&v] } else { v }, is_del)
    };
    let mut edges: Vec<EdgeId> = Vec::new();
    let mut loops: Vec<LoopId> = Vec::new();
    let mut verts: Vec<VertId> = Vec::new();
    let l_first = bm.face(f).l_first;
    let mut l_curr = l_first;
    let (mut v_curr, mut is_del_v_curr) = map_vert(bm, l_curr);
    let mut bail = false;
    loop {
        let l_next = bm.lp(l_curr).next;
        let (v_next, is_del_v_next) = map_vert(bm, l_next);
        let e_new = if !(is_del_v_curr || is_del_v_next) {
            Some(bm.lp(l_curr).e)
        } else if v_curr == v_next {
            None
        } else {
            bm.edge_exists(v_curr, v_next)
        };
        if let Some(e_new) = e_new {
            if bm.oflag_test(v_curr, VERT_IN_FACE) {
                // We can't make the face, bail out.
                edges.clear();
                bail = true;
                break;
            }
            bm.oflag_enable(v_curr, VERT_IN_FACE);
            edges.push(e_new);
            loops.push(l_curr);
            verts.push(v_curr);
        }
        v_curr = v_next;
        is_del_v_curr = is_del_v_next;
        l_curr = l_next;
        if l_curr == l_first {
            break;
        }
    }
    for &v in &verts {
        bm.oflag_disable(v, VERT_IN_FACE);
    }
    if bail || edges.len() < 3 {
        return None;
    }
    if let Some(f_exist) = bm.face_exists(&verts) {
        return Some((f_exist, false));
    }
    let f_new = bm.face_create(&verts, &edges, Some(f), CreateFlag::empty())?;
    for (k, l_new) in bm.face_loops(f_new).into_iter().enumerate() {
        bm.loop_attrs_copy(loops[k], l_new);
    }
    Some((f_new, true))
}

/// The body of `bmo_weld_verts_exec` (inside the operator's flag layer).
pub(crate) fn weld_exec(
    bm: &mut BMesh,
    map: &TargetMap,
    use_centroid: bool,
    average_vert_data: bool,
) {
    let average_vert_data = average_vert_data || use_centroid;
    // Maintain selection history.
    let has_selected = !bm.select_history.is_empty();
    let mut targetmap_all: FxHashMap<BMElem, BMElem> = FxHashMap::default();
    // Clusters by survivor, in first-seen order (values in vertex order).
    let mut cluster_keys: Vec<VertId> = Vec::new();
    let mut clusters: FxHashMap<VertId, Vec<VertId>> = FxHashMap::default();

    // Mark merge verts for deletion.
    for v in bm.vert_ids() {
        let Some(&v_dst) = map.get(&v) else {
            continue;
        };
        bm.oflag_enable(v, ELE_DEL);
        // Merge the vertex flags, else we get randomly selected/unselected verts.
        mods::elem_flag_merge_ex(bm, BMElem::Vert(v), BMElem::Vert(v_dst), ElemFlag::HIDDEN);
        if has_selected {
            targetmap_all.insert(BMElem::Vert(v), BMElem::Vert(v_dst));
        }
        if average_vert_data && v_dst != v {
            clusters
                .entry(v_dst)
                .or_insert_with(|| {
                    cluster_keys.push(v_dst);
                    Vec::new()
                })
                .push(v);
        }
    }

    if use_centroid {
        for &v_dst in &cluster_keys {
            let mut centroid = bm.vert(v_dst).co;
            let mut count = 1;
            for &d in &clusters[&v_dst] {
                let c = bm.vert(d).co;
                centroid = [centroid[0] + c[0], centroid[1] + c[1], centroid[2] + c[2]];
                count += 1;
            }
            let s = 1.0 / count as f32;
            bm.vert_mut(v_dst).co = [centroid[0] * s, centroid[1] * s, centroid[2] * s];
        }
    }
    if average_vert_data {
        for &v_dst in &cluster_keys {
            let mut src = vec![v_dst];
            src.extend(clusters[&v_dst].iter().copied());
            // `CustomData_bmesh_interp` with no weights: `1 / count` each.
            let w = vec![1.0 / src.len() as f32; src.len()];
            bm.vert_data_interp(&src, &w, v_dst);
        }
    }

    // Check if any faces are getting their own corners merged together, split face if so.
    for f in bm.face_ids() {
        if bm.faces.contains(f.0) {
            frap_math::budget::check();
            remdoubles_splitface(bm, f, map);
        }
    }

    for e in bm.edge_ids() {
        let [ev1, ev2] = bm.edge(e).v;
        let is_del_v1 = bm.oflag_test(ev1, ELE_DEL);
        let is_del_v2 = bm.oflag_test(ev2, ELE_DEL);
        if !(is_del_v1 || is_del_v2) {
            continue;
        }
        let v1 = if is_del_v1 { map[&ev1] } else { ev1 };
        let v2 = if is_del_v2 { map[&ev2] } else { ev2 };
        if v1 == v2 {
            bm.oflag_enable(e, EDGE_COL);
        } else {
            // Always merge flags, even for edges we already created.
            let e_new = match bm.edge_exists(v1, v2) {
                Some(x) => x,
                None => bm.edge_create(v1, v2, Some(e), CreateFlag::empty()),
            };
            mods::elem_flag_merge_ex(bm, BMElem::Edge(e_new), BMElem::Edge(e), ElemFlag::HIDDEN);
            if has_selected {
                targetmap_all.insert(BMElem::Edge(e), BMElem::Edge(e_new));
            }
        }
        bm.oflag_enable(e, ELE_DEL);
    }

    // Faces get "modified" by creating new faces here, then at the end the old faces are
    // deleted.
    for f in bm.face_ids() {
        if !bm.faces.contains(f.0) {
            continue;
        }
        frap_math::budget::check();
        let mut vert_delete = false;
        let mut edge_collapse = 0usize;
        for l in bm.face_loops(f) {
            if bm.oflag_test(bm.lp(l).v, ELE_DEL) {
                vert_delete = true;
            }
            if bm.oflag_test(bm.lp(l).e, EDGE_COL) {
                edge_collapse += 1;
            }
        }
        if !vert_delete {
            continue;
        }
        let mut use_in_place = false;
        let mut f_new = None;
        bm.oflag_enable(f, ELE_DEL);
        if (bm.face(f).len as usize).saturating_sub(edge_collapse) >= 3
            && let Some((fx, created)) = remdoubles_createface(bm, f, map)
        {
            f_new = Some(fx);
            if created {
                mods::face_swap_data(bm, fx, f);
                bm.oflag_disable(f, ELE_DEL);
                bm.face_kill(fx);
                use_in_place = true;
            } else {
                mods::elem_flag_merge_ex(bm, BMElem::Face(fx), BMElem::Face(f), ElemFlag::HIDDEN);
            }
        }
        if !use_in_place && let Some(fx) = f_new {
            debug_assert_ne!(f, fx);
            if has_selected {
                targetmap_all.insert(BMElem::Face(f), BMElem::Face(fx));
            }
            if bm.act_face == Some(f) {
                bm.act_face = Some(fx);
            }
        }
    }

    if has_selected {
        mods::select_history_merge_from_targetmap(bm, &targetmap_all);
    }
    bm.delete_oflag_context(ELE_DEL, DeleteContext::TaggedOnly);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(bm: &BMesh) -> Result<(), Vec<String>> {
        let e = bm.validate();
        if e.is_empty() { Ok(()) } else { Err(e) }
    }

    /// Two quads side by side whose shared edge is duplicated (`v1`/`v4` and `v2`/`v5` are
    /// doubles).
    fn split_quads() -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let co = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        let v: Vec<VertId> = co
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        bm.face_create_verts(&[v[0], v[1], v[2], v[3]], None, CreateFlag::empty(), true);
        bm.face_create_verts(&[v[4], v[6], v[7], v[5]], None, CreateFlag::empty(), true);
        (bm, v)
    }

    #[test]
    fn welding_doubles_joins_two_quads() {
        let (mut bm, v) = split_quads();
        let i = WeldVertsIn {
            targetmap: vec![(v[4], v[1]), (v[5], v[2])],
            ..Default::default()
        };
        weld_verts(&mut bm, &i).unwrap();
        assert_eq!(bm.verts_num(), 6);
        assert_eq!(bm.edges_num(), 7);
        assert_eq!(bm.faces_num(), 2);
        validate(&bm).unwrap();
        // The second face kept its slot and now uses v1/v2.
        let f1 = crate::FaceId(1);
        let verts = bm.face_verts(f1);
        assert!(verts.contains(&v[1]) && verts.contains(&v[2]));
    }

    #[test]
    fn collapsing_an_edge_turns_a_quad_into_a_triangle() {
        let (mut bm, v) = split_quads();
        let i = WeldVertsIn {
            targetmap: vec![(v[3], v[2])],
            use_centroid: true,
            ..Default::default()
        };
        weld_verts(&mut bm, &i).unwrap();
        validate(&bm).unwrap();
        assert_eq!(bm.face(crate::FaceId(0)).len, 3);
        assert_eq!(bm.vert(v[2]).co, [0.5, 1.0, 0.0]);
    }

    #[test]
    fn dead_vertex_is_refused() {
        let (mut bm, v) = split_quads();
        bm.vert_kill(v[7]);
        let i = WeldVertsIn {
            targetmap: vec![(v[7], v[0])],
            ..Default::default()
        };
        assert!(weld_verts(&mut bm, &i).is_err());
    }
}
