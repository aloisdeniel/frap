// SPDX-License-Identifier: GPL-3.0-or-later
//! Boundary construction: offset meeting points, `build_boundary` and its special cases,
//! miters, seams, and the width-adjustment pass (`adjust_offsets`). Ported from
//! blender/source/blender/bmesh/tools/bmesh_bevel.cc (Blender 5.2.4, GPL-2.0-or-later).

use super::math::{self, V3};
use super::types::{
    Affect, AngleKind, BEVEL_EPSILON_BIG, BEVEL_GOOD_ANGLE, BEVEL_MATCH_SPEC_WEIGHT, Bev, Bevel,
    Bnd, Eh, MeshKind, Miter, OffsetType, VmeshMethod, bevel_epsilon_ang, bevel_epsilon_ang_dot,
    bevel_small_ang,
};
use super::util::{co, contig_ldata_around_vert, edge_calc_length, fno};
use crate::types::{BMesh, ElemFlag, FaceId, VertId};

/// `HASNOT_SEAMSHARP` selector.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum SeamSharp {
    Seam,
    Sharp,
}

impl Bevel {
    /// `BM_edge_other_vert(e->e, v)`.
    #[inline]
    pub(crate) fn other_vert(&self, bm: &BMesh, e: Eh, v: VertId) -> VertId {
        bm.edge_other_vert(self.ehe(e), v)
    }

    /// `slide_dist`: the point a distance `d` from `v` along `e->e`.
    pub(crate) fn slide_dist(&self, bm: &BMesh, e: Eh, v: VertId, mut d: f32) -> V3 {
        let (dir, len) =
            math::normalize_v3(math::sub(co(bm, v), co(bm, self.other_vert(bm, e, v))));
        if d > len {
            d = len - (50.0 * super::types::BEVEL_EPSILON_D) as f32;
        }
        math::madd(co(bm, v), dir, -d)
    }

    /// `is_outside_edge`: is `p` not on the edge `e`? Returns the closer end if so.
    fn is_outside_edge(&self, bm: &BMesh, e: Eh, p: V3) -> Option<VertId> {
        let [v1, v2] = bm.edge(self.ehe(e)).v;
        let l1 = co(bm, v1);
        let u = math::sub(co(bm, v2), l1);
        let h = math::sub(p, l1);
        let (u, lenu) = math::normalize_v3(u);
        let lambda = math::dot(u, h);
        if lambda <= -BEVEL_EPSILON_BIG * lenu {
            return Some(v1);
        }
        if lambda >= (1.0 + BEVEL_EPSILON_BIG) * lenu {
            return Some(v2);
        }
        None
    }

    /// `edges_angle_kind`: is the angle from `e1` to `e2` at `v` smaller than, equal to or
    /// larger than 180 degrees?
    pub(crate) fn edges_angle_kind(&self, bm: &BMesh, e1: Eh, e2: Eh, v: VertId) -> AngleKind {
        let v1 = self.other_vert(bm, e1, v);
        let v2 = self.other_vert(bm, e2, v);
        let dir1 = math::norm(math::sub(co(bm, v), co(bm, v1)));
        let dir2 = math::norm(math::sub(co(bm, v), co(bm, v2)));
        // First check for in-line edges using a simpler test.
        if nearly_parallel_normalized(dir1, dir2) {
            return AngleKind::Straight;
        }
        let cross = math::norm(math::cross(dir1, dir2));
        let no = if let Some(f) = self.eh(e1).fnext {
            fno(bm, f)
        } else if let Some(f) = self.eh(e2).fprev {
            fno(bm, f)
        } else {
            bm.vert(v).no
        };
        if math::dot(cross, no) < 0.0 {
            return AngleKind::Larger;
        }
        AngleKind::Smaller
    }

    /// `point_between_edges`: is `p` (on the plane of `e1` and `e2`) between them, sweeping
    /// CCW?
    fn point_between_edges(&self, bm: &BMesh, p: V3, v: VertId, f: FaceId, e1: Eh, e2: Eh) -> bool {
        let v1 = self.other_vert(bm, e1, v);
        let v2 = self.other_vert(bm, e2, v);
        let dir1 = math::norm(math::sub(co(bm, v), co(bm, v1)));
        let dir2 = math::norm(math::sub(co(bm, v), co(bm, v2)));
        let dirco = math::norm(math::sub(co(bm, v), p));
        let mut ang11 = math::angle_normalized_v3v3(dir1, dir2);
        let mut ang1co = math::angle_normalized_v3v3(dir1, dirco);
        let no = math::cross(dir1, dir2);
        if math::dot(no, fno(bm, f)) < 0.0 {
            ang11 = (std::f64::consts::PI * 2.0) as f32 - ang11;
        }
        let no = math::cross(dir1, dirco);
        if math::dot(no, fno(bm, f)) < 0.0 {
            ang1co = (std::f64::consts::PI * 2.0) as f32 - ang1co;
        }
        ang11 - ang1co > -bevel_epsilon_ang()
    }

    /// `offset_meet_lines_percent_or_absolute`: the lines whose intersection defines the
    /// boundary point between `e1` and `e2` for the PERCENT and ABSOLUTE offset types.
    fn offset_meet_lines_percent_or_absolute(
        &self,
        bm: &BMesh,
        e1: Eh,
        e2: Eh,
        v: VertId,
    ) -> [V3; 4] {
        let v1 = self.other_vert(bm, e1, v);
        let v2 = self.other_vert(bm, e2, v);
        let f1 = self.eh(e1).fnext;
        let f2 = self.eh(e2).fprev;
        if let (Some(f1), Some(f2)) = (f1, f2) {
            let l = bm.face_vert_share_loop(f1, v1).expect("loop of v1");
            let e0 = bm.lp(l).e;
            let l = bm.face_vert_share_loop(f2, v2).expect("loop of v2");
            let e3 = bm.lp(bm.lp(l).prev).e;
            let l = bm.face_vert_share_loop(f1, v).expect("loop of v");
            let e4 = bm.lp(bm.lp(l).prev).e;
            let l = bm.face_vert_share_loop(f2, v).expect("loop of v");
            let e5 = bm.lp(l).e;
            let e1e = self.ehe(e1);
            let e2e = self.ehe(e2);
            // All the legs must be visible from their opposite legs.
            let no_offsets = !edge_edge_angle_less_than_180(bm, e0, e1e, f1)
                || !edge_edge_angle_less_than_180(bm, e1e, e4, f1)
                || !edge_edge_angle_less_than_180(bm, e2e, e3, f2)
                || !edge_edge_angle_less_than_180(bm, e5, e2e, f1);
            if !no_offsets {
                let (d0, d3, d4, d5) = if self.offset_type == OffsetType::Absolute {
                    (self.offset, self.offset, self.offset, self.offset)
                } else {
                    (
                        self.offset * edge_calc_length(bm, e0) / 100.0,
                        self.offset * edge_calc_length(bm, e3) / 100.0,
                        self.offset * edge_calc_length(bm, e4) / 100.0,
                        self.offset * edge_calc_length(bm, e5) / 100.0,
                    )
                };
                let (e1_wt, e2_wt) = if self.use_weights {
                    let w = |e| {
                        self.bweight_edge.map_or(0.0, |li| {
                            super::construct::float_at(&bm.edata, li, crate::EdgeId::slot(e))
                        })
                    };
                    (w(e1e), w(e2e))
                } else {
                    (1.0, 1.0)
                };
                return [
                    slide_dist_edge(bm, e4, v, d4 * e1_wt),
                    slide_dist_edge(bm, e0, v1, d0 * e1_wt),
                    slide_dist_edge(bm, e5, v, d5 * e2_wt),
                    slide_dist_edge(bm, e3, v2, d3 * e2_wt),
                ];
            }
        }
        [co(bm, v), co(bm, v1), co(bm, v), co(bm, v2)]
    }

    /// `offset_meet`: the meeting point of the offset edges of `e1` and `e2` (which share `v`
    /// and maybe face `f`; `e1` precedes `e2` CCW).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn offset_meet(
        &self,
        bm: &BMesh,
        e1: Eh,
        e2: Eh,
        v: VertId,
        f: Option<FaceId>,
        edges_between: bool,
        e_in_plane: Option<Eh>,
    ) -> V3 {
        let vco = co(bm, v);
        let mut dir1 = math::sub(vco, co(bm, self.other_vert(bm, e1, v)));
        let dir2 = math::sub(co(bm, self.other_vert(bm, e2, v)), vco);
        let (dir1n, dir2p) = if edges_between {
            let e1next = self.enext(e1);
            let e2prev = self.eprev(e2);
            (
                math::sub(co(bm, self.other_vert(bm, e1next, v)), vco),
                math::sub(vco, co(bm, self.other_vert(bm, e2prev, v))),
            )
        } else {
            ([0.0; 3], [0.0; 3])
        };
        let mut ang = math::angle_v3v3(dir1, dir2);
        let mut meetco;
        if ang < bevel_epsilon_ang() {
            // Special case: e1 and e2 are parallel; put the offset point perpendicular to both.
            let norm_v = if let Some(f) = f {
                fno(bm, f)
            } else {
                let mut fcount = 0;
                let mut n = [0.0f32; 3];
                let mut eloop = e1;
                while eloop != e2 {
                    if let Some(fnx) = self.eh(eloop).fnext {
                        n = math::add(n, fno(bm, fnx));
                        fcount += 1;
                    }
                    eloop = self.enext(eloop);
                }
                if fcount == 0 {
                    bm.vert(v).no
                } else {
                    math::scale(n, 1.0 / fcount as f32)
                }
            };
            dir1 = math::add(dir1, dir2);
            let norm_perp1 = math::norm(math::cross(dir1, norm_v));
            let mut d = self.eh(e1).offset_r.max(self.eh(e2).offset_l);
            d /= libm::cosf(ang / 2.0);
            meetco = math::madd(vco, norm_perp1, d);
        } else if (ang - math::PI_F).abs() < bevel_epsilon_ang() {
            // Special case: e1 and e2 are anti-parallel, so the bevel is into a zero-area face.
            let d = self.eh(e1).offset_r.max(self.eh(e2).offset_l);
            meetco = self.slide_dist(bm, e2, v, d);
        } else {
            // Get the normal to the plane where the meet point should be.
            let mut f = f;
            let (norm_v1, norm_v2);
            if let Some(ff) = f.filter(|_| ang < bevel_small_ang()) {
                let n = fno(bm, ff);
                norm_v1 = n;
                norm_v2 = n;
            } else if !edges_between {
                let mut n = math::norm(math::cross(dir2, dir1));
                let refn = f.map_or(bm.vert(v).no, |f| fno(bm, f));
                if math::dot(n, refn) < 0.0 {
                    n = math::neg(n);
                }
                norm_v1 = n;
                norm_v2 = n;
            } else {
                // Separate faces; get face normals at corners for each separately.
                let mut n1 = math::norm(math::cross(dir1n, dir1));
                f = self.eh(e1).fnext;
                if math::dot(n1, f.map_or(bm.vert(v).no, |f| fno(bm, f))) < 0.0 {
                    n1 = math::neg(n1);
                }
                let mut n2 = math::norm(math::cross(dir2, dir2p));
                f = self.eh(e2).fprev;
                if math::dot(n2, f.map_or(bm.vert(v).no, |f| fno(bm, f))) < 0.0 {
                    n2 = math::neg(n2);
                }
                norm_v1 = n1;
                norm_v2 = n2;
            }
            // Vectors perpendicular to each edge, perpendicular to norm_v, into the face.
            let norm_perp1 = math::norm(math::cross(dir1, norm_v1));
            let norm_perp2 = math::norm(math::cross(dir2, norm_v2));
            let (off1a, off1b, off2a, off2b) =
                if matches!(self.offset_type, OffsetType::Percent | OffsetType::Absolute) {
                    let l = self.offset_meet_lines_percent_or_absolute(bm, e1, e2, v);
                    (l[0], l[1], l[2], l[3])
                } else {
                    let off1a = math::madd(vco, norm_perp1, self.eh(e1).offset_r);
                    let off1b = math::add(off1a, dir1);
                    let off2a = math::madd(vco, norm_perp2, self.eh(e2).offset_l);
                    let off2b = math::add(off2a, dir2);
                    (off1a, off1b, off2a, off2b)
                };
            // Intersect the offset lines.
            let (isect_kind, i1, isect2) = math::isect_line_line_v3(off1a, off1b, off2a, off2b);
            meetco = i1;
            if isect_kind == 0 {
                // Lines are collinear: we already tested for this with a different epsilon.
                meetco = off1a;
            } else {
                // The lines intersect, but is it at a reasonable place?
                if self.eh(e1).offset_r == 0.0
                    && let Some(closer_v) = self.is_outside_edge(bm, e1, meetco)
                {
                    meetco = co(bm, closer_v);
                }
                if self.eh(e2).offset_l == 0.0
                    && let Some(closer_v) = self.is_outside_edge(bm, e2, meetco)
                {
                    meetco = co(bm, closer_v);
                }
                if edges_between && self.eh(e1).offset_r > 0.0 && self.eh(e2).offset_l > 0.0 {
                    // Try to drop meetco to a face between e1 and e2.
                    if isect_kind == 2 {
                        // Lines didn't meet in 3d: get average of meetco and isect2.
                        meetco = math::mid(meetco, isect2);
                    }
                    let mut e = e1;
                    while e != e2 {
                        let enx = self.enext(e);
                        let Some(fnext) = self.eh(e).fnext else {
                            e = enx;
                            continue;
                        };
                        let plane = math::plane_from_point_normal_v3(vco, fno(bm, fnext));
                        let dropco = math::closest_to_plane_normalized_v3(plane, meetco);
                        // Don't drop to the faces next to the in-plane edge.
                        if let Some(eip) = e_in_plane {
                            let eip_f = self.eh(eip).fnext.expect("in-plane edge face");
                            ang = math::angle_v3v3(fno(bm, fnext), fno(bm, eip_f));
                            if ang.abs() < bevel_small_ang()
                                || (ang - math::PI_F).abs() < bevel_small_ang()
                            {
                                e = enx;
                                continue;
                            }
                        }
                        if self.point_between_edges(bm, dropco, v, fnext, e, enx) {
                            meetco = dropco;
                            break;
                        }
                        e = enx;
                    }
                }
            }
        }
        meetco
    }

    /// `offset_meet_edge`: the meeting point between `e1` and `e2` (one of which has zero
    /// offsets) and the angle between them; `None` when there is no meeting point.
    fn offset_meet_edge(&self, bm: &BMesh, e1: Eh, e2: Eh, v: VertId) -> (Option<V3>, f32) {
        let vco = co(bm, v);
        let dir1 = math::norm(math::sub(co(bm, self.other_vert(bm, e1, v)), vco));
        let dir2 = math::norm(math::sub(co(bm, self.other_vert(bm, e2, v)), vco));
        // Find the angle from dir1 to dir2 as viewed from the vertex normal side.
        let mut ang = math::angle_normalized_v3v3(dir1, dir2);
        if ang.abs() < BEVEL_GOOD_ANGLE {
            return (None, 0.0);
        }
        let fno_ = math::cross(dir1, dir2);
        if math::dot(fno_, bm.vert(v).no) < 0.0 {
            ang = 2.0 * math::PI_F - ang; // Angle is reflex.
            return (None, ang);
        }
        if (ang - math::PI_F).abs() < BEVEL_GOOD_ANGLE {
            return (None, ang);
        }
        let sinang = libm::sinf(ang);
        let meetco = if self.eh(e1).offset_r == 0.0 {
            math::madd(vco, dir1, self.eh(e2).offset_l / sinang)
        } else {
            math::madd(vco, dir2, self.eh(e1).offset_r / sinang)
        };
        (Some(meetco), ang)
    }

    /// `good_offset_on_edge_between`: neither side sees a reflex angle.
    fn good_offset_on_edge_between(&self, bm: &BMesh, e1: Eh, e2: Eh, emid: Eh, v: VertId) -> bool {
        self.offset_meet_edge(bm, e1, emid, v).0.is_some()
            && self.offset_meet_edge(bm, emid, e2, v).0.is_some()
    }

    /// `offset_on_edge_between`: the meeting point for `e1` and `e2` on the in-between edge
    /// `emid`. Returns the point and, when it is a compromise between two meeting points, the
    /// ratio of the sines of the angles.
    fn offset_on_edge_between(
        &self,
        bm: &BMesh,
        e1: Eh,
        e2: Eh,
        emid: Eh,
        v: VertId,
    ) -> (V3, Option<f32>) {
        debug_assert!(self.eh(e1).is_bev && self.eh(e2).is_bev && !self.eh(emid).is_bev);
        let (meet1, ang1) = self.offset_meet_edge(bm, e1, emid, v);
        let (meet2, ang2) = self.offset_meet_edge(bm, emid, e2, v);
        if matches!(self.offset_type, OffsetType::Percent | OffsetType::Absolute) {
            let v2 = self.other_vert(bm, emid, v);
            let meetco = if self.offset_type == OffsetType::Percent {
                let mut wt = 1.0f32;
                if self.use_weights {
                    wt = self.bweight_edge.map_or(0.0, |li| {
                        0.5 * (super::construct::float_at(&bm.edata, li, self.ehe(e1).slot())
                            + super::construct::float_at(&bm.edata, li, self.ehe(e2).slot()))
                    });
                }
                math::interp(co(bm, v), co(bm, v2), wt * self.offset / 100.0)
            } else {
                let dir = math::norm(math::sub(co(bm, v2), co(bm, v)));
                math::madd(co(bm, v), dir, self.offset)
            };
            let sinratio = if ang1 == 0.0 {
                1.0
            } else {
                libm::sinf(ang2) / libm::sinf(ang1)
            };
            return (meetco, Some(sinratio));
        }
        match (meet1, meet2) {
            (Some(m1), Some(m2)) => {
                // ang1 should not be 0, but be paranoid.
                let sinratio = if ang1 == 0.0 {
                    1.0
                } else {
                    libm::sinf(ang2) / libm::sinf(ang1)
                };
                (math::mid(m1, m2), Some(sinratio))
            }
            (Some(m1), None) => (m1, None),
            (None, Some(m2)) => (m2, None),
            // Neither offset line met emid: all three lines are on top of each other.
            (None, None) => (self.slide_dist(bm, emid, v, self.eh(e1).offset_r), None),
        }
    }

    /// `offset_in_plane`: offset by `e->offset` in the plane with normal `plane_no` (an
    /// arbitrary plane when `None`), on the left if `left`.
    fn offset_in_plane(&self, bm: &BMesh, e: Eh, plane_no: Option<V3>, left: bool) -> V3 {
        let eh = self.eh(e);
        let [v1, v2] = bm.edge(self.ehe(e)).v;
        let v = if eh.is_rev { v2 } else { v1 };
        let dir = math::norm(math::sub(co(bm, self.other_vert(bm, e, v)), co(bm, v)));
        let no = match plane_no {
            Some(n) => n,
            None => {
                let mut n = [0.0f32; 3];
                if dir[0].abs() < dir[1].abs() {
                    n[0] = 1.0;
                } else {
                    n[1] = 1.0;
                }
                n
            }
        };
        let fdir = if left {
            math::cross(dir, no)
        } else {
            math::cross(no, dir)
        };
        let fdir = math::norm(fdir);
        math::madd(
            co(bm, v),
            fdir,
            if left { eh.offset_l } else { eh.offset_r },
        )
    }

    /// `adjust_bound_vert`.
    #[inline]
    pub(crate) fn adjust_bound_vert(&mut self, b: Bnd, p: V3) {
        self.bnd_mut(b).nv.co = p;
    }

    /// `check_edge_data_seam_sharp_edges`.
    fn check_edge_data_seam_sharp_edges(&mut self, bm: &BMesh, bv: Bev, flag: SeamSharp) {
        let hasnot = |s: &Self, e: Eh| {
            let fl = bm.edge(s.ehe(e)).flag;
            match flag {
                SeamSharp::Seam => !fl.contains(ElemFlag::SEAM),
                SeamSharp::Sharp => fl.contains(ElemFlag::SMOOTH),
            }
        };
        let mut efirst = self.bev(bv).edge(0);
        let mut e = efirst;
        // Get to the first edge with seam or sharp edge data.
        while hasnot(self, e) {
            e = self.enext(e);
            if e == efirst {
                break;
            }
        }
        // If no such edge is found, return.
        if hasnot(self, e) {
            return;
        }
        efirst = e;
        loop {
            let mut flag_count = 0;
            let mut ne = self.enext(e);
            while hasnot(self, ne) && ne != efirst {
                if self.eh(ne).is_bev {
                    flag_count += 1;
                }
                ne = self.enext(ne);
            }
            if ne == e || (ne == efirst && hasnot(self, efirst)) {
                break;
            }
            let rv = self.eh(e).rightv.expect("right boundvert");
            match flag {
                SeamSharp::Seam => self.bnd_mut(rv).seam_len = flag_count,
                SeamSharp::Sharp => self.bnd_mut(rv).sharp_len = flag_count,
            }
            e = ne;
            if e == efirst {
                break;
            }
        }
    }

    /// `set_bound_vert_seams`: the `any_seam` property of a BevVert and its BoundVerts.
    pub(crate) fn set_bound_vert_seams(&mut self, bm: &BMesh, bv: Bev) {
        let vm = self.bvm(bv);
        let mut any = false;
        for v in self.bounds(vm) {
            let mut vseam = false;
            let mut e = self.bnd(v).efirst;
            let elast = self.bnd(v).elast;
            while let Some(eh) = e {
                vseam |= self.eh(eh).is_seam;
                if Some(eh) == elast {
                    break;
                }
                e = Some(self.enext(eh));
            }
            self.bnd_mut(v).any_seam = vseam;
            any |= vseam;
        }
        self.bev_mut(bv).any_seam = any;
        if self.mark_seam {
            self.check_edge_data_seam_sharp_edges(bm, bv, SeamSharp::Seam);
        }
        if self.mark_sharp {
            self.check_edge_data_seam_sharp_edges(bm, bv, SeamSharp::Sharp);
        }
    }

    /// `eh_on_plane`: is `e` between two faces with normals 0 or 180 degrees apart?
    fn eh_on_plane(&self, bm: &BMesh, e: Eh) -> bool {
        let eh = self.eh(e);
        if let (Some(fp), Some(fnx)) = (eh.fprev, eh.fnext) {
            let d = math::dot(fno(bm, fp), fno(bm, fnx));
            if (d + 1.0).abs() <= BEVEL_EPSILON_BIG || (d - 1.0).abs() <= BEVEL_EPSILON_BIG {
                return true;
            }
        }
        false
    }

    /// `build_boundary_vertex_only`.
    fn build_boundary_vertex_only(&mut self, bm: &BMesh, bv: Bev, construct: bool) {
        debug_assert_eq!(self.affect_type, Affect::Vertices);
        let vm = self.bvm(bv);
        let v = self.bev(bv).v;
        let efirst = self.bev(bv).edge(0);
        let mut e = efirst;
        loop {
            let p = self.slide_dist(bm, e, v, self.eh(e).offset_l);
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                self.bnd_mut(b).efirst = Some(e);
                self.bnd_mut(b).elast = Some(e);
                self.eh_mut(e).leftv = Some(b);
                self.eh_mut(e).rightv = Some(b);
            } else {
                let lv = self.eh(e).leftv.expect("left boundvert");
                self.adjust_bound_vert(lv, p);
            }
            e = self.enext(e);
            if e == efirst {
                break;
            }
        }
        if construct {
            self.set_bound_vert_seams(bm, bv);
            // Also check for seams at the vertex itself.
            if self.affect_vertices_odd
                && !self.bev(bv).any_seam
                && !contig_ldata_around_vert(bm, v)
            {
                self.bev_mut(bv).any_seam = true;
            }
            let seg = self.seg;
            let m = self.vm_mut(vm);
            m.mesh_kind = if m.count == 2 {
                MeshKind::None
            } else if seg == 1 {
                MeshKind::Poly
            } else {
                MeshKind::Adj
            };
        }
    }

    /// `build_boundary_terminal_edge`: only one edge beveled at `bv`.
    fn build_boundary_terminal_edge(&mut self, bm: &BMesh, bv: Bev, efirst: Eh, construct: bool) {
        let vm = self.bvm(bv);
        let v = self.bev(bv).v;
        let edgecount = self.bev(bv).edgecount;
        let mut e = efirst;
        if edgecount == 2 {
            // Only 2 edges in: terminate the edge with an artificial vertex on the unbeveled
            // edge.
            let eh = self.eh(e);
            let no = eh.fprev.or(eh.fnext).map(|f| fno(bm, f));
            let p = self.offset_in_plane(bm, e, no, true);
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                let bb = self.bnd_mut(b);
                bb.efirst = Some(e);
                bb.elast = Some(e);
                bb.ebev = Some(e);
                self.eh_mut(e).leftv = Some(b);
            } else {
                let lv = self.eh(e).leftv.expect("left boundvert");
                self.adjust_bound_vert(lv, p);
            }
            let eh = self.eh(e);
            let no = eh.fnext.or(eh.fprev).map(|f| fno(bm, f));
            let p = self.offset_in_plane(bm, e, no, false);
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                let bb = self.bnd_mut(b);
                bb.efirst = Some(e);
                bb.elast = Some(e);
                self.eh_mut(e).rightv = Some(b);
            } else {
                let rv = self.eh(e).rightv.expect("right boundvert");
                self.adjust_bound_vert(rv, p);
            }
            // Make an artificial extra point along the unbeveled edge, and form a triangle.
            let en = self.enext(e);
            let p = self.slide_dist(bm, en, v, self.eh(e).offset_l);
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                let bb = self.bnd_mut(b);
                bb.efirst = Some(en);
                bb.elast = Some(en);
                self.eh_mut(en).leftv = Some(b);
                self.eh_mut(en).rightv = Some(b);
                self.set_bound_vert_seams(bm, bv);
            } else {
                let lv = self.eh(en).leftv.expect("left boundvert");
                self.adjust_bound_vert(lv, p);
            }
        } else {
            // More than 2 edges in: put on-edge verts on all the other edges and join with the
            // beveled edge to make a poly or adj mesh.
            let leg_slide = matches!(self.offset_type, OffsetType::Percent | OffsetType::Absolute);
            let ep = self.eprev(e);
            let p = if leg_slide {
                self.slide_dist(bm, ep, v, self.eh(e).offset_l)
            } else {
                self.offset_meet(bm, ep, e, v, self.eh(e).fprev, false, None)
            };
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                let bb = self.bnd_mut(b);
                bb.efirst = Some(ep);
                bb.elast = Some(e);
                bb.ebev = Some(e);
                self.eh_mut(e).leftv = Some(b);
                self.eh_mut(ep).leftv = Some(b);
                self.eh_mut(ep).rightv = Some(b);
            } else {
                let lv = self.eh(e).leftv.expect("left boundvert");
                self.adjust_bound_vert(lv, p);
            }
            e = self.enext(e);
            let ep = self.eprev(e);
            let p = if leg_slide {
                self.slide_dist(bm, e, v, self.eh(ep).offset_r)
            } else {
                self.offset_meet(bm, ep, e, v, self.eh(e).fprev, false, None)
            };
            if construct {
                let b = self.add_new_bound_vert(vm, p);
                let bb = self.bnd_mut(b);
                bb.efirst = Some(ep);
                bb.elast = Some(e);
                self.eh_mut(e).leftv = Some(b);
                self.eh_mut(e).rightv = Some(b);
                self.eh_mut(ep).rightv = Some(b);
            } else {
                let lv = self.eh(e).leftv.expect("left boundvert");
                self.adjust_bound_vert(lv, p);
            }
            // For the edges not adjacent to the beveled edge, slide the bevel amount along.
            let mut d = self.eh(efirst).offset_l_spec;
            if self.profile < 0.25 {
                d *= 2.0f32.sqrt(); // Go further along the edge to make room for the profile.
            }
            e = self.enext(e);
            while self.enext(e) != efirst {
                let p = self.slide_dist(bm, e, v, d);
                if construct {
                    let b = self.add_new_bound_vert(vm, p);
                    let bb = self.bnd_mut(b);
                    bb.efirst = Some(e);
                    bb.elast = Some(e);
                    self.eh_mut(e).leftv = Some(b);
                    self.eh_mut(e).rightv = Some(b);
                } else {
                    let lv = self.eh(e).leftv.expect("left boundvert");
                    self.adjust_bound_vert(lv, p);
                }
                e = self.enext(e);
            }
        }

        if edgecount >= 3 {
            // Special case: snap the profile to the plane of the adjacent two edges.
            let bndv = self.bstart(vm);
            debug_assert!(self.bnd(bndv).ebev.is_some());
            self.set_profile_params(bm, bv, bndv);
            self.move_profile_plane(bm, bndv, v);
        }

        if construct {
            self.set_bound_vert_seams(bm, bv);
            let m = self.vm_mut(vm);
            m.mesh_kind = if m.count == 2 && edgecount == 3 {
                MeshKind::None
            } else if m.count == 3 {
                MeshKind::TriFan
            } else {
                MeshKind::Poly
            };
        }
    }

    /// `adjust_miter_coords`: helper for `build_boundary` handling special outer miters.
    fn adjust_miter_coords(&mut self, bm: &BMesh, bv: Bev, emiter: Eh) {
        let v = self.bev(bv).v;
        let v1 = self.eh(emiter).rightv.expect("right boundvert");
        let v3 = if self.miter_outer == Miter::Patch {
            self.bnext(self.bnext(v1))
        } else {
            debug_assert_eq!(self.miter_outer, Miter::Arc);
            self.bnext(v1)
        };
        let v1prev = self.bprev(v1);
        let v3next = self.bnext(v3);
        let co2 = self.bnd(v1).nv.co;
        if self.bnd(v1).is_arc_start {
            self.bnd_mut(v1).profile.middle = co2;
        }
        // co1 is the intersection of the line through co2 in the direction of emiter->e and
        // the plane with normal the direction of emiter->e through v1prev.
        let vother = self.other_vert(bm, emiter, v);
        let edge_dir = math::norm(math::sub(co(bm, v), co(bm, vother)));
        let d = self.offset / (self.seg as f32 / 2.0); // A fallback amount to move.
        let line_p = math::madd(co2, edge_dir, d);
        let co1 = math::isect_line_plane_v3(co2, line_p, self.bnd(v1prev).nv.co, edge_dir)
            .unwrap_or(line_p);
        self.adjust_bound_vert(v1, co1);
        // co3 is similar, but the plane is through v3next and the line is on the other side
        // of the miter edge.
        let emiter_other = self.bnd(v3).elast.expect("miter edge");
        let vother = self.other_vert(bm, emiter_other, v);
        let edge_dir = math::norm(math::sub(co(bm, v), co(bm, vother)));
        let line_p = math::madd(co2, edge_dir, d);
        // Blender writes the fallback to co1 here (a bug), leaving co3 uninitialised; frap
        // keeps co3 at the fallback point.
        let co3 = math::isect_line_plane_v3(co2, line_p, self.bnd(v3next).nv.co, edge_dir)
            .unwrap_or(line_p);
        self.adjust_bound_vert(v3, co3);
    }

    /// `adjust_miter_inner_coords`.
    fn adjust_miter_inner_coords(&mut self, bm: &BMesh, bv: Bev, emiter: Option<Eh>) {
        let vm = self.bvm(bv);
        let bvv = self.bev(bv).v;
        let vstart = self.bstart(vm);
        let mut v = vstart;
        loop {
            if self.bnd(v).is_arc_start {
                let v3 = self.bnext(v);
                let e = self.bnd(v).efirst.expect("arc edge");
                if Some(e) != emiter {
                    let p = self.bnd(v).nv.co;
                    let vother = self.other_vert(bm, e, bvv);
                    let edge_dir = math::norm(math::sub(co(bm, vother), co(bm, bvv)));
                    self.bnd_mut(v).nv.co = math::madd(p, edge_dir, self.spread);
                    let e3 = self.bnd(v3).elast.expect("arc edge");
                    let vother = self.other_vert(bm, e3, bvv);
                    let edge_dir = math::norm(math::sub(co(bm, vother), co(bm, bvv)));
                    self.bnd_mut(v3).nv.co = math::madd(p, edge_dir, self.spread);
                }
                v = self.bnext(v3);
            } else {
                v = self.bnext(v);
            }
            if v == vstart {
                break;
            }
        }
    }

    /// `build_boundary`: the circular list of BoundVerts of `bv` (or, without `construct`,
    /// their recalculated coordinates).
    pub(crate) fn build_boundary(&mut self, bm: &BMesh, bv: Bev, construct: bool) {
        // Current bevel does nothing if only one edge into a vertex.
        if self.bev(bv).edgecount <= 1 {
            return;
        }
        if self.affect_type == Affect::Vertices {
            self.build_boundary_vertex_only(bm, bv, construct);
            return;
        }
        let vm = self.bvm(bv);
        let v = self.bev(bv).v;
        // Find a beveled edge to be efirst.
        let efirst = self.next_bev(bv, None).expect("a beveled edge");
        if self.bev(bv).selcount == 1 {
            // Special case: only one beveled edge in.
            self.build_boundary_terminal_edge(bm, bv, efirst, construct);
            return;
        }
        // Special miters outside only for 3 or more beveled edges.
        let miter_outer = if self.bev(bv).selcount >= 3 {
            self.miter_outer
        } else {
            Miter::Sharp
        };
        let miter_inner = self.miter_inner;
        // The first beveled edge of an outside miter (at most 1 per bv).
        let mut emiter: Option<Eh> = None;

        let mut e = efirst;
        loop {
            debug_assert!(self.eh(e).is_bev);
            let mut eon: Option<Eh> = None;
            // Analyze edges until the next beveled edge: "in plane" (preceding and subsequent
            // faces are coplanar) or not.
            let mut in_plane = 0;
            let mut not_in_plane = 0;
            let mut enip: Option<Eh> = None;
            let mut eip: Option<Eh> = None;
            let mut e2 = self.enext(e);
            while !self.eh(e2).is_bev {
                if self.eh_on_plane(bm, e2) {
                    in_plane += 1;
                    eip = Some(e2);
                } else {
                    not_in_plane += 1;
                    enip = Some(e2);
                }
                e2 = self.enext(e2);
            }
            let mut r = 0.0f32;
            let p;
            if in_plane == 0 && not_in_plane == 0 {
                p = self.offset_meet(bm, e, e2, v, self.eh(e).fnext, false, None);
            } else if not_in_plane > 0 {
                let enip = enip.expect("not-in-plane edge");
                if self.loop_slide
                    && not_in_plane == 1
                    && self.good_offset_on_edge_between(bm, e, e2, enip, v)
                {
                    let (m, sr) = self.offset_on_edge_between(bm, e, e2, enip, v);
                    p = m;
                    if let Some(sr) = sr {
                        eon = Some(enip);
                        r = sr;
                    }
                } else {
                    p = self.offset_meet(bm, e, e2, v, None, true, eip);
                }
            } else {
                // in_plane > 0 and not_in_plane == 0.
                let eip = eip.expect("in-plane edge");
                if self.loop_slide
                    && in_plane == 1
                    && self.good_offset_on_edge_between(bm, e, e2, eip, v)
                {
                    let (m, sr) = self.offset_on_edge_between(bm, e, e2, eip, v);
                    p = m;
                    if let Some(sr) = sr {
                        eon = Some(eip);
                        r = sr;
                    }
                } else {
                    // All edges between e and e2 are in the same plane: treat it like the
                    // case where there are no edges between.
                    p = self.offset_meet(bm, e, e2, v, self.eh(e).fnext, false, None);
                }
            }

            let ang_kind = self.edges_angle_kind(bm, e, e2, v);
            let special =
                (miter_outer != Miter::Sharp && emiter.is_none() && ang_kind == AngleKind::Larger)
                    || (miter_inner != Miter::Sharp && ang_kind == AngleKind::Smaller);
            if construct {
                let bnd = self.add_new_bound_vert(vm, p);
                {
                    let b = self.bnd_mut(bnd);
                    b.efirst = Some(e);
                    b.elast = Some(e2);
                    b.ebev = Some(e2);
                    b.eon = eon;
                    if eon.is_some() {
                        b.sinratio = r;
                    }
                }
                self.eh_mut(e).rightv = Some(bnd);
                self.eh_mut(e2).leftv = Some(bnd);
                let mut e3 = self.enext(e);
                while e3 != e2 {
                    self.eh_mut(e3).leftv = Some(bnd);
                    self.eh_mut(e3).rightv = Some(bnd);
                    e3 = self.enext(e3);
                }
                // Special mitering: only one outer reflex angle per vertex.
                if special {
                    if ang_kind == AngleKind::Larger {
                        emiter = Some(e);
                    }
                    // Make one or two more boundverts; for now all have the same coordinate.
                    let v1 = bnd;
                    self.bnd_mut(v1).ebev = None;
                    let patch = ang_kind == AngleKind::Larger && miter_outer == Miter::Patch;
                    let v2 = if patch {
                        Some(self.add_new_bound_vert(vm, p))
                    } else {
                        None
                    };
                    let v3 = self.add_new_bound_vert(vm, p);
                    {
                        let b3 = self.bnd_mut(v3);
                        b3.ebev = Some(e2);
                        b3.efirst = Some(e2);
                        b3.elast = Some(e2);
                        b3.eon = None;
                    }
                    self.eh_mut(e2).leftv = Some(v3);
                    if patch {
                        let v2 = v2.expect("patch boundvert");
                        self.bnd_mut(v1).is_patch_start = true;
                        let (v1eon, v1sr) = (self.bnd(v1).eon, self.bnd(v1).sinratio);
                        {
                            let b2 = self.bnd_mut(v2);
                            b2.eon = v1eon;
                            b2.sinratio = v1sr;
                            b2.ebev = None;
                        }
                        {
                            let b1 = self.bnd_mut(v1);
                            b1.eon = None;
                            b1.sinratio = 1.0;
                            b1.elast = Some(e);
                        }
                        if self.enext(e) == e2 {
                            self.bnd_mut(v2).efirst = None;
                            self.bnd_mut(v2).elast = None;
                        } else {
                            self.bnd_mut(v2).efirst = Some(self.enext(e));
                            let mut e3 = self.enext(e);
                            while e3 != e2 {
                                self.eh_mut(e3).leftv = Some(v2);
                                self.eh_mut(e3).rightv = Some(v2);
                                self.bnd_mut(v2).elast = Some(e3);
                                e3 = self.enext(e3);
                            }
                        }
                    } else {
                        self.bnd_mut(v1).is_arc_start = true;
                        self.bnd_mut(v1).profile.middle = p;
                        if self.enext(e) == e2 {
                            let ef = self.bnd(v1).efirst;
                            self.bnd_mut(v1).elast = ef;
                        } else {
                            let between = in_plane + not_in_plane;
                            let bet2 = between / 2;
                            let betodd = between % 2 == 1;
                            let mut i = 0;
                            // First half of in-between edges at index 0, second half at index
                            // seg; if between is odd, the middle one at the mid-index.
                            let mut e3 = self.enext(e);
                            while e3 != e2 {
                                self.bnd_mut(v1).elast = Some(e3);
                                let pi = if i < bet2 {
                                    0
                                } else if betodd && i == bet2 {
                                    self.seg / 2
                                } else {
                                    self.seg
                                };
                                self.eh_mut(e3).profile_index = pi;
                                i += 1;
                                e3 = self.enext(e3);
                            }
                        }
                    }
                }
            } else if special {
                if ang_kind == AngleKind::Larger {
                    emiter = Some(e);
                }
                let v1 = self.eh(e).rightv.expect("right boundvert");
                let (v2, v3) = if ang_kind == AngleKind::Larger && miter_outer == Miter::Patch {
                    let v2 = self.bnext(v1);
                    (Some(v2), self.bnext(v2))
                } else {
                    (None, self.bnext(v1))
                };
                self.adjust_bound_vert(v1, p);
                if let Some(v2) = v2 {
                    self.adjust_bound_vert(v2, p);
                }
                self.adjust_bound_vert(v3, p);
            } else {
                let rv = self.eh(e).rightv.expect("right boundvert");
                self.adjust_bound_vert(rv, p);
            }
            e = e2;
            if e == efirst {
                break;
            }
        }

        if miter_inner != Miter::Sharp {
            self.adjust_miter_inner_coords(bm, bv, emiter);
        }
        if let Some(em) = emiter {
            self.adjust_miter_coords(bm, bv, em);
        }

        if construct {
            self.set_bound_vert_seams(bm, bv);
            let seg1 = self.eh(efirst).seg == 1;
            let method = self.vmesh_method;
            let m = self.vm_mut(vm);
            m.mesh_kind = if m.count == 2 {
                MeshKind::None
            } else if seg1 {
                MeshKind::Poly
            } else {
                match method {
                    VmeshMethod::Adj => MeshKind::Adj,
                    VmeshMethod::Cutoff => MeshKind::Cutoff,
                }
            };
        }
    }

    /// `adjust_the_cycle_or_chain`: least-squares adjustment of the offsets of one cycle or
    /// chain of BoundVerts linked through `adjchain`.
    fn adjust_the_cycle_or_chain(&mut self, vstart: Bnd, iscycle: bool) {
        let mut np = 0usize;
        let mut v = Some(vstart);
        while let Some(vv) = v {
            np += 1;
            v = self.bnd(vv).adjchain;
            if v == Some(vstart) {
                break;
            }
        }
        let nrows = if iscycle { 3 * np } else { 3 * np - 3 };
        let mut entries: Vec<(usize, usize, f64)> = Vec::new();
        let mut b = vec![0.0f64; nrows];
        let weight = BEVEL_MATCH_SPEC_WEIGHT;
        let mut v = vstart;
        let mut i = 0usize;
        loop {
            if iscycle || i < np - 1 {
                let eright = self.bnd(v).efirst.expect("efirst");
                let adj = self.bnd(v).adjchain.expect("adjchain");
                let enextleft = self.bnd(adj).elast.expect("elast");
                // Residue i: width difference between eright and eleft of next.
                entries.push((i, i, 1.0));
                if iscycle {
                    let r = if i > 0 { i - 1 } else { np - 1 };
                    entries.push((r, i, -f64::from(self.bnd(v).sinratio)));
                } else if i > 0 {
                    entries.push((i - 1, i, -f64::from(self.bnd(v).sinratio)));
                }
                // Right offset for parameter i matches its spec; weighted.
                let row = if iscycle { np + 2 * i } else { np - 1 + 2 * i };
                entries.push((row, i, weight));
                b[row] += weight * f64::from(self.eh(eright).offset_r);
                // Left offset for parameter i matches its spec; weighted.
                let row = row + 1;
                let col = if i == np - 1 { 0 } else { i + 1 };
                entries.push((row, col, weight * f64::from(self.bnd(adj).sinratio)));
                b[row] += weight * f64::from(self.eh(enextleft).offset_l);
            } else {
                // Not a cycle, and last of chain.
                entries.push((i - 1, i, -1.0));
            }
            i += 1;
            match self.bnd(v).adjchain {
                Some(n) if n != vstart => v = n,
                _ => break,
            }
        }
        let Some(x) = math::least_squares(nrows, np, &entries, &b) else {
            // Eigen's solver failure leaves the variables at zero.
            return self.apply_cycle_solution(vstart, iscycle, np, &vec![0.0; np]);
        };
        self.apply_cycle_solution(vstart, iscycle, np, &x);
    }

    fn apply_cycle_solution(&mut self, vstart: Bnd, iscycle: bool, np: usize, x: &[f64]) {
        let mut v = vstart;
        let mut i = 0usize;
        loop {
            let val = x[i];
            if iscycle || i < np - 1 {
                let eright = self.bnd(v).efirst.expect("efirst");
                let eleft = self.bnd(v).elast.expect("elast");
                self.eh_mut(eright).offset_r = val as f32;
                if iscycle || v != vstart {
                    let sr = f64::from(self.bnd(v).sinratio);
                    self.eh_mut(eleft).offset_l = (sr * val) as f32;
                }
            } else {
                let eleft = self.bnd(v).elast.expect("elast");
                self.eh_mut(eleft).offset_l = val as f32;
            }
            i += 1;
            match self.bnd(v).adjchain {
                Some(n) if n != vstart => v = n,
                _ => break,
            }
        }
    }

    /// `find_other_end_edge_half`: the EdgeHalf of the other end of `e->e` and its BevVert
    /// (both `None` when that end is not beveled).
    pub(crate) fn find_other_end_edge_half(&self, bm: &BMesh, e: Eh) -> (Option<Eh>, Option<Bev>) {
        let [v1, v2] = bm.edge(self.ehe(e)).v;
        let other = if self.eh(e).is_rev { v1 } else { v2 };
        match self.find_bevvert(other) {
            Some(bvo) => {
                let eo = self.find_edge_half(bvo, self.ehe(e));
                debug_assert!(eo.is_some());
                (eo, Some(bvo))
            }
            None => (None, None),
        }
    }

    /// `adjust_offsets`: even out the widths of chains and cycles of loop-slide BoundVerts,
    /// then rebuild the boundaries.
    pub(crate) fn adjust_offsets(&mut self, bm: &BMesh) {
        // For repeatability, iterate over all verts of the mesh.
        for bmv in bm.vert_ids() {
            if !bm.vert(bmv).flag.contains(ElemFlag::TAG) {
                continue;
            }
            let Some(bv) = self.find_bevvert(bmv) else {
                continue;
            };
            let vm = self.bvm(bv);
            let start = self.bstart(vm);
            let mut vanchor = start;
            loop {
                if !(self.bnd(vanchor).visited || self.bnd(vanchor).eon.is_none()) {
                    self.adjust_from_anchor(bm, bv, vanchor);
                }
                vanchor = self.bnext(vanchor);
                if vanchor == start {
                    break;
                }
            }
        }
        // Rebuild boundaries with the new width specs.
        for bmv in bm.vert_ids() {
            if bm.vert(bmv).flag.contains(ElemFlag::TAG)
                && let Some(bv) = self.find_bevvert(bmv)
            {
                self.build_boundary(bm, bv, false);
            }
        }
    }

    /// The body of `adjust_offsets` for one anchor BoundVert.
    fn adjust_from_anchor(&mut self, bm: &BMesh, bv: Bev, vanchor: Bnd) {
        let _ = bv;
        let mut v = vanchor;
        let mut vchainstart = vanchor;
        let mut vchainend = vanchor;
        let mut iscycle = false;
        let mut chainlen = 1;
        // First follow paired edges in the left->right direction.
        while self.bnd(v).eon.is_some() && !self.bnd(v).visited && !iscycle {
            self.bnd_mut(v).visited = true;
            let Some(ef) = self.bnd(v).efirst else {
                break;
            };
            let (Some(enext), _) = self.find_other_end_edge_half(bm, ef) else {
                break;
            };
            let vnext = self.eh(enext).leftv.expect("left boundvert");
            self.bnd_mut(v).adjchain = Some(vnext);
            vchainend = vnext;
            chainlen += 1;
            if self.bnd(vnext).visited {
                if vnext != vchainstart {
                    break;
                }
                self.adjust_the_cycle_or_chain(vchainstart, true);
                iscycle = true;
            }
            v = vnext;
        }
        if !iscycle {
            // right->left direction, changing vchainstart at each step.
            self.bnd_mut(v).adjchain = None;
            v = vchainstart;
            loop {
                self.bnd_mut(v).visited = true;
                let Some(el) = self.bnd(v).elast else {
                    break;
                };
                let (Some(enext), _) = self.find_other_end_edge_half(bm, el) else {
                    break;
                };
                let vnext = self.eh(enext).rightv.expect("right boundvert");
                self.bnd_mut(vnext).adjchain = Some(v);
                chainlen += 1;
                vchainstart = vnext;
                v = vnext;
                if self.bnd(v).visited || self.bnd(v).eon.is_none() {
                    break;
                }
            }
            if chainlen >= 3
                && self.bnd(vchainstart).eon.is_none()
                && self.bnd(vchainend).eon.is_none()
            {
                self.adjust_the_cycle_or_chain(vchainstart, false);
            }
        }
    }
}

/// `nearly_parallel_normalized`.
pub(crate) fn nearly_parallel_normalized(d1: V3, d2: V3) -> bool {
    let direction_dot = math::dot(d1, d2);
    math::compare_ff(direction_dot.abs(), 1.0, bevel_epsilon_ang_dot())
}

/// `nearly_parallel`.
pub(crate) fn nearly_parallel(d1: V3, d2: V3) -> bool {
    let ang = math::angle_v3v3(d1, d2);
    ang.abs() < bevel_epsilon_ang() || (ang - math::PI_F).abs() < bevel_epsilon_ang()
}

/// `slide_dist` on a plain edge (the fake EdgeHalfs of the percent/absolute case).
fn slide_dist_edge(bm: &BMesh, e: crate::EdgeId, v: VertId, mut d: f32) -> V3 {
    let (dir, len) = math::normalize_v3(math::sub(co(bm, v), co(bm, bm.edge_other_vert(e, v))));
    if d > len {
        d = len - (50.0 * super::types::BEVEL_EPSILON_D) as f32;
    }
    math::madd(co(bm, v), dir, -d)
}

/// `edge_edge_angle_less_than_180`: is the angle swept from `e1` to `e2`, CCW from the normal
/// side of `f`, neither reflex nor straight?
fn edge_edge_angle_less_than_180(
    bm: &BMesh,
    e1: crate::EdgeId,
    e2: crate::EdgeId,
    f: FaceId,
) -> bool {
    let [a1, a2] = bm.edge(e1).v;
    let [b1, b2] = bm.edge(e2).v;
    let (v, v1, v2) = if a1 == b1 {
        (a1, a2, b2)
    } else if a1 == b2 {
        (a1, a2, b1)
    } else if a2 == b1 {
        (a2, a1, b2)
    } else if a2 == b2 {
        (a2, a1, b1)
    } else {
        debug_assert!(false, "edges do not share a vertex");
        return false;
    };
    let dir1 = math::sub(co(bm, v1), co(bm, v));
    let dir2 = math::sub(co(bm, v2), co(bm, v));
    let cross = math::cross(dir1, dir2);
    math::dot(cross, fno(bm, f)) > 0.0
}
