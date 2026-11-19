// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex merging on a `Mesh` (Blender 5.2.4 `geometry/intern/mesh_merge_verts.cc`): the weld
//! context (collapsed edges, duplicate edges, faces split where a vertex repeats, duplicate
//! faces), the rebuilt mesh with mixed or copied custom data, and the two merge-map builders
//! the WELD modifier uses (`mesh_merge_by_distance_all` over `kdtree_calc_duplicates_fast`,
//! `mesh_merge_by_distance_connected`). MIRROR and ARRAY call [`mesh_merge_verts`] with their
//! own maps.
//!
//! Ported from blender/source/blender/geometry/intern/mesh_merge_verts.cc and
//! blender/source/blender/blenlib/BLI_kdtree.hh (`kdtree_calc_duplicates_fast`,
//! `kdtree_balance`).

use frap_mesh::Mesh;
use smallvec::SmallVec;

use super::util::{self, Carry, DomainMaps, Topology};

/// Indicates when the element was not computed.
const OUT_OF_CONTEXT: i32 = -1;
/// Indicates if the edge or face will be collapsed.
const ELEM_COLLAPSED: i32 = -2;

#[derive(Copy, Clone, Debug)]
struct WeldEdge {
    edge_orig: i32,
    vert_a: i32,
    vert_b: i32,
}

/// `WeldLoop`; Blender's `flag` aliases `edge` (a union), so a collapsed loop has
/// `edge == ELEM_COLLAPSED`.
#[derive(Copy, Clone, Debug)]
struct WeldLoop {
    edge: i32,
    vert: i32,
    loop_orig: i32,
    loop_next: i32,
}

/// `WeldPoly`; `flag` aliases `poly_dst`.
#[derive(Copy, Clone, Debug)]
struct WeldPoly {
    poly_dst: i32,
    poly_orig: i32,
    loop_start: i32,
    loop_end: i32,
    loop_ctx_start: i32,
    loop_ctx_len: i32,
}

#[derive(Default)]
struct WeldMesh {
    double_verts: Vec<i32>,
    double_edges: Vec<i32>,
    edge_dest_map: Vec<i32>,
    wloop: Vec<WeldLoop>,
    wpoly: Vec<WeldPoly>,
    wpoly_new_len: usize,
    loop_map: Vec<i32>,
    face_map: Vec<i32>,
    vert_kill_len: usize,
    edge_kill_len: usize,
    loop_kill_len: usize,
    face_kill_len: usize,
}

/// `WeldLoopOfPolyIter`.
struct LoopIter<'a> {
    loop_iter: i32,
    loop_end: i32,
    loop_ctx_start: i32,
    loop_ctx_len: i32,
    want_group: bool,
    group: SmallVec<[i32; 8]>,
    wloop: &'a [WeldLoop],
    corner_verts: &'a [i32],
    corner_edges: &'a [i32],
    loop_map: &'a [i32],
    v: i32,
    e: i32,
}

impl<'a> LoopIter<'a> {
    fn begin(
        wp: &WeldPoly,
        wloop: &'a [WeldLoop],
        corner_verts: &'a [i32],
        corner_edges: &'a [i32],
        loop_map: &'a [i32],
        want_group: bool,
    ) -> Option<LoopIter<'a>> {
        if wp.poly_dst == ELEM_COLLAPSED {
            return None;
        }
        let mut it = LoopIter {
            loop_iter: wp.loop_start,
            loop_end: wp.loop_end,
            loop_ctx_start: wp.loop_ctx_start,
            loop_ctx_len: wp.loop_ctx_len,
            want_group,
            group: SmallVec::new(),
            wloop,
            corner_verts,
            corner_edges,
            loop_map,
            v: OUT_OF_CONTEXT,
            e: OUT_OF_CONTEXT,
        };
        if it.next() { Some(it) } else { None }
    }

    fn next(&mut self) -> bool {
        if self.loop_iter > self.loop_end {
            return false;
        }
        let l = self.loop_iter;
        let mut l_next = l + 1;
        let loop_ctx = self.loop_map[l as usize];
        if loop_ctx != OUT_OF_CONTEXT {
            let wl = self.wloop[loop_ctx as usize];
            self.v = wl.vert;
            self.e = wl.edge;
            if wl.loop_next > l {
                // Allow the loop to break.
                l_next = wl.loop_next;
            }
            if self.want_group {
                self.group.clear();
                let start = self.loop_ctx_start as usize;
                for w in &self.wloop[start..start + self.loop_ctx_len as usize] {
                    if w.vert == self.v {
                        self.group.push(w.loop_orig);
                    }
                }
            }
        } else {
            self.v = self.corner_verts[l as usize];
            self.e = self.corner_edges[l as usize];
            if self.want_group {
                self.group.clear();
                self.group.push(l);
            }
        }
        self.loop_iter = l_next;
        true
    }
}

// ---------------------------------------------------------------------------- vert API

fn weld_vert_ctx_alloc_and_setup(vert_dest_map: &mut [i32], vert_kill_len: usize) -> Vec<i32> {
    let mut wvert = Vec::with_capacity((2 * vert_kill_len).min(vert_dest_map.len()));
    for i in 0..vert_dest_map.len() {
        frap_math::budget::check();
        if vert_dest_map[i] != OUT_OF_CONTEXT {
            let vert_dest = vert_dest_map[i];
            wvert.push(i as i32);
            if vert_dest_map[vert_dest as usize] != vert_dest {
                // The target vertex is also part of the context and needs to be referenced.
                vert_dest_map[vert_dest as usize] = vert_dest;
                wvert.push(vert_dest);
            }
        }
    }
    wvert
}

// ---------------------------------------------------------------------------- edge API

fn weld_edge_ctx_alloc_and_find_collapsed(
    edges: &[[i32; 2]],
    vert_dest_map: &[i32],
    r_edge_dest_map: &mut [i32],
) -> (Vec<WeldEdge>, usize) {
    let mut edge_collapsed_len = 0;
    let mut wedge = Vec::with_capacity(edges.len());
    for (i, e) in edges.iter().enumerate() {
        frap_math::budget::check();
        let (v1, v2) = (e[0], e[1]);
        let v_dest_1 = vert_dest_map[v1 as usize];
        let v_dest_2 = vert_dest_map[v2 as usize];
        if v_dest_1 == OUT_OF_CONTEXT && v_dest_2 == OUT_OF_CONTEXT {
            r_edge_dest_map[i] = OUT_OF_CONTEXT;
            continue;
        }
        let vert_a = if v_dest_1 == OUT_OF_CONTEXT {
            v1
        } else {
            v_dest_1
        };
        let vert_b = if v_dest_2 == OUT_OF_CONTEXT {
            v2
        } else {
            v_dest_2
        };
        if vert_a == vert_b {
            r_edge_dest_map[i] = ELEM_COLLAPSED;
            edge_collapsed_len += 1;
        } else {
            wedge.push(WeldEdge {
                edge_orig: i as i32,
                vert_a,
                vert_b,
            });
            r_edge_dest_map[i] = i as i32;
        }
    }
    (wedge, edge_collapsed_len)
}

fn weld_edge_find_doubles(
    weld_edges: &[WeldEdge],
    mvert_num: usize,
    r_edge_dest_map: &mut [i32],
) -> usize {
    let mut edge_double_kill_len = 0;
    if weld_edges.is_empty() {
        return 0;
    }
    // Add +1 to allow calculation of the length of the last group.
    let mut v_links = vec![0i32; mvert_num + 1];
    for we in weld_edges {
        v_links[we.vert_a as usize] += 1;
        v_links[we.vert_b as usize] += 1;
    }
    let mut link_len = 0;
    for v in v_links.iter_mut().take(mvert_num) {
        link_len += *v;
        *v = link_len;
    }
    v_links[mvert_num] = link_len;
    let mut link_edge_buffer = vec![0i32; link_len as usize];
    // Use a reverse for loop to ensure that indexes are assigned in ascending order.
    for i in (0..weld_edges.len()).rev() {
        let we = weld_edges[i];
        v_links[we.vert_a as usize] -= 1;
        link_edge_buffer[v_links[we.vert_a as usize] as usize] = i as i32;
        v_links[we.vert_b as usize] -= 1;
        link_edge_buffer[v_links[we.vert_b as usize] as usize] = i as i32;
    }
    for (i, we) in weld_edges.iter().enumerate() {
        frap_math::budget::check();
        if r_edge_dest_map[we.edge_orig as usize] != we.edge_orig {
            // Already a duplicate.
            continue;
        }
        let (a, b) = (we.vert_a as usize, we.vert_b as usize);
        let link_a = v_links[a] as usize;
        let link_b = v_links[b] as usize;
        let mut edges_len_a = v_links[a + 1] as usize - link_a;
        let mut edges_len_b = v_links[b + 1] as usize - link_b;
        let edge_orig = we.edge_orig;
        if edges_len_a <= 1 || edges_len_b <= 1 {
            // This edge would form a group with only one element.
            r_edge_dest_map[edge_orig as usize] = OUT_OF_CONTEXT;
            continue;
        }
        let mut ia = link_a;
        let mut ib = link_b;
        let edge_double_len_prev = edge_double_kill_len;
        while edges_len_a > 0 {
            edges_len_a -= 1;
            let e_ctx_a = link_edge_buffer[ia];
            ia += 1;
            if e_ctx_a == i as i32 {
                continue;
            }
            while edges_len_b > 0 && link_edge_buffer[ib] < e_ctx_a {
                ib += 1;
                edges_len_b -= 1;
            }
            if edges_len_b == 0 {
                break;
            }
            let e_ctx_b = link_edge_buffer[ib];
            if e_ctx_a == e_ctx_b {
                let we_b = weld_edges[e_ctx_b as usize];
                r_edge_dest_map[we_b.edge_orig as usize] = edge_orig;
                edge_double_kill_len += 1;
            }
        }
        if edge_double_len_prev == edge_double_kill_len {
            r_edge_dest_map[edge_orig as usize] = OUT_OF_CONTEXT;
        }
    }
    edge_double_kill_len
}

// ---------------------------------------------------------------------- poly and loop API

fn weld_poly_loop_ctx_alloc(
    face_offsets: &[i32],
    corner_verts: &[i32],
    corner_edges: &[i32],
    vert_dest_map: &[i32],
    wm: &mut WeldMesh,
) {
    let faces_num = face_offsets.len() - 1;
    let mut loop_map = vec![0i32; corner_verts.len()];
    let mut face_map = vec![0i32; faces_num];
    let mut wloop: Vec<WeldLoop> = Vec::with_capacity(corner_verts.len());
    let mut wpoly: Vec<WeldPoly> = Vec::with_capacity(faces_num);
    for i in 0..faces_num {
        frap_math::budget::check();
        let loopstart = face_offsets[i];
        let totloop = face_offsets[i + 1] - loopstart;
        let loop_end = loopstart + totloop - 1;
        let v_first = corner_verts[loopstart as usize];
        let v_dest_first = vert_dest_map[v_first as usize];
        let is_vert_first_ctx = v_dest_first != OUT_OF_CONTEXT;
        let mut v_next = v_first;
        let mut v_dest_next = v_dest_first;
        let mut is_vert_next_ctx = is_vert_first_ctx;
        let prev_wloop_len = wloop.len();
        for loop_orig in loopstart..loopstart + totloop {
            let v = v_next;
            let v_dest = v_dest_next;
            let is_vert_ctx = is_vert_next_ctx;
            let loop_next;
            if loop_orig != loop_end {
                loop_next = loop_orig + 1;
                v_next = corner_verts[loop_next as usize];
                v_dest_next = vert_dest_map[v_next as usize];
                is_vert_next_ctx = v_dest_next != OUT_OF_CONTEXT;
            } else {
                loop_next = loopstart;
                v_next = v_first;
                v_dest_next = v_dest_first;
                is_vert_next_ctx = is_vert_first_ctx;
            }
            if is_vert_ctx || is_vert_next_ctx {
                let e = corner_edges[loop_orig as usize];
                let e_dest = wm.edge_dest_map[e as usize];
                let is_edge_ctx = e_dest != OUT_OF_CONTEXT;
                wloop.push(WeldLoop {
                    vert: if is_vert_ctx { v_dest } else { v },
                    edge: if is_edge_ctx { e_dest } else { e },
                    loop_orig,
                    loop_next,
                });
                loop_map[loop_orig as usize] = wloop.len() as i32 - 1;
            } else {
                loop_map[loop_orig as usize] = OUT_OF_CONTEXT;
            }
        }
        if wloop.len() != prev_wloop_len {
            let loop_ctx_len = (wloop.len() - prev_wloop_len) as i32;
            wpoly.push(WeldPoly {
                poly_dst: OUT_OF_CONTEXT,
                poly_orig: i as i32,
                loop_start: loopstart,
                loop_end,
                loop_ctx_start: prev_wloop_len as i32,
                loop_ctx_len,
            });
            face_map[i] = wpoly.len() as i32 - 1;
        } else {
            face_map[i] = OUT_OF_CONTEXT;
        }
    }
    wm.wloop = wloop;
    wm.wpoly = wpoly;
    wm.wpoly_new_len = 0;
    wm.loop_map = loop_map;
    wm.face_map = face_map;
}

fn weld_poly_split_recursive(
    mut poly_loop_len: i32,
    vert_dest_map: &[i32],
    wp_i: usize,
    wm: &mut WeldMesh,
    r_poly_kill: &mut usize,
    r_loop_kill: &mut usize,
) {
    if poly_loop_len < 3 {
        return;
    }
    let mut loop_kill = 0usize;
    let mut loop_end = wm.wpoly[wp_i].loop_end;
    let ctx_end = wm.loop_map[loop_end as usize];
    let mut wla_prev: Option<usize> = (ctx_end != OUT_OF_CONTEXT).then_some(ctx_end as usize);
    let mut la = wm.wpoly[wp_i].loop_start;
    'outer: loop {
        'body: {
            let loop_ctx_a = wm.loop_map[la as usize];
            if loop_ctx_a == OUT_OF_CONTEXT {
                la += 1;
                wla_prev = None;
                break 'body;
            }
            let mut wla = loop_ctx_a as usize;
            let vert_a = wm.wloop[wla].vert;
            if vert_dest_map[vert_a as usize] == OUT_OF_CONTEXT {
                // Only test vertices that will be merged.
                la = wm.wloop[wla].loop_next;
                wla_prev = Some(wla);
                break 'body;
            }
            let mut dist_a = 1;
            let mut lb_prev = la;
            let mut wlb_prev: Option<usize> = Some(wla);
            let mut lb = wm.wloop[wla].loop_next;
            'lb: loop {
                'inner: {
                    let loop_ctx_b = wm.loop_map[lb as usize];
                    if loop_ctx_b == OUT_OF_CONTEXT {
                        dist_a += 1;
                        lb_prev = lb;
                        wlb_prev = None;
                        lb += 1;
                        break 'inner;
                    }
                    let wlb = loop_ctx_b as usize;
                    let vert_b = wm.wloop[wlb].vert;
                    if vert_a != vert_b {
                        dist_a += 1;
                        lb_prev = lb;
                        wlb_prev = Some(wlb);
                        lb = wm.wloop[wlb].loop_next;
                        break 'inner;
                    }
                    let dist_b = poly_loop_len - dist_a;
                    if dist_a == 1 || dist_b == 1 {
                        // A collapsed loop (already flagged).
                    } else if dist_a == 2 && dist_b == 2 {
                        // All loops are "collapsed"; flagging the face is enough.
                        loop_kill += 4;
                        wm.wpoly[wp_i].poly_dst = ELEM_COLLAPSED;
                        *r_poly_kill += 1;
                        *r_loop_kill += loop_kill;
                        return;
                    } else {
                        if let Some(p) = wla_prev {
                            wm.wloop[p].loop_next = lb;
                        }
                        if let Some(p) = wlb_prev {
                            wm.wloop[p].loop_next = la;
                        }
                        if wm.wpoly[wp_i].loop_start == la {
                            wm.wpoly[wp_i].loop_start = lb;
                        }
                        if dist_a == 2 {
                            wm.wloop[wla].edge = ELEM_COLLAPSED;
                            if let Some(p) = wlb_prev {
                                wm.wloop[p].edge = ELEM_COLLAPSED;
                            }
                            loop_kill += 2;
                        } else if dist_b == 2 {
                            wm.wloop[wlb].edge = ELEM_COLLAPSED;
                            if let Some(p) = wla_prev {
                                wm.wloop[p].edge = ELEM_COLLAPSED;
                            }
                            loop_kill += 2;
                            wm.wpoly[wp_i].loop_start = la;
                            loop_end = lb_prev;
                            wm.wpoly[wp_i].loop_end = lb_prev;
                            poly_loop_len = dist_a;
                            break 'lb;
                        } else {
                            let parent = wm.wpoly[wp_i];
                            wm.wpoly.push(WeldPoly {
                                poly_dst: OUT_OF_CONTEXT,
                                poly_orig: parent.poly_orig,
                                loop_start: la,
                                loop_end: lb_prev,
                                loop_ctx_start: parent.loop_ctx_start,
                                loop_ctx_len: parent.loop_ctx_len,
                            });
                            wm.wpoly_new_len += 1;
                            let new_i = wm.wpoly.len() - 1;
                            weld_poly_split_recursive(
                                dist_a,
                                vert_dest_map,
                                new_i,
                                wm,
                                r_poly_kill,
                                r_loop_kill,
                            );
                        }
                        la = lb;
                        wla = wlb;
                        poly_loop_len = dist_b;
                        dist_a = 1;
                    }
                    wlb_prev = Some(wlb);
                    lb_prev = lb;
                    lb = wm.wloop[wlb].loop_next;
                }
                if lb_prev == loop_end {
                    break;
                }
            }
            wla_prev = Some(wla);
            if la == loop_end {
                // No need to start again.
                break 'outer;
            }
            la = wm.wloop[wla].loop_next;
        }
        if la == loop_end {
            break;
        }
    }
    *r_loop_kill += loop_kill;
}

fn weld_poly_loop_ctx_setup_collapsed_and_split(
    remain_edge_ctx_len: usize,
    vert_dest_map: &[i32],
    wm: &mut WeldMesh,
) {
    if remain_edge_ctx_len == 0 {
        wm.face_kill_len = wm.wpoly.len();
        wm.loop_kill_len = wm.wloop.len();
        for wp in &mut wm.wpoly {
            wp.poly_dst = ELEM_COLLAPSED;
        }
        return;
    }
    let mut face_kill_len = 0usize;
    let mut loop_kill_len = 0usize;
    // `wpoly.len()` may change during the loop: only the original items are walked.
    let original = wm.wpoly.len();
    for i in 0..original {
        frap_math::budget::check();
        let mut poly_loop_len = wm.wpoly[i].loop_end - wm.wpoly[i].loop_start + 1;
        let mut wl_prev: Option<usize> = None;
        let mut chang_loop_start = false;
        let mut l = wm.wpoly[i].loop_start;
        loop {
            'body: {
                let loop_ctx = wm.loop_map[l as usize];
                if loop_ctx == OUT_OF_CONTEXT {
                    wl_prev = None;
                    break 'body;
                }
                let wl = loop_ctx as usize;
                let edge_dest = wm.wloop[wl].edge;
                if edge_dest == ELEM_COLLAPSED {
                    wm.wloop[wl].edge = ELEM_COLLAPSED;
                    if poly_loop_len == 3 {
                        wm.wpoly[i].poly_dst = ELEM_COLLAPSED;
                        face_kill_len += 1;
                        loop_kill_len += 3;
                        poly_loop_len = 0;
                        // `break` out of the do-while.
                        l = i32::MIN;
                        break 'body;
                    }
                    if l == wm.wpoly[i].loop_start {
                        chang_loop_start = true;
                    }
                    loop_kill_len += 1;
                    poly_loop_len -= 1;
                } else {
                    if chang_loop_start {
                        wm.wpoly[i].loop_start = l;
                        chang_loop_start = false;
                    }
                    if let Some(p) = wl_prev {
                        wm.wloop[p].loop_next = l;
                    }
                    wl_prev = Some(wl);
                }
            }
            if l == i32::MIN {
                break;
            }
            let cont = l != wm.wpoly[i].loop_end;
            l += 1;
            if !cont {
                break;
            }
        }
        if poly_loop_len != 0 {
            if let Some(p) = wl_prev {
                wm.wloop[p].loop_next = wm.wpoly[i].loop_start;
                wm.wpoly[i].loop_end = wm.wloop[p].loop_orig;
            }
            weld_poly_split_recursive(
                poly_loop_len,
                vert_dest_map,
                i,
                wm,
                &mut face_kill_len,
                &mut loop_kill_len,
            );
        }
    }
    wm.face_kill_len = face_kill_len;
    wm.loop_kill_len = loop_kill_len;
}

/// `poly_find_doubles`: groups of faces made of the same edges. Returns the number of
/// duplicates, with the group offsets and buffer (`buffer[offsets[g]]` is the kept face).
fn poly_find_doubles(
    poly_offs: &[usize],
    poly_num: usize,
    corners: &[i32],
    corner_index_max: usize,
) -> (usize, Vec<usize>, Vec<usize>) {
    fn intersect(a: &[usize], b: &[usize], is_double: &[bool], out: &mut Vec<usize>) {
        out.clear();
        let (mut ia, mut ib) = (0, 0);
        while ia < a.len() && ib < b.len() {
            let (va, vb) = (a[ia], b[ib]);
            if va < vb {
                ia += 1;
            } else if vb < va {
                ib += 1;
            } else {
                if !is_double[va] {
                    out.push(va);
                }
                ia += 1;
                ib += 1;
            }
        }
    }
    let mut linked_faces_offset = vec![0usize; corner_index_max + 1];
    for &e in corners {
        linked_faces_offset[e as usize] += 1;
    }
    let mut buffer_len = 0;
    for o in linked_faces_offset.iter_mut().take(corner_index_max) {
        buffer_len += *o;
        *o = buffer_len;
    }
    linked_faces_offset[corner_index_max] = buffer_len;
    if buffer_len == 0 {
        return (0, vec![0], Vec::new());
    }
    let mut linked_faces_buffer = vec![0usize; buffer_len];
    for face_index in (0..poly_num).rev() {
        let (s, e) = (poly_offs[face_index], poly_offs[face_index + 1]);
        for corner_index in (s..e).rev() {
            let elem = corners[corner_index] as usize;
            linked_faces_offset[elem] -= 1;
            linked_faces_buffer[linked_faces_offset[elem]] = face_index;
        }
    }
    let mut doubles_buffer = vec![0usize; poly_num];
    let mut doubles_offsets: Vec<usize> = vec![0];
    let mut is_double = vec![false; poly_num];
    let mut doubles_buffer_num = 0usize;
    let mut isect: Vec<usize> = Vec::new();
    let mut faces_a: Vec<usize> = Vec::new();
    // Declared outside the face loop, as in Blender.
    let mut doubles_num = 0;
    for face_index in 0..poly_num {
        frap_math::budget::check();
        if is_double[face_index] {
            continue;
        }
        let (s, e) = (poly_offs[face_index], poly_offs[face_index + 1]);
        let corner_num = e - s;
        if corner_num == 0 {
            continue;
        }
        doubles_buffer[doubles_buffer_num] = face_index;
        let elem = corners[s] as usize;
        let lo = linked_faces_offset[elem];
        let hi = linked_faces_offset[elem + 1];
        if hi - lo == 1 {
            continue;
        }
        // Skip polygons with lower index as these have already been checked.
        let pos = linked_faces_buffer[lo..hi]
            .iter()
            .position(|&f| f == face_index)
            .expect("a face is linked to its own corners");
        faces_a.clear();
        faces_a.extend_from_slice(&linked_faces_buffer[lo + pos + 1..hi]);
        for &corner in &corners[s + 1..e] {
            let elem = corner as usize;
            let lo = linked_faces_offset[elem];
            let hi = linked_faces_offset[elem + 1];
            let pos = linked_faces_buffer[lo..hi]
                .iter()
                .position(|&f| f == face_index)
                .expect("a face is linked to its own corners");
            let faces_b = &linked_faces_buffer[lo + pos + 1..hi];
            intersect(&faces_a, faces_b, &is_double, &mut isect);
            doubles_num = isect.len();
            if doubles_num == 0 {
                break;
            }
            std::mem::swap(&mut faces_a, &mut isect);
        }
        if doubles_num != 0 {
            for (k, &d) in faces_a.iter().enumerate() {
                is_double[d] = true;
                doubles_buffer[doubles_buffer_num + 1 + k] = d;
            }
            doubles_buffer_num += doubles_num;
            doubles_buffer_num += 1;
            doubles_offsets.push(doubles_buffer_num);
            if doubles_buffer_num + 1 == poly_num {
                // The last slot is the remaining unduplicated face.
                break;
            }
        }
    }
    doubles_buffer.truncate(doubles_buffer_num);
    let count = doubles_buffer_num - (doubles_offsets.len() - 1);
    (count, doubles_offsets, doubles_buffer)
}

fn weld_poly_find_doubles(
    corner_verts: &[i32],
    corner_edges: &[i32],
    medge_len: usize,
    wm: &mut WeldMesh,
) {
    if wm.face_kill_len == wm.wpoly.len() {
        return;
    }
    let face_len = wm.wpoly.len();
    let mut poly_offs = vec![0usize; face_len + 1];
    let mut new_corner_edges: Vec<i32> = Vec::with_capacity(corner_verts.len());
    for (fi, wp) in wm.wpoly.iter().enumerate() {
        poly_offs[fi] = new_corner_edges.len();
        let Some(mut it) = LoopIter::begin(
            wp,
            &wm.wloop,
            corner_verts,
            corner_edges,
            &wm.loop_map,
            false,
        ) else {
            continue;
        };
        if wp.poly_dst != OUT_OF_CONTEXT {
            continue;
        }
        loop {
            new_corner_edges.push(it.e);
            if !it.next() {
                break;
            }
        }
    }
    poly_offs[face_len] = new_corner_edges.len();
    let (doubles_num, offsets, buffer) =
        poly_find_doubles(&poly_offs, face_len, &new_corner_edges, medge_len);
    if doubles_num != 0 {
        let mut loop_kill_num = 0;
        for g in 0..offsets.len() - 1 {
            let poly_dst = wm.wpoly[buffer[offsets[g]]].poly_orig;
            for &wpoly_index in &buffer[offsets[g] + 1..offsets[g + 1]] {
                wm.wpoly[wpoly_index].poly_dst = poly_dst;
                loop_kill_num += poly_offs[wpoly_index + 1] - poly_offs[wpoly_index];
            }
        }
        wm.face_kill_len += doubles_num;
        wm.loop_kill_len += loop_kill_num;
    }
}

// ---------------------------------------------------------------------------- mesh API

fn weld_mesh_context_create(
    mesh: &Mesh,
    vert_dest_map: &mut [i32],
    vert_kill_len: usize,
    get_doubles: bool,
) -> WeldMesh {
    let edges = mesh.edges();
    let face_offsets = mesh.face_offsets();
    let corner_verts = mesh.corner_verts();
    let corner_edges = mesh.corner_edges();
    let wvert = weld_vert_ctx_alloc_and_setup(vert_dest_map, vert_kill_len);
    let mut wm = WeldMesh {
        vert_kill_len,
        edge_dest_map: vec![0; edges.len()],
        ..WeldMesh::default()
    };
    let (wedge, edge_collapsed_len) =
        weld_edge_ctx_alloc_and_find_collapsed(edges, vert_dest_map, &mut wm.edge_dest_map);
    let edge_double_kill_len =
        weld_edge_find_doubles(&wedge, mesh.verts_num(), &mut wm.edge_dest_map);
    wm.edge_kill_len = edge_collapsed_len + edge_double_kill_len;
    weld_poly_loop_ctx_alloc(
        face_offsets,
        corner_verts,
        corner_edges,
        vert_dest_map,
        &mut wm,
    );
    weld_poly_loop_ctx_setup_collapsed_and_split(
        wedge.len() - edge_double_kill_len,
        vert_dest_map,
        &mut wm,
    );
    weld_poly_find_doubles(corner_verts, corner_edges, edges.len(), &mut wm);
    if get_doubles {
        wm.double_verts = wvert;
        wm.double_edges = wedge
            .iter()
            .filter(|we| wm.edge_dest_map[we.edge_orig as usize] >= 0)
            .map(|we| we.edge_orig)
            .collect();
    }
    wm
}

// ---------------------------------------------------------------------------- merging

/// `merge_customdata_all`: the source indices of every kept element and the final index of
/// every source element.
fn merge_customdata_all(
    dest_map: &[i32],
    double_elems: &[i32],
    dest_size: usize,
    do_mix_data: bool,
) -> (Vec<SmallVec<[usize; 2]>>, Vec<usize>) {
    let source_size = dest_map.len();
    // Groups of elements merging into each target (ascending order).
    let mut groups: Vec<SmallVec<[usize; 2]>> = Vec::new();
    if do_mix_data {
        groups = vec![SmallVec::new(); source_size];
        // In `double_elems` order (Blender fills its buffer back to front).
        for &e in double_elems {
            groups[dest_map[e as usize] as usize].push(e as usize);
        }
    }
    let mut src_groups: Vec<SmallVec<[usize; 2]>> = Vec::with_capacity(dest_size);
    let mut final_map = vec![0i64; source_size];
    let mut finalize_map = false;
    let mut dest_index = 0usize;
    for i in 0..source_size {
        let d = dest_map[i];
        if d == OUT_OF_CONTEXT {
            final_map[i] = dest_index as i64;
            src_groups.push(SmallVec::from_slice(&[i]));
            dest_index += 1;
        } else if d == i as i32 {
            if do_mix_data {
                src_groups.push(std::mem::take(&mut groups[i]));
            } else {
                src_groups.push(SmallVec::from_slice(&[i]));
            }
            final_map[i] = dest_index as i64;
            dest_index += 1;
        } else if d == ELEM_COLLAPSED {
            // Any value will do. This field must not be accessed anymore.
            final_map[i] = 0;
        } else if (d as usize) < i {
            final_map[i] = final_map[d as usize];
        } else {
            // Mark as negative to set at the end.
            final_map[i] = -i64::from(d);
            finalize_map = true;
        }
    }
    if finalize_map {
        for i in 0..source_size {
            if final_map[i] < 0 {
                final_map[i] = final_map[(-final_map[i]) as usize];
            }
        }
    }
    debug_assert_eq!(dest_index, dest_size);
    (
        src_groups,
        final_map.into_iter().map(|x| x as usize).collect(),
    )
}

fn create_merged_mesh(
    mesh: &Mesh,
    vert_dest_map: &mut [i32],
    removed_vertex_count: usize,
    do_mix_data: bool,
) -> Mesh {
    let wm = weld_mesh_context_create(mesh, vert_dest_map, removed_vertex_count, do_mix_data);
    let src_edges = mesh.edges();
    let src_offsets = mesh.face_offsets();
    let src_corner_verts = mesh.corner_verts();
    let src_corner_edges = mesh.corner_edges();
    let totvert = mesh.verts_num();
    let totedge = mesh.edges_num();
    let result_nverts = totvert - wm.vert_kill_len;
    let result_nedges = totedge - wm.edge_kill_len;
    let result_nloops = src_corner_verts.len() - wm.loop_kill_len;
    let result_nfaces = mesh.faces_num() - wm.face_kill_len + wm.wpoly_new_len;

    // Vertices.
    let (vert_groups, vert_final_map) =
        merge_customdata_all(vert_dest_map, &wm.double_verts, result_nverts, do_mix_data);
    // Edges.
    let (edge_groups, edge_final_map) = merge_customdata_all(
        &wm.edge_dest_map,
        &wm.double_edges,
        result_nedges,
        do_mix_data,
    );
    let dst_edges: Vec<[i32; 2]> = edge_groups
        .iter()
        .map(|g| {
            let e = src_edges[g[0]];
            [
                vert_final_map[e[0] as usize] as i32,
                vert_final_map[e[1] as usize] as i32,
            ]
        })
        .collect();

    // Faces/loops.
    let mut corner_groups: Vec<SmallVec<[usize; 2]>> = Vec::with_capacity(result_nloops);
    let mut dst_corner_verts: Vec<i32> = Vec::with_capacity(result_nloops);
    let mut dst_corner_edges: Vec<i32> = Vec::with_capacity(result_nloops);
    let mut dst_offsets: Vec<i32> = Vec::with_capacity(result_nfaces + 1);
    let mut dst_to_src_faces: Vec<usize> = Vec::with_capacity(result_nfaces);
    for i in 0..mesh.faces_num() {
        let loop_start = dst_corner_verts.len() as i32;
        let poly_ctx = wm.face_map[i];
        if poly_ctx == OUT_OF_CONTEXT {
            for loop_orig in src_offsets[i] as usize..src_offsets[i + 1] as usize {
                corner_groups.push(SmallVec::from_slice(&[loop_orig]));
                dst_corner_verts.push(vert_final_map[src_corner_verts[loop_orig] as usize] as i32);
                dst_corner_edges.push(edge_final_map[src_corner_edges[loop_orig] as usize] as i32);
            }
        } else {
            let wp = &wm.wpoly[poly_ctx as usize];
            let Some(mut it) = LoopIter::begin(
                wp,
                &wm.wloop,
                src_corner_verts,
                src_corner_edges,
                &wm.loop_map,
                true,
            ) else {
                continue;
            };
            if wp.poly_dst != OUT_OF_CONTEXT {
                continue;
            }
            loop {
                corner_groups.push(it.group.iter().map(|&l| l as usize).collect());
                dst_corner_verts.push(vert_final_map[it.v as usize] as i32);
                dst_corner_edges.push(edge_final_map[it.e as usize] as i32);
                if !it.next() {
                    break;
                }
            }
        }
        dst_to_src_faces.push(i);
        dst_offsets.push(loop_start);
    }
    // New polygons.
    let new_start = wm.wpoly.len() - wm.wpoly_new_len;
    for wp in &wm.wpoly[new_start..] {
        let loop_start = dst_corner_verts.len() as i32;
        let Some(mut it) = LoopIter::begin(
            wp,
            &wm.wloop,
            src_corner_verts,
            src_corner_edges,
            &wm.loop_map,
            true,
        ) else {
            continue;
        };
        if wp.poly_dst != OUT_OF_CONTEXT {
            continue;
        }
        loop {
            corner_groups.push(it.group.iter().map(|&l| l as usize).collect());
            dst_corner_verts.push(vert_final_map[it.v as usize] as i32);
            dst_corner_edges.push(edge_final_map[it.e as usize] as i32);
            if !it.next() {
                break;
            }
        }
        dst_offsets.push(loop_start);
    }
    dst_offsets.push(dst_corner_verts.len() as i32);
    let faces_out = dst_offsets.len() - 1;
    let mut face_map: Vec<Option<usize>> = dst_to_src_faces.iter().map(|&f| Some(f)).collect();
    face_map.resize(faces_out, None);

    let positions = match util::mix_groups(
        &frap_mesh::AttrData::Float3(std::sync::Arc::new(mesh.positions().to_vec())),
        &vert_groups,
    ) {
        Some(frap_mesh::AttrData::Float3(v)) => std::sync::Arc::unwrap_or_clone(v),
        _ => unreachable!("positions are FLOAT_VECTOR"),
    };
    let topo = Topology {
        positions,
        edges: dst_edges,
        face_offsets: dst_offsets,
        corner_verts: dst_corner_verts,
        corner_edges: dst_corner_edges,
    };
    util::rebuild(
        mesh,
        topo,
        &DomainMaps {
            vert: Carry::Mix(vert_groups),
            edge: Carry::Mix(edge_groups),
            face: Carry::Gather(face_map),
            corner: Carry::Mix(corner_groups),
        },
    )
}

/// `geometry::mesh_merge_verts(mesh, vert_dest_map, vert_dest_map_len, do_mix_data)`:
/// `vert_dest_map[v]` is the vertex `v` merges into, or `-1`; no chains.
pub(crate) fn mesh_merge_verts(
    mesh: &Mesh,
    vert_dest_map: &mut [i32],
    vert_dest_map_len: usize,
    do_mix_data: bool,
) -> Mesh {
    create_merged_mesh(mesh, vert_dest_map, vert_dest_map_len, do_mix_data)
}

/// `mesh_merge_by_distance_all`: `None` when nothing merges.
pub(crate) fn mesh_merge_by_distance_all(
    mesh: &Mesh,
    selection: Option<&[bool]>,
    merge_distance: f32,
) -> Option<Mesh> {
    let positions = mesh.positions();
    let mut vert_dest_map = vec![OUT_OF_CONTEXT; mesh.verts_num()];
    let mut tree = KdTree::default();
    for (i, &co) in positions.iter().enumerate() {
        if selection.is_none_or(|s| s[i]) {
            tree.insert(i as i32, co);
        }
    }
    tree.balance();
    let vert_kill_len = tree.calc_duplicates_fast(merge_distance, true, &mut vert_dest_map);
    if vert_kill_len == 0 {
        return None;
    }
    Some(create_merged_mesh(
        mesh,
        &mut vert_dest_map,
        vert_kill_len,
        true,
    ))
}

/// `mesh_merge_by_distance_connected`: `None` when nothing merges.
pub(crate) fn mesh_merge_by_distance_connected(
    mesh: &Mesh,
    selection: &[bool],
    merge_distance: f32,
    only_loose_edges: bool,
) -> Option<Mesh> {
    let positions = mesh.positions();
    let edges = mesh.edges();
    let n = mesh.verts_num();
    let mut vert_kill_len = 0;
    let mut vert_dest_map: Vec<i32> = (0..n as i32).collect();
    let mut cluster_co: Vec<[f32; 3]> = positions.to_vec();
    let mut merged_verts = vec![0i32; n];
    let merge_dist_sq = merge_distance * merge_distance;
    let loose: Option<Vec<bool>> = only_loose_edges.then(|| {
        let mut used = vec![false; edges.len()];
        for &e in mesh.corner_edges() {
            used[e as usize] = true;
        }
        used.into_iter().map(|u| !u).collect()
    });
    for (i, e) in edges.iter().enumerate() {
        if loose.as_ref().is_some_and(|l| !l[i]) {
            continue;
        }
        let mut v1 = e[0] as usize;
        let mut v2 = e[1] as usize;
        while v1 as i32 != vert_dest_map[v1] {
            v1 = vert_dest_map[v1] as usize;
        }
        while v2 as i32 != vert_dest_map[v2] {
            v2 = vert_dest_map[v2] as usize;
        }
        if v1 == v2 {
            continue;
        }
        if !selection.is_empty() && (!selection[v1] || !selection[v2]) {
            continue;
        }
        if v1 > v2 {
            std::mem::swap(&mut v1, &mut v2);
        }
        let c1 = cluster_co[v1];
        let c2 = cluster_co[v2];
        let edgedir = [c2[0] - c1[0], c2[1] - c1[1], c2[2] - c1[2]];
        let dist_sq = edgedir[0] * edgedir[0] + edgedir[1] * edgedir[1] + edgedir[2] * edgedir[2];
        if dist_sq <= merge_dist_sq {
            let influence =
                (merged_verts[v2] + 1) as f32 / (merged_verts[v1] + merged_verts[v2] + 2) as f32;
            for k in 0..3 {
                cluster_co[v1][k] += edgedir[k] * influence;
            }
            merged_verts[v1] += merged_verts[v2] + 1;
            vert_dest_map[v2] = v1 as i32;
            vert_kill_len += 1;
        }
    }
    if vert_kill_len == 0 {
        return None;
    }
    for i in 0..n {
        if i as i32 == vert_dest_map[i] {
            vert_dest_map[i] = OUT_OF_CONTEXT;
        } else {
            let mut v = i;
            while v as i32 != vert_dest_map[v] && vert_dest_map[v] != OUT_OF_CONTEXT {
                v = vert_dest_map[v] as usize;
            }
            vert_dest_map[v] = v as i32;
            vert_dest_map[i] = v as i32;
        }
    }
    Some(create_merged_mesh(
        mesh,
        &mut vert_dest_map,
        vert_kill_len,
        true,
    ))
}

// ---------------------------------------------------------------------------- kd-tree

const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct KdNode {
    left: u32,
    right: u32,
    co: [f32; 3],
    index: i32,
    d: u8,
}

/// The parts of `KDTree<float3>` `kdtree_calc_duplicates_fast` needs: the same quick-select
/// balance as `frap_geom::kdtree` (the result depends on the tree layout at range boundaries).
#[derive(Default)]
struct KdTree {
    nodes: Vec<KdNode>,
    root: u32,
    max_node_index: i32,
}

impl KdTree {
    fn insert(&mut self, index: i32, co: [f32; 3]) {
        self.nodes.push(KdNode {
            left: UNSET,
            right: UNSET,
            co,
            index,
            d: 0,
        });
        self.max_node_index = self.max_node_index.max(index);
    }

    // Ported from blender/source/blender/blenlib/BLI_kdtree.hh (detail::kdtree_balance).
    fn balance_rec(nodes: &mut [KdNode], mut axis: u8, ofs: u32) -> u32 {
        let n = nodes.len();
        if n == 0 {
            return UNSET;
        }
        if n == 1 {
            return ofs;
        }
        let ax = axis as usize;
        let mut left: i64 = 0;
        let mut right: i64 = n as i64 - 1;
        let median = (n / 2) as i64;
        while right > left {
            let co = nodes[right as usize].co[ax];
            let mut i = left - 1;
            let mut j = right;
            loop {
                loop {
                    i += 1;
                    #[allow(clippy::neg_cmp_op_on_partial_ord)]
                    if !(nodes[i as usize].co[ax] < co) {
                        break;
                    }
                }
                loop {
                    j -= 1;
                    if !(nodes[j as usize].co[ax] > co && j > left) {
                        break;
                    }
                }
                if i >= j {
                    break;
                }
                nodes.swap(i as usize, j as usize);
            }
            nodes.swap(i as usize, right as usize);
            if i >= median {
                right = i - 1;
            }
            if i <= median {
                left = i + 1;
            }
        }
        let median = median as usize;
        nodes[median].d = axis;
        axis = (axis + 1) % 3;
        let (lo, hi) = nodes.split_at_mut(median);
        let l = Self::balance_rec(lo, axis, ofs);
        let r = Self::balance_rec(&mut hi[1..], axis, median as u32 + 1 + ofs);
        nodes[median].left = l;
        nodes[median].right = r;
        median as u32 + ofs
    }

    fn balance(&mut self) {
        for n in &mut self.nodes {
            n.left = UNSET;
            n.right = UNSET;
        }
        self.root = Self::balance_rec(&mut self.nodes, 0, 0);
    }

    // Ported from blender/source/blender/blenlib/BLI_kdtree.hh (detail::deduplicate_recursive).
    #[allow(clippy::too_many_arguments)]
    fn deduplicate_recursive(
        &self,
        i: u32,
        search: i32,
        search_co: [f32; 3],
        range: f32,
        range_sq: f32,
        duplicates: &mut [i32],
        found: &mut usize,
    ) {
        let node = &self.nodes[i as usize];
        let d = node.d as usize;
        if search_co[d] + range <= node.co[d] {
            if node.left != UNSET {
                self.deduplicate_recursive(
                    node.left, search, search_co, range, range_sq, duplicates, found,
                );
            }
        } else if search_co[d] - range >= node.co[d] {
            if node.right != UNSET {
                self.deduplicate_recursive(
                    node.right, search, search_co, range, range_sq, duplicates, found,
                );
            }
        } else {
            if search != node.index && duplicates[node.index as usize] == -1 {
                let ds = util::len_sq(node.co, search_co);
                if ds <= range_sq {
                    duplicates[node.index as usize] = search;
                    *found += 1;
                }
            }
            if node.left != UNSET {
                self.deduplicate_recursive(
                    node.left, search, search_co, range, range_sq, duplicates, found,
                );
            }
            if node.right != UNSET {
                self.deduplicate_recursive(
                    node.right, search, search_co, range, range_sq, duplicates, found,
                );
            }
        }
    }

    // Ported from blender/source/blender/blenlib/BLI_kdtree.hh (kdtree_calc_duplicates_fast).
    /// `kdtree_calc_duplicates_fast(tree, range, use_index_order, duplicates)`.
    fn calc_duplicates_fast(
        &self,
        range: f32,
        use_index_order: bool,
        duplicates: &mut [i32],
    ) -> usize {
        let mut found = 0usize;
        if self.root == UNSET {
            return 0;
        }
        let range_sq = range * range;
        let mut search_one = |index: i32, co: [f32; 3], duplicates: &mut [i32]| {
            let d = duplicates[index as usize];
            if d == -1 || d == index {
                let prev = found;
                self.deduplicate_recursive(
                    self.root, index, co, range, range_sq, duplicates, &mut found,
                );
                if found != prev {
                    // Prevent chains of doubles.
                    duplicates[index as usize] = index;
                }
            }
        };
        if use_index_order {
            let mut order = vec![-1i64; (self.max_node_index + 1).max(0) as usize];
            for (ni, n) in self.nodes.iter().enumerate() {
                order[n.index as usize] = ni as i64;
            }
            for (index, &ni) in order.iter().enumerate() {
                if ni == -1 {
                    continue;
                }
                search_one(index as i32, self.nodes[ni as usize].co, duplicates);
            }
        } else {
            for n in &self.nodes {
                search_one(n.index, n.co, duplicates);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates_fast_marks_targets() {
        let mut t = KdTree::default();
        let pts = [
            [0.0, 0.0, 0.0],
            [0.0005, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        for (i, p) in pts.iter().enumerate() {
            t.insert(i as i32, *p);
        }
        t.balance();
        let mut d = vec![-1; 4];
        assert_eq!(t.calc_duplicates_fast(0.001, true, &mut d), 2);
        assert_eq!(d, [0, 0, -1, 0]);
    }

    #[test]
    fn welding_a_doubled_quad_strip() {
        // Two quads side by side whose shared edge is duplicated (8 vertices).
        let mut m = Mesh::new("m");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            &[],
            &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
            false,
        );
        let out = mesh_merge_by_distance_all(&m, None, 0.001).expect("merges");
        assert_eq!(out.verts_num(), 6);
        assert_eq!(out.edges_num(), 7);
        assert_eq!(out.faces_num(), 2);
        assert_eq!(out.corners_num(), 8);
        assert_eq!(out.corner_verts(), &[0, 1, 2, 3, 1, 4, 5, 2]);
    }
}
