// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.normals_split_custom_set_from_vertices`: per-vertex custom normals encoded as the
//! `custom_normal` INT16_2D corner attribute in each smooth fan's normal space.
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (Blender 5.2,
//! GPL-2.0-or-later): `mesh_set_custom_normals` (`use_vertices = true`),
//! `mesh_normals_corner_custom_set`, `corner_space_custom_normal_to_data`,
//! `unit_float_to_short`, and the fan/space computation of `normals_calc_corners`
//! (`collect_corner_info`, `calc_local_edge_indices`, `add_corner_to_edge`,
//! `traverse_fan_local_corners`, `accumulate_fan_normal`, `corner_fan_space_define`), as
//! `frap-geom`'s decoding port does it. With vertex normals no edge is ever tagged sharp, so the
//! `sharp_edge` attribute Blender adds stays all false.

use frap_geom::normals::{
    CornerNormalSpace, V3, VertToFaceMap, add, cross, dot, normalize, safe_acos_approx, scale, sub,
};

/// `LNOR_SPACE_TRIGO_THRESHOLD`.
const LNOR_SPACE_TRIGO_THRESHOLD: f32 = 1.0 - 1e-4;

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (corner_fan_space_define).
fn corner_fan_space_define(
    lnor: V3,
    vec_ref: V3,
    vec_other: V3,
    edge_vectors: &[V3],
) -> CornerNormalSpace {
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
fn unit_float_to_short(v: f32) -> i16 {
    (v * f32::from(i16::MAX) + 0.5).floor() as i16
}

/// `compare_v3v3(a, b, limit)`.
fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (corner_space_custom_normal_to_data).
/// Encodes `custom` in `space`.
pub(crate) fn custom_normal_to_data(space: &CornerNormalSpace, custom: V3) -> [i16; 2] {
    // A null vector is the "no custom normal" value; so is the automatic normal itself.
    if custom == [0.0; 3] || compare_v3v3(space.vec_lnor, custom, 1e-4) {
        return [0, 0];
    }
    let pi2 = std::f32::consts::PI * 2.0;
    let cos_alpha = dot(space.vec_lnor, custom);
    let alpha = safe_acos_approx(cos_alpha);
    let a = if alpha > space.ref_alpha {
        unit_float_to_short(-(pi2 - alpha) / (pi2 - space.ref_alpha))
    } else {
        unit_float_to_short(alpha / space.ref_alpha)
    };
    let vec = normalize(sub(custom, scale(space.vec_lnor, cos_alpha)));
    let cos_beta = dot(space.vec_ref, vec);
    let b = if cos_beta < LNOR_SPACE_TRIGO_THRESHOLD {
        let mut beta = safe_acos_approx(cos_beta);
        if dot(space.vec_ortho, vec) < 0.0 {
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
    [a, b]
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
    OneCorner {
        local_corner_1: usize,
        winding_towards_vert: bool,
    },
    TwoCorners {
        local_corner_1: usize,
        local_corner_2: usize,
    },
    Sharp,
}

fn add_corner_to_edge(
    corner_edges: &[i32],
    sharp_edges: Option<&[bool]>,
    local_corner: usize,
    corner: usize,
    other_corner: usize,
    winding_towards_vert: bool,
    info: &mut EdgeInfo,
) {
    match *info {
        EdgeInfo::Uninitialized => {
            if let Some(sharp) = sharp_edges {
                let c = if winding_towards_vert {
                    other_corner
                } else {
                    corner
                };
                if sharp
                    .get(corner_edges[c] as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    *info = EdgeInfo::Sharp;
                    return;
                }
            }
            *info = EdgeInfo::OneCorner {
                local_corner_1: local_corner,
                winding_towards_vert,
            };
        }
        EdgeInfo::OneCorner {
            local_corner_1,
            winding_towards_vert: w,
        } => {
            if w == winding_towards_vert {
                *info = EdgeInfo::Sharp;
                return;
            }
            *info = EdgeInfo::TwoCorners {
                local_corner_1,
                local_corner_2: local_corner,
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

/// Mesh topology and flags the encoder reads.
pub(crate) struct Input<'a> {
    pub positions: &'a [V3],
    pub face_offsets: &'a [i32],
    pub corner_verts: &'a [i32],
    pub corner_edges: &'a [i32],
    /// "True" face normals.
    pub face_normals: &'a [V3],
    /// `Mesh::vert_normals()` (replaces zero custom normals).
    pub vert_normals: &'a [V3],
    pub sharp_edges: Option<&'a [bool]>,
    pub sharp_faces: Option<&'a [bool]>,
}

/// `mesh_normals_corner_custom_set(…, use_vertices = true, …)`: the `custom_normal` values
/// for per-vertex custom normals `vert_custom` (one per vertex; zero = automatic).
pub(crate) fn encode_from_vertices(inp: &Input<'_>, vert_custom: &[V3]) -> Vec<[i16; 2]> {
    let corner_verts = inp.corner_verts;
    let mut out = vec![[0i16; 2]; corner_verts.len()];
    if corner_verts.is_empty() {
        return out;
    }
    // Zero vectors take the vertex normal; the others are normalised with `math::normalize`
    // (division by the length) before they are encoded, as `mesh_set_custom_normals` does.
    // Without it, a glTF normal whose length rounds just below 1 can encode an `alpha` of
    // several units where Blender's dot product with the fan normal rounds to exactly 1
    // (`alpha` 0): an imported Blender-written file re-exported 2e-4 off (M34S5-3).
    let custom: Vec<V3> = vert_custom
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            if n == [0.0; 3] {
                inp.vert_normals.get(i).copied().unwrap_or(n)
            } else {
                normalize(n)
            }
        })
        .collect();
    let map = VertToFaceMap::new(inp.positions.len(), inp.face_offsets, corner_verts);
    let face_range = |f: usize| {
        (
            inp.face_offsets[f] as usize,
            inp.face_offsets[f + 1] as usize,
        )
    };
    let mut infos: Vec<VertCornerInfo> = Vec::new();
    let mut local_edge_verts: Vec<usize> = Vec::new();
    let mut edge_infos: Vec<EdgeInfo> = Vec::new();
    let mut edge_dirs: Vec<V3> = Vec::new();
    let mut visited: Vec<bool> = Vec::new();
    let mut fan: Vec<usize> = Vec::new();
    for (vert, &vert_co) in inp.positions.iter().enumerate() {
        let vert_faces = map.get(vert);
        if vert_faces.is_empty() {
            continue;
        }
        infos.clear();
        for &face in vert_faces {
            let face = face as usize;
            let (s, e) = face_range(face);
            let Some(corner) = (s..e).find(|&c| corner_verts[c] as usize == vert) else {
                continue;
            };
            let cp = if corner == s { e - 1 } else { corner - 1 };
            let cn = if corner + 1 == e { s } else { corner + 1 };
            infos.push(VertCornerInfo {
                face,
                corner,
                corner_prev: cp,
                corner_next: cn,
                vert_prev: corner_verts[cp] as usize,
                vert_next: corner_verts[cn] as usize,
                local_edge_prev: 0,
                local_edge_next: 0,
            });
        }
        local_edge_verts.clear();
        let index_of_or_add = |list: &mut Vec<usize>, v: usize| -> usize {
            if let Some(i) = list.iter().position(|&x| x == v) {
                i
            } else {
                list.push(v);
                list.len() - 1
            }
        };
        for info in infos.iter_mut() {
            info.local_edge_prev = index_of_or_add(&mut local_edge_verts, info.vert_prev);
            info.local_edge_next = index_of_or_add(&mut local_edge_verts, info.vert_next);
        }
        edge_infos.clear();
        edge_infos.resize(local_edge_verts.len(), EdgeInfo::Uninitialized);
        for (local_corner, info) in infos.iter().enumerate() {
            if inp.sharp_faces.is_some_and(|s| s[info.face]) {
                edge_infos[info.local_edge_prev] = EdgeInfo::Sharp;
                edge_infos[info.local_edge_next] = EdgeInfo::Sharp;
                continue;
            }
            add_corner_to_edge(
                inp.corner_edges,
                inp.sharp_edges,
                local_corner,
                info.corner,
                info.corner_prev,
                true,
                &mut edge_infos[info.local_edge_prev],
            );
            add_corner_to_edge(
                inp.corner_edges,
                inp.sharp_edges,
                local_corner,
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
                .map(|&v| normalize(sub(inp.positions[v], vert_co))),
        );
        let mut visited_count = 0usize;
        visited.clear();
        visited.resize(infos.len(), false);
        let mut start = 0usize;
        loop {
            fan.clear();
            traverse_fan(&infos, &edge_infos, start, &mut fan);
            let fan_normal = if fan.len() == 1 {
                inp.face_normals[infos[fan[0]].face]
            } else {
                let mut n = [0.0f32; 3];
                for &lc in &fan {
                    let info = &infos[lc];
                    let factor = safe_acos_approx(dot(
                        edge_dirs[info.local_edge_prev],
                        edge_dirs[info.local_edge_next],
                    ));
                    n = add(n, scale(inp.face_normals[info.face], factor));
                }
                normalize(n)
            };
            let local_edge_first = infos[fan[0]].local_edge_next;
            let local_edge_last = infos[*fan.last().expect("fan is non-empty")].local_edge_prev;
            let mut fan_edge_dirs: Vec<V3> = Vec::new();
            if fan.len() > 1 {
                for &lc in &fan {
                    fan_edge_dirs.push(edge_dirs[infos[lc].local_edge_next]);
                }
                if local_edge_last != local_edge_first {
                    fan_edge_dirs.push(edge_dirs[local_edge_last]);
                }
            }
            let space = corner_fan_space_define(
                fan_normal,
                edge_dirs[local_edge_first],
                edge_dirs[local_edge_last],
                &fan_edge_dirs,
            );
            // Every corner of a vertex fan has the same (vertex) custom normal; the fan's
            // average is taken as Blender does (sum, then × 1/n) and encoded once.
            let data = if fan.len() < 2 {
                custom_normal_to_data(&space, custom[vert])
            } else {
                let mut avg = [0.0f32; 3];
                for _ in &fan {
                    avg = add(avg, custom[vert]);
                }
                let f = 1.0 / fan.len() as f32;
                custom_normal_to_data(&space, scale(avg, f))
            };
            for &lc in &fan {
                out[infos[lc].corner] = data;
            }
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_normal_encodes_to_zero() {
        let space = corner_fan_space_define([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], &[]);
        assert_eq!(custom_normal_to_data(&space, [0.0, 0.0, 1.0]), [0, 0]);
        assert_eq!(custom_normal_to_data(&space, [0.0; 3]), [0, 0]);
    }

    #[test]
    fn encode_decode_round_trip() {
        let space = corner_fan_space_define(
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.1, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.1, 0.0]],
        );
        let n = normalize([0.2, 0.1, 0.9]);
        let d = custom_normal_to_data(&space, n);
        let back = frap_geom::normals::corner_space_custom_data_to_normal(&space, d);
        assert!(dot(n, back) > 0.9999, "{n:?} → {d:?} → {back:?}");
    }

    #[test]
    fn custom_normals_are_normalized_before_encoding() {
        // M34S5-3: the top-centre corner of the `export_apply_stack` cube (Bevel + Subdivision)
        // re-imported from Blender's own re-export, a single-corner fan. The glTF normal's
        // squared length rounds to 0.99999994; normalised, its dot product with the fan normal
        // is exactly 1, so Blender 5.2.4 stores `alpha` 0 (verified: `(0, -16384)`), where the
        // raw vector encoded `alpha` 7 (2e-4 off after decoding).
        let positions = [
            [0.556_723_4, 8.558_992e-9, 0.988_022_8],
            [1.069_874e-9, -1.883_714_7e-8, 0.995_175_1],
            [9.900_878e-9, -0.556_723_4, 0.988_022_8],
        ];
        let face_offsets = [0, 3];
        let corner_verts = [0, 1, 2];
        let corner_edges = [0, 1, 2];
        let face_normals =
            frap_geom::normals::normals_calc_faces(&positions, &face_offsets, &corner_verts);
        let vert_normals = [face_normals[0]; 3];
        let inp = Input {
            positions: &positions,
            face_offsets: &face_offsets,
            corner_verts: &corner_verts,
            corner_edges: &corner_edges,
            face_normals: &face_normals,
            vert_normals: &vert_normals,
            sharp_edges: None,
            sharp_faces: None,
        };
        let custom = [
            [0.0; 3],
            [0.012_500_546, -0.012_500_546, 0.999_843_7],
            [0.0; 3],
        ];
        let data = encode_from_vertices(&inp, &custom);
        assert_eq!(data[1], [0, -16384]);
    }
}
