// SPDX-License-Identifier: GPL-3.0-or-later
//! MIRROR (`MirrorModifier`): for each enabled axis (X, then Y, then Z) the mesh is doubled with
//! the copy mirrored about the object's origin (or about `mirror_object`), optionally bisected
//! first along the mirror plane (`use_bisect_axis`, `use_bisect_flip_axis`,
//! `bisect_threshold`), and vertices within `merge_threshold` of their mirror image merge
//! (`use_mirror_merge`). The copy's faces are flipped; its UVs can be mirrored
//! (`use_mirror_u/v`, `use_mirror_udim`, `mirror_offset_u/v`) and offset (`offset_u/v`), and
//! its vertex-group weights move to the flipped group names (`use_mirror_vertex_groups`).
//!
//! `use_clip` only constrains edit-mode tools (extrude, smooth, transform); it never changes the
//! evaluated mesh.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_mirror.cc and
//! blender/source/blender/blenkernel/intern/mesh_mirror.cc (Blender 5.2.4), with
//! `BLI_string_flip_side_name` (blenlib/intern/string_utils.cc) and
//! `BKE_object_defgroup_flip_map` / `BKE_defvert_flip(_merged)` (blenkernel/intern/deform.cc).
//!
//! Not ported: custom split normals of the copy (`custom_normal` keeps the source's encoded
//! values; frap has no corner-space encoder yet): a WARNING says so when the mesh has them.

use frap_bmesh::{BMesh, ElemFlag, FromMeshParams, ToMeshParams};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_rna::{IdAny, IdRef, ObjectId, Report, ReportType};
use frap_scene::generated::modifier_settings::MirrorSettings;
use smallvec::SmallVec;

use super::weld::merge::mesh_merge_verts;
use super::weld::util::{
    self, CORNER_EDGE, CORNER_VERT, Carry, DomainMaps, M4, Topology, invert_m4, len_sq,
    m4_from_glam, mul_m4_m4m4, mul_m4_v3, unit_m4,
};
use crate::prelude::*;

fn mirror_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Mirror(m) = s
        && let Some(o) = m.mirror_object
    {
        v.push(o);
    }
    v
}

#[modifier(
    MIRROR,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    depends_on = mirror_deps,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Mirror(s) = &md.settings else {
        return Ok(mesh);
    };
    // The mirror transform about the mirror object (`tmp`, `itmp`) and the object's scale.
    let mirror_ob = s
        .mirror_object
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|o| cx.main().objects.get(*o).is_some());
    let frame = match mirror_ob {
        Some(mob) => {
            let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
            let mirror_world = m4_from_glam(cx.object_matrix_world(mob));
            Some(MirrorFrame::new(&ob_world, &mirror_world))
        }
        None => None,
    };
    if mesh
        .attributes()
        .get(frap_mesh::CUSTOM_NORMAL)
        .is_some_and(|a| a.domain == Domain::Corner)
    {
        cx.report(Report::new(
            ReportType::Warning,
            format!(
                "Modifier '{}': custom normals of the mirrored half are not mirrored by frap",
                md.name
            ),
        ));
    }
    let mut result = mesh;
    for axis in 0..3 {
        if s.use_axis[axis] {
            result = mirror_apply_on_axis(s, frame.as_ref(), &result, axis);
        }
    }
    Ok(result)
}

/// The matrices of a mirror object: `tmp` maps object coordinates into the mirror object's
/// space, `itmp` back; `ob_scale` is the object's squared axis lengths over their maximum.
pub(crate) struct MirrorFrame {
    tmp: M4,
    itmp: M4,
    ob_scale: [f32; 3],
}

impl MirrorFrame {
    /// From the object's and the mirror object's world matrices.
    pub(crate) fn new(ob_world: &M4, mirror_world: &M4) -> MirrorFrame {
        let tmp = mul_m4_m4m4(&invert_m4(mirror_world), ob_world);
        let itmp = invert_m4(&tmp);
        let mut ob_scale = [0.0f32; 3];
        for (k, s) in ob_scale.iter_mut().enumerate() {
            let c = ob_world[k];
            *s = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
        }
        MirrorFrame {
            tmp,
            itmp,
            ob_scale,
        }
    }
}

/// `mirror_apply_on_axis`: one axis, with the merge of the vertices found on the plane.
pub(crate) fn mirror_apply_on_axis(
    s: &MirrorSettings,
    frame: Option<&MirrorFrame>,
    mesh: &Mesh,
    axis: usize,
) -> Mesh {
    let (result, merge) = mirror_on_axis(s, frame, mesh, axis, true);
    match merge {
        Some((mut map, len)) if len > 0 => mesh_merge_verts(&result, &mut map, len, false),
        _ => result,
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_mirror.cc
// (BKE_mesh_mirror_apply_mirror_on_axis_for_modifier).
/// The doubled mesh and, with `use_mirror_merge`, the merge map and its length.
fn mirror_on_axis(
    s: &MirrorSettings,
    frame: Option<&MirrorFrame>,
    mesh: &Mesh,
    axis: usize,
    use_correct_order_on_merge: bool,
) -> (Mesh, Option<(Vec<i32>, usize)>) {
    let tolerance_sq = s.merge_threshold * s.merge_threshold;
    let do_vtargetmap = s.use_mirror_merge;
    let do_bisect = s.use_bisect_axis[axis];

    // `mtx` is the mirror transformation.
    let mut mtx = unit_m4();
    mtx[axis][axis] = -1.0;
    let mut plane_co = [0.0f32; 3];
    let mut plane_no = [0.0f32; 3];
    if let Some(f) = frame {
        // Into mirror-object space, mirror, back (`mul_m4_series(mtx, itmp, mtx, tmp)`).
        mtx = mul_m4_m4m4(&mul_m4_m4m4(&f.itmp, &mtx), &f.tmp);
        if do_bisect {
            plane_co = [f.itmp[3][0], f.itmp[3][1], f.itmp[3][2]];
            plane_no = [f.itmp[axis][0], f.itmp[axis][1], f.itmp[axis][2]];
            // Account for non-uniform scale in `ob` (#87592).
            let mut ob_scale = f.ob_scale;
            let ob_scale_max = ob_scale[0].max(ob_scale[1]).max(ob_scale[2]);
            if ob_scale_max != 0.0 {
                for v in &mut ob_scale {
                    *v *= 1.0 / ob_scale_max;
                }
                for k in 0..3 {
                    plane_no[k] *= ob_scale[k];
                }
            }
        }
    } else if do_bisect {
        plane_co = [mtx[3][0], mtx[3][1], mtx[3][2]];
        // Negated: that axis is inverted (for the mirror transform).
        plane_no = [-mtx[axis][0], -mtx[axis][1], -mtx[axis][2]];
    }

    let bisected;
    let mesh = if do_bisect {
        bisected = bisect_on_mirror_plane(s, mesh, axis, plane_co, plane_no);
        &bisected
    } else {
        mesh
    };

    let src_verts_num = mesh.verts_num();
    let src_edges_num = mesh.edges_num();
    let src_faces_num = mesh.faces_num();
    let src_loops_num = mesh.corners_num();
    let src_offsets = mesh.face_offsets();

    // Topology: the copy follows the original, its faces flipped (`bke::mesh_flip_faces`:
    // the first corner stays, the others reverse; corner edges reverse).
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(2 * src_verts_num);
    positions.extend_from_slice(mesh.positions());
    positions.extend_from_slice(mesh.positions());
    let mut edges: Vec<[i32; 2]> = Vec::with_capacity(2 * src_edges_num);
    edges.extend_from_slice(mesh.edges());
    let nv = src_verts_num as i32;
    edges.extend(mesh.edges().iter().map(|e| [e[0] + nv, e[1] + nv]));
    let mut face_offsets: Vec<i32> = Vec::with_capacity(2 * src_faces_num + 1);
    face_offsets.extend_from_slice(&src_offsets[..src_faces_num]);
    let nl = src_loops_num as i32;
    face_offsets.extend(src_offsets.iter().map(|o| o + nl));
    let mut corner_verts: Vec<i32> = Vec::with_capacity(2 * src_loops_num);
    corner_verts.extend_from_slice(mesh.corner_verts());
    let mut corner_edges: Vec<i32> = Vec::with_capacity(2 * src_loops_num);
    corner_edges.extend_from_slice(mesh.corner_edges());
    let mut corner_map: Vec<usize> = (0..src_loops_num).collect();
    let ne = src_edges_num as i32;
    for f in 0..src_faces_num {
        let (st, en) = (src_offsets[f] as usize, src_offsets[f + 1] as usize);
        let n = en - st;
        for k in 0..n {
            let src_c = st + if k == 0 { 0 } else { n - k };
            corner_map.push(src_c);
            corner_verts.push(mesh.corner_verts()[src_c] + nv);
            corner_edges.push(mesh.corner_edges()[st + n - 1 - k] + ne);
        }
    }

    // Mirror the vertex coordinates and build the merge map.
    let mut vtmap: Vec<i32> = Vec::new();
    let mut vtmap_len = 0usize;
    if do_vtargetmap {
        vtmap = vec![-1; 2 * src_verts_num];
    }
    for i in 0..src_verts_num {
        let vi = src_verts_num + i;
        positions[vi] = mul_m4_v3(&mtx, positions[vi]);
        if do_vtargetmap {
            // Always merge from the copy into the original (`use_correct_order_on_merge`, the
            // default of new modifiers), unless an old file says otherwise (#91444).
            if len_sq(positions[i], positions[vi]) < tolerance_sq {
                if use_correct_order_on_merge {
                    vtmap[src_verts_num + i] = i as i32;
                } else {
                    vtmap[i] = vi as i32;
                }
                vtmap_len += 1;
                // Average location.
                let a = positions[i];
                let b = positions[vi];
                let mid = [
                    (a[0] + b[0]) * 0.5,
                    (a[1] + b[1]) * 0.5,
                    (a[2] + b[2]) * 0.5,
                ];
                positions[vi] = mid;
                positions[i] = mid;
            }
        }
    }

    let topo = Topology {
        positions,
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
    };
    let twice = |n: usize| Carry::gather((0..n).chain(0..n));
    let mut result = util::rebuild(
        mesh,
        topo,
        &DomainMaps {
            vert: twice(src_verts_num),
            edge: twice(src_edges_num),
            face: twice(src_faces_num),
            corner: Carry::gather(corner_map),
        },
    );
    // `flip_corner_data` skips STRING attributes: theirs are copied unflipped.
    unflip_string_corners(mesh, &mut result);

    // UVs.
    if s.use_mirror_u || s.use_mirror_v || s.offset_u != 0.0 || s.offset_v != 0.0 {
        let uv_offset = [s.mirror_offset_u, s.mirror_offset_v];
        let uv_offset_copy = [s.offset_u, s.offset_v];
        for name in util::uv_map_names(&result) {
            let Some(uvs) = util::float2_mut(&mut result, &name) else {
                continue;
            };
            for uv in &mut uvs[src_loops_num..] {
                if s.use_mirror_u {
                    let u = uv[0];
                    uv[0] = if s.use_mirror_udim {
                        u.ceil() - libm::fmodf(u, 1.0) + uv_offset[0]
                    } else {
                        1.0 - u + uv_offset[0]
                    };
                }
                if s.use_mirror_v {
                    let v = uv[1];
                    uv[1] = if s.use_mirror_udim {
                        v.ceil() - libm::fmodf(v, 1.0) + uv_offset[1]
                    } else {
                        1.0 - v + uv_offset[1]
                    };
                }
                uv[0] += uv_offset_copy[0];
                uv[1] += uv_offset_copy[1];
            }
        }
    }

    // Vertex groups.
    if s.use_mirror_vertex_groups && result.deform_verts().is_some() {
        let flip_map = defgroup_flip_map(&result.vertex_group_names);
        if let Some(flip_map) = flip_map {
            let dv = result.deform_verts_mut();
            for i in 0..src_verts_num {
                let merged = if use_correct_order_on_merge {
                    do_vtargetmap && vtmap[i + src_verts_num] != -1
                } else {
                    do_vtargetmap && vtmap[i] != -1
                };
                if merged {
                    // Merged vertices get both groups (on the original when the copy merges
                    // into it).
                    let target = if use_correct_order_on_merge {
                        i
                    } else {
                        src_verts_num + i
                    };
                    defvert_flip_merged(&mut dv[target], &flip_map);
                } else {
                    defvert_flip(&mut dv[src_verts_num + i], &flip_map);
                }
            }
        }
    }

    let merge = do_vtargetmap.then_some((vtmap, vtmap_len));
    (result, merge)
}

/// Copies the STRING corner attributes of `src` to both halves without the face flip.
fn unflip_string_corners(src: &Mesh, result: &mut Mesh) {
    let names: Vec<(smol_str::SmolStr, AttrData)> = src
        .attributes()
        .iter()
        .filter(|a| a.domain == Domain::Corner && matches!(a.data, AttrData::String(_)))
        .filter(|a| a.name != CORNER_VERT && a.name != CORNER_EDGE)
        .map(|a| (a.name.clone(), a.data.clone()))
        .collect();
    let n = src.corners_num();
    for (name, data) in names {
        let map: Vec<usize> = (0..n).chain(0..n).collect();
        result.attributes_mut().insert(Attribute {
            name,
            domain: Domain::Corner,
            data: data.gather_with_default(&map, map.len()),
        });
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_mirror.cc
// (BKE_mesh_mirror_bisect_on_mirror_plane_for_modifier).
/// Cuts `mesh` along the mirror plane through `plane_co` with normal `plane_no` and deletes
/// the vertices beyond it (more than `bisect_threshold` on the mirrored side).
fn bisect_on_mirror_plane(
    s: &MirrorSettings,
    mesh: &Mesh,
    axis: usize,
    plane_co: [f32; 3],
    mut plane_no: [f32; 3],
) -> Mesh {
    let do_bisect_flip_axis = s.use_bisect_flip_axis[axis];
    let bisect_distance = s.bisect_threshold;
    let mut bm = BMesh::from_mesh(
        mesh,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );
    // Keep the part on the non-mirrored side unless the flip option is set.
    if !do_bisect_flip_axis {
        plane_no = [-plane_no[0], -plane_no[1], -plane_no[2]];
    }
    let plane = [
        plane_no[0],
        plane_no[1],
        plane_no[2],
        -util::dot(plane_no, plane_co),
    ];
    // `BM_mesh_bisect_plane(bm, plane, true, false, 0, 0, bisect_distance)`: without tags every
    // edge is cut-tested and no face starts in the stack, and loose vertices are snapped too.
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
    }
    for f in bm.face_ids() {
        bm.face_mut(f).flag.insert(ElemFlag::TAG);
    }
    for v in bm.vert_ids() {
        if bm.vert(v).e.is_none() {
            let co = bm.vert(v).co;
            let side = plane_side(&plane, co);
            if side > -bisect_distance && side < bisect_distance {
                let pn = [plane[0], plane[1], plane[2]];
                let l = util::dot(pn, pn);
                let c = [
                    co[0] + pn[0] * (-side / l),
                    co[1] + pn[1] * (-side / l),
                    co[2] + pn[2] * (-side / l),
                ];
                bm.vert_mut(v).co = c;
            }
        }
    }
    frap_bmesh::ops::bisect_plane::mesh_bisect_plane(&mut bm, plane, true, 0, 0, bisect_distance);
    // Plane for the vertex killing.
    let plane_offset = [plane[0], plane[1], plane[2], plane[3] - bisect_distance];
    for v in bm.vert_ids() {
        if plane_side(&plane_offset, bm.vert(v).co) > 0.0 {
            bm.vert_kill(v);
        }
    }
    bm.to_mesh(mesh, &ToMeshParams::default())
}

/// `plane_point_side_v3`.
fn plane_side(plane: &[f32; 4], co: [f32; 3]) -> f32 {
    plane[0] * co[0] + plane[1] * co[1] + plane[2] * co[2] + plane[3]
}

// ------------------------------------------------------------------------- vertex groups

// Ported from blender/source/blender/blenkernel/intern/deform.cc
// (BKE_object_defgroup_flip_map with use_default = false).
/// For each group, the group of its flipped name, or `-1`; `None` without groups.
fn defgroup_flip_map(names: &[smol_str::SmolStr]) -> Option<Vec<i32>> {
    if names.is_empty() {
        return None;
    }
    let mut map = vec![-1i32; names.len()];
    for (i, name) in names.iter().enumerate() {
        // Skip groups already mapped from their pair.
        if map[i] != -1 {
            continue;
        }
        let flipped = flip_side_name(name, false);
        if flipped != name.as_str()
            && let Some(j) = names.iter().position(|n| n.as_str() == flipped)
        {
            map[i] = j as i32;
            map[j] = i as i32;
        }
    }
    Some(map)
}

// Ported from blender/source/blender/blenkernel/intern/deform.cc (BKE_defvert_flip).
fn defvert_flip(dv: &mut frap_mesh::DeformVert, flip_map: &[i32]) {
    for w in dv.weights.iter_mut() {
        if let Some(&m) = flip_map.get(w.0 as usize)
            && m != -1
        {
            w.0 = m as u32;
        }
    }
}

// Ported from blender/source/blender/blenkernel/intern/deform.cc (BKE_defvert_flip_merged).
fn defvert_flip_merged(dv: &mut frap_mesh::DeformVert, flip_map: &[i32]) {
    let mut i = 0;
    while i < dv.weights.len() {
        let (def_nr, weight) = dv.weights[i];
        if let Some(&m) = flip_map.get(def_nr as usize)
            && m != -1
        {
            match dv.weights.iter().position(|w| w.0 == m as u32) {
                Some(j) => {
                    let w_cpy = dv.weights[j].1;
                    // Swap the weights.
                    dv.weights[j].1 = weight;
                    dv.weights[i].1 = w_cpy;
                }
                None => {
                    // Copy the weight to the flipped group (`BKE_defvert_ensure_index`).
                    dv.weights.push((m as u32, weight));
                }
            }
        }
        i += 1;
    }
}

// Ported from blender/source/blender/blenlib/intern/string_utils.cc
// (BLI_string_flip_side_name).
/// `BLI_string_flip_side_name(name, strip_number)`: swaps a left/right marker (`L`/`R`,
/// `l`/`r` with a separator, `left`/`right` in any case) at the start or end of `name`,
/// keeping a `.NNN` number suffix unless `strip_number`.
pub(crate) fn flip_side_name(from: &str, strip_number: bool) -> String {
    let is_char_sep = |c: u8| matches!(c, b'.' | b' ' | b'-' | b'_');
    let mut name = from.as_bytes().to_vec();
    if name.len() < 3 {
        // We don't support names such as `.R` or `.L`.
        return from.to_owned();
    }
    // First the case of a `.###` extension: the last period followed by a digit.
    let mut number: Vec<u8> = Vec::new();
    if name[name.len() - 1].is_ascii_digit()
        && let Some(idx) = name.iter().rposition(|&c| c == b'.')
        && name.get(idx + 1).is_some_and(u8::is_ascii_digit)
    {
        if !strip_number {
            number = name[idx..].to_vec();
        }
        name.truncate(idx);
    }
    let len = name.len();
    let mut prefix: Vec<u8> = name.clone();
    let mut suffix: Vec<u8> = Vec::new();
    let mut replace: &[u8] = b"";
    let flip_char = |c: u8| -> Option<&'static [u8]> {
        match c {
            b'l' => Some(b"r"),
            b'r' => Some(b"l"),
            b'L' => Some(b"R"),
            b'R' => Some(b"L"),
            _ => None,
        }
    };
    let mut is_set = false;
    // A separator with a trailing `r R l L`.
    if len > 1
        && is_char_sep(name[len - 2])
        && let Some(r) = flip_char(name[len - 1])
    {
        prefix.truncate(len - 1);
        replace = r;
        is_set = true;
    }
    // Beginning with `r R l L` and a separator after it.
    if !is_set
        && name.get(1).is_some_and(|&c| is_char_sep(c))
        && let Some(r) = flip_char(name[0])
    {
        replace = r;
        suffix = name[1..].to_vec();
        prefix.clear();
        is_set = true;
    }
    if !is_set && len > 5 {
        let lower = name.to_ascii_lowercase();
        let find = |pat: &[u8]| lower.windows(pat.len()).position(|w| w == pat);
        // `BLI_strcasestr`: the first occurrence, used only at the start or the end.
        let at_edge = |i: Option<usize>, plen: usize| i.filter(|&i| i == 0 || i + plen == len);
        if let Some(i) = at_edge(find(b"right"), 5) {
            replace = if name[i] == b'r' {
                b"left"
            } else if name[i + 1] == b'I' {
                b"LEFT"
            } else {
                b"Left"
            };
            prefix.truncate(i);
            suffix = name[i + 5..].to_vec();
        } else if let Some(i) = at_edge(find(b"left"), 4) {
            replace = if name[i] == b'l' {
                b"right"
            } else if name[i + 1] == b'E' {
                b"RIGHT"
            } else {
                b"Right"
            };
            prefix.truncate(i);
            suffix = name[i + 4..].to_vec();
        }
    }
    let mut out = prefix;
    out.extend_from_slice(replace);
    out.extend_from_slice(&suffix);
    out.extend_from_slice(&number);
    String::from_utf8(out).unwrap_or_else(|_| from.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_side_names() {
        for (a, b) in [
            ("Arm.L", "Arm.R"),
            ("arm_r", "arm_l"),
            ("L.arm", "R.arm"),
            ("LeftArm", "RightArm"),
            ("arm_left", "arm_right"),
            ("ARM_RIGHT", "ARM_LEFT"),
            ("Arm.L.001", "Arm.R.001"),
            ("Arm", "Arm"),
            ("L", "L"),
        ] {
            assert_eq!(flip_side_name(a, false), b, "{a}");
        }
    }

    #[test]
    fn mirror_of_a_cube_merges_the_plane_vertices() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        // Shift the cube so its -X face lies on the mirror plane.
        let mut m = cube.clone();
        for p in m.positions_mut() {
            p[0] += 1.0;
        }
        let s = MirrorSettings::default();
        let out = mirror_apply_on_axis(&s, None, &m, 0);
        assert_eq!(out.verts_num(), 12);
        assert_eq!(out.faces_num(), 11);
        let s = MirrorSettings {
            use_mirror_merge: false,
            ..MirrorSettings::default()
        };
        let out = mirror_apply_on_axis(&s, None, &m, 0);
        assert_eq!(out.verts_num(), 16);
        assert_eq!(out.faces_num(), 12);
        // The copy is mirrored and its faces flipped.
        assert_eq!(out.positions()[8][0], -m.positions()[0][0]);
        let f6: Vec<i32> = out.face(6).map(|c| out.corner_verts()[c]).collect();
        let f0: Vec<i32> = m.face(0).map(|c| m.corner_verts()[c]).collect();
        assert_eq!(f6, [f0[0] + 8, f0[3] + 8, f0[2] + 8, f0[1] + 8]);
    }

    #[test]
    fn bisect_keeps_the_positive_side() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let s = MirrorSettings {
            use_bisect_axis: [true, false, false],
            use_mirror_merge: false,
            ..MirrorSettings::default()
        };
        let out = mirror_apply_on_axis(&s, None, &cube, 0);
        // The cut cube (4 new vertices on x = 0, the 4 at x = -1 deleted) doubled.
        assert_eq!(out.verts_num(), 16);
        assert!(out.positions()[..8].iter().all(|p| p[0] >= 0.0));
        assert!(out.positions()[8..].iter().all(|p| p[0] <= 0.0));
    }
}
