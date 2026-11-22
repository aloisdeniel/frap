// SPDX-License-Identifier: GPL-3.0-or-later
//! `orient_type='NORMAL'` in Edit Mode: the orientation built from the selection of the edit
//! mesh. Ported from `getTransformOrientation_ex` (edit-mesh branch),
//! `bm_mesh_elems_select_get_n__internal` and `ED_getTransformOrientationMatrix`
//! (`blender/source/blender/editors/transform/transform_orientations.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! The normal and plane are computed in object space from (in order of precedence) the active
//! element (pivot `ACTIVE_ELEMENT` only), the selected faces, three selected vertices, one
//! selected edge or two vertices, one vertex, or more vertices; then mapped to world space (the
//! normal by the inverse transpose, or both by the object matrix for edge results).

use frap_bmesh::{BMElem, BMesh, ElemFlag, VertId};
use glam::{Mat3, Mat4, Vec3};

use super::bmq::{
    self, co, edge_len_sq, edge_ordered_verts, editselection_normal, editselection_plane,
    face_calc_tangent_pair_auto, fno, has, vert_edge_pair, vert_tri_calc_tangent_from_edge, vno,
};
use super::mathx::{
    create_space_normal, create_space_normal_tangent, cross, invert_m3, is_zero, len_sq, normalize,
    normalized, ortho, project, project_plane_normalized,
};

/// `ORIENTATION_*` results.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `ORIENTATION_NONE`: nothing selected.
    None,
    /// `ORIENTATION_VERT`: a normal only.
    Vert,
    /// `ORIENTATION_EDGE`: a normal and an edge direction.
    Edge,
    /// `ORIENTATION_FACE`: a normal and a tangent.
    Face,
}

/// `bm_mesh_elems_select_get_n__internal`: up to `n` selected elements of one type, from the
/// selection history (newest first, only when the whole run is of that type) or in mesh order.
fn select_get_n(bm: &BMesh, n: usize, want_vert: bool) -> Vec<BMElem> {
    let total = {
        let (v, e, _) = bm.selected_counts();
        if want_vert { v } else { e }
    };
    let n = n.min(total);
    let is_type = |e: &BMElem| match e {
        BMElem::Vert(_) => want_vert,
        BMElem::Edge(_) => !want_vert,
        BMElem::Face(_) => false,
    };
    if !bm.select_history.is_empty() {
        let mut out = Vec::new();
        for ese in bm.select_history.iter().rev() {
            if !has(bm, *ese, ElemFlag::SELECT) {
                continue;
            }
            if !is_type(ese) {
                out.clear();
                break;
            }
            out.push(*ese);
            if out.len() == n {
                break;
            }
        }
        if !out.is_empty() && out.len() == n {
            return out;
        }
    }
    let mut out = Vec::new();
    if n == 0 {
        return out;
    }
    if want_vert {
        for v in bm.vert_ids() {
            if has(bm, v, ElemFlag::SELECT) {
                out.push(BMElem::Vert(v));
                if out.len() == n {
                    break;
                }
            }
        }
    } else {
        for e in bm.edge_ids() {
            if has(bm, e, ElemFlag::SELECT) {
                out.push(BMElem::Edge(e));
                if out.len() == n {
                    break;
                }
            }
        }
    }
    out
}

fn as_vert(e: BMElem) -> VertId {
    match e {
        BMElem::Vert(v) => v,
        _ => unreachable!("vertex selection"),
    }
}

/// The edit-mesh part of `getTransformOrientation_ex`: object-space normal and plane.
fn mesh_normal_plane(bm: &BMesh, active_only: bool) -> (Vec3, Vec3, Kind) {
    let mut normal = Vec3::ZERO;
    let mut plane = Vec3::ZERO;
    let mut result = Kind::None;
    let (totvertsel, totedgesel, totfacesel) = bm.selected_counts();

    let active = if active_only {
        bmq::select_history_active_get(bm)
    } else {
        None
    };
    if let Some(ese) = active {
        normal = editselection_normal(bm, ese);
        // `ese.prev` is null for the element `BM_select_history_active_get` fills in.
        plane = editselection_plane(bm, ese, None);
        result = match ese {
            BMElem::Vert(_) => Kind::Vert,
            BMElem::Edge(_) => Kind::Edge,
            BMElem::Face(_) => Kind::Face,
        };
    } else if totfacesel >= 1 {
        let mut n = Vec3::ZERO;
        let mut pair = [Vec3::ZERO; 2];
        let mut count = 0;
        for f in bm.face_ids() {
            if has(bm, f, ElemFlag::SELECT) {
                let (a, b) = face_calc_tangent_pair_auto(bm, f);
                n += fno(bm, f);
                pair[0] += a;
                pair[1] += b;
                count += 1;
            }
        }
        let index = if count == 1 {
            0
        } else {
            let nu = normalized(n);
            let o0 = cross(nu, normalized(pair[0]));
            let o1 = cross(nu, normalized(pair[1]));
            if len_sq(o0) > len_sq(o1) { 0 } else { 1 }
        };
        normal += n;
        plane += pair[index];
        result = Kind::Face;
    } else if totvertsel == 3 {
        let sel = select_get_n(bm, 3, true);
        if sel.len() == 3 {
            let v: Vec<VertId> = sel.into_iter().map(as_vert).collect();
            let c = [co(bm, v[0]), co(bm, v[1]), co(bm, v[2])];
            normal = Vec3::from_array(
                frap_bmesh::query::geom::normal_tri_v3(
                    c[0].to_array(),
                    c[1].to_array(),
                    c[2].to_array(),
                )
                .0,
            );
            let no_test = vno(bm, v[0]) + vno(bm, v[1]) + vno(bm, v[2]);
            if super::mathx::dot(no_test, normal) < 0.0 {
                normal = -normal;
            }
            let mut best = None;
            if totedgesel >= 1 {
                let mut best_len = 0.0f32;
                for j in 0..3 {
                    if let Some(e) = bm.edge_exists(v[j], v[(j + 1) % 3])
                        && has(bm, e, ElemFlag::SELECT)
                    {
                        let l = edge_len_sq(bm, e);
                        if best.is_none() || best_len < l {
                            best = Some(e);
                            best_len = l;
                        }
                    }
                }
            }
            plane = if let Some(e) = best {
                let (a, b) = if bm.edge_is_boundary(e) {
                    edge_ordered_verts(bm, e)
                } else {
                    let ed = bm.edge(e);
                    (ed.v[0], ed.v[1])
                };
                co(bm, a) - co(bm, b)
            } else {
                vert_tri_calc_tangent_from_edge(c)
            };
        }
        result = Kind::Face;
    } else if totedgesel == 1 || totvertsel == 2 {
        let mut pair: Option<(VertId, VertId)> = None;
        let mut eed = None;
        if totedgesel == 1 {
            if let Some(BMElem::Edge(e)) = select_get_n(bm, 1, false).first().copied() {
                let ed = bm.edge(e);
                pair = Some((ed.v[0], ed.v[1]));
                eed = Some(e);
            }
        } else {
            let s = select_get_n(bm, 2, true);
            if s.len() == 2 {
                pair = Some((as_vert(s[0]), as_vert(s[1])));
            }
        }
        if let Some((mut a, mut b)) = pair {
            let active_vert = match bm.select_history.last() {
                Some(BMElem::Vert(v)) => Some(*v),
                _ => None,
            };
            let mut swap = false;
            if active_vert == Some(b) {
                swap = true;
            } else if let Some(e) = eed
                && bm.edge_is_boundary(e)
            {
                let l = bm.edge(e).l.expect("boundary loop");
                if bm.lp(l).v != a {
                    swap = true;
                }
            }
            if swap {
                std::mem::swap(&mut a, &mut b);
            }
            normal = vno(bm, b) + vno(bm, a);
            let (p, l) = normalize(co(bm, b) - co(bm, a));
            plane = p;
            if l != 0.0 {
                normal = project_plane_normalized(normal, plane);
                let (n, nl) = normalize(normal);
                normal = if nl == 0.0 { ortho(plane) } else { n };
            }
        }
        result = Kind::Edge;
    } else if totvertsel == 1 {
        if let Some(BMElem::Vert(v)) = select_get_n(bm, 1, true).first().copied() {
            normal = vno(bm, v);
            if let Some((e0, e1)) = vert_edge_pair(bm, v) {
                let mut pv = [bm.edge_other_vert(e0, v), bm.edge_other_vert(e1, v)];
                let swap = if bm.edge_is_boundary(e0) {
                    let l = bm.edge(e0).l.expect("boundary loop");
                    bm.lp(l).v != v
                } else {
                    edge_len_sq(bm, e0) < edge_len_sq(bm, e1)
                };
                if swap {
                    pv.swap(0, 1);
                }
                let d0 = normalized(co(bm, v) - co(bm, pv[0]));
                let d1 = normalized(co(bm, pv[1]) - co(bm, v));
                plane = d0 + d1;
            }
        }
        result = if is_zero(plane) {
            Kind::Vert
        } else {
            Kind::Edge
        };
    } else if totvertsel > 3 {
        let mut n = Vec3::ZERO;
        for v in bm.vert_ids() {
            if has(bm, v, ElemFlag::SELECT) {
                n += vno(bm, v);
            }
        }
        normal = normalized(n);
        result = Kind::Vert;
    }
    // "Not needed but this matches 2.68 and older behavior."
    plane = -plane;
    (normal, plane, result)
}

/// `getTransformOrientation_ex` for a mesh in Edit Mode: the world-space normal and plane
/// (both normalized) and what they were made from. `obmat` is the edit object's world matrix;
/// `active_only` is pivot `ACTIVE_ELEMENT`.
pub fn orientation_vectors(bm: &BMesh, obmat: &Mat4, active_only: bool) -> (Vec3, Vec3, Kind) {
    let (mut normal, mut plane, kind) = mesh_normal_plane(bm, active_only);
    let obmat3 = Mat3::from_mat4(*obmat);
    if kind == Kind::Edge {
        normal = obmat3 * normal;
        plane = obmat3 * plane;
        normal -= project(normal, plane);
    } else {
        let mat = invert_m3(&obmat3).unwrap_or(Mat3::ZERO).transpose();
        normal = mat * normal;
        plane = mat * plane;
    }
    (normalized(normal), normalized(plane), kind)
}

/// `ED_getTransformOrientationMatrix` for a mesh in Edit Mode: the NORMAL orientation (columns
/// are the axes). `obmat` is the active object's world matrix; `active_only` is pivot
/// `ACTIVE_ELEMENT`.
pub fn normal_orientation(bm: &BMesh, obmat: &Mat4, active_only: bool) -> Mat3 {
    let (normal, plane, mut kind) = orientation_vectors(bm, obmat, active_only);
    if matches!(kind, Kind::Edge | Kind::Face) && is_zero(plane) {
        kind = Kind::Vert;
    }
    let m = match kind {
        Kind::Vert => create_space_normal(normal),
        Kind::Edge | Kind::Face => create_space_normal_tangent(normal, plane),
        Kind::None => None,
    };
    m.unwrap_or(Mat3::IDENTITY)
}

// Ported from blender/source/blender/editors/transform/transform_orientations.cc
// (createMeshSpace, getTransformOrientation), Blender 5.2.4, GPL-2.0-or-later.
/// `createMeshSpace`: the custom orientation `transform.create_orientation` makes from the
/// selection of the mesh in Edit Mode and its default name (`Vertex`, `Edge`, `Face`); `None`
/// when nothing is selected or the vectors are degenerate (Blender's "Unable to create
/// orientation"). Unlike the NORMAL orientation, an edge or face result without a plane is
/// not demoted to a vertex one.
pub fn mesh_space(bm: &BMesh, obmat: &Mat4) -> Option<(Mat3, &'static str)> {
    // `getTransformOrientation` passes `V3D_AROUND_CENTER_BOUNDS`: never the active element.
    let (normal, plane, kind) = orientation_vectors(bm, obmat, false);
    match kind {
        Kind::Vert => create_space_normal(normal).map(|m| (m, "Vertex")),
        Kind::Edge => create_space_normal_tangent(normal, plane).map(|m| (m, "Edge")),
        Kind::Face => create_space_normal_tangent(normal, plane).map(|m| (m, "Face")),
        Kind::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_bmesh::BMesh;

    #[test]
    fn empty_selection_is_identity() {
        let bm = BMesh::new();
        assert_eq!(
            normal_orientation(&bm, &Mat4::IDENTITY, false),
            Mat3::IDENTITY
        );
    }
}
