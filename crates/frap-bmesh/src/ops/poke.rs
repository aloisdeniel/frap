// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.poke` (Blender 5.2.4 `bmo_poke_exec`, `operators/bmo_poke.cc`), implemented by
//! WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.poke(bmesh, faces=[], offset=0.0, center_mode='MEAN_WEIGHTED', use_relative_offset=False)`.
//!
//! **Create/kill order.** For each input face, in input order: the centre vertex is created,
//! then one triangle per corner from `l_first` (`(l.v, l.next.v, centre)`, with
//! `BM_face_create_verts`: the spoke edge from the centre to `l.next.v` is created before the
//! triangle, the first triangle also creates the spoke to `l_first.v`), and finally the face
//! is killed (`BM_face_kill`), freeing its slot for the next face's first triangle. The
//! centre's loop data is interpolated once from the face (`BM_loop_interp_from_face`, with
//! vertex data) and copied to the other triangles; the centre gets the face normal and is
//! moved by `offset` (times the mean centre–corner distance with `use_relative_offset`).
//! Default cube top face: centre vertex 8, triangles `[7,3,8] [3,1,8] [1,5,8] [5,7,8]`, the
//! first in face slot 6 then compacted to 5 ([MESH §4.13]).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use super::triangulate::math;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, FaceId, VertId};

/// Input slots of `poke` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct PokeIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `offset`.
    pub offset: f32,
    /// `center_mode` (enum identifier: `MEAN_WEIGHTED`, `MEAN`, `BOUNDS`).
    pub center_mode: &'static str,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
}

impl Default for PokeIn {
    fn default() -> Self {
        PokeIn {
            faces: Vec::new(),
            offset: 0.0,
            center_mode: "MEAN_WEIGHTED",
            use_relative_offset: false,
        }
    }
}

/// Output slots of `poke`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PokeOut {
    /// `verts.out` then `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `verts.out`: the centre vertices, vertex order.
    pub verts: Vec<VertId>,
    /// `faces.out`: the new triangles, face order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_poke_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const ELE_NEW: u16 = 1;

/// Runs `poke` on `bm`.
pub fn poke(bm: &mut BMesh, i: &PokeIn) -> Result<PokeOut, BmError> {
    let center: fn(&BMesh, FaceId) -> [f32; 3] = match i.center_mode {
        "MEAN_WEIGHTED" => math::face_calc_center_median_weighted,
        "BOUNDS" => math::face_calc_center_bounds,
        "MEAN" => math::face_calc_center_median,
        other => {
            return Err(BmError::Invalid(format!(
                "poke: unknown center_mode '{other}' (expected MEAN_WEIGHTED, MEAN or BOUNDS)"
            )));
        }
    };
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "poke: dead face {dead:?} in faces"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        for &f in &i.faces {
            if !bm.faces.contains(f.0) {
                // Listed twice: Blender would read freed memory; skip it.
                continue;
            }
            frap_math::budget::check();
            let f_center = center(bm, f);
            let v_center = bm.vert_create(f_center, None, CreateFlag::empty());
            bm.oflag_enable(v_center, ELE_NEW);
            let mut offset_fac = if i.use_relative_offset { 0.0f32 } else { 1.0 };
            let loops = bm.face_loops(f);
            let mut l_center_example = None;
            for &l_iter in &loops {
                let (v1, l_next) = (bm.lp(l_iter).v, bm.lp(l_iter).next);
                let v2 = bm.lp(l_next).v;
                let f_new = bm
                    .face_create_verts(&[v1, v2, v_center], Some(f), CreateFlag::empty(), true)
                    .expect("invariant: a fan triangle over distinct vertices");
                let l_new = bm.face(f_new).l_first;
                let l_new_prev = bm.lp(l_new).prev;
                let l_new_next = bm.lp(l_new).next;
                match l_center_example {
                    None => {
                        bm.loop_interp_from_face(l_new_prev, f, true);
                        l_center_example = Some(l_new_prev);
                    }
                    Some(ex) => bm.loop_attrs_copy(ex, l_new_prev),
                }
                // Copy loop data.
                bm.loop_attrs_copy(l_iter, l_new);
                bm.loop_attrs_copy(l_next, l_new_next);
                bm.oflag_enable(f_new, ELE_NEW);
                if i.use_relative_offset {
                    offset_fac += math::len_v3v3(f_center, bm.vert(v1).co);
                }
            }
            if i.use_relative_offset {
                offset_fac /= bm.face(f).len as f32;
            }
            let fno = bm.face(f).no;
            bm.vert_mut(v_center).no = fno;
            let co = bm.vert(v_center).co;
            bm.vert_mut(v_center).co = math::madd(co, fno, i.offset * offset_fac);
            bm.face_kill(f);
        }
        let mut out = PokeOut::default();
        for v in bm.vert_ids() {
            if bm.oflag_test(v, ELE_NEW) {
                out.verts.push(v);
                out.geom.push(BMElem::Vert(v));
            }
        }
        for f in bm.face_ids() {
            if bm.oflag_test(f, ELE_NEW) {
                out.faces.push(f);
                out.geom.push(BMElem::Face(f));
            }
        }
        out
    }))
}
