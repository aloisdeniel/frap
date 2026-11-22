// SPDX-License-Identifier: GPL-3.0-or-later
//! BOOLEAN `solver='FLOAT'`: the object's mesh and each operand in one BMesh, cut and
//! classified by `BM_mesh_intersect` (`frap_bmesh::ops::intersect`).
//!
// Ported from blender/source/blender/modifiers/intern/MOD_boolean.cc (get_quick_mesh,
// bm_face_isect_pair, BMD_mesh_bm_create, BMD_mesh_intersection and the FLOAT part of
// modify_mesh), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The BMesh holds the operand first (its vertices, edges and faces, flipped when exactly one
//! of the two objects has a negative scale), then the object's mesh; the custom-data layers
//! are the object's mesh's followed by the operand's others (`BM_mesh_copy_init_customdata_
//! from_mesh_array` keeps the object's mesh first), an element without a layer gets the
//! default. The operand faces are tagged (`BM_ELEM_SELECT_UV`, `bm_face_isect_pair`'s side 1),
//! the triangulation is `BM_mesh_calc_tessellation_beauty` of the untransformed operand, then
//! the operand's vertices move into the object's space, its face normals with the inverse
//! transpose and its material slots are remapped by index (`BKE_object_material_remap_calc`).
//! Blender's off-by-zero is kept: an operand without vertices (or faces) makes the loops
//! transform (tag) every vertex (face), which only a COLLECTION operand reaches, since an
//! OBJECT operand without faces takes `get_quick_mesh`.

use std::sync::Arc;

use frap_bmesh::ops::intersect::tessellate::mesh_calc_tessellation_beauty;
use frap_bmesh::ops::intersect::{BooleanMode, IntersectIn, intersect};
use frap_bmesh::{BMesh, ElemFlag, FromMeshParams, ToMeshParams};
use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use frap_rna::{IdRef, MaterialId, ObjectId};
use frap_scene::generated::modifier_settings::BooleanSettings;
use glam::{Mat3, Mat4, Vec3};

use super::{collection_objects, concat, material_remap_index_based, object_materials};
use crate::prelude::*;

/// One operand: its evaluated mesh, world matrix and material slots.
struct Operand {
    mesh: Arc<Mesh>,
    world: Mat4,
    slots: Vec<Option<IdRef<MaterialId>>>,
}

/// The FLOAT part of `modify_mesh` (after `BMD_error_messages`).
pub(super) fn float_boolean(
    s: &BooleanSettings,
    mesh: Mesh,
    cx: &mut dyn ModEvalCx,
) -> Result<Mesh, ModError> {
    let self_ref = cx.object_ref();
    let self_world = cx.object_matrix_world(self_ref);
    let self_slots = object_materials(cx.main(), cx.object());
    let mode = match s.operation.as_str() {
        "INTERSECT" => BooleanMode::Intersect,
        "UNION" => BooleanMode::Union,
        _ => BooleanMode::Difference,
    };
    if s.operand_type == "OBJECT" {
        let Some(ob) = s.object.and_then(|o| o.typed::<ObjectId>()) else {
            return Ok(mesh);
        };
        let Some(op) = operand(cx, ob) else {
            return Ok(mesh);
        };
        if let Some(quick) = quick_mesh(&mesh, self_world, &op, mode) {
            return Ok(quick);
        }
        return Ok(intersect_pair(
            &mesh,
            &mesh,
            self_world,
            &self_slots,
            &op,
            mode,
            s.double_threshold,
        ));
    }
    let mut result: Option<Mesh> = None;
    for ob in collection_objects(cx.main(), s.collection) {
        if ob == self_ref {
            continue;
        }
        let Some(op) = operand(cx, ob) else {
            continue;
        };
        // "Needed for multiple objects to work": the first result is made from the object's
        // mesh, the next ones are written over the previous result.
        let base = result.as_ref().unwrap_or(&mesh);
        let template = base;
        let r = intersect_pair(
            base,
            template,
            self_world,
            &self_slots,
            &op,
            mode,
            s.double_threshold,
        );
        result = Some(r);
    }
    Ok(result.unwrap_or(mesh))
}

/// The evaluated mesh of a mesh object (`BKE_modifier_get_evaluated_mesh_from_evaluated_
/// object`), or `None` (not a mesh, or not evaluated).
fn operand(cx: &mut dyn ModEvalCx, ob: IdRef<ObjectId>) -> Option<Operand> {
    let is_mesh = cx
        .main()
        .objects
        .get(ob)
        .is_some_and(|o| o.object_type() == "MESH");
    if !is_mesh {
        return None;
    }
    let mesh = cx.evaluated_mesh(ob).ok()?;
    let world = cx.object_matrix_world(ob);
    let slots = cx
        .main()
        .objects
        .get(ob)
        .map(|o| object_materials(cx.main(), o))
        .unwrap_or_default();
    Some(Operand { mesh, world, slots })
}

/// `invert_m4_m4(imat, ob_self->object_to_world); mul_m4_m4m4(omat, imat, ob_operand->
/// object_to_world)`.
fn operand_matrix(self_world: Mat4, op: &Operand) -> Mat4 {
    self_world.inverse() * op.world
}

/// `mul_m4_v3`.
fn mul_m4_v3(m: &Mat4, co: [f32; 3]) -> [f32; 3] {
    m.transform_point3(Vec3::from(co)).to_array()
}

/// `is_negative_m4`.
fn is_negative_m4(m: &Mat4) -> bool {
    let (x, y, z) = (
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    );
    x.cross(y).dot(z) < 0.0
}

/// `get_quick_mesh`: when one of the meshes has no faces, the result without a boolean.
fn quick_mesh(mesh: &Mesh, self_world: Mat4, op: &Operand, mode: BooleanMode) -> Option<Mesh> {
    if mesh.faces_num() != 0 && op.mesh.faces_num() != 0 {
        return None;
    }
    Some(match mode {
        BooleanMode::Intersect => Mesh::new(mesh.header.name.clone()),
        BooleanMode::Union if mesh.faces_num() != 0 => mesh.clone(),
        BooleanMode::Union => {
            let mut r = (*op.mesh).clone();
            let omat = operand_matrix(self_world, op);
            let pos: Vec<[f32; 3]> = r.positions().iter().map(|&p| mul_m4_v3(&omat, p)).collect();
            r.set_positions(pos);
            r
        }
        _ => mesh.clone(),
    })
}

/// The mesh `BMD_mesh_bm_create` loads into the BMesh: `first`'s elements, then `second`'s
/// (indices offset), with `second`'s layers first in the layer order.
fn join(first: &Mesh, second: &Mesh) -> Mesh {
    let (nv, ne, nf, nc) = (
        first.verts_num(),
        first.edges_num(),
        first.faces_num(),
        first.corners_num(),
    );
    let mut j = Mesh::new(second.header.name.clone());
    j.uv_active = second.uv_active.clone();
    j.uv_active_render = second.uv_active_render.clone();
    j.vertex_group_names = second.vertex_group_names.clone();
    let mut positions = first.positions().to_vec();
    positions.extend_from_slice(second.positions());
    j.set_positions(positions);
    let mut edges = first.edges().to_vec();
    edges.extend(
        second
            .edges()
            .iter()
            .map(|e| [e[0] + nv as i32, e[1] + nv as i32]),
    );
    let mut face_offsets = first.face_offsets().to_vec();
    face_offsets.extend(second.face_offsets()[1..].iter().map(|&o| o + nc as i32));
    let mut corner_verts = first.corner_verts().to_vec();
    corner_verts.extend(second.corner_verts().iter().map(|&v| v + nv as i32));
    let mut corner_edges = first.corner_edges().to_vec();
    corner_edges.extend(second.corner_edges().iter().map(|&e| e + ne as i32));
    j.set_topology(face_offsets, corner_verts, edges);
    j.attributes_mut().insert(Attribute {
        name: ".corner_edge".into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_edges)),
    });

    let sizes = |d: Domain, m: &Mesh| match d {
        Domain::Point => m.verts_num(),
        Domain::Edge => m.edges_num(),
        Domain::Face => m.faces_num(),
        Domain::Corner => m.corners_num(),
    };
    let builtin = |n: &str| {
        matches!(
            n,
            "position" | ".edge_verts" | ".corner_vert" | ".corner_edge"
        )
    };
    let mut names: Vec<(smol_str::SmolStr, Domain, AttrData)> = Vec::new();
    for m in [second, first] {
        for a in m.attributes().iter() {
            if builtin(&a.name) || names.iter().any(|(n, _, _)| *n == a.name) {
                continue;
            }
            names.push((a.name.clone(), a.domain, a.data.default_of_len(0)));
        }
    }
    for (name, domain, proto) in names {
        let parts: Vec<AttrData> = [first, second]
            .iter()
            .map(|m| {
                let n = sizes(domain, m);
                match m.attributes().get(&name) {
                    Some(a)
                        if a.domain == domain
                            && a.data.data_type() == proto.data_type()
                            && a.data.len() == n =>
                    {
                        a.data.clone()
                    }
                    _ => proto.default_of_len(n),
                }
            })
            .collect();
        let total = sizes(domain, first) + sizes(domain, second);
        j.attributes_mut().insert(Attribute {
            name,
            domain,
            data: concat(&parts, total),
        });
    }
    if first.deform_verts().is_some() || second.deform_verts().is_some() {
        let mut dv: Vec<DeformVert> = first
            .deform_verts()
            .map(<[DeformVert]>::to_vec)
            .unwrap_or_else(|| vec![DeformVert::default(); nv]);
        dv.extend(
            second
                .deform_verts()
                .map(<[DeformVert]>::to_vec)
                .unwrap_or_else(|| vec![DeformVert::default(); second.verts_num()]),
        );
        *j.deform_verts_mut() = dv;
    }
    let _ = nf;
    j
}

/// `BMD_mesh_bm_create` + `BMD_mesh_intersection` + `BKE_mesh_from_bmesh_for_eval_nomain`:
/// `base` cut by the operand; the non-geometry data comes from `template`.
fn intersect_pair(
    base: &Mesh,
    template: &Mesh,
    self_world: Mat4,
    self_slots: &[Option<IdRef<MaterialId>>],
    op: &Operand,
    mode: BooleanMode,
    double_threshold: f32,
) -> Mesh {
    let is_flip = is_negative_m4(&self_world) != is_negative_m4(&op.world);
    let joined = join(&op.mesh, base);
    let mut bm = BMesh::from_mesh(
        &joined,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            ..FromMeshParams::default()
        },
    );
    let faces = bm.face_ids();
    let verts = bm.vert_ids();
    // Blender's loops stop at `++i == end`: an end of 0 never stops them.
    let take = |n: usize, len: usize| if n == 0 { len } else { n.min(len) };
    let i_verts_end = take(op.mesh.verts_num(), verts.len());
    let i_faces_end = take(op.mesh.faces_num(), faces.len());
    // The self mesh is loaded second (`is_new == false`): its UV selection is not.
    for &f in &faces {
        bm.face_mut(f).flag.remove(ElemFlag::SELECT_UV);
    }
    if is_flip && op.mesh.faces_num() > 0 {
        for &f in &faces[..op.mesh.faces_num().min(faces.len())] {
            bm.face_normal_flip(f);
        }
    }

    // `BM_mesh_calc_tessellation_beauty`, before the operand moves (original normals).
    let looptris = mesh_calc_tessellation_beauty(&bm);

    let omat = operand_matrix(self_world, op);
    for &v in &verts[..i_verts_end] {
        frap_math::budget::check();
        let co = bm.vert(v).co;
        bm.vert_mut(v).co = mul_m4_v3(&omat, co);
    }
    // The face normals for `BM_face_split_edgenet`.
    let mut nmat = Mat3::from_mat4(omat).inverse();
    if is_flip {
        nmat = -nmat;
    }
    let nmat_t = nmat.transpose();
    let totcol = op.slots.len();
    let remap = material_remap_index_based(self_slots, &op.slots);
    for &f in &faces[..i_faces_end] {
        frap_math::budget::check();
        let no = Vec3::from(bm.face(f).no);
        let (n, _) = frap_geom::normals::normalize_v3((nmat_t * no).to_array());
        let fm = bm.face_mut(f);
        fm.no = n;
        // Temp tag to test which side split faces are from.
        fm.flag.insert(ElemFlag::SELECT_UV);
        fm.mat_nr = if (fm.mat_nr as i32) < totcol as i32 && fm.mat_nr >= 0 {
            remap.get(fm.mat_nr as usize).copied().unwrap_or(0) as i16
        } else {
            0
        };
    }

    let test = |bm: &BMesh, f: frap_bmesh::FaceId| -> i32 {
        i32::from(bm.face(f).flag.contains(ElemFlag::SELECT_UV))
    };
    let _ = intersect(
        &mut bm,
        &IntersectIn {
            looptris: &looptris,
            test_fn: &test,
            use_self: false,
            use_separate: false,
            use_dissolve: true,
            use_island_connect: true,
            use_partial_connect: false,
            use_edge_tag: false,
            boolean_mode: mode,
            eps: double_threshold,
        },
    );

    // The eval conversion writes no UV selection: drop the temporary tag.
    for f in bm.face_ids() {
        bm.face_mut(f).flag.remove(ElemFlag::SELECT_UV);
        for l in bm.face_loops(f) {
            bm.lp_mut(l)
                .flag
                .remove(ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE);
        }
    }
    bm.to_mesh(template, &ToMeshParams::default())
}
