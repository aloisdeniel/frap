// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.spin` (Blender 5.2.4 `bmo_spin_exec`, `operators/bmo_dupe.cc`), implemented by
//! WP 2.10. Uses `extrude_face_region` (WP 2.5) and `duplicate` (WP 2.1).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.spin(bmesh, geom=[], cent=Vector(), axis=Vector(), dvec=Vector(), angle=0.0, space=Matrix(), steps=0, use_merge=False, use_normal_flip=False, use_duplicate=False)`.
//!
//! **Create/kill order.** `steps` times: with `use_duplicate` a nested `duplicate` of `geom`
//! (its copy is rotated by `angle · (a+1)/steps` about `axis` through `cent`); otherwise a
//! nested `extrude_face_region` of the previous step's result (`geom_last`, keeping the
//! original geometry on the merge path), whose new vertices are first put back at their
//! un-rotated origin and then rotated by the step's total angle (one fresh rotation per step,
//! no accumulated error). With `use_merge` (only when `steps >= 3` and not duplicating) the
//! last step instead splices its vertices onto the original ones (`BM_vert_splice`), then its
//! edges onto their doubles (`BM_edge_splice`) and kills faces that double an existing one.
//! With a non-zero `dvec` the step's result is also translated by `rmat · dvec · (a+1)`.
//! Rotations and translations happen in `space` (`space⁻¹ · M · space`). The original vertex
//! of every extruded vertex is tracked as Blender does, through its index stored in the
//! bits of `no[0]` (copied into every extruded vertex); normals are recalculated at the end.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use super::triangulate::math::{self, M4};
use crate::IDENTITY;
use crate::ops::duplicate::{DuplicateIn, duplicate};
use crate::ops::extrude_face_region::{ExtrudeFaceRegionIn, extrude_face_region};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `spin` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SpinIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `cent`.
    pub cent: [f32; 3],
    /// `axis`.
    pub axis: [f32; 3],
    /// `dvec`.
    pub dvec: [f32; 3],
    /// `angle`.
    pub angle: f32,
    /// `space` (Blender layout `m[col][row]`).
    pub space: [[f32; 4]; 4],
    /// `steps`.
    pub steps: i32,
    /// `use_merge`.
    pub use_merge: bool,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_duplicate`.
    pub use_duplicate: bool,
}

impl Default for SpinIn {
    fn default() -> Self {
        SpinIn {
            geom: Vec::new(),
            cent: [0.0; 3],
            axis: [0.0; 3],
            dvec: [0.0; 3],
            angle: 0.0,
            space: IDENTITY,
            steps: 0,
            use_merge: false,
            use_normal_flip: false,
            use_duplicate: false,
        }
    }
}

/// Output slots of `spin`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SpinOut {
    /// `geom_last.out` (same as [`SpinOut::geom_last`]).
    pub geom: Vec<BMElem>,
    /// `geom_last.out`: the result of the last step.
    pub geom_last: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_spin_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

fn verts_of(geom: &[BMElem]) -> Vec<VertId> {
    geom.iter()
        .filter_map(|g| match g {
            BMElem::Vert(v) => Some(*v),
            _ => None,
        })
        .collect()
}

/// `BM_edge_find_double`.
fn edge_find_double(bm: &BMesh, e: EdgeId) -> Option<EdgeId> {
    let [v, v_other] = bm.edge(e).v;
    let mut e_iter = bm.disk_edge_next(e, v);
    while e_iter != e {
        if bm.vert_in_edge(e_iter, v_other) {
            return Some(e_iter);
        }
        e_iter = bm.disk_edge_next(e_iter, v);
    }
    None
}

/// Runs `spin` on `bm`.
pub fn spin(bm: &mut BMesh, i: &SpinIn) -> Result<SpinOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "spin: dead element {dead:?} in geom"
        )));
    }
    let cent = i.cent;
    let axis = math::normalize_v3(i.axis).0;
    let dvec = i.dvec;
    let use_dvec = !math::is_zero_v3(dvec);
    let steps = i.steps.max(0);
    let angle_total = i.angle;
    let do_dupli = i.use_duplicate;
    let use_normal_flip = i.use_normal_flip;
    let use_merge = i.use_merge && steps >= 3 && !do_dupli;
    let space: M4 = i.space;

    bm.bmo_exec(TYPE_FLAG, |bm| {
        // When merging, store the original vertices to splice them back together; when
        // extruding, always restore the original location before rotating. Every vertex's
        // index goes into the bits of `no[0]` ("Evil!" in Blender) so extruded copies carry it.
        let all = bm.vert_ids();
        let vtable: Vec<VertId> = if use_merge { all.clone() } else { Vec::new() };
        let mut vtable_coords: Vec<[f32; 3]> = Vec::new();
        if use_merge || !do_dupli {
            for (idx, &v) in all.iter().enumerate() {
                if !do_dupli {
                    vtable_coords.push(bm.vert(v).co);
                }
                bm.vert_mut(v).no[0] = f32::from_bits(idx as u32);
            }
        }
        let orig_index = |bm: &BMesh, v: VertId| bm.vert(v).no[0].to_bits() as usize;

        let mut geom_last = i.geom.clone();
        for a in 0..steps {
            frap_math::budget::check();
            // The step's rotation, computed independently to avoid accumulating error.
            let step_angle = angle_total * ((a + 1) as f32 / steps as f32);
            let rmat = math::axis_angle_normalized_to_mat3(axis, step_angle);
            let rot = math::transform_pivot_set_m4(&math::m4_from_m3(&rmat), cent);
            if do_dupli {
                let dup = duplicate(
                    bm,
                    &DuplicateIn {
                        geom: i.geom.clone(),
                        ..DuplicateIn::default()
                    },
                )?;
                math::bmo_transform(bm, &verts_of(&dup.geom), &rot, &space, false);
                geom_last = dup.geom;
            } else {
                let ext = extrude_face_region(
                    bm,
                    &ExtrudeFaceRegionIn {
                        geom: geom_last.clone(),
                        use_keep_orig: use_merge,
                        use_normal_flip: use_normal_flip && a == 0,
                        use_normal_from_adjacent: a != 0,
                        skip_input_flip: true,
                        ..ExtrudeFaceRegionIn::default()
                    },
                )?;
                let mut elems = ext.geom;
                if !(use_merge && a == steps - 1) {
                    if a != 0 {
                        // Reset each new vertex to its un-rotated origin.
                        for v in verts_of(&elems) {
                            let idx = orig_index(bm, v);
                            if let Some(&co) = vtable_coords.get(idx) {
                                bm.vert_mut(v).co = co;
                            }
                        }
                    }
                    math::bmo_transform(bm, &verts_of(&elems), &rot, &space, false);
                    geom_last = elems;
                } else {
                    // Merge the first and last vertices and edges (keeping `geom.out`).
                    let mut k = 0;
                    while k < elems.len() {
                        if let BMElem::Vert(v_src) = elems[k] {
                            let v_dst = vtable[orig_index(bm, v_src)];
                            bm.vert_splice(v_dst, v_src);
                            elems.swap_remove(k);
                        } else {
                            k += 1;
                        }
                    }
                    let mut k = 0;
                    while k < elems.len() {
                        if let BMElem::Edge(e_src) = elems[k]
                            && bm.edges.contains(e_src.0)
                            && let Some(e_dst) = edge_find_double(bm, e_src)
                        {
                            bm.edge_splice(e_dst, e_src);
                            elems.swap_remove(k);
                            continue;
                        }
                        k += 1;
                    }
                    // Full copies of faces may cause overlap.
                    let mut k = 0;
                    while k < elems.len() {
                        if let BMElem::Face(f_src) = elems[k]
                            && bm.faces.contains(f_src.0)
                            && bm.face_find_double(f_src).is_some()
                        {
                            bm.face_kill(f_src);
                            elems.swap_remove(k);
                            continue;
                        }
                        k += 1;
                    }
                    geom_last = elems;
                }
            }
            if use_dvec {
                let mut dvec_step = math::mul_m3_v3(&rmat, dvec);
                dvec_step = math::scale(dvec_step, (a + 1) as f32);
                let mut tmat = math::UNIT_M4;
                tmat[3][0] = dvec_step[0];
                tmat[3][1] = dvec_step[1];
                tmat[3][2] = dvec_step[2];
                math::bmo_transform(bm, &verts_of(&geom_last), &tmat, &space, false);
            }
        }
        geom_last.retain(|&g| bm.elem_alive(g));
        Ok(SpinOut {
            geom: geom_last.clone(),
            geom_last,
        })
    })
}
