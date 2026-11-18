// SPDX-License-Identifier: GPL-3.0-or-later
//! Geometry construction: the new faces around beveled vertices (`build_vmesh` and the
//! poly, tri-fan, rings and cutoff patterns), the edge polygons, the rebuilt original faces,
//! wire reattachment, edge-data extension and face strength. Every face is created through
//! [`Bevel::bev_create_ngon`]. Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc
//! (Blender 5.2.4, GPL-2.0-or-later).

use std::sync::Arc;

use frap_mesh::AttrData;
use rustc_hash::FxHashMap;

use super::math::{self, V3};
use super::types::{
    Affect, BEVEL_EPSILON, BEVEL_EPSILON_BIG, Bev, Bevel, Bnd, EDGE_OUT, Eh, FKind, FaceStrength,
    MeshKind, PRO_LINE_R, PRO_SQUARE_IN_R, PRO_SQUARE_R, VERT_OUT, Vm,
};
use super::util::{co, fno, vert_face_check};
use crate::customdata::CdLayer;
use crate::query::geom;
use crate::types::{BMElem, BMesh, CreateFlag, EdgeId, ElemFlag, FaceId, VertId};

/// `BM_ELEM_LONG_TAG`: the unused tag Blender uses for the long loops of edge polygons.
pub(crate) const LONG_TAG: ElemFlag = ElemFlag::TAG_ALT;

/// `FACE_STRENGTH_*`.
const FACE_STRENGTH_WEAK: i32 = -16384;
const FACE_STRENGTH_MEDIUM: i32 = 0;
const FACE_STRENGTH_STRONG: i32 = 16384;

/// `MOD_WEIGHTEDNORMALS_FACEWEIGHT_CDLAYER_ID`.
const FACEWEIGHT_LAYER: &str = "__mod_weightednormals_faceweight";

impl Bevel {
    /// `bev_create_ngon`: a face over `verts` with attributes from `facerep` (else
    /// `face_arr[0]`) and loop data interpolated from the corresponding `face_arr` faces (else
    /// `facerep`), snapping corners to `snap_edge_arr` edges for the interpolation. ALL face
    /// creation goes through here.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bev_create_ngon(
        &mut self,
        bm: &mut BMesh,
        verts: &[VertId],
        face_arr: Option<&[Option<FaceId>]>,
        facerep: Option<FaceId>,
        snap_edge_arr: Option<&[Option<EdgeId>]>,
        bv: Option<VertId>,
        nv_bv_map: Option<&FxHashMap<VertId, VertId>>,
        mat_nr: i32,
        do_interp: bool,
    ) -> Option<FaceId> {
        let f = bm.face_create_verts(verts, facerep, CreateFlag::empty(), true)?;
        let first_arr = face_arr.and_then(|a| a.first().copied().flatten());
        if let Some(src) = facerep.or(first_arr) {
            if src != f {
                bm.face_attrs_copy(src, f);
            }
            if do_interp {
                for (i, l) in bm.face_loops(f).into_iter().enumerate() {
                    let interp_f = match face_arr {
                        Some(a) => {
                            debug_assert_eq!(bm.lp(l).v, verts[i]);
                            a[i]
                        }
                        None => facerep,
                    };
                    let Some(interp_f) = interp_f else {
                        continue;
                    };
                    let bme = snap_edge_arr.and_then(|s| s[i]);
                    let lv = bm.lp(l).v;
                    let save_co = co(bm, lv);
                    if let Some(bme) = bme {
                        let [a, b] = bm.edge(bme).v;
                        bm.vert_mut(lv).co =
                            math::closest_to_line_segment_v3(save_co, co(bm, a), co(bm, b));
                    }
                    bm.loop_interp_from_face(l, interp_f, true);
                    if bme.is_some() {
                        bm.vert_mut(lv).co = save_co;
                    }
                }
            }
        }
        bm.face_mut(f).flag.insert(ElemFlag::TAG);
        for e in bm.face_edges(f) {
            // So the operator can select the newly created geometry.
            bm.oflag_enable(e, EDGE_OUT);
        }
        if mat_nr >= 0 {
            bm.face_mut(f).mat_nr = mat_nr as i16;
        }
        let attached = first_arr.or(facerep);
        self.register_and_update_uv(bm, f, attached, bv, nv_bv_map);
        Some(f)
    }

    /// `create_mesh_bmvert`: the BMVert of `mesh_vert(vm, i, j, k)`.
    fn create_mesh_bmvert(&mut self, bm: &mut BMesh, vm: Vm, i: i32, j: i32, k: i32, eg: VertId) {
        let p = self.vm(vm).mv(i, j, k).co;
        let v = bm.vert_create(p, Some(eg), CreateFlag::empty());
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        bm.oflag_enable(v, VERT_OUT);
        self.vm_mut(vm).mv_mut(i, j, k).v = Some(v);
    }

    #[inline]
    fn mvv(&self, vm: Vm, i: i32, j: i32, k: i32) -> VertId {
        self.vm(vm).mv(i, j, k).v.expect("mesh vertex")
    }

    /// `boundvert_rep_face`: a representative face for faces created around `v`, and maybe
    /// a second, different choice.
    fn boundvert_rep_face(&self, v: Bnd) -> (Option<FaceId>, Option<FaceId>) {
        let b = self.bnd(v);
        let mut frep2 = None;
        let frep;
        if let Some(ebev) = b.ebev {
            frep = self.eh(ebev).fprev;
            let ef = b.efirst.expect("efirst");
            if self.eh(ef).fprev != frep {
                frep2 = self.eh(ef).fprev;
            }
        } else if let Some(ef) = b.efirst {
            let el = b.elast.expect("elast");
            let (ef_, el_) = (self.eh(ef), self.eh(el));
            if ef_.fprev.is_some() {
                frep = ef_.fprev;
                if el_.fnext != frep {
                    frep2 = el_.fnext;
                } else if ef_.fnext != frep {
                    frep2 = ef_.fnext;
                } else if el_.fprev != frep {
                    frep2 = ef_.fprev;
                }
            } else if ef_.fnext.is_some() {
                frep = ef_.fnext;
                if el_.fnext != frep {
                    frep2 = el_.fnext;
                }
            } else {
                frep = el_.fprev;
            }
        } else if let Some(pel) = self.bnd(b.prev).elast {
            let mut fr = self.eh(pel).fnext;
            if let Some(nef) = self.bnd(b.next).efirst {
                if fr.is_some() {
                    frep2 = self.eh(nef).fprev;
                } else {
                    fr = self.eh(nef).fprev;
                }
            }
            frep = fr;
        } else {
            frep = None;
        }
        (frep, frep2)
    }

    /// `choose_rep_face`: a tie-breaking choice among `faces` (null entries allowed).
    fn choose_rep_face(&self, bm: &BMesh, faces: &[Option<FaceId>]) -> Option<FaceId> {
        const LEN: usize = 6;
        let nfaces = faces.len();
        let mut value_vecs = vec![[0.0f32; LEN]; nfaces];
        let mut still_viable = vec![false; nfaces];
        let mut num_viable = 0;
        for (f, bmf) in faces.iter().enumerate() {
            let Some(bmf) = *bmf else {
                continue;
            };
            still_viable[f] = true;
            num_viable += 1;
            let idx = bm.index_get(BMElem::Face(bmf));
            let fa = bm.face(bmf);
            let cent = super::util::face_calc_center_bounds(bm, bmf);
            value_vecs[f] = [
                // First tie-breaker: lower math-layer connected component id.
                match &self.math_layer_info.face_component {
                    Some(fc) => usize::try_from(idx)
                        .ok()
                        .and_then(|i| fc.get(i))
                        .map_or(0.0, |&c| c as f32),
                    None => 0.0,
                },
                // Selected face beats unselected one.
                if fa.flag.contains(ElemFlag::SELECT) { 0.0 } else { 1.0 },
                // Lower material index.
                if fa.mat_nr >= 0 { f32::from(fa.mat_nr) } else { 0.0 },
                // z, x, y components of the face center.
                cent[2],
                cent[0],
                cent[1],
            ];
        }
        // A face with a unique minimum value for a value_index, trying each index in turn.
        let mut best_f: Option<usize> = None;
        let mut value_index = 0;
        while num_viable > 1 && value_index < LEN {
            for f in 0..nfaces {
                if !still_viable[f] || Some(f) == best_f {
                    continue;
                }
                let Some(b) = best_f else {
                    best_f = Some(f);
                    continue;
                };
                if value_vecs[f][value_index] < value_vecs[b][value_index] {
                    best_f = Some(f);
                    // Previous f's are not viable any more.
                    for i in (0..f).rev() {
                        if still_viable[i] {
                            still_viable[i] = false;
                            num_viable -= 1;
                        }
                    }
                } else if value_vecs[f][value_index] > value_vecs[b][value_index] {
                    still_viable[f] = false;
                    num_viable -= 1;
                }
            }
            value_index += 1;
        }
        faces[best_f.unwrap_or(0)]
    }

    /// `get_incident_edges`: the (up to) two edges of `f` incident to `v`.
    fn get_incident_edges(bm: &BMesh, f: Option<FaceId>, v: VertId) -> (Option<EdgeId>, Option<EdgeId>) {
        let mut r = (None, None);
        let Some(f) = f else {
            return r;
        };
        for e in bm.face_edges(f) {
            let [a, b] = bm.edge(e).v;
            if a == v || b == v {
                if r.0.is_none() {
                    r.0 = Some(e);
                } else if r.1.is_none() {
                    r.1 = Some(e);
                }
            }
        }
        r
    }

    /// `find_closer_edge`.
    fn find_closer_edge(bm: &BMesh, p: V3, e1: EdgeId, e2: EdgeId) -> EdgeId {
        let [a1, b1] = bm.edge(e1).v;
        let [a2, b2] = bm.edge(e2).v;
        let dsq1 = math::dist_squared_to_line_segment_v3(p, co(bm, a1), co(bm, b1));
        let dsq2 = math::dist_squared_to_line_segment_v3(p, co(bm, a2), co(bm, b2));
        if dsq1 < dsq2 { e1 } else { e2 }
    }

    /// `find_face_internal_boundverts`: the (up to 3) BoundVerts of `bv` inside face `f`.
    fn find_face_internal_boundverts(&self, bm: &BMesh, bv: Bev, f: Option<FaceId>) -> [Option<Bnd>; 3] {
        let mut out = [None; 3];
        let Some(f) = f else {
            return out;
        };
        let mut n = 0;
        for v in self.bounds(self.bvm(bv)) {
            if super::util::face_point_inside_test(bm, f, self.bnd(v).nv.co) {
                out[n] = Some(v);
                n += 1;
                if n == 3 {
                    break;
                }
            }
        }
        out
    }

    /// `projected_boundary_area`: the 2D area of the BoundVerts of `bv` snapped into `f`.
    fn projected_boundary_area(&self, bm: &BMesh, bv: Bev, f: FaceId) -> f32 {
        let rows = geom::axis_dominant_v3_rows(fno(bm, f));
        let (e1, e2) = Self::get_incident_edges(bm, Some(f), self.bev(bv).v);
        let (e1, e2) = (e1.expect("incident edge"), e2.expect("incident edge"));
        let unsnapped = self.find_face_internal_boundverts(bm, bv, Some(f));
        let mut proj_co = Vec::new();
        for v in self.bounds(self.bvm(bv)) {
            let p = co(bm, self.bnd(v).nv.v.expect("boundary vertex"));
            if unsnapped.contains(&Some(v)) {
                proj_co.push(geom::project_v2(&rows, p));
            } else {
                let [a1, b1] = bm.edge(e1).v;
                let [a2, b2] = bm.edge(e2).v;
                let snap1 = math::closest_to_line_segment_v3(p, co(bm, a1), co(bm, b1));
                let snap2 = math::closest_to_line_segment_v3(p, co(bm, a2), co(bm, b2));
                let d1_sq = math::len_sq_v3v3(snap1, p);
                let d2_sq = math::len_sq_v3v3(snap2, p);
                proj_co.push(geom::project_v2(&rows, if d1_sq <= d2_sq { snap1 } else { snap2 }));
            }
        }
        math::area_poly_v2(&proj_co)
    }

    /// `frep_for_center_poly`: a good representative face among the faces around `bv`.
    fn frep_for_center_poly(&self, bm: &BMesh, bv: Bev) -> Option<FaceId> {
        let b = self.bev(bv);
        let mut any_bmf = None;
        let consider_all_faces = b.selcount == 1 || self.affect_vertices_odd;
        let mut fchoices: Vec<Option<FaceId>> = Vec::new();
        for i in 0..b.edgecount {
            let eh = self.eh(b.edge(i));
            if !eh.is_bev && !consider_all_faces {
                continue;
            }
            let Some(bmf) = self.choose_rep_face(bm, &[eh.fprev, eh.fnext]) else {
                continue;
            };
            if any_bmf.is_none() {
                any_bmf = Some(bmf);
            }
            if !fchoices.contains(&Some(bmf)) {
                if self.math_layer_info.has_math_layers
                    && self.projected_boundary_area(bm, bv, bmf) < BEVEL_EPSILON_BIG
                {
                    continue; // A bad UV poly.
                }
                fchoices.push(Some(bmf));
            }
        }
        if fchoices.is_empty() {
            return any_bmf;
        }
        self.choose_rep_face(bm, &fchoices)
    }

    /// `build_center_ngon`.
    fn build_center_ngon(&mut self, bm: &mut BMesh, bv: Bev, mat_nr: i32) {
        let vm = self.bvm(bv);
        let ns2 = self.vm(vm).seg / 2;
        let bvv = self.bev(bv).v;
        let (frep, frep_e1, frep_e2, frep_unsnapped) = if self.bev(bv).any_seam {
            let frep = self.frep_for_center_poly(bm, bv);
            let (e1, e2) = Self::get_incident_edges(bm, frep, bvv);
            let un = self.find_face_internal_boundverts(bm, bv, frep);
            (frep, e1, e2, un)
        } else {
            (None, None, None, [None; 3])
        };
        let mut vv = Vec::new();
        let mut vf = Vec::new();
        let mut ve = Vec::new();
        for v in self.bounds(vm) {
            let i = self.bnd(v).index;
            let mv = self.mvv(vm, i, ns2, ns2);
            vv.push(mv);
            if frep.is_some() {
                vf.push(frep);
                if frep_unsnapped.contains(&Some(v)) {
                    ve.push(None);
                } else {
                    ve.push(Some(Self::find_closer_edge(
                        bm,
                        co(bm, mv),
                        frep_e1.expect("edge"),
                        frep_e2.expect("edge"),
                    )));
                }
            } else {
                vf.push(self.boundvert_rep_face(v).0);
                ve.push(None);
            }
        }
        let f = self.bev_create_ngon(bm, &vv, Some(&vf), frep, Some(&ve), Some(bvv), None, mat_nr, true);
        self.record_face_kind(f, FKind::Vert);
    }

    /// `build_square_in_vmesh`: triangle corner with profile 0: boundary verts merge with
    /// the previous ones; only a center poly for odd segment counts.
    fn build_square_in_vmesh(&mut self, bm: &mut BMesh, bv: Bev, vm1: Vm) {
        let vm = self.bvm(bv);
        let n = self.vm(vm).count;
        let ns = self.vm(vm).seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        let bvv = self.bev(bv).v;
        for i in 0..n {
            for k in 1..ns {
                let c = self.vm(vm1).mv(i, 0, k).co;
                self.vm_mut(vm).mv_mut(i, 0, k).co = c;
                if i > 0 && k <= ns2 {
                    let v = self.vm(vm).mv(i - 1, 0, ns - k).v;
                    self.vm_mut(vm).mv_mut(i, 0, k).v = v;
                } else if i == n - 1 && k > ns2 {
                    let v = self.vm(vm).mv(0, 0, ns - k).v;
                    self.vm_mut(vm).mv_mut(i, 0, k).v = v;
                } else {
                    self.create_mesh_bmvert(bm, vm, i, 0, k, bvv);
                }
            }
        }
        if odd == 1 {
            for i in 0..n {
                let v = self.vm(vm).mv(i, 0, ns2).v;
                self.vm_mut(vm).mv_mut(i, ns2, ns2).v = v;
            }
            self.build_center_ngon(bm, bv, self.mat_nr);
        }
    }

    /// `bevel_build_rings`: the ADJ pattern: interior mesh points by cubic subdivision, then
    /// the BMVerts and faces.
    fn bevel_build_rings(&mut self, bm: &mut BMesh, bv: Bev, vpipe: Option<Bnd>) {
        let mat_nr = self.mat_nr;
        let bvm = self.bvm(bv);
        let n_bndv = self.vm(bvm).count;
        let ns = self.vm(bvm).seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        let bvv = self.bev(bv).v;
        debug_assert!(n_bndv >= 3 && ns > 1);
        let edges_aff = self.affect_type == Affect::Edges;

        let vm1 = if self.pro_super_r == PRO_SQUARE_R && self.bev(bv).selcount >= 3 && odd == 0 {
            self.square_out_adj_vmesh(bm, bv)
        } else if let Some(vp) = vpipe {
            self.pipe_adj_vmesh(bm, bv, vp)
        } else if self.tri_corner_test(bm, bv) == 1 {
            let vm1 = self.tri_corner_adj_vmesh(bm, bv);
            // PRO_SQUARE_IN_R has merging boundary edges and no internal ring polys except
            // maybe a center ngon.
            if self.pro_super_r == PRO_SQUARE_IN_R {
                self.build_square_in_vmesh(bm, bv, vm1);
                return;
            }
            vm1
        } else {
            self.adj_vmesh(bm, bv)
        };

        // Copy the final vmesh into bv->vmesh, make BMVerts and BMFaces.
        let vm = bvm;
        for i in 0..n_bndv {
            for j in 0..=ns2 {
                for k in 0..=ns {
                    if j == 0 && (k == 0 || k == ns) {
                        continue; // Boundary corners already made.
                    }
                    if !self.vm(vm).is_canon(i, j, k) {
                        continue;
                    }
                    let c = self.vm(vm1).mv(i, j, k).co;
                    self.vm_mut(vm).mv_mut(i, j, k).co = c;
                    self.create_mesh_bmvert(bm, vm, i, j, k, bvv);
                }
            }
        }
        self.vm_mut(vm).copy_equiv_verts();

        // The interpolation face of each BoundVert.
        let bounds = self.bounds(vm);
        let mut bndv_rep_faces: Vec<Option<FaceId>> = vec![None; n_bndv as usize];
        for &b in &bounds {
            bndv_rep_faces[self.bnd(b).index as usize] = self.boundvert_rep_face(b).0;
        }

        // With an odd number of segments, data to break interpolation ties.
        let mut center_verts: Vec<VertId> = vec![VertId(0); n_bndv as usize];
        let mut center_edge_snaps: Vec<Option<EdgeId>> = vec![None; n_bndv as usize];
        let mut center_face_interps: Vec<Option<FaceId>> = vec![None; n_bndv as usize];
        let mut frep_beats_next: Vec<bool> = vec![false; n_bndv as usize];
        let mut center_frep = None;
        if odd == 1 && edges_aff {
            center_frep = self.frep_for_center_poly(bm, bv);
            for i in 0..n_bndv as usize {
                let inext = (i + 1) % n_bndv as usize;
                let fchoices = [bndv_rep_faces[i], bndv_rep_faces[inext]];
                let fwinner = self.choose_rep_face(bm, &fchoices);
                frep_beats_next[i] = fwinner == bndv_rep_faces[i];
            }
        }
        // Make the polygons.
        for &bndv in &bounds {
            let i = self.bnd(bndv).index;
            let nx = self.bnext(bndv);
            let pv = self.bprev(bndv);
            let inext = self.bnd(nx).index;
            let f = bndv_rep_faces[i as usize];
            let f2 = bndv_rep_faces[inext as usize];
            let fc = if odd == 1 && edges_aff {
                if frep_beats_next[i as usize] { f } else { f2 }
            } else {
                None
            };
            let (e, eprev, enext) = if !edges_aff {
                (self.bnd(bndv).efirst, self.bnd(pv).efirst, self.bnd(nx).efirst)
            } else {
                (self.bnd(bndv).ebev, self.bnd(pv).ebev, self.bnd(nx).ebev)
            };
            let bme = e.map(|e| self.ehe(e));
            let bmeprev = eprev.map(|e| self.ehe(e));
            let bmenext = enext.map(|e| self.ehe(e));
            // For odd ns, polys with lower left corner at (i,j,k) for j in [0, ns2-1], k in
            // [0, ns2], then the center ngon; for even ns, j and k in [0, ns2-1].
            for j in 0..ns2 {
                for k in 0..ns2 + odd {
                    let bmvs = [
                        self.mvv(vm, i, j, k),
                        self.mvv(vm, i, j, k + 1),
                        self.mvv(vm, i, j + 1, k + 1),
                        self.mvv(vm, i, j + 1, k),
                    ];
                    let mut fr: [Option<FaceId>; 4];
                    let mut se: [Option<EdgeId>; 4] = [None; 4];
                    if !edges_aff {
                        fr = [f2; 4];
                        if j < k {
                            if k == ns2 && j == ns2 - 1 {
                                let ne = self.bnd(nx).efirst.expect("efirst");
                                se[2] = Some(self.ehe(ne));
                                se[3] = bme;
                            }
                        } else if j == k {
                            // Only one edge attached to v, since vertex only.
                            se[0] = bme;
                            se[2] = bme;
                            if !self.eh(e.expect("vertex edge")).is_seam {
                                fr[3] = f;
                            }
                        }
                    } else {
                        // Edge bevel.
                        fr = [f; 4];
                        if odd == 1 {
                            let b1 = if eprev.is_some_and(|x| self.eh(x).is_seam) { bmeprev } else { None };
                            let b2 = if e.is_some_and(|x| self.eh(x).is_seam) { bme } else { None };
                            let b3 = if enext.is_some_and(|x| self.eh(x).is_seam) { bmenext } else { None };
                            se = snap_edges_for_vmesh_vert(
                                i,
                                j,
                                k,
                                ns,
                                ns2,
                                n_bndv,
                                b1,
                                b2,
                                b3,
                                &bndv_rep_faces,
                                center_frep,
                                &frep_beats_next,
                            );
                            if k == ns2 {
                                if e.is_none_or(|x| self.eh(x).is_seam) {
                                    fr = [fc; 4];
                                } else {
                                    fr[0] = f;
                                    fr[3] = f;
                                    fr[1] = f2;
                                    fr[2] = f2;
                                }
                                if j == ns2 - 1 {
                                    // The 4th vertex of these faces is used for the center poly.
                                    center_verts[i as usize] = bmvs[3];
                                    center_edge_snaps[i as usize] = se[3];
                                    center_face_interps[i as usize] =
                                        if self.bev(bv).any_seam { center_frep } else { f };
                                }
                            }
                        } else {
                            // Edge bevel, even number of segments.
                            if k == ns2 - 1 {
                                se[1] = bme;
                            }
                            if j == ns2 - 1 && self.bnd(pv).ebev.is_some() {
                                se[3] = bmeprev;
                            }
                            se[2] = if se[1].is_some() { se[1] } else { se[3] };
                        }
                    }
                    let r_f = self.bev_create_ngon(
                        bm,
                        &bmvs,
                        Some(&fr),
                        None,
                        Some(&se),
                        Some(bvv),
                        None,
                        mat_nr,
                        true,
                    );
                    self.record_face_kind(r_f, FKind::Vert);
                }
            }
        }

        // Center ngon.
        if odd == 1 {
            if edges_aff {
                let frep = if self.bev(bv).any_seam {
                    self.frep_for_center_poly(bm, bv)
                } else {
                    None
                };
                let cen_f = self.bev_create_ngon(
                    bm,
                    &center_verts,
                    Some(&center_face_interps),
                    frep,
                    Some(&center_edge_snaps),
                    Some(bvv),
                    None,
                    mat_nr,
                    true,
                );
                self.record_face_kind(cen_f, FKind::Vert);
            } else {
                self.build_center_ngon(bm, bv, mat_nr);
            }
        }
    }

    /// `bevel_build_cutoff`: the "cutoff" vertex mesh, a face closing off each incoming
    /// profile.
    fn bevel_build_cutoff(&mut self, bm: &mut BMesh, bv: Bev) {
        let vm = self.bvm(bv);
        let n_bndv = self.vm(vm).count;
        let bvv = self.bev(bv).v;
        let vno = bm.vert(bvv).no;
        let bounds = self.bounds(vm);
        // The corner vertices at the bottom of the cutoff faces.
        for &bndv in &bounds {
            let i = self.bnd(bndv).index;
            let pv = self.bprev(bndv);
            // "Down": along the intersection of the two adjacent profile planes.
            let mut down_direction =
                math::cross(self.bnd(bndv).profile.plane_no, self.bnd(pv).profile.plane_no);
            if math::dot(down_direction, vno) > 0.0 {
                down_direction = math::neg(down_direction);
            }
            // Move down by the average profile height of the two adjacent profiles.
            let length = (self.bnd(bndv).profile.height / 2.0f32.sqrt()
                + self.bnd(pv).profile.height / 2.0f32.sqrt())
                / 2.0;
            let new_vert = math::madd(self.bnd(bndv).nv.co, down_direction, length);
            // This profile's first corner vert and the last profile's second.
            let pi = self.bnd(pv).index;
            self.vm_mut(vm).mv_mut(i, 1, 0).co = new_vert;
            self.vm_mut(vm).mv_mut(pi, 1, 1).co = new_vert;
        }
        // Disable the center face if the corner vertices share the same location.
        let mut build_center_face = true;
        if n_bndv == 3 {
            let m = self.vm(vm);
            let (a, b, c) = (m.mv(0, 1, 0).co, m.mv(1, 1, 0).co, m.mv(2, 1, 0).co);
            build_center_face &= math::len_sq_v3v3(a, b) > BEVEL_EPSILON;
            build_center_face &= math::len_sq_v3v3(a, c) > BEVEL_EPSILON;
            build_center_face &= math::len_sq_v3v3(b, c) > BEVEL_EPSILON;
        }
        // The corner vertex BMVerts.
        if build_center_face {
            for &bndv in &bounds {
                let i = self.bnd(bndv).index;
                self.create_mesh_bmvert(bm, vm, i, 1, 0, bvv);
                // The second corner vertex of the previous profile shares this BMVert.
                let pi = self.bnd(self.bprev(bndv)).index;
                let v = self.vm(vm).mv(i, 1, 0).v;
                self.vm_mut(vm).mv_mut(pi, 1, 1).v = v;
            }
        } else {
            // The same BMVert for all of the corner vertices.
            self.create_mesh_bmvert(bm, vm, 0, 1, 0, bvv);
            let v = self.vm(vm).mv(0, 1, 0).v;
            for i in 1..n_bndv {
                self.vm_mut(vm).mv_mut(i, 1, 0).v = v;
            }
        }
        // The profile cutoff faces.
        let seg = self.seg;
        for &bndv in &bounds {
            let i = self.bnd(bndv).index;
            let mut face_bmverts = vec![self.mvv(vm, i, 1, 0)];
            for k in 0..seg + 1 {
                face_bmverts.push(self.mvv(vm, i, 0, k));
            }
            if build_center_face {
                face_bmverts.push(self.mvv(vm, i, 1, 1));
            }
            self.bev_create_ngon(bm, &face_bmverts, None, None, None, Some(bvv), None, self.mat_nr, true);
        }
        // The bottom face.
        if build_center_face {
            let face_bmverts: Vec<VertId> = (0..n_bndv).map(|i| self.mvv(vm, i, 1, 0)).collect();
            self.bev_create_ngon(bm, &face_bmverts, None, None, None, Some(bvv), None, self.mat_nr, true);
        }
    }

    /// `bevel_build_poly`: a single polygon (with the profile points of beveled edges).
    fn bevel_build_poly(&mut self, bm: &mut BMesh, bv: Bev) -> Option<FaceId> {
        let vm = self.bvm(bv);
        let bvv = self.bev(bv).v;
        let (repface, re1, re2, unsnapped) = if self.bev(bv).any_seam {
            let rf = self.frep_for_center_poly(bm, bv);
            let (e1, e2) = Self::get_incident_edges(bm, rf, bvv);
            let un = self.find_face_internal_boundverts(bm, bv, rf);
            (rf, e1, e2, un)
        } else {
            (None, None, None, [None; 3])
        };
        let mut bmverts = Vec::new();
        let mut bmedges = Vec::new();
        let mut bmfaces = Vec::new();
        for bndv in self.bounds(vm) {
            // Vertices of the vertex ngon, and the faces in which to interpolate UVs.
            let nvv = self.bnd(bndv).nv.v.expect("boundary vertex");
            bmverts.push(nvv);
            if repface.is_some() {
                bmfaces.push(repface);
                if unsnapped.contains(&Some(bndv)) {
                    bmedges.push(None);
                } else {
                    bmedges.push(Some(Self::find_closer_edge(
                        bm,
                        co(bm, nvv),
                        re1.expect("edge"),
                        re2.expect("edge"),
                    )));
                }
            } else {
                bmfaces.push(self.boundvert_rep_face(bndv).0);
                bmedges.push(None);
            }
            if let Some(ebev) = self.bnd(bndv).ebev {
                let eseg = self.eh(ebev).seg;
                if eseg > 1 {
                    let idx = self.bnd(bndv).index;
                    for k in 1..eseg {
                        let mv = self.mvv(vm, idx, 0, k);
                        bmverts.push(mv);
                        if repface.is_some() {
                            bmfaces.push(repface);
                            let fe = Self::find_closer_edge(
                                bm,
                                co(bm, mv),
                                re1.expect("edge"),
                                re2.expect("edge"),
                            );
                            bmedges.push(if k < eseg / 2 { None } else { Some(fe) });
                        } else {
                            bmfaces.push(self.boundvert_rep_face(bndv).0);
                            bmedges.push(None);
                        }
                    }
                }
            }
        }
        if bmverts.len() > 2 {
            let f = self.bev_create_ngon(
                bm,
                &bmverts,
                Some(&bmfaces),
                repface,
                Some(&bmedges),
                Some(bvv),
                None,
                self.mat_nr,
                true,
            );
            self.record_face_kind(f, FKind::Vert);
            f
        } else {
            None
        }
    }

    /// `bevel_build_trifan`: the poly, split into a fan from its last vertex.
    fn bevel_build_trifan(&mut self, bm: &mut BMesh, bv: Bev) {
        let Some(mut f) = self.bevel_build_poly(bm, bv) else {
            return;
        };
        // The polygon starts at the previous vertex: make it into a fan.
        let mut l_fan = bm.lp(bm.face(f).l_first).prev;
        let v_fan = bm.lp(l_fan).v;
        while bm.face(f).len > 3 {
            debug_assert_eq!(v_fan, bm.lp(l_fan).v);
            let l_b = bm.lp(bm.lp(l_fan).next).next;
            let Some((f_new, l_new)) = bm.face_split(f, l_fan, l_b, None, false) else {
                break;
            };
            bm.oflag_enable(bm.lp(l_new).e, EDGE_OUT);
            if bm.face(f_new).len > bm.face(f).len {
                f = f_new;
                let ln = bm.lp(l_new);
                if ln.v == v_fan {
                    l_fan = l_new;
                } else if bm.lp(ln.next).v == v_fan {
                    l_fan = ln.next;
                } else if bm.lp(ln.prev).v == v_fan {
                    l_fan = ln.prev;
                } else {
                    debug_assert!(false, "fan vertex not found");
                }
            } else {
                let lf = bm.lp(l_fan);
                if lf.v == v_fan {
                    // l_fan stays.
                } else if bm.lp(lf.next).v == v_fan {
                    l_fan = lf.next;
                } else if bm.lp(lf.prev).v == v_fan {
                    l_fan = lf.prev;
                } else {
                    debug_assert!(false, "fan vertex not found");
                }
            }
            self.record_face_kind(Some(f_new), FKind::Vert);
        }
    }

    /// `bevel_vert_two_edges`: vertex bevel with two boundary verts: a curved edge for
    /// `seg > 1`, and the edges themselves when there is no face at the vertex.
    fn bevel_vert_two_edges(&mut self, bm: &mut BMesh, bv: Bev) {
        let vm = self.bvm(bv);
        debug_assert!(self.vm(vm).count == 2 && self.affect_type == Affect::Vertices);
        let bvv = self.bev(bv).v;
        let v1 = self.mvv(vm, 0, 0, 0);
        let v2 = self.mvv(vm, 1, 0, 0);
        let ns = self.vm(vm).seg;
        if ns > 1 {
            // Set up the profile parameters.
            let bndv = self.bstart(vm);
            let (c1, c2, cv) = (co(bm, v1), co(bm, v2), co(bm, bvv));
            let super_r = self.pro_super_r;
            {
                let pro = &mut self.bnd_mut(bndv).profile;
                pro.super_r = super_r;
                pro.start = c1;
                pro.end = c2;
                pro.middle = cv;
                // Don't use projection.
                pro.plane_co = [0.0; 3];
                pro.plane_no = [0.0; 3];
                pro.proj_dir = [0.0; 3];
            }
            for k in 1..ns {
                let p = self.get_profile_point(bndv, k, ns);
                self.vm_mut(vm).mv_mut(0, 0, k).co = p;
                self.create_mesh_bmvert(bm, vm, 0, 0, k, bvv);
            }
            self.vm_mut(vm).mv_mut(0, 0, ns).co = c2;
            for k in 1..ns {
                self.vm_mut(vm).copy_mesh_vert(1, 0, ns - k, 0, 0, k);
            }
        }
        if !vert_face_check(bm, bvv) {
            let e_eg = self.ehe(self.bev(bv).edge(0));
            for k in 0..ns {
                let a = self.mvv(vm, 0, 0, k);
                let b = self.mvv(vm, 0, 0, k + 1);
                let bme = bm.edge_create(a, b, Some(e_eg), CreateFlag::NO_DOUBLE);
                bm.oflag_enable(bme, EDGE_OUT);
            }
        }
    }

    /// `build_vmesh`: the BMVerts of the boundary and interior of the vertex mesh, and its
    /// faces.
    pub(crate) fn build_vmesh(&mut self, bm: &mut BMesh, bv: Bev) {
        let vm = self.bvm(bv);
        let bvv = self.bev(bv).v;
        let n = self.vm(vm).count;
        let ns = self.vm(vm).seg;
        self.vm_mut(vm).alloc_mesh();

        // Special case: just two beveled edges welded together.
        let weld = self.bev(bv).selcount == 2 && n == 2;
        let mut weld1: Option<Bnd> = None;
        let mut weld2: Option<Bnd> = None;

        // Make (i, 0, 0) mesh verts for all i boundverts.
        let bounds = self.bounds(vm);
        for &bndv in &bounds {
            let i = self.bnd(bndv).index;
            let c = self.bnd(bndv).nv.co;
            self.vm_mut(vm).mv_mut(i, 0, 0).co = c;
            self.create_mesh_bmvert(bm, vm, i, 0, 0, bvv);
            self.bnd_mut(bndv).nv.v = self.vm(vm).mv(i, 0, 0).v;
            // Find boundverts and move profile planes if this is a weld case.
            if weld && self.bnd(bndv).ebev.is_some() {
                if let Some(w1) = weld1 {
                    weld2 = Some(bndv);
                    self.set_profile_params(bm, bv, w1);
                    self.set_profile_params(bm, bv, bndv);
                    self.move_weld_profile_planes(bm, bv, w1, bndv);
                } else {
                    weld1 = Some(bndv);
                }
            }
        }

        // Calculate all profiles once, the last point before the mesh verts are created.
        self.calculate_vm_profiles(bm, bv, vm);

        // Copy other ends to (i, 0, ns) for all i, and fill in profiles for edges.
        let mesh_kind = self.vm(vm).mesh_kind;
        for &bndv in &bounds {
            let i = self.bnd(bndv).index;
            let ni = self.bnd(self.bnext(bndv)).index;
            // bndv's last vert along the boundary arc is the first of the next BoundVert's.
            self.vm_mut(vm).copy_mesh_vert(i, 0, ns, ni, 0, 0);
            if mesh_kind != MeshKind::Adj {
                for k in 1..ns {
                    if self.bnd(bndv).ebev.is_some() {
                        let p = self.get_profile_point(bndv, k, ns);
                        self.vm_mut(vm).mv_mut(i, 0, k).co = p;
                        if !weld {
                            // Done later with (possibly) better positions for the weld case.
                            self.create_mesh_bmvert(bm, vm, i, 0, k, bvv);
                        }
                    } else if n == 2 {
                        // One edge beveled and this is the v without ebev: copy the verts
                        // from the other v, in reverse order.
                        self.vm_mut(vm).copy_mesh_vert(i, 0, k, 1 - i, 0, ns - k);
                    }
                }
            }
        }

        // The profile for the weld case (just a connection between the two boundverts).
        if weld {
            self.vm_mut(vm).mesh_kind = MeshKind::None;
            let (w1, w2) = (weld1.expect("weld1"), weld2.expect("weld2"));
            let (i1, i2) = (self.bnd(w1).index, self.bnd(w2).index);
            let (r1, r2) = (self.bnd(w1).profile.super_r, self.bnd(w2).profile.super_r);
            for k in 1..ns {
                let v_weld1 = self.vm(vm).mv(i1, 0, k).co;
                let v_weld2 = self.vm(vm).mv(i2, 0, ns - k).co;
                // Use the point from the other profile if one is in a special case.
                let p = if r1 == PRO_LINE_R && r2 != PRO_LINE_R {
                    v_weld2
                } else if r2 == PRO_LINE_R && r1 != PRO_LINE_R {
                    v_weld1
                } else {
                    // The profiles may not be snapped to the same plane: use the midpoint.
                    math::mid(v_weld1, v_weld2)
                };
                self.vm_mut(vm).mv_mut(i1, 0, k).co = p;
                self.create_mesh_bmvert(bm, vm, i1, 0, k, bvv);
            }
            for k in 1..ns {
                self.vm_mut(vm).copy_mesh_vert(i2, 0, ns - k, i1, 0, k);
            }
        }

        // The pipe case ADJ mesh is used for both the ADJ and CUTOFF options.
        let mut vpipe = None;
        if (n == 3 || n == 4) && self.seg > 1 {
            vpipe = self.pipe_test(bm, bv);
            if vpipe.is_some() {
                self.vm_mut(vm).mesh_kind = MeshKind::Adj;
            }
        }

        match self.vm(vm).mesh_kind {
            MeshKind::None => {
                if n == 2 && self.affect_type == Affect::Vertices {
                    self.bevel_vert_two_edges(bm, bv);
                }
            }
            MeshKind::Poly => {
                self.bevel_build_poly(bm, bv);
            }
            MeshKind::Adj => self.bevel_build_rings(bm, bv, vpipe),
            MeshKind::TriFan => self.bevel_build_trifan(bm, bv),
            MeshKind::Cutoff => self.bevel_build_cutoff(bm, bv),
        }
    }

    /// `bevvert_is_weld_cross`: a weld with four edges, the two beveled ones opposite.
    fn bevvert_is_weld_cross(&self, bv: Bev) -> bool {
        let b = self.bev(bv);
        b.edgecount == 4
            && b.selcount == 2
            && ((self.eh(b.edge(0)).is_bev && self.eh(b.edge(2)).is_bev)
                || (self.eh(b.edge(1)).is_bev && self.eh(b.edge(3)).is_bev))
    }

    /// `weld_cross_attrs_copy`: copy edge attributes across the non-beveled crossing edges of
    /// a cross weld.
    fn weld_cross_attrs_copy(&self, bm: &mut BMesh, bv: Bev, vm: Vm, vmindex: i32, e: Eh) {
        let b = self.bev(bv);
        let mut bme_prev = None;
        let mut bme_next = None;
        for i in 0..4 {
            if b.edge(i) == e {
                bme_prev = Some(self.ehe(b.edge((i + 3) % 4)));
                bme_next = Some(self.ehe(b.edge((i + 1) % 4)));
                break;
            }
        }
        let (Some(bme_prev), Some(bme_next)) = (bme_prev, bme_next) else {
            return;
        };
        // Seams and sharp edges cross only if that way on both sides.
        let (fp, fnx) = (bm.edge(bme_prev).flag, bm.edge(bme_next).flag);
        let disable_seam = fp.contains(ElemFlag::SEAM) != fnx.contains(ElemFlag::SEAM);
        let enable_smooth = fp.contains(ElemFlag::SMOOTH) != fnx.contains(ElemFlag::SMOOTH);
        let nseg = self.eh(e).seg;
        for i in 0..nseg {
            let Some(bme) = bm.edge_exists(self.mvv(vm, vmindex, 0, i), self.mvv(vm, vmindex, 0, i + 1))
            else {
                continue;
            };
            bm.edge_attrs_copy(bme_prev, bme);
            if disable_seam {
                bm.edge_mut(bme).flag.remove(ElemFlag::SEAM);
            }
            if enable_smooth {
                bm.edge_mut(bme).flag.insert(ElemFlag::SMOOTH);
            }
        }
    }

    /// `bevel_build_edge_polygons`: the bevel polygons along the selected edge `bme`.
    pub(crate) fn bevel_build_edge_polygons(&mut self, bm: &mut BMesh, bme: EdgeId) {
        let mat_nr = self.mat_nr;
        if !bm.edge_is_manifold(bme) {
            return;
        }
        let [ev1, ev2] = bm.edge(bme).v;
        let (Some(bv1), Some(bv2)) = (self.find_bevvert(ev1), self.find_bevvert(ev2)) else {
            return;
        };
        let (Some(e1), Some(e2)) = (self.find_edge_half(bv1, bme), self.find_edge_half(bv2, bme))
        else {
            return;
        };
        let nseg = self.eh(e1).seg;
        debug_assert!(nseg > 0 && nseg == self.eh(e2).seg);
        let (e1l, e1r) = (self.eh(e1).leftv.expect("leftv"), self.eh(e1).rightv.expect("rightv"));
        let (e2l, e2r) = (self.eh(e2).leftv.expect("leftv"), self.eh(e2).rightv.expect("rightv"));
        let bmv1 = self.bnd(e1l).nv.v.expect("boundary vertex");
        let bmv4 = self.bnd(e1r).nv.v.expect("boundary vertex");
        let bmv2 = self.bnd(e2r).nv.v.expect("boundary vertex");
        let bmv3 = self.bnd(e2l).nv.v.expect("boundary vertex");

        let f1 = self.eh(e1).fprev;
        let f2 = self.eh(e1).fnext;
        let faces = [f1, f1, f2, f2];
        let i1 = self.bnd(e1l).index;
        let i2 = self.bnd(e2l).index;
        let vm1 = self.bvm(bv1);
        let vm2 = self.bvm(bv2);
        let (bv1v, bv2v) = (self.bev(bv1).v, self.bev(bv2).v);

        let mut verts = [bmv1, bmv2, bmv2, bmv1];
        let mut nv_bv_map: FxHashMap<VertId, VertId> = FxHashMap::default();
        nv_bv_map.entry(verts[0]).or_insert(bv1v);
        nv_bv_map.entry(verts[1]).or_insert(bv2v);

        let odd = nseg % 2 == 1;
        let mid = nseg / 2;
        let e1_seam = self.eh(e1).is_seam;
        let mut f_choice = None;
        let mut center_adj_k = -1;
        if odd && e1_seam {
            f_choice = self.choose_rep_face(bm, &[f1, f2]);
            if nseg > 1 {
                center_adj_k = if f_choice == f1 { mid + 2 } else { mid };
            }
        }
        for k in 1..=nseg {
            verts[3] = self.mvv(vm1, i1, 0, k);
            verts[2] = self.mvv(vm2, i2, 0, nseg - k);
            nv_bv_map.entry(verts[3]).or_insert(bv1v);
            nv_bv_map.entry(verts[2]).or_insert(bv2v);
            let r_f = if odd && k == mid + 1 {
                if e1_seam {
                    // Straddles a seam: interpolate in f_choice and snap the loops whose verts
                    // are in the non-chosen face to bme.
                    let edges = if f_choice == f1 {
                        [None, None, Some(bme), Some(bme)]
                    } else {
                        [Some(bme), Some(bme), None, None]
                    };
                    self.bev_create_ngon(bm, &verts, None, f_choice, Some(&edges), None, Some(&nv_bv_map), mat_nr, true)
                } else {
                    // Straddles but not a seam: left half in f1, right half in f2.
                    self.bev_create_ngon(bm, &verts, Some(&faces), f_choice, None, None, Some(&nv_bv_map), mat_nr, true)
                }
            } else if odd && k == center_adj_k && e1_seam {
                // The strip next to the center one, in another UV island: snap the edge near
                // the seam to bme.
                let (edges, f_interp) = if k == mid {
                    ([None, None, Some(bme), Some(bme)], f1)
                } else {
                    ([Some(bme), Some(bme), None, None], f2)
                };
                self.bev_create_ngon(bm, &verts, None, f_interp, Some(&edges), None, Some(&nv_bv_map), mat_nr, true)
            } else if !odd && k == mid {
                // Left poly that touches an even center line on the right.
                let edges = [None, None, Some(bme), Some(bme)];
                self.bev_create_ngon(bm, &verts, None, f1, Some(&edges), None, Some(&nv_bv_map), mat_nr, true)
            } else if !odd && k == mid + 1 {
                // Right poly that touches an even center line on the left.
                let edges = [Some(bme), Some(bme), None, None];
                self.bev_create_ngon(bm, &verts, None, f2, Some(&edges), None, Some(&nv_bv_map), mat_nr, true)
            } else {
                // Doesn't cross or touch the center line: interpolate in f1 or f2.
                let f = if k <= mid { f1 } else { f2 };
                self.bev_create_ngon(bm, &verts, None, f, None, None, Some(&nv_bv_map), mat_nr, true)
            };
            self.record_face_kind(r_f, FKind::Edge);
            // Tag the long edges: those out of verts[0] and verts[2].
            if let Some(r_f) = r_f {
                for l in bm.face_loops(r_f) {
                    let lv = bm.lp(l).v;
                    if lv == verts[0] || lv == verts[2] {
                        bm.lp_mut(l).flag.insert(LONG_TAG);
                    }
                }
            }
            verts[0] = verts[3];
            verts[1] = verts[2];
        }

        // Copy edge data to the first and last edge.
        if let Some(bme1) = bm.edge_exists(bmv1, bmv2)
            && bme1 != bme
        {
            bm.edge_attrs_copy(bme, bme1);
        }
        if let Some(bme2) = bm.edge_exists(bmv3, bmv4)
            && bme2 != bme
        {
            bm.edge_attrs_copy(bme, bme2);
        }
        // A "weld cross" end wants continuity of edge attributes across its end edge(s).
        if self.bevvert_is_weld_cross(bv1) {
            self.weld_cross_attrs_copy(bm, bv1, vm1, i1, e1);
        }
        if self.bevvert_is_weld_cross(bv2) {
            self.weld_cross_attrs_copy(bm, bv2, vm2, i2, e2);
        }
    }

    /// `bev_rebuild_polygon`: rebuild face `f`, which has at least one beveled vertex.
    /// Returns whether it was rebuilt.
    fn bev_rebuild_polygon(&mut self, bm: &mut BMesh, f: FaceId) -> bool {
        let mut do_rebuild = false;
        let mut vv: Vec<VertId> = Vec::new();
        let mut ee: Vec<EdgeId> = Vec::new();
        let mut nv_bv_map: FxHashMap<VertId, VertId> = FxHashMap::default();
        for l in bm.face_loops(f) {
            let (lv, le) = (bm.lp(l).v, bm.lp(l).e);
            if !bm.vert(lv).flag.contains(ElemFlag::TAG) {
                vv.push(lv);
                ee.push(le);
                // Keep the old vertex, i.e. map it to itself.
                nv_bv_map.entry(lv).or_insert(lv);
                continue;
            }
            let lprev = bm.lp(l).prev;
            let bv = self.find_bevvert(lv).expect("beveled vertex");
            let vm = self.bvm(bv);
            let e = self.find_edge_half(bv, le).expect("edge half");
            let bme = self.ehe(e);
            let eprev = self.find_edge_half(bv, bm.lp(lprev).e).expect("edge half");

            // Which direction around our vertex do we travel to match the orientation of f?
            let go_ccw = if self.eprev(e) == eprev {
                if self.eprev(eprev) == e {
                    // Valence 2 vertex: use whether f is e->fnext to break the tie.
                    self.eh(e).fnext != Some(f)
                } else {
                    true // Going CCW around bv to trace this corner.
                }
            } else if self.eprev(eprev) == e {
                false // Going CW around bv to trace this corner.
            } else {
                // Edges in face are non-contiguous in our ordering around bv.
                self.count_ccw_edges_between(eprev, e) < self.count_ccw_edges_between(e, eprev)
            };
            let mut on_profile_start = false;
            let (mut vstart, vend);
            if go_ccw {
                vstart = self.eh(eprev).rightv.expect("rightv");
                vend = self.eh(e).leftv.expect("leftv");
                if self.eh(e).profile_index > 0 {
                    vstart = self.bprev(vstart);
                    on_profile_start = true;
                }
            } else {
                vstart = self.eh(eprev).leftv.expect("leftv");
                vend = self.eh(e).rightv.expect("rightv");
                if self.eh(eprev).profile_index > 0 {
                    vstart = self.bnext(vstart);
                    on_profile_start = true;
                }
            }
            let mut v = vstart;
            if !on_profile_start {
                let nvv = self.bnd(v).nv.v.expect("boundary vertex");
                vv.push(nvv);
                ee.push(bme);
                nv_bv_map.entry(nvv).or_insert(lv);
            }
            let seg = self.vm(vm).seg;
            while v != vend {
                if go_ccw {
                    let i = self.bnd(v).index;
                    let kstart = if on_profile_start {
                        on_profile_start = false;
                        self.eh(e).profile_index
                    } else {
                        1
                    };
                    let kend = if self.eh(eprev).rightv == Some(v) && self.eh(eprev).profile_index > 0
                    {
                        self.eh(eprev).profile_index
                    } else {
                        seg
                    };
                    for k in kstart..=kend {
                        if let Some(bmv) = self.vm(vm).mv(i, 0, k).v {
                            vv.push(bmv);
                            ee.push(bme);
                            nv_bv_map.entry(bmv).or_insert(lv);
                        }
                    }
                    v = self.bnext(v);
                } else {
                    // Going CW.
                    let i = self.bnd(self.bprev(v)).index;
                    let kstart = if on_profile_start {
                        on_profile_start = false;
                        self.eh(eprev).profile_index
                    } else {
                        seg - 1
                    };
                    let kend = if self.eh(e).rightv == Some(self.bprev(v)) && self.eh(e).profile_index > 0
                    {
                        self.eh(e).profile_index
                    } else {
                        0
                    };
                    let mut k = kstart;
                    while k >= kend {
                        if let Some(bmv) = self.vm(vm).mv(i, 0, k).v {
                            vv.push(bmv);
                            ee.push(bme);
                            nv_bv_map.entry(bmv).or_insert(lv);
                        }
                        k -= 1;
                    }
                    v = self.bprev(v);
                }
            }
            do_rebuild = true;
        }
        if !do_rebuild {
            return false;
        }
        let n = vv.len();
        let f_new = self.bev_create_ngon(bm, &vv, None, Some(f), None, None, Some(&nv_bv_map), -1, true);

        // Copy attributes from old edges.
        let mut bme_prev = ee[n - 1];
        for k in 0..n {
            let Some(bme_new) = bm.edge_exists(vv[k], vv[(k + 1) % n]) else {
                continue;
            };
            if ee[k] != bme_new {
                bm.edge_attrs_copy(ee[k], bme_new);
                // Undo seam and smooth for corner segments if those attrs aren't contiguous
                // around the face.
                if k < n - 1 && ee[k] == ee[k + 1] {
                    let (fk, fp) = (bm.edge(ee[k]).flag, bm.edge(bme_prev).flag);
                    if fk.contains(ElemFlag::SEAM) && !fp.contains(ElemFlag::SEAM) {
                        bm.edge_mut(bme_new).flag.remove(ElemFlag::SEAM);
                    }
                    // "sharp" should be contiguous, so reverse the test.
                    if !fk.contains(ElemFlag::SMOOTH) && fp.contains(ElemFlag::SMOOTH) {
                        bm.edge_mut(bme_new).flag.insert(ElemFlag::SMOOTH);
                    }
                } else {
                    bme_prev = ee[k];
                }
            }
        }

        // Don't select newly created boundary faces, nor return them.
        if let Some(f_new) = f_new {
            self.record_face_kind(Some(f_new), FKind::Recon);
            bm.face_mut(f_new).flag.remove(ElemFlag::TAG);
            // Also don't want new edges that aren't part of a new bevel face.
            for bme in bm.face_edges(f_new) {
                let keep = bm
                    .edge_faces(bme)
                    .into_iter()
                    .any(|fo| bm.face(fo).flag.contains(ElemFlag::TAG));
                if !keep {
                    bm.oflag_disable(bme, EDGE_OUT);
                }
            }
        }
        true
    }

    /// `bevel_rebuild_existing_polygons`: rebuild every face touching `v` (the original faces
    /// are deleted later; `rebuilt` records them).
    pub(crate) fn bevel_rebuild_existing_polygons(
        &mut self,
        bm: &mut BMesh,
        v: VertId,
        rebuilt: &mut Vec<FaceId>,
        rebuilt_set: &mut rustc_hash::FxHashSet<FaceId>,
    ) {
        for f in bm.vert_faces(v) {
            if !rebuilt_set.contains(&f) && self.bev_rebuild_polygon(bm, f) {
                rebuilt_set.insert(f);
                rebuilt.push(f);
            }
        }
    }

    /// `bevel_reattach_wires`: reattach wire edges to the closest new vertex.
    pub(crate) fn bevel_reattach_wires(&self, bm: &mut BMesh, v: VertId) {
        let Some(bv) = self.find_bevvert(v) else {
            return;
        };
        let b = self.bev(bv);
        if b.wirecount == 0 || b.vmesh.0 as usize >= self.vms.len() {
            return;
        }
        let vm = b.vmesh;
        if self.vm(vm).boundstart.is_none() {
            return;
        }
        for &e in &b.wire_edges {
            // The new vertex closest to the other end of e.
            let mut vclosest: Option<VertId> = None;
            let mut dclosest = f32::MAX;
            let mut votherclosest: Option<VertId> = None;
            let vother = bm.edge_other_vert(e, v);
            let mut bvother = None;
            if bm.vert(vother).flag.contains(ElemFlag::TAG) {
                bvother = self.find_bevvert(vother);
                if bvother.is_none_or(|bo| self.vm(self.bvm(bo)).boundstart.is_none()) {
                    return; // Shouldn't happen.
                }
            }
            for bndv in self.bounds(vm) {
                let bco = self.bnd(bndv).nv.co;
                if let Some(bo) = bvother {
                    for bndvother in self.bounds(self.bvm(bo)) {
                        let d = math::len_sq_v3v3(self.bnd(bndvother).nv.co, bco);
                        if d < dclosest {
                            vclosest = self.bnd(bndv).nv.v;
                            votherclosest = self.bnd(bndvother).nv.v;
                            dclosest = d;
                        }
                    }
                } else {
                    let d = math::len_sq_v3v3(co(bm, vother), bco);
                    if d < dclosest {
                        vclosest = self.bnd(bndv).nv.v;
                        votherclosest = Some(vother);
                        dclosest = d;
                    }
                }
            }
            if let (Some(a), Some(b)) = (vclosest, votherclosest) {
                bm.edge_create(a, b, Some(e), CreateFlag::NO_DOUBLE);
            }
        }
    }

    /// `bevel_extend_edge_data_ex`: extend seam (or sharp) marks around the edges added at
    /// `bv`.
    fn bevel_extend_edge_data_ex(&self, bm: &mut BMesh, bv: Bev, seam: bool) {
        let vm = self.bvm(bv);
        let count = self.vm(vm).count;
        let seg = self.vm(vm).seg;
        let boundstart = self.bstart(vm);
        let mut bcur = boundstart;
        let mut start = bcur;
        let set = |bm: &mut BMesh, a: VertId, b: VertId| {
            if let Some(e) = bm.edge_exists(a, b) {
                if seam {
                    bm.edge_mut(e).flag.insert(ElemFlag::SEAM);
                } else {
                    bm.edge_mut(e).flag.remove(ElemFlag::SMOOTH);
                }
            }
        };
        loop {
            let extend_len = if seam {
                self.bnd(bcur).seam_len
            } else {
                self.bnd(bcur).sharp_len
            };
            if extend_len != 0 {
                if self.bnd(boundstart).seam_len == 0 && start == boundstart {
                    start = bcur; // The first boundvert with seam_len > 0.
                }
                let idx = self.bnd(bcur).index;
                for i in idx..idx + extend_len {
                    let mut v1 = self.mvv(vm, i % count, 0, 0);
                    for k in 1..seg {
                        let v2 = self.mvv(vm, i % count, 0, k);
                        set(bm, v1, v2);
                        v1 = v2;
                    }
                    let v3 = self.mvv(vm, (i + 1) % count, 0, 0);
                    set(bm, v1, v3);
                    bcur = self.bnext(bcur);
                }
            } else {
                bcur = self.bnext(bcur);
            }
            if bcur == start {
                break;
            }
        }
    }

    /// `bevel_extend_edge_data`.
    pub(crate) fn bevel_extend_edge_data(&self, bm: &mut BMesh, bv: Bev) {
        let vm = self.bvm(bv);
        if self.vm(vm).mesh_kind == MeshKind::TriFan || self.bev(bv).selcount < 2 {
            return;
        }
        self.bevel_extend_edge_data_ex(bm, bv, true);
        self.bevel_extend_edge_data_ex(bm, bv, false);
    }

    /// `bevel_set_weighted_normal_face_strength`.
    pub(crate) fn bevel_set_weighted_normal_face_strength(&self, bm: &mut BMesh) {
        let mode = self.face_strength_mode;
        let li = match bm.pdata.find(FACEWEIGHT_LAYER) {
            Some(i) if matches!(bm.pdata.layers[i].data, AttrData::Int(_)) => i,
            _ => {
                let n = bm.faces.capacity();
                bm.pdata.add_layer(CdLayer {
                    name: FACEWEIGHT_LAYER.into(),
                    data: AttrData::Int(Arc::new(vec![0; n])),
                })
            }
        };
        let faces = bm.face_ids();
        let kinds: Vec<FKind> = faces.iter().map(|&f| self.get_face_kind(f)).collect();
        let AttrData::Int(col) = &mut bm.pdata.layers[li].data else {
            return;
        };
        let col = Arc::make_mut(col);
        for (f, kind) in faces.into_iter().zip(kinds) {
            let (strength, do_set) = match kind {
                FKind::Vert => (FACE_STRENGTH_WEAK, mode >= FaceStrength::New),
                FKind::Edge => (FACE_STRENGTH_MEDIUM, mode >= FaceStrength::New),
                FKind::Recon => (FACE_STRENGTH_STRONG, mode >= FaceStrength::Affected),
                FKind::Orig => (FACE_STRENGTH_STRONG, mode == FaceStrength::All),
            };
            if do_set {
                if col.len() <= f.slot() {
                    col.resize(f.slot() + 1, 0);
                }
                col[f.slot()] = strength;
            }
        }
    }

    /// Clears `BM_ELEM_LONG_TAG` on the loops of the edge faces.
    pub(crate) fn clear_long_tags(&self, bm: &mut BMesh) {
        for f in bm.face_ids() {
            if self.get_face_kind(f) != FKind::Edge {
                continue;
            }
            for l in bm.face_loops(f) {
                bm.lp_mut(l).flag.remove(LONG_TAG);
            }
        }
    }
}

/// `snap_edge_for_center_vmesh_vert`.
fn snap_edge_for_center_vmesh_vert(
    i: i32,
    n_bndv: i32,
    eprev: Option<EdgeId>,
    enext: Option<EdgeId>,
    bndv_rep_faces: &[Option<FaceId>],
    center_frep: Option<FaceId>,
    frep_beats_next: &[bool],
) -> Option<EdgeId> {
    let previ = ((i + n_bndv - 1) % n_bndv) as usize;
    let nexti = ((i + 1) % n_bndv) as usize;
    if frep_beats_next[previ] && bndv_rep_faces[previ] == center_frep {
        return eprev;
    }
    if !frep_beats_next[i as usize] && bndv_rep_faces[nexti] == center_frep {
        return enext;
    }
    None
}

/// `snap_edges_for_vmesh_vert`: the edges to snap to (or none) for the corners of the ADJ
/// face with lower left corner at `(i, j, k)` (odd segment counts only).
#[allow(clippy::too_many_arguments)]
fn snap_edges_for_vmesh_vert(
    i: i32,
    j: i32,
    k: i32,
    ns: i32,
    ns2: i32,
    n_bndv: i32,
    eprev: Option<EdgeId>,
    enext: Option<EdgeId>,
    enextnext: Option<EdgeId>,
    bndv_rep_faces: &[Option<FaceId>],
    center_frep: Option<FaceId>,
    frep_beats_next: &[bool],
) -> [Option<EdgeId>; 4] {
    let mut r = [None; 4];
    for (corner, out) in r.iter_mut().enumerate() {
        if ns % 2 == 0 {
            continue;
        }
        let previ = ((i + n_bndv - 1) % n_bndv) as usize;
        let jj = if corner < 2 { j } else { j + 1 };
        let kk = if corner == 0 || corner == 3 { k } else { k + 1 };
        if jj < ns2 && kk < ns2 {
            // No snap.
        } else if jj < ns2 && kk == ns2 {
            // On the left side of the center strip quads, but not on the center poly.
            if !frep_beats_next[i as usize] {
                *out = enext;
            }
        } else if jj < ns2 && kk == ns2 + 1 {
            // On the right side of the center strip quads, but not on the center poly.
            if frep_beats_next[i as usize] {
                *out = enext;
            }
        } else if jj == ns2 && kk < ns2 {
            // On the top of the top strip quads, but not on the center poly.
            if frep_beats_next[previ] {
                *out = eprev;
            }
        } else if jj == ns2 && kk == ns2 {
            // Center poly vert for boundvert i.
            *out = snap_edge_for_center_vmesh_vert(
                i,
                n_bndv,
                eprev,
                enext,
                bndv_rep_faces,
                center_frep,
                frep_beats_next,
            );
        } else if jj == ns2 && kk == ns2 + 1 {
            // Center poly vert for boundvert i+1.
            let nexti = (i + 1) % n_bndv;
            *out = snap_edge_for_center_vmesh_vert(
                nexti,
                n_bndv,
                enext,
                enextnext,
                bndv_rep_faces,
                center_frep,
                frep_beats_next,
            );
        }
    }
    r
}
