// SPDX-License-Identifier: GPL-3.0-or-later
//! The BMesh kernel steps of `mesh.rip` that `frap_bmesh` keeps private: separating a set of
//! corners of one vertex into a new vertex (`bmesh_kernel_unglue_region_make_vert_multi`,
//! `BM_face_loop_separate_multi`) and moving a whole connected fan to a new vertex
//! (`bmesh_kernel_unglue_region_make_vert_multi_isolated`,
//! `BM_face_loop_separate_multi_isolated`), with the disk and radial cycle primitives they
//! need, written on the public element fields of [`BMesh`] exactly as `frap_bmesh::kernel`
//! implements them.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_core.cc, bmesh_structure.cc and
//! bmesh_mods.cc (Blender 5.2.4, GPL-2.0-or-later).

use frap_bmesh::{BMesh, CreateFlag, DiskLink, EdgeId, LoopId, VertId};
use std::collections::HashSet;

fn disk_link(bm: &BMesh, e: EdgeId, v: VertId) -> DiskLink {
    let ed = bm.edge(e);
    ed.disk[(ed.v[1] == v) as usize]
}
fn disk_link_mut(bm: &mut BMesh, e: EdgeId, v: VertId) -> &mut DiskLink {
    let ed = bm.edge_mut(e);
    let i = (ed.v[1] == v) as usize;
    &mut ed.disk[i]
}

/// `bmesh_disk_edge_append`: inserts `e` before `v->e` in the disk cycle of `v`.
fn disk_edge_append(bm: &mut BMesh, e: EdgeId, v: VertId) {
    match bm.vert(v).e {
        None => {
            *disk_link_mut(bm, e, v) = DiskLink { next: e, prev: e };
            bm.vert_mut(v).e = Some(e);
        }
        Some(ve) => {
            let dl2 = disk_link(bm, ve, v);
            *disk_link_mut(bm, e, v) = DiskLink {
                next: ve,
                prev: dl2.prev,
            };
            disk_link_mut(bm, ve, v).prev = e;
            disk_link_mut(bm, dl2.prev, v).next = e;
        }
    }
}

/// `bmesh_disk_edge_remove`.
fn disk_edge_remove(bm: &mut BMesh, e: EdgeId, v: VertId) {
    let dl1 = disk_link(bm, e, v);
    if dl1.prev != e {
        disk_link_mut(bm, dl1.prev, v).next = dl1.next;
    }
    if dl1.next != e {
        disk_link_mut(bm, dl1.next, v).prev = dl1.prev;
    }
    if bm.vert(v).e == Some(e) {
        bm.vert_mut(v).e = if e != dl1.next { Some(dl1.next) } else { None };
    }
    *disk_link_mut(bm, e, v) = DiskLink { next: e, prev: e };
}

/// `bmesh_disk_vert_replace`.
fn disk_vert_replace(bm: &mut BMesh, e: EdgeId, v_dst: VertId, v_src: VertId) {
    disk_edge_remove(bm, e, v_src);
    let ed = bm.edge_mut(e);
    let i = if ed.v[0] == v_src { 0 } else { 1 };
    ed.v[i] = v_dst;
    ed.disk[i] = DiskLink { next: e, prev: e };
    disk_edge_append(bm, e, v_dst);
}

/// `bmesh_radial_loop_append`.
fn radial_loop_append(bm: &mut BMesh, e: EdgeId, l: LoopId) {
    match bm.edge(e).l {
        None => {
            let ll = bm.lp_mut(l);
            ll.radial_next = l;
            ll.radial_prev = l;
        }
        Some(el) => {
            let el_next = bm.lp(el).radial_next;
            {
                let ll = bm.lp_mut(l);
                ll.radial_prev = el;
                ll.radial_next = el_next;
            }
            bm.lp_mut(el_next).radial_prev = l;
            bm.lp_mut(el).radial_next = l;
        }
    }
    bm.edge_mut(e).l = Some(l);
    bm.lp_mut(l).e = e;
}

/// `bmesh_radial_loop_remove`.
fn radial_loop_remove(bm: &mut BMesh, e: EdgeId, l: LoopId) {
    let (rn, rp) = (bm.lp(l).radial_next, bm.lp(l).radial_prev);
    if rn != l {
        if bm.edge(e).l == Some(l) {
            bm.edge_mut(e).l = Some(rn);
        }
        bm.lp_mut(rn).radial_prev = rp;
        bm.lp_mut(rp).radial_next = rn;
    } else if bm.edge(e).l == Some(l) {
        bm.edge_mut(e).l = None;
    }
    let ll = bm.lp_mut(l);
    ll.radial_next = l;
    ll.radial_prev = l;
}

/// `bmesh_kernel_unglue_region_make_vert_multi` (`BM_face_loop_separate_multi`): the corners
/// `larr` (all of one vertex) get a new vertex; edges used by them and by other corners are
/// duplicated, edges used only by them move to the new vertex. When the corners own every
/// edge of the vertex nothing is split and the vertex itself is returned.
pub(crate) fn face_loop_separate_multi(bm: &mut BMesh, larr: &[LoopId]) -> VertId {
    let v_sep = bm.lp(larr[0]).v;
    let mut is_mixed_edge_any = false;
    let mut is_mixed_loop_any = false;
    // `LOOP_VISIT` / `EDGE_VISIT`.
    let mut loop_visit: HashSet<LoopId> = HashSet::new();
    let mut edge_visit: HashSet<EdgeId> = HashSet::new();
    for &l_sep in larr {
        let (prev, next) = (bm.lp(l_sep).prev, bm.lp(l_sep).next);
        loop_visit.insert(l_sep);
        // Weak, but it simplifies the edge checks during the radial loop below.
        loop_visit.insert(next);
        loop_visit.insert(prev);
        for lp in [l_sep, prev] {
            edge_visit.insert(bm.lp(lp).e);
        }
    }
    let mut edges: Vec<EdgeId> = Vec::new();
    if let Some(e_first) = bm.vert(v_sep).e {
        let mut e_iter = e_first;
        loop {
            if edge_visit.contains(&e_iter) {
                let l_first = bm.edge(e_iter).l.expect("edge of a corner has loops");
                let mut l_iter = l_first;
                let mut is_mixed_loop = false;
                loop {
                    if !loop_visit.contains(&l_iter) {
                        is_mixed_loop = true;
                        break;
                    }
                    l_iter = bm.lp(l_iter).radial_next;
                    if l_iter == l_first {
                        break;
                    }
                }
                if is_mixed_loop {
                    // Make the first loop one we do not own, for the quick check below.
                    bm.edge_mut(e_iter).l = Some(l_iter);
                    is_mixed_loop_any = true;
                }
                edges.push(e_iter);
            } else {
                // At least one edge attached is not connected to our loops.
                is_mixed_edge_any = true;
            }
            e_iter = bm.disk_edge_next(e_iter, v_sep);
            if e_iter == e_first {
                break;
            }
        }
    }
    let v_new = if !is_mixed_loop_any && !is_mixed_edge_any {
        // The corners own their edges: nothing to split away from.
        v_sep
    } else {
        let co = bm.vert(v_sep).co;
        let v_new = bm.vert_create(co, Some(v_sep), CreateFlag::empty());
        for &e in &edges {
            let el = bm.edge(e).l.expect("loops");
            if !loop_visit.contains(&el) {
                // Some loops owned by us, some by other corners.
                let [a, b] = bm.edge(e).v;
                let (v1, v2) = if a == v_sep { (v_new, b) } else { (a, v_new) };
                let e_new = bm.edge_create(v1, v2, Some(e), CreateFlag::empty());
                // Move the loops of `larr` to the new edge.
                let l_first = el;
                let mut l_iter = l_first;
                loop {
                    let l_next = bm.lp(l_iter).radial_next;
                    if loop_visit.contains(&l_iter) {
                        radial_loop_remove(bm, e, l_iter);
                        radial_loop_append(bm, e_new, l_iter);
                        bm.lp_mut(l_iter).e = e_new;
                    }
                    l_iter = l_next;
                    if l_iter == l_first {
                        break;
                    }
                }
            } else {
                // We own the edge entirely: replace the vertex.
                disk_vert_replace(bm, e, v_new, v_sep);
            }
        }
        v_new
    };
    // The corner vertices are handled last.
    for &l_sep in larr {
        bm.lp_mut(l_sep).v = v_new;
    }
    v_new
}

/// `bmesh_edge_vert_swap__recursive`.
fn edge_vert_swap_recursive(bm: &mut BMesh, e: EdgeId, v_dst: VertId, v_src: VertId) {
    disk_vert_replace(bm, e, v_dst, v_src);
    let Some(l_first) = bm.edge(e).l else {
        return;
    };
    let mut l_iter = l_first;
    loop {
        let (lv, prev, next) = {
            let l = bm.lp(l_iter);
            (l.v, l.prev, l.next)
        };
        if lv == v_src {
            bm.lp_mut(l_iter).v = v_dst;
            let pe = bm.lp(prev).e;
            if bm.edge(pe).v.contains(&v_src) {
                edge_vert_swap_recursive(bm, pe, v_dst, v_src);
            }
        } else if bm.lp(next).v == v_src {
            bm.lp_mut(next).v = v_dst;
            let ne = bm.lp(next).e;
            if bm.edge(ne).v.contains(&v_src) {
                edge_vert_swap_recursive(bm, ne, v_dst, v_src);
            }
        }
        l_iter = bm.lp(l_iter).radial_next;
        if l_iter == l_first {
            break;
        }
    }
}

/// `bmesh_kernel_unglue_region_make_vert_multi_isolated`
/// (`BM_face_loop_separate_multi_isolated`): the fan of faces connected to `l_sep` around its
/// vertex moves to a new vertex.
pub(crate) fn face_loop_separate_multi_isolated(bm: &mut BMesh, l_sep: LoopId) -> VertId {
    let v = bm.lp(l_sep).v;
    let co = bm.vert(v).co;
    let v_new = bm.vert_create(co, Some(v), CreateFlag::empty());
    let e = bm.lp(l_sep).e;
    edge_vert_swap_recursive(bm, e, v_new, v);
    v_new
}
