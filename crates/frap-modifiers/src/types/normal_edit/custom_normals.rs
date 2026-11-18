// SPDX-License-Identifier: GPL-3.0-or-later
//! Setting custom split normals (`custom_normal`, `INT16_2D` on corners): the fan spaces of
//! `normals_calc_corners(…, r_fan_spaces, …)`, `corner_space_custom_normal_to_data` and
//! `mesh_normals_corner_custom_set` (which tags sharp edges where a smooth fan's requested
//! normals disagree, recomputes the fans and encodes the average normal of each fan). Used by
//! WEIGHTED_NORMAL, NORMAL_EDIT and TRIANGULATE (`keep_custom_normals`).
//!
//! `frap-geom` has the decoding half (`normals_calc_corners`, `corner_space_custom_data_to_normal`);
//! this module adds the encoding half next to its users (a submodule of a file WP 4.3 owns,
//! ARCHITECTURE §22.1).
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (Blender 5.2.4):
//! `corner_fan_space_define`, `collect_corner_info`, `add_corner_to_edge`,
//! `calc_local_edge_indices`, `calc_connecting_edge_info`, `traverse_fan_local_corners`,
//! `accumulate_fan_normal`, `handle_fan_result_and_custom_normals`, `normals_calc_corners`,
//! `corner_space_custom_normal_to_data`, `unit_float_to_short`,
//! `mesh_normals_corner_custom_set` and `mesh_set_custom_normals`.

use std::sync::Arc;

use frap_geom::normals::{
    CornerNormalSpace, V3, VertToFaceMap, corner_space_custom_data_to_normal, cross, dot,
    normalize, normals_calc_faces, normals_calc_verts, safe_acos_approx, scale, sub,
};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};

/// `LNOR_SPACE_TRIGO_THRESHOLD`.
pub(crate) const LNOR_SPACE_TRIGO_THRESHOLD: f32 = 1.0 - 1e-4;

/// The fan spaces of a mesh (`CornerNormalSpaceArray` with `create_corners_by_space`).
#[derive(Clone, Debug, Default)]
pub(crate) struct FanSpaces {
    /// One space per smooth fan.
    pub spaces: Vec<CornerNormalSpace>,
    /// `corner_space_indices`.
    pub corner_space: Vec<usize>,
    /// `corners_by_space`: the corners of each fan, in traversal order.
    pub corners_by_space: Vec<Vec<usize>>,
}

/// The mesh data the corner-normal functions read.
#[derive(Copy, Clone)]
pub(crate) struct MeshView<'a> {
    pub positions: &'a [V3],
    pub face_offsets: &'a [i32],
    pub corner_verts: &'a [i32],
    pub corner_edges: &'a [i32],
    pub vert_to_face: &'a VertToFaceMap,
    pub face_normals: &'a [V3],
    pub sharp_faces: Option<&'a [bool]>,
}

#[derive(Copy, Clone, Debug)]
struct VertCornerInfo {
    face: usize,
    corner: usize,
    corner_prev: usize,
    corner_next: usize,
    vert_prev: usize,
    vert_next: usize,
    local_edge_prev: usize,
    local_edge_next: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum EdgeInfo {
    Uninitialized,
    OneCorner { local_corner_1: usize, towards: bool },
    TwoCorners { local_corner_1: usize, local_corner_2: usize },
    Sharp,
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (corner_fan_space_define).
fn fan_space_define(lnor: V3, vec_ref: V3, vec_other: V3, edge_vectors: &[V3]) -> CornerNormalSpace {
    let mut space = CornerNormalSpace::default();
    let pi2 = std::f32::consts::PI * 2.0;
    let dtp_ref = dot(vec_ref, lnor);
    let dtp_other = dot(vec_other, lnor);
    if dtp_ref.abs() >= LNOR_SPACE_TRIGO_THRESHOLD || dtp_other.abs() >= LNOR_SPACE_TRIGO_THRESHOLD
    {
        return space;
    }
    space.vec_lnor = lnor;
    if !edge_vectors.is_empty() {
        let mut alpha = 0.0f32;
        for &v in edge_vectors {
            alpha += safe_acos_approx(dot(v, lnor));
        }
        space.ref_alpha = alpha / edge_vectors.len() as f32;
    } else {
        space.ref_alpha =
            (safe_acos_approx(dot(vec_ref, lnor)) + safe_acos_approx(dot(vec_other, lnor))) / 2.0;
    }
    space.vec_ref = normalize(sub(vec_ref, scale(lnor, dtp_ref)));
    space.vec_ortho = normalize(cross(lnor, space.vec_ref));
    let vec_other_proj = normalize(sub(vec_other, scale(lnor, dtp_other)));
    let dtp = dot(space.vec_ref, vec_other_proj);
    if dtp < LNOR_SPACE_TRIGO_THRESHOLD {
        let beta = safe_acos_approx(dtp);
        space.ref_beta = if dot(space.vec_ortho, vec_other_proj) < 0.0 {
            pi2 - beta
        } else {
            beta
        };
    } else {
        space.ref_beta = pi2;
    }
    space
}

/// `unit_float_to_short`.
fn unit_float_to_short(val: f32) -> i16 {
    libm::floorf(val * i16::MAX as f32 + 0.5) as i16
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (corner_space_custom_normal_to_data).
/// Encodes `custom` in `space` (zero, or equal to the automatic normal within 1e-4: `(0, 0)`).
pub(crate) fn custom_normal_to_data(space: &CornerNormalSpace, custom: V3) -> [i16; 2] {
    let close = (0..3).all(|k| (space.vec_lnor[k] - custom[k]).abs() <= 1e-4);
    if custom == [0.0; 3] || close {
        return [0, 0];
    }
    let pi2 = (std::f64::consts::PI * 2.0) as f32;
    let cos_alpha = dot(space.vec_lnor, custom);
    let alpha = safe_acos_approx(cos_alpha);
    let c0 = if alpha > space.ref_alpha {
        unit_float_to_short(-(pi2 - alpha) / (pi2 - space.ref_alpha))
    } else {
        unit_float_to_short(alpha / space.ref_alpha)
    };
    let v = normalize([
        space.vec_lnor[0] * -cos_alpha + custom[0],
        space.vec_lnor[1] * -cos_alpha + custom[1],
        space.vec_lnor[2] * -cos_alpha + custom[2],
    ]);
    let cos_beta = dot(space.vec_ref, v);
    let c1 = if cos_beta < LNOR_SPACE_TRIGO_THRESHOLD {
        let mut beta = safe_acos_approx(cos_beta);
        if dot(space.vec_ortho, v) < 0.0 {
            beta = pi2 - beta;
        }
        if beta > space.ref_beta {
            unit_float_to_short(-(pi2 - beta) / (pi2 - space.ref_beta))
        } else {
            unit_float_to_short(beta / space.ref_beta)
        }
    } else {
        0
    };
    [c0, c1]
}

fn add_corner_to_edge(
    corner_edges: &[i32],
    sharp_edges: Option<&[bool]>,
    local_corner: usize,
    corner: usize,
    other_corner: usize,
    towards: bool,
    info: &mut EdgeInfo,
) {
    match *info {
        EdgeInfo::Uninitialized => {
            if let Some(sharp) = sharp_edges {
                let c = if towards { other_corner } else { corner };
                if sharp[corner_edges[c] as usize] {
                    *info = EdgeInfo::Sharp;
                    return;
                }
            }
            *info = EdgeInfo::OneCorner {
                local_corner_1: local_corner,
                towards,
            };
        }
        EdgeInfo::OneCorner {
            local_corner_1,
            towards: t,
        } => {
            *info = if t == towards {
                EdgeInfo::Sharp
            } else {
                EdgeInfo::TwoCorners {
                    local_corner_1,
                    local_corner_2: local_corner,
                }
            };
        }
        _ => *info = EdgeInfo::Sharp,
    }
}

fn traverse_fan(infos: &[VertCornerInfo], edges: &[EdgeInfo], start: usize, fan: &mut Vec<usize>) {
    fan.push(start);
    let mut current = start;
    let mut local_edge = infos[current].local_edge_next;
    let mut cyclic = false;
    while let EdgeInfo::TwoCorners {
        local_corner_1,
        local_corner_2,
    } = edges[local_edge]
    {
        current = if current == local_corner_1 {
            local_corner_2
        } else {
            local_corner_1
        };
        if current == start {
            cyclic = true;
            break;
        }
        fan.push(current);
        local_edge = infos[current].local_edge_next;
    }
    fan.reverse();
    if cyclic {
        let mut best = 0;
        for i in 1..fan.len() {
            if infos[fan[i]].corner < infos[fan[best]].corner {
                best = i;
            }
        }
        fan.rotate_left(best);
        return;
    }
    let mut current = start;
    let mut local_edge = infos[current].local_edge_prev;
    while let EdgeInfo::TwoCorners {
        local_corner_1,
        local_corner_2,
    } = edges[local_edge]
    {
        current = if current == local_corner_1 {
            local_corner_2
        } else {
            local_corner_1
        };
        fan.push(current);
        local_edge = infos[current].local_edge_prev;
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (normals_calc_corners,
// handle_fan_result_and_custom_normals).
/// Corner normals and the fan spaces (fans in vertex order, then traversal order).
pub(crate) fn calc_corners_with_spaces(
    m: &MeshView<'_>,
    sharp_edges: Option<&[bool]>,
    custom: Option<&[[i16; 2]]>,
) -> (Vec<V3>, FanSpaces) {
    let ncorners = m.corner_verts.len();
    let mut out = vec![[0.0f32; 3]; ncorners];
    let mut spaces = FanSpaces {
        spaces: Vec::new(),
        corner_space: vec![usize::MAX; ncorners],
        corners_by_space: Vec::new(),
    };
    if ncorners == 0 {
        return (out, spaces);
    }
    let fr = |f: usize| (m.face_offsets[f] as usize, m.face_offsets[f + 1] as usize);
    let mut infos: Vec<VertCornerInfo> = Vec::new();
    let mut local_edge_verts: Vec<usize> = Vec::new();
    let mut edge_infos: Vec<EdgeInfo> = Vec::new();
    let mut edge_dirs: Vec<V3> = Vec::new();
    let mut visited: Vec<bool> = Vec::new();
    let mut fan: Vec<usize> = Vec::new();
    for (vert, &vert_co) in m.positions.iter().enumerate() {
        frap_math::budget::check();
        let vert_faces = m.vert_to_face.get(vert);
        if vert_faces.is_empty() {
            continue;
        }
        infos.clear();
        for &face in vert_faces {
            let face = face as usize;
            let (s, e) = fr(face);
            let corner = (s..e)
                .find(|&c| m.corner_verts[c] as usize == vert)
                .expect("vertex-to-face map is consistent");
            let cp = if corner == s { e - 1 } else { corner - 1 };
            let cn = if corner + 1 == e { s } else { corner + 1 };
            infos.push(VertCornerInfo {
                face,
                corner,
                corner_prev: cp,
                corner_next: cn,
                vert_prev: m.corner_verts[cp] as usize,
                vert_next: m.corner_verts[cn] as usize,
                local_edge_prev: 0,
                local_edge_next: 0,
            });
        }
        local_edge_verts.clear();
        for info in infos.iter_mut() {
            for (v, slot) in [
                (info.vert_prev, &mut info.local_edge_prev),
                (info.vert_next, &mut info.local_edge_next),
            ] {
                *slot = match local_edge_verts.iter().position(|&x| x == v) {
                    Some(i) => i,
                    None => {
                        local_edge_verts.push(v);
                        local_edge_verts.len() - 1
                    }
                };
            }
        }
        edge_infos.clear();
        edge_infos.resize(local_edge_verts.len(), EdgeInfo::Uninitialized);
        for (lc, info) in infos.iter().enumerate() {
            if m.sharp_faces.is_some_and(|s| s[info.face]) {
                edge_infos[info.local_edge_prev] = EdgeInfo::Sharp;
                edge_infos[info.local_edge_next] = EdgeInfo::Sharp;
                continue;
            }
            add_corner_to_edge(
                m.corner_edges,
                sharp_edges,
                lc,
                info.corner,
                info.corner_prev,
                true,
                &mut edge_infos[info.local_edge_prev],
            );
            add_corner_to_edge(
                m.corner_edges,
                sharp_edges,
                lc,
                info.corner,
                info.corner_next,
                false,
                &mut edge_infos[info.local_edge_next],
            );
        }
        edge_dirs.clear();
        edge_dirs.extend(
            local_edge_verts
                .iter()
                .map(|&v| normalize(sub(m.positions[v], vert_co))),
        );
        let mut visited_count = 0usize;
        visited.clear();
        visited.resize(infos.len(), false);
        let mut start = 0usize;
        loop {
            fan.clear();
            traverse_fan(&infos, &edge_infos, start, &mut fan);
            let mut fan_normal = if fan.len() == 1 {
                m.face_normals[infos[fan[0]].face]
            } else {
                let mut n = [0.0f32; 3];
                for &lc in &fan {
                    let info = &infos[lc];
                    let factor = safe_acos_approx(dot(
                        edge_dirs[info.local_edge_prev],
                        edge_dirs[info.local_edge_next],
                    ));
                    let fnor = m.face_normals[info.face];
                    n = [
                        n[0] + fnor[0] * factor,
                        n[1] + fnor[1] * factor,
                        n[2] + fnor[2] * factor,
                    ];
                }
                normalize(n)
            };
            // handle_fan_result_and_custom_normals.
            let local_edge_first = infos[fan[0]].local_edge_next;
            let local_edge_last = infos[*fan.last().expect("non-empty fan")].local_edge_prev;
            let mut fan_edge_dirs: Vec<V3> = Vec::new();
            if fan.len() > 1 {
                for &lc in &fan {
                    fan_edge_dirs.push(edge_dirs[infos[lc].local_edge_next]);
                }
                if local_edge_last != local_edge_first {
                    fan_edge_dirs.push(edge_dirs[local_edge_last]);
                }
            }
            let space = fan_space_define(
                fan_normal,
                edge_dirs[local_edge_first],
                edge_dirs[local_edge_last],
                &fan_edge_dirs,
            );
            if let Some(custom) = custom {
                let mut avg = [0i32; 2];
                for &lc in &fan {
                    let c = custom[infos[lc].corner];
                    avg[0] += c[0] as i32;
                    avg[1] += c[1] as i32;
                }
                let n = fan.len() as i32;
                fan_normal =
                    corner_space_custom_data_to_normal(&space, [(avg[0] / n) as i16, (avg[1] / n) as i16]);
            }
            let space_index = spaces.spaces.len();
            spaces.spaces.push(space);
            let corners: Vec<usize> = fan.iter().map(|&lc| infos[lc].corner).collect();
            for &c in &corners {
                out[c] = fan_normal;
                spaces.corner_space[c] = space_index;
            }
            spaces.corners_by_space.push(corners);
            visited_count += fan.len();
            if visited_count == infos.len() {
                break;
            }
            for &lc in &fan {
                visited[lc] = true;
            }
            while visited[start] {
                start += 1;
            }
        }
    }
    (out, spaces)
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (mesh_normals_corner_custom_set).
/// Encodes `custom` (per corner, or per vertex with `use_vertices`; zero vectors take the
/// automatic normal) into `clnors`, tagging `sharp_edges` where a fan's normals disagree.
pub(crate) fn normals_corner_custom_set(
    m: &MeshView<'_>,
    vert_normals: &[V3],
    use_vertices: bool,
    custom: &mut [V3],
    sharp_edges: &mut [bool],
    clnors: &mut [[i16; 2]],
) {
    let ncorners = m.corner_verts.len();
    let mut done = vec![false; ncorners];
    let mut corner_to_face = vec![0usize; ncorners];
    for f in 0..m.face_offsets.len().saturating_sub(1) {
        for c in m.face_offsets[f] as usize..m.face_offsets[f + 1] as usize {
            corner_to_face[c] = f;
        }
    }
    let (corner_normals, mut spaces) =
        calc_corners_with_spaces(m, Some(sharp_edges), Some(clnors));
    if use_vertices {
        for (i, n) in custom.iter_mut().enumerate().take(m.positions.len()) {
            if *n == [0.0; 3] {
                *n = vert_normals[i];
            }
        }
    } else {
        for (i, n) in custom.iter_mut().enumerate() {
            if *n == [0.0; 3] {
                *n = corner_normals[i];
            }
        }
    }
    if use_vertices {
        done.fill(true);
    } else {
        for i in 0..ncorners {
            if done[i] {
                continue;
            }
            let space_index = spaces.corner_space[i];
            if space_index == usize::MAX {
                done[i] = true;
                continue;
            }
            let fan = &spaces.corners_by_space[space_index];
            if fan.is_empty() {
                done[i] = true;
                continue;
            }
            let mut prev_corner: Option<usize> = None;
            let mut org_nor: Option<V3> = None;
            let tag = |corner: usize, prev: usize, sharp_edges: &mut [bool]| {
                let f = corner_to_face[corner];
                let (s, e) = (m.face_offsets[f] as usize, m.face_offsets[f + 1] as usize);
                let corner_prev = if corner == s { e - 1 } else { corner - 1 };
                let edge = m.corner_edges[corner];
                let edge_prev = m.corner_edges[corner_prev];
                let prev_edge = m.corner_edges[prev];
                sharp_edges[if prev_edge == edge_prev {
                    prev_edge
                } else {
                    edge
                } as usize] = true;
            };
            for &corner in fan.iter().rev() {
                let nor = custom[corner];
                match org_nor {
                    None => org_nor = Some(nor),
                    Some(o) => {
                        if dot(o, nor) < LNOR_SPACE_TRIGO_THRESHOLD {
                            tag(corner, prev_corner.expect("set with org_nor"), sharp_edges);
                            org_nor = Some(nor);
                        }
                    }
                }
                prev_corner = Some(corner);
                done[corner] = true;
            }
            if fan.len() > 1
                && let Some(o) = org_nor
            {
                let corner = *fan.last().expect("non-empty fan");
                let nor = custom[corner];
                if dot(o, nor) < LNOR_SPACE_TRIGO_THRESHOLD {
                    tag(corner, prev_corner.expect("set with org_nor"), sharp_edges);
                }
            }
        }
        let (_, s) = calc_corners_with_spaces(m, Some(sharp_edges), Some(clnors));
        spaces = s;
    }
    for i in 0..ncorners {
        let space_index = spaces.corner_space[i];
        if space_index == usize::MAX {
            done[i] = false;
            continue;
        }
        if !done[i] {
            continue;
        }
        let fan = &spaces.corners_by_space[space_index];
        let nidx = |c: usize| if use_vertices { m.corner_verts[c] as usize } else { c };
        if fan.len() < 2 {
            clnors[i] = custom_normal_to_data(&spaces.spaces[space_index], custom[nidx(i)]);
            done[i] = false;
        } else {
            let mut avg = [0.0f32; 3];
            for &c in fan {
                let n = custom[nidx(c)];
                avg = [avg[0] + n[0], avg[1] + n[1], avg[2] + n[2]];
                done[c] = false;
            }
            let f = 1.0 / fan.len() as f32;
            let avg = [avg[0] * f, avg[1] * f, avg[2] * f];
            let data = custom_normal_to_data(&spaces.spaces[space_index], avg);
            for &c in fan {
                clnors[c] = data;
            }
        }
    }
}

/// The "true" face and vertex normals of `mesh` (`face_normals_true`, `vert_normals_true`)
/// and its vertex → face map.
pub(crate) fn true_normals(mesh: &Mesh) -> (Vec<V3>, Vec<V3>, VertToFaceMap) {
    let map = VertToFaceMap::new(mesh.verts_num(), mesh.face_offsets(), mesh.corner_verts());
    let fnor = normals_calc_faces(mesh.positions(), mesh.face_offsets(), mesh.corner_verts());
    let vnor = normals_calc_verts(
        mesh.positions(),
        mesh.face_offsets(),
        mesh.corner_verts(),
        &map,
        &fnor,
    );
    (fnor, vnor, map)
}

/// `sharp_face` as a face-domain slice, when present.
pub(crate) fn sharp_faces(mesh: &Mesh) -> Option<Vec<bool>> {
    mesh.sharp_face_flags().map(|c| c.into_owned())
}

/// `lookup_or_add_for_write_span<bool>("sharp_edge", Edge)`: the current values, `None` when an
/// attribute of that name has another type or domain.
pub(crate) fn sharp_edges_for_write(mesh: &Mesh) -> Option<Vec<bool>> {
    match mesh.attributes().get(frap_mesh::SHARP_EDGE) {
        None => Some(vec![false; mesh.edges_num()]),
        Some(Attribute {
            domain: Domain::Edge,
            data: AttrData::Bool(v),
            ..
        }) => Some(v.to_vec()),
        Some(_) => None,
    }
}

/// `lookup_or_add_for_write_span<short2>("custom_normal", Corner)`: the current values,
/// `None` when an attribute of that name has another type or domain.
pub(crate) fn clnors_for_write(mesh: &Mesh) -> Option<Vec<[i16; 2]>> {
    match mesh.attributes().get(frap_mesh::CUSTOM_NORMAL) {
        None => Some(vec![[0, 0]; mesh.corners_num()]),
        Some(Attribute {
            domain: Domain::Corner,
            data: AttrData::Int16x2(v),
            ..
        }) => Some(v.to_vec()),
        Some(_) => None,
    }
}

/// Writes `sharp_edge` and `custom_normal`.
pub(crate) fn store(mesh: &mut Mesh, sharp_edges: Option<Vec<bool>>, clnors: Option<Vec<[i16; 2]>>) {
    let attrs = mesh.attributes_mut();
    if let Some(s) = sharp_edges {
        attrs.insert(Attribute {
            name: frap_mesh::SHARP_EDGE.into(),
            domain: Domain::Edge,
            data: AttrData::Bool(Arc::new(s)),
        });
    }
    if let Some(c) = clnors {
        attrs.insert(Attribute {
            name: frap_mesh::CUSTOM_NORMAL.into(),
            domain: Domain::Corner,
            data: AttrData::Int16x2(Arc::new(c)),
        });
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (mesh_set_custom_normals).
/// `bke::mesh_set_custom_normals(_from_verts)_normalized`: sets `custom` (normalised, per corner
/// or per vertex) as the mesh's custom normals.
pub(crate) fn mesh_set_custom_normals(mesh: &mut Mesh, custom: &mut [V3], use_vertices: bool) {
    let Some(mut clnors) = clnors_for_write(mesh) else {
        return;
    };
    let Some(mut sharp) = sharp_edges_for_write(mesh) else {
        store(mesh, None, Some(clnors));
        return;
    };
    let (fnor, vnor, map) = true_normals(mesh);
    let sf = sharp_faces(mesh);
    let view = MeshView {
        positions: mesh.positions(),
        face_offsets: mesh.face_offsets(),
        corner_verts: mesh.corner_verts(),
        corner_edges: mesh.corner_edges(),
        vert_to_face: &map,
        face_normals: &fnor,
        sharp_faces: sf.as_deref(),
    };
    normals_corner_custom_set(&view, &vnor, use_vertices, custom, &mut sharp, &mut clnors);
    store(mesh, Some(sharp), Some(clnors));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let space = fan_space_define(
            [0.0, 0.0, 1.0],
            normalize([1.0, 0.0, -0.2]),
            normalize([0.0, 1.0, -0.2]),
            &[],
        );
        assert!(space.ref_alpha > 0.0 && space.ref_beta > 0.0);
        let want = normalize([0.3, 0.2, 1.0]);
        let data = custom_normal_to_data(&space, want);
        let got = corner_space_custom_data_to_normal(&space, data);
        for k in 0..3 {
            assert!((got[k] - want[k]).abs() < 2e-3, "{got:?} vs {want:?}");
        }
        assert_eq!(custom_normal_to_data(&space, [0.0, 0.0, 1.0]), [0, 0]);
    }

    #[test]
    fn cube_fans_split_where_normals_differ() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        m.shade_smooth();
        // Every corner asks for its face normal: every edge becomes sharp.
        let fnor = normals_calc_faces(m.positions(), m.face_offsets(), m.corner_verts());
        let mut custom: Vec<V3> = (0..m.faces_num())
            .flat_map(|f| std::iter::repeat_n(fnor[f], m.face(f).len()))
            .collect();
        mesh_set_custom_normals(&mut m, &mut custom, false);
        assert!(m.sharp_edges().unwrap().iter().all(|s| *s));
        for (c, n) in m.corner_normals().iter().enumerate() {
            let f = (0..m.faces_num()).find(|&f| m.face(f).contains(&c)).unwrap();
            for k in 0..3 {
                assert!((n[k] - fnor[f][k]).abs() < 1e-4);
            }
        }
    }
}
