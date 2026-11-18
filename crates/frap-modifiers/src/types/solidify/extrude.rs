// SPDX-License-Identifier: GPL-3.0-or-later
//! SOLIDIFY `EXTRUDE` ("Simple"): the mesh and a flipped copy (the shell, unless `use_rim_only`)
//! offset along the vertex normals (`use_even_offset` scales by the face angles,
//! `use_quality_normals` uses angle-weighted edge normals, `thickness_clamp` limits the offset
//! by the shortest edge or the vertex angle), with rim quads along the boundary edges
//! (`use_rim`), material offsets, crease and bevel-weight options and shell/rim vertex groups.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_solidify_extrude.cc
//! (MOD_solidify_extrude_modifyMesh, mesh_calc_hq_normal), Blender 5.2.4.

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_scene::generated::modifier_settings::SolidifySettings;

use super::math::*;
use crate::types::weld::util::{
    self, Carry, DomainMaps, Topology, defgroup_index, defvert_find_weight,
};

const INVALID_UNUSED: u32 = u32::MAX;
const INVALID_PAIR: u32 = u32::MAX - 1;

/// True face normals (`Mesh::face_normals_true`, Newell's method).
pub(crate) fn face_normals_true(mesh: &Mesh) -> Vec<[f32; 3]> {
    frap_geom::normals::normals_calc_faces(
        mesh.positions(),
        mesh.face_offsets(),
        mesh.corner_verts(),
    )
}

/// `Mesh::vert_normals_true`: vertex normals ignoring custom normals.
pub(crate) fn vert_normals_true(mesh: &Mesh) -> Arc<Vec<[f32; 3]>> {
    if mesh.attributes().get(frap_mesh::CUSTOM_NORMAL).is_none() {
        return mesh.vert_normals();
    }
    let mut m = mesh.clone();
    m.attributes_mut().remove(frap_mesh::CUSTOM_NORMAL);
    m.vert_normals()
}

// Ported from blender/source/blender/modifiers/intern/MOD_solidify_extrude.cc
// (mesh_calc_hq_normal).
/// Angle-weighted vertex normals from the edge normals; edges with three or more faces are
/// skipped and tagged in `edge_tmp_tag` (`USE_NONMANIFOLD_WORKAROUND`).
fn mesh_calc_hq_normal(
    mesh: &Mesh,
    face_normals: &[[f32; 3]],
    vert_normals_true: &[[f32; 3]],
    edge_tmp_tag: &mut [bool],
) -> Vec<[f32; 3]> {
    let edges = mesh.edges();
    let offsets = mesh.face_offsets();
    let corner_edges = mesh.corner_edges();
    // `EdgeFaceRef`: (0, 0) uninitialised, `p2 == -1` one face, both -1 three or more.
    let mut edge_ref = vec![(0i32, 0i32); edges.len()];
    let is_init = |r: (i32, i32)| !(r.0 == 0 && r.1 == 0);
    for i in 0..mesh.faces_num() {
        for &e in &corner_edges[offsets[i] as usize..offsets[i + 1] as usize] {
            let r = &mut edge_ref[e as usize];
            if !is_init(*r) {
                *r = (i as i32, -1);
            } else if r.0 != -1 && r.1 == -1 {
                r.1 = i as i32;
            } else {
                // 3+ faces using an edge: not useful.
                *r = (-1, -1);
                edge_tmp_tag[e as usize] = true;
            }
        }
    }
    let mut r_vert_nors = vec![[0.0f32; 3]; mesh.verts_num()];
    for (i, e) in edges.iter().enumerate() {
        let r = edge_ref[i];
        if is_init(r) && r.0 != -1 {
            let edge_normal = if r.1 != -1 {
                mid_v3_v3v3_angle_weighted(face_normals[r.0 as usize], face_normals[r.1 as usize])
            } else {
                face_normals[r.0 as usize]
            };
            let (a, b) = (e[0] as usize, e[1] as usize);
            r_vert_nors[a] = add(r_vert_nors[a], edge_normal);
            r_vert_nors[b] = add(r_vert_nors[b], edge_normal);
        }
    }
    for (i, n) in r_vert_nors.iter_mut().enumerate() {
        let (nn, l) = normalize(*n);
        *n = if l == 0.0 { vert_normals_true[i] } else { nn };
    }
    r_vert_nors
}

// Ported from blender/source/blender/modifiers/intern/MOD_solidify_extrude.cc
// (MOD_solidify_extrude_modifyMesh).
/// The solidified mesh; `totcol` is the object's material slot count.
pub(crate) fn solidify_extrude(s: &SolidifySettings, mesh: &Mesh, totcol: usize) -> Mesh {
    let verts_num = mesh.verts_num();
    let edges_num = mesh.edges_num();
    let faces_num = mesh.faces_num();
    let loops_num = mesh.corners_num();
    let mut new_loops = 0usize;
    let mut new_polys = 0usize;
    let mut new_verts = 0usize;
    let mut new_edges = 0usize;
    let mut rim_verts = 0usize;

    // Material offsets only with 2 or more materials.
    let mat_nr_max: i32 = if totcol > 1 { totcol as i32 - 1 } else { 0 };
    let mat_ofs = if mat_nr_max != 0 { s.material_offset } else { 0 };
    let mat_ofs_rim = if mat_nr_max != 0 {
        s.material_offset_rim
    } else {
        0
    };

    let need_face_normals = s.use_quality_normals
        || s.use_even_offset
        || s.use_thickness_angle_clamp
        || s.bevel_convex != 0.0;
    let ofs_orig = -(((-s.offset + 1.0) * 0.5) * s.thickness);
    let ofs_new = s.thickness + ofs_orig;
    let offset_fac_vg = s.thickness_vertex_group;
    let offset_fac_vg_inv = 1.0 - s.thickness_vertex_group;
    let bevel_convex = s.bevel_convex;
    let do_flip = s.use_flip_normals;
    let do_clamp = s.thickness_clamp != 0.0;
    let do_angle_clamp = do_clamp && s.use_thickness_angle_clamp;
    let do_bevel_convex = bevel_convex != 0.0;
    let do_rim = s.use_rim;
    let do_shell = !(do_rim && s.use_rim_only);

    // Weights.
    let defgrp_invert = s.invert_vertex_group;
    let shell_defgrp_index = defgroup_index(mesh, &s.shell_vertex_group);
    let rim_defgrp_index = defgroup_index(mesh, &s.rim_vertex_group);
    let defgrp_index = defgroup_index(mesh, &s.vertex_group);
    let dvert = defgrp_index.and(mesh.deform_verts());

    let stride = if do_shell { 2 } else { 1 };
    let vert_normals = vert_normals_true(mesh);
    let orig_positions = mesh.positions();
    let orig_edges = mesh.edges();
    let orig_offsets = mesh.face_offsets();
    let orig_corner_verts = mesh.corner_verts();
    let orig_corner_edges = mesh.corner_edges();
    let face_normals: Vec<[f32; 3]> = if need_face_normals {
        face_normals_true(mesh)
    } else {
        Vec::new()
    };
    let face = |i: usize| orig_offsets[i] as usize..orig_offsets[i + 1] as usize;

    let mut new_vert_arr: Vec<u32> = Vec::new();
    let mut new_edge_arr: Vec<u32> = Vec::new();
    let mut old_vert_arr: Vec<u32> = vec![0; verts_num];
    let mut edge_users: Vec<u32> = Vec::new();
    let mut edge_order: Vec<i32> = Vec::new();

    if do_rim {
        let mut orig_mvert_tag = vec![false; verts_num];
        edge_users = vec![INVALID_UNUSED; edges_num];
        edge_order = vec![0; edges_num];
        for i in 0..faces_num {
            let f = face(i);
            let mut corner_i_prev = f.end - 1;
            for (j, corner_i) in f.clone().enumerate() {
                let vert_i = orig_corner_verts[corner_i];
                let prev_vert_i = orig_corner_verts[corner_i_prev];
                // Add an edge user.
                let eidx = orig_corner_edges[corner_i_prev] as usize;
                if edge_users[eidx] == INVALID_UNUSED {
                    let edge = orig_edges[eidx];
                    edge_users[eidx] = if (prev_vert_i > vert_i) == (edge[0] < edge[1]) {
                        i as u32
                    } else {
                        (i + faces_num) as u32
                    };
                    edge_order[eidx] = j as i32;
                } else {
                    edge_users[eidx] = INVALID_PAIR;
                }
                corner_i_prev = corner_i;
            }
        }
        for eidx in 0..edges_num {
            if edge_users[eidx] != INVALID_UNUSED && edge_users[eidx] != INVALID_PAIR {
                orig_mvert_tag[orig_edges[eidx][0] as usize] = true;
                orig_mvert_tag[orig_edges[eidx][1] as usize] = true;
                new_edge_arr.push(eidx as u32);
                new_polys += 1;
                new_loops += 4;
            }
        }
        for i in 0..verts_num {
            if orig_mvert_tag[i] {
                old_vert_arr[i] = new_vert_arr.len() as u32;
                new_vert_arr.push(i as u32);
                rim_verts += 1;
            } else {
                old_vert_arr[i] = INVALID_UNUSED;
            }
        }
    }
    if !do_shell {
        // Only the rim vertices; each extruded face needs an opposite edge.
        new_verts = rim_verts;
        new_edges = new_polys;
    }

    let mut edge_tmp_tag = vec![false; edges_num];
    let mut vert_nors: Option<Vec<[f32; 3]>> = if s.use_quality_normals {
        Some(mesh_calc_hq_normal(
            mesh,
            &face_normals,
            &vert_normals,
            &mut edge_tmp_tag,
        ))
    } else {
        None
    };

    let res_verts = verts_num * stride + new_verts;
    let res_edges = edges_num * stride + new_edges + rim_verts;
    let res_faces = faces_num * stride + new_polys;
    let res_loops = loops_num * stride + new_loops;

    // Topology and attribute carry maps.
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(res_verts);
    let mut edges: Vec<[i32; 2]> = Vec::with_capacity(res_edges);
    let mut face_offsets: Vec<i32> = Vec::with_capacity(res_faces + 1);
    let mut corner_verts: Vec<i32> = Vec::with_capacity(res_loops);
    let mut corner_edges: Vec<i32> = Vec::with_capacity(res_loops);
    let mut vmap: Vec<Option<usize>> = Vec::with_capacity(res_verts);
    let mut emap: Vec<Option<usize>> = Vec::with_capacity(res_edges);
    let mut fmap: Vec<Option<usize>> = Vec::with_capacity(res_faces);
    let mut cmap: Vec<Option<usize>> = Vec::with_capacity(res_loops);

    positions.extend_from_slice(orig_positions);
    vmap.extend((0..verts_num).map(Some));
    edges.extend_from_slice(orig_edges);
    emap.extend((0..edges_num).map(Some));
    face_offsets.extend_from_slice(&orig_offsets[..faces_num]);
    fmap.extend((0..faces_num).map(Some));
    corner_verts.extend_from_slice(orig_corner_verts);
    corner_edges.extend_from_slice(orig_corner_edges);
    cmap.extend((0..loops_num).map(Some));
    if do_shell {
        positions.extend_from_slice(orig_positions);
        vmap.extend((0..verts_num).map(Some));
        let nv = verts_num as i32;
        edges.extend(orig_edges.iter().map(|e| [e[0] + nv, e[1] + nv]));
        emap.extend((0..edges_num).map(Some));
        let nl = loops_num as i32;
        face_offsets.extend(orig_offsets[..faces_num].iter().map(|o| o + nl));
        fmap.extend((0..faces_num).map(Some));
        // The copy keeps its first corner and reverses the others (so the diagonals of the new
        // face match the original); corner edges shift to follow.
        let ne = edges_num as i32;
        for i in 0..faces_num {
            let f = face(i);
            let n = f.len();
            for p in 0..n {
                let src = f.start + (n - p) % n;
                cmap.push(Some(src));
                corner_verts.push(orig_corner_verts[src] + nv);
                corner_edges.push(orig_corner_edges[f.start + n - 1 - p] + ne);
            }
        }
    } else {
        for &v in &new_vert_arr {
            positions.push(orig_positions[v as usize]);
            vmap.push(Some(v as usize));
        }
        for i in 0..edges_num {
            if edge_users[i] != INVALID_UNUSED && edge_users[i] != INVALID_PAIR {
                let e = orig_edges[i];
                edges.push([
                    (old_vert_arr[e[0] as usize] as usize + verts_num) as i32,
                    (old_vert_arr[e[1] as usize] as usize + verts_num) as i32,
                ]);
                emap.push(Some(i));
            }
        }
    }

    // Rim edges (one per rim vertex) and rim faces.
    for i in 0..rim_verts {
        let v = new_vert_arr[i] as usize;
        edges.push([
            v as i32,
            ((if do_shell { v } else { i }) + verts_num) as i32,
        ]);
        emap.push(None);
    }
    let rim_edge = |v: usize| -> i32 { (edges_num * stride + old_vert_arr[v] as usize + new_edges) as i32 };
    for i in 0..new_polys {
        let eidx = new_edge_arr[i] as usize;
        let mut pidx = edge_users[eidx] as usize;
        let flip = if pidx >= faces_num {
            pidx -= faces_num;
            true
        } else {
            false
        };
        let edge = orig_edges[eidx];
        fmap.push(Some(pidx));
        face_offsets.push(corner_verts.len() as i32);
        let old_face_size = face(pidx).len() as i32;
        let start = orig_offsets[pidx];
        // Previous loop.
        let k1 = (start + ((edge_order[eidx] - 1) + old_face_size) % old_face_size) as usize;
        let k2 = (start + edge_order[eidx]) as usize;
        cmap.extend([Some(k2), Some(k1), Some(k1), Some(k2)]);
        let (a, b) = if flip {
            (edge[1] as usize, edge[0] as usize)
        } else {
            (edge[0] as usize, edge[1] as usize)
        };
        let copy_of = |v: usize| -> i32 {
            ((if do_shell { v } else { old_vert_arr[v] as usize }) + verts_num) as i32
        };
        let copy_edge = ((if do_shell { eidx } else { i }) + edges_num) as i32;
        corner_verts.extend([a as i32, b as i32, copy_of(b), copy_of(a)]);
        corner_edges.extend([eidx as i32, rim_edge(b), copy_edge, rim_edge(a)]);
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

    // `bevel_weight_edge` (written by bevel convex and the rim) and `material_index` exist on
    // the result.
    let orig_vert_bweight = util::float_attr(mesh, "bevel_weight_vert", Domain::Point);
    let want_bweight = mesh.attributes().get("bevel_weight_edge").is_some()
        || do_bevel_convex
        || orig_vert_bweight.is_some();
    let mut bweight: Option<Vec<f32>> = want_bweight.then(|| {
        util::float_attr(&result, "bevel_weight_edge", Domain::Edge)
            .map(|v| v.to_vec())
            .unwrap_or_else(|| vec![0.0; res_edges])
    });
    let mut material: Vec<i32> = util::int_attr(&result, "material_index", Domain::Face)
        .map(|v| v.to_vec())
        .unwrap_or_else(|| vec![0; res_faces]);
    if do_shell && mat_ofs != 0 {
        for m in &mut material[faces_num..2 * faces_num] {
            *m = (*m + mat_ofs).clamp(0, mat_nr_max);
        }
    }

    // `INIT_VERT_ARRAY_OFFSETS(test)`: (i_end, do_shell_align, vert_index).
    let init_offsets = |test: bool| -> (usize, bool, usize) {
        if ((ofs_new >= ofs_orig) == do_flip) == test {
            (verts_num, true, 0)
        } else if do_shell {
            (verts_num, true, verts_num)
        } else {
            (new_verts, false, verts_num)
        }
    };
    let positions = result.positions_mut();
    if !s.use_even_offset {
        // No even thickness: very simple.
        let offset = s.thickness.abs() * s.thickness_clamp;
        let offset_sq = offset * offset;
        let mut vert_lens: Vec<f32> = Vec::new();
        if do_clamp {
            vert_lens = vec![f32::MAX; verts_num];
            for e in &orig_edges[..] {
                let (a, b) = (e[0] as usize, e[1] as usize);
                let l = len_squared_v3v3(positions[a], positions[b]);
                vert_lens[a] = vert_lens[a].min(l);
                vert_lens[b] = vert_lens[b].min(l);
            }
        }
        let (vert_angs, edge_angs) = edge_angles(
            mesh,
            &face_normals,
            do_angle_clamp,
            do_bevel_convex,
            do_rim,
            &mut edge_users,
            false,
        );
        for (test, ofs) in [(false, ofs_new), (true, ofs_orig)] {
            if ofs == 0.0 {
                continue;
            }
            let (i_end, do_shell_align, mut vert_index) = init_offsets(test);
            let mut ofs_new_vgroup = ofs;
            for i_orig in 0..i_end {
                let i = if do_shell_align {
                    i_orig
                } else {
                    new_vert_arr[i_orig] as usize
                };
                if let (Some(dv), Some(g)) = (dvert, defgrp_index) {
                    let w = defvert_find_weight(&dv[i], g);
                    let w = if defgrp_invert { 1.0 - w } else { w };
                    ofs_new_vgroup = (offset_fac_vg + (w * offset_fac_vg_inv)) * ofs;
                }
                if do_clamp && offset > f32::EPSILON {
                    // Always reset: it may have been set before.
                    if dvert.is_none() {
                        ofs_new_vgroup = ofs;
                    }
                    if do_angle_clamp {
                        let cos_ang = if !test {
                            libm::cosf(
                                ((2.0 * std::f64::consts::PI - f64::from(vert_angs[i])) * 0.5)
                                    as f32,
                            )
                        } else {
                            // Blender reads the angle at `i_orig` here.
                            libm::cosf(vert_angs[i_orig] * 0.5)
                        };
                        if cos_ang > 0.0 {
                            let max_off = vert_lens[i].sqrt() * 0.5 / cos_ang;
                            if max_off < offset * 0.5 {
                                ofs_new_vgroup *= max_off / offset * 2.0;
                            }
                        }
                    } else if vert_lens[i] < offset_sq {
                        let scalar = vert_lens[i].sqrt() / offset;
                        ofs_new_vgroup *= scalar;
                    }
                }
                let n = match &vert_nors {
                    Some(vn) => vn[i],
                    None => vert_normals[i],
                };
                positions[vert_index] = madd(positions[vert_index], n, ofs_new_vgroup);
                vert_index += 1;
            }
        }
        if do_bevel_convex && let Some(bw) = bweight.as_mut() {
            apply_bevel_convex(bw, &edge_users, &edge_angs, bevel_convex, edges_num, do_shell);
        }
    } else {
        let check_non_manifold = s.use_quality_normals;
        let mut vert_angles = vec![0.0f32; verts_num];
        let mut vert_accum = vec![0.0f32; verts_num];
        let vn: Vec<[f32; 3]> = vert_nors.take().unwrap_or_else(|| vert_normals.to_vec());
        for i in 0..faces_num {
            let f = face(i);
            let n = f.len();
            let fv = &orig_corner_verts[f.clone()];
            let fe = &orig_corner_edges[f.clone()];
            let mut i_curr = n - 1;
            let mut nor_prev =
                normalized(sub(positions[fv[i_curr - 1] as usize], positions[fv[i_curr] as usize]));
            for i_next in 0..n {
                let nor_next =
                    normalized(sub(positions[fv[i_curr] as usize], positions[fv[i_next] as usize]));
                let angle = angle_normalized_v3v3(nor_prev, nor_next).max(f32::EPSILON);
                let vidx = fv[i_curr] as usize;
                vert_accum[vidx] += angle;
                // Skip 3+ face user edges.
                if !check_non_manifold
                    || (!edge_tmp_tag[fe[i_curr] as usize] && !edge_tmp_tag[fe[i_next] as usize])
                {
                    vert_angles[vidx] +=
                        shell_v3v3_normalized_to_dist(vn[vidx], face_normals[i]) * angle;
                } else {
                    vert_angles[vidx] += angle;
                }
                nor_prev = nor_next;
                i_curr = i_next;
            }
        }
        // Vertex group support.
        if let (Some(dv), Some(g)) = (dvert, defgrp_index) {
            for i in 0..verts_num {
                let w = defvert_find_weight(&dv[i], g);
                let w = if defgrp_invert { 1.0 - w } else { w };
                vert_angles[i] *= offset_fac_vg + (w * offset_fac_vg_inv);
            }
        }
        let (vert_angs, edge_angs) = edge_angles(
            mesh,
            &face_normals,
            do_angle_clamp,
            do_bevel_convex,
            do_rim,
            &mut edge_users,
            true,
        );
        if do_clamp {
            let clamp_fac = 1.0 + if do_angle_clamp { s.offset.abs() } else { 0.0 };
            let offset = s.thickness.abs() * s.thickness_clamp * clamp_fac;
            if offset > f32::EPSILON {
                let offset_sq = offset * offset;
                let mut vert_lens_sq = vec![f32::MAX; verts_num];
                for e in &orig_edges[..] {
                    let (a, b) = (e[0] as usize, e[1] as usize);
                    let l = len_squared_v3v3(positions[a], positions[b]);
                    vert_lens_sq[a] = vert_lens_sq[a].min(l);
                    vert_lens_sq[b] = vert_lens_sq[b].min(l);
                }
                if do_angle_clamp {
                    for i in 0..verts_num {
                        let cos_ang = libm::cosf(vert_angs[i] * 0.5);
                        if cos_ang > 0.0 {
                            let max_off = vert_lens_sq[i].sqrt() * 0.5 / cos_ang;
                            if max_off < offset * 0.5 {
                                vert_angles[i] *= max_off / offset * 2.0;
                            }
                        }
                    }
                } else {
                    for i in 0..verts_num {
                        if vert_lens_sq[i] < offset_sq {
                            vert_angles[i] *= vert_lens_sq[i].sqrt() / offset;
                        }
                    }
                }
            }
        }
        if do_bevel_convex && let Some(bw) = bweight.as_mut() {
            apply_bevel_convex(bw, &edge_users, &edge_angs, bevel_convex, edges_num, do_shell);
        }
        for (test, ofs) in [(false, ofs_new), (true, ofs_orig)] {
            if ofs == 0.0 {
                continue;
            }
            let (i_end, do_shell_align, mut vert_index) = init_offsets(test);
            for i_orig in 0..i_end {
                let i_other = if do_shell_align {
                    i_orig
                } else {
                    new_vert_arr[i_orig] as usize
                };
                // Zero if unselected.
                if vert_accum[i_other] != 0.0 {
                    positions[vert_index] = madd(
                        positions[vert_index],
                        vn[i_other],
                        ofs * (vert_angles[i_other] / vert_accum[i_other]),
                    );
                }
                vert_index += 1;
            }
        }
    }

    // Weights of the rim and shell vertex groups.
    if (shell_defgrp_index.is_some() || rim_defgrp_index.is_some()) && res_verts > 0 {
        let res_n = result.verts_num();
        let dv = result.deform_verts_mut();
        let mut ensure = |v: usize, g: u32| match dv[v].weights.iter_mut().find(|w| w.0 == g) {
            Some(w) => w.1 = 1.0,
            None => dv[v].weights.push((g, 1.0)),
        };
        if let Some(g) = rim_defgrp_index {
            for i in 0..rim_verts {
                let v = new_vert_arr[i] as usize;
                ensure(v, g);
                ensure((if do_shell { v } else { i }) + verts_num, g);
            }
        }
        if let Some(g) = shell_defgrp_index {
            for v in verts_num..res_n {
                ensure(v, g);
            }
        }
    }

    // Rim crease, bevel weight and materials.
    let mut crease: Option<Vec<f32>> = None;
    if do_rim {
        let (crease_rim, crease_outer, crease_inner) =
            (s.edge_crease_rim, s.edge_crease_outer, s.edge_crease_inner);
        if crease_rim != 0.0 || crease_outer != 0.0 || crease_inner != 0.0 {
            crease = Some(
                util::float_attr(&result, "crease_edge", Domain::Edge)
                    .map(|v| v.to_vec())
                    .unwrap_or_else(|| vec![0.0; res_edges]),
            );
        }
        let first_rim_edge = edges_num * stride + new_edges;
        for i in 0..rim_verts {
            let e = first_rim_edge + i;
            if let (Some(bw), Some(vbw)) = (bweight.as_mut(), orig_vert_bweight) {
                bw[e] = vbw[new_vert_arr[i] as usize];
            }
            if let Some(c) = crease.as_mut()
                && crease_rim != 0.0
            {
                c[e] = crease_rim;
            }
        }
        for i in 0..new_polys {
            let eidx = new_edge_arr[i] as usize;
            let fi = faces_num * stride + i;
            if mat_ofs_rim != 0 {
                material[fi] = (material[fi] + mat_ofs_rim).clamp(0, mat_nr_max);
            }
            if let Some(c) = crease.as_mut() {
                if crease_outer != 0.0 {
                    let t = c[eidx] + crease_outer;
                    c[eidx] = if t > 1.0 { 1.0 } else { t };
                }
                if crease_inner != 0.0 {
                    let k = edges_num + if do_shell { eidx } else { i };
                    let t = c[k] + crease_inner;
                    c[k] = if t > 1.0 { 1.0 } else { t };
                }
            }
        }
    }
    let attrs = result.attributes_mut();
    if let Some(bw) = bweight {
        attrs.insert(Attribute {
            name: "bevel_weight_edge".into(),
            domain: Domain::Edge,
            data: AttrData::Float(Arc::new(bw)),
        });
    }
    attrs.insert(Attribute {
        name: "material_index".into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(material)),
    });
    if let Some(c) = crease {
        attrs.insert(Attribute {
            name: "crease_edge".into(),
            domain: Domain::Edge,
            data: AttrData::Float(Arc::new(c)),
        });
    }
    result
}

/// The vertex angles of the angle clamp and the signed edge angles of bevel convex, from the
/// two faces of each manifold edge; marks those edges `INVALID_PAIR` in `edge_users` without a
/// rim (`edge_users` then only serves bevel convex). `even` selects the even-thickness variant
/// of the vertex angle (`π − angle(n0, n1)` instead of the signed angle).
fn edge_angles(
    mesh: &Mesh,
    face_normals: &[[f32; 3]],
    do_angle_clamp: bool,
    do_bevel_convex: bool,
    do_rim: bool,
    edge_users: &mut Vec<u32>,
    even: bool,
) -> (Vec<f32>, Vec<f32>) {
    let mut vert_angs = Vec::new();
    let mut edge_angs = Vec::new();
    if !(do_angle_clamp || do_bevel_convex) {
        return (vert_angs, edge_angs);
    }
    let edges = mesh.edges();
    let offsets = mesh.face_offsets();
    let cv = mesh.corner_verts();
    let ce = mesh.corner_edges();
    let positions = mesh.positions();
    if do_angle_clamp {
        vert_angs = vec![(0.5 * std::f64::consts::PI) as f32; mesh.verts_num()];
    }
    if do_bevel_convex {
        edge_angs = vec![0.0; edges.len()];
        if !do_rim {
            // Blender leaves this array uninitialised; unused edges never read as a pair.
            *edge_users = vec![INVALID_UNUSED; edges.len()];
        }
    }
    let mut pairs = vec![[INVALID_UNUSED; 2]; edges.len()];
    for i in 0..mesh.faces_num() {
        let (st, en) = (offsets[i] as usize, offsets[i + 1] as usize);
        let mut prev = en - 1;
        for corner_i in st..en {
            let vert_i = cv[corner_i];
            let prev_vert_i = cv[prev];
            let eidx = ce[prev] as usize;
            let ed = edges[eidx];
            let flip = usize::from((prev_vert_i > vert_i) == (ed[0] < ed[1]));
            if pairs[eidx][flip] == INVALID_UNUSED {
                pairs[eidx][flip] = i as u32;
            } else {
                pairs[eidx] = [INVALID_PAIR; 2];
            }
            prev = corner_i;
        }
    }
    let valid = |x: u32| x != INVALID_UNUSED && x != INVALID_PAIR;
    for (i, ed) in edges.iter().enumerate() {
        let [p0, p1] = pairs[i];
        if !(valid(p0) && valid(p1)) {
            continue;
        }
        let n0 = face_normals[p0 as usize];
        let n1 = face_normals[p1 as usize];
        let (a, b) = (ed[0] as usize, ed[1] as usize);
        let signed = || {
            let e = normalized(sub(positions[a], positions[b]));
            angle_signed_on_axis_v3v3_v3(n0, n1, e)
        };
        if even {
            if do_angle_clamp {
                let angle = std::f32::consts::PI - angle_normalized_v3v3(n0, n1);
                vert_angs[a] = vert_angs[a].max(angle);
                vert_angs[b] = vert_angs[b].max(angle);
            }
            if do_bevel_convex {
                edge_angs[i] = signed();
                if !do_rim {
                    edge_users[i] = INVALID_PAIR;
                }
            }
        } else {
            let angle = signed();
            if do_angle_clamp {
                vert_angs[a] = vert_angs[a].max(angle);
                vert_angs[b] = vert_angs[b].max(angle);
            }
            if do_bevel_convex {
                edge_angs[i] = angle;
                if !do_rim {
                    edge_users[i] = INVALID_PAIR;
                }
            }
        }
    }
    (vert_angs, edge_angs)
}

/// Bevel convex: adds `bevel_convex` (clamped to the edge's convexity) to the bevel weight of
/// the manifold edges and of their shell copies.
fn apply_bevel_convex(
    bw: &mut [f32],
    edge_users: &[u32],
    edge_angs: &[f32],
    bevel_convex: f32,
    edges_num: usize,
    do_shell: bool,
) {
    let pi = std::f32::consts::PI;
    for i in 0..edges_num {
        if edge_users.get(i) == Some(&INVALID_PAIR) {
            let angle = edge_angs[i];
            bw[i] = (bw[i]
                + if angle < pi {
                    bevel_convex.clamp(0.0, 1.0)
                } else {
                    bevel_convex.clamp(-1.0, 0.0)
                })
            .clamp(0.0, 1.0);
            if do_shell {
                bw[i + edges_num] = (bw[i + edges_num]
                    + if angle > pi {
                        bevel_convex.clamp(0.0, 1.0)
                    } else {
                        bevel_convex.clamp(-1.0, 0.0)
                    })
                .clamp(0.0, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_gets_a_shell_and_a_rim() {
        let mut m = Mesh::new("p");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2, 3]],
            false,
        );
        let s = SolidifySettings {
            thickness: 0.1,
            ..SolidifySettings::default()
        };
        let out = solidify_extrude(&s, &m, 0);
        assert_eq!(out.verts_num(), 8);
        assert_eq!(out.faces_num(), 6);
        assert_eq!(out.edges_num(), 12);
        // Offset −1: the copy goes against the normal (+Z) by the thickness.
        assert!((out.positions()[4][2] + 0.1).abs() < 1e-6);
        assert!(out.positions()[0][2].abs() < 1e-6);
    }
}
