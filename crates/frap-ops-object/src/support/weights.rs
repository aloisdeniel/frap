// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared parts of the M8 vertex-group and shape-key operators (`object.vertex_group_*`,
//! `vertex_weight_*`, `shape_key_*`, `update_shapes`; `editors/object/object_vgroup.cc`,
//! `object_shapekey.cc`): the vertex-group locks (`vgroup_validmap_get`, the subsets of
//! `group_select_mode` / `WT_VGROUP_*`), the mirror maps of `ED_vgroup_mirror` and the active
//! vertex of weight copying.
//!
//! WP 8.3.7. Ported from blender/source/blender/editors/object/object_vgroup.cc,
//! blender/source/blender/blenkernel/intern/object_deform.cc, deform.cc and modifier.cc
//! (Blender 5.2.4, GPL-2.0-or-later).
//!
//! * [`Weights`]: `vgroup_parray_alloc` as a snapshot. The weights of every vertex of a mesh
//!   (of its edit BMesh in Edit Mode, in vertex index order) with the selection and hiding
//!   flags; an operator edits the snapshot and [`Weights::store`] writes it back.
//! * [`subset`], [`subset_items`]: `BKE_object_defgroup_subset_from_select_type` and the
//!   `group_select_mode` items (`vgroup_selection_itemf_helper`), with the deform-bone map of
//!   `BKE_object_defgroup_validmap_get` and the pose-bone selection of
//!   `BKE_object_defgroup_selected_get`.
//! * [`symmetry`]: `Mesh.use_mirror_x` / `use_mirror_topology` (`ME_SYMMETRY_X`,
//!   `ME_EDIT_MIRROR_TOPO`), which frap meshes do not store yet: both are `False`, Blender's
//!   default, so the X-mirror weight sync of the weight tools never runs (deviations §2).
//!   The sync itself ([`mirror_sync`], [`mirror_assign`], [`mirror_update`]) is ported and
//!   tested.

pub mod defvert;
pub mod geom;
pub mod groups;
pub mod mirror;

use frap_bmesh::ElemFlag;
use frap_bmesh::shape::deform;
use frap_mesh::{DeformVert, Domain};
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, OpCtx};
use frap_rna::{IdRef, KeyId, MeshId, ObjectId};
use frap_scene::id::Object;
use frap_scene::{Context, ObjectMode};
use smol_str::SmolStr;

// ------------------------------------------------------------------------------- snapshot

/// `vgroup_parray_alloc(id, use_vert_sel = false)` as a snapshot: the weights of every
/// vertex, in vertex index order, and what the operators filter them by.
#[derive(Clone, Debug)]
pub struct Weights {
    /// The mesh.
    pub me: IdRef<MeshId>,
    /// Read from the edit BMesh.
    pub edit: bool,
    /// One entry per vertex.
    pub dv: Vec<DeformVert>,
    /// Vertex selection (`BM_ELEM_SELECT`, `.select_vert`).
    pub select: Vec<bool>,
    /// Vertex hiding (`BM_ELEM_HIDDEN`, `.hide_vert`).
    pub hide: Vec<bool>,
    /// The BMesh vertex of every index (Edit Mode).
    ids: Vec<frap_bmesh::VertId>,
}

/// Whether `me` is in Edit Mode (has an edit BMesh).
pub fn in_edit(cx: &mut OpCtx, me: IdRef<MeshId>) -> bool {
    cx.split_edit().1.meshes.contains_key(&me.key())
}

impl Weights {
    fn read(cx: &mut OpCtx, me: IdRef<MeshId>, create: bool) -> Option<Weights> {
        if in_edit(cx, me) {
            let (_, edit) = cx.split_edit();
            let em = edit.meshes.get(&me.key())?;
            let bm = &em.bm;
            let layer = bm.vdata.deform.as_ref();
            if layer.is_none() && !create {
                return None;
            }
            let ids = bm.vert_ids();
            let dv = ids
                .iter()
                .map(|v| {
                    layer
                        .and_then(|d| d.get(v.slot()))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
            let select = ids
                .iter()
                .map(|&v| bm.vert(v).flag.contains(ElemFlag::SELECT))
                .collect();
            let hide = ids
                .iter()
                .map(|&v| bm.vert(v).flag.contains(ElemFlag::HIDDEN))
                .collect();
            return Some(Weights {
                me,
                edit: true,
                dv,
                select,
                hide,
                ids,
            });
        }
        let m = cx.main().meshes.get(me)?;
        let n = m.verts_num();
        let dv = match m.deform_verts() {
            Some(d) => d.to_vec(),
            None if create => vec![DeformVert::default(); n],
            None => return None,
        };
        Some(Weights {
            me,
            edit: false,
            dv,
            select: deform::mesh_vert_selection(m),
            hide: m
                .bool_attribute(".hide_vert", Domain::Point)
                .map(|s| s.to_vec())
                .unwrap_or_else(|| vec![false; n]),
            ids: Vec::new(),
        })
    }

    /// `vgroup_parray_alloc`: `None` when the mesh has no weight layer (`CD_MDEFORMVERT`).
    pub fn load(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Weights> {
        Self::read(cx, me, false)
    }

    /// `BKE_object_defgroup_data_create` when the mesh has no weight layer, then
    /// [`Weights::load`]: every vertex without weights. The layer exists once stored.
    pub fn load_or_create(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Weights> {
        Self::read(cx, me, true)
    }

    /// Number of vertices.
    pub fn len(&self) -> usize {
        self.dv.len()
    }

    /// No vertices.
    pub fn is_empty(&self) -> bool {
        self.dv.is_empty()
    }

    /// `vgroup_parray_alloc(use_vert_sel)`: whether vertex `i` is in the array (selected, or
    /// every vertex without `use_vert_sel`).
    pub fn in_array(&self, i: usize, use_vert_sel: bool) -> bool {
        !use_vert_sel || self.select.get(i).copied().unwrap_or(false)
    }

    /// Writes the weights back (the edit BMesh in Edit Mode; the layer is created when
    /// missing).
    pub fn store(&self, cx: &mut OpCtx) -> Result<(), OpError> {
        if self.edit {
            let bm = cx.bmesh_mut(self.me)?;
            let d = deform::bmesh_deform_mut(bm);
            for (k, v) in self.ids.iter().enumerate() {
                if let Some(slot) = d.get_mut(v.slot()) {
                    *slot = self.dv[k].clone();
                }
            }
            return Ok(());
        }
        let m = cx.mesh_mut(self.me)?;
        let d = m.deform_verts_mut();
        let n = d.len().min(self.dv.len());
        d[..n].clone_from_slice(&self.dv[..n]);
        Ok(())
    }
}

/// `BKE_object_defgroup_remove_all`: every group of the object goes with its weights (from the
/// edit BMesh in Edit Mode) and the active index becomes -1. As `VertexGroups.clear()`, frap
/// keeps an all-empty weight column where Blender frees `CD_MDEFORMVERT`.
pub fn remove_all_groups(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
) -> Result<(), OpError> {
    if in_edit(cx, me)
        && let Some(d) = cx.bmesh_mut(me)?.vdata.deform.as_mut()
    {
        for dv in d.iter_mut() {
            dv.weights.clear();
        }
    }
    let m = cx.mesh_mut(me)?;
    if m.deform_verts().is_some() {
        for dv in m.deform_verts_mut().iter_mut() {
            dv.weights.clear();
        }
    }
    m.vertex_group_names.clear();
    m.vertex_group_locks.clear();
    cx.object_mut(ob)?.vertex_groups_active_index = -1;
    Ok(())
}

/// The vertex positions and edges of a mesh (of its edit BMesh in Edit Mode, by index), and
/// for every vertex its edges in Blender's iteration order (`BM_EDGES_OF_VERT` disk order in
/// Edit Mode, `build_vert_to_edge_map`'s edge order otherwise).
pub struct Topology {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Edge vertex pairs.
    pub edges: Vec<[usize; 2]>,
    /// The edges of every vertex.
    pub vert_edges: Vec<Vec<usize>>,
}

impl Topology {
    /// The topology of `me`.
    pub fn load(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Topology> {
        if in_edit(cx, me) {
            let (_, edit) = cx.split_edit();
            let bm = &edit.meshes.get(&me.key())?.bm;
            let vids = bm.vert_ids();
            let eids = bm.edge_ids();
            let mut vindex = vec![usize::MAX; bm.verts.capacity().max(1)];
            for (i, v) in vids.iter().enumerate() {
                if v.slot() >= vindex.len() {
                    vindex.resize(v.slot() + 1, usize::MAX);
                }
                vindex[v.slot()] = i;
            }
            let mut eindex = vec![usize::MAX; bm.edges.capacity().max(1)];
            for (i, e) in eids.iter().enumerate() {
                if e.slot() >= eindex.len() {
                    eindex.resize(e.slot() + 1, usize::MAX);
                }
                eindex[e.slot()] = i;
            }
            let positions = vids.iter().map(|&v| bm.vert(v).co).collect();
            let edges = eids
                .iter()
                .map(|&e| {
                    let ed = bm.edge(e);
                    [vindex[ed.v[0].slot()], vindex[ed.v[1].slot()]]
                })
                .collect();
            let vert_edges = vids
                .iter()
                .map(|&v| {
                    bm.vert_edges(v)
                        .into_iter()
                        .map(|e| eindex[e.slot()])
                        .collect()
                })
                .collect();
            return Some(Topology {
                positions,
                edges,
                vert_edges,
            });
        }
        let m = cx.main().meshes.get(me)?;
        let positions = m.positions().to_vec();
        let edges: Vec<[usize; 2]> = m
            .edges()
            .iter()
            .map(|e| [e[0] as usize, e[1] as usize])
            .collect();
        let mut vert_edges = vec![Vec::new(); positions.len()];
        for (k, e) in edges.iter().enumerate() {
            vert_edges[e[0]].push(k);
            if e[1] != e[0] {
                vert_edges[e[1]].push(k);
            }
        }
        Some(Topology {
            positions,
            edges,
            vert_edges,
        })
    }

    /// The vertex across edge `e` from `v`.
    pub fn other(&self, e: usize, v: usize) -> usize {
        let ed = self.edges[e];
        if ed[0] == v { ed[1] } else { ed[0] }
    }
}

/// `BM_mesh_active_vert_get` (`ED_mesh_active_dvert_get_em`): the index of the active vertex of
/// the edit BMesh, the last element of its selection history when it is a vertex, provided the
/// mesh has vertex groups and a weight layer. In Object Mode Blender reads
/// `BKE_mesh_mselect_active_get`, a selection history frap meshes do not keep outside Edit
/// Mode: `None`.
pub fn active_vert(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<usize> {
    if cx
        .main()
        .meshes
        .get(me)
        .is_none_or(|m| m.vertex_group_names.is_empty())
    {
        return None;
    }
    let (_, edit) = cx.split_edit();
    let bm = &edit.meshes.get(&me.key())?.bm;
    bm.vdata.deform.as_ref()?;
    let frap_bmesh::BMElem::Vert(v) = *bm.select_history.last()? else {
        return None;
    };
    bm.vert_ids().iter().position(|&x| x == v)
}

// --------------------------------------------------------------------- groups and locks

/// The group names of a mesh.
pub fn group_names(cx: &OpCtx, me: IdRef<MeshId>) -> Vec<SmolStr> {
    cx.main()
        .meshes
        .get(me)
        .map(|m| m.vertex_group_names.clone())
        .unwrap_or_default()
}

/// `BKE_object_defgroup_lock_flags_get`: the lock of every group, `None` when no group is
/// locked.
pub fn lock_flags(cx: &OpCtx, me: IdRef<MeshId>) -> Option<Vec<bool>> {
    cx.main()
        .meshes
        .get(me)
        .and_then(|m| m.vertex_group_lock_flags())
}

/// The lock of every group (all false when none is).
pub fn locks(cx: &OpCtx, me: IdRef<MeshId>) -> Vec<bool> {
    let n = cx
        .main()
        .meshes
        .get(me)
        .map_or(0, |m| m.vertex_group_names.len());
    lock_flags(cx, me).unwrap_or_else(|| vec![false; n])
}

/// `Mesh.use_mirror_x` and `Mesh.use_mirror_topology` (`ME_SYMMETRY_X`, `ME_EDIT_MIRROR_TOPO`).
/// frap meshes do not store them yet, so both are Blender's default, `False` (deviations §2).
pub fn symmetry(_cx: &OpCtx, _me: IdRef<MeshId>) -> (bool, bool) {
    (false, false)
}

/// The mesh of a mesh object.
pub fn mesh_of(c: &Context, ob: &Object) -> Option<IdRef<MeshId>> {
    let me = ob.data?.typed::<MeshId>()?;
    c.main.meshes.get(me).map(|_| me)
}

/// The active object and its mesh.
pub fn active_mesh(cx: &OpCtx) -> Option<(IdRef<ObjectId>, IdRef<MeshId>)> {
    let c = cx.ctx();
    let (ob, o) = poll::active(&c)?;
    Some((ob, mesh_of(&c, o)?))
}

/// `BKE_object_defgroup_active_index_get(ob) - 1` when it names a group.
pub fn active_group(c: &Context, ob: &Object) -> Option<usize> {
    let n = mesh_of(c, ob)
        .and_then(|me| c.main.meshes.get(me))
        .map_or(0, |m| m.vertex_group_names.len());
    usize::try_from(ob.vertex_groups_active_index)
        .ok()
        .filter(|&a| a < n)
}

// ------------------------------------------------------------------------------- subsets

/// `BKE_object_pose_context_check`: an armature with a pose, in Pose Mode.
fn pose_context(c: &Context, ob: Option<IdRef<ObjectId>>) -> Option<IdRef<ObjectId>> {
    let r = ob?;
    let o = c.main.objects.get(r)?;
    (o.object_type() == "ARMATURE" && o.pose.is_some() && o.mode == ObjectMode::Pose).then_some(r)
}

/// The armature objects of the object's armature modifiers, the virtual one first
/// (`BKE_modifiers_get_virtual_modifierlist`: a parent armature with `parent_type =
/// 'ARMATURE'`), each with whether the modifier is enabled in the viewport
/// (`eModifierMode_Realtime | eModifierMode_Virtual`). An unset modifier object is `None`.
fn armature_modifiers(c: &Context, ob: &Object) -> Vec<(Option<IdRef<ObjectId>>, bool)> {
    let mut out = Vec::new();
    if let Some(p) = ob.parent
        && ob.parent_type.as_str() == "ARMATURE"
        && c.main
            .objects
            .get(p)
            .is_some_and(|o| o.object_type() == "ARMATURE")
    {
        out.push((Some(p), true));
    }
    for m in &ob.modifiers {
        if m.settings.type_identifier() != "ARMATURE" {
            continue;
        }
        let target = m
            .settings
            .id_refs()
            .into_iter()
            .find_map(|(k, r)| (k == "object").then_some(r).flatten())
            .and_then(|r| r.typed::<ObjectId>())
            .filter(|r| c.main.objects.get(*r).is_some());
        out.push((target, m.show_viewport));
    }
    out
}

/// `BKE_modifiers_is_deformed_by_armature`: the first selected armature of the armature
/// modifiers (virtual one first), else the last modifier's armature.
pub fn deformed_by_armature(c: &Context, ob: &Object) -> Option<IdRef<ObjectId>> {
    let selected = c.selected_objects();
    let mut armature = None;
    for (target, _) in armature_modifiers(c, ob) {
        armature = target;
        if let Some(a) = target
            && selected.contains(&a)
        {
            return Some(a);
        }
    }
    armature
}

/// `BKE_object_pose_armature_get`: the object itself when it is an armature in Pose Mode, else
/// its deforming armature when that one is.
pub fn pose_armature(c: &Context, ob: IdRef<ObjectId>) -> Option<IdRef<ObjectId>> {
    if let Some(a) = pose_context(c, Some(ob)) {
        return Some(a);
    }
    let o = c.main.objects.get(ob)?;
    pose_context(c, deformed_by_armature(c, o))
}

/// The group names of an object (its mesh's).
fn names_of<'a>(c: &Context<'a>, ob: &Object) -> &'a [SmolStr] {
    mesh_of(c, ob)
        .and_then(|me| c.main.meshes.get(me))
        .map_or(&[], |m| m.vertex_group_names.as_slice())
}

/// `BKE_object_defgroup_validmap_get`: the groups named after a deform bone (a pose channel
/// whose bone has `use_deform`) of an armature of the enabled armature modifiers.
pub fn validmap_deform(c: &Context, ob: &Object) -> Vec<bool> {
    let names = names_of(c, ob);
    let mut valid = vec![false; names.len()];
    for (target, enabled) in armature_modifiers(c, ob) {
        if !enabled {
            continue;
        }
        let Some(a) = target.and_then(|a| c.main.objects.get(a)) else {
            continue;
        };
        let Some(pose) = a.pose.as_ref() else {
            continue;
        };
        let arm = a
            .data
            .and_then(|d| d.typed())
            .and_then(|d| c.main.armatures.get(d));
        for chan in &pose.bones {
            let deform = arm
                .and_then(|arm| arm.bone(&chan.name))
                .is_none_or(|b| b.use_deform);
            if !deform {
                continue;
            }
            if let Some(i) = names.iter().position(|n| *n == chan.name) {
                valid[i] = true;
            }
        }
    }
    valid
}

/// `BKE_object_defgroup_selected_get`: the groups named after a selected pose bone of the pose
/// armature ([`pose_armature`]); none without one.
pub fn selected_bones(c: &Context, ob: IdRef<ObjectId>) -> Vec<bool> {
    let Some(o) = c.main.objects.get(ob) else {
        return Vec::new();
    };
    let names = names_of(c, o);
    let pose = pose_armature(c, ob)
        .and_then(|a| c.main.objects.get(a))
        .and_then(|a| a.pose.as_ref());
    names
        .iter()
        .map(|n| pose.and_then(|p| p.bone(n)).is_some_and(|b| b.select))
        .collect()
}

/// `BKE_object_defgroup_subset_from_select_type`: the groups of the subset `mode` (`ACTIVE`,
/// `BONE_SELECT`, `BONE_DEFORM`, `ALL`; an empty mode is `default`) and their count.
pub fn subset(c: &Context, ob: IdRef<ObjectId>, mode: &str) -> (Vec<bool>, usize) {
    let Some(o) = c.main.objects.get(ob) else {
        return (Vec::new(), 0);
    };
    let n = names_of(c, o).len();
    let v = match mode {
        "ACTIVE" => {
            let mut v = vec![false; n];
            if let Some(a) = active_group(c, o) {
                v[a] = true;
            }
            v
        }
        "BONE_SELECT" => selected_bones(c, ob),
        "BONE_DEFORM" => validmap_deform(c, o),
        _ => vec![true; n],
    };
    let k = v.iter().filter(|x| **x).count();
    (v, k)
}

/// A `group_select_mode` value: the given one, or the property default (`ACTIVE` when the
/// property offers it, else `ALL`) for an unset (empty) one.
pub fn subset_mode(value: &str, with_active: bool) -> &str {
    match value {
        "" if with_active => "ACTIVE",
        "" => "ALL",
        v => v,
    }
}

/// `vgroup_selection_itemf_helper`: the `group_select_mode` items for the active object
/// (`ACTIVE` with `with_active`, `BONE_SELECT` with a pose armature, `BONE_DEFORM` when an
/// armature deforms it, `ALL`).
pub fn subset_items(c: &Context, with_active: bool) -> Vec<EnumItemOwned> {
    let item = |id: &str, name: &str, desc: &str| EnumItemOwned {
        identifier: id.into(),
        name: name.into(),
        description: desc.into(),
    };
    let mut v = Vec::new();
    if with_active {
        v.push(item("ACTIVE", "Active Group", "The active Vertex Group"));
    }
    if let Some((ob, o)) = poll::active(c) {
        if pose_armature(c, ob).is_some() {
            v.push(item(
                "BONE_SELECT",
                "Selected Pose Bones",
                "All Vertex Groups assigned to Selection",
            ));
        }
        if deformed_by_armature(c, o).is_some() {
            v.push(item(
                "BONE_DEFORM",
                "Deform Pose Bones",
                "All Vertex Groups assigned to Deform Bones",
            ));
        }
    }
    v.push(item("ALL", "All Groups", "All Vertex Groups"));
    v
}

/// `ToolSettings.vertex_group_subset` (`ts->vgroupsubset`), which `vertex_weight_copy` and
/// `vertex_weight_normalize_active_vertex` read. Not bound in frap yet: Blender's default,
/// `ALL` (deviations §2).
pub fn tool_subset(_c: &Context) -> &'static str {
    "ALL"
}

/// `vgroup_do_remap` after the group list of `me` was re-ordered to `new_names` (the same
/// names): every weight follows its group (`BKE_defvert_remap`, the edit BMesh in Edit Mode)
/// and the active index follows the active group. An active index past the last group stays
/// as it is (Blender reads past its remap table there).
pub fn reorder_groups(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    new_names: Vec<SmolStr>,
) -> Result<(), OpError> {
    let old = group_names(cx, me);
    let map = groups::remap_from_names(&old, &new_names);
    cx.mesh_mut(me)?.vertex_group_names = new_names;
    if let Some(mut w) = Weights::load(cx, me) {
        for dv in &mut w.dv {
            if !dv.weights.is_empty() {
                defvert::remap(dv, &map);
            }
        }
        w.store(cx)?;
    }
    let o = cx.object_mut(ob)?;
    if let Some(&to) = usize::try_from(o.vertex_groups_active_index)
        .ok()
        .and_then(|a| map.get(a))
    {
        o.vertex_groups_active_index = to as i32;
    }
    Ok(())
}

/// `check_vertex_group_accessible`: group `def_nr` exists (`Invalid vertex group index`) and
/// is not locked (`Vertex group is locked`); Blender reports these as errors, so the call
/// raises `RuntimeError`.
pub fn check_group_accessible(
    cx: &OpCtx,
    me: IdRef<MeshId>,
    def_nr: i32,
) -> Result<usize, OpError> {
    let m = cx
        .main()
        .meshes
        .get(me)
        .ok_or_else(|| OpError::Runtime("Invalid vertex group index".into()))?;
    let g = usize::try_from(def_nr)
        .ok()
        .filter(|&g| g < m.vertex_group_names.len())
        .ok_or_else(|| OpError::Runtime("Invalid vertex group index".into()))?;
    if m.vertex_group_locked(g) {
        return Err(OpError::Runtime("Vertex group is locked".into()));
    }
    Ok(g)
}

// ------------------------------------------------------------------------ mirror editing

/// `ED_mesh_report_mirror`: `N vertices mirrored` (INFO), `N vertices mirrored, M failed`
/// (WARNING).
pub fn report_mirror(cx: &mut OpCtx, totmirr: usize, totfail: usize) {
    if totfail > 0 {
        cx.report(
            ReportType::Warning,
            format!("{totmirr} vertices mirrored, {totfail} failed"),
        );
    } else {
        cx.report(ReportType::Info, format!("{totmirr} vertices mirrored"));
    }
}

/// `ED_mesh_mirror_get_vert` for every vertex (the Edit Mode cache in Edit Mode, the Object
/// Mode lookup otherwise), with the mesh's `use_mirror_topology`.
pub fn mirror_map(w: &Weights, topo: &Topology, use_topology: bool) -> Vec<Option<usize>> {
    if w.edit {
        // `editbmesh_get_x_mirror_vert`: the spatial lookup (nearest within the threshold)
        // or the topology table of the BMesh.
        if use_topology {
            return mirror::topology_table(topo.positions.len(), &topo.edges);
        }
        let t = mirror::SpatialTable::new(&topo.positions);
        return (0..topo.positions.len())
            .map(|i| {
                let p = topo.positions[i];
                if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()) {
                    return None;
                }
                t.mirror_of(i)
            })
            .collect();
    }
    mirror::object_mirror_map(&topo.positions, &topo.edges, use_topology)
}

/// `vgroup_parray_mirror_sync`: every vertex of the array (`in_array`) copies the subset of
/// its weights to its mirror vertex under the flipped group names
/// (`BKE_defvert_mirror_subset`); the mirror joins the array.
pub fn mirror_sync(
    dv: &mut [DeformVert],
    in_array: &mut [bool],
    mirr: &[Option<usize>],
    subset: &[bool],
    flip_map: &[i32],
) {
    for i in 0..dv.len() {
        if !in_array[i] {
            continue;
        }
        if let Some(j) = mirr[i] {
            if j == i {
                let src = dv[i].clone();
                defvert::mirror_subset(&mut dv[i], &src, subset, flip_map, true);
            } else {
                let src = dv[i].clone();
                defvert::mirror_subset(&mut dv[j], &src, subset, flip_map, false);
            }
            in_array[j] = true;
        }
    }
}

/// `vgroup_parray_mirror_assign`: an unselected vertex whose mirror (another vertex) is in the
/// array joins it.
pub fn mirror_assign(in_array: &mut [bool], mirr: &[Option<usize>]) {
    for i in 0..in_array.len() {
        if !in_array[i]
            && let Some(j) = mirr[i]
            && j != i
            && in_array[j]
        {
            in_array[i] = true;
        }
    }
}

/// `mesh_defvert_mirror_update_internal(ob, dst, src, def_nr)`: with `group = None` every
/// weight of `src` is written to its unlocked flip partner in `dst`
/// (`BKE_object_defgroup_flip_map_unlocked`, `BKE_defvert_sync_mapped` with ensure), else the
/// weight of `group` is written to its flipped group.
pub fn mirror_update(
    dst: &mut DeformVert,
    src: &DeformVert,
    group: Option<usize>,
    names: &[SmolStr],
    locks: &[bool],
) {
    match group {
        None => {
            let flip_map = groups::flip_map(names, locks, true, true);
            defvert::sync_mapped(dst, src, &flip_map, true);
        }
        Some(g) => {
            if let Some(f) = groups::flip_index(names, g, true) {
                *deform::ensure(dst, f as u32) = deform::weight(src, g as u32);
            }
        }
    }
}

/// `mesh_defvert_mirror_update_em/ob(ob, vidx)` for vertex `i` of a snapshot: copies its
/// weights to its mirror vertex ([`mirror_update`]).
pub fn mirror_update_vert(
    w: &mut Weights,
    mirr: &[Option<usize>],
    i: usize,
    group: Option<usize>,
    names: &[SmolStr],
    locks: &[bool],
) {
    if let Some(j) = mirr.get(i).copied().flatten()
        && j != i
    {
        let src = w.dv[i].clone();
        mirror_update(&mut w.dv[j], &src, group, names, locks);
    }
}

// ----------------------------------------------------------------------------- shape keys

/// `get_weights_array` of shape-key evaluation: the weights of group `name` of `mesh` for every
/// vertex, `None` when the mesh has no such group or no weights (full influence).
pub fn group_weights(mesh: &frap_mesh::Mesh, name: &str) -> Option<Vec<f32>> {
    let g = mesh.vertex_group_names.iter().position(|n| n == name)? as u32;
    let dv = mesh.deform_verts()?;
    Some(dv.iter().map(|d| deform::weight(d, g)).collect())
}

/// `BKE_keyblock_from_object`: the object's key and active block (`shapenr - 1`).
pub fn active_block(c: &Context, ob: IdRef<ObjectId>) -> Option<(IdRef<KeyId>, usize)> {
    let o = c.main.objects.get(ob)?;
    let me = mesh_of(c, o)?;
    let k = c.main.meshes.get(me)?.shape_keys?;
    let key = c.main.shape_keys.get(k)?;
    let a = usize::try_from(o.active_shape_key_index).ok()?;
    (a < key.key_blocks.len()).then_some((k, a))
}

/// `shape_key_mirror` on block data: with the mirror map of the mesh, a vertex mirrored to
/// itself flips X, a pair swaps (both flipped) once. Returns `(mirrored, failed)`.
pub fn mirror_block(data: &mut [[f32; 3]], mirr: &[Option<usize>]) -> (usize, usize) {
    let mut tag = vec![false; data.len()];
    let (mut totmirr, mut totfail) = (0, 0);
    for i1 in 0..mirr.len().min(data.len()) {
        match mirr[i1] {
            Some(i2) if i2 == i1 => {
                data[i1][0] = -data[i1][0];
                tag[i1] = true;
                totmirr += 1;
            }
            Some(i2) if i2 < data.len() => {
                if !tag[i1] && !tag[i2] {
                    data.swap(i1, i2);
                    data[i1][0] = -data[i1][0];
                    data[i2][0] = -data[i2][0];
                    totmirr += 1;
                }
                tag[i1] = true;
                tag[i2] = true;
            }
            _ => totfail += 1,
        }
    }
    (totmirr, totfail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dv(w: &[(u32, f32)]) -> DeformVert {
        DeformVert {
            weights: w.iter().copied().collect(),
        }
    }

    #[test]
    fn mirror_sync_copies_to_the_flipped_group_of_the_mirror() {
        let names: Vec<SmolStr> = ["A.L", "A.R"].iter().map(SmolStr::new).collect();
        let flip_map = groups::flip_map(&names, &[], true, false);
        let mut d = vec![dv(&[(0, 0.75)]), dv(&[(1, 0.1)]), dv(&[(0, 0.5)])];
        let mut in_array = vec![true, false, false];
        let mirr = vec![Some(1), Some(0), Some(2)];
        mirror_sync(&mut d, &mut in_array, &mirr, &[true, true], &flip_map);
        assert_eq!(d[1].weights.as_slice(), &[(1, 0.75)]);
        assert_eq!(in_array, vec![true, true, false]);
        let mut in_array = vec![false, false, false];
        mirror_assign(&mut in_array, &mirr);
        assert_eq!(in_array, vec![false, false, false]);
        let mut in_array = vec![false, true, true];
        mirror_assign(&mut in_array, &mirr);
        assert_eq!(in_array, vec![true, true, true]);
    }

    #[test]
    fn mirror_update_skips_locked_partners() {
        let names: Vec<SmolStr> = ["A.L", "A.R", "B"].iter().map(SmolStr::new).collect();
        let src = dv(&[(0, 0.25), (2, 0.5)]);
        let mut dst = dv(&[(1, 0.9)]);
        mirror_update(&mut dst, &src, None, &names, &[false, false, false]);
        assert_eq!(dst.weights.as_slice(), &[(1, 0.25), (2, 0.5)]);
        // A.L and A.R locked: neither looks for its partner (the unlocked flip map is the
        // identity), so the weights land in their own groups.
        let mut dst = dv(&[(1, 0.9)]);
        mirror_update(&mut dst, &src, None, &names, &[true, true, false]);
        assert_eq!(dst.weights.as_slice(), &[(1, 0.9), (0, 0.25), (2, 0.5)]);
        let mut dst = DeformVert::default();
        mirror_update(&mut dst, &src, Some(0), &names, &[]);
        assert_eq!(dst.weights.as_slice(), &[(1, 0.25)]);
    }

    #[test]
    fn mirror_block_swaps_pairs_and_flips_centres() {
        let mut data = vec![
            [1.0, 2.0, 0.0],
            [-1.0, 3.0, 0.0],
            [0.5, 1.0, 0.0],
            [0.0, 4.0, 0.0],
        ];
        let mirr = vec![Some(1), Some(0), None, Some(3)];
        assert_eq!(mirror_block(&mut data, &mirr), (2, 1));
        assert_eq!(data[0], [1.0, 3.0, 0.0]);
        assert_eq!(data[1], [-1.0, 2.0, 0.0]);
        assert_eq!(data[2], [0.5, 1.0, 0.0]);
        assert_eq!(data[3], [-0.0, 4.0, 0.0]);
    }

    #[test]
    fn subset_modes() {
        assert_eq!(subset_mode("", true), "ACTIVE");
        assert_eq!(subset_mode("", false), "ALL");
        assert_eq!(subset_mode("BONE_DEFORM", true), "BONE_DEFORM");
    }
}
