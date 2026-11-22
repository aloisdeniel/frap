// SPDX-License-Identifier: GPL-3.0-or-later
//! `stitch_process_data`: which selected UVs (or UV edges) can be stitched, where they go
//! (the static island's UV, or the midpoint), the islands' translation and rotation, and
//! (with `final`) the new UVs, the UV selection and the cleared seams. The preview passes run
//! too (Blender runs them from `stitch_init_all` before the final one, and they leave flags
//! and the static island behind); their preview buffers are not built.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_smart_stitch.cc
//! (`stitch_process_data`, `determine_uv_stitchability`, `determine_uv_edge_stitchability`,
//! `stitch_validate_uv_stitchability`, `stitch_validate_edge_stitchability`,
//! `stitch_setup_face_preview_for_uv_group`, `stitch_propagate_uv_final_position`,
//! `stitch_island_calculate_edge_rotation`, `stitch_island_calculate_vert_rotation`,
//! `stitch_calculate_island_snapping`, `stitch_uv_rotate`, `stitch_check_*_stitchable`),
//! Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::ElemFlag;
use frap_uv::param::bli;

use super::state::{
    Mode, STITCH_BOUNDARY, STITCH_PROCESSED, STITCH_SELECTED, STITCH_STITCHABLE,
    STITCH_STITCHABLE_CANDIDATE, Ssc, StitchState,
};
use crate::select::{Uv, UvSel};

/// `IslandStitchData`.
#[derive(Copy, Clone, Debug, Default)]
struct Island {
    rotation: f32,
    rotation_neg: f32,
    translation: Uv,
    median_point: Uv,
    num_of_elements: i32,
    num_rot_elements: i32,
    num_rot_elements_neg: i32,
    added_for_preview: bool,
    stitchable_candidate: bool,
    use_edge_rotation: bool,
}

/// `UVVertAverage`.
#[derive(Copy, Clone, Debug, Default)]
struct Average {
    uv: Uv,
    count: u16,
}

/// `max_ff(-1, min_ff(1, x))` then `acosf`.
fn acos_clamped(x: f32) -> f32 {
    libm::acosf((-1.0f32).max(1.0f32.min(x)))
}

/// `stitch_uv_rotate`.
fn uv_rotate(mat: &[[f32; 2]; 2], median: Uv, uv: &mut Uv, aspect: f32) {
    uv[1] /= aspect;
    uv[0] -= median[0];
    uv[1] -= median[1];
    let r = bli::mul_m2_v2(mat, *uv);
    uv[0] = r[0] + median[0];
    uv[1] = r[1] + median[1];
    uv[1] *= aspect;
}

struct Pass<'a, 'b> {
    ssc: &'a mut Ssc,
    st: &'a mut StitchState,
    s: &'a mut UvSel<'b>,
    islands: Vec<Island>,
    is_active: bool,
}

impl Pass<'_, '_> {
    fn el_island(&self, e: u32) -> i32 {
        self.st.map.storage[e as usize].island as i32
    }
    fn el_uv(&self, e: u32) -> Uv {
        self.s.uv(self.st.map.storage[e as usize].l)
    }
    fn edge_island(&self, edge: u32) -> i32 {
        self.el_island(self.st.edges[edge as usize].element)
    }

    /// `stitch_check_uvs_stitchable`.
    fn uvs_stitchable(&self, element: u32, element_iter: u32) -> bool {
        if element_iter == element {
            return false;
        }
        if self.ssc.use_limit {
            let a = self.el_uv(element);
            let b = self.el_uv(element_iter);
            let limit = self.ssc.limit_dist;
            return (a[0] - b[0]).abs() < limit && (a[1] - b[1]).abs() < limit;
        }
        true
    }

    /// `stitch_check_edges_stitchable`.
    fn edges_stitchable(&self, edge: u32, edge_iter: u32) -> bool {
        if edge_iter == edge {
            return false;
        }
        if self.ssc.use_limit {
            let e = self.st.edges[edge as usize];
            let it = self.st.edges[edge_iter as usize];
            let uv = |u: u32| self.el_uv(self.st.uvs[u as usize]);
            let (o1, i1, o2, i2) = (uv(e.uv1), uv(it.uv1), uv(e.uv2), uv(it.uv2));
            let limit = self.ssc.limit_dist;
            return (o1[0] - i1[0]).abs() < limit
                && (o1[1] - i1[1]).abs() < limit
                && (o2[0] - i2[0]).abs() < limit
                && (o2[1] - i2[1]).abs() < limit;
        }
        true
    }

    /// `stitch_check_uvs_state_stitchable`.
    fn uvs_state_stitchable(&self, element: u32, element_iter: u32) -> bool {
        let same = self.el_island(element) == self.el_island(element_iter);
        if (self.ssc.snap_islands && same) || (!self.ssc.midpoints && same) {
            return false;
        }
        self.uvs_stitchable(element, element_iter)
    }

    /// `stitch_check_edges_state_stitchable`.
    fn edges_state_stitchable(&self, edge: u32, edge_iter: u32) -> bool {
        let same = self.edge_island(edge) == self.edge_island(edge_iter);
        if (self.ssc.snap_islands && same) || (!self.ssc.midpoints && same) {
            return false;
        }
        self.edges_stitchable(edge, edge_iter)
    }

    /// The elements of `element`'s vertex (`BM_uv_element_get_head` and `->next`).
    fn vertex_elements(&self, element: u32) -> Vec<u32> {
        self.st.map.iter_from(self.st.map.head(self.s, element))
    }

    /// `determine_uv_stitchability`.
    fn determine_uv(&mut self, element: u32) {
        for it in self.vertex_elements(element) {
            if self.st.map.storage[it as usize].separate && self.uvs_stitchable(element, it) {
                let (a, b) = (self.el_island(it), self.el_island(element));
                self.islands[a as usize].stitchable_candidate = true;
                self.islands[b as usize].stitchable_candidate = true;
                self.st.map.storage[element as usize].flag |= STITCH_STITCHABLE_CANDIDATE;
            }
        }
    }

    /// `determine_uv_edge_stitchability`.
    fn determine_edge(&mut self, edge: u32) {
        for it in self.st.edge_chain(edge) {
            if self.edges_stitchable(edge, it) {
                let (a, b) = (self.edge_island(it), self.edge_island(edge));
                self.islands[a as usize].stitchable_candidate = true;
                self.islands[b as usize].stitchable_candidate = true;
                self.st.edges[edge as usize].flag |= STITCH_STITCHABLE_CANDIDATE;
            }
        }
    }

    /// `stitch_setup_face_preview_for_uv_group` (only its island flag matters here).
    fn setup_preview_for_uv_group(&mut self, element: u32) {
        let island = self.el_island(element);
        if self.ssc.snap_islands && !self.ssc.midpoints && self.ssc.static_island == island {
            return;
        }
        if self.ssc.snap_islands {
            self.islands[island as usize].added_for_preview = true;
        }
    }

    /// `stitch_validate_uv_stitchability`.
    fn validate_uv(&mut self, element: u32) {
        if !self.is_active {
            return;
        }
        for it in self.vertex_elements(element) {
            if !self.st.map.storage[it as usize].separate || it == element {
                continue;
            }
            if self.uvs_state_stitchable(element, it)
                && (self.el_island(it) == self.ssc.static_island
                    || self.el_island(element) == self.ssc.static_island)
            {
                self.st.map.storage[element as usize].flag |= STITCH_STITCHABLE;
                self.setup_preview_for_uv_group(element);
                return;
            }
        }
    }

    /// `stitch_validate_edge_stitchability`.
    fn validate_edge(&mut self, edge: u32) {
        if !self.is_active {
            return;
        }
        for it in self.st.edge_chain(edge) {
            if it == edge {
                continue;
            }
            if self.edges_state_stitchable(edge, it)
                && (self.edge_island(it) == self.ssc.static_island
                    || self.edge_island(edge) == self.ssc.static_island)
            {
                self.st.edges[edge as usize].flag |= STITCH_STITCHABLE;
                let e = self.st.edges[edge as usize];
                self.setup_preview_for_uv_group(self.st.uvs[e.uv1 as usize]);
                self.setup_preview_for_uv_group(self.st.uvs[e.uv2 as usize]);
                return;
            }
        }
    }

    /// `stitch_propagate_uv_final_position`.
    fn propagate(&mut self, element: u32, index: usize, final_position: &[Average], final_: bool) {
        if self.st.map.storage[element as usize].flag & STITCH_STITCHABLE == 0 {
            return;
        }
        let mut it = Some(element);
        while let Some(e) = it {
            let l = self.st.map.storage[e as usize].l;
            self.st.map.storage[e as usize].flag |= STITCH_PROCESSED;
            if final_ {
                self.s.uv[l.slot()] = final_position[index].uv;
                self.s.uv_select_enable(l);
            }
            if !self.ssc.snap_islands
                || (!self.ssc.midpoints && self.el_island(e) == self.ssc.static_island)
            {
                self.st.map.storage[e as usize].flag &= STITCH_SELECTED;
            }
            it = self.st.map.storage[e as usize].next;
            if let Some(n) = it
                && self.st.map.storage[n as usize].separate
            {
                break;
            }
        }
    }

    /// `stitch_island_calculate_edge_rotation`.
    fn edge_rotation(&mut self, edge: u32, avg: &[Average], uvfinal_map: Option<&[u32]>) {
        let e = self.st.edges[edge as usize];
        let element1 = self.st.uvs[e.uv1 as usize];
        let element2 = self.st.uvs[e.uv2 as usize];
        let luv1 = self.el_uv(element1);
        let luv2 = self.el_uv(element2);
        let (index1, index2) = match uvfinal_map {
            Some(m) => (m[element1 as usize] as usize, m[element2 as usize] as usize),
            None => (e.uv1 as usize, e.uv2 as usize),
        };
        let aspect = self.st.aspect;
        let mut uv1 = [luv2[0] - luv1[0], luv2[1] - luv1[1]];
        uv1[1] /= aspect;
        let mut uv2 = [
            avg[index2].uv[0] - avg[index1].uv[0],
            avg[index2].uv[1] - avg[index1].uv[1],
        ];
        uv2[1] /= aspect;
        let (uv1, _) = bli::normalize2(uv1);
        let (uv2, _) = bli::normalize2(uv2);
        let edgecos = uv1[0] * uv2[0] + uv1[1] * uv2[1];
        let edgesin = uv1[0] * uv2[1] - uv1[1] * uv2[0];
        let rotation = acos_clamped(edgecos);
        let ii = self.el_island(element1) as usize;
        let isl = &mut self.islands[ii];
        if edgesin > 0.0 {
            isl.num_rot_elements += 1;
            isl.rotation += rotation;
        } else {
            isl.num_rot_elements_neg += 1;
            isl.rotation_neg += rotation;
        }
    }

    /// `stitch_island_calculate_vert_rotation`.
    fn vert_rotation(&mut self, element: u32) {
        let island = self.el_island(element);
        if island == self.ssc.static_island && !self.ssc.midpoints {
            return;
        }
        let (mut rotation, mut rotation_neg) = (0.0f32, 0.0f32);
        let (mut rot_elem, mut rot_elem_neg) = (0i32, 0i32);
        for it in self.vertex_elements(element) {
            if !self.st.map.storage[it as usize].separate || !self.uvs_state_stitchable(element, it)
            {
                continue;
            }
            if !self.ssc.midpoints && self.el_island(it) != self.ssc.static_island {
                continue;
            }
            let n1 = self.st.normals[self.st.umap[it as usize] as usize];
            let n2 = self.st.normals[self.st.umap[element as usize] as usize];
            let normal = [-n2[0], -n2[1]];
            let edgecos = normal[0] * n1[0] + normal[1] * n1[1];
            let edgesin = normal[0] * n1[1] - normal[1] * n1[0];
            if edgesin > 0.0 {
                rotation += acos_clamped(edgecos);
                rot_elem += 1;
            } else {
                rotation_neg += acos_clamped(edgecos);
                rot_elem_neg += 1;
            }
        }
        if self.ssc.midpoints {
            rotation /= 2.0;
            rotation_neg /= 2.0;
        }
        let isl = &mut self.islands[island as usize];
        isl.num_rot_elements += rot_elem;
        isl.rotation += rotation;
        isl.num_rot_elements_neg += rot_elem_neg;
        isl.rotation_neg += rotation_neg;
    }

    /// `stitch_calculate_island_snapping`.
    fn island_snapping(&mut self, final_: bool) {
        let aspect = self.st.aspect;
        for i in 0..self.st.map.total_islands {
            if !self.islands[i].added_for_preview {
                continue;
            }
            let isl = &mut self.islands[i];
            let totelem = isl.num_rot_elements_neg + isl.num_rot_elements;
            if isl.num_rot_elements > 1 {
                isl.rotation /= isl.num_rot_elements as f32;
            }
            if isl.num_rot_elements_neg > 1 {
                isl.rotation_neg /= isl.num_rot_elements_neg as f32;
            }
            if isl.num_of_elements > 1 {
                let n = isl.num_of_elements as f32;
                isl.median_point[0] /= n;
                isl.median_point[1] /= n;
                isl.translation[0] /= n;
                isl.translation[1] /= n;
            }
            isl.median_point[1] /= aspect;
            let rotation = if isl.rotation + isl.rotation_neg < core::f32::consts::FRAC_PI_2
                || isl.num_rot_elements == 0
                || isl.num_rot_elements_neg == 0
            {
                (isl.rotation * isl.num_rot_elements as f32
                    - isl.rotation_neg * isl.num_rot_elements_neg as f32)
                    / totelem as f32
            } else {
                (isl.rotation * isl.num_rot_elements as f32
                    + (2.0 * core::f32::consts::PI - isl.rotation_neg)
                        * isl.num_rot_elements_neg as f32)
                    / totelem as f32
            };
            let mat = bli::angle_to_mat2(rotation);
            let (median, translation) = (isl.median_point, isl.translation);
            for e in self.st.map.island(i) {
                let el = self.st.map.storage[e];
                if el.flag & STITCH_PROCESSED == 0 && final_ {
                    let luv = &mut self.s.uv[el.l.slot()];
                    uv_rotate(&mat, median, luv, aspect);
                    luv[0] += translation[0];
                    luv[1] += translation[1];
                }
                self.st.map.storage[e].flag &= STITCH_SELECTED;
            }
        }
    }
}

/// `stitch_process_data` for one mesh. `is_active`: `state == ssc->states[active_object_index]`.
pub(crate) fn process_data(
    ssc: &mut Ssc,
    st: &mut StitchState,
    s: &mut UvSel,
    is_active: bool,
    final_: bool,
) {
    let total_islands = st.map.total_islands;
    let previous_island = ssc.static_island;
    let mut p = Pass {
        ssc,
        st,
        s,
        islands: vec![Island::default(); total_islands],
        is_active,
    };
    let mode = p.ssc.mode;
    let selection = p.st.selection.clone();

    // First determine the stitchability of the UVs.
    for &sel in &selection {
        match mode {
            Mode::Vert => p.determine_uv(sel),
            Mode::Edge => p.determine_edge(sel),
        }
    }
    for i in 0..total_islands {
        p.st.island_is_stitchable[i] = p.islands[i].stitchable_candidate;
    }
    if is_active {
        // Set the static island to one that is added for the preview.
        let n = total_islands as i32;
        p.ssc.static_island %= n;
        while !p.islands[p.ssc.static_island as usize].stitchable_candidate {
            p.ssc.static_island = (p.ssc.static_island + 1) % n;
            if p.ssc.static_island == previous_island {
                break;
            }
        }
    }
    for &sel in &selection {
        match mode {
            Mode::Vert => {
                let flag = &mut p.st.map.storage[sel as usize].flag;
                if *flag & STITCH_STITCHABLE_CANDIDATE != 0 {
                    *flag &= !STITCH_STITCHABLE_CANDIDATE;
                    p.validate_uv(sel);
                }
            }
            Mode::Edge => {
                let flag = &mut p.st.edges[sel as usize].flag;
                if *flag & STITCH_STITCHABLE_CANDIDATE != 0 {
                    *flag &= !STITCH_STITCHABLE_CANDIDATE;
                    p.validate_edge(sel);
                }
            }
        }
    }
    if !is_active {
        return;
    }

    // The final coordinates of the UVs.
    let snap = p.ssc.snap_islands;
    let midpoints = p.ssc.midpoints;
    let static_island = p.ssc.static_island;
    let (mut final_position, mut uvfinal_map) = match mode {
        Mode::Vert => (
            vec![Average::default(); selection.len()],
            vec![0u32; p.st.map.storage.len()],
        ),
        Mode::Edge => (vec![Average::default(); p.st.uvs.len()], Vec::new()),
    };

    // First pass: final positions of the stitchable UVs.
    for (i, &sel) in selection.iter().enumerate() {
        match mode {
            Mode::Vert => {
                let element = sel;
                if p.st.map.storage[element as usize].flag & STITCH_STITCHABLE != 0 {
                    let luv = p.el_uv(element);
                    uvfinal_map[element as usize] = i as u32;
                    final_position[i] = Average { uv: luv, count: 1 };
                    if snap && p.el_island(element) == static_island && !midpoints {
                        continue;
                    }
                    let vi = p.st.map.vidx[p.s.lv(p.st.map.storage[element as usize].l).slot()];
                    for it in p.st.map.iter_from(p.st.map.vertex[vi as usize]) {
                        if p.st.map.storage[it as usize].separate
                            && p.uvs_state_stitchable(element, it)
                        {
                            let luv = p.el_uv(it);
                            if midpoints {
                                let f = &mut final_position[i];
                                f.uv[0] += luv[0];
                                f.uv[1] += luv[1];
                                f.count += 1;
                            } else if p.el_island(it) == static_island {
                                final_position[i].uv = luv;
                            }
                        }
                    }
                }
                if midpoints {
                    let f = &mut final_position[i];
                    f.uv[0] /= f32::from(f.count);
                    f.uv[1] /= f32::from(f.count);
                }
            }
            Mode::Edge => {
                let edge = sel;
                let e = p.st.edges[edge as usize];
                if e.flag & STITCH_STITCHABLE == 0 {
                    continue;
                }
                let luv1 = p.el_uv(p.st.uvs[e.uv1 as usize]);
                let luv2 = p.el_uv(p.st.uvs[e.uv2 as usize]);
                final_position[e.uv1 as usize] = Average { uv: luv1, count: 1 };
                final_position[e.uv2 as usize] = Average { uv: luv2, count: 1 };
                let (u1, u2) = (p.st.uvs[e.uv1 as usize], p.st.uvs[e.uv2 as usize]);
                p.st.map.storage[u1 as usize].flag |= STITCH_STITCHABLE;
                p.st.map.storage[u2 as usize].flag |= STITCH_STITCHABLE;
                if snap && p.edge_island(edge) == static_island && !midpoints {
                    continue;
                }
                for it in p.st.edge_chain(edge) {
                    if p.edges_state_stitchable(edge, it) {
                        let ie = p.st.edges[it as usize];
                        let luv1 = p.el_uv(p.st.uvs[ie.uv1 as usize]);
                        let luv2 = p.el_uv(p.st.uvs[ie.uv2 as usize]);
                        if midpoints {
                            let f1 = &mut final_position[e.uv1 as usize];
                            f1.uv[0] += luv1[0];
                            f1.uv[1] += luv1[1];
                            f1.count += 1;
                            let f2 = &mut final_position[e.uv2 as usize];
                            f2.uv[0] += luv2[0];
                            f2.uv[1] += luv2[1];
                            f2.count += 1;
                        } else if p.edge_island(it) == static_island {
                            final_position[e.uv1 as usize].uv = luv1;
                            final_position[e.uv2 as usize].uv = luv2;
                        }
                    }
                }
            }
        }
    }
    // The mean position (edge mode: after the loop, for shared UVs).
    if mode == Mode::Edge && midpoints {
        for f in &mut final_position {
            f.uv[0] /= f32::from(f.count);
            f.uv[1] /= f32::from(f.count);
        }
    }

    // Second pass: island rotation and translation before modifying any UV.
    if snap {
        match mode {
            Mode::Vert => {
                for (i, &element) in selection.iter().enumerate() {
                    if p.st.map.storage[element as usize].flag & STITCH_STITCHABLE != 0 {
                        let luv = p.el_uv(element);
                        let ii = p.el_island(element) as usize;
                        let isl = &mut p.islands[ii];
                        isl.translation[0] += final_position[i].uv[0] - luv[0];
                        isl.translation[1] += final_position[i].uv[1] - luv[1];
                        isl.median_point[0] += luv[0];
                        isl.median_point[1] += luv[1];
                        isl.num_of_elements += 1;
                    }
                }
                // Only calculate rotation when an edge has been fully selected.
                for i in 0..p.st.edges.len() {
                    let e = p.st.edges[i];
                    let (u1, u2) = (p.st.uvs[e.uv1 as usize], p.st.uvs[e.uv2 as usize]);
                    if e.flag & STITCH_BOUNDARY != 0
                        && p.st.map.storage[u1 as usize].flag & STITCH_STITCHABLE != 0
                        && p.st.map.storage[u2 as usize].flag & STITCH_STITCHABLE != 0
                    {
                        p.edge_rotation(i as u32, &final_position, Some(&uvfinal_map));
                        let isl = p.el_island(u1) as usize;
                        p.islands[isl].use_edge_rotation = true;
                    }
                }
                if final_ && p.ssc.clear_seams {
                    for i in 0..p.st.edges.len() {
                        let e = p.st.edges[i];
                        let (u1, u2) = (p.st.uvs[e.uv1 as usize], p.st.uvs[e.uv2 as usize]);
                        if p.st.map.storage[u1 as usize].flag & STITCH_STITCHABLE != 0
                            && p.st.map.storage[u2 as usize].flag & STITCH_STITCHABLE != 0
                        {
                            clear_seam(&mut p, i as u32);
                        }
                    }
                }
                for &element in &selection {
                    if !p.islands[p.el_island(element) as usize].use_edge_rotation
                        && p.st.map.storage[element as usize].flag & STITCH_STITCHABLE != 0
                    {
                        p.vert_rotation(element);
                    }
                }
            }
            Mode::Edge => {
                for (i, fp) in final_position.iter().enumerate() {
                    let element = p.st.uvs[i];
                    if p.st.map.storage[element as usize].flag & STITCH_STITCHABLE != 0 {
                        let luv = p.el_uv(element);
                        let ii = p.el_island(element) as usize;
                        let isl = &mut p.islands[ii];
                        isl.translation[0] += fp.uv[0] - luv[0];
                        isl.translation[1] += fp.uv[1] - luv[1];
                        isl.median_point[0] += luv[0];
                        isl.median_point[1] += luv[1];
                        isl.num_of_elements += 1;
                    }
                }
                for &edge in &selection {
                    if p.st.edges[edge as usize].flag & STITCH_STITCHABLE != 0 {
                        p.edge_rotation(edge, &final_position, None);
                        let u1 = p.st.uvs[p.st.edges[edge as usize].uv1 as usize];
                        let isl = p.el_island(u1) as usize;
                        p.islands[isl].use_edge_rotation = true;
                    }
                }
                if final_ && p.ssc.clear_seams {
                    for &edge in &selection {
                        if p.st.edges[edge as usize].flag & STITCH_STITCHABLE != 0 {
                            clear_seam(&mut p, edge);
                        }
                    }
                }
            }
        }
    }

    // Third pass: propagate the changes to the coincident UVs.
    for (i, &sel) in selection.iter().enumerate() {
        match mode {
            Mode::Vert => p.propagate(sel, i, &final_position, final_),
            Mode::Edge => {
                let e = p.st.edges[sel as usize];
                p.propagate(p.st.uvs[e.uv1 as usize], e.uv1 as usize, &final_position, final_);
                p.propagate(p.st.uvs[e.uv2 as usize], e.uv2 as usize, &final_position, final_);
                p.st.edges[sel as usize].flag &= STITCH_SELECTED | STITCH_BOUNDARY;
            }
        }
    }

    // Final pass: the island translation and rotation.
    if snap {
        p.island_snapping(final_);
    }
}

/// `BM_elem_flag_disable(edge->element->l->e, BM_ELEM_SEAM)`.
fn clear_seam(p: &mut Pass, edge: u32) {
    let l = p.st.map.storage[p.st.edges[edge as usize].element as usize].l;
    let e = p.s.le(l);
    p.s.bm.edge_mut(e).flag.remove(ElemFlag::SEAM);
}
