// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode in the session (ARCHITECTURE §6.3, §13.2): the cached read-only view of an edit
//! BMesh, the `RNA_EDITMODE_WRITE` guard on mesh geometry writes, and the `edit_mesh` block of
//! responses. Owned by WP 2.2 (Edit-mode integration, §22.4); M2 W0 wired every call site.
//!
//! - **Reads** of a mesh in Edit Mode go through [`mesh_view`]: one `BMesh::to_mesh` per BMesh
//!   generation (`EditMesh::bm_gen`), cached in `EditMesh::view`. `Main` is never touched, so no
//!   `gen` is bumped and `is_dirty` and the evaluation fingerprints stay as they are
//!   (deviation D-05: Blender reads the data from before Edit Mode until
//!   `update_from_editmode()`; frap reads are always fresh). The resolver reaches it through
//!   [`edit_view`] for every RNA path rooted at such a mesh.
//! - **Writes** to mesh geometry (element structs, layers, attribute data, `foreach_set`,
//!   `from_pydata`, `vertices.add`, `uv_layers.new`, …) of a mesh in Edit Mode raise
//!   `RuntimeError` code `RNA_EDITMODE_WRITE` with a `mode_set(mode='OBJECT')` hint (D-06:
//!   Blender silently drops them when Edit Mode is left). Non-geometry writes (`mesh.name`,
//!   `mesh.materials.append`, the active UV map or colour attribute) are allowed: they live on
//!   the original mesh, which `BMesh::to_mesh` uses as its template.
//!   `Object.data` of an object in Edit Mode is refused the same way (Blender's
//!   `rna_Object_data_set` ignores it silently; frap would otherwise lose the edit BMesh).
//! - **Responses** carry an `edit_mesh` block per object in Edit Mode (§13.2); the selection
//!   history is capped to its last [`HISTORY_CAP`] entries outside `--verbosity full`.
//! - **Hooks** ([`hooks`]): `tool_settings.mesh_select_mode` switches the active edit mesh
//!   (`EDBM_selectmode_set`), `Mesh.total_*_sel` read the edit BMesh's counts.

use std::sync::Arc;

use frap_bmesh::{BMElem, BMesh, SelectMode, ToMeshParams};
use frap_mesh::Mesh;
use frap_rna::{
    FrapError, FuncDef, IdAny, IdCode, IdRef, MeshId, ObjectId, PropDef, PyExc, RnaPtr, Root, Seg,
    codes,
};
use frap_scene::{ContextMode, EditMesh, EditState, HostRead, Main, ObjectMode};
use serde_json::{Value as J, json};

pub(crate) mod hooks;
#[cfg(test)]
mod tests;

/// Selection-history entries a `normal` response carries per object (the most recent ones);
/// `full` carries them all, and `select_history_len` always gives the total.
pub(crate) const HISTORY_CAP: usize = 8;

/// `Host::mesh_view` (§6.3): the edit-mode view of `mesh` while it is in Edit Mode, the
/// original otherwise.
pub(crate) fn mesh_view(main: &Main, edit: &EditState, mesh: IdRef<MeshId>) -> Arc<Mesh> {
    let original = main
        .meshes
        .get_arc(mesh)
        .expect("invariant: mesh_view of an existing mesh");
    match edit.meshes.get(&mesh.key()) {
        Some(em) => view_of(em, &original),
        None => original,
    }
}

/// The view of a mesh in Edit Mode: the cached `to_mesh` conversion of its BMesh. The cache
/// is keyed by `bm_gen` (`OpCtx::bmesh_mut` resets it); the non-geometry part (name,
/// materials, active layers, …) comes from the current original mesh, so a non-geometry write
/// made in Edit Mode shows at once.
fn view_of(em: &EditMesh, original: &Arc<Mesh>) -> Arc<Mesh> {
    let (generation, cached) = em
        .view
        .cell
        .get_or_init(|| (em.bm_gen, Arc::new(convert(&em.bm, original))));
    if *generation != em.bm_gen {
        // A BMesh changed without resetting its view (cannot happen through `bmesh_mut`):
        // never serve stale geometry.
        return Arc::new(convert(&em.bm, original));
    }
    if cached.header.generation == original.header.generation
        && cached.header.name == original.header.name
    {
        return Arc::clone(cached);
    }
    let mut m = (**cached).clone();
    graft_non_geometry(&mut m, original);
    Arc::new(m)
}

/// `BM_mesh_bm_to_me` for a read (`calc_object_remap` off, as `update_from_editmode`).
fn convert(bm: &BMesh, original: &Mesh) -> Mesh {
    let mut m = bm.to_mesh(original, &ToMeshParams::default());
    graft_non_geometry(&mut m, original);
    m
}

/// Copies what `BMesh::to_mesh` takes from its template (everything but geometry and
/// attributes) from `original`.
fn graft_non_geometry(m: &mut Mesh, original: &Mesh) {
    m.header = original.header.clone();
    m.materials = original.materials.clone();
    m.vertex_group_names = original.vertex_group_names.clone();
    m.shape_keys = original.shape_keys;
    m.uv_active = original.uv_active.clone();
    m.uv_active_render = original.uv_active_render.clone();
    m.color_active = original.color_active.clone();
    m.color_default = original.color_default.clone();
    m.attributes_active = original.attributes_active.clone();
    m.texspace = original.texspace;
}

/// The view an RNA path rooted at `id` reads (`with_struct`): `Some` for a mesh in Edit Mode.
pub(crate) fn edit_view(host: &dyn HostRead, id: IdAny) -> Option<Arc<Mesh>> {
    let me = id.typed::<MeshId>()?;
    if !host.edit().meshes.contains_key(&me.key()) || host.main().meshes.get(me).is_none() {
        return None;
    }
    Some(host.mesh_view(me))
}

/// Mesh collections whose items are geometry (elements, normals, layers, attributes).
const GEOMETRY: &[&str] = &[
    "vertices",
    "edges",
    "loops",
    "polygons",
    "vertex_normals",
    "polygon_normals",
    "corner_normals",
    "loop_triangles",
    "loop_triangle_polygons",
    "uv_layers",
    "vertex_colors",
    "skin_vertices",
    "attributes",
    "color_attributes",
];

/// Collection API structs below a geometry collection whose own properties are not geometry
/// (the active layer, the active polygon): they are stored on the original mesh.
const API_STRUCTS: &[&str] = &[
    "UVLoopLayers",
    "AttributeGroupMesh",
    "LoopColors",
    "MeshPolygons",
];

/// Layer properties that are not geometry (`uv_layers['UVMap'].active = True`).
fn layer_flag(srna: &str, prop: &str) -> bool {
    matches!(srna, "MeshUVLoopLayer" | "MeshLoopColorLayer")
        && matches!(prop, "active" | "active_render" | "active_clone")
}

/// RNA functions that rewrite mesh geometry: `(struct, function)`.
const GEOMETRY_FUNCS: &[(&str, &str)] = &[
    ("Mesh", "transform"),
    ("Mesh", "flip_normals"),
    ("Mesh", "set_sharp_from_angle"),
    ("Mesh", "split_faces"),
    ("Mesh", "normals_split_custom_set"),
    ("Mesh", "normals_split_custom_set_from_vertices"),
    ("Mesh", "clear_geometry"),
    ("Mesh", "validate"),
    ("Mesh", "validate_material_indices"),
    ("Mesh", "from_pydata"),
    ("Mesh", "shade_flat"),
    ("Mesh", "shade_smooth"),
    ("MeshVertices", "add"),
    ("MeshEdges", "add"),
    ("MeshPolygons", "add"),
    ("MeshLoops", "add"),
    ("UVLoopLayers", "new"),
    ("UVLoopLayers", "remove"),
    ("AttributeGroupMesh", "new"),
    ("AttributeGroupMesh", "remove"),
    ("LoopColors", "new"),
    ("LoopColors", "remove"),
    ("MeshUVLoopLayer", "pin_ensure"),
];

/// The mesh in Edit Mode a pointer is rooted at, if any.
fn edit_mesh_root(host: &dyn HostRead, ptr: &RnaPtr) -> Option<IdRef<MeshId>> {
    let Root::Id(id) = ptr.root else {
        return None;
    };
    if id.code != IdCode::ME {
        return None;
    }
    let me = id.typed::<MeshId>()?;
    host.edit().meshes.contains_key(&me.key()).then_some(me)
}

/// Renaming a layer (`uv_layers['UVMap'].name`, `attributes['x'].name`): Blender renames the
/// edit BMesh's layer and keeps it; frap does not reach the BMesh layers through RNA yet.
fn layer_rename(srna: &str, prop: &str) -> bool {
    prop == "name"
        && (matches!(srna, "MeshUVLoopLayer" | "MeshLoopColorLayer") || srna.ends_with("Attribute"))
}

/// The `RNA_EDITMODE_WRITE` error for `what` on the mesh `me`; `discarded` when Blender would
/// drop the write on leaving Edit Mode (geometry), else frap merely does not support it there.
fn editmode_write(
    host: &dyn HostRead,
    me: IdRef<MeshId>,
    what: &str,
    discarded: bool,
) -> FrapError {
    let name = host.main().meshes.name_of(me).unwrap_or("?").to_owned();
    let why = if discarded {
        "Blender would discard the write when leaving Edit Mode"
    } else {
        "Blender applies it to the edit mesh; frap does not support it in Edit Mode yet"
    };
    FrapError::new(
        PyExc::RuntimeError,
        codes::RNA_EDITMODE_WRITE,
        format!("frap: cannot write {what} of mesh {name:?} while it is in Edit Mode ({why})"),
    )
    .with_detail("mesh", name)
    .with_hint("bpy.ops.object.mode_set(mode='OBJECT')")
}

/// `Object.data = …` on an object in Edit Mode: `rna_Object_data_set` returns early (the data
/// stays), which frap reports as `RNA_EDITMODE_WRITE` rather than ignoring silently.
fn check_object_data(host: &dyn HostRead, ptr: &RnaPtr, prop: &PropDef) -> Result<(), FrapError> {
    if ptr.srna.identifier != "Object" || prop.identifier != "data" || !ptr.path.is_empty() {
        return Ok(());
    }
    let Root::Id(id) = ptr.root else {
        return Ok(());
    };
    let Some(ob) = id.typed::<ObjectId>() else {
        return Ok(());
    };
    let Some(o) = host.main().objects.get(ob) else {
        return Ok(());
    };
    if o.mode != ObjectMode::Edit {
        return Ok(());
    }
    let name = o.header.name.to_string();
    Err(FrapError::new(
        PyExc::RuntimeError,
        codes::RNA_EDITMODE_WRITE,
        format!(
            "frap: cannot assign Object.data of object {name:?} while it is in Edit Mode \
             (Blender ignores the assignment and keeps the data)"
        ),
    )
    .with_detail("object", name)
    .with_hint("bpy.ops.object.mode_set(mode='OBJECT')"))
}

/// Refuses a write to mesh geometry (`vertices`, `edges`, `polygons`, `loops`, `attributes`,
/// `uv_layers[…].data`, …) of a mesh in Edit Mode (`RNA_EDITMODE_WRITE`, §6.3). `ptr` is the
/// struct written to, `prop` the property (or collection) written.
pub(crate) fn check_write(
    host: &dyn HostRead,
    ptr: &RnaPtr,
    prop: &'static PropDef,
) -> Result<(), FrapError> {
    check_object_data(host, ptr, prop)?;
    let Some(me) = edit_mesh_root(host, ptr) else {
        return Ok(());
    };
    if !ptr.path.is_empty() && layer_rename(ptr.srna.identifier, prop.identifier) {
        let what = format!("{}.{}", ptr.srna.identifier, prop.identifier);
        return Err(editmode_write(host, me, &what, false));
    }
    let geometry = match ptr.path.first() {
        // A property (or collection) of the Mesh itself.
        None => GEOMETRY.contains(&prop.identifier),
        Some(Seg::Prop(coll)) if GEOMETRY.contains(coll) => {
            !API_STRUCTS.contains(&ptr.srna.identifier)
                && !layer_flag(ptr.srna.identifier, prop.identifier)
        }
        Some(_) => false,
    };
    if geometry {
        let what = format!("{}.{}", ptr.srna.identifier, prop.identifier);
        return Err(editmode_write(host, me, &what, true));
    }
    Ok(())
}

/// Refuses an RNA function that rewrites mesh geometry (`from_pydata`, `clear_geometry`,
/// `transform`, `vertices.add`, `uv_layers.new`, …) on a mesh in Edit Mode
/// (`RNA_EDITMODE_WRITE`).
pub(crate) fn check_func(
    host: &dyn HostRead,
    this: &RnaPtr,
    func: &'static FuncDef,
) -> Result<(), FrapError> {
    let Some(me) = edit_mesh_root(host, this) else {
        return Ok(());
    };
    let srna = this.srna.identifier;
    if GEOMETRY_FUNCS.contains(&(srna, func.identifier)) {
        let what = format!("{srna}.{}()", func.identifier);
        return Err(editmode_write(host, me, &what, true));
    }
    Ok(())
}

/// Dense indices (the element's index in `mesh.vertices`, `mesh.edges` or `mesh.polygons`
/// after leaving Edit Mode: the number of live pool slots before it) of a few elements, in one
/// pass per pool over the slots up to the highest one asked for.
struct DenseIndex {
    verts: Vec<(u32, usize)>,
    edges: Vec<(u32, usize)>,
    faces: Vec<(u32, usize)>,
}

impl DenseIndex {
    fn new(bm: &BMesh, elems: &[BMElem]) -> DenseIndex {
        fn build<T>(slots: &[Option<T>], mut wanted: Vec<u32>) -> Vec<(u32, usize)> {
            wanted.sort_unstable();
            wanted.dedup();
            let mut out = Vec::with_capacity(wanted.len());
            let (mut live, mut next) = (0usize, 0usize);
            for w in wanted {
                let w = w as usize;
                if w >= slots.len() {
                    break;
                }
                live += slots[next..w].iter().filter(|s| s.is_some()).count();
                next = w;
                if slots[w].is_some() {
                    out.push((w as u32, live));
                }
            }
            out
        }
        let pick = |f: fn(&BMElem) -> Option<u32>| elems.iter().filter_map(f).collect::<Vec<_>>();
        DenseIndex {
            verts: build(
                &bm.verts.slots,
                pick(|e| match e {
                    BMElem::Vert(v) => Some(v.slot() as u32),
                    _ => None,
                }),
            ),
            edges: build(
                &bm.edges.slots,
                pick(|e| match e {
                    BMElem::Edge(x) => Some(x.slot() as u32),
                    _ => None,
                }),
            ),
            faces: build(
                &bm.faces.slots,
                pick(|e| match e {
                    BMElem::Face(f) => Some(f.slot() as u32),
                    _ => None,
                }),
            ),
        }
    }

    /// `{"type": "VERT"|"EDGE"|"FACE", "index": i}`; `None` for a stale element.
    fn json(&self, e: BMElem) -> Option<J> {
        let find = |v: &[(u32, usize)], slot: usize| {
            v.binary_search_by_key(&(slot as u32), |p| p.0)
                .ok()
                .map(|i| v[i].1)
        };
        let (ty, index) = match e {
            BMElem::Vert(v) => ("VERT", find(&self.verts, v.slot())?),
            BMElem::Edge(x) => ("EDGE", find(&self.edges, x.slot())?),
            BMElem::Face(f) => ("FACE", find(&self.faces, f.slot())?),
        };
        Some(json!({"type": ty, "index": index}))
    }
}

/// `bm->selectmode` as Blender's identifiers (`["VERT", "EDGE"]`).
fn select_mode_json(m: SelectMode) -> J {
    [
        (SelectMode::VERT, "VERT"),
        (SelectMode::EDGE, "EDGE"),
        (SelectMode::FACE, "FACE"),
    ]
    .iter()
    .filter(|(b, _)| m.contains(*b))
    .map(|(_, n)| J::from(*n))
    .collect()
}

/// One object's entry of the `edit_mesh` block: the whole selection history with `full`, else
/// its last [`HISTORY_CAP`] entries (`select_history_len` is the total either way).
pub(crate) fn edit_mesh_entry(bm: &BMesh, full: bool) -> J {
    let (sv, se, sf) = bm.selected_counts();
    let tris: usize = bm
        .faces
        .iter()
        .map(|(_, f)| (f.len as usize).saturating_sub(2))
        .sum();
    let all = &bm.select_history;
    let shown = if full {
        &all[..]
    } else {
        &all[all.len().saturating_sub(HISTORY_CAP)..]
    };
    let active = bm.select_history_active();
    let mut wanted = shown.to_vec();
    wanted.extend(active);
    let dense = DenseIndex::new(bm, &wanted);
    let history: Vec<J> = shown.iter().filter_map(|e| dense.json(*e)).collect();
    json!({
        "verts": bm.verts_num(),
        "edges": bm.edges_num(),
        "faces": bm.faces_num(),
        "tris": tris,
        "select_mode": select_mode_json(bm.select_mode),
        "selected": {"verts": sv, "edges": se, "faces": sf},
        "active": active.and_then(|e| dense.json(e)).unwrap_or(J::Null),
        "select_history": history,
        "select_history_len": all.len(),
    })
}

/// The `edit_mesh` block of a response (§13.2), `None` outside mesh Edit Mode: one entry per
/// object in Edit Mode (`context.objects_in_mode_unique_data`, the active object first), keyed
/// by object name. `full` keeps the whole selection history (`--verbosity full`, the golden
/// dumper).
pub(crate) fn edit_mesh_block(host: &dyn HostRead, full: bool) -> Option<J> {
    let ctx = host.ctx();
    if ctx.mode() != ContextMode::EditMesh {
        return None;
    }
    let mut block = serde_json::Map::new();
    for ob in ctx.objects_in_mode_unique_data() {
        let Some(o) = ctx.main.objects.get(ob) else {
            continue;
        };
        let Some(me) = o.data.and_then(|d| d.typed::<MeshId>()) else {
            continue;
        };
        let Some(em) = host.edit().meshes.get(&me.key()) else {
            continue;
        };
        block.insert(o.header.name.to_string(), edit_mesh_entry(&em.bm, full));
    }
    Some(J::Object(block))
}

impl crate::Session {
    /// The `edit_mesh` block of the current state (§13.2), `None` outside mesh Edit Mode;
    /// `full` keeps every selection-history entry (the golden dumper compares them all).
    pub fn edit_mesh_block(&self, full: bool) -> Option<J> {
        edit_mesh_block(self.host(), full)
    }
}
