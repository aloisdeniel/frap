// SPDX-License-Identifier: GPL-3.0-or-later
//! UV handling: connected face components in UV space (`math_layer_info_init`), UV vertex
//! buckets (`determine_uv_vert_connectivity`, `update_uv_vert_map`) and the final UV merge
//! (`bevel_merge_uvs`). Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc
//! (Blender 5.2.4, GPL-2.0-or-later).
//!
//! Blender keeps the buckets in hash sets and maps; frap keeps them in insertion order and
//! a `BTreeMap` by vertex, which only changes the order of the float sums of the UV average.

use std::sync::Arc;

use frap_mesh::AttrData;
use rustc_hash::FxHashMap;

use super::types::Bevel;
use super::util::{contig_ldata_across_edge, face_calc_center_bounds};
use crate::types::{BMElem, BMesh, ElemTypes, FaceId, LoopId, VertId};

/// `STD_UV_CONNECT_LIMIT`.
const STD_UV_CONNECT_LIMIT: f32 = 0.0001;

/// The positions of the `FLOAT2` loop layers (`CustomData_get_n_offset(ldata, FLOAT2, i)`).
fn uv_layers(bm: &BMesh) -> Vec<usize> {
    bm.ldata
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l.data, AttrData::Float2(_)))
        .map(|(i, _)| i)
        .collect()
}

fn uv_at(bm: &BMesh, layer: usize, l: LoopId) -> [f32; 2] {
    match &bm.ldata.layers[layer].data {
        AttrData::Float2(c) => c[l.slot()],
        _ => [0.0; 2],
    }
}

impl Bevel {
    /// `math_layer_info_init`: whether there are UV layers and, for an odd segment count, the
    /// connected component (in UV space) of every face.
    pub(crate) fn math_layer_info_init(&mut self, bm: &mut BMesh) {
        self.math_layer_info.has_math_layers = !uv_layers(bm).is_empty();
        self.math_layer_info.face_component = None;
        if !self.math_layer_info.has_math_layers || self.seg % 2 == 0 {
            return;
        }
        bm.elem_index_ensure(ElemTypes::FACE);
        let faces = bm.face_ids();
        let totface = faces.len();
        let mut face_component = vec![-1i32; totface];
        let mut in_stack = vec![false; totface];
        let mut stack: Vec<FaceId> = Vec::with_capacity(totface);
        // Set all component ids by DFS from faces with unassigned components.
        let mut current_component = -1;
        for f in 0..totface {
            if face_component[f] != -1 || in_stack[f] {
                continue;
            }
            current_component += 1;
            stack.clear();
            stack.push(faces[f]);
            in_stack[f] = true;
            while let Some(bmf) = stack.pop() {
                let bmf_index = bm.index_of(BMElem::Face(bmf));
                in_stack[bmf_index] = false;
                if face_component[bmf_index] != -1 {
                    continue;
                }
                face_component[bmf_index] = current_component;
                // Neighbours share an edge with bmf and are UV-contiguous across it.
                for bme in bm.face_edges(bmf) {
                    for bmf_other in bm.edge_faces(bme) {
                        if bmf_other == bmf {
                            continue;
                        }
                        let oi = bm.index_of(BMElem::Face(bmf_other));
                        if face_component[oi] != -1 || in_stack[oi] {
                            continue;
                        }
                        if contig_ldata_across_edge(bm, bme, bmf, bmf_other) {
                            stack.push(bmf_other);
                            in_stack[oi] = true;
                        }
                    }
                }
            }
        }
        // Components 0 and 1 should be the topmost and bottom-most (in z) components.
        if current_component > 0 {
            let mut top_face_z = -1e30f32;
            let mut top_face_component = -1;
            let mut bot_face_z = 1e30f32;
            let mut bot_face_component = -1;
            for (f, &bmf) in faces.iter().enumerate() {
                let fz = face_calc_center_bounds(bm, bmf)[2];
                if fz > top_face_z {
                    top_face_z = fz;
                    top_face_component = face_component[f];
                }
                if fz < bot_face_z {
                    bot_face_z = fz;
                    bot_face_component = face_component[f];
                }
            }
            let c0 = face_component[0];
            swap_face_components(&mut face_component, c0, top_face_component);
            if bot_face_component != top_face_component {
                if bot_face_component == 0 {
                    // It was swapped with the old top_face_component.
                    bot_face_component = top_face_component;
                }
                let c1 = face_component[1];
                swap_face_components(&mut face_component, c1, bot_face_component);
            }
        }
        self.math_layer_info.face_component = Some(face_component);
    }

    /// `uv_vert_map_init`.
    pub(crate) fn uv_vert_map_init(&mut self, bm: &BMesh) {
        self.uv_vert_maps = vec![Default::default(); uv_layers(bm).len()];
    }

    /// `uv_vert_map_pop`: forget vertex `v` in every UV map.
    pub(crate) fn uv_vert_map_pop(&mut self, v: VertId) {
        for m in &mut self.uv_vert_maps {
            m.remove(&v);
        }
    }

    /// `determine_uv_vert_connectivity`: buckets of the loops of `v` whose UVs overlap.
    pub(crate) fn determine_uv_vert_connectivity(&mut self, bm: &BMesh, v: VertId) {
        let layers = uv_layers(bm);
        debug_assert_eq!(layers.len(), self.uv_vert_maps.len());
        for (i, &layer) in layers.iter().enumerate() {
            let mut buckets: Vec<Vec<LoopId>> = Vec::new();
            for l in bm.vert_loops(v) {
                let luv = uv_at(bm, layer, l);
                let found = buckets.iter_mut().find(|b| {
                    b.iter().any(|&l2| {
                        let luv2 = uv_at(bm, layer, l2);
                        (luv[0] - luv2[0]).abs() <= STD_UV_CONNECT_LIMIT
                            && (luv[1] - luv2[1]).abs() <= STD_UV_CONNECT_LIMIT
                    })
                });
                match found {
                    Some(b) => {
                        if !b.contains(&l) {
                            b.push(l);
                        }
                    }
                    None => buckets.push(vec![l]),
                }
            }
            debug_assert!(!self.uv_vert_maps[i].contains_key(&v));
            self.uv_vert_maps[i].insert(v, buckets);
        }
    }

    /// `register_uv_face` + `update_uv_vert_map`: record the new face `f` with its
    /// representative face and add its loops to the UV vertex buckets.
    pub(crate) fn register_and_update_uv(
        &mut self,
        bm: &BMesh,
        f: FaceId,
        attached_frep: Option<FaceId>,
        bv: Option<VertId>,
        nv_bv_map: Option<&FxHashMap<VertId, VertId>>,
    ) {
        self.uv_face_hash.insert(f, attached_frep);
        let Some(attached) = attached_frep else {
            return;
        };
        let loops = bm.face_loops(f);
        for mi in 0..self.uv_vert_maps.len() {
            for &l in &loops {
                let lv = bm.lp(l).v;
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    self.uv_vert_maps[mi].entry(lv)
                {
                    // A new vertex: register it with a new bucket.
                    slot.insert(vec![vec![l]]);
                    continue;
                }
                // orig_v is the vertex of the original mesh taking part in the bevel.
                let orig_v = match nv_bv_map {
                    Some(m) => m.get(&lv).copied(),
                    None => bv,
                };
                let orig_l = orig_v.and_then(|ov| bm.face_vert_share_loop(attached, ov));
                debug_assert!(orig_l.is_some());
                let mut target: Option<LoopId> = None;
                for l2 in bm.vert_loops(lv) {
                    if l2 == l {
                        continue;
                    }
                    let Some(&Some(attached2)) = self.uv_face_hash.get(&bm.lp(l2).f) else {
                        continue;
                    };
                    let orig_l2 = orig_v.and_then(|ov| bm.face_vert_share_loop(attached2, ov));
                    let connected = match (orig_v, orig_l, orig_l2) {
                        (Some(ov), Some(a), Some(b)) => {
                            self.uv_vert_maps[mi].get(&ov).is_some_and(|bs| {
                                bs.iter().any(|bk| bk.contains(&a) && bk.contains(&b))
                            })
                        }
                        _ => false,
                    };
                    // Join the bucket of l2 when both faces have the same representative, or
                    // their representatives were UV-connected at the original vertex.
                    if attached == attached2 || connected {
                        let has = self.uv_vert_maps[mi][&lv].iter().any(|bk| bk.contains(&l2));
                        if has {
                            target = Some(l2);
                            break;
                        }
                    }
                }
                let buckets = self.uv_vert_maps[mi].get_mut(&lv).expect("vertex buckets");
                match target {
                    Some(l2) => {
                        let bk = buckets
                            .iter_mut()
                            .find(|bk| bk.contains(&l2))
                            .expect("bucket of l2");
                        if !bk.contains(&l) {
                            bk.push(l);
                        }
                    }
                    None => buckets.push(vec![l]),
                }
            }
        }
    }

    /// `bevel_merge_uvs`: average the UVs of the loops in each bucket.
    pub(crate) fn bevel_merge_uvs(&self, bm: &mut BMesh) {
        let layers = uv_layers(bm);
        for (i, &layer) in layers.iter().enumerate() {
            let AttrData::Float2(col) = &mut bm.ldata.layers[layer].data else {
                continue;
            };
            let col = Arc::make_mut(col);
            for buckets in self.uv_vert_maps[i].values() {
                for bucket in buckets {
                    let n = bucket.len();
                    if n <= 1 {
                        continue;
                    }
                    let mut uv = [0.0f32; 2];
                    for &l in bucket {
                        let luv = col[l.slot()];
                        uv[0] += luv[0];
                        uv[1] += luv[1];
                    }
                    let f = 1.0 / n as f32;
                    uv = [uv[0] * f, uv[1] * f];
                    for &l in bucket {
                        col[l.slot()] = uv;
                    }
                }
            }
        }
    }
}

/// `swap_face_components`: swap the values `c1` and `c2` wherever they occur.
fn swap_face_components(face_component: &mut [i32], c1: i32, c2: i32) {
    if c1 == c2 {
        return;
    }
    for c in face_component.iter_mut() {
        if *c == c1 {
            *c = c2;
        } else if *c == c2 {
            *c = c1;
        }
    }
}
