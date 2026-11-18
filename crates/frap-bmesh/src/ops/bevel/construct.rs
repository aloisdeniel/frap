// SPDX-License-Identifier: GPL-3.0-or-later
//! Construction around a beveled vertex: edge order, EdgeHalf offsets
//! (`bevel_vert_construct`, `find_bevel_edge_order`, `fast_bevel_edge_order`,
//! `bevel_edge_order_extend`). Ported from
//! blender/source/blender/bmesh/tools/bmesh_bevel.cc (Blender 5.2.4, GPL-2.0-or-later).

use super::math;
use super::types::{
    Affect, Bev, BevVert, BEVEL_EPSILON, Bevel, EdgeHalf, Eh, OffsetType, VMesh, Vm,
};
use super::util::{co, contig_ldata_across_edge, edge_calc_length, fno};
use crate::types::{BMesh, EdgeId, ElemFlag, FaceId, VertId};

impl Bevel {
    fn tag_test(&self, e: EdgeId) -> bool {
        self.edge_tag.get(e.slot()).copied().unwrap_or(false)
    }
    fn tag_set(&mut self, e: EdgeId, value: bool) {
        let s = e.slot();
        if self.edge_tag.len() <= s {
            self.edge_tag.resize(s + 1, false);
        }
        self.edge_tag[s] = value;
    }

    /// `edges_face_connected_at_vert`: do `bme1` and `bme2` (sharing a vertex) share a face?
    fn edges_face_connected_at_vert(bm: &BMesh, bme1: EdgeId, bme2: EdgeId) -> bool {
        bm.edge_loops(bme1).into_iter().any(|l| {
            let ll = bm.lp(l);
            bm.lp(ll.prev).e == bme2 || bm.lp(ll.next).e == bme2
        })
    }

    /// `bevel_edge_order_extend`: try to extend `bv->edges[]` beyond `i` by finding more
    /// successor edges; returns the index ending the best path found (tagged).
    fn bevel_edge_order_extend(&mut self, bm: &BMesh, bv: Bev, i: i32) -> i32 {
        let b = self.bev(bv).clone();
        let v = b.v;
        let bme = self.ehe(b.edge(i));
        let mut sucs: Vec<EdgeId> = Vec::new();
        for l in bm.edge_loops(bme) {
            let ll = bm.lp(l);
            let bme2 = if ll.v == v {
                bm.lp(ll.prev).e
            } else {
                bm.lp(ll.next).e
            };
            if !self.tag_test(bme2) {
                sucs.push(bme2);
            }
        }
        let mut save_path: Vec<EdgeId> = Vec::new();
        let mut bestj = i;
        let j = i;
        for nextbme in sucs {
            debug_assert!(!self.tag_test(nextbme));
            debug_assert!(j + 1 < b.edgecount);
            self.eh_mut(b.edge(j + 1)).e = Some(nextbme);
            self.tag_set(nextbme, true);
            let tryj = self.bevel_edge_order_extend(bm, bv, j + 1);
            if tryj > bestj
                || (tryj == bestj
                    && Self::edges_face_connected_at_vert(
                        bm,
                        self.ehe(b.edge(tryj)),
                        self.ehe(b.edge(0)),
                    ))
            {
                bestj = tryj;
                save_path.clear();
                for k in j + 1..=bestj {
                    save_path.push(self.ehe(b.edge(k)));
                }
            }
            // Now reset to the path only going to j.
            for k in j + 1..=tryj {
                let ek = self.ehe(b.edge(k));
                self.tag_set(ek, false);
                self.eh_mut(b.edge(k)).e = None;
            }
        }
        if bestj > j {
            for k in j + 1..=bestj {
                let e = save_path[(k - (j + 1)) as usize];
                self.eh_mut(b.edge(k)).e = Some(e);
                self.tag_set(e, true);
            }
        }
        bestj
    }

    /// `fast_bevel_edge_order` (the legacy O(n²) version Blender uses).
    fn fast_bevel_edge_order(&mut self, bm: &BMesh, bv: Bev) -> bool {
        let b = self.bev(bv).clone();
        let ntot = b.edgecount;
        let mut bme = self.ehe(b.edge(0));
        if bm.edge(bme).l.is_none() {
            return false;
        }
        for i in 1..ntot {
            // Find an unflagged edge bme2 that shares a face f with previous bme.
            let mut num_shared_face = 0;
            let mut first_suc: Option<EdgeId> = None;
            for bme2 in bm.vert_edges(b.v) {
                if self.tag_test(bme2) {
                    continue;
                }
                for f in bm.edge_faces(bme2) {
                    if bm.face_edge_share_loop(f, bme).is_some() {
                        num_shared_face += 1;
                        if first_suc.is_none() {
                            first_suc = Some(bme2);
                        }
                    }
                }
                if num_shared_face >= 3 {
                    break;
                }
            }
            if num_shared_face == 1 || (i == 1 && num_shared_face == 2) {
                bme = first_suc.expect("shared face successor");
                self.eh_mut(b.edge(i)).e = Some(bme);
                self.tag_set(bme, true);
            } else {
                for k in 1..i {
                    let ek = self.ehe(b.edge(k));
                    self.tag_set(ek, false);
                    self.eh_mut(b.edge(k)).e = None;
                }
                return false;
            }
        }
        true
    }

    /// `find_bevel_edge_order`: fills `bv->edges` with a good ordering of the non-wire edges
    /// around `bv->v`, then the faces between them.
    fn find_bevel_edge_order(&mut self, bm: &BMesh, bv: Bev, first_bme: EdgeId) {
        let b = self.bev(bv).clone();
        let ntot = b.edgecount;
        let mut first_bme = Some(first_bme);
        let mut i = 0;
        loop {
            let fb = first_bme.expect("first edge");
            self.eh_mut(b.edge(i)).e = Some(fb);
            self.tag_set(fb, true);
            if i == 0 && self.fast_bevel_edge_order(bm, bv) {
                break;
            }
            i = self.bevel_edge_order_extend(bm, bv, i);
            i += 1;
            if i >= b.edgecount {
                break;
            }
            // Not done yet: find a new first_bme.
            first_bme = None;
            for bme in bm.vert_edges(b.v) {
                if self.tag_test(bme) {
                    continue;
                }
                if first_bme.is_none() {
                    first_bme = Some(bme);
                }
                if bm.edge_face_count(bme) == 1 {
                    first_bme = Some(bme);
                    break;
                }
            }
        }
        // Now fill in the faces.
        for i in 0..ntot {
            let e = b.edge(i);
            let e2 = if i == b.edgecount - 1 {
                b.edge(0)
            } else {
                b.edge(i + 1)
            };
            let bme = self.ehe(e);
            let bme2 = self.ehe(e2);
            if self.eh(e).fnext.is_some() || self.eh(e2).fprev.is_some() {
                continue;
            }
            // Which faces have successive loops that are for bme and bme2? Prefer one that has
            // the loop in the same direction as e.
            let mut bestf: Option<FaceId> = None;
            for l in bm.edge_loops(bme) {
                let ll = bm.lp(l);
                let f = ll.f;
                if (bm.lp(ll.prev).e == bme2 || bm.lp(ll.next).e == bme2)
                    && (bestf.is_none() || ll.v == b.v)
                {
                    bestf = Some(f);
                }
                if bestf.is_some() {
                    self.eh_mut(e).fnext = bestf;
                    self.eh_mut(e2).fprev = bestf;
                }
            }
        }
    }

    /// `bev_ccw_test`: 1 if `a` and `b` are in CCW order on the normal side of `f`, -1 if
    /// reversed, 0 without a shared face.
    fn bev_ccw_test(bm: &BMesh, a: EdgeId, b: EdgeId, f: Option<FaceId>) -> i32 {
        let Some(f) = f else {
            return 0;
        };
        let (Some(la), Some(lb)) = (bm.face_edge_share_loop(f, a), bm.face_edge_share_loop(f, b))
        else {
            return 0;
        };
        if bm.lp(lb).next == la { 1 } else { -1 }
    }

    /// `edge_face_angle`: the angle between the two faces adjacent to `e` (0 without two).
    fn edge_face_angle(&self, bm: &BMesh, e: Eh) -> f32 {
        let eh = self.eh(e);
        if let (Some(fp), Some(fn_)) = (eh.fprev, eh.fnext) {
            return math::PI_F - math::angle_normalized_v3v3(fno(bm, fp), fno(bm, fn_));
        }
        0.0
    }

    /// `bevel_vert_construct`: the BevVert of `v`, or `None` (and `v` untagged) when `v` is
    /// not beveled.
    pub(crate) fn bevel_vert_construct(&mut self, bm: &mut BMesh, v: VertId) -> Option<Bev> {
        let mut nsel = 0;
        let mut tot_edges = 0;
        let mut tot_wire = 0;
        let mut first_bme: Option<EdgeId> = None;
        let vertices = self.affect_type == Affect::Vertices;
        for bme in bm.vert_edges(v) {
            let face_count = bm.edge_face_count(bme);
            self.tag_set(bme, false);
            if bm.edge(bme).flag.contains(ElemFlag::TAG) && !vertices {
                debug_assert_eq!(face_count, 2);
                nsel += 1;
                if first_bme.is_none() {
                    first_bme = Some(bme);
                }
            }
            if face_count == 1 {
                // Good to start the face chain from this edge.
                first_bme = Some(bme);
            }
            if face_count > 0 || vertices {
                tot_edges += 1;
            }
            if bm.edge_is_wire(bme) {
                tot_wire += 1;
                // If edge beveling, exclude wire edges from the edges array.
                if !vertices {
                    self.tag_set(bme, true);
                }
            }
        }
        if first_bme.is_none() {
            first_bme = bm.vert(v).e;
        }

        if (nsel == 0 && !vertices) || (tot_edges < 2 && vertices) {
            // Signal this vert isn't being beveled.
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
            return None;
        }

        let vm = Vm(self.vms.len() as u32);
        self.vms.push(VMesh {
            seg: self.seg,
            ..VMesh::default()
        });
        let first_eh = self.ehs.len() as u32;
        self.ehs
            .extend(std::iter::repeat_n(EdgeHalf::default(), tot_edges as usize));
        let bv = Bev(self.bevs.len() as u32);
        self.bevs.push(BevVert {
            v,
            edgecount: tot_edges,
            selcount: nsel,
            wirecount: tot_wire,
            offset: self.offset,
            any_seam: false,
            edges: first_eh,
            wire_edges: Vec::new(),
            vmesh: vm,
        });
        self.vert_hash.insert(v, bv);

        self.find_bevel_edge_order(bm, bv, first_bme.expect("vertex with edges"));

        // Fill in other attributes of EdgeHalfs.
        let b = self.bev(bv).clone();
        for i in 0..tot_edges {
            let e = b.edge(i);
            let bme = self.ehe(e);
            let tagged = bm.edge(bme).flag.contains(ElemFlag::TAG);
            let is_rev = bm.edge(bme).v[1] == v;
            let seg = self.seg;
            let eh = self.eh_mut(e);
            if tagged && !vertices {
                eh.is_bev = true;
                eh.seg = seg;
            } else {
                eh.is_bev = false;
                eh.seg = 0;
            }
            eh.is_rev = is_rev;
            eh.leftv = None;
            eh.rightv = None;
            eh.profile_index = 0;
        }

        // Now done with the tag flag.
        for bme in bm.vert_edges(v) {
            self.tag_set(bme, false);
        }

        // If the edge array doesn't go CCW around the vertex from the average normal side,
        // reverse the array, being careful to reverse the face pointers too.
        if tot_edges > 1 {
            let mut ccw_test_sum = 0;
            for i in 0..tot_edges {
                ccw_test_sum += Self::bev_ccw_test(
                    bm,
                    self.ehe(b.edge(i)),
                    self.ehe(b.edge((i + 1) % tot_edges)),
                    self.eh(b.edge(i)).fnext,
                );
            }
            if ccw_test_sum < 0 {
                for i in 0..=(tot_edges / 2) - 1 {
                    let (a, z) = (b.edge(i).0 as usize, b.edge(tot_edges - i - 1).0 as usize);
                    self.ehs.swap(a, z);
                    let ea = &mut self.ehs[a];
                    std::mem::swap(&mut ea.fprev, &mut ea.fnext);
                    let ez = &mut self.ehs[z];
                    std::mem::swap(&mut ez.fprev, &mut ez.fnext);
                }
                if tot_edges % 2 == 1 {
                    let m = &mut self.ehs[b.edge(tot_edges / 2).0 as usize];
                    std::mem::swap(&mut m.fprev, &mut m.fnext);
                }
            }
        }

        let mut vert_axis = [0.0f32; 3];
        if vertices {
            // Modify the offset by the bevel weight if it is used.
            if self.use_weights {
                let weight = self.bweight_vert.map_or(0.0, |li| float_at(&bm.vdata, li, v.slot()));
                self.bev_mut(bv).offset *= weight;
            }
            // Find the center axis. NOTE: Don't use the vertex normal.
            if matches!(self.offset_type, OffsetType::Width | OffsetType::Depth) {
                for i in 0..tot_edges {
                    let v2 = bm.edge_other_vert(self.ehe(b.edge(i)), v);
                    let edge_dir = math::norm(math::sub(co(bm, v), co(bm, v2)));
                    vert_axis = math::add(vert_axis, edge_dir);
                }
            }
        }

        // Set offsets for each beveled edge.
        let bv_offset = self.bev(bv).offset;
        for i in 0..tot_edges {
            let e = b.edge(i);
            {
                let eh = self.eh_mut(e);
                eh.next = b.edge((i + 1) % tot_edges);
                eh.prev = b.edge((i + tot_edges - 1) % tot_edges);
            }
            let bme = self.ehe(e);
            if self.eh(e).is_bev {
                // Convert the distance specified by the user into offsets along the faces on
                // the left and right sides of this edgehalf.
                let mut l_spec;
                let mut r_spec = 0.0;
                match self.offset_type {
                    OffsetType::Offset => l_spec = self.offset,
                    OffsetType::Width => {
                        let z = (2.0 * libm::sinf(self.edge_face_angle(bm, e) / 2.0)).abs();
                        l_spec = if z < BEVEL_EPSILON {
                            0.01 * self.offset
                        } else {
                            self.offset / z
                        };
                    }
                    OffsetType::Depth => {
                        let z = libm::cosf(self.edge_face_angle(bm, e) / 2.0).abs();
                        l_spec = if z < BEVEL_EPSILON {
                            0.01 * self.offset
                        } else {
                            self.offset / z
                        };
                    }
                    OffsetType::Percent => {
                        let pe = self.ehe(self.eprev(e));
                        let ne = self.ehe(self.enext(e));
                        l_spec = edge_calc_length(bm, pe) * self.offset / 100.0;
                        r_spec = edge_calc_length(bm, ne) * self.offset / 100.0;
                    }
                    OffsetType::Absolute => {
                        l_spec = self.offset;
                        r_spec = self.offset;
                    }
                }
                if !matches!(self.offset_type, OffsetType::Percent | OffsetType::Absolute) {
                    r_spec = l_spec;
                }
                if self.use_weights {
                    let weight = self
                        .bweight_edge
                        .map_or(0.0, |li| float_at(&bm.edata, li, bme.slot()));
                    l_spec *= weight;
                    r_spec *= weight;
                }
                let eh = self.eh_mut(e);
                eh.offset_l_spec = l_spec;
                eh.offset_r_spec = r_spec;
            } else if vertices {
                // Weight has already been applied to bv->offset, if present.
                let l_spec = match self.offset_type {
                    OffsetType::Offset | OffsetType::Absolute => bv_offset,
                    OffsetType::Width => {
                        let v2 = bm.edge_other_vert(bme, v);
                        let edge_dir = math::sub(co(bm, v), co(bm, v2));
                        let z = (2.0 * libm::sinf(math::angle_v3v3(vert_axis, edge_dir))).abs();
                        if z < BEVEL_EPSILON {
                            0.01 * self.offset
                        } else {
                            self.offset / z
                        }
                    }
                    OffsetType::Depth => {
                        let v2 = bm.edge_other_vert(bme, v);
                        let edge_dir = math::sub(co(bm, v), co(bm, v2));
                        let z = libm::cosf(math::angle_v3v3(vert_axis, edge_dir)).abs();
                        if z < BEVEL_EPSILON {
                            0.01 * self.offset
                        } else {
                            self.offset / z
                        }
                    }
                    OffsetType::Percent => edge_calc_length(bm, bme) * bv_offset / 100.0,
                };
                let eh = self.eh_mut(e);
                eh.offset_l_spec = l_spec;
                eh.offset_r_spec = l_spec;
            } else {
                let eh = self.eh_mut(e);
                eh.offset_l_spec = 0.0;
                eh.offset_r_spec = 0.0;
            }
            let (fp, fnx) = {
                let eh = self.eh_mut(e);
                eh.offset_l = eh.offset_l_spec;
                eh.offset_r = eh.offset_r_spec;
                (eh.fprev, eh.fnext)
            };
            let is_seam = match (fp, fnx) {
                (Some(fp), Some(fnx)) => !contig_ldata_across_edge(bm, bme, fp, fnx),
                _ => true,
            };
            self.eh_mut(e).is_seam = is_seam;
        }

        // Collect wire edges if we found any earlier.
        if tot_wire != 0 {
            let wires: Vec<EdgeId> = bm
                .vert_edges(v)
                .into_iter()
                .filter(|&e| bm.edge_is_wire(e))
                .collect();
            debug_assert_eq!(wires.len() as i32, tot_wire);
            self.bev_mut(bv).wire_edges = wires;
        }

        Some(bv)
    }
}

/// `BM_ELEM_CD_GET_FLOAT` of a `FLOAT` layer at an element slot.
pub(crate) fn float_at(cd: &crate::customdata::CustomData, layer: usize, slot: usize) -> f32 {
    match &cd.layers[layer].data {
        frap_mesh::AttrData::Float(c) => c.get(slot).copied().unwrap_or(0.0),
        _ => 0.0,
    }
}
