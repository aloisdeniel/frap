// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.reorder_vertices_spatial()`, tier T1
//! (`tests/golden/cases/edit/tools_m8/reorder_vertices_spatial.py`). Port of
//! `mesh_reorder_vertices_spatial_exec` and its poll (`editors/mesh/meshtools.cc`) and
//! `bke::mesh_apply_spatial_organization` (`blenkernel/intern/mesh.cc`, with the PBVH
//! partition helpers of `blenkernel/intern/pbvh.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll: the active object is a mesh. A mesh without faces or vertices cancels. The faces are
//! split recursively like a sculpt BVH (more than 2500 faces: halves along the dominant axis of
//! the face-centre bounds, through its middle, `std::partition` order; then by material
//! index); the leaves in node order give the new face order, and the vertices follow the
//! leaves (each leaf's own vertices in ascending index, then the ones it shares with earlier
//! leaves), loose vertices last. Point, face and corner attributes (vertex-group weights
//! included) are reordered, edges keep their order with remapped vertices; shape keys are not
//! touched (as in Blender). INFO `Mesh faces and vertices reordered spatially`.
//!
//! The operator works on the mesh datablock: in Edit Mode the edit BMesh is unchanged and
//! overwrites the mesh when Edit Mode is left, as in Blender.

use frap_mesh::{AttrData, Domain, Mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::MeshId;
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;

/// `mesh_reorder_vertices_spatial_poll`: the active object is a mesh.
fn active_mesh(c: &Context) -> Result<(), PollFail> {
    match poll::active(c) {
        Some((_, ob)) if ob.object_type() == "MESH" => Ok(()),
        _ => Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh object active: bpy.context.view_layer.objects.active = bpy.data.objects['Cube']",
        )),
    }
}

const ACTIVE_MESH: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveType("MESH")],
    check: active_mesh,
};

/// `STACK_FIXED_DEPTH`.
const STACK_FIXED_DEPTH: i32 = 100;
/// The `target_group_size` of `compute_local_mesh_groups`.
const TARGET_GROUP_SIZE: usize = 2500;

type Bounds = ([f32; 3], [f32; 3]);

/// `NonContiguousGroup`.
#[derive(Clone, Debug, Default)]
struct Group {
    unique_verts: Vec<i32>,
    faces: Vec<i32>,
    shared_verts: Vec<i32>,
    children_offset: usize,
}

fn negative_bounds() -> Bounds {
    ([f32::MAX; 3], [f32::MIN; 3])
}

fn min_max(p: [f32; 3], b: &mut Bounds) {
    for k in 0..3 {
        b.0[k] = b.0[k].min(p[k]);
        b.1[k] = b.1[k].max(p[k]);
    }
}

/// `math::dominant_axis`.
fn dominant_axis(a: [f32; 3]) -> usize {
    let (x, y, z) = (a[0].abs(), a[1].abs(), a[2].abs());
    if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    }
}

/// libc++ `std::partition` (the bidirectional-iterator algorithm, also used for random-access
/// iterators): the elements satisfying `pred` first; returns their count.
pub(crate) fn std_partition<T>(a: &mut [T], mut pred: impl FnMut(&T) -> bool) -> usize {
    let mut first = 0usize;
    let mut last = a.len();
    loop {
        loop {
            if first == last {
                return first;
            }
            if !pred(&a[first]) {
                break;
            }
            first += 1;
        }
        loop {
            last -= 1;
            if first == last {
                return first;
            }
            if pred(&a[last]) {
                break;
            }
        }
        a.swap(first, last);
        first += 1;
    }
}

/// `bke::pbvh::leaf_needs_material_split`.
fn leaf_needs_material_split(faces: &[i32], material: &[i32]) -> bool {
    if material.is_empty() || faces.is_empty() {
        return false;
    }
    let first = material[faces[0] as usize];
    faces.iter().any(|&f| material[f as usize] != first)
}

/// `partition_faces_recursively`.
#[allow(clippy::too_many_arguments)]
fn partition_faces_recursively(
    centers: &[[f32; 3]],
    faces: &mut [i32],
    groups: &mut Vec<Group>,
    node: usize,
    depth: i32,
    bounds_precalc: Option<Bounds>,
    material: &[i32],
) {
    let small = faces.len() <= TARGET_GROUP_SIZE || depth >= STACK_FIXED_DEPTH - 1;
    if small && !leaf_needs_material_split(faces, material) {
        groups[node].children_offset = 0;
        groups[node].faces = faces.to_vec();
        return;
    }
    let children_start = groups.len();
    groups[node].children_offset = children_start;
    groups.resize(children_start + 2, Group::default());

    let split = if !small {
        let bounds = bounds_precalc.unwrap_or_else(|| {
            let mut b = negative_bounds();
            for &f in faces.iter() {
                min_max(centers[f as usize], &mut b);
            }
            b
        });
        let ext = [
            bounds.1[0] - bounds.0[0],
            bounds.1[1] - bounds.0[1],
            bounds.1[2] - bounds.0[2],
        ];
        let axis = dominant_axis(ext);
        let middle = (bounds.0[axis] + bounds.1[axis]) * 0.5;
        // `partition_along_axis`.
        std_partition(faces, |&f| centers[f as usize][axis] >= middle)
    } else {
        // `partition_material_indices`.
        let first = material[faces[0] as usize];
        std_partition(faces, |&f| material[f as usize] == first)
    };
    let (front, back) = faces.split_at_mut(split);
    partition_faces_recursively(centers, front, groups, children_start, depth + 1, None, material);
    partition_faces_recursively(
        centers,
        back,
        groups,
        children_start + 1,
        depth + 1,
        None,
        material,
    );
}

/// `build_vertex_groups_for_leaves`.
fn build_vertex_groups_for_leaves(mesh: &Mesh, groups: &mut [Group]) {
    let corner_verts = mesh.corner_verts();
    let mut vert_used = vec![false; mesh.verts_num()];
    for g in groups.iter_mut() {
        if g.children_offset != 0 || g.faces.is_empty() {
            continue;
        }
        let mut verts: Vec<i32> = Vec::new();
        for &f in &g.faces {
            verts.extend_from_slice(&corner_verts[mesh.face(f as usize)]);
        }
        verts.sort_unstable();
        verts.dedup();
        for v in verts {
            if vert_used[v as usize] {
                g.shared_verts.push(v);
            } else {
                vert_used[v as usize] = true;
                g.unique_verts.push(v);
            }
        }
    }
}

/// `compute_local_mesh_groups`.
fn compute_local_mesh_groups(mesh: &Mesh) -> Vec<Group> {
    let faces_num = mesh.faces_num();
    if faces_num == 0 {
        return Vec::new();
    }
    let positions = mesh.positions();
    let corner_verts = mesh.corner_verts();
    let mut centers = Vec::with_capacity(faces_num);
    let mut total = negative_bounds();
    for f in 0..faces_num {
        // `bke::pbvh::calc_face_bounds` and `Bounds::center`.
        let mut b = negative_bounds();
        for &v in &corner_verts[mesh.face(f)] {
            min_max(positions[v as usize], &mut b);
        }
        centers.push([
            (b.0[0] + b.1[0]) * 0.5,
            (b.0[1] + b.1[1]) * 0.5,
            (b.0[2] + b.1[2]) * 0.5,
        ]);
        min_max(b.0, &mut total);
        min_max(b.1, &mut total);
    }
    let material: Vec<i32> = match mesh
        .attributes()
        .get("material_index")
        .filter(|a| a.domain == Domain::Face)
        .map(|a| &a.data)
    {
        Some(AttrData::Int(v)) => v.to_vec(),
        _ => Vec::new(),
    };
    let mut prim: Vec<i32> = (0..faces_num as i32).collect();
    let mut groups = vec![Group::default()];
    partition_faces_recursively(&centers, &mut prim, &mut groups, 0, 0, Some(total), &material);
    build_vertex_groups_for_leaves(mesh, &mut groups);
    groups
}

/// `gather` of one column: `out[i] = data[src[i]]`.
fn gather(data: &AttrData, src: &[usize]) -> AttrData {
    data.gather_with_default(src, src.len())
}

/// `mesh_apply_spatial_organization`.
pub(crate) fn apply_spatial_organization(mesh: &mut Mesh) {
    let verts_num = mesh.verts_num();
    if verts_num == 0 || mesh.faces_num() == 0 {
        return;
    }
    let groups = compute_local_mesh_groups(mesh);
    let mut new_vert_order: Vec<usize> = Vec::with_capacity(verts_num);
    let mut new_face_order: Vec<usize> = Vec::with_capacity(mesh.faces_num());
    let mut added = vec![false; verts_num];
    for g in &groups {
        for &v in g.unique_verts.iter().chain(&g.shared_verts) {
            if !added[v as usize] {
                new_vert_order.push(v as usize);
                added[v as usize] = true;
            }
        }
        new_face_order.extend(g.faces.iter().map(|&f| f as usize));
    }
    for (v, a) in added.iter_mut().enumerate() {
        if !*a {
            new_vert_order.push(v);
            *a = true;
        }
    }
    let mut vert_reverse_map = vec![0i32; verts_num];
    for (i, &v) in new_vert_order.iter().enumerate() {
        vert_reverse_map[v] = i as i32;
    }

    let edges: Vec<[i32; 2]> = mesh
        .edges()
        .iter()
        .map(|e| [vert_reverse_map[e[0] as usize], vert_reverse_map[e[1] as usize]])
        .collect();
    let old_offsets = mesh.face_offsets().to_vec();
    let corner_verts = mesh.corner_verts().to_vec();
    // The corner order of the new faces (old corner indices).
    let mut corner_src: Vec<usize> = Vec::with_capacity(corner_verts.len());
    let mut new_offsets: Vec<i32> = Vec::with_capacity(old_offsets.len());
    new_offsets.push(0);
    for &f in &new_face_order {
        let (s, e) = (old_offsets[f] as usize, old_offsets[f + 1] as usize);
        corner_src.extend(s..e);
        new_offsets.push(corner_src.len() as i32);
    }
    let new_corner_verts: Vec<i32> = corner_src
        .iter()
        .map(|&c| vert_reverse_map[corner_verts[c] as usize])
        .collect();

    // Vertex groups first (`foreach_vertex_group`), each written back through the weights
    // `VMutableArray` (`set`: a zero weight is stored only where the vertex is in the group).
    let groups_num = mesh.vertex_group_names.len();
    if groups_num > 0 {
        let dverts = mesh.deform_verts_mut();
        for g in 0..groups_num as u32 {
            let weight_of = |dv: &frap_mesh::DeformVert| {
                dv.weights
                    .iter()
                    .find(|w| w.0 == g)
                    .map_or(0.0, |w| w.1)
            };
            let values: Vec<f32> = new_vert_order.iter().map(|&o| weight_of(&dverts[o])).collect();
            for (dv, value) in dverts.iter_mut().zip(values) {
                if let Some(w) = dv.weights.iter_mut().find(|w| w.0 == g) {
                    w.1 = value;
                } else if value != 0.0 {
                    dv.weights.push((g, value));
                }
            }
        }
    }

    let corner_edges_new = {
        let ce = mesh.corner_edges();
        corner_src.iter().map(|&c| ce[c]).collect::<Vec<i32>>()
    };
    let names: Vec<(smol_str::SmolStr, Domain)> = mesh
        .attributes()
        .iter()
        .map(|a| (a.name.clone(), a.domain))
        .collect();
    mesh.set_topology(new_offsets, new_corner_verts, edges);
    let attrs = mesh.attributes_mut();
    for (name, domain) in names {
        if name == ".corner_vert" || name == ".edge_verts" {
            continue;
        }
        let Some(a) = attrs.get_mut(&name) else {
            continue;
        };
        match domain {
            Domain::Face => a.data = gather(&a.data, &new_face_order),
            Domain::Point => a.data = gather(&a.data, &new_vert_order),
            Domain::Corner if name == ".corner_edge" => {
                a.data = AttrData::Int(std::sync::Arc::new(corner_edges_new.clone()));
            }
            Domain::Corner => a.data = gather(&a.data, &corner_src),
            Domain::Edge => {}
        }
    }
}

#[operator(poll = ACTIVE_MESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::ReorderVerticesSpatial) -> OpResult {
    let Some(me) = poll::active(&cx.ctx()).and_then(|(_, o)| o.data?.typed::<MeshId>()) else {
        return Ok(OpStatus::Cancelled);
    };
    let (faces, verts) = match cx.main().meshes.get(me) {
        Some(m) => (m.faces_num(), m.verts_num()),
        None => return Ok(OpStatus::Cancelled),
    };
    if faces == 0 || verts == 0 {
        return Ok(OpStatus::Cancelled);
    }
    apply_spatial_organization(cx.mesh_mut(me)?);
    cx.report(
        ReportType::Info,
        "Mesh faces and vertices reordered spatially",
    );
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_matches_libcxx() {
        // libc++: [1, 8, 3, 6, 5, 4, 7, 2] with "even first" swaps 1<->2, 3<->4, 5<->6.
        let mut a = [1, 8, 3, 6, 5, 4, 7, 2];
        let n = std_partition(&mut a, |x| x % 2 == 0);
        assert_eq!(n, 4);
        assert_eq!(a, [2, 8, 4, 6, 5, 3, 7, 1]);
        let mut e: [i32; 0] = [];
        assert_eq!(std_partition(&mut e, |_| true), 0);
        let mut all = [2, 4];
        assert_eq!(std_partition(&mut all, |x| x % 2 == 0), 2);
    }

    #[test]
    fn dominant_axis_prefers_later_on_ties() {
        assert_eq!(dominant_axis([1.0, 1.0, 1.0]), 2);
        assert_eq!(dominant_axis([2.0, 1.0, 1.0]), 0);
        assert_eq!(dominant_axis([1.0, 2.0, 1.0]), 1);
        assert_eq!(dominant_axis([2.0, 2.0, 1.0]), 1);
    }
}
