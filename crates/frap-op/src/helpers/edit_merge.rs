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
//! rounding (exact when neither matrix rotates).
//!
//! **Pool slots.** Blender runs the `create_*` builder inside the edit BMesh, so its
//! allocations pop the edit BMesh's freed slots first (LIFO), in the builder's creation order.
//! For every builder that kills nothing and creates in index order (plane, cube, circle, grid,
//! monkey), creating the elements in index order through the kernel puts each in Blender's
//! slot. The cone and cylinder builder kills cap
//! centres, collapses zero-radius rings and dissolves fans into n-gons: its kills free slots
//! later allocations take, and `dissolve_faces` follows the slot order of the faces it joins.
//! The merge therefore runs `bmo_create_cone_exec` itself in the edit BMesh ([`BuildPlan`],
//! with frap-bmesh's `BM_edge_collapse`, `BM_faces_join` and island walker) and takes the
//! positions and UVs from the generator's mesh (`frap_mesh::primitives::trace` says which
//! builder ran). The circle's NGON fill creates its centre vertex first and kills it when the
//! fan is dissolved, so `bmo_create_circle_exec` is replayed the same way: the freed centre
//! slot is the one the next primitive's first vertex takes, as in Blender. `BM_mesh_calc_uvs_cube`, `_grid` and the cylinder's side quads hand out UVs in
//! face pool order: they are re-dealt in the edit BMesh's order ([`uvs_in_pool_order`]).
//! Not replayed: the UV and ico sphere builders recreate elements (subdivide, extrude +
//! `remove_doubles`) in an order other than the index order, so a sphere added while the edit
//! mesh has freed slots does not fill them as Blender does, and the UV sphere's own freed slots
//! are not left behind for later allocations.

use std::sync::Arc;

use frap_bmesh::walkers::WalkOpts;
use frap_bmesh::{BMElem, BMesh, CdLayer, CreateFlag, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use frap_math::glam::Mat4;
use frap_mesh::primitives::trace::{CircleTopology, ConeTopology};
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
    // The generator ran just before (same thread): its record, if it matches this mesh.
    let build = frap_mesh::primitives::trace::take_last()
        .filter(|b| b.counts == (mesh.verts_num(), mesh.edges_num(), mesh.faces_num()))
        .unwrap_or_default();
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
    let builder = match (build.cone, build.circle) {
        (Some(t), _) => Some(Builder::Cone(t)),
        (None, Some(t)) => Some(Builder::Circle(t)),
        (None, None) => None,
    };
    let new = match builder.and_then(|b| BuildPlan::new(b, &mesh)) {
        Some(plan) => plan.create(bm, &mesh, &to_edit)?,
        None => create(bm, &mesh, &to_edit)?,
    };
    if let (Some(layer), Some(uvs)) = (active_uv, prim_uvs) {
        let all: Vec<usize> = (0..mesh.faces_num()).collect();
        let uvs = if build.uvs_follow_face_order {
            uvs_in_pool_order(&new.faces, &mesh, uvs, &all)
        } else if build
            .cone
            .is_some_and(|t| !t.radius1_zero && !t.radius2_zero)
        {
            // `BM_mesh_calc_uvs_cone` unwraps the side quads of a cylinder in face order.
            let quads: Vec<usize> = all
                .into_iter()
                .filter(|&i| mesh.face(i).len() == 4)
                .collect();
            uvs_in_pool_order(&new.faces, &mesh, uvs, &quads)
        } else {
            uvs
        };
        write_uvs(bm, layer, &new, &mesh, &uvs);
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

/// The elements a merge created: `faces[i]` is face `i` of the primitive's mesh, and
/// `corners[i][j]` the loop of its corner `j` (the first loop of a face need not be its
/// first corner: a cap n-gon starts where `dissolve_faces` starts it).
struct Created {
    verts: Vec<VertId>,
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
    corners: Vec<Vec<LoopId>>,
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
    let corners = faces.iter().map(|f| bm.face_loops(*f)).collect();
    Ok(Created {
        verts,
        edges,
        faces,
        corners,
    })
}

/// `mul_m4_v3` (Blender's operation order: `m[0][k]·x + m[1][k]·y + m[2][k]·z + m[3][k]`).
fn mul_m4_v3(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    let c = m.to_cols_array_2d();
    std::array::from_fn(|k| c[0][k] * p[0] + c[1][k] * p[1] + c[2][k] * p[2] + c[3][k])
}

/// `BM_mesh_calc_uvs_cube` / `_grid` (and the side quads of `BM_mesh_calc_uvs_cone`) hand out
/// UVs by face iteration order: among the faces `subset` (mesh indices, in the generator's
/// order), the one that comes k-th in the edit BMesh's face pool gets the UVs of the k-th.
/// Returns the corner UVs rearranged accordingly (unchanged when the order is the same or the
/// faces differ in size).
fn uvs_in_pool_order(
    faces: &[FaceId],
    mesh: &Mesh,
    uvs: Vec<[f32; 2]>,
    subset: &[usize],
) -> Vec<[f32; 2]> {
    if faces.len() != mesh.faces_num() || subset.is_empty() {
        return uvs;
    }
    let mut by_slot: Vec<usize> = subset.to_vec();
    by_slot.sort_by_key(|&i| faces[i].slot());
    let n0 = mesh.face(subset[0]).len();
    if by_slot == subset || subset.iter().any(|&i| mesh.face(i).len() != n0) {
        return uvs;
    }
    let mut out = uvs.clone();
    for (&i, &k) in by_slot.iter().zip(subset) {
        // Face `i` comes where face `k` came in the generator: it takes `k`'s block.
        for (dst, src) in mesh.face(i).zip(mesh.face(k)) {
            if let (Some(d), Some(s)) = (out.get_mut(dst), uvs.get(src)) {
                *d = *s;
            }
        }
    }
    out
}

/// The primitive's UVs (`calc_uvs`) on the new faces' loops, corner by corner.
fn write_uvs(bm: &mut BMesh, layer: usize, new: &Created, mesh: &Mesh, uvs: &[[f32; 2]]) {
    bm.ldata.ensure_slots(bm.loops.capacity());
    let mut writes = Vec::new();
    for (i, loops) in new.corners.iter().enumerate() {
        for (l, c) in loops.iter().zip(mesh.face(i)) {
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

// ---------------------------------------------------------------------------------------------
// Cones and cylinders: `bmo_create_cone_exec` run in the edit BMesh.

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (bmo_create_cone_exec,
// Blender 5.2.4, GPL-2.0-or-later), without the matrix and `calc_uvs`: positions and UVs come
// from frap's generator, the topology and every allocation and kill from this port.
/// Builds the cone's topology in `bm` (positions zero); returns the vertices in creation order.
fn build_cone(bm: &mut BMesh, t: &ConeTopology) -> Vec<VertId> {
    let segs = t.segments as usize;
    let mut serial = Vec::with_capacity(2 * segs + 2);
    let vert = |bm: &mut BMesh, serial: &mut Vec<VertId>| {
        frap_math::budget::check();
        let v = bm.vert_create([0.0; 3], None, CreateFlag::empty());
        serial.push(v);
        v
    };
    // `BM_face_create_quad_tri`: `BM_face_create_verts(…, create_edges = true)`.
    let quad_tri = |bm: &mut BMesh, verts: &[VertId]| {
        bm.face_create_verts(verts, None, CreateFlag::empty(), true)
    };
    if segs == 0 {
        return serial;
    }
    let cent = t
        .cap_ends
        .then(|| (vert(bm, &mut serial), vert(bm, &mut serial)));
    let mut cap_faces: Vec<FaceId> = Vec::new();
    let mut side_faces: Vec<FaceId> = Vec::with_capacity(segs.saturating_sub(1));
    let (mut first, mut last) = (None, None);
    for i in 0..segs {
        let v1 = vert(bm, &mut serial);
        let v2 = vert(bm, &mut serial);
        if let Some((lv1, lv2)) = last {
            if let Some((c1, c2)) = cent {
                cap_faces.extend(quad_tri(bm, &[c1, lv1, v1]));
                cap_faces.extend(quad_tri(bm, &[c2, v2, lv2]));
            }
            side_faces.extend(quad_tri(bm, &[lv1, lv2, v2, v1]));
        } else {
            first = Some((v1, v2));
        }
        last = Some((v1, v2));
        let _ = i;
    }
    let ((v1, v2), (fv1, fv2)) = match (last, first) {
        (Some(l), Some(f)) => (l, f),
        _ => return serial,
    };
    if let Some((c1, c2)) = cent {
        cap_faces.extend(quad_tri(bm, &[c1, v1, fv1]));
        cap_faces.extend(quad_tri(bm, &[c2, fv2, v2]));
    }
    // The closing side face is not one of `side_faces` (`side_faces_len = segs - 1`).
    let _ = quad_tri(bm, &[v1, v2, fv2, fv1]);

    // "Collapse vertices at the first end": `BM_edge_collapse(l->prev->e, l->prev->v)`.
    if t.radius1_zero {
        if let Some((c1, _)) = cent {
            bm.vert_kill(c1);
        }
        for &f in &side_faces {
            if !bm.faces.contains(f.0) {
                continue;
            }
            let l = bm.lp(bm.face(f).l_first).prev;
            let (e, v) = (bm.lp(l).e, bm.lp(l).v);
            bm.join_vert_kill_edge(e, v, true, true, true);
        }
    }
    // "Collapse vertices at the second end": `BM_edge_collapse(l->next->e, l->next->v)`.
    if t.radius2_zero {
        if let Some((_, c2)) = cent {
            bm.vert_kill(c2);
        }
        for &f in &side_faces {
            if !bm.faces.contains(f.0) {
                continue;
            }
            let l = bm.lp(bm.face(f).l_first).next;
            let (e, v) = (bm.lp(l).e, bm.lp(l).v);
            bm.join_vert_kill_edge(e, v, true, true, true);
        }
    }
    if !t.cap_tris {
        dissolve_faces(bm, &cap_faces);
    }
    serial
}

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (bmo_create_circle_exec,
// Blender 5.2.4, GPL-2.0-or-later), without the matrix and `calc_uvs`.
/// Builds the circle's topology in `bm` (positions zero); returns the vertices in creation
/// order (the centre first with `cap_ends`).
fn build_circle(bm: &mut BMesh, t: &CircleTopology) -> Vec<VertId> {
    let segs = t.segments as usize;
    let mut serial = Vec::with_capacity(segs + 1);
    if segs == 0 {
        return serial;
    }
    let mut vert = |bm: &mut BMesh| {
        frap_math::budget::check();
        let v = bm.vert_create([0.0; 3], None, CreateFlag::empty());
        serial.push(v);
        v
    };
    let cent = t.cap_ends.then(|| vert(bm));
    let mut fan: Vec<FaceId> = Vec::with_capacity(if t.cap_ends { segs } else { 0 });
    let (mut first, mut last) = (None, None);
    for _ in 0..segs {
        let v1 = vert(bm);
        if let Some(lv1) = last {
            bm.edge_create(v1, lv1, None, CreateFlag::empty());
            if let Some(c) = cent {
                // `BM_face_create_quad_tri`: `BM_face_create_verts(…, create_edges = true)`.
                fan.extend(bm.face_create_verts(&[c, lv1, v1], None, CreateFlag::empty(), true));
            }
        }
        first.get_or_insert(v1);
        last = Some(v1);
    }
    let (Some(v1), Some(fv1)) = (last, first) else {
        return serial;
    };
    bm.edge_create(fv1, v1, None, CreateFlag::empty());
    if let Some(c) = cent {
        fan.extend(bm.face_create_verts(&[c, v1, fv1], None, CreateFlag::empty(), true));
        if !t.cap_tris {
            dissolve_faces(bm, &fan);
        }
    }
    serial
}

/// A builder replayed in the edit BMesh.
#[derive(Copy, Clone, Debug)]
enum Builder {
    /// `bmo_create_cone_exec` (cones and cylinders).
    Cone(ConeTopology),
    /// `bmo_create_circle_exec`.
    Circle(CircleTopology),
}

impl Builder {
    fn build(&self, bm: &mut BMesh) -> Vec<VertId> {
        match self {
            Builder::Cone(t) => build_cone(bm, t),
            Builder::Circle(t) => build_circle(bm, t),
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_dissolve.cc (bmo_dissolve_faces_exec
// with use_verts off, Blender 5.2.4, GPL-2.0-or-later).
/// `dissolve_faces faces=…`: the regions (`BMW_ISLAND_MANIFOLD` over the input faces, started
/// from each still-tagged input face in mesh order) joined with `BM_faces_join(…, do_del)`.
fn dissolve_faces(bm: &mut BMesh, faces: &[FaceId]) {
    let mut input: Vec<FaceId> = faces
        .iter()
        .copied()
        .filter(|f| bm.faces.contains(f.0))
        .collect();
    // `BMO_ITER` over a buffer filled from the flag: mesh (slot) order.
    input.sort_unstable_by_key(|f| f.0);
    let marked: std::collections::HashSet<FaceId> = input.iter().copied().collect();
    let mut tagged = marked.clone();
    let mut regions: Vec<Vec<FaceId>> = Vec::new();
    for &f in &input {
        if !tagged.contains(&f) {
            continue;
        }
        let mask = |_: &BMesh, x: FaceId| marked.contains(&x);
        let opts = WalkOpts {
            test_hidden: false,
            mask_face: Some(&mask),
            ..WalkOpts::default()
        };
        let region = bm.walk_face_island_ex(f, &opts, true);
        if region.len() >= 2 {
            for x in &region {
                tagged.remove(x);
            }
            regions.push(region);
        }
    }
    for region in regions {
        frap_math::budget::check();
        if let (Some(f_new), Some(_double)) = bm.faces_join(&region, true, true) {
            bm.face_kill(f_new);
        }
    }
}

/// A cone, cylinder or circle merge: the builder, checked against the generator's mesh by
/// building it once in an empty BMesh (where Blender's creation order is the mesh's index
/// order). `mesh_index[k]` is the mesh index of the k-th created vertex, `None` when it dies.
struct BuildPlan {
    builder: Builder,
    mesh_index: Vec<Option<usize>>,
}

impl BuildPlan {
    /// `None` when the port does not reproduce the generator's mesh (then the generic merge
    /// runs).
    fn new(builder: Builder, mesh: &Mesh) -> Option<BuildPlan> {
        let mut fresh = BMesh::new();
        let serial = builder.build(&mut fresh);
        let rank = dense_ranks(&fresh.verts.slots);
        let mesh_index: Vec<Option<usize>> = serial.iter().map(|v| rank[v.slot()]).collect();
        if fresh.verts_num() != mesh.verts_num()
            || fresh.edges_num() != mesh.edges_num()
            || fresh.faces_num() != mesh.faces_num()
        {
            return None;
        }
        // Every face, corner by corner from its first loop, is the mesh's face of that index.
        let cv = mesh.corner_verts();
        for (i, f) in fresh.face_ids().into_iter().enumerate() {
            let ours: Vec<Option<usize>> = fresh
                .face_verts(f)
                .into_iter()
                .map(|v| rank[v.slot()])
                .collect();
            let theirs: Vec<Option<usize>> = cv[mesh.face(i)]
                .iter()
                .map(|&v| usize::try_from(v).ok())
                .collect();
            if ours != theirs {
                return None;
            }
        }
        Some(BuildPlan {
            builder,
            mesh_index,
        })
    }

    /// Runs the builder in the edit BMesh, places the surviving vertices (`m`: generator space
    /// to edit object space) and matches the new faces to the mesh's.
    fn create(&self, bm: &mut BMesh, mesh: &Mesh, m: &Mat4) -> Result<Created, OpError> {
        let (old_v, old_e, old_f) = (
            live_slots(&bm.verts.slots),
            live_slots(&bm.edges.slots),
            live_slots(&bm.faces.slots),
        );
        let is_new = |old: &[bool], slot: usize| !old.get(slot).copied().unwrap_or(false);
        let serial = self.builder.build(bm);
        let positions = mesh.positions();
        let mut by_index: Vec<Option<VertId>> = vec![None; mesh.verts_num()];
        for (v, idx) in serial.iter().zip(&self.mesh_index) {
            if let Some(i) = *idx {
                if !bm.verts.contains(v.0) {
                    return Err(OpError::Runtime("primitive: cone vertex lost".into()));
                }
                bm.vert_mut(*v).co = mul_m4_v3(m, positions[i]);
                by_index[i] = Some(*v);
            }
        }
        let verts: Vec<VertId> = by_index.iter().flatten().copied().collect();
        let mesh_of: std::collections::HashMap<VertId, usize> = by_index
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (v, i)))
            .collect();
        let edges: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|e| is_new(&old_e, e.slot()))
            .collect();
        let new_faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|f| is_new(&old_f, f.slot()))
            .collect();
        let new_verts = bm
            .vert_ids()
            .into_iter()
            .filter(|v| is_new(&old_v, v.slot()))
            .count();
        if new_verts != mesh.verts_num()
            || verts.len() != mesh.verts_num()
            || edges.len() != mesh.edges_num()
            || new_faces.len() != mesh.faces_num()
        {
            return Err(OpError::Runtime(
                "primitive: the cone built in Edit Mode does not match its mesh".into(),
            ));
        }
        // Mesh faces by their (sorted) vertices.
        let cv = mesh.corner_verts();
        let mut key_of: std::collections::HashMap<Vec<usize>, usize> =
            std::collections::HashMap::with_capacity(mesh.faces_num());
        for i in 0..mesh.faces_num() {
            let mut k: Vec<usize> = cv[mesh.face(i)]
                .iter()
                .filter_map(|&v| usize::try_from(v).ok())
                .collect();
            k.sort_unstable();
            key_of.insert(k, i);
        }
        let mut faces: Vec<Option<FaceId>> = vec![None; mesh.faces_num()];
        let mut corners: Vec<Vec<LoopId>> = vec![Vec::new(); mesh.faces_num()];
        for f in new_faces {
            let loops = bm.face_loops(f);
            let idx: Vec<usize> = loops
                .iter()
                .filter_map(|l| mesh_of.get(&bm.lp(*l).v).copied())
                .collect();
            let mut k = idx.clone();
            k.sort_unstable();
            let i = *key_of.get(&k).ok_or_else(|| {
                OpError::Runtime("primitive: a cone face has no match in its mesh".into())
            })?;
            // The mesh face's corners, each matched to the loop at the same vertex.
            corners[i] = cv[mesh.face(i)]
                .iter()
                .map(|&v| {
                    let at = idx.iter().position(|&x| x as i32 == v).unwrap_or(0);
                    loops[at]
                })
                .collect();
            faces[i] = Some(f);
        }
        let faces: Vec<FaceId> = faces.into_iter().flatten().collect();
        Ok(Created {
            verts,
            edges,
            faces,
            corners,
        })
    }
}

/// Whether each slot is live.
fn live_slots<T>(slots: &[Option<T>]) -> Vec<bool> {
    slots.iter().map(Option::is_some).collect()
}

/// The dense index of every live slot (`None` for a free one).
fn dense_ranks<T>(slots: &[Option<T>]) -> Vec<Option<usize>> {
    let mut n = 0;
    slots
        .iter()
        .map(|s| {
            s.as_ref().map(|_| {
                n += 1;
                n - 1
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_mesh::primitives::circle::{Circle, circle};
    use frap_mesh::primitives::cone::{Cone, FillType, cone};

    /// The port of `bmo_create_circle_exec` reproduces the generator for every fill type.
    #[test]
    fn circle_port_matches_the_generator() {
        for vertices in [3, 4, 5, 8, 32] {
            for fill_type in [FillType::Nothing, FillType::Ngon, FillType::Trifan] {
                let mesh = circle(&Circle {
                    vertices,
                    fill_type,
                    ..Circle::default()
                });
                let t = frap_mesh::primitives::trace::take_last()
                    .and_then(|b| b.circle)
                    .expect("circle topology");
                assert!(
                    BuildPlan::new(Builder::Circle(t), &mesh).is_some(),
                    "vertices {vertices} {fill_type:?}"
                );
            }
        }
    }

    /// The NGON circle frees its centre vertex slot (created first, killed by the dissolve):
    /// the next element created takes it, as in Blender (a plane after an NGON circle in Edit
    /// Mode starts at the circle's first index).
    #[test]
    fn ngon_circle_leaves_its_centre_slot_free() {
        let mesh = circle(&Circle {
            vertices: 8,
            fill_type: FillType::Ngon,
            ..Circle::default()
        });
        let t = frap_mesh::primitives::trace::take_last()
            .and_then(|b| b.circle)
            .expect("circle topology");
        let plan = BuildPlan::new(Builder::Circle(t), &mesh).expect("plan");
        let mut bm = BMesh::new();
        for _ in 0..3 {
            bm.vert_create([0.0; 3], None, CreateFlag::empty());
        }
        let made = plan
            .create(&mut bm, &mesh, &Mat4::IDENTITY)
            .expect("circle");
        assert_eq!(made.verts.len(), 8);
        assert_eq!(bm.verts.free, vec![3]);
        let next = bm.vert_create([0.0; 3], None, CreateFlag::empty());
        assert_eq!(next.slot(), 3);
        assert_eq!(bm.validate(), Vec::<String>::new());
    }

    /// The port of `bmo_create_cone_exec` reproduces the generator (Blender's object-mode
    /// order, `spec/blender-5.2/primitives`) for every fill type, zero radius and segment count,
    /// so the edit merge never falls back for a cone or cylinder.
    #[test]
    fn cone_port_matches_the_generator() {
        for segments in [3, 4, 5, 8, 32] {
            for fill in [FillType::Nothing, FillType::Ngon, FillType::Trifan] {
                for (r1, r2) in [(1.0, 1.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.5, 2.0)] {
                    let p = Cone {
                        vertices: segments,
                        radius1: r1,
                        radius2: r2,
                        end_fill_type: fill,
                        ..Cone::default()
                    };
                    let mesh = cone(&p);
                    let build = frap_mesh::primitives::trace::take_last().expect("a record");
                    let t = build.cone.expect("cone topology");
                    assert!(
                        BuildPlan::new(Builder::Cone(t), &mesh).is_some(),
                        "segments {segments} {fill:?} radii ({r1}, {r2})"
                    );
                }
            }
        }
    }

    /// A cylinder merged into an edit BMesh with freed slots fills them and keeps every face's
    /// corners on the generator's vertices.
    #[test]
    fn cone_port_in_a_bmesh_with_freed_slots() {
        let p = Cone {
            vertices: 6,
            radius1: 1.0,
            radius2: 1.0,
            ..Cone::default()
        };
        let mesh = cone(&p);
        let t = frap_mesh::primitives::trace::take_last()
            .and_then(|b| b.cone)
            .expect("cone topology");
        let plan = BuildPlan::new(Builder::Cone(t), &mesh).expect("plan");
        let mut bm = BMesh::new();
        // Two cylinders: the second fills the slots the first one's caps freed.
        plan.create(&mut bm, &mesh, &Mat4::IDENTITY).expect("first");
        let freed = bm.faces.free.clone();
        assert!(!freed.is_empty());
        let second = plan
            .create(&mut bm, &mesh, &Mat4::IDENTITY)
            .expect("second");
        assert!(second.faces.iter().any(|f| freed.contains(&f.0)));
        assert_eq!(second.faces.len(), mesh.faces_num());
        for (i, loops) in second.corners.iter().enumerate() {
            let want: Vec<[f32; 3]> = mesh.corner_verts()[mesh.face(i)]
                .iter()
                .map(|&v| mesh.positions()[v as usize])
                .collect();
            let got: Vec<[f32; 3]> = loops.iter().map(|&l| bm.vert(bm.lp(l).v).co).collect();
            assert_eq!(got, want, "face {i}");
        }
        assert_eq!(bm.validate(), Vec::<String>::new());
    }
}
