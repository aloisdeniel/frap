// SPDX-License-Identifier: GPL-3.0-or-later
//! Adding a primitive while a mesh is in Edit Mode (`make_prim_init`, `make_prim_from_bmo_args`
//! and `make_prim_finish` in `editors/mesh/editmesh_add.cc`, `EDBM_op_call_and_selectf` in
//! `editmesh_utils.cc`, `ED_mesh_uv_ensure` / `ED_mesh_uv_add` in `mesh_data.cc`, Blender
//! 5.2.4, GPL-2.0-or-later). Owned by WP 2.2 (ARCHITECTURE §22.4).
//!
//! Blender builds the new geometry **into the edit BMesh** of the edit object; no object is
//! added and the operator answers the edit object:
//! 1. `calc_uvs` on a BMesh without a UV map adds one (`UVMap`, made unique among the
//!    attribute names), makes it the active UV map of the mesh and gives the **selected** faces
//!    their default UVs (`reset_uv_map`);
//! 2. the `bmesh.ops.create_*` call creates the vertices, edges and faces at the creation
//!    matrix `ED_object_new_primitive_matrix(edit object)`: the placement relative to the edit
//!    object's world matrix;
//! 3. every element is deselected (`BM_mesh_elem_hflag_disable_all`, which also clears the
//!    selection history), the new vertices are selected and the selection is flushed in
//!    vertex mode (`EDBM_selectmode_flush_ex(em, SCE_SELECT_VERTEX)`), so exactly the new
//!    vertices, edges and faces end up selected.
//!
//! The geometry comes from frap's generators (`frap_mesh::primitives`), built for an object at
//! the placement (`creation_matrix(object_matrix(at), …)`); it is moved into the edit object's
//! space with `inverse(edit object world) · object_matrix(at)`, the same transform up to float
//! rounding (exact when neither matrix rotates). Elements are created in the generator's order
//! (vertices, then edges, then faces, each in index order), which is Blender's creation order
//! for every primitive that kills no element while it is built; it is also the resulting index
//! order whenever the edit BMesh has no freed pool slots (the order Blender's pools would fill
//! freed slots in differs for the UV and ico spheres, whose builders kill elements).

use std::sync::Arc;

use frap_bmesh::{BMElem, BMesh, CdLayer, CreateFlag, EdgeId, ElemFlag, VertId};
use frap_math::glam::Mat4;
use frap_mesh::{AttrData, Mesh};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::Object;
use frap_scene::id::transform::world_matrix;
use smol_str::SmolStr;

use crate::ctx::{OpCtx, Placement};
use crate::def::OpError;

/// The UV map name `ED_mesh_uv_add(mesh, nullptr, …)` asks for (`DATA_("UVMap")`).
const UV_MAP: &str = "UVMap";

/// Merges `mesh` (built with the creation matrix of `at`) into the edit BMesh of `edit_ob`,
/// selecting only the new geometry; returns the edit object.
pub fn merge_into_edit_mesh(
    cx: &mut OpCtx,
    edit_ob: IdRef<ObjectId>,
    mesh: Mesh,
    at: Placement,
) -> Result<IdRef<ObjectId>, OpError> {
    let me = cx
        .main()
        .objects
        .get(edit_ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<MeshId>())
        .ok_or_else(|| OpError::Runtime("the edit object has no mesh".into()))?;
    let to_edit = edit_space(cx, edit_ob, &at);
    // The generators add a UV map exactly when `calc_uvs` is on.
    let prim_uvs = mesh.uv_maps().first().map(|(_, uv)| uv.to_vec());
    let uv_active = cx.main().meshes.get(me).and_then(|m| m.uv_active.clone());

    // 1. `ED_mesh_uv_ensure` (only with `calc_uvs`).
    let mut active_uv = {
        let bm = cx.bmesh_mut(me)?;
        active_uv_layer(bm, uv_active.as_deref())
    };
    if prim_uvs.is_some() && active_uv.is_none() {
        let name = {
            let bm = cx.bmesh_mut(me)?;
            let name = unique_layer_name(bm, UV_MAP);
            add_uv_layer(bm, &name);
            name
        };
        cx.mesh_mut(me)?.uv_active = Some(name.clone());
        active_uv = {
            let bm = cx.bmesh_mut(me)?;
            active_uv_layer(bm, Some(&name))
        };
    }

    // 2. + 3. Create, then select only the new elements.
    let bm = cx.bmesh_mut(me)?;
    deselect_all(bm);
    let new = create(bm, &mesh, &to_edit)?;
    if let (Some(layer), Some(uvs)) = (active_uv, prim_uvs) {
        write_uvs(bm, layer, &new.faces, &mesh, &uvs);
    }
    for v in &new.verts {
        set_select(bm, BMElem::Vert(*v));
    }
    for e in &new.edges {
        set_select(bm, BMElem::Edge(*e));
    }
    for f in &new.faces {
        set_select(bm, BMElem::Face(*f));
    }
    bm.normals_update();
    Ok(edit_ob)
}

/// `inverse(edit object world) · object_matrix(at)`: from the space the generator built the
/// mesh in (an object freshly added at `at`) to the edit object's local space.
fn edit_space(cx: &OpCtx, edit_ob: IdRef<ObjectId>, at: &Placement) -> Mat4 {
    let world = world_matrix(&cx.main().objects, edit_ob);
    let mut ob = Object::new("", None);
    ob.location = at.location;
    ob.rotation_euler = at.rotation;
    let placed = ob.matrix_basis();
    if world == Mat4::IDENTITY {
        placed
    } else {
        world.inverse() * placed
    }
}

/// The BMesh loop layer `calc_uvs` writes to (`CustomData_get_offset(&bm->ldata,
/// CD_PROP_FLOAT2)`: the active UV map), by position in `ldata`.
fn active_uv_layer(bm: &BMesh, active: Option<&str>) -> Option<usize> {
    let is_uv = |l: &CdLayer| matches!(l.data, AttrData::Float2(_)) && !l.name.starts_with('.');
    active
        .and_then(|n| bm.ldata.layers.iter().position(|l| l.name == n && is_uv(l)))
        .or_else(|| bm.ldata.layers.iter().position(is_uv))
}

/// `BKE_attribute_calc_unique_name` over the BMesh layers of every domain.
fn unique_layer_name(bm: &BMesh, requested: &str) -> SmolStr {
    let taken = |n: &str| {
        [&bm.vdata, &bm.edata, &bm.ldata, &bm.pdata]
            .iter()
            .any(|cd| cd.find(n).is_some())
    };
    frap_rna::naming::unique_name(&taken, requested, 63, "Attribute")
}

/// `BM_data_layer_add_named(…, CD_PROP_FLOAT2, name)` then `reset_uv_map` for the selected
/// faces (`ED_mesh_uv_add` with `do_init` and no other UV map).
fn add_uv_layer(bm: &mut BMesh, name: &str) {
    let mut uvs = vec![[0.0f32; 2]; bm.loops.capacity()];
    for (_, f) in bm.faces.iter() {
        if !f.flag.contains(ElemFlag::SELECT) {
            continue;
        }
        let reset = reset_face_uvs(f.len as usize);
        let mut l = f.l_first;
        for uv in reset {
            uvs[l.slot()] = uv;
            l = bm.loops.get(l.0).map_or(l, |x| x.next);
        }
    }
    bm.ldata.layers.push(CdLayer {
        name: name.into(),
        data: AttrData::Float2(Arc::new(uvs)),
    });
}

/// `mesh_uv_reset_array`, as `Mesh::add_uv_map` computes it for a mesh without UV maps (the
/// same values `uv_layers.new()` gives in Object Mode).
fn reset_face_uvs(len: usize) -> Vec<[f32; 2]> {
    let mut m = Mesh::new("");
    m.set_positions(vec![[0.0; 3]; len]);
    let n = len as i32;
    m.set_topology(
        vec![0, n],
        (0..n).collect(),
        (0..n).map(|i| [i, (i + 1) % n.max(1)]).collect(),
    );
    match m.add_uv_map(UV_MAP, true) {
        Some(name) => m
            .uv_maps()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, uv)| uv.to_vec())
            .unwrap_or_else(|| vec![[0.0; 2]; len]),
        None => vec![[0.0; 2]; len],
    }
}

/// `BM_mesh_elem_hflag_disable_all(bm, BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_SELECT, false)`:
/// the fast path (every element, hidden ones too) and the selection history cleared.
fn deselect_all(bm: &mut BMesh) {
    bm.select_history.clear();
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::SELECT);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::SELECT);
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(ElemFlag::SELECT);
    }
}

/// Selects a new (never hidden) element without flushing: the vertex-mode flush selects every
/// edge and face whose vertices are all selected, which are exactly the new ones.
fn set_select(bm: &mut BMesh, e: BMElem) {
    let flag = match e {
        BMElem::Vert(v) => bm.verts.get_mut(v.0).map(|x| &mut x.flag),
        BMElem::Edge(x) => bm.edges.get_mut(x.0).map(|x| &mut x.flag),
        BMElem::Face(f) => bm.faces.get_mut(f.0).map(|x| &mut x.flag),
    };
    if let Some(flag) = flag
        && !flag.contains(ElemFlag::HIDDEN)
    {
        flag.insert(ElemFlag::SELECT);
    }
}

/// The elements a merge created, in creation order.
struct Created {
    verts: Vec<VertId>,
    edges: Vec<EdgeId>,
    faces: Vec<frap_bmesh::FaceId>,
}

/// Creates the mesh's vertices (transformed by `m`), edges and faces in index order.
fn create(bm: &mut BMesh, mesh: &Mesh, m: &Mat4) -> Result<Created, OpError> {
    let mut verts = Vec::with_capacity(mesh.verts_num());
    for p in mesh.positions() {
        frap_math::budget::check();
        verts.push(bm.vert_create(mul_m4_v3(m, *p), None, CreateFlag::empty()));
    }
    let vert = |i: i32| -> Result<VertId, OpError> {
        usize::try_from(i)
            .ok()
            .and_then(|i| verts.get(i).copied())
            .ok_or_else(|| OpError::Runtime("primitive: vertex index out of range".into()))
    };
    let mut edges = Vec::with_capacity(mesh.edges_num());
    for e in mesh.edges() {
        frap_math::budget::check();
        edges.push(bm.edge_create(vert(e[0])?, vert(e[1])?, None, CreateFlag::empty()));
    }
    let mut faces = Vec::with_capacity(mesh.faces_num());
    let (cv, ce) = (mesh.corner_verts(), mesh.corner_edges());
    for f in 0..mesh.faces_num() {
        frap_math::budget::check();
        let r = mesh.face(f);
        let fv = cv[r.clone()]
            .iter()
            .map(|&v| vert(v))
            .collect::<Result<Vec<_>, _>>()?;
        let fe = ce[r]
            .iter()
            .map(|&e| {
                usize::try_from(e)
                    .ok()
                    .and_then(|e| edges.get(e).copied())
                    .ok_or_else(|| OpError::Runtime("primitive: edge index out of range".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let face = bm
            .face_create(&fv, &fe, None, CreateFlag::empty())
            .ok_or_else(|| OpError::Runtime("primitive: degenerate face".into()))?;
        faces.push(face);
    }
    Ok(Created {
        verts,
        edges,
        faces,
    })
}

/// `mul_m4_v3` (Blender's operation order: `m[0][k]·x + m[1][k]·y + m[2][k]·z + m[3][k]`).
fn mul_m4_v3(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    let c = m.to_cols_array_2d();
    std::array::from_fn(|k| c[0][k] * p[0] + c[1][k] * p[1] + c[2][k] * p[2] + c[3][k])
}

/// The primitive's UVs (`calc_uvs`) on the new faces' loops, in the face's corner order (the
/// first loop of a new face is its first corner).
fn write_uvs(
    bm: &mut BMesh,
    layer: usize,
    faces: &[frap_bmesh::FaceId],
    mesh: &Mesh,
    uvs: &[[f32; 2]],
) {
    bm.ldata.ensure_slots(bm.loops.capacity());
    let mut writes = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        let corners = mesh.face(i);
        for (l, c) in bm.face_loops(*f).into_iter().zip(corners) {
            if let Some(uv) = uvs.get(c) {
                writes.push((l.slot(), *uv));
            }
        }
    }
    if let Some(CdLayer {
        data: AttrData::Float2(col),
        ..
    }) = bm.ldata.layers.get_mut(layer)
    {
        let col = Arc::make_mut(col);
        for (slot, uv) in writes {
            if let Some(x) = col.get_mut(slot) {
                *x = uv;
            }
        }
    }
}
