// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_remap`: reorders the vertices, edges and faces of a BMesh in place.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_mesh.cc (`BM_mesh_remap`, Blender
//! 5.2.4, GPL-2.0-or-later).
//!
//! Blender copies every element struct into the mempool slot of the element whose current
//! index is the new index (`*verts_pool[new_idx] = copy_of_old`), so the set of memory
//! slots stays the same and only their contents move; the custom data blocks travel with the
//! element header. frap's elements are pool slots, so the same permutation is applied to the
//! pool slots and to every per-slot column (custom data, deform weights, shape-key layers,
//! the shape key index), then every reference (disk links, loop pointers, the selection
//! history and the active face) is rewritten. Loops do not move.

use frap_bmesh::customdata::CustomData;
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemTypes, FaceId, Pool, VertId};

/// `dst slot -> src slot` for a pool whose live slots in iteration order are `slots` and whose
/// element at iteration index `i` moves to iteration index `map[i]`; identity elsewhere.
fn src_of_dst(slots: &[u32], map: &[u32], capacity: usize) -> Vec<usize> {
    let mut src: Vec<usize> = (0..capacity).collect();
    for (i, &m) in map.iter().enumerate() {
        src[slots[m as usize] as usize] = slots[i] as usize;
    }
    src
}

/// `old slot -> new slot` (identity for free slots).
fn new_of_old(slots: &[u32], map: &[u32], capacity: usize) -> Vec<u32> {
    let mut n: Vec<u32> = (0..capacity as u32).collect();
    for (i, &m) in map.iter().enumerate() {
        n[slots[i] as usize] = slots[m as usize];
    }
    n
}

fn permute_pool<T: Clone>(pool: &mut Pool<T>, src: &[usize]) {
    let old = pool.slots.clone();
    for (d, &s) in src.iter().enumerate() {
        if d != s {
            pool.slots[d] = old[s].clone();
        }
    }
}

fn permute_cd(cd: &mut CustomData, src: &[usize]) {
    cd.ensure_slots(src.len());
    for l in &mut cd.layers {
        let len = l.data.len();
        let idx: Vec<usize> = (0..len).map(|d| src.get(d).copied().unwrap_or(d)).collect();
        l.data = l.data.gather_with_default(&idx, len);
    }
    if let Some(dv) = &mut cd.deform {
        let old = dv.clone();
        for (d, &s) in src.iter().enumerate() {
            if d < dv.len() && s < old.len() {
                dv[d] = old[s].clone();
            }
        }
    }
}

fn permute_vec<T: Clone>(v: &mut [T], src: &[usize]) {
    let old = v.to_vec();
    for (d, &s) in src.iter().enumerate() {
        if d < v.len() && s < old.len() {
            v[d] = old[s].clone();
        }
    }
}

/// `BM_mesh_remap(bm, vert_idx, edge_idx, face_idx)`: `map[i]` is the new index of the
/// element whose index (iteration order) is `i`; `None` leaves that type alone.
pub(crate) fn mesh_remap(
    bm: &mut BMesh,
    vert_idx: Option<&[u32]>,
    edge_idx: Option<&[u32]>,
    face_idx: Option<&[u32]>,
) {
    if vert_idx.is_none() && edge_idx.is_none() && face_idx.is_none() {
        return;
    }
    let mut vmap: Option<Vec<u32>> = None;
    let mut emap: Option<Vec<u32>> = None;
    let mut fmap: Option<Vec<u32>> = None;

    if let Some(map) = vert_idx {
        let slots: Vec<u32> = bm.verts.ids().collect();
        let cap = bm.verts.capacity();
        let src = src_of_dst(&slots, map, cap);
        permute_pool(&mut bm.verts, &src);
        permute_cd(&mut bm.vdata, &src);
        for s in &mut bm.shape_layers {
            if s.co.len() < cap {
                s.co.resize(cap, [0.0; 3]);
            }
            permute_vec(&mut s.co, &src);
        }
        if let Some(ki) = &mut bm.shape_keyindex {
            if ki.len() < cap {
                ki.resize(cap, 0);
            }
            permute_vec(ki, &src);
        }
        vmap = Some(new_of_old(&slots, map, cap));
        bm.index.dirty |= ElemTypes::VERT;
    }
    if let Some(map) = edge_idx {
        let slots: Vec<u32> = bm.edges.ids().collect();
        let cap = bm.edges.capacity();
        let src = src_of_dst(&slots, map, cap);
        permute_pool(&mut bm.edges, &src);
        permute_cd(&mut bm.edata, &src);
        emap = Some(new_of_old(&slots, map, cap));
        bm.index.dirty |= ElemTypes::EDGE;
    }
    if let Some(map) = face_idx {
        let slots: Vec<u32> = bm.faces.ids().collect();
        let cap = bm.faces.capacity();
        let src = src_of_dst(&slots, map, cap);
        permute_pool(&mut bm.faces, &src);
        permute_cd(&mut bm.pdata, &src);
        fmap = Some(new_of_old(&slots, map, cap));
        bm.index.dirty |= ElemTypes::FACE | ElemTypes::LOOP;
    }

    let mv = |v: VertId, m: &Option<Vec<u32>>| m.as_ref().map_or(v, |m| VertId(m[v.slot()]));
    let me = |e: EdgeId, m: &Option<Vec<u32>>| m.as_ref().map_or(e, |m| EdgeId(m[e.slot()]));
    let mf = |f: FaceId, m: &Option<Vec<u32>>| m.as_ref().map_or(f, |m| FaceId(m[f.slot()]));

    // Verts' pointers, only edge pointers.
    if emap.is_some() {
        for v in bm.verts.slots.iter_mut().flatten() {
            if let Some(e) = v.e {
                v.e = Some(me(e, &emap));
            }
        }
    }
    // Edges' pointers: vertices and disk links.
    if vmap.is_some() || emap.is_some() {
        for e in bm.edges.slots.iter_mut().flatten() {
            e.v = [mv(e.v[0], &vmap), mv(e.v[1], &vmap)];
            for d in &mut e.disk {
                d.next = me(d.next, &emap);
                d.prev = me(d.prev, &emap);
            }
        }
    }
    // Loops: vertex, edge and face pointers.
    for l in bm.loops.slots.iter_mut().flatten() {
        l.v = mv(l.v, &vmap);
        l.e = me(l.e, &emap);
        l.f = mf(l.f, &fmap);
    }
    // Selection history.
    for ese in &mut bm.select_history {
        *ese = match *ese {
            BMElem::Vert(v) => BMElem::Vert(mv(v, &vmap)),
            BMElem::Edge(e) => BMElem::Edge(me(e, &emap)),
            BMElem::Face(f) => BMElem::Face(mf(f, &fmap)),
        };
    }
    // Active face (`bm->act_face`).
    if let Some(f) = bm.act_face {
        bm.act_face = Some(mf(f, &fmap));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_maps_are_inverse() {
        // Slots 0, 2, 3 live; element 0 → index 2, 1 → 0, 2 → 1.
        let slots = [0u32, 2, 3];
        let map = [2u32, 0, 1];
        let src = src_of_dst(&slots, &map, 4);
        let new = new_of_old(&slots, &map, 4);
        assert_eq!(src, vec![2, 1, 3, 0]);
        assert_eq!(new, vec![3, 1, 0, 2]);
        for (d, &s) in src.iter().enumerate() {
            assert_eq!(new[s] as usize, d);
        }
    }
}
