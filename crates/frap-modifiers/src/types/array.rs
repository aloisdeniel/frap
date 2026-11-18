// SPDX-License-Identifier: GPL-3.0-or-later
//! ARRAY (`ArrayModifier`): `count` copies (or as many as fit in `fit_length`) chained by an
//! offset matrix built from a constant offset, an offset relative to the mesh bounds and an
//! offset object; optional start/end caps (other objects' evaluated meshes); vertices of
//! consecutive copies (and of the first and last copy, `use_merge_vertices_cap`, and of the caps)
//! within `merge_threshold` merge; the UVs of copy `c` move by `c · (offset_u, offset_v)`.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_array.cc (Blender 5.2.4).
//! `FIT_CURVE` needs curve objects, which frap does not have (OPERATORS.md: CURVE is out of
//! scope): without a curve it behaves as Blender's does with none, like `FIT_LENGTH`.

use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use frap_rna::{IdAny, IdRef, ObjectId, Report, ReportType};
use frap_scene::generated::modifier_settings::ArraySettings;
use smallvec::SmallVec;

use super::weld::merge::mesh_merge_verts;
use super::weld::util::{
    self, Carry, DomainMaps, M4, Topology, compare_len, invert_m4, len_sq, m4_from_glam,
    mul_m4_m4m4, mul_m4_v3, unit_m4,
};
use crate::prelude::*;

fn array_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Array(a) = s {
        for o in [a.start_cap, a.end_cap, a.curve, a.offset_object]
            .into_iter()
            .flatten()
        {
            v.push(o);
        }
    }
    v
}

#[modifier(
    ARRAY,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    depends_on = array_deps,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Array(s) = &md.settings else {
        return Ok(mesh);
    };
    let me = cx.object_ref();
    let object = |id: Option<IdAny>| id.and_then(IdRef::<ObjectId>::from_any);
    // `is_disabled`: a cap that is not a mesh object (a placeholder of a missing library).
    for cap in [object(s.start_cap), object(s.end_cap)].into_iter().flatten() {
        if cx
            .main()
            .objects
            .get(cap)
            .is_some_and(|o| o.object_type() != "MESH")
        {
            return Ok(mesh);
        }
    }
    let cap_mesh = |cx: &mut dyn ModEvalCx, id: Option<IdAny>| -> Option<Mesh> {
        let ob = object(id)?;
        if ob == me {
            return None;
        }
        cx.evaluated_mesh(ob).ok().map(|m| (*m).clone())
    };
    let start_cap = cap_mesh(cx, s.start_cap);
    let end_cap = cap_mesh(cx, s.end_cap);
    let offset_ob = if s.use_object_offset {
        object(s.offset_object).filter(|o| cx.main().objects.get(*o).is_some())
    } else {
        None
    };
    let matrices = offset_ob.map(|o| {
        (
            m4_from_glam(cx.object_matrix_world(me)),
            m4_from_glam(cx.object_matrix_world(o)),
        )
    });
    let mut errors = Vec::new();
    let out = do_array(
        s,
        &mesh,
        matrices.as_ref().map(|(a, b)| (a, b)),
        start_cap.as_ref(),
        end_cap.as_ref(),
        &mut errors,
    );
    for e in errors {
        cx.report(Report::new(
            ReportType::Warning,
            format!("Modifier '{}': {e}", md.name),
        ));
    }
    Ok(out)
}

/// Element counts of a mesh part.
#[derive(Copy, Clone, Default)]
struct Counts {
    verts: usize,
    edges: usize,
    loops: usize,
    faces: usize,
}

impl Counts {
    fn of(m: Option<&Mesh>) -> Counts {
        m.map_or(Counts::default(), |m| Counts {
            verts: m.verts_num(),
            edges: m.edges_num(),
            loops: m.corners_num(),
            faces: m.faces_num(),
        })
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_array.cc (arrayModifier_doArray).
/// The arrayed mesh; `matrices` are the object's and the offset object's world matrices;
/// `errors` receives Blender's modifier error texts.
pub(crate) fn do_array(
    s: &ArraySettings,
    mesh: &Mesh,
    matrices: Option<(&M4, &M4)>,
    start_cap: Option<&Mesh>,
    end_cap: Option<&Mesh>,
    errors: &mut Vec<&'static str>,
) -> Mesh {
    let use_merge = s.use_merge_vertices;
    let chunk = Counts::of(Some(mesh));
    let sc = Counts::of(start_cap);
    let ec = Counts::of(end_cap);
    let mut count = s.count;

    // Build up the offset matrix, accumulating all settings options.
    let mut offset = unit_m4();
    if s.use_constant_offset {
        for k in 0..3 {
            offset[3][k] += s.constant_offset_displace[k];
        }
    }
    if s.use_relative_offset
        && let Some([min, max]) = bounds(mesh.positions())
    {
        for j in (0..3).rev() {
            offset[3][j] += s.relative_offset_displace[j] * (max[j] - min[j]);
        }
    }
    if let Some((ob_world, offset_world)) = matrices {
        let obinv = invert_m4(ob_world);
        offset = mul_m4_m4m4(&mul_m4_m4m4(&offset, &obinv), offset_world);
    }
    // Some scaling: the mapping of consecutive copies is not a translation.
    let scale = [
        len3(&offset[0]),
        len3(&offset[1]),
        len3(&offset[2]),
    ];
    let offset_has_scale = scale != [1.0, 1.0, 1.0];

    // About 67 million vertices max.
    let max_verts_num: usize = 1 << 26;
    if matches!(s.fit_type.as_str(), "FIT_LENGTH" | "FIT_CURVE") {
        let float_epsilon = 1e-6f32;
        let mut offset_is_too_small = false;
        let dist = len3(&offset[3]);
        if dist > float_epsilon {
            // Length = first copy start to last copy end; a tiny offset for rounding errors.
            count = ((s.fit_length + float_epsilon) / dist + 1.0) as i32;
            if (count.max(0) as usize) * chunk.verts + sc.verts + ec.verts > max_verts_num {
                count = 1;
                offset_is_too_small = true;
            }
        } else {
            // No translation: just make one copy.
            count = 1;
            offset_is_too_small = true;
        }
        if offset_is_too_small {
            errors.push(
                "The offset is too small, we cannot generate the amount of geometry it would \
                 require",
            );
        }
    } else if (count.max(0) as usize) * chunk.verts + sc.verts + ec.verts > max_verts_num {
        count = 1;
        errors.push(
            "The amount of copies is too high, we cannot generate the amount of geometry it \
             would require",
        );
    }
    let count = count.max(1) as usize;

    // The counts before merging doubles.
    let result_nverts = chunk.verts * count + sc.verts + ec.verts;
    let result_nedges = chunk.edges * count + sc.edges + ec.edges;
    let result_nloops = chunk.loops * count + sc.loops + ec.loops;
    let result_nfaces = chunk.faces * count + sc.faces + ec.faces;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(result_nverts);
    let mut edges: Vec<[i32; 2]> = Vec::with_capacity(result_nedges);
    let mut face_offsets: Vec<i32> = Vec::with_capacity(result_nfaces + 1);
    let mut corner_verts: Vec<i32> = Vec::with_capacity(result_nloops);
    let mut corner_edges: Vec<i32> = Vec::with_capacity(result_nloops);
    let mut full_doubles_map: Vec<i32> = if use_merge {
        vec![-1; result_nverts]
    } else {
        Vec::new()
    };

    // The chunks.
    let mut current_offset = unit_m4();
    for c in 0..count {
        if c > 0 {
            // Recalculate the cumulative offset.
            current_offset = mul_m4_m4m4(&current_offset, &offset);
        }
        for &p in mesh.positions() {
            positions.push(if c > 0 {
                mul_m4_v3(&current_offset, p)
            } else {
                p
            });
        }
        let (cv, ce) = ((c * chunk.verts) as i32, (c * chunk.edges) as i32);
        edges.extend(mesh.edges().iter().map(|e| [e[0] + cv, e[1] + cv]));
        let cl = (c * chunk.loops) as i32;
        face_offsets.extend(mesh.face_offsets()[..chunk.faces].iter().map(|o| o + cl));
        corner_verts.extend(mesh.corner_verts().iter().map(|v| v + cv));
        corner_edges.extend(mesh.corner_edges().iter().map(|e| e + ce));

        // Merge between chunk n and n-1.
        if use_merge && c >= 1 {
            if !offset_has_scale && c >= 2 {
                // Mapping chunk 3 to chunk 2 is a translation of mapping 2 to 1.
                let mut this_chunk_index = c * chunk.verts;
                let mut prev_chunk_index = (c - 1) * chunk.verts;
                for _ in 0..chunk.verts {
                    let mut target = full_doubles_map[prev_chunk_index];
                    if target != -1 {
                        target += chunk.verts as i32;
                        while target != -1
                            && !matches!(full_doubles_map[target as usize], t if t == -1 || t == target)
                        {
                            // Follow a mapped target only while the final target stays close.
                            if compare_len(
                                positions[this_chunk_index],
                                positions[full_doubles_map[target as usize] as usize],
                                s.merge_threshold,
                            ) {
                                target = full_doubles_map[target as usize];
                            } else {
                                target = -1;
                            }
                        }
                    }
                    full_doubles_map[this_chunk_index] = target;
                    this_chunk_index += 1;
                    prev_chunk_index += 1;
                }
            } else {
                map_doubles(
                    &mut full_doubles_map,
                    &positions,
                    (c - 1) * chunk.verts,
                    chunk.verts,
                    c * chunk.verts,
                    chunk.verts,
                    s.merge_threshold,
                );
            }
        }
    }

    let last_chunk_start = (count - 1) * chunk.verts;
    if use_merge && s.use_merge_vertices_cap && count > 1 {
        // Merge the first and last copies.
        map_doubles(
            &mut full_doubles_map,
            &positions,
            last_chunk_start,
            chunk.verts,
            0,
            chunk.verts,
            s.merge_threshold,
        );
    }

    // Caps: `mesh_merge_transform`.
    let mut cap_part = |cap: &Mesh, cap_offset: &M4, positions: &mut Vec<[f32; 3]>| {
        let (vb, eb, lb) = (positions.len() as i32, edges.len() as i32, corner_verts.len() as i32);
        positions.extend(cap.positions().iter().map(|&p| mul_m4_v3(cap_offset, p)));
        edges.extend(cap.edges().iter().map(|e| [e[0] + vb, e[1] + vb]));
        face_offsets.extend(cap.face_offsets()[..cap.faces_num()].iter().map(|o| o + lb));
        corner_verts.extend(cap.corner_verts().iter().map(|v| v + vb));
        corner_edges.extend(cap.corner_edges().iter().map(|e| e + eb));
    };
    if let Some(cap) = start_cap {
        let start_cap_start = positions.len();
        let start_offset = invert_m4(&offset);
        cap_part(cap, &start_offset, &mut positions);
        if use_merge {
            // Identify doubles with the first chunk.
            map_doubles(
                &mut full_doubles_map,
                &positions,
                0,
                chunk.verts,
                start_cap_start,
                sc.verts,
                s.merge_threshold,
            );
        }
    }
    if let Some(cap) = end_cap {
        let end_cap_start = positions.len();
        let end_offset = mul_m4_m4m4(&current_offset, &offset);
        cap_part(cap, &end_offset, &mut positions);
        if use_merge {
            // Identify doubles with the last chunk.
            map_doubles(
                &mut full_doubles_map,
                &positions,
                last_chunk_start,
                chunk.verts,
                end_cap_start,
                ec.verts,
                s.merge_threshold,
            );
        }
    }
    face_offsets.push(corner_verts.len() as i32);

    // Attributes: the chunks copy the mesh's, the caps their own where name, domain and type
    // match (`LegacyMeshInterpolator`), defaults elsewhere.
    let chunked = |n: usize, cap_n: usize| -> Carry {
        Carry::Gather(
            (0..count)
                .flat_map(|_| (0..n).map(Some))
                .chain(std::iter::repeat_n(None, cap_n))
                .collect(),
        )
    };
    let topo = Topology {
        positions,
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
    };
    let mut result = util::rebuild(
        mesh,
        topo,
        &DomainMaps {
            vert: chunked(chunk.verts, sc.verts + ec.verts),
            edge: chunked(chunk.edges, sc.edges + ec.edges),
            face: chunked(chunk.faces, sc.faces + ec.faces),
            corner: chunked(chunk.loops, sc.loops + ec.loops),
        },
    );
    let starts = Counts {
        verts: chunk.verts * count,
        edges: chunk.edges * count,
        loops: chunk.loops * count,
        faces: chunk.faces * count,
    };
    if let Some(cap) = start_cap {
        copy_cap_attributes(&mut result, cap, starts);
    }
    if let Some(cap) = end_cap {
        let at = Counts {
            verts: starts.verts + sc.verts,
            edges: starts.edges + sc.edges,
            loops: starts.loops + sc.loops,
            faces: starts.faces + sc.faces,
        };
        copy_cap_attributes(&mut result, cap, at);
    }

    // UVs.
    if chunk.loops > 0 && (s.offset_u != 0.0 || s.offset_v != 0.0) {
        for name in util::uv_map_names(&result) {
            let Some(uvs) = util::float2_mut(&mut result, &name) else {
                continue;
            };
            for c in 1..count {
                let off = [s.offset_u * c as f32, s.offset_v * c as f32];
                for uv in &mut uvs[c * chunk.loops..(c + 1) * chunk.loops] {
                    uv[0] += off[0];
                    uv[1] += off[1];
                }
            }
        }
    }

    // Merging.
    if use_merge {
        let mut tot_doubles = 0usize;
        for i in 0..result_nverts {
            let mut new_i = full_doubles_map[i];
            if new_i != -1 {
                // Follow chains of doubles (merging the caps is likely to create some).
                while !matches!(full_doubles_map[new_i as usize], t if t == -1 || t == new_i) {
                    new_i = full_doubles_map[new_i as usize];
                }
                if i as i32 == new_i {
                    full_doubles_map[i] = -1;
                } else {
                    full_doubles_map[i] = new_i;
                    tot_doubles += 1;
                }
            }
        }
        if tot_doubles > 0 {
            result = mesh_merge_verts(&result, &mut full_doubles_map, tot_doubles, false);
        }
    }
    result
}

/// Copies a cap's attributes into the cap's element ranges of `result` (the ones `result` has
/// with the same name, domain and type), adding `material_index` when the cap has one, and
/// remaps the cap's vertex-group weights by group name.
fn copy_cap_attributes(result: &mut Mesh, cap: &Mesh, at: Counts) {
    let cap_material = util::int_attr(cap, "material_index", Domain::Face).is_some();
    if cap_material && util::int_attr(result, "material_index", Domain::Face).is_none() {
        let n = result.faces_num();
        result.attributes_mut().insert(Attribute {
            name: "material_index".into(),
            domain: Domain::Face,
            data: AttrData::Int(std::sync::Arc::new(vec![0; n])),
        });
    }
    let names: Vec<smol_str::SmolStr> = result
        .attributes()
        .iter()
        .filter(|a| !util::is_topology(&a.name))
        .map(|a| a.name.clone())
        .collect();
    for name in names {
        let Some(src) = cap.attributes().get(&name) else {
            continue;
        };
        let Some(dst) = result.attributes().get(&name) else {
            continue;
        };
        if src.domain != dst.domain || src.data.data_type() != dst.data.data_type() {
            continue;
        }
        let base = match src.domain {
            Domain::Point => at.verts,
            Domain::Edge => at.edges,
            Domain::Face => at.faces,
            Domain::Corner => at.loops,
        };
        let src_data = src.data.clone();
        let len = src_data.len();
        if let Some(dst) = result.attributes_mut().get_mut(&name) {
            for i in 0..len {
                util::set_from(&mut dst.data, base + i, &src_data, i);
            }
        }
    }
    // Vertex groups (`BKE_object_defgroup_index_map_apply`): only when the result has weights.
    if result.deform_verts().is_some() {
        let remap: Vec<i32> = cap
            .vertex_group_names
            .iter()
            .map(|n| {
                result
                    .vertex_group_names
                    .iter()
                    .position(|r| r == n)
                    .map_or(-1, |i| i as i32)
            })
            .collect();
        let cap_dv: Vec<DeformVert> = cap
            .deform_verts()
            .map(|d| d.to_vec())
            .unwrap_or_else(|| vec![DeformVert::default(); cap.verts_num()]);
        let dv = result.deform_verts_mut();
        for (i, d) in cap_dv.into_iter().enumerate() {
            let mut d = d;
            d.weights.retain(|w| remap.get(w.0 as usize).is_some_and(|&m| m != -1));
            for w in d.weights.iter_mut() {
                w.0 = remap[w.0 as usize] as u32;
            }
            dv[at.verts + i] = d;
        }
    }
}

/// `len_v3` of a matrix column.
fn len3(c: &[f32; 4]) -> f32 {
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// `Mesh::bounds_min_max`.
fn bounds(p: &[[f32; 3]]) -> Option<[[f32; 3]; 2]> {
    let first = *p.first()?;
    let mut lo = first;
    let mut hi = first;
    for q in &p[1..] {
        for k in 0..3 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    Some([lo, hi])
}

// Ported from blender/source/blender/modifiers/intern/MOD_array.cc (dm_mvert_map_doubles).
/// Maps the vertices of the source range to their closest vertex of the target range within
/// `dist` (scanning both sorted by coordinate sum), following existing mappings while their
/// final target stays within `dist`; already mapped sources are skipped.
fn map_doubles(
    doubles_map: &mut [i32],
    positions: &[[f32; 3]],
    target_start: usize,
    target_verts_num: usize,
    source_start: usize,
    source_verts_num: usize,
    dist: f32,
) {
    let sum = |p: [f32; 3]| p[0] + p[1] + p[2];
    // Just above `sqrt(3)`.
    let dist3 = (1.732_050_8_f32 + 0.00005) * dist;
    let sorted = |start: usize, n: usize| -> Vec<(usize, [f32; 3], f32)> {
        let mut v: Vec<(usize, [f32; 3], f32)> = (start..start + n)
            .map(|i| (i, positions[i], sum(positions[i])))
            .collect();
        v.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        v
    };
    let targets = sorted(target_start, target_verts_num);
    let sources = sorted(source_start, source_verts_num);
    let mut low_bound = 0usize;
    let mut target_scan_completed = false;
    for &(vi, co, _) in &sources {
        let mut best_target_vertex: i32 = -1;
        let mut best_dist_sq = dist * dist;
        // Assigned in an earlier call (other chunks).
        if doubles_map[vi] != -1 {
            continue;
        }
        if target_scan_completed {
            doubles_map[vi] = -1;
            continue;
        }
        let source_sumco = sum(co);
        // Skip the targets more than `dist3` lower in coordinate sum (for every remaining
        // source as well).
        while low_bound < target_verts_num && targets[low_bound].2 < source_sumco - dist3 {
            low_bound += 1;
        }
        if low_bound >= target_verts_num {
            doubles_map[vi] = -1;
            target_scan_completed = true;
            continue;
        }
        let mut it = low_bound;
        while it < target_verts_num && targets[it].2 <= source_sumco + dist3 {
            let dist_sq = len_sq(co, targets[it].1);
            if dist_sq <= best_dist_sq {
                best_dist_sq = dist_sq;
                best_target_vertex = targets[it].0 as i32;
                // A mapped target is followed only while its final target stays close.
                while best_target_vertex != -1
                    && !matches!(doubles_map[best_target_vertex as usize], t if t == -1 || t == best_target_vertex)
                {
                    if compare_len(
                        positions[vi],
                        positions[doubles_map[best_target_vertex as usize] as usize],
                        dist,
                    ) {
                        best_target_vertex = doubles_map[best_target_vertex as usize];
                    } else {
                        best_target_vertex = -1;
                    }
                }
            }
            it += 1;
        }
        doubles_map[vi] = best_target_vertex;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_cubes_merge_into_a_bar() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let s = ArraySettings {
            count: 3,
            use_merge_vertices: true,
            ..ArraySettings::default()
        };
        let out = do_array(&s, &cube, None, None, None, &mut Vec::new());
        // 8 + 3·4 vertices, 3·6 − 4 faces (the touching faces merge into one each).
        assert_eq!(out.verts_num(), 16);
        assert_eq!(out.faces_num(), 16);
        let s = ArraySettings {
            count: 3,
            ..ArraySettings::default()
        };
        let out = do_array(&s, &cube, None, None, None, &mut Vec::new());
        assert_eq!(out.verts_num(), 24);
        assert_eq!(out.positions()[16][0], cube.positions()[0][0] + 4.0);
    }

    #[test]
    fn fit_length_counts_copies() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let s = ArraySettings {
            fit_type: "FIT_LENGTH".into(),
            fit_length: 7.0,
            ..ArraySettings::default()
        };
        let out = do_array(&s, &cube, None, None, None, &mut Vec::new());
        assert_eq!(out.verts_num(), 8 * 4);
        let mut errors = Vec::new();
        let s = ArraySettings {
            fit_type: "FIT_LENGTH".into(),
            use_relative_offset: false,
            ..ArraySettings::default()
        };
        let out = do_array(&s, &cube, None, None, None, &mut errors);
        assert_eq!(out.verts_num(), 8);
        assert_eq!(errors.len(), 1);
    }
}
