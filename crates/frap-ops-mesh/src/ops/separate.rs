// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.separate(type='SELECTED')`, tier T1 (`tests/golden/cases/edit/misc/
//! separate_*.py`). Port of `edbm_separate_exec`, `mesh_separate_selected`,
//! `mesh_separate_material`, `mesh_separate_loose`, `mesh_separate_tagged`,
//! `mesh_separate_arrays` and `mesh_separate_material_assign_mat_nr`
//! (`editors/mesh/editmesh_tools.cc`), `BM_mesh_calc_edge_groups_as_arrays`
//! (`bmesh/intern/bmesh_query.cc`), `BM_mesh_copy_arrays` (`bmesh_mesh_duplicate.cc`) and the
//! `dest` path of `bmo_duplicate_exec` (`bmo_dupe.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! **In Edit Mode** (every mesh in Edit Mode, multi-object):
//! - `SELECTED`: the selected (visible) elements are copied into a new mesh, then deleted
//!   from the edit mesh with the `FACES` context (vertices and edges still used by unselected
//!   faces stay), and the edit mesh is deselected. With nothing selected in any edit mesh:
//!   `RuntimeError: Error: Nothing selected`.
//! - `MATERIAL`: while faces remain, the faces with the material index of the first face
//!   (and their vertices and edges) are separated; when they are all the faces left, the
//!   edit object keeps them instead, its mesh reduced to that one material (index 0).
//! - `LOOSE`: the connected parts (edge-connected, found from each vertex in vertex order by
//!   a depth-first walk) other than the first each become a new object; the edit mesh keeps
//!   the first part and is deselected. Elements of a part keep the walk order.
//!
//! Each part becomes a copy of the edit **object** (`object_add_duplicate`: name uniquified,
//! `Cube.001`, transform, modifiers and slots kept, not in Edit Mode, linked to the
//! collections of the original when it is selected, else to the active collection) with a
//! copy of its **mesh** (`USER_DUP_MESH`; action copied as the factory Duplicate Data
//! preferences say) whose geometry is the separated part (selection cleared), and it is
//! selected; the edit object stays active and in Edit Mode ([MESH §4.11]). With `MATERIAL`
//! the new mesh keeps only the part's material.
//!
//! **In Object Mode** `MATERIAL` and `LOOSE` work on every selected editable mesh object (the
//! remaining part is written back to its mesh); `SELECTED` is `RuntimeError: Error: Selection
//! not supported in object mode`. Nothing separated: `CANCELLED`.
//!
//! Shape keys are not carried into the new meshes (frap has no key-block data before M6;
//! WARNING); custom split normals are not carried (deviations §2).

use std::collections::BTreeMap;

use frap_bmesh::ops::delete::{DeleteIn, delete};
use frap_bmesh::ops::duplicate;
use frap_bmesh::{
    BMElem, BMesh, CdLayer, CreateFlag, EdgeId, ElemFlag, FaceId, FromMeshParams, ToMeshParams,
    VertId,
};
use frap_mesh::Mesh;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId, SceneId};
use frap_scene::id::{Collection, SlotLink, SlotOverride, layers};
use frap_scene::{BaseFlag, ChangeLog, Main, ObjectMode};
use frap_spec_ops_mesh::mesh::{self, separate::Type};

use super::split::edbm;

// ---------------------------------------------------------------- copying into a new BMesh

/// Copies `verts`, `edges` and `faces` of `src` into a new BMesh in this order (each list in
/// the order given; a face's loops from `l_first`), as `BM_mesh_copy_arrays` and the `dest`
/// path of `duplicate` do: positions, normals, flags (selection and UV selection cleared),
/// materials and every custom-data layer, vertex groups and shape keys. The new BMesh has
/// no selection history and no active face.
fn copy_to_new(src: &BMesh, verts: &[VertId], edges: &[EdgeId], faces: &[FaceId]) -> BMesh {
    let mut dst = BMesh::new();
    let mut vmap: BTreeMap<VertId, VertId> = BTreeMap::new();
    for &v in verts {
        let s = src.vert(v);
        let n = dst.vert_create(s.co, None, CreateFlag::SKIP_CD);
        let d = dst.vert_mut(n);
        d.no = s.no;
        d.flag = s.flag - ElemFlag::SELECT;
        vmap.insert(v, n);
    }
    let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    for &e in edges {
        let s = src.edge(e);
        let n = dst.edge_create(vmap[&s.v[0]], vmap[&s.v[1]], None, CreateFlag::SKIP_CD);
        dst.edge_mut(n).flag = s.flag - ElemFlag::SELECT;
        emap.insert(e, n);
    }
    let mut lslots: Vec<usize> = Vec::new();
    for &f in faces {
        let loops = src.face_loops(f);
        let vtar: Vec<VertId> = loops.iter().map(|&l| vmap[&src.lp(l).v]).collect();
        let edar: Vec<EdgeId> = loops.iter().map(|&l| emap[&src.lp(l).e]).collect();
        let n = dst
            .face_create(&vtar, &edar, None, CreateFlag::SKIP_CD)
            .expect("invariant: a copied face has corners");
        let s = src.face(f);
        let (no, mat, flag) = (s.no, s.mat_nr, s.flag);
        let d = dst.face_mut(n);
        d.no = no;
        d.mat_nr = mat;
        d.flag = flag - (ElemFlag::SELECT | ElemFlag::SELECT_UV);
        let keep = ElemFlag::SELECT | ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE;
        for (ls, ld) in loops.iter().zip(dst.face_loops(n)) {
            let lf = src.lp(*ls).flag - keep;
            dst.lp_mut(ld).flag = lf;
            lslots.push(ls.slot());
        }
    }
    // The new pools are dense in creation order: gather every column in that order.
    let vslots: Vec<usize> = verts.iter().map(|v| v.slot()).collect();
    let eslots: Vec<usize> = edges.iter().map(|e| e.slot()).collect();
    let fslots: Vec<usize> = faces.iter().map(|f| f.slot()).collect();
    let gather = |cd: &frap_bmesh::CustomData, slots: &[usize]| {
        let mut out = cd.clone();
        out.layers = cd
            .layers
            .iter()
            .map(|l| CdLayer {
                name: l.name.clone(),
                data: l.data.gather_with_default(slots, slots.len()),
            })
            .collect();
        out.deform = cd
            .deform
            .as_ref()
            .map(|d| slots.iter().map(|&s| d[s].clone()).collect());
        out
    };
    dst.vdata = gather(&src.vdata, &vslots);
    dst.edata = gather(&src.edata, &eslots);
    dst.pdata = gather(&src.pdata, &fslots);
    dst.ldata = gather(&src.ldata, &lslots);
    dst.shape_layers = src
        .shape_layers
        .iter()
        .map(|s| frap_bmesh::ShapeLayer {
            name: s.name.clone(),
            co: vslots
                .iter()
                .map(|&i| s.co.get(i).copied().unwrap_or([0.0; 3]))
                .collect(),
        })
        .collect();
    dst.select_mode = src.select_mode;
    dst
}

/// The creation order of `bmesh.ops.duplicate(geom=tagged, dest=bm_new)`: tagged vertices in
/// vertex order, tagged edges in edge order (missing vertices first), tagged faces in face
/// order (missing vertices, then missing edges, in loop order, first).
fn dupe_order(bm: &BMesh) -> (Vec<VertId>, Vec<EdgeId>, Vec<FaceId>) {
    let tag = ElemFlag::TAG;
    let mut vdone = rustc_hash::FxHashSet::default();
    let mut edone = rustc_hash::FxHashSet::default();
    let (mut vs, mut es, mut fs) = (Vec::new(), Vec::new(), Vec::new());
    for v in bm.vert_ids() {
        if bm.vert(v).flag.contains(tag) && vdone.insert(v) {
            vs.push(v);
        }
    }
    for e in bm.edge_ids() {
        if bm.edge(e).flag.contains(tag) && !edone.contains(&e) {
            for v in bm.edge(e).v {
                if vdone.insert(v) {
                    vs.push(v);
                }
            }
            edone.insert(e);
            es.push(e);
        }
    }
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(tag) {
            for v in bm.face_verts(f) {
                if vdone.insert(v) {
                    vs.push(v);
                }
            }
            for e in bm.face_edges(f) {
                if edone.insert(e) {
                    es.push(e);
                }
            }
            fs.push(f);
        }
    }
    (vs, es, fs)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc
// (BM_mesh_calc_edge_groups_as_arrays).
/// The edge-connected parts: `(verts, edges, faces)` per part, each in walk order.
#[allow(clippy::type_complexity)]
fn edge_groups(bm: &mut BMesh) -> Vec<(Vec<VertId>, Vec<EdgeId>, Vec<FaceId>)> {
    let tag = ElemFlag::TAG;
    clear_tags(bm);
    let mut groups = Vec::new();
    for v_init in bm.vert_ids() {
        if bm.vert(v_init).flag.contains(tag) {
            continue;
        }
        let (mut verts, mut edges, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        bm.vert_mut(v_init).flag.insert(tag);
        verts.push(v_init);
        if bm.vert(v_init).e.is_some() {
            let mut stack: Vec<VertId> = Vec::new();
            let mut v_iter = v_init;
            loop {
                frap_math::budget::check();
                for e in bm.vert_edges(v_iter) {
                    if bm.edge(e).flag.contains(tag) {
                        continue;
                    }
                    bm.edge_mut(e).flag.insert(tag);
                    edges.push(e);
                    for f in bm.edge_faces(e) {
                        if !bm.face(f).flag.contains(tag) {
                            bm.face_mut(f).flag.insert(tag);
                            faces.push(f);
                        }
                    }
                    let v_other = bm.edge_other_vert(e, v_iter);
                    if !bm.vert(v_other).flag.contains(tag) {
                        bm.vert_mut(v_other).flag.insert(tag);
                        verts.push(v_other);
                        stack.push(v_other);
                    }
                }
                match stack.pop() {
                    Some(v) => v_iter = v,
                    None => break,
                }
            }
        }
        groups.push((verts, edges, faces));
    }
    groups
}

/// `BM_mesh_elem_hflag_disable_all(bm, BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_TAG, false)`.
fn clear_tags(bm: &mut BMesh) {
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
    }
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.remove(ElemFlag::TAG);
    }
    for f in bm.face_ids() {
        bm.face_mut(f).flag.remove(ElemFlag::TAG);
    }
}

// ---------------------------------------------------------------- objects

/// The source object and the new BMesh of one separated part.
struct Part {
    bm: BMesh,
}

/// `BKE_collection_object_add_from` / the active collection: where `object_add_duplicate`
/// links the copy of `src` (every scene collection holding `src` when its base is selected,
/// else the context collection).
fn link_copy(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: IdRef<SceneId>,
    src_selected: bool,
    active_collection: Option<IdRef<frap_rna::CollectionId>>,
    src: IdRef<ObjectId>,
    new: IdRef<ObjectId>,
) {
    fn has(c: &Collection, ob: IdRef<ObjectId>) -> bool {
        c.objects.contains(&ob)
    }
    let mut targets: Vec<Option<IdRef<frap_rna::CollectionId>>> = Vec::new();
    if src_selected {
        if main.scenes.get(scene).is_some_and(|s| has(&s.collection, src)) {
            targets.push(None);
        }
        // Every collection below the master collection, depth first.
        let mut stack: Vec<IdRef<frap_rna::CollectionId>> = main
            .scenes
            .get(scene)
            .map(|s| s.collection.children.iter().rev().copied().collect())
            .unwrap_or_default();
        let mut seen = Vec::new();
        while let Some(c) = stack.pop() {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            let Some(col) = main.collections.get(c) else {
                continue;
            };
            if has(col, src) && col.header.library.is_none() {
                targets.push(Some(c));
            }
            stack.extend(col.children.iter().rev().copied());
        }
    }
    if targets.is_empty() {
        targets.push(active_collection);
    }
    for t in targets {
        match t {
            None => {
                if let Some(s) = main.scenes.get_mut(scene, log)
                    && !s.collection.objects.contains(&new)
                {
                    s.collection.objects.push(new);
                }
            }
            Some(c) => {
                if let Some(col) = main.collections.get_mut(c, log)
                    && !col.objects.contains(&new)
                {
                    col.objects.push(new);
                }
            }
        }
    }
}

/// `mesh_separate_material_assign_mat_nr`: `ob` and its mesh keep only slot `mat_nr` (as
/// slot 0); a mesh without materials stays without. `BKE_id_material_clear` also clears the
/// mesh's material indices (`material_index` is removed).
fn assign_mat_nr(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>, mat_nr: i16) {
    let Some(me) = main
        .objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<MeshId>())
    else {
        return;
    };
    if main
        .meshes
        .get(me)
        .is_some_and(|m| m.attributes().get("material_index").is_some())
        && let Some(m) = main.meshes.get_mut(me, log)
    {
        m.attributes_mut().remove("material_index");
    }
    let (Some(o), Some(m)) = (main.objects.get(ob), main.meshes.get(me)) else {
        return;
    };
    let i = mat_nr.max(0) as usize;
    let totcol = m.materials.len();
    let new_len = if totcol > 0 {
        let ov = o.material_slot_overrides.get(i).cloned().unwrap_or_default();
        let ov = if i < totcol { ov } else { SlotOverride::default() };
        let ma_obdata = m.materials.get(i).copied().flatten();
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials = vec![ma_obdata];
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.material_slot_overrides = vec![SlotOverride {
                link: if ov.link == SlotLink::Object {
                    SlotLink::Object
                } else {
                    SlotLink::Data
                },
                material: ov.material,
            }];
        }
        1
    } else {
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials.clear();
        }
        0
    };
    // `BKE_objects_materials_sync_length_all`: every user of the mesh has that many slots.
    let users: Vec<IdRef<ObjectId>> = main
        .objects
        .iter()
        .filter(|(r, o)| *r != ob && o.data.is_some_and(|d| d.typed::<MeshId>() == Some(me)))
        .map(|(r, _)| r)
        .collect();
    for u in users {
        if let Some(o) = main.objects.get_mut(u, log) {
            o.material_slot_overrides.truncate(new_len);
        }
    }
}

/// `object_add_duplicate` with `USER_DUP_MESH | USER_DUP_ACT` and the geometry of `part`
/// written into the mesh copy (`BM_mesh_bm_to_me`), then `base_select(SELECT)`. Returns the
/// new object.
fn add_part_object(
    cx: &mut OpCtx,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    part: Part,
) -> Result<IdRef<ObjectId>, OpError> {
    let (scene, vl_index, src_selected, active_collection) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let vl_index = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let selected = vl
            .bases
            .iter()
            .find(|b| b.object == src_ob)
            .is_some_and(|b| b.flag.contains(BaseFlag::SELECTED));
        (c.win.scene, vl_index, selected, c.collection())
    };
    let mut shape_keys_dropped = false;
    let ob_new = {
        let (main, log) = cx.main_mut();
        let template = main
            .meshes
            .get(src_me)
            .cloned()
            .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))?;
        let mut new_mesh: Mesh = part.bm.to_mesh(&template, &ToMeshParams::default());
        let h = &mut new_mesh.header;
        h.use_fake_user = false;
        h.library = None;
        h.is_linked_packed = false;
        if new_mesh.shape_keys.take().is_some() {
            shape_keys_dropped = true;
        }
        let mesh_name = template.header.name.clone();
        let me_new = main.meshes.insert(&mesh_name, new_mesh, log);
        let mut ob = main
            .objects
            .get(src_ob)
            .cloned()
            .ok_or_else(|| OpError::Runtime("object no longer exists".into()))?;
        ob.header.use_fake_user = false;
        ob.header.library = None;
        ob.header.is_linked_packed = false;
        ob.data = Some(me_new.any());
        ob.mode = ObjectMode::Object;
        // `USER_DUP_ACT`: the object's action is copied too.
        if let Some(ad) = ob.animation_data.as_mut()
            && let Some(act) = ad.action
            && let Some(a) = main.actions.get(act).cloned()
        {
            let name = a.header.name.clone();
            ad.action = Some(main.actions.insert(&name, a, log));
        }
        let ob_name = ob.header.name.clone();
        let ob_new = main.objects.insert(&ob_name, ob, log);
        link_copy(main, log, scene, src_selected, active_collection, src_ob, ob_new);
        layers::sync_all(main, log);
        layers::select_set(main, log, scene, vl_index, ob_new, true).map_err(OpError::Rna)?;
        log.context_changed = true;
        ob_new
    };
    if shape_keys_dropped {
        cx.report(
            ReportType::Warning,
            "frap: shape keys are not carried into separated meshes yet (M6)",
        );
    }
    Ok(ob_new)
}

// ---------------------------------------------------------------- separate modes

/// `mesh_separate_tagged`: the tagged elements go to a new object, then are deleted from `bm`
/// (`FACES` context) and `bm` is deselected.
fn separate_tagged(
    cx: &mut OpCtx,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    bm: &mut BMesh,
) -> Result<IdRef<ObjectId>, OpError> {
    let (vs, es, fs) = dupe_order(bm);
    let mut part = copy_to_new(bm, &vs, &es, &fs);
    part.normals_update();
    // The `duplicate` call on the old mesh is a top-level operator.
    bm.bmesh_edit_end(duplicate::TYPE_FLAG);
    let mut geom: Vec<BMElem> = Vec::new();
    for v in bm.vert_ids() {
        if bm.vert(v).flag.contains(ElemFlag::TAG) {
            geom.push(BMElem::Vert(v));
        }
    }
    for e in bm.edge_ids() {
        if bm.edge(e).flag.contains(ElemFlag::TAG) {
            geom.push(BMElem::Edge(e));
        }
    }
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::TAG) {
            geom.push(BMElem::Face(f));
        }
    }
    delete(
        bm,
        &DeleteIn {
            geom,
            context: "FACES",
        },
    )
    .map_err(|e| OpError::Runtime(format!("frap: internal bmesh error: {e}")))?;
    // De-select loose data (this used to get deleted).
    edbm::deselect_all(bm);
    add_part_object(cx, src_ob, src_me, Part { bm: part })
}

/// `mesh_separate_selected`.
fn separate_selected(
    cx: &mut OpCtx,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    bm: &mut BMesh,
) -> Result<bool, OpError> {
    clear_tags(bm);
    for v in bm.vert_ids() {
        let f = bm.vert(v).flag;
        if f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN) {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
    }
    for e in bm.edge_ids() {
        let f = bm.edge(e).flag;
        if f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN) {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
    }
    for x in bm.face_ids() {
        let f = bm.face(x).flag;
        if f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN) {
            bm.face_mut(x).flag.insert(ElemFlag::TAG);
        }
    }
    separate_tagged(cx, src_ob, src_me, bm)?;
    Ok(true)
}

/// `mesh_separate_material`.
fn separate_material(
    cx: &mut OpCtx,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    bm: &mut BMesh,
) -> Result<bool, OpError> {
    let mut result = false;
    while let Some(f_cmp) = bm.face_ids().first().copied() {
        frap_math::budget::check();
        let mat_nr = bm.face(f_cmp).mat_nr;
        clear_tags(bm);
        let mut tot = 0;
        for f in bm.face_ids() {
            if bm.face(f).mat_nr == mat_nr {
                bm.face_mut(f).flag.insert(ElemFlag::TAG);
                for l in bm.face_loops(f) {
                    let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                    bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                    bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                }
                tot += 1;
            }
        }
        // Leave the current object with some materials.
        if tot == bm.faces_num() {
            let (main, log) = cx.main_mut();
            assign_mat_nr(main, log, src_ob, mat_nr);
            for f in bm.face_ids() {
                bm.face_mut(f).mat_nr = 0;
            }
            break;
        }
        // Move the selection into a separate object.
        let ob_new = separate_tagged(cx, src_ob, src_me, bm)?;
        let (main, log) = cx.main_mut();
        assign_mat_nr(main, log, ob_new, mat_nr);
        result = true;
    }
    Ok(result)
}

/// `mesh_separate_loose`.
fn separate_loose(
    cx: &mut OpCtx,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    bm: &mut BMesh,
) -> Result<bool, OpError> {
    let groups = edge_groups(bm);
    if groups.len() <= 1 {
        return Ok(false);
    }
    // Separate out all groups except the first.
    for (verts, edges, faces) in groups.into_iter().skip(1) {
        let part = copy_to_new(bm, &verts, &edges, &faces);
        for &v in &verts {
            if bm.verts.contains(v.0) {
                bm.vert_kill(v);
            }
        }
        add_part_object(cx, src_ob, src_me, Part { bm: part })?;
    }
    edbm::deselect_all(bm);
    Ok(true)
}

fn run(
    cx: &mut OpCtx,
    ty: Type,
    src_ob: IdRef<ObjectId>,
    src_me: IdRef<MeshId>,
    bm: &mut BMesh,
) -> Result<bool, OpError> {
    match ty {
        Type::Selected => separate_selected(cx, src_ob, src_me, bm),
        Type::Material => separate_material(cx, src_ob, src_me, bm),
        Type::Loose => separate_loose(cx, src_ob, src_me, bm),
    }
}

#[operator(poll = poll::SCENE_EDITABLE, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Separate) -> OpResult {
    let ty = p.r#type.value;
    let mut changed_multi = false;
    if poll::editmesh(&cx.ctx()).is_ok() {
        let bases = helpers::iter_edit_meshes(cx);
        let mut empty_selection_len = 0;
        for &(ob, me) in &bases {
            if ty == Type::Selected {
                let (v, e, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
                if v == 0 && e == 0 && f == 0 {
                    empty_selection_len += 1;
                    if empty_selection_len == bases.len() {
                        return Err(OpError::Runtime("Nothing selected".into()));
                    }
                    continue;
                }
            }
            // The edit BMesh is taken out while new objects are added, then put back.
            let mut bm = std::mem::take(cx.bmesh_mut(me)?);
            let r = run(cx, ty, ob, me, &mut bm);
            *cx.bmesh_mut(me)? = bm;
            changed_multi |= r?;
        }
    } else {
        if ty == Type::Selected {
            return Err(OpError::Runtime(
                "Selection not supported in object mode".into(),
            ));
        }
        let selected = cx.ctx().selected_editable_objects();
        for ob in selected {
            let Some(me) = cx
                .main()
                .objects
                .get(ob)
                .and_then(|o| o.data)
                .and_then(|d| d.typed::<MeshId>())
            else {
                continue;
            };
            let Some(mesh) = cx.main().meshes.get(me) else {
                continue;
            };
            if mesh.header.library.is_some() {
                continue;
            }
            let mut bm = BMesh::from_mesh(
                mesh,
                &FromMeshParams {
                    calc_face_normal: true,
                    calc_vert_normal: true,
                    use_shapekey: false,
                    active_shapekey: 0,
                },
            );
            let changed = run(cx, ty, ob, me, &mut bm)?;
            if changed {
                let template = cx
                    .main()
                    .meshes
                    .get(me)
                    .cloned()
                    .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))?;
                let new = bm.to_mesh(&template, &ToMeshParams::default());
                *cx.mesh_mut(me)? = new;
            }
            changed_multi |= changed;
        }
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
