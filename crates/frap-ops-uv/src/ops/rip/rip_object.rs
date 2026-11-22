// SPDX-License-Identifier: GPL-3.0-or-later
//! `uv_rip_object`: the UV rip of one mesh, shared by `uv.rip` and `uv.rip_move`.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_rip.cc (`ULData`,
//! `bm_loop_find_other_radial_loop_with_visible_face`,
//! `bm_loop_find_other_fan_loop_with_visible_face`, `bm_vert_step_fan_loop_uv`,
//! `bm_loop_uv_select_single_vert_validate`, `bm_loop_calc_uv_angle_from_dir`,
//! `uv_rip_single_from_loop`, `uv_rip_pairs_*`, `uv_rip_object`) and
//! blender/source/blender/bmesh/intern/bmesh_query_uv.cc (`BM_loop_uv_share_edge_check`,
//! `BM_edge_uv_share_vert_check`, `BM_face_uv_calc_cross`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender keeps the rip sets in hash sets of loop pointers; frap keeps them in insertion
//! order. Only the float sums of the rip-pair centre and directions depend on that order.

use frap_bmesh::{EdgeId, LoopId, VertId};

use crate::select::{Uv, UvSel};

use super::super::weld::common::{angle, dot, normalize, sub};

/// `ULData` (Blender keeps it in `BMLoop.head.index`).
#[derive(Copy, Clone, Debug, Default)]
struct Ul {
    is_select_edge: bool,
    is_select_vert_single: bool,
    is_select_all: bool,
    in_stack: bool,
    in_rip_pairs: bool,
    side: u8,
    side_was_swapped: bool,
}

/// Why a rip changed nothing (`BKE_report` `RPT_ERROR`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RipError {
    /// `Edge must have connected edges`
    Edge,
    /// `Vertex must have connected vertices`
    Vertex,
}

impl RipError {
    /// The report message.
    pub(crate) fn message(self) -> &'static str {
        match self {
            RipError::Edge => "Edge must have connected edges",
            RipError::Vertex => "Vertex must have connected vertices",
        }
    }
}

struct Rip<'a, 'b> {
    s: &'a mut UvSel<'b>,
    ul: Vec<Ul>,
    /// `BM_ELEM_TAG` of the faces: visible.
    ftag: Vec<bool>,
    aspect_y: f32,
}

/// `equals_v2v2`.
#[inline]
fn eq(a: Uv, b: Uv) -> bool {
    a[0] == b[0] && a[1] == b[1]
}

impl Rip<'_, '_> {
    fn ul(&self, l: LoopId) -> Ul {
        self.ul[l.slot()]
    }
    fn ul_mut(&mut self, l: LoopId) -> &mut Ul {
        &mut self.ul[l.slot()]
    }
    fn tagged(&self, l: LoopId) -> bool {
        self.ftag[self.s.lf(l).slot()]
    }

    /// `BM_loop_uv_share_edge_check`.
    fn share_edge(&self, la: LoopId, lb: LoopId) -> bool {
        self.s.share_edge(la, lb)
    }

    /// `BM_edge_uv_share_vert_check`.
    fn edge_share_vert(&self, e: EdgeId, la: LoopId, lb: LoopId) -> bool {
        if !eq(self.s.uv(la), self.s.uv(lb)) {
            return false;
        }
        let other = |l: LoopId| {
            if self.s.le(l) == e {
                self.s.next(l)
            } else {
                self.s.prev(l)
            }
        };
        eq(self.s.uv(other(la)), self.s.uv(other(lb)))
    }

    /// `BM_face_uv_calc_cross` (`cross_poly_v2`).
    fn face_uv_cross(&self, l: LoopId) -> f32 {
        let loops = self.s.face_loops(self.s.lf(l));
        let mut cross = 0.0f32;
        let mut prev = self.s.uv(*loops.last().expect("a face has corners"));
        for &lc in &loops {
            let cur = self.s.uv(lc);
            cross += (prev[0] - cur[0]) * (cur[1] + prev[1]);
            prev = cur;
        }
        cross
    }

    /// `bm_loop_find_other_radial_loop_with_visible_face`.
    fn other_radial_loop(&self, l_src: LoopId) -> Option<LoopId> {
        let mut l_other = None;
        let mut it = self.s.radial_next(l_src);
        if it != l_src {
            loop {
                if self.tagged(it) && self.ul(it).is_select_edge && self.share_edge(l_src, it) {
                    if l_other.is_none() {
                        l_other = Some(it);
                    } else {
                        return None;
                    }
                }
                it = self.s.radial_next(it);
                if it == l_src {
                    break;
                }
            }
        }
        l_other
    }

    /// `bm_loop_find_other_fan_loop_with_visible_face`.
    fn other_fan_loop(&self, l_src: LoopId, v_src: VertId) -> Option<LoopId> {
        let mut l_other = None;
        let mut it = self.s.radial_next(l_src);
        if it != l_src {
            loop {
                if self.tagged(it) && self.share_edge(l_src, it) {
                    if l_other.is_none() {
                        l_other = Some(it);
                    } else {
                        l_other = None;
                        break;
                    }
                }
                it = self.s.radial_next(it);
                if it == l_src {
                    break;
                }
            }
        }
        let l = l_other?;
        if self.s.lv(l) == v_src {
            Some(l)
        } else if self.s.lv(self.s.next(l)) == v_src {
            Some(self.s.next(l))
        } else if self.s.lv(self.s.prev(l)) == v_src {
            Some(self.s.prev(l))
        } else {
            None
        }
    }

    /// `bm_vert_step_fan_loop_uv`.
    fn step_fan(&self, l: LoopId, e_step: &mut EdgeId) -> Option<LoopId> {
        let e_prev = *e_step;
        let l_next = if self.s.le(l) == e_prev {
            self.s.prev(l)
        } else if self.s.le(self.s.prev(l)) == e_prev {
            l
        } else {
            return None;
        };
        *e_step = self.s.le(l_next);
        self.other_fan_loop(l_next, self.s.lv(l))
    }

    /// `bm_loop_uv_select_single_vert_validate`.
    fn single_vert_validate(&mut self, l_init: LoopId) {
        let luv_init = self.s.uv(l_init);
        let loops = self.s.bm.vert_loops(self.s.lv(l_init));
        let mut is_single_vert = true;
        for &l in &loops {
            if eq(luv_init, self.s.uv(l))
                && (self.ul(self.s.prev(l)).is_select_edge || self.ul(l).is_select_edge)
            {
                is_single_vert = false;
                break;
            }
        }
        if !is_single_vert {
            for &l in &loops {
                if self.ul(l).is_select_vert_single && eq(luv_init, self.s.uv(l)) {
                    self.ul_mut(l).is_select_vert_single = false;
                }
            }
        }
    }

    /// `bm_loop_calc_uv_angle_from_dir`: `(corner angle, edge angle, edge index)`.
    fn uv_angle_from_dir(&self, l: LoopId, dir: Uv) -> (f32, f32, i32) {
        let luv = self.s.uv(l);
        let luv_prev = self.s.uv(self.s.prev(l));
        let luv_next = self.s.uv(self.s.next(l));
        let mut d0 = sub(luv, luv_prev);
        let mut d2 = sub(luv, luv_next);
        d0[1] /= self.aspect_y;
        d2[1] /= self.aspect_y;
        normalize(&mut d0);
        normalize(&mut d2);
        let mut d1 = sub(d0, d2);
        normalize(&mut d1);
        d1 = [d1[1], -d1[0]];
        if self.face_uv_cross(l) > 0.0 {
            d1 = [-d1[0], -d1[1]];
        }
        let angles = [angle(dir, d0), angle(dir, d1), angle(dir, d2)];
        if angles[0] < angles[2] {
            (angles[1], angles[0], -1)
        } else {
            (angles[1], angles[2], 1)
        }
    }

    /// `uv_rip_single_from_loop`: the loops of the rip (side 0 is ripped away).
    fn single_from_loop(&mut self, l_init_orig: LoopId, co: Uv) -> Vec<LoopId> {
        let co_center = self.s.uv(l_init_orig);
        let mut loops: Vec<LoopId> = Vec::new();
        let mut l_init = None;
        let mut l_init_edge = None;
        let mut corner_angle_best = f32::MAX;
        let mut edge_angle_best = f32::MAX;
        let mut edge_index_best = 0;

        let mut dir_co = sub(co_center, co);
        dir_co[1] /= self.aspect_y;
        if normalize(&mut dir_co) == 0.0 {
            dir_co[1] = 1.0;
        }
        let mut uv_fan_count_all = 0;
        for l in self.s.bm.vert_loops(self.s.lv(l_init_orig)) {
            if !self.tagged(l) || !eq(co_center, self.s.uv(l)) {
                continue;
            }
            uv_fan_count_all += 1;
            let u = self.ul_mut(l);
            u.is_select_vert_single = true;
            u.side = 0;
            loops.push(l);
            let (corner, mut edge_angle, edge_index) = self.uv_angle_from_dir(l, dir_co);
            if corner_angle_best == f32::MAX || corner < corner_angle_best {
                corner_angle_best = corner;
                l_init = Some(l);
            }
            // Concave corners are not further away than they should be.
            edge_angle = corner.min(edge_angle);
            if edge_angle_best == f32::MAX || edge_angle < edge_angle_best {
                edge_angle_best = edge_angle;
                edge_index_best = edge_index;
                l_init_edge = Some(l);
            }
        }
        let (Some(l_init), Some(l_init_edge)) = (l_init, l_init_edge) else {
            return loops;
        };

        // Walk around `l_init` in both directions of the UV fan.
        let mut uv_fan_count_contiguous = 1;
        self.ul_mut(l_init).side = 1;
        for i in 0..2 {
            let mut e_prev = if i == 1 {
                self.s.le(l_init)
            } else {
                self.s.le(self.s.prev(l_init))
            };
            let mut l_iter = l_init;
            while let Some(next) = self.step_fan(l_iter, &mut e_prev) {
                if next == l_init || self.ul(next).side != 0 {
                    break;
                }
                l_iter = next;
                uv_fan_count_contiguous += 1;
                self.ul_mut(l_iter).side = 1;
            }
        }
        if uv_fan_count_contiguous == uv_fan_count_all {
            for &l in &loops {
                self.ul_mut(l).side = 0;
            }
            if uv_fan_count_contiguous <= 2 {
                // Simple case: rip away the closest loop.
                self.ul_mut(l_init).side = 1;
            } else {
                // Rip away from the closest edge.
                let l_radial_init = if edge_index_best == -1 {
                    self.s.prev(l_init_edge)
                } else {
                    l_init_edge
                };
                let mut it = l_radial_init;
                loop {
                    if self.share_edge(l_radial_init, it) {
                        let l = if self.s.lv(it) == self.s.lv(l_init) {
                            it
                        } else {
                            self.s.next(it)
                        };
                        self.ul_mut(l).side = 1;
                    }
                    it = self.s.radial_next(it);
                    if it == l_radial_init {
                        break;
                    }
                }
            }
        }
        loops
    }

    /// `uv_rip_pairs_calc_uv_angle`.
    fn pairs_uv_angle(&self, l_init: LoopId, side: u8) -> f32 {
        let luv_init = self.s.uv(l_init);
        let mut angle_of_side = 0.0f32;
        for l in self.s.bm.vert_loops(self.s.lv(l_init)) {
            let u = self.ul(l);
            if u.in_rip_pairs && u.side == side && eq(luv_init, self.s.uv(l)) {
                let luv = self.s.uv(l);
                let mut dir_prev = sub(self.s.uv(self.s.prev(l)), luv);
                let mut dir_next = sub(self.s.uv(self.s.next(l)), luv);
                dir_prev[1] /= self.aspect_y;
                dir_next[1] /= self.aspect_y;
                let a = angle(dir_prev, dir_next);
                if a.is_finite() {
                    angle_of_side += a;
                }
            }
        }
        angle_of_side
    }

    /// `uv_rip_pairs_loop_count_on_side`.
    fn pairs_count_on_side(&self, l_init: LoopId, side: u8) -> i32 {
        let luv_init = self.s.uv(l_init);
        self.s
            .bm
            .vert_loops(self.s.lv(l_init))
            .into_iter()
            .filter(|&l| {
                let u = self.ul(l);
                u.in_rip_pairs && u.side == side && eq(luv_init, self.s.uv(l))
            })
            .count() as i32
    }

    /// `uv_rip_pairs_loop_change_sides_test`.
    fn change_sides_test(&mut self, l_switch: LoopId, l_target: LoopId) -> bool {
        let side_a = self.ul(l_switch).side;
        let side_b = self.ul(l_target).side;
        let count_a = self.pairs_count_on_side(l_switch, side_a);
        let count_b = self.pairs_count_on_side(l_target, side_b);
        if count_a + count_b == 4 {
            return count_a > count_b;
        }
        let angle_a_before = self.pairs_uv_angle(l_switch, side_a);
        let angle_b_before = self.pairs_uv_angle(l_target, side_b);
        self.ul_mut(l_switch).side = side_b;
        let angle_a_after = self.pairs_uv_angle(l_switch, side_a);
        let angle_b_after = self.pairs_uv_angle(l_target, side_b);
        self.ul_mut(l_switch).side = side_a;
        (angle_a_before - angle_b_before).abs() > (angle_a_after - angle_b_after).abs()
    }

    /// `UV_SET_SIDE_AND_REMOVE_FROM_RAIL`.
    fn set_side_and_remove(
        &mut self,
        l: LoopId,
        side: u8,
        stack: &mut Vec<LoopId>,
        pairs: &mut Vec<LoopId>,
    ) {
        if !self.ul(l).in_stack {
            stack.push(l);
            self.ul_mut(l).in_stack = true;
        }
        if self.ul(l).in_rip_pairs {
            self.ul_mut(l).in_rip_pairs = false;
            if let Some(i) = pairs.iter().position(|&x| x == l) {
                pairs.remove(i);
            }
        }
        let u = self.ul_mut(l);
        u.side = side;
        u.side_was_swapped = true;
    }

    /// Pushes `l_other` on `side`, or moves it to `side` when that is better.
    fn visit_same_side(
        &mut self,
        l_other: LoopId,
        l_step: LoopId,
        side: u8,
        stack: &mut Vec<LoopId>,
        pairs: &mut Vec<LoopId>,
    ) {
        let u = self.ul(l_other);
        if !u.in_rip_pairs && !u.in_stack {
            stack.push(l_other);
            let u = self.ul_mut(l_other);
            u.in_stack = true;
            u.side = side;
        } else if u.side != side
            && !u.side_was_swapped
            && self.change_sides_test(l_other, l_step)
        {
            self.set_side_and_remove(l_other, side, stack, pairs);
        }
    }

    /// `uv_rip_pairs_from_loop`: the loops of both sides along the selected edges.
    fn pairs_from_loop(&mut self, l_init: LoopId) -> Vec<LoopId> {
        let mut pairs: Vec<LoopId> = Vec::new();
        let mut stack: Vec<LoopId> = vec![l_init];
        self.ul_mut(l_init).in_stack = true;
        while let Some(l_step) = stack.pop() {
            let side = self.ul(l_step).side;
            self.ul_mut(l_step).in_stack = false;
            self.ul_mut(l_step).in_rip_pairs = true;
            pairs.push(l_step);

            if self.ul(l_step).is_select_edge {
                // The other side, when it exists.
                if let Some(l_other) = self.other_radial_loop(l_step) {
                    let u = self.ul(l_other);
                    if !u.in_rip_pairs && !u.in_stack {
                        stack.push(l_other);
                        let u = self.ul_mut(l_other);
                        u.in_stack = true;
                        u.side = 1 - side;
                    } else if u.side == side && !u.side_was_swapped {
                        self.set_side_and_remove(l_other, 1 - side, &mut stack, &mut pairs);
                    }
                }
                // The next loop along the edge on the same side.
                let l_other = self.s.next(l_step);
                self.visit_same_side(l_other, l_step, side, &mut stack, &mut pairs);
            }

            // The fan of loops around `l_step`, in both directions.
            for i in 0..2 {
                let l_radial_first = if i == 1 { l_step } else { self.s.prev(l_step) };
                if l_radial_first == self.s.radial_next(l_radial_first) {
                    continue;
                }
                let e_radial = self.s.le(l_radial_first);
                let mut it = self.s.radial_next(l_radial_first);
                loop {
                    if !self.ul(it).is_select_edge && self.tagged(it) {
                        let l_other = if self.s.lv(it) == self.s.lv(l_step) {
                            it
                        } else {
                            self.s.next(it)
                        };
                        if self.edge_share_vert(e_radial, l_other, l_step) {
                            self.visit_same_side(l_other, l_step, side, &mut stack, &mut pairs);
                        }
                    }
                    it = self.s.radial_next(it);
                    if it == l_radial_first {
                        break;
                    }
                }
            }
        }
        pairs
    }

    /// `uv_rip_pairs_calc_center_and_direction`: `(center, side directions, both sides)`.
    fn pairs_center_and_direction(&self, pairs: &[LoopId]) -> (Uv, [Uv; 2], bool) {
        let mut center = [0.0f32; 2];
        let mut side_total = [0i32; 2];
        let mut dir_side = [[0.0f32; 2]; 2];
        for &l in pairs {
            let side = self.ul(l).side as usize;
            let luv = self.s.uv(l);
            center[0] += luv[0];
            center[1] += luv[1];
            if !self.ul(l).is_select_edge {
                let d = sub(self.s.uv(self.s.next(l)), luv);
                dir_side[side][0] += d[0];
                dir_side[side][1] += d[1];
            }
            if !self.ul(self.s.prev(l)).is_select_edge {
                let d = sub(self.s.uv(self.s.prev(l)), luv);
                dir_side[side][0] += d[0];
                dir_side[side][1] += d[1];
            }
            side_total[side] += 1;
        }
        for d in &mut dir_side {
            normalize(d);
        }
        let f = 1.0 / pairs.len() as f32;
        center[0] *= f;
        center[1] *= f;
        (center, dir_side, side_total[0] != 0 && side_total[1] != 0)
    }
}

/// `uv_rip_object`: `Ok(changed)` or the error reported when nothing could be ripped.
pub(crate) fn rip_object(s: &mut UvSel, co: Uv, aspect_y: f32) -> Result<bool, RipError> {
    if s.ts.sync {
        s.prepare_sync_select();
    } else {
        s.prepare_custom_data();
    }
    let faces = s.faces();
    let mut ftag = vec![false; s.bm.faces.capacity()];
    for &f in &faces {
        ftag[f.slot()] = s.face_visible(f);
    }
    let ul = vec![Ul::default(); s.bm.loops.capacity()];
    let mut r = Rip {
        s,
        ul,
        ftag,
        aspect_y,
    };
    let mut changed = false;

    let mut is_select_all_any = false;
    for &f in &faces {
        if !r.ftag[f.slot()] {
            continue;
        }
        let loops = r.s.face_loops(f);
        let mut is_all = true;
        for &l in &loops {
            if r.s.loop_vert_select_get(l) {
                if r.s.loop_edge_select_get(l) {
                    r.ul_mut(l).is_select_edge = true;
                } else if !r.s.loop_edge_select_get(r.s.prev(l)) {
                    r.ul_mut(l).is_select_vert_single = true;
                    is_all = false;
                } else {
                    is_all = false;
                }
            } else {
                is_all = false;
            }
        }
        if is_all {
            for &l in &loops {
                r.ul_mut(l).is_select_all = true;
            }
            is_select_all_any = true;
        }
    }
    // Remove `is_select_vert_single` when connected to selected edges.
    for &f in &faces {
        if !r.ftag[f.slot()] {
            continue;
        }
        for l in r.s.face_loops(f) {
            if r.ul(l).is_select_vert_single {
                r.single_vert_validate(l);
            }
        }
    }

    // Selected faces: isolate them (not a rip, a quick way to detach the selection).
    if is_select_all_any {
        for &f in &faces {
            if !r.ftag[f.slot()] {
                continue;
            }
            for l in r.s.face_loops(f) {
                if !r.ul(l).is_select_all {
                    if r.s.loop_vert_select_get(l) {
                        r.s.loop_vert_select_set(l, false);
                        changed = true;
                    }
                    if r.s.loop_edge_select_get(l) {
                        r.s.loop_edge_select_set(l, false);
                        changed = true;
                    }
                }
            }
        }
        return Ok(changed);
    }

    let mut vert_selected = false;
    let mut edge_selected = false;
    for &f in &faces {
        if !r.ftag[f.slot()] {
            continue;
        }
        for l in r.s.face_loops(f) {
            if r.ul(l).is_select_edge {
                edge_selected = true;
                if !r.ul(l).in_rip_pairs {
                    let pairs = r.pairs_from_loop(l);
                    let (center, dir_side, both) = r.pairs_center_and_direction(&pairs);
                    let mut side_from_cursor = -1i32;
                    if both {
                        let mut dir_cursor = sub(center, co);
                        normalize(&mut dir_cursor);
                        side_from_cursor = i32::from(
                            dot(dir_side[0], dir_cursor) - dot(dir_side[1], dir_cursor) < 0.0,
                        );
                    }
                    for l_iter in pairs {
                        if i32::from(r.ul(l_iter).side) == side_from_cursor {
                            r.s.uv_select_disable(l_iter);
                            changed = true;
                        }
                        *r.ul_mut(l_iter) = Ul::default();
                    }
                }
            } else if r.ul(l).is_select_vert_single {
                vert_selected = true;
                let loops = r.single_from_loop(l, co);
                for l_iter in loops {
                    if r.ul(l_iter).side == 0 {
                        r.s.uv_select_disable(l_iter);
                        changed = true;
                    }
                    *r.ul_mut(l_iter) = Ul::default();
                }
            }
        }
    }
    if edge_selected && !changed {
        return Err(RipError::Edge);
    }
    if vert_selected && !changed {
        return Err(RipError::Vertex);
    }
    if changed {
        if r.s.ts.sync {
            r.s.bm_flush_from_loop_verts();
        } else {
            r.s.select_flush_from_verts(false);
        }
    }
    Ok(changed)
}
