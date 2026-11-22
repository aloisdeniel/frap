// SPDX-License-Identifier: GPL-3.0-or-later
//! VERTEX_WEIGHT_PROXIMITY (`VertexWeightProximityModifier`): sets the weights of the vertices
//! of `vertex_group` from their distance to `target`: the object's origin (`OBJECT` mode, the
//! same distance for every vertex, object to object), or the nearest of the target's evaluated
//! vertices, edges and faces (`GEOMETRY` mode, `proximity_geometry`; without one of them, or
//! without a target mesh, the distance from each vertex to the target's origin). Distances map
//! linearly from `min_dist` (weight 0) to `max_dist` (weight 1), then through `falloff_type`;
//! the influence and `normalize` follow. No vertex joins or leaves the group.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weightvgproximity.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `get_vert2geom_distance` (with its last-hit search bound),
//! `get_vert2ob_distance`, `get_ob2ob_distance`, `do_map` and `modify_mesh`. The BVH queries
//! are those of the data transfer (`crate::data_transfer::bvhutils`, Blender's
//! `bvhutils.cc`).

use frap_geom::bvh::Nearest;
use frap_rna::{IdAny, ObjectId};
use frap_scene::generated::modifier_settings::VertexWeightProximitySettings;

use super::vertex_weight_edit::weightvg::{
    self, Falloff, Mask, do_mask, falloff_rng, group_index, update_vg,
};
use super::weld::util::{M4, m4_from_glam, mul_m4_v3};
use crate::data_transfer::SpaceTransform;
use crate::data_transfer::bvhutils::MeshTree;
use crate::data_transfer::math::{V3, len_sq_v3v3, len_v3v3};
use crate::prelude::*;

/// `foreach_ID_link`: the target, the mask texture and its mapping object.
fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::VertexWeightProximity(w) = s {
        v.extend(w.target);
        v.extend(w.mask_texture);
        v.extend(w.mask_tex_map_object);
    }
    v
}

/// `is_disabled`: no vertex group or no target.
fn disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::VertexWeightProximity(w) if w.vertex_group.is_empty() => {
            Some("VertexWeightProximity: no vertex group")
        }
        ModifierSettings::VertexWeightProximity(w) if w.target.is_none() => {
            Some("VertexWeightProximity: no target")
        }
        _ => None,
    }
}

#[modifier(
    VERTEX_WEIGHT_PROXIMITY,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::VertexWeightProximity(s) = &md.settings else {
        return Ok(mesh);
    };
    if s.mask_texture.is_some() {
        return Err(weightvg::texture_unsupported("VERTEX_WEIGHT_PROXIMITY"));
    }
    let Some(target) = s.target.and_then(|t| t.typed::<ObjectId>()) else {
        return Ok(mesh);
    };
    if cx.main().objects.get(target).is_none() {
        return Ok(mesh);
    }
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    let target_world = m4_from_glam(cx.object_matrix_world(target));
    let uses_geometry = s.proximity_mode == "GEOMETRY"
        && ["VERTEX", "EDGE", "FACE"]
            .iter()
            .any(|g| s.proximity_geometry.contains(*g));
    let target_mesh = if uses_geometry {
        let is_mesh = cx
            .main()
            .objects
            .get(target)
            .is_some_and(|o| o.object_type() == "MESH");
        if is_mesh {
            cx.evaluated_mesh(target).ok()
        } else {
            None
        }
    } else {
        None
    };
    let name = cx.object().header.name.to_string();
    Ok(vertex_weight_proximity(
        s,
        mesh,
        &name,
        &ob_world,
        &target_world,
        target_mesh.as_deref(),
    ))
}

/// The location of a world matrix.
fn location(m: &M4) -> V3 {
    [m[3][0], m[3][1], m[3][2]]
}

/// `get_vert2geom_distance`: per vertex, the distances to the nearest target vertex, edge and
/// face (`None` for the kinds not asked), in the target's space.
fn vert2geom_distance(
    positions: &[V3],
    indices: Option<&[usize]>,
    trees: [Option<&MeshTree<'_>>; 3],
    loc2trgt: &SpaceTransform,
    n: usize,
) -> [Option<Vec<f32>>; 3] {
    let mut dist: [Option<Vec<f32>>; 3] =
        std::array::from_fn(|k| trees[k].map(|_| vec![0.0f32; n]));
    let mut last_hit_co = [[0.0f32; 3]; 3];
    let mut is_init = [false; 3];
    for iter in 0..n {
        let co = loc2trgt.apply(positions[indices.map_or(iter, |x| x[iter])]);
        for k in 0..3 {
            let (Some(tree), Some(d)) = (trees[k], dist[k].as_mut()) else {
                continue;
            };
            let mut nearest = Nearest::within(if is_init[k] {
                len_sq_v3v3(co, last_hit_co[k])
            } else {
                f32::MAX
            });
            tree.find_nearest(co, &mut nearest);
            d[iter] = nearest.dist_sq.sqrt();
            if nearest.index.is_some() {
                last_hit_co[k] = nearest.co;
                is_init[k] = true;
            }
        }
    }
    dist
}

/// `do_map`: distances to weights.
fn map_distances(w: &mut [f32], min_d: f32, max_d: f32) {
    let range_inv = 1.0 / (max_d - min_d);
    if max_d == min_d {
        for x in w.iter_mut().rev() {
            *x = if *x >= max_d { 1.0 } else { 0.0 };
        }
    } else if max_d > min_d {
        for x in w.iter_mut().rev() {
            *x = if *x >= max_d {
                1.0
            } else if *x <= min_d {
                0.0
            } else {
                (*x - min_d) * range_inv
            };
        }
    } else {
        for x in w.iter_mut().rev() {
            *x = if *x <= max_d {
                1.0
            } else if *x >= min_d {
                0.0
            } else {
                (*x - min_d) * range_inv
            };
        }
    }
}

/// `modify_mesh` of VERTEX_WEIGHT_PROXIMITY for an object at `ob_world`, the target at
/// `target_world` with its evaluated mesh (`None`: no mesh, distances to its origin).
pub(crate) fn vertex_weight_proximity(
    s: &VertexWeightProximitySettings,
    mut mesh: Mesh,
    object_name: &str,
    ob_world: &M4,
    target_world: &M4,
    target_mesh: Option<&Mesh>,
) -> Mesh {
    let verts_num = mesh.verts_num();
    if verts_num == 0 || mesh.vertex_group_names.is_empty() {
        return mesh;
    }
    let Some(g) = group_index(&mesh, &s.vertex_group) else {
        return mesh;
    };
    let Some(dvert) = mesh.deform_verts().filter(|d| !d.is_empty()) else {
        return mesh;
    };
    let mut indices = Vec::new();
    let mut org_w = Vec::new();
    for (i, dv) in dvert.iter().enumerate() {
        if let Some(k) = frap_bmesh::shape::deform::find(dv, g) {
            indices.push(i);
            org_w.push(dv.weights[k].1);
        }
    }
    let index_num = indices.len();
    if index_num == 0 {
        return mesh;
    }
    let idx = (index_num != verts_num).then_some(indices.as_slice());
    let positions = mesh.positions().to_vec();
    let obr_loc = location(target_world);
    let vert2ob = |out: &mut Vec<f32>| {
        for i in (0..index_num).rev() {
            let v = mul_m4_v3(ob_world, positions[idx.map_or(i, |x| x[i])]);
            out[i] = len_v3v3(v, obr_loc);
        }
    };
    let mut new_w = vec![0.0f32; index_num];
    if s.proximity_mode == "OBJECT" {
        let d = len_v3v3(location(ob_world), obr_loc);
        new_w.fill(d);
    } else {
        let use_v = s.proximity_geometry.contains("VERTEX");
        let use_e = s.proximity_geometry.contains("EDGE");
        let use_f = s.proximity_geometry.contains("FACE");
        match target_mesh.filter(|_| use_v || use_e || use_f) {
            Some(tm) => {
                let loc2trgt = SpaceTransform::from_matrices(ob_world, target_world);
                let tris = tm.looptris();
                let tv = use_v.then(|| MeshTree::verts(tm.positions(), None));
                let te = use_e.then(|| MeshTree::edges(tm.positions(), tm.edges()));
                let tf = use_f.then(|| {
                    MeshTree::tris(tm.positions(), tm.corner_verts(), &tris, tm.face_offsets(), None)
                });
                let d = vert2geom_distance(
                    &positions,
                    idx,
                    [tv.as_ref(), te.as_ref(), tf.as_ref()],
                    &loc2trgt,
                    index_num,
                );
                for i in 0..index_num {
                    let mut w = d[0].as_ref().map_or(f32::MAX, |v| v[i]);
                    if let Some(e) = &d[1] {
                        w = e[i].min(w);
                    }
                    if let Some(f) = &d[2] {
                        w = f[i].min(w);
                    }
                    new_w[i] = w;
                }
            }
            None => vert2ob(&mut new_w),
        }
    }
    map_distances(&mut new_w, s.min_dist, s.max_dist);
    let falloff = Falloff::from_identifier(&s.falloff_type);
    if s.invert_falloff || falloff != Falloff::None {
        let mut rng = falloff_rng(falloff, object_name);
        weightvg::do_map(&mut new_w, falloff, s.invert_falloff, rng.as_mut());
    }
    let mask = Mask {
        constant: s.mask_constant,
        vertex_group: &s.mask_vertex_group,
        invert_vertex_group: s.invert_mask_vertex_group,
    };
    do_mask(&mesh, idx, &mut org_w, &new_w, &mask);
    update_vg(
        mesh.deform_verts_mut(),
        g,
        idx,
        &org_w,
        false,
        0.0,
        false,
        0.0,
        s.normalize,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::weld::util::unit_m4;
    use frap_mesh::DeformVert;

    fn line_mesh() -> Mesh {
        let mut m = Mesh::new("l");
        let verts: Vec<V3> = (0..5).map(|i| [i as f32 * 0.5, 0.0, 0.0]).collect();
        m.from_pydata(&verts, &[[0, 1], [1, 2], [2, 3], [3, 4]], &[], false);
        m.vertex_group_names.push("G".into());
        for dv in m.deform_verts_mut().iter_mut() {
            *dv = DeformVert {
                weights: smallvec::smallvec![(0, 1.0)],
            };
        }
        m
    }

    fn weights(m: &Mesh) -> Vec<f32> {
        m.deform_verts()
            .unwrap()
            .iter()
            .map(|d| (d.weights[0].1 * 1e5).round() / 1e5)
            .collect()
    }

    #[test]
    fn distance_to_target_vertices_maps_linearly() {
        let mut s = VertexWeightProximitySettings {
            vertex_group: "G".into(),
            ..Default::default()
        };
        s.proximity_mode = "GEOMETRY".into();
        s.proximity_geometry = [smol_str::SmolStr::new("VERTEX")].into_iter().collect();
        s.min_dist = 0.0;
        s.max_dist = 2.0;
        s.mask_constant = 1.0;
        let mut target = Mesh::new("t");
        target.from_pydata(&[[0.0, 0.0, 0.0]], &[], &[], false);
        let id = unit_m4();
        let m = vertex_weight_proximity(&s, line_mesh(), "L", &id, &id, Some(&target));
        assert_eq!(weights(&m), [0.0, 0.25, 0.5, 0.75, 1.0]);
        // OBJECT mode: one distance for all (the objects coincide: 0).
        s.proximity_mode = "OBJECT".into();
        let m = vertex_weight_proximity(&s, line_mesh(), "L", &id, &id, None);
        assert_eq!(weights(&m), [0.0; 5]);
        // Inverted range.
        s.proximity_mode = "GEOMETRY".into();
        s.min_dist = 2.0;
        s.max_dist = 0.0;
        let m = vertex_weight_proximity(&s, line_mesh(), "L", &id, &id, Some(&target));
        assert_eq!(weights(&m), [1.0, 0.75, 0.5, 0.25, 0.0]);
    }
}
