// SPDX-License-Identifier: GPL-3.0-or-later
//! SOLIDIFY `NON_MANIFOLD` ("Complex"): every vertex is split into one new vertex per group of
//! faces around it (the faces between two consecutive edges, sorted by angle around each
//! edge), so edges with more than two faces and vertices where surfaces touch get correct
//! shells; the new vertex is placed by the `CONSTRAINTS` (planes of the adjacent faces),
//! `FIXED` or `EVEN` method, boundary vertices optionally corrected (`ROUND`, `FLAT`), and rims
//! close the open boundaries and the splits.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_solidify_nonmanifold.cc
//! (MOD_solidify_nonmanifold_modifyMesh), Blender 5.2.4. Blender's pointer graph (face sides,
//! new edge refs, shared edge-to-face lists, per-vertex edge-group arrays ended by an invalid
//! entry) is kept as index arenas with the same layout, so every walk visits the same elements
//! in the same order.

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_scene::generated::modifier_settings::SolidifySettings;

use super::extrude::face_normals_true;
use super::math::*;
use crate::ModError;
use crate::types::weld::util::{
    self, Carry, DomainMaps, Topology, defgroup_index, defvert_find_weight,
};

const EMPTY: u32 = u32::MAX;
const PI_F64: f64 = std::f64::consts::PI;

/// `project_v3_v3`: removes from `r` its component along `a`; returns that component.
fn project_v3_v3(r: &mut V3, a: V3) -> f32 {
    let d = dot(*r, a);
    r[0] -= a[0] * d;
    r[1] -= a[1] * d;
    r[2] -= a[2] * d;
    d
}

fn angle_signed_on_axis_normalized_v3v3_v3(n: V3, ref_n: V3, axis: V3) -> f32 {
    let d = dot(n, ref_n).clamp(-1.0, 1.0);
    let mut angle = libm::acosf(d);
    let c = cross(n, ref_n);
    if dot(c, axis) >= 0.0 {
        angle = (2.0 * PI_F64 - f64::from(angle)) as f32;
    }
    angle
}

/// The closest value with `|value| >= epsilon`.
fn clamp_nonzero(value: f32, epsilon: f32) -> f32 {
    if value < 0.0 {
        value.min(-epsilon)
    } else {
        value.max(epsilon)
    }
}

/// `angle_v3v3v3`.
fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    angle_normalized_v3v3(normalized(sub(b, a)), normalized(sub(b, c)))
}

/// `invert_m3` (adjoint over determinant; unchanged adjoint when singular).
fn invert_m3(m: &mut [[f32; 3]; 3]) {
    let (m00, m01, m02) = (m[0][0], m[0][1], m[0][2]);
    let (m10, m11, m12) = (m[1][0], m[1][1], m[1][2]);
    let (m20, m21, m22) = (m[2][0], m[2][1], m[2][2]);
    let mut r = [
        [
            m11 * m22 - m12 * m21,
            -m01 * m22 + m02 * m21,
            m01 * m12 - m02 * m11,
        ],
        [
            -m10 * m22 + m12 * m20,
            m00 * m22 - m02 * m20,
            -m00 * m12 + m02 * m10,
        ],
        [
            m10 * m21 - m11 * m20,
            -m00 * m21 + m01 * m20,
            m00 * m11 - m01 * m10,
        ],
    ];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
    if det != 0.0 {
        let det = 1.0 / det;
        for row in &mut r {
            for v in row.iter_mut() {
                *v *= det;
            }
        }
    }
    *m = r;
}

/// `mul_v3_m3v3`.
fn mul_v3_m3v3(m: &[[f32; 3]; 3], a: V3) -> V3 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
        m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
    ]
}

fn xyz(p: &[f32; 4]) -> V3 {
    [p[0], p[1], p[2]]
}

/// `NewFaceRef`: one side of a face (`face_sides[2 i + reversed]`).
struct FaceSide {
    start: usize,
    len: usize,
    index: usize,
    reversed: bool,
    link_edges: Vec<Option<usize>>,
}

/// `OldEdgeFaceRef`: the faces around an edge (shared by edges merged into one).
#[derive(Clone)]
struct OldEdgeFaceRef {
    faces: Vec<u32>,
    faces_reversed: Vec<bool>,
    used: u32,
}

/// `OldVertEdgeRef`: the edges around a vertex (`edges.len()` is the allocated size).
struct OldVertEdgeRef {
    edges: Vec<u32>,
    edges_len: u32,
}

/// `NewEdgeRef`.
struct NewEdgeRef {
    old_edge: usize,
    faces: [Option<usize>; 2],
    /// (vertex, index in its edge-group array).
    link_edge_groups: [Option<(usize, usize)>; 2],
    angle: f32,
    new_edge: u32,
}

/// `EdgeGroup`.
#[derive(Clone, Default)]
struct EdgeGroup {
    valid: bool,
    edges: Vec<usize>,
    open_face_edge: u32,
    is_orig_closed: bool,
    is_even_split: bool,
    split: u32,
    is_singularity: bool,
    topo_group: u32,
    co: V3,
    no: V3,
    new_vert: u32,
}

impl EdgeGroup {
    fn new(edges: Vec<usize>, topo_group: u32) -> EdgeGroup {
        EdgeGroup {
            valid: true,
            edges,
            open_face_edge: EMPTY,
            is_orig_closed: true,
            is_even_split: false,
            split: 0,
            is_singularity: false,
            topo_group,
            co: [0.0; 3],
            no: [0.0; 3],
            new_vert: EMPTY,
        }
    }
}

/// `memmove(eg + dst, eg + src, count)` on a group array (growing it with invalid groups).
fn memmove_groups(eg: &mut Vec<EdgeGroup>, dst: usize, src: usize, count: usize) {
    if eg.len() < dst + count + 1 {
        eg.resize(dst + count + 1, EdgeGroup::default());
    }
    if dst > src {
        for t in (0..count).rev() {
            eg[dst + t] = eg[src + t].clone();
        }
    } else {
        for t in 0..count {
            eg[dst + t] = eg[src + t].clone();
        }
    }
}

/// The number of valid groups (they are contiguous from the start).
fn valid_len(eg: &[EdgeGroup]) -> usize {
    eg.iter().position(|g| !g.valid).unwrap_or(eg.len())
}

// Ported from blender/source/blender/modifiers/intern/MOD_solidify_nonmanifold.cc
// (MOD_solidify_nonmanifold_modifyMesh).
/// The solidified mesh; `totcol` is the object's material slot count.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn solidify_nonmanifold(
    s: &SolidifySettings,
    mesh: &Mesh,
    totcol: usize,
) -> Result<Mesh, ModError> {
    let verts_num = mesh.verts_num();
    let edges_num = mesh.edges_num();
    let faces_num = mesh.faces_num();
    if faces_num == 0 && verts_num != 0 {
        return Ok(mesh.clone());
    }

    // Material offsets only with 2 or more materials.
    let mat_nrs: i32 = if totcol > 1 { totcol as i32 } else { 1 };
    let mat_nr_max = mat_nrs - 1;
    let mat_ofs = if mat_nrs > 1 { s.material_offset } else { 0 };
    let mat_ofs_rim = if mat_nrs > 1 {
        s.material_offset_rim
    } else {
        0
    };

    // Offsets from the original surface along the positive (front) and negative (back) normal.
    let ofs_front = (s.offset + 1.0) * 0.5 * s.thickness;
    let ofs_back = ofs_front - s.thickness * s.offset;
    let ofs_front_clamped = clamp_nonzero(ofs_front, 1e-5);
    let ofs_back_clamped = clamp_nonzero(ofs_back, 1e-5);
    let offset_fac_vg = s.thickness_vertex_group;
    let offset_fac_vg_inv = 1.0 - s.thickness_vertex_group;
    let offset = s.thickness.abs() * s.thickness_clamp;
    let do_angle_clamp = s.use_thickness_angle_clamp;
    // Inverted with a negative thickness (simple solidify keeps the faces facing outside).
    let do_flip = s.use_flip_normals == (s.thickness > 0.0);
    let do_rim = s.use_rim;
    let do_shell = !(s.use_rim && s.use_rim_only);
    let do_clamp = s.thickness_clamp != 0.0;
    let bevel_convex = s.bevel_convex;
    let constraints = s.nonmanifold_thickness_mode == "CONSTRAINTS";
    let even_mode = s.nonmanifold_thickness_mode == "EVEN";
    let boundary_mode = s.nonmanifold_boundary_mode.as_str();

    let defgrp_invert = s.invert_vertex_group;
    let shell_defgrp_index = defgroup_index(mesh, &s.shell_vertex_group);
    let rim_defgrp_index = defgroup_index(mesh, &s.rim_vertex_group);
    let defgrp_index = defgroup_index(mesh, &s.vertex_group);
    let dvert = defgrp_index.and(mesh.deform_verts());
    let do_flat_faces = dvert.is_some() && s.use_flat_faces;
    let weight_of = |v: usize| -> f32 {
        match (dvert, defgrp_index) {
            (Some(dv), Some(g)) => defvert_find_weight(&dv[v], g),
            _ => 0.0,
        }
    };

    let orig_positions = mesh.positions();
    let orig_edges = mesh.edges();
    let orig_offsets = mesh.face_offsets();
    let orig_corner_verts = mesh.corner_verts();
    let orig_corner_edges = mesh.corner_edges();
    let face_range = |f: usize| orig_offsets[f] as usize..orig_offsets[f + 1] as usize;

    let orig_vert_bweight = util::float_attr(mesh, "bevel_weight_vert", Domain::Point);
    let orig_edge_bweight = util::float_attr(mesh, "bevel_weight_edge", Domain::Edge);
    let orig_edge_crease = util::float_attr(mesh, "crease_edge", Domain::Edge);

    let mut new_verts_num: u32 = 0;
    let mut new_edges_num: u32 = 0;
    let mut new_loops_num: u32 = 0;
    let mut new_faces_num: u32 = 0;

    // Face normals (modified below for faces without area).
    let mut face_nors = face_normals_true(mesh);
    let mut face_sides: Vec<FaceSide> = Vec::with_capacity(faces_num * 2);
    let mut null_faces = vec![false; faces_num];
    let mut largest_ngon = 3usize;
    for i in 0..faces_num {
        let f = face_range(i);
        if len_sq(face_nors[i]) < 0.5 {
            let edge = orig_edges[orig_corner_edges[f.start] as usize];
            let edgedir = sub(
                orig_positions[edge[1] as usize],
                orig_positions[edge[0] as usize],
            );
            if edgedir[2].abs() < edgedir[1].abs() {
                face_nors[i][2] = 1.0;
            } else {
                face_nors[i][1] = 1.0;
            }
            if constraints {
                null_faces[i] = true;
            }
        }
        for reversed in [false, true] {
            face_sides.push(FaceSide {
                start: f.start,
                len: f.len(),
                index: i,
                reversed,
                link_edges: vec![None; f.len()],
            });
        }
        largest_ngon = largest_ngon.max(f.len());
        if do_shell {
            new_faces_num += 2;
            new_loops_num += (f.len() * 2) as u32;
        }
    }
    let _ = largest_ngon;

    let mut edge_adj_faces_len = vec![0u32; edges_num];
    for &e in orig_corner_edges {
        edge_adj_faces_len[e as usize] += 1;
    }

    // Original edge → new edge list; original edge lengths; edge groups per original vertex.
    let mut edge_lists: Vec<Vec<usize>> = Vec::new();
    let mut orig_edge_data_arr: Vec<Option<usize>> = vec![None; edges_num];
    let mut orig_edge_lengths = vec![0.0f32; edges_num];
    let mut orig_vert_groups_arr: Vec<Option<Vec<EdgeGroup>>> = vec![None; verts_num];
    let mut vm: Vec<u32> = (0..verts_num as u32).collect();

    let mut edge_index: u32 = 0;
    let mut has_singularities = false;

    let mut vert_adj_edges: Vec<Option<OldVertEdgeRef>> = (0..verts_num).map(|_| None).collect();
    let mut orig_mvert_co: Vec<V3> = orig_positions.to_vec();
    let mut nedges: Vec<NewEdgeRef> = Vec::new();

    // Edge → faces map.
    {
        let mut refs: Vec<OldEdgeFaceRef> = Vec::new();
        let mut edge_adj_faces: Vec<Option<usize>> = vec![None; edges_num];
        for i in 0..faces_num {
            for corner in face_range(i) {
                let vert = orig_corner_verts[corner];
                let edge = orig_corner_edges[corner] as usize;
                let reversed = orig_edges[edge][1] != vert;
                match edge_adj_faces[edge] {
                    None => {
                        let len = edge_adj_faces_len[edge] as usize;
                        let mut faces = vec![EMPTY; len];
                        let mut faces_reversed = vec![false; len];
                        faces[0] = i as u32;
                        faces_reversed[0] = reversed;
                        refs.push(OldEdgeFaceRef {
                            faces,
                            faces_reversed,
                            used: 1,
                        });
                        edge_adj_faces[edge] = Some(refs.len() - 1);
                    }
                    Some(r) => {
                        let r = &mut refs[r];
                        for k in 1..r.faces.len() {
                            if r.faces[k] == EMPTY {
                                r.faces[k] = i as u32;
                                r.faces_reversed[k] = reversed;
                                break;
                            }
                        }
                    }
                }
            }
        }

        let mut vert_adj_edges_len = vec![0u32; verts_num];
        // Edge lengths, merging of short edges and zero faces.
        {
            let mut face_singularity = vec![false; faces_num];
            let merge_tolerance_sqr = s.nonmanifold_merge_threshold * s.nonmanifold_merge_threshold;
            let mut combined_verts = vec![0u32; verts_num];
            for i in 0..edges_num {
                let edge = orig_edges[i];
                if edge_adj_faces_len[i] == 0 {
                    continue;
                }
                let mut v1 = vm[edge[0] as usize];
                let mut v2 = vm[edge[1] as usize];
                if v1 == v2 {
                    continue;
                }
                if v2 < v1 {
                    std::mem::swap(&mut v1, &mut v2);
                }
                let mut edgedir = sub(orig_mvert_co[v2 as usize], orig_mvert_co[v1 as usize]);
                orig_edge_lengths[i] = len_sq(edgedir);
                if orig_edge_lengths[i] <= merge_tolerance_sqr {
                    // Merge, unless that would create a higher face count.
                    let in12 = |v: u32| v == v1 || v == v2;
                    let mut can_merge = true;
                    let mut k = 0;
                    while k < edges_num && can_merge {
                        if k != i
                            && edge_adj_faces_len[k] > 0
                            && (in12(vm[orig_edges[k][0] as usize])
                                != in12(vm[orig_edges[k][1] as usize]))
                        {
                            let r = edge_adj_faces[k].expect("an edge with faces has a list");
                            let mut j = 0;
                            while j < refs[r].faces.len() && can_merge {
                                let f = face_range(refs[r].faces[j] as usize);
                                let n = f.len();
                                let mut changes = 0u32;
                                // At least 3 unique vertices must remain (#150854).
                                let mut has_multiple_unique_others = false;
                                let mut unique_other_vert = EMPTY;
                                let mut cur = n - 1;
                                let mut next = 0;
                                while next < n && changes <= 2 {
                                    let cur_v = vm[orig_corner_verts[f.start + cur] as usize];
                                    let next_v = vm[orig_corner_verts[f.start + next] as usize];
                                    changes += u32::from(in12(cur_v) != in12(next_v));
                                    if !in12(cur_v) {
                                        if unique_other_vert == EMPTY {
                                            unique_other_vert = cur_v;
                                        } else if unique_other_vert != cur_v {
                                            has_multiple_unique_others = true;
                                        }
                                    }
                                    cur = next;
                                    next += 1;
                                }
                                can_merge = can_merge
                                    && changes <= 2
                                    && !(changes == 2 && !has_multiple_unique_others);
                                j += 1;
                            }
                        }
                        k += 1;
                    }
                    if !can_merge {
                        orig_edge_lengths[i] = 0.0;
                        vert_adj_edges_len[v1 as usize] += 1;
                        vert_adj_edges_len[v2 as usize] += 1;
                        continue;
                    }
                    let (c1, c2) = (combined_verts[v1 as usize], combined_verts[v2 as usize]);
                    edgedir = mul(edgedir, (c2 + 1) as f32 / (c1 + c2 + 2) as f32);
                    orig_mvert_co[v1 as usize] = add(orig_mvert_co[v1 as usize], edgedir);
                    for m in vm.iter_mut().skip(v2 as usize) {
                        if *m == v2 {
                            *m = v1;
                        }
                    }
                    vert_adj_edges_len[v1 as usize] += vert_adj_edges_len[v2 as usize];
                    vert_adj_edges_len[v2 as usize] = 0;
                    combined_verts[v1 as usize] += combined_verts[v2 as usize] + 1;
                    if do_shell {
                        new_loops_num = new_loops_num.wrapping_sub(edge_adj_faces_len[i] * 2);
                    }
                    edge_adj_faces_len[i] = 0;
                    edge_adj_faces[i] = None;
                } else {
                    orig_edge_lengths[i] = orig_edge_lengths[i].sqrt();
                    vert_adj_edges_len[v1 as usize] += 1;
                    vert_adj_edges_len[v2 as usize] += 1;
                }
            }
            // Remove zero faces in a second pass.
            for i in 0..edges_num {
                let edge = orig_edges[i];
                let v1 = vm[edge[0] as usize];
                let v2 = vm[edge[1] as usize];
                if v1 == v2
                    && let Some(r) = edge_adj_faces[i]
                {
                    for j in 0..refs[r].faces.len() {
                        let face = refs[r].faces[j] as usize;
                        if !face_singularity[face] {
                            let is_singularity = face_range(face)
                                .all(|c| vm[orig_corner_verts[c] as usize] == v1);
                            if is_singularity {
                                face_singularity[face] = true;
                                if do_shell {
                                    new_faces_num = new_faces_num.wrapping_sub(2);
                                }
                            }
                        }
                    }
                    if do_shell {
                        new_loops_num = new_loops_num.wrapping_sub(edge_adj_faces_len[i] * 2);
                    }
                    edge_adj_faces_len[i] = 0;
                    edge_adj_faces[i] = None;
                }
            }
        }

        // Vertex → edges.
        for i in 0..edges_num {
            if edge_adj_faces_len[i] == 0 {
                continue;
            }
            let edge = orig_edges[i];
            let vs = [vm[edge[0] as usize], vm[edge[1] as usize]];
            let mut invalid_edge_index: u32 = 0;
            let mut invalid_edge_reversed = false;
            for j in 0..2 {
                let vert = vs[j] as usize;
                let len = vert_adj_edges_len[vert] as usize;
                if len == 0 {
                    continue;
                }
                match vert_adj_edges[vert].as_mut() {
                    None => {
                        let mut edges = vec![EMPTY; len];
                        edges[0] = i as u32;
                        vert_adj_edges[vert] = Some(OldVertEdgeRef {
                            edges,
                            edges_len: 1,
                        });
                    }
                    Some(r) => {
                        let mut k = 0usize;
                        while k < len && k <= r.edges_len as usize {
                            let e = r.edges[k];
                            if e == EMPTY || k == r.edges_len as usize {
                                r.edges[k] = i as u32;
                                r.edges_len += 1;
                                break;
                            }
                            let oe = orig_edges[e as usize];
                            if vm[oe[0] as usize] == vs[1 - j] {
                                invalid_edge_index = e + 1;
                                invalid_edge_reversed = j == 0;
                                break;
                            }
                            if vm[oe[1] as usize] == vs[1 - j] {
                                invalid_edge_index = e + 1;
                                invalid_edge_reversed = j == 1;
                                break;
                            }
                            k += 1;
                        }
                        if invalid_edge_index != 0 {
                            if j == 1
                                && let Some(r0) = vert_adj_edges[vs[0] as usize].as_mut()
                            {
                                // Should never actually be executed.
                                r0.edges_len -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            // Remove zero faces in the shape of an edge.
            if invalid_edge_index != 0 {
                let existing = (invalid_edge_index - 1) as usize;
                let current = i;
                let (Some(ia), Some(ib)) = (edge_adj_faces[existing], edge_adj_faces[current])
                else {
                    continue;
                };
                let mut i_adj = refs[ia].clone();
                let mut invalid_adj = refs[ib].clone();
                let mut j = 0u32;
                for k in 0..i_adj.faces.len() {
                    for l in 0..invalid_adj.faces.len() {
                        if i_adj.faces[k] == invalid_adj.faces[l] && i_adj.faces[k] != EMPTY {
                            i_adj.faces[k] = EMPTY;
                            invalid_adj.faces[l] = EMPTY;
                            j += 1;
                        }
                    }
                }
                if do_shell {
                    new_faces_num = new_faces_num.wrapping_sub(2 * j);
                    new_loops_num = new_loops_num.wrapping_sub(4 * j);
                }
                let mut faces = Vec::new();
                let mut faces_reversed = Vec::new();
                for k in 0..i_adj.faces.len() {
                    if i_adj.faces[k] != EMPTY {
                        faces.push(i_adj.faces[k]);
                        faces_reversed.push(i_adj.faces_reversed[k]);
                    }
                }
                for k in 0..invalid_adj.faces.len() {
                    if invalid_adj.faces[k] != EMPTY {
                        faces.push(invalid_adj.faces[k]);
                        faces_reversed.push(invalid_edge_reversed != invalid_adj.faces_reversed[k]);
                    }
                }
                edge_adj_faces_len[current] = 0;
                edge_adj_faces_len[existing] = faces.len() as u32;
                let used = refs[ia].used + refs[ib].used;
                refs[ia] = OldEdgeFaceRef {
                    faces,
                    faces_reversed,
                    used,
                };
                edge_adj_faces[current] = Some(ia);
            }
        }

        // Filter duplicate faces (only the faces around one edge are compared).
        for i in 0..edges_num {
            if edge_adj_faces_len[i] == 0 {
                continue;
            }
            let Some(a) = edge_adj_faces[i] else {
                continue;
            };
            let mut adj_len = refs[a].faces.len();
            if adj_len <= 1 {
                continue;
            }
            let mut j = 0;
            while j < adj_len {
                let face = refs[a].faces[j] as usize;
                let jf = face_range(face);
                let (j_loopstart, totloop) = (jf.start, jf.len());
                let j_first_v = vm[orig_corner_verts[j_loopstart] as usize];
                let mut k = j + 1;
                while k < adj_len {
                    let kf = face_range(refs[a].faces[k] as usize);
                    if kf.len() != totloop {
                        k += 1;
                        continue;
                    }
                    let k_loopstart = kf.start;
                    // The first vertex of the first face in the second face.
                    let mut l = 0;
                    while l < totloop && vm[orig_corner_verts[k_loopstart + l] as usize] != j_first_v
                    {
                        l += 1;
                    }
                    if l == totloop {
                        k += 1;
                        continue;
                    }
                    // Do all following corners have equal vertices?
                    let reversed = refs[a].faces_reversed[j] != refs[a].faces_reversed[k];
                    let mut has_diff = false;
                    let mut n = (l + totloop) as i64;
                    for m in 0..totloop {
                        if has_diff {
                            break;
                        }
                        let vert = orig_corner_verts[j_loopstart + m];
                        has_diff = vm[vert as usize]
                            != vm[orig_corner_verts
                                [k_loopstart + (n as usize) % totloop]
                                as usize];
                        n += if reversed { -1 } else { 1 };
                    }
                    if !has_diff {
                        // Equal faces: discard one (j).
                        let mut del_loops = 0u32;
                        for m in 0..totloop {
                            let e = orig_corner_edges[j_loopstart + m] as usize;
                            let Some(er) = edge_adj_faces[e] else {
                                continue;
                            };
                            let mut face_index = j;
                            if er != a {
                                match refs[er].faces.iter().position(|&f| f as usize == face) {
                                    Some(p) => face_index = p,
                                    None => continue,
                                }
                            } else {
                                adj_len -= 1;
                            }
                            refs[er].faces.remove(face_index);
                            refs[er].faces_reversed.remove(face_index);
                            if edge_adj_faces_len[e] > 0 {
                                edge_adj_faces_len[e] -= 1;
                                if edge_adj_faces_len[e] == 0 {
                                    refs[er].used -= 1;
                                    edge_adj_faces[e] = None;
                                }
                            } else if refs[er].used > 1 {
                                for n2 in 0..edges_num {
                                    if edge_adj_faces[n2] == Some(er) && edge_adj_faces_len[n2] > 0
                                    {
                                        edge_adj_faces_len[n2] -= 1;
                                        if edge_adj_faces_len[n2] == 0 {
                                            refs[er].used -= 1;
                                            edge_adj_faces[n2] = None;
                                        }
                                        break;
                                    }
                                }
                            }
                            del_loops += 1;
                        }
                        if do_shell {
                            new_faces_num = new_faces_num.wrapping_sub(2);
                            new_loops_num = new_loops_num.wrapping_sub(2 * del_loops);
                        }
                        break;
                    }
                    k += 1;
                }
                j += 1;
            }
        }

        // `NewEdgeRef` lists.
        for i in 0..edges_num {
            if edge_adj_faces_len[i] == 0 {
                continue;
            }
            let edge = orig_edges[i];
            let v1 = vm[edge[0] as usize] as usize;
            let v2 = vm[edge[1] as usize] as usize;
            let edgedir: V3;
            if orig_edge_lengths[i] > f32::EPSILON {
                edgedir = mul(
                    sub(orig_mvert_co[v2], orig_mvert_co[v1]),
                    1.0 / orig_edge_lengths[i],
                );
            } else {
                // Smart fallback: merging is not essential, but gives better results.
                let pos = orig_mvert_co[v2];
                let dir_of = |v: usize| -> V3 {
                    let mut d = [0.0f32; 3];
                    if let Some(link) = vert_adj_edges[v].as_ref() {
                        for j in 0..link.edges_len as usize {
                            let e = link.edges[j] as usize;
                            if edge_adj_faces_len[e] > 0 && e != i {
                                let oe = orig_edges[e];
                                let other_v = vm[if vm[oe[0] as usize] as usize == v {
                                    oe[1]
                                } else {
                                    oe[0]
                                } as usize] as usize;
                                d = add(d, sub(orig_mvert_co[other_v], pos));
                            }
                        }
                    }
                    d
                };
                let (n, len) = normalize(sub(dir_of(v2), dir_of(v1)));
                edgedir = if len == 0.0 { [0.0, 0.0, 1.0] } else { n };
            }
            let Some(a) = edge_adj_faces[i] else {
                continue;
            };
            let adj_len = refs[a].faces.len();
            let mut sorted: Vec<(f32, usize)> = Vec::with_capacity(adj_len.max(1));
            let new_edges_len;
            if adj_len > 1 {
                new_edges_len = adj_len;
                // Keys for sorting.
                let mut ref_nor = [0.0f32; 3];
                for j in 0..adj_len {
                    let reverse = refs[a].faces_reversed[j];
                    let face_i = refs[a].faces[j] as usize;
                    let mut nor = if reverse {
                        negate(face_nors[face_i])
                    } else {
                        face_nors[face_i]
                    };
                    let mut d = 1.0f32;
                    if face_range(face_i).len() > 3 {
                        d = project_v3_v3(&mut nor, edgedir);
                        if d != 0.0 {
                            let (nn, l) = normalize(nor);
                            nor = nn;
                            d = l;
                        } else {
                            d = 1.0;
                        }
                    }
                    let angle = if d == 0.0 {
                        0.0
                    } else if j == 0 {
                        ref_nor = nor;
                        0.0
                    } else {
                        -angle_signed_on_axis_normalized_v3v3_v3(nor, ref_nor, edgedir)
                    };
                    sorted.push((angle, face_i * 2 + usize::from(reverse)));
                }
                // Sort the faces by their order around the edge.
                sorted.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                new_edges_len = 2;
                let face_i = refs[a].faces[0] as usize;
                sorted.push((0.0, face_i * 2 + usize::from(refs[a].faces_reversed[0])));
                if do_rim {
                    // Only the loops parallel to the edge for now.
                    new_loops_num = new_loops_num.wrapping_add(2);
                    new_faces_num = new_faces_num.wrapping_add(1);
                }
            }
            let list_id = edge_lists.len();
            let mut list: Vec<usize> = Vec::with_capacity(new_edges_len);
            for j in 0..new_edges_len {
                let faces: [Option<usize>; 2];
                let angle: f32;
                if adj_len > 1 {
                    let next_j = if j + 1 == adj_len { 0 } else { j + 1 };
                    faces = [Some(sorted[j].1), Some(sorted[next_j].1 ^ 1)];
                    let mut an = sorted[next_j].0 - sorted[j].0;
                    if an < 0.0 {
                        an = (f64::from(an) + 2.0 * PI_F64) as f32;
                    }
                    angle = an;
                } else {
                    // `face ± j`: the side itself, then the other side.
                    faces = [Some(if j == 0 { sorted[0].1 } else { sorted[0].1 ^ 1 }), None];
                    angle = 0.0;
                }
                let new_edge = if do_shell || (adj_len == 1 && do_rim) {
                    0
                } else {
                    EMPTY
                };
                nedges.push(NewEdgeRef {
                    old_edge: i,
                    faces,
                    link_edge_groups: [None, None],
                    angle,
                    new_edge,
                });
                let id = nedges.len() - 1;
                list.push(id);
                for fk in faces.into_iter().flatten() {
                    let fs = &face_sides[fk];
                    for l in 0..fs.len {
                        let e = orig_corner_edges[fs.start + l] as usize;
                        if edge_adj_faces[e] == edge_adj_faces[i] {
                            if e != i && orig_edge_data_arr[e].is_none() {
                                orig_edge_data_arr[e] = Some(list_id);
                            }
                            face_sides[fk].link_edges[l] = Some(id);
                            break;
                        }
                    }
                }
            }
            edge_lists.push(list);
            orig_edge_data_arr[i] = Some(list_id);
            if do_shell || (adj_len == 1 && do_rim) {
                new_edges_num = new_edges_num.wrapping_add(new_edges_len as u32);
            }
        }
    }

    // Sorted edge groups for every vertex.
    for i in 0..verts_num {
        let Some(adj) = vert_adj_edges[i].as_ref() else {
            continue;
        };
        if adj.edges_len < 2 {
            continue;
        }
        let mut eg_index: i64 = -1;
        let mut contains_long_groups = false;
        let mut topo_groups: u32 = 0;
        let mut edge_groups: Vec<EdgeGroup>;
        {
            // Initial sorted creation.
            let mut unassigned: Vec<Option<usize>> = Vec::new();
            for j in 0..adj.edges_len as usize {
                if let Some(l) = orig_edge_data_arr[adj.edges[j] as usize] {
                    unassigned.extend(edge_lists[l].iter().map(|&e| Some(e)));
                }
            }
            let unassigned_len = unassigned.len();
            // A group always has at least 2 edges.
            let edge_groups_len = unassigned_len / 2;
            edge_groups = vec![EdgeGroup::default(); edge_groups_len + 1];
            let mut assigned = 0usize;
            let mut found_edge_index = 0usize;
            let mut eg_track_faces: [Option<usize>; 2] = [None, None];
            let mut last_open_edge_track: Option<usize> = None;
            while assigned < unassigned_len {
                let mut found_edge: Option<usize> = None;
                let mut insert_at_start = false;
                if eg_index >= 0 && edge_groups[eg_index as usize].edges.is_empty() {
                    // A new group was started in the last iteration: find an unused edge to
                    // start it.
                    let egi = eg_index as usize;
                    let mut j = 0;
                    let mut edge: Option<usize> = None;
                    while edge.is_none() && j < unassigned_len {
                        edge = unassigned[j];
                        j += 1;
                        if let (Some(e), Some(track)) = (edge, last_open_edge_track)
                            && (nedges[e].faces[0] != Some(track) || nedges[e].faces[1].is_some())
                        {
                            edge = None;
                        }
                    }
                    if edge.is_none() && last_open_edge_track.is_some() {
                        topo_groups += 1;
                        last_open_edge_track = None;
                        edge_groups[egi].topo_group += 1;
                        j = 0;
                        while edge.is_none() && j < unassigned_len {
                            edge = unassigned[j];
                            j += 1;
                        }
                    } else if last_open_edge_track.is_none() && eg_index > 0 {
                        topo_groups += 1;
                        edge_groups[egi].topo_group += 1;
                    }
                    let Some(e) = edge else {
                        return Err(ModError::Error(
                            "Internal Error: solidify edge group without a start edge".into(),
                        ));
                    };
                    found_edge_index = j - 1;
                    found_edge = Some(e);
                    let ne = &nedges[e];
                    if last_open_edge_track.is_none()
                        && vm[orig_edges[ne.old_edge][0] as usize] as usize == i
                    {
                        eg_track_faces = [ne.faces[0], ne.faces[1]];
                        if ne.faces[1].is_none() {
                            last_open_edge_track = ne.faces[0].map(|f| f ^ 1);
                        }
                    } else {
                        eg_track_faces = [ne.faces[1], ne.faces[0]];
                    }
                } else if eg_index >= 0 {
                    let egi = eg_index as usize;
                    found_edge_index = 0;
                    while found_edge_index < unassigned_len {
                        if let Some(e) = unassigned[found_edge_index] {
                            let ne = &nedges[e];
                            if ne.faces[0] == eg_track_faces[1] {
                                insert_at_start = false;
                                eg_track_faces[1] = ne.faces[1];
                                found_edge = Some(e);
                                if ne.faces[1].is_none() {
                                    edge_groups[egi].is_orig_closed = false;
                                    last_open_edge_track = ne.faces[0].map(|f| f ^ 1);
                                }
                                break;
                            }
                            if ne.faces[0] == eg_track_faces[0] {
                                insert_at_start = true;
                                eg_track_faces[0] = ne.faces[1];
                                found_edge = Some(e);
                                if ne.faces[1].is_none() {
                                    edge_groups[egi].is_orig_closed = false;
                                }
                                break;
                            }
                            if ne.faces[1].is_some() {
                                if ne.faces[1] == eg_track_faces[1] {
                                    insert_at_start = false;
                                    eg_track_faces[1] = ne.faces[0];
                                    found_edge = Some(e);
                                    break;
                                }
                                if ne.faces[1] == eg_track_faces[0] {
                                    insert_at_start = true;
                                    eg_track_faces[0] = ne.faces[0];
                                    found_edge = Some(e);
                                    break;
                                }
                            }
                        }
                        found_edge_index += 1;
                    }
                }
                if let Some(e) = found_edge {
                    let egi = eg_index as usize;
                    unassigned[found_edge_index] = None;
                    assigned += 1;
                    let g = &mut edge_groups[egi];
                    if insert_at_start {
                        g.edges.insert(0, e);
                    } else {
                        g.edges.push(e);
                    }
                    if nedges[*g.edges.last().expect("just inserted")].faces[1].is_some() {
                        last_open_edge_track = None;
                    }
                    if g.edges.len() > 3 {
                        contains_long_groups = true;
                    }
                } else {
                    // The first iteration, or the current group is complete.
                    eg_index += 1;
                    let egi = eg_index as usize;
                    if egi >= edge_groups.len() {
                        edge_groups.resize(egi + 1, EdgeGroup::default());
                    }
                    edge_groups[egi] = EdgeGroup::new(Vec::new(), topo_groups);
                    eg_track_faces = [None, None];
                }
            }
            // `eg_index` is the number of groups from here on.
            eg_index += 1;
            // `topo_groups` is the number of topology groups from here on.
            topo_groups += 1;
            let _ = topo_groups;
        }
        let eg_count = eg_index as usize;
        if edge_groups.len() < eg_count + 1 {
            edge_groups.resize(eg_count + 1, EdgeGroup::default());
        }

        // Split long self-intersection groups.
        if contains_long_groups {
            let mut splits = 0u32;
            let mut add_index = 0usize;
            for j in 0..eg_count {
                let edges_len = edge_groups[j + add_index].edges.len();
                if edges_len <= 3 {
                    continue;
                }
                let g = edge_groups[j + add_index].clone();
                let mut doubles = vec![false; edges_len];
                let mut has_doubles = false;
                for k in 0..edges_len {
                    for l in k + 1..edges_len {
                        if nedges[g.edges[k]].old_edge == nedges[g.edges[l]].old_edge {
                            doubles[k] = true;
                            doubles[l] = true;
                            has_doubles = true;
                        }
                    }
                }
                if !has_doubles {
                    continue;
                }
                let prior_splits = splits;
                let prior_index = add_index;
                let mut unique_start: i64 = -1;
                let mut first_unique_end: i64 = -1;
                let mut last_split: i64 = -1;
                let mut first_split: i64 = -1;
                let mut first_even_split = false;
                let mut real_k = 0usize;
                let el = edges_len as i64;
                while real_k < edges_len
                    || (g.is_orig_closed
                        && ((real_k as i64)
                            <= (if first_unique_end == -1 { 0 } else { first_unique_end }) + el
                            || first_split != last_split))
                {
                    let k = real_k % edges_len;
                    if !doubles[k] {
                        if first_unique_end != -1 && unique_start == -1 {
                            unique_start = real_k as i64;
                        }
                    } else if first_unique_end == -1 {
                        first_unique_end = k as i64;
                    } else if unique_start != -1 {
                        let split = ((unique_start as usize + real_k + 1) / 2) % edges_len;
                        let is_even_split = ((unique_start as usize + real_k) & 1) != 0;
                        if last_split != -1 {
                            // Override g on the first split (no insert).
                            if prior_splits != splits {
                                memmove_groups(
                                    &mut edge_groups,
                                    j + add_index + 1,
                                    j + add_index,
                                    eg_count - j,
                                );
                                add_index += 1;
                            }
                            let ls = last_split as usize;
                            let edges: Vec<usize> = if ls > split {
                                g.edges[ls..]
                                    .iter()
                                    .chain(g.edges[..split].iter())
                                    .copied()
                                    .collect()
                            } else {
                                g.edges[ls..split].to_vec()
                            };
                            let mut ng = EdgeGroup::new(edges, g.topo_group);
                            ng.is_orig_closed = g.is_orig_closed;
                            ng.is_even_split = is_even_split;
                            ng.split = (add_index - prior_index + 1) as u32
                                + u32::from(!g.is_orig_closed);
                            if edge_groups.len() <= j + add_index + 1 {
                                edge_groups.resize(j + add_index + 2, EdgeGroup::default());
                            }
                            edge_groups[j + add_index] = ng;
                            splits += 1;
                        }
                        last_split = split as i64;
                        if first_split == -1 {
                            first_split = split as i64;
                            first_even_split = is_even_split;
                        }
                        unique_start = -1;
                    }
                    real_k += 1;
                }
                if first_split != -1 && !g.is_orig_closed {
                    if prior_splits != splits {
                        memmove_groups(
                            &mut edge_groups,
                            j + prior_index + 1,
                            j + prior_index,
                            eg_count + add_index - (j + prior_index),
                        );
                        memmove_groups(
                            &mut edge_groups,
                            j + add_index + 2,
                            j + add_index + 1,
                            eg_count - j,
                        );
                        add_index += 1;
                    } else {
                        memmove_groups(
                            &mut edge_groups,
                            j + add_index + 2,
                            j + add_index + 1,
                            eg_count - j - 1,
                        );
                    }
                    let fs = first_split as usize;
                    let mut ga = EdgeGroup::new(g.edges[..fs].to_vec(), g.topo_group);
                    ga.is_orig_closed = g.is_orig_closed;
                    ga.is_even_split = first_even_split;
                    ga.split = 1;
                    edge_groups[j + prior_index] = ga;
                    add_index += 1;
                    splits += 1;
                    let ls = last_split as usize;
                    let mut gb = EdgeGroup::new(g.edges[ls..].to_vec(), g.topo_group);
                    gb.is_orig_closed = g.is_orig_closed;
                    gb.is_even_split = false;
                    gb.split = (add_index - prior_index + 1) as u32;
                    if edge_groups.len() <= j + add_index + 1 {
                        edge_groups.resize(j + add_index + 2, EdgeGroup::default());
                    }
                    edge_groups[j + add_index] = gb;
                }
                if first_unique_end != -1 && prior_splits == splits {
                    has_singularities = true;
                    edge_groups[j + add_index].is_singularity = true;
                }
            }
        }

        // Count new edges, loops and faces; link the edges to their groups.
        {
            let n_valid = valid_len(&edge_groups);
            let mut new_verts = 0u32;
            let mut contains_open_splits = false;
            let mut open_edges = 0u32;
            let mut contains_splits = 0u32;
            let mut last_added = 0u32;
            let mut first_added = 0u32;
            let mut first_set = false;
            for gi in 0..n_valid {
                for &e in &edge_groups[gi].edges {
                    let flip =
                        usize::from(vm[orig_edges[nedges[e].old_edge][1] as usize] as usize == i);
                    nedges[e].link_edge_groups[flip] = Some((i, gi));
                }
                let g = &mut edge_groups[gi];
                let mut added = 0u32;
                if do_shell || (do_rim && !g.is_orig_closed) {
                    g.new_vert = new_verts_num;
                    new_verts_num += 1;
                    if do_rim || (do_shell && g.split != 0) {
                        new_verts += 1;
                        contains_splits += u32::from(g.split != 0);
                        contains_open_splits |= g.split != 0 && !g.is_orig_closed;
                        added = g.split;
                    }
                }
                open_edges += u32::from(added < last_added);
                if !first_set {
                    first_set = true;
                    first_added = added;
                }
                last_added = added;
                let topo = g.topo_group;
                if gi + 1 >= n_valid || topo != edge_groups[gi + 1].topo_group {
                    if new_verts > 2 {
                        new_faces_num = new_faces_num.wrapping_add(1);
                        new_edges_num = new_edges_num.wrapping_add(new_verts);
                        open_edges += u32::from(first_added < last_added);
                        open_edges -= u32::from(open_edges != 0 && !contains_open_splits);
                        if do_shell && do_rim {
                            new_loops_num = new_loops_num.wrapping_add(new_verts * 2);
                        } else if do_shell {
                            new_loops_num =
                                new_loops_num.wrapping_add((new_verts * 2).wrapping_sub(open_edges));
                        } else {
                            new_loops_num = new_loops_num.wrapping_add(
                                (new_verts * 2)
                                    .wrapping_add(open_edges)
                                    .wrapping_sub(contains_splits),
                            );
                        }
                    } else if new_verts == 2 {
                        new_edges_num = new_edges_num.wrapping_add(1);
                        new_loops_num = new_loops_num.wrapping_add(
                            2u32.wrapping_sub(u32::from(
                                !(do_rim && do_shell) && contains_open_splits,
                            )),
                        );
                    }
                    new_verts = 0;
                    contains_open_splits = false;
                    contains_splits = 0;
                    open_edges = 0;
                    last_added = 0;
                    first_added = 0;
                    first_set = false;
                }
            }
        }
        orig_vert_groups_arr[i] = Some(edge_groups);
    }
    drop(vert_adj_edges);

    // Edge-group vertex coordinates.
    {
        let face_weight: Vec<f32> = if do_flat_faces {
            (0..faces_num)
                .map(|i| {
                    let mut scalar_vgroup = 1.0f32;
                    for c in face_range(i) {
                        let w = weight_of(orig_corner_verts[c] as usize);
                        let w = if defgrp_invert { 1.0 - w } else { w };
                        scalar_vgroup = w.min(scalar_vgroup);
                    }
                    offset_fac_vg + (scalar_vgroup * offset_fac_vg_inv)
                })
                .collect()
        } else {
            Vec::new()
        };
        for i in 0..verts_num {
            let Some(mut groups) = orig_vert_groups_arr[i].take() else {
                continue;
            };
            let n_valid = valid_len(&groups);
            for g in groups.iter_mut().take(n_valid) {
                if g.is_singularity {
                    g.co = orig_mvert_co[i];
                    continue;
                }
                let mut nor: V3 = g.no;
                let mut move_nor: V3 = [0.0; 3];
                let mut disable_boundary_fix =
                    boundary_mode == "NONE" || g.is_orig_closed || g.split != 0;
                let mut approximate_free_direction = false;
                let edges_len = g.edges.len();
                let cycle = (g.is_orig_closed && g.split == 0) || g.is_even_split;
                if constraints {
                    // Planes `[nx, ny, nz, ofs]`.
                    let mut planes: Vec<[f32; 4]> = Vec::with_capacity(edges_len + 1);
                    let mut first_edge: Option<usize> = None;
                    let mut fallback_nor = [0.0f32; 3];
                    let mut fallback_ofs = 0.0f32;
                    for k in 0..edges_len {
                        if k & 1 == 0 || (!cycle && k == edges_len - 1) {
                            let e = g.edges[k];
                            for l in 0..2 {
                                let Some(face) = nedges[e].faces[l] else {
                                    continue;
                                };
                                if first_edge.is_some_and(|fe| {
                                    nedges[fe].faces[0] == Some(face)
                                        || nedges[fe].faces[1] == Some(face)
                                }) {
                                    continue;
                                }
                                let fs = &face_sides[face];
                                let mut ofs = if fs.reversed {
                                    ofs_back_clamped
                                } else {
                                    ofs_front_clamped
                                };
                                // `face_weight` makes faces thinner.
                                if do_flat_faces {
                                    ofs *= face_weight[fs.index];
                                }
                                let sign = if fs.reversed { -1.0 } else { 1.0 };
                                let n = mul(face_nors[fs.index], sign);
                                if !null_faces[fs.index] {
                                    planes.push([n[0], n[1], n[2], ofs]);
                                } else {
                                    // The approximate normal of a null face, when nothing else
                                    // is available.
                                    fallback_nor = n;
                                    fallback_ofs = ofs;
                                }
                            }
                            if (cycle && k == 0) || (!cycle && k + 3 >= edges_len) {
                                first_edge = Some(e);
                            }
                        }
                    }
                    let mut queue_index = planes.len();
                    if queue_index > 2 {
                        // The two most different normals.
                        let mut min_p = 2.0f32;
                        let mut min_n0 = 0;
                        let mut min_n1 = 0;
                        for k in 0..queue_index {
                            for m in k + 1..queue_index {
                                let p = dot(xyz(&planes[k]), xyz(&planes[m]));
                                if p < min_p {
                                    min_p = p;
                                    min_n0 = k;
                                    min_n1 = m;
                                }
                            }
                        }
                        // Put them first.
                        if min_n1 != 0 {
                            planes.swap(min_n0, 0);
                            planes.swap(min_n1, 1);
                        } else {
                            planes.swap(min_n0, 1);
                        }
                        // The third most important normal.
                        min_p = 1.0;
                        min_n1 = 2;
                        for k in 2..queue_index {
                            let max_p = dot(xyz(&planes[0]), xyz(&planes[k]))
                                .max(dot(xyz(&planes[1]), xyz(&planes[k])));
                            if max_p <= min_p {
                                min_p = max_p;
                                min_n1 = k;
                            }
                        }
                        planes.swap(min_n1, 2);
                    }
                    // Remove or average duplicate normals.
                    while queue_index > 2 {
                        let mut best_n0 = 0;
                        let mut best_n1 = 0;
                        let mut best_p = -1.0f32;
                        let mut best_ofs_diff = 0.0f32;
                        for k in 0..queue_index {
                            for m in k + 1..queue_index {
                                let p = dot(xyz(&planes[m]), xyz(&planes[k]));
                                let ofs_diff = (planes[m][3] - planes[k][3]).abs();
                                if p > best_p + f32::EPSILON
                                    || (p >= best_p && ofs_diff < best_ofs_diff)
                                {
                                    best_p = p;
                                    best_ofs_diff = ofs_diff;
                                    best_n0 = k;
                                    best_n1 = m;
                                }
                            }
                        }
                        // No equal planes: crucial for the numerical stability below.
                        if best_p < 0.98 {
                            break;
                        }
                        let merged = normalized(add(xyz(&planes[best_n0]), xyz(&planes[best_n1])));
                        let w = (planes[best_n0][3] + planes[best_n1][3]) * 0.5;
                        planes[best_n0] = [merged[0], merged[1], merged[2], w];
                        queue_index -= 1;
                        planes.remove(best_n1);
                    }
                    let size = queue_index;
                    // Allow ~10% thickness change for the boundary fix.
                    let boundary_fix_threshold = 0.7f32;
                    if size > 3 {
                        // The most general least squares method.
                        let mut mat = [[0.0f32; 3]; 3];
                        for (k, row) in mat.iter_mut().enumerate() {
                            for p in planes.iter().take(size) {
                                *row = madd(*row, xyz(p), p[k]);
                            }
                            // A small epsilon keeps the invert stable.
                            row[k] += 5e-5;
                        }
                        invert_m3(&mut mat);
                        nor = [0.0; 3];
                        for p in planes.iter().take(size) {
                            nor = madd(nor, xyz(p), p[3]);
                        }
                        nor = mul_v3_m3v3(&mat, nor);
                        if !disable_boundary_fix {
                            let mut greatest_angle_cos = 1.0f32;
                            for k in 0..2 {
                                for m in 2..size {
                                    let p = dot(xyz(&planes[m]), xyz(&planes[k]));
                                    greatest_angle_cos = p.min(greatest_angle_cos);
                                }
                            }
                            if greatest_angle_cos > boundary_fix_threshold {
                                approximate_free_direction = true;
                            } else {
                                disable_boundary_fix = true;
                            }
                        }
                    } else if size > 1 {
                        // Up to 3 constraint normals: a simple solution.
                        let stop_explosion = 0.999 - s.offset.abs() * 0.05;
                        let q = dot(xyz(&planes[0]), xyz(&planes[1]));
                        let mut d = 1.0 - q * q;
                        move_nor = normalized(cross(xyz(&planes[0]), xyz(&planes[1])));
                        let (w0, w1) = (planes[0][3], planes[1][3]);
                        let (p0, p1);
                        if d > f32::EPSILON * 10.0 && q < stop_explosion {
                            d = 1.0 / d;
                            p0 = mul(xyz(&planes[0]), (w0 - w1 * q) * d);
                            p1 = mul(xyz(&planes[1]), (w1 - w0 * q) * d);
                        } else {
                            d = 1.0 / (q.abs() + 1.0);
                            p0 = mul(xyz(&planes[0]), w0 * d);
                            p1 = mul(xyz(&planes[1]), w1 * d);
                        }
                        nor = add(p0, p1);
                        if size == 3 {
                            let d = dot(xyz(&planes[2]), move_nor);
                            // Ignore a third plane almost orthogonal to the free direction.
                            if d.abs() > 0.02 {
                                let tmp = madd(nor, xyz(&planes[2]), -planes[2][3]);
                                let tmp = mul(move_nor, dot(xyz(&planes[2]), tmp) / d);
                                nor = sub(nor, tmp);
                                // Disable the boundary fix if the constraints would be
                                // majorly unsatisfied.
                                if d.abs() > 1.0 - boundary_fix_threshold {
                                    disable_boundary_fix = true;
                                }
                            }
                        }
                        approximate_free_direction = false;
                    } else if size == 1 {
                        // Face corner case.
                        nor = mul(xyz(&planes[0]), planes[0][3]);
                        if edges_len > 2 {
                            disable_boundary_fix = true;
                            approximate_free_direction = true;
                        }
                    } else {
                        // Fallback for null faces.
                        nor = mul(fallback_nor, fallback_ofs);
                        disable_boundary_fix = true;
                    }
                } else {
                    // Fixed/Even method.
                    let mut total_angle = 0.0f32;
                    let mut total_angle_back = 0.0f32;
                    let mut first_edge: Option<usize> = None;
                    let mut nor_back = [0.0f32; 3];
                    let mut has_back = false;
                    let mut has_front = false;
                    for k in 0..edges_len {
                        if k & 1 == 0 || (!cycle && k == edges_len - 1) {
                            let e = g.edges[k];
                            for l in 0..2 {
                                let Some(face) = nedges[e].faces[l] else {
                                    continue;
                                };
                                if first_edge.is_some_and(|fe| {
                                    nedges[fe].faces[0] == Some(face)
                                        || nedges[fe].faces[1] == Some(face)
                                }) {
                                    continue;
                                }
                                let fs = &face_sides[face];
                                let mut angle = 1.0f32;
                                let mut ofs = if fs.reversed {
                                    -ofs_back_clamped
                                } else {
                                    ofs_front_clamped
                                };
                                if do_flat_faces {
                                    ofs *= face_weight[fs.index];
                                }
                                if even_mode {
                                    let mut corner_next = fs.start;
                                    let mut corner = corner_next + (fs.len - 1);
                                    let mut corner_prev = corner - 1;
                                    let mut m = 0;
                                    while m < fs.len
                                        && vm[orig_corner_verts[corner] as usize] as usize != i
                                    {
                                        corner_prev = corner;
                                        corner = corner_next;
                                        m += 1;
                                        corner_next += 1;
                                    }
                                    angle = angle_v3v3v3(
                                        orig_mvert_co
                                            [vm[orig_corner_verts[corner_prev] as usize] as usize],
                                        orig_mvert_co[i],
                                        orig_mvert_co
                                            [vm[orig_corner_verts[corner_next] as usize] as usize],
                                    );
                                    if fs.reversed {
                                        total_angle_back += angle * ofs * ofs;
                                    } else {
                                        total_angle += angle * ofs * ofs;
                                    }
                                } else if fs.reversed {
                                    total_angle_back += 1.0;
                                } else {
                                    total_angle += 1.0;
                                }
                                let face_nor = mul(face_nors[fs.index], angle * ofs);
                                if fs.reversed {
                                    nor_back = add(nor_back, face_nor);
                                    has_back = true;
                                } else {
                                    nor = add(nor, face_nor);
                                    has_front = true;
                                }
                            }
                            if (cycle && k == 0) || (!cycle && k + 3 >= edges_len) {
                                first_edge = Some(e);
                            }
                        }
                    }
                    // The normal length of the selected method.
                    if even_mode {
                        if has_front {
                            let length_sq = len_sq(nor);
                            if length_sq > f32::EPSILON {
                                nor = mul(nor, total_angle / length_sq);
                            }
                        }
                        if has_back {
                            let length_sq = len_sq(nor_back);
                            if length_sq > f32::EPSILON {
                                nor_back = mul(nor_back, total_angle_back / length_sq);
                            }
                            if !has_front {
                                nor = nor_back;
                            }
                        }
                        if has_front && has_back {
                            let nor_length = len(nor);
                            let nor_back_length = len(nor_back);
                            let mut q = dot(nor, nor_back);
                            if q.abs() > f32::EPSILON {
                                q /= nor_length * nor_back_length;
                            }
                            let mut d = 1.0 - q * q;
                            if d > f32::EPSILON {
                                d = 1.0 / d;
                                if nor_length > f32::EPSILON {
                                    nor = mul(nor, (1.0 - nor_back_length * q / nor_length) * d);
                                }
                                if nor_back_length > f32::EPSILON {
                                    nor_back = mul(
                                        nor_back,
                                        (1.0 - nor_length * q / nor_back_length) * d,
                                    );
                                }
                                nor = add(nor, nor_back);
                            } else {
                                nor = add(mul(nor, 0.5), mul(nor_back, 0.5));
                            }
                        }
                    } else {
                        if has_front && total_angle > f32::EPSILON {
                            nor = mul(nor, 1.0 / total_angle);
                        }
                        if has_back && total_angle_back > f32::EPSILON {
                            nor_back = mul(nor_back, 1.0 / total_angle_back);
                            nor = add(nor, nor_back);
                            if has_front && total_angle > f32::EPSILON {
                                nor = mul(nor, 0.5);
                            }
                        }
                    }
                    // `move_nor` for the boundary fix.
                    if !disable_boundary_fix && edges_len > 2 {
                        approximate_free_direction = true;
                    } else {
                        disable_boundary_fix = true;
                    }
                }
                if approximate_free_direction {
                    // `move_nor` for the boundary fix.
                    let mut k = 1usize;
                    while k + 1 < edges_len {
                        let edge = orig_edges[nedges[g.edges[k]].old_edge];
                        let other = if vm[edge[0] as usize] as usize == i {
                            edge[1]
                        } else {
                            edge[0]
                        };
                        move_nor = add(
                            move_nor,
                            sub(orig_mvert_co[other as usize], orig_mvert_co[i]),
                        );
                        k += 1;
                    }
                    if k == 1 {
                        disable_boundary_fix = true;
                    } else {
                        let (n, l) = normalize(move_nor);
                        move_nor = n;
                        disable_boundary_fix = l == 0.0;
                    }
                }
                // Fix boundary vertices.
                if !disable_boundary_fix {
                    let other_co = |e: usize| -> V3 {
                        let edge = orig_edges[nedges[e].old_edge];
                        let other = if vm[edge[0] as usize] as usize == i {
                            edge[1]
                        } else {
                            edge[0]
                        };
                        sub(orig_mvert_co[other as usize], orig_mvert_co[i])
                    };
                    let e0 = other_co(g.edges[0]);
                    let e1 = other_co(g.edges[edges_len - 1]);
                    let constr_nor = if boundary_mode == "FLAT" {
                        normalized(cross(e0, e1))
                    } else {
                        // ROUND.
                        let side_nor = |e: usize| -> V3 {
                            let fs = &face_sides[nedges[e].faces[0].expect("a face")];
                            if fs.reversed {
                                negate(face_nors[fs.index])
                            } else {
                                face_nors[fs.index]
                            }
                        };
                        let f0 = side_nor(g.edges[0]);
                        let f1 = side_nor(g.edges[edges_len - 1]);
                        let n0 = normalized(cross(e0, f0));
                        let n1 = normalized(cross(f1, e1));
                        normalized(add(n0, n1))
                    };
                    let d = dot(constr_nor, move_nor);
                    // Only allow the thickness to increase about 10 times.
                    if d.abs() > 0.1 {
                        move_nor = mul(move_nor, dot(constr_nor, nor) / d);
                        nor = sub(nor, move_nor);
                    }
                }
                let mut scalar_vgroup = 1.0f32;
                // Vertex group.
                if dvert.is_some() && !do_flat_faces {
                    let w = weight_of(i);
                    let w = if defgrp_invert { 1.0 - w } else { w };
                    scalar_vgroup = offset_fac_vg + (w * offset_fac_vg_inv);
                }
                // Clamping.
                if do_clamp {
                    if do_angle_clamp {
                        if edges_len > 2 {
                            let mut min_length = 0.0f32;
                            let mut angle = (0.5 * PI_F64) as f32;
                            for (k, &e) in g.edges.iter().enumerate() {
                                let length = orig_edge_lengths[nedges[e].old_edge];
                                angle = nedges[e].angle.max(angle);
                                if length < min_length || k == 0 {
                                    min_length = length;
                                }
                            }
                            let cos_ang = libm::cosf(angle * 0.5);
                            if cos_ang > 0.0 {
                                let max_off = min_length * 0.5 / cos_ang;
                                if max_off < offset * 0.5 {
                                    scalar_vgroup *= max_off / offset * 2.0;
                                }
                            }
                        }
                    } else {
                        let mut min_length = 0.0f32;
                        for (k, &e) in g.edges.iter().enumerate() {
                            let length = orig_edge_lengths[nedges[e].old_edge];
                            if length < min_length || k == 0 {
                                min_length = length;
                            }
                        }
                        if min_length < offset {
                            scalar_vgroup *= min_length / offset;
                        }
                    }
                }
                nor = mul(nor, scalar_vgroup);
                g.no = nor;
                g.co = add(nor, orig_mvert_co[i]);
            }
            orig_vert_groups_arr[i] = Some(groups);
        }
    }

    let group = |gs: &Vec<Option<Vec<EdgeGroup>>>, link: Option<(usize, usize)>| -> Option<EdgeGroup> {
        let (v, gi) = link?;
        gs[v].as_ref().map(|g| g[gi].clone())
    };
    let link_singular = |gs: &Vec<Option<Vec<EdgeGroup>>>, link: Option<(usize, usize)>| -> bool {
        group(gs, link).is_some_and(|g| g.is_singularity)
    };
    let link_new_vert = |gs: &Vec<Option<Vec<EdgeGroup>>>, link: Option<(usize, usize)>| -> u32 {
        group(gs, link).map_or(EMPTY, |g| g.new_vert)
    };
    let link_open_face_edge =
        |gs: &Vec<Option<Vec<EdgeGroup>>>, link: Option<(usize, usize)>| -> u32 {
            group(gs, link).map_or(EMPTY, |g| g.open_face_edge)
        };

    // Corrections for adjacent one-sided groups around a vertex (no edge duplicates or null
    // faces).
    let mut singularity_edges: Vec<[u32; 2]> = Vec::new();
    if has_singularities {
        has_singularities = false;
        for i in 0..edges_num {
            let Some(l) = orig_edge_data_arr[i] else {
                continue;
            };
            if !((do_shell || edge_adj_faces_len[i] == 1) && nedges[edge_lists[l][0]].old_edge == i)
            {
                continue;
            }
            for &e in &edge_lists[l] {
                let lk = nedges[e].link_edge_groups;
                if link_singular(&orig_vert_groups_arr, lk[0])
                    && link_singular(&orig_vert_groups_arr, lk[1])
                {
                    let v1 = link_new_vert(&orig_vert_groups_arr, lk[0]);
                    let v2 = link_new_vert(&orig_vert_groups_arr, lk[1]);
                    let exists_already = singularity_edges
                        .iter()
                        .any(|p| (p[0] == v1 && p[1] == v2) || (p[0] == v2 && p[1] == v1));
                    if !exists_already {
                        has_singularities = true;
                        singularity_edges.push([v1, v2]);
                        if edge_adj_faces_len[i] == 1 && do_rim {
                            new_loops_num = new_loops_num.wrapping_sub(2);
                            new_faces_num = new_faces_num.wrapping_sub(1);
                        }
                    } else {
                        new_edges_num = new_edges_num.wrapping_sub(1);
                    }
                }
            }
        }
    }
    let totsingularity = singularity_edges.len() as u32;

    // The result.
    let nv = new_verts_num as usize;
    let ne_total = new_edges_num as usize;
    if ne_total > 1 << 28 || new_loops_num as usize > 1 << 28 || new_faces_num as usize > 1 << 28 {
        return Err(ModError::Error(
            "Internal Error: solidify element counts out of range".into(),
        ));
    }
    let mut positions = vec![[0.0f32; 3]; nv];
    let mut vmap: Vec<Option<usize>> = vec![None; nv];
    let mut edges = vec![[0i32; 2]; ne_total];
    let mut emap: Vec<Option<usize>> = vec![None; ne_total];
    let mut face_offsets: Vec<i32> = Vec::with_capacity(new_faces_num as usize + 1);
    let mut fmap: Vec<Option<usize>> = Vec::with_capacity(new_faces_num as usize);
    let mut material: Vec<i32> = Vec::with_capacity(new_faces_num as usize);
    let mut corner_verts: Vec<i32> = Vec::with_capacity(new_loops_num as usize);
    let mut corner_edges: Vec<i32> = Vec::with_capacity(new_loops_num as usize);
    let mut cmap: Vec<Option<usize>> = Vec::with_capacity(new_loops_num as usize);
    let src_material_index = util::int_attr(mesh, "material_index", Domain::Face);
    let src_mat = |f: usize| src_material_index.map_or(0, |m| m[f]);
    let want_bweight = mesh.attributes().get("bevel_weight_edge").is_some()
        || bevel_convex != 0.0
        || orig_vert_bweight.is_some();
    let mut bweight: Option<Vec<f32>> = want_bweight.then(|| vec![0.0; ne_total]);
    let vertex_crease = util::float_attr(mesh, "crease_vert", Domain::Point);
    let mut crease: Option<Vec<f32>> =
        (vertex_crease.is_some() || orig_edge_crease.is_some()).then(|| vec![0.0; ne_total]);
    let use_dvert = shell_defgrp_index.is_some() || rim_defgrp_index.is_some();
    let mut dst_dvert_add: Vec<(usize, u32)> = Vec::new();
    let out_of_range = || ModError::Error("Internal Error: edges array wrong size".into());

    // New vertices.
    for i in 0..verts_num {
        if let Some(gs) = &orig_vert_groups_arr[i] {
            for g in gs.iter().take_while(|g| g.valid) {
                if g.new_vert != EMPTY {
                    let v = g.new_vert as usize;
                    vmap[v] = Some(i);
                    positions[v] = g.co;
                }
            }
        }
    }

    // Edges.
    edge_index += totsingularity;
    for i in 0..edges_num {
        let Some(l) = orig_edge_data_arr[i] else {
            continue;
        };
        if !((do_shell || edge_adj_faces_len[i] == 1) && nedges[edge_lists[l][0]].old_edge == i) {
            continue;
        }
        for &e in &edge_lists[l] {
            if nedges[e].new_edge == EMPTY {
                continue;
            }
            let lk = nedges[e].link_edge_groups;
            let v1 = link_new_vert(&orig_vert_groups_arr, lk[0]);
            let v2 = link_new_vert(&orig_vert_groups_arr, lk[1]);
            let mut insert = edge_index;
            if has_singularities
                && link_singular(&orig_vert_groups_arr, lk[0])
                && link_singular(&orig_vert_groups_arr, lk[1])
            {
                if let Some(j) = singularity_edges
                    .iter()
                    .position(|p| (p[0] == v1 && p[1] == v2) || (p[0] == v2 && p[1] == v1))
                {
                    insert = j as u32;
                }
            } else {
                edge_index += 1;
            }
            let ins = insert as usize;
            if ins >= ne_total {
                return Err(out_of_range());
            }
            emap[ins] = Some(i);
            edges[ins] = [v1 as i32, v2 as i32];
            if let Some(c) = crease.as_mut() {
                c[ins] = orig_edge_crease.map_or(0.0, |oc| oc[nedges[e].old_edge]);
            }
            if let Some(bw) = bweight.as_mut() {
                bw[ins] = orig_edge_bweight.map_or(0.0, |ob| ob[nedges[e].old_edge]);
                if bevel_convex != 0.0 && nedges[e].faces[1].is_some() {
                    let a = f64::from(nedges[e].angle);
                    let eps = f64::from(f32::EPSILON);
                    bw[ins] = (bw[ins]
                        + if a > PI_F64 + eps {
                            bevel_convex.clamp(0.0, 1.0)
                        } else if a < PI_F64 - eps {
                            bevel_convex.clamp(-1.0, 0.0)
                        } else {
                            0.0
                        })
                    .clamp(0.0, 1.0);
                }
            }
            nedges[e].new_edge = insert;
        }
    }

    // Boundary edges and faces (around the vertices).
    for i in 0..verts_num {
        let Some(mut gs) = orig_vert_groups_arr[i].take() else {
            continue;
        };
        let n_valid = valid_len(&gs);
        let mv_crease = vertex_crease.map_or(0.0, |c| c[i]);
        let mv_bweight = orig_vert_bweight.map_or(0.0, |b| b[i]);
        let mut last_g: Option<usize> = None;
        let mut first_g: Option<usize> = None;
        let mut last_max_crease = 0.0f32;
        let mut first_max_crease = 0.0f32;
        let mut last_max_bweight = 0.0f32;
        let mut first_max_bweight = 0.0f32;
        let mut g2 = 0usize;
        let mut j = 0u32;
        for gi in 0..n_valid {
            let is_open = |g: &EdgeGroup| (do_rim && !g.is_orig_closed) || (do_shell && g.split != 0);
            if is_open(&gs[gi]) {
                let g = &gs[gi];
                let mut max_crease = 0.0f32;
                let mut max_bweight = 0.0f32;
                let el = g.edges.len();
                if el == 2 {
                    if crease.is_some() {
                        max_crease = orig_edge_crease.map_or(0.0, |oc| {
                            oc[nedges[g.edges[0]].old_edge].min(oc[nedges[g.edges[1]].old_edge])
                        });
                    }
                } else {
                    for k in 1..el.saturating_sub(1) {
                        let orig_edge_index = nedges[g.edges[k]].old_edge;
                        if crease.is_some()
                            && let Some(oc) = orig_edge_crease
                            && oc[orig_edge_index] > max_crease
                        {
                            max_crease = oc[orig_edge_index];
                        }
                        let ne_k = nedges[g.edges[k]].new_edge;
                        if ne_k != EMPTY
                            && let Some(bw) = bweight.as_ref()
                        {
                            max_bweight = bw[ne_k as usize].max(max_bweight);
                        }
                    }
                }
                let bweight_open_edge = orig_edge_bweight.map_or(0.0, |ob| {
                    ob[nedges[g.edges[0]].old_edge].min(ob[nedges[g.edges[el - 1]].old_edge])
                });
                if bweight_open_edge > 0.0 {
                    max_bweight = bweight_open_edge.min(max_bweight);
                } else if bevel_convex < 0.0 {
                    max_bweight = 0.0;
                }
                if first_g.is_none() {
                    first_g = Some(gi);
                    first_max_crease = max_crease;
                    first_max_bweight = max_bweight;
                } else {
                    let lg = last_g.expect("set with first_g");
                    let ei = edge_index as usize;
                    if ei >= ne_total {
                        return Err(out_of_range());
                    }
                    gs[lg].open_face_edge = edge_index;
                    emap[ei] = Some(nedges[gs[lg].edges[0]].old_edge);
                    edges[ei] = [gs[lg].new_vert as i32, gs[gi].new_vert as i32];
                    if let Some(c) = crease.as_mut() {
                        c[ei] = mv_crease.max(last_max_crease.min(max_crease));
                    }
                    if let Some(bw) = bweight.as_mut() {
                        bw[ei] = mv_bweight.max(last_max_bweight.min(max_bweight));
                    }
                    edge_index += 1;
                }
                last_g = Some(gi);
                last_max_crease = max_crease;
                last_max_bweight = max_bweight;
                j += 1;
            }
            if gi + 1 >= n_valid || gs[gi].topo_group != gs[gi + 1].topo_group {
                if j == 2 {
                    gs[last_g.expect("two groups")].open_face_edge = edge_index - 1;
                }
                if j > 2 {
                    let lg = last_g.expect("groups");
                    let fg = first_g.expect("groups");
                    let ei = edge_index as usize;
                    if ei >= ne_total {
                        return Err(out_of_range());
                    }
                    emap[ei] = Some(nedges[gs[lg].edges[0]].old_edge);
                    gs[lg].open_face_edge = edge_index;
                    edges[ei] = [gs[lg].new_vert as i32, gs[fg].new_vert as i32];
                    if let Some(c) = crease.as_mut() {
                        c[ei] = mv_crease.max(last_max_crease.min(first_max_crease));
                    }
                    if let Some(bw) = bweight.as_mut() {
                        bw[ei] = mv_bweight.max(last_max_bweight.min(first_max_bweight));
                    }
                    edge_index += 1;

                    // The material index by consensus.
                    let mut most_mat_nr = 0i32;
                    let mut most_mat_nr_face = 0usize;
                    let mut most_mat_nr_count = 0u32;
                    for l in 0..mat_nrs {
                        let mut count = 0u32;
                        let mut face = 0usize;
                        let mut k = 0;
                        let mut g3 = g2;
                        while g3 < n_valid && k < j {
                            let g = &gs[g3];
                            if is_open(g) {
                                // Both far ends (in faces) of the group.
                                let f0 = face_sides[nedges[g.edges[0]].faces[0].expect("face")].index;
                                if src_mat(f0) == l {
                                    face = f0;
                                    count += 1;
                                }
                                let le = &nedges[g.edges[g.edges.len() - 1]];
                                if let Some(f1) = le.faces[1] {
                                    let f1 = face_sides[f1].index;
                                    if src_mat(f1) == l {
                                        face = f1;
                                        count += 1;
                                    }
                                } else {
                                    let fl = face_sides[le.faces[0].expect("face")].index;
                                    if src_mat(fl) == l {
                                        face = fl;
                                        count += 1;
                                    }
                                }
                                k += 1;
                            }
                            g3 += 1;
                        }
                        if count > most_mat_nr_count {
                            most_mat_nr = l;
                            most_mat_nr_face = face;
                            most_mat_nr_count = count;
                        }
                    }
                    fmap.push(Some(most_mat_nr_face));
                    face_offsets.push(corner_verts.len() as i32);
                    let mat = most_mat_nr
                        + if gs[gi].is_orig_closed || !do_rim {
                            0
                        } else {
                            mat_ofs_rim
                        };
                    material.push(mat.clamp(0, mat_nr_max));

                    let mut loops_data: Vec<usize> = vec![0; j as usize];
                    let mut k = 0usize;
                    while g2 < n_valid && k < j as usize {
                        let g = &gs[g2];
                        if is_open(g) {
                            let fs = &face_sides[nedges[g.edges[0]].faces[0].expect("face")];
                            for c in fs.start..fs.start + fs.len {
                                if vm[orig_corner_verts[c] as usize] as usize == i {
                                    loops_data[k] = c;
                                    break;
                                }
                            }
                            k += 1;
                        }
                        g2 += 1;
                    }
                    let ju = j as usize;
                    let ei = edge_index as usize;
                    if !do_flip {
                        for (k, &ld) in loops_data.iter().enumerate() {
                            cmap.push(Some(ld));
                            corner_verts.push(edges[ei - ju + k][0]);
                            corner_edges.push((ei - ju + k) as i32);
                        }
                    } else {
                        for k in 1..=ju {
                            cmap.push(Some(loops_data[ju - k]));
                            corner_verts.push(edges[ei - k][1]);
                            corner_edges.push((ei - k) as i32);
                        }
                    }
                }
                // Reset for the next face.
                j = 0;
                last_g = None;
                first_g = None;
                last_max_crease = 0.0;
                first_max_crease = 0.0;
                last_max_bweight = 0.0;
                first_max_bweight = 0.0;
            }
        }
        orig_vert_groups_arr[i] = Some(gs);
    }

    let open_face_edge = |gs: &Vec<Option<Vec<EdgeGroup>>>, l: Option<(usize, usize)>| -> u32 {
        link_open_face_edge(gs, l)
    };

    // Boundary faces (the rim along open edges).
    if do_rim {
        for i in 0..edges_num {
            let Some(l) = orig_edge_data_arr[i] else {
                continue;
            };
            if !(edge_adj_faces_len[i] == 1 && nedges[edge_lists[l][0]].old_edge == i) {
                continue;
            }
            let edge1 = &nedges[edge_lists[l][0]];
            let edge2 = &nedges[edge_lists[l][1]];
            let gsr = &orig_vert_groups_arr;
            let v1_singularity = link_singular(gsr, edge1.link_edge_groups[0])
                && link_singular(gsr, edge2.link_edge_groups[0]);
            let v2_singularity = link_singular(gsr, edge1.link_edge_groups[1])
                && link_singular(gsr, edge2.link_edge_groups[1]);
            if v1_singularity && v2_singularity {
                continue;
            }
            let fs = &face_sides[edge1.faces[0].expect("face")];
            let orig_face_index = fs.index;
            fmap.push(Some(orig_face_index));
            face_offsets.push(corner_verts.len() as i32);
            material.push((src_mat(orig_face_index) + mat_ofs_rim).clamp(0, mat_nr_max));
            let mut loop1: usize = usize::MAX;
            let mut loop2: usize = usize::MAX;
            let oe = orig_edges[edge1.old_edge];
            let old_v1 = vm[oe[0] as usize];
            let old_v2 = vm[oe[1] as usize];
            for c in fs.start..fs.start + fs.len {
                let vert = vm[orig_corner_verts[c] as usize];
                if vert == old_v1 {
                    loop1 = c;
                } else if vert == old_v2 {
                    loop2 = c;
                }
            }
            if loop1 == usize::MAX || loop2 == usize::MAX {
                return Err(ModError::Error(
                    "Internal Error: solidify rim face without its edge corners".into(),
                ));
            }
            if edge1.new_edge as usize >= ne_total || edge2.new_edge as usize >= ne_total {
                return Err(out_of_range());
            }
            let e1 = edges[edge1.new_edge as usize];
            let e2 = edges[edge2.new_edge as usize];
            let e1i = edge1.new_edge as i32;
            let e2i = edge2.new_edge as i32;
            let mut push = |cm: usize, v: i32, e: i32, rim: bool| {
                if rim && let Some(gidx) = rim_defgrp_index {
                    dst_dvert_add.push((v as usize, gidx));
                }
                cmap.push(Some(cm));
                corner_verts.push(v);
                corner_edges.push(e);
            };
            let pick = |idx: u32, other: i32, fallback: u32| -> i32 {
                let Some(ofe) = edges.get(idx as usize) else {
                    return fallback as i32;
                };
                if other == ofe[0] || other == ofe[1] {
                    idx as i32
                } else {
                    fallback as i32
                }
            };
            if !do_flip {
                push(loop1, e1[0], e1i, true);
                if !v2_singularity {
                    let ofi = open_face_edge(gsr, edge1.link_edge_groups[1]);
                    let e = pick(ofi, e2[1], open_face_edge(gsr, edge2.link_edge_groups[1]));
                    push(loop2, e1[1], e, true);
                }
                push(loop2, e2[1], e2i, true);
                if !v1_singularity {
                    let ofi = open_face_edge(gsr, edge2.link_edge_groups[0]);
                    let e = pick(ofi, e1[0], open_face_edge(gsr, edge1.link_edge_groups[0]));
                    push(loop1, e2[0], e, true);
                }
            } else {
                if !v1_singularity {
                    let ofi = open_face_edge(gsr, edge1.link_edge_groups[0]);
                    let e = pick(ofi, e2[0], open_face_edge(gsr, edge2.link_edge_groups[0]));
                    push(loop1, e1[0], e, true);
                }
                push(loop1, e2[0], e2i, true);
                if !v2_singularity {
                    let ofi = open_face_edge(gsr, edge2.link_edge_groups[1]);
                    let e = pick(ofi, e1[1], open_face_edge(gsr, edge1.link_edge_groups[1]));
                    push(loop2, e2[1], e, true);
                }
                push(loop2, e1[1], e1i, true);
            }
        }
    }

    // The shell faces.
    if do_shell {
        for i in 0..faces_num * 2 {
            let fr = &face_sides[i];
            let loopstart = fr.start;
            let mut totloop = fr.len;
            let valid_link = |l: Option<usize>| l.is_some_and(|e| nedges[e].new_edge != EMPTY);
            while totloop > 0 && !valid_link(fr.link_edges[totloop - 1]) {
                totloop -= 1;
            }
            if totloop == 0 {
                continue;
            }
            let mut face_loops: Vec<usize> = Vec::new();
            let mut face_verts: Vec<u32> = Vec::new();
            let mut face_edges: Vec<u32> = Vec::new();
            let mut valid_edges = 0u32;
            let mut prior_edge = fr.link_edges[totloop - 1].expect("valid");
            let mut prior_flip = usize::from(
                vm[orig_edges[nedges[prior_edge].old_edge][0] as usize]
                    == vm[orig_corner_verts[loopstart + (totloop - 1)] as usize],
            );
            for j in 0..totloop {
                let Some(new_edge) = fr.link_edges[j] else {
                    continue;
                };
                if nedges[new_edge].new_edge == EMPTY {
                    continue;
                }
                valid_edges += 1;
                let ne = &nedges[new_edge];
                let flip = usize::from(
                    vm[orig_edges[ne.old_edge][1] as usize]
                        == vm[orig_corner_verts[loopstart + j] as usize],
                );
                // The vertex in the current loop, and in the next one.
                let new_v1 = link_new_vert(&orig_vert_groups_arr, ne.link_edge_groups[flip]);
                let new_v2 = link_new_vert(&orig_vert_groups_arr, ne.link_edge_groups[1 - flip]);
                if face_verts.is_empty() || *face_verts.last().expect("non-empty") != new_v1 {
                    face_loops.push(loopstart + j);
                    let fe = if fr.reversed {
                        link_open_face_edge(
                            &orig_vert_groups_arr,
                            nedges[prior_edge].link_edge_groups[prior_flip],
                        )
                    } else {
                        link_open_face_edge(&orig_vert_groups_arr, ne.link_edge_groups[flip])
                    };
                    face_edges.push(fe);
                    face_verts.push(new_v1);
                }
                prior_edge = new_edge;
                prior_flip = 1 - flip;
                if j < totloop - 1 || face_verts[0] != new_v2 {
                    face_loops.push(loopstart + (j + 1) % totloop);
                    face_edges.push(ne.new_edge);
                    face_verts.push(new_v2);
                } else {
                    face_edges[0] = ne.new_edge;
                }
            }
            let k = face_verts.len();
            if k > 2 && valid_edges > 2 {
                fmap.push(Some(i / 2));
                face_offsets.push(corner_verts.len() as i32);
                let mat = src_mat(fr.index) + if fr.reversed != do_flip { mat_ofs } else { 0 };
                material.push(mat.clamp(0, mat_nr_max));
                if fr.reversed != do_flip {
                    for l in (0..k).rev() {
                        if let Some(gidx) = shell_defgrp_index {
                            dst_dvert_add.push((face_verts[l] as usize, gidx));
                        }
                        cmap.push(Some(face_loops[l]));
                        corner_verts.push(face_verts[l] as i32);
                        corner_edges.push(face_edges[l] as i32);
                    }
                } else {
                    let mut l = k - 1;
                    for next_l in 0..k {
                        cmap.push(Some(face_loops[l]));
                        corner_verts.push(face_verts[l] as i32);
                        corner_edges.push(face_edges[next_l] as i32);
                        l = next_l;
                    }
                }
            }
        }
    }

    if edge_index != new_edges_num {
        return Err(ModError::Error(format!(
            "Internal Error: edges array wrong size: {new_edges_num} instead of {edge_index}"
        )));
    }
    if fmap.len() != new_faces_num as usize {
        return Err(ModError::Error(format!(
            "Internal Error: faces array wrong size: {new_faces_num} instead of {}",
            fmap.len()
        )));
    }
    if cmap.len() != new_loops_num as usize {
        return Err(ModError::Error(format!(
            "Internal Error: loops array wrong size: {new_loops_num} instead of {}",
            cmap.len()
        )));
    }
    face_offsets.push(corner_verts.len() as i32);

    let mut result = util::rebuild(
        mesh,
        Topology {
            positions,
            edges,
            face_offsets,
            corner_verts,
            corner_edges,
        },
        &DomainMaps {
            vert: Carry::Gather(vmap),
            edge: Carry::Gather(emap),
            face: Carry::Gather(fmap),
            corner: Carry::Gather(cmap),
        },
    );
    if use_dvert && result.verts_num() > 0 {
        let dv = result.deform_verts_mut();
        for (v, g) in dst_dvert_add {
            match dv[v].weights.iter_mut().find(|w| w.0 == g) {
                Some(w) => w.1 = 1.0,
                None => dv[v].weights.push((g, 1.0)),
            }
        }
    }
    let attrs = result.attributes_mut();
    attrs.insert(Attribute {
        name: "material_index".into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(material)),
    });
    if let Some(bw) = bweight {
        attrs.insert(Attribute {
            name: "bevel_weight_edge".into(),
            domain: Domain::Edge,
            data: AttrData::Float(Arc::new(bw)),
        });
    }
    if let Some(c) = crease {
        attrs.insert(Attribute {
            name: "crease_edge".into(),
            domain: Domain::Edge,
            data: AttrData::Float(Arc::new(c)),
        });
        // A rim deletes the vertex creases of the result.
        if do_rim {
            attrs.remove("crease_vert");
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_gets_an_inner_shell() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let s = SolidifySettings {
            solidify_mode: "NON_MANIFOLD".into(),
            thickness: 0.2,
            ..SolidifySettings::default()
        };
        let out = solidify_nonmanifold(&s, &cube, 0).expect("solidifies");
        assert_eq!(out.verts_num(), 16);
        assert_eq!(out.faces_num(), 12);
        assert_eq!(out.edges_num(), 24);
    }

    #[test]
    fn a_plane_gets_a_rim() {
        let mut m = Mesh::new("p");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2, 3]],
            false,
        );
        let s = SolidifySettings {
            solidify_mode: "NON_MANIFOLD".into(),
            thickness: 0.1,
            ..SolidifySettings::default()
        };
        let out = solidify_nonmanifold(&s, &m, 0).expect("solidifies");
        assert_eq!(out.verts_num(), 8);
        assert_eq!(out.faces_num(), 6);
        assert_eq!(out.edges_num(), 12);
    }
}
