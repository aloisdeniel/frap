// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the data-API RNA functions of this directory (WP 1.22). Not a concept:
//! the leading underscore keeps it out of the §10.5 file-to-registration check.
//!
//! Behaviour notes verified on Blender 5.2.4 (`-b --factory-startup`):
//! - `bpy.data.<coll>.remove(id)` (`do_unlink=True`, `BKE_id_delete`) clears every reference
//!   to the ID, and deletes the objects that use it as their data (a "never null" usage):
//!   removing mesh `Cube` removes object `Cube`, removing a camera removes its objects.
//!   `do_unlink=False` raises `RuntimeError: Error: Object 'Cube' must have zero users to be
//!   removed, found 1 (try with do_unlink=True parameter)` when real users (fake user not
//!   counted) remain.
//! - `ID.copy()` gives the lowest free `.NNN` name, clears `use_fake_user`, makes a linked ID
//!   local, keeps every reference (an object copy shares its data, keeps its parent and slot
//!   overrides, and is linked to no collection).
//! - `mesh.materials.append/pop/clear` resize the slot arrays of every object using the mesh
//!   (`BKE_objects_materials_test_all` → `BKE_object_material_resize(…, do_id_user=false)`):
//!   shrinking keeps the object's entries beyond the new count (hidden), growing past the
//!   object's old count drops them, a count of 0 frees them. `pop(index)` decrements every
//!   non-zero `material_index` ≥ `index` (`BKE_mesh_material_index_remove`); `clear()` removes
//!   the `material_index` attribute (`BKE_mesh_material_index_clear`).

use frap_mesh::{AttrData, Domain, Mesh};
use frap_nodes::{NodeTree, SocketAddr};
use frap_rna::{
    CollectionId, Detached, EmbeddedSlot, IdAny, IdCode, IdRef, IdType, MeshId, ObjectId, PyExc,
    RnaError, RnaErrorKind, RnaPtr, Root, SceneId, Seg, StructDef, Value,
};
use frap_scene::id::Collection;
use frap_scene::{ChangeLog, FuncArgs, Host, IdMap, Main, ObjectMode};
use smol_str::SmolStr;

/// `RuntimeError` with Blender's report wording (`"Error: …\n"`, as `BKE_report(RPT_ERROR)`
/// reaches Python).
pub(crate) fn report_error(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Error: {msg}\n"),
    )
}

/// An inconsistency between the call target and the data (should not happen).
pub(crate) fn internal(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("frap internal: {msg}"),
    )
}

/// A case frap does not implement yet (`NotImplementedError`, code `RNA_NOT_IMPLEMENTED`).
pub(crate) fn not_implemented(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::NotImplementedError,
        format!("frap: {msg}"),
    )
}

/// A frap-only rejection of input Blender accepts silently and turns into invalid data
/// (`ValueError`).
pub(crate) fn invalid(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::ValueError,
        format!("frap: {msg}"),
    )
}

/// Blender's report errors reach Python with a trailing newline (`"Error: …\n"`); adds it to
/// an `Error: …` message from a lower crate that left it out.
pub(crate) fn as_report(mut e: RnaError) -> RnaError {
    if e.message.starts_with("Error: ") && !e.message.ends_with('\n') {
        e.message.push('\n');
    }
    e
}

// ---------------------------------------------------------------------------------- arguments

/// An ID argument (`None` when the caller passed `None`).
pub(crate) fn arg_id(args: &FuncArgs, name: &str) -> Option<IdAny> {
    match args.get(name) {
        Some(Value::Id(a)) => *a,
        Some(Value::Ptr(p)) if p.path.is_empty() => match p.root {
            Root::Id(a) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

/// A struct-pointer argument (nodes, sockets, links, a depsgraph).
pub(crate) fn arg_ptr<'a>(args: &'a FuncArgs, name: &str) -> Option<&'a RnaPtr> {
    match args.get(name) {
        Some(Value::Ptr(p)) => Some(p),
        _ => None,
    }
}

/// A string or enum argument (`""` when absent).
pub(crate) fn arg_str<'a>(args: &'a FuncArgs, name: &str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or_default()
}

/// A boolean argument.
pub(crate) fn arg_bool(args: &FuncArgs, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

/// An integer argument.
pub(crate) fn arg_int(args: &FuncArgs, name: &str, default: i64) -> i64 {
    match args.get(name) {
        Some(Value::Int(i)) => *i,
        _ => default,
    }
}

// ---------------------------------------------------------------------- detached pointers

/// Refuses a mutating RNA function called on a [`Detached`] pointer (an `evaluated_get()` ID,
/// a `to_mesh()` result, or a struct reached from one): frap reads the original data through
/// it, so the call would change the original, which Blender's copy keeps untouched (WP 1.22
/// deviation, deviations.md). Property writes are refused by the session the same way.
pub(crate) fn refuse_detached(this: &RnaPtr, func: &str) -> Result<(), RnaError> {
    if this.detached.is_none() {
        return Ok(());
    }
    Err(not_implemented(format!(
        "{}.{func}() called on an evaluated or temporary copy (evaluated_get(), to_mesh()); \
         frap has no separate copies yet (M4) and would change the original data: call it \
         on the original datablock (bpy.data.<collection>['Name'], or .original)",
        this.srna.identifier
    )))
}

/// `this` marked as standing for a Blender copy outside `bpy.data`.
pub(crate) fn detached(this: RnaPtr, kind: Detached) -> RnaPtr {
    RnaPtr {
        detached: Some(kind),
        ..this
    }
}

/// Whether the depsgraph of `scene` evaluates `a`: the scene and every ID it reaches through
/// user references (collections, objects, their data, materials, world, actions), plus
/// object parents. Verified on 5.2.4: `evaluated_get()` of an unused material, mesh or an
/// object linked to no scene returns the original (`is_evaluated == False`).
pub(crate) fn in_depsgraph(main: &Main, scene: IdRef<SceneId>, a: IdAny) -> bool {
    let mut edges: Vec<(IdAny, IdAny)> = Vec::new();
    main.for_each_user_ref(&mut |user, used| edges.push((user, used)));
    for (r, ob) in main.objects.iter() {
        if let Some(p) = ob.parent {
            edges.push((r.any(), p.any()));
        }
    }
    let mut seen = vec![scene.any()];
    let mut stack = vec![scene.any()];
    while let Some(x) = stack.pop() {
        if x == a {
            return true;
        }
        for &(u, v) in &edges {
            if u == x && !seen.contains(&v) {
                seen.push(v);
                stack.push(v);
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------------- IDs

/// The ID a pointer is rooted at (the owner for an embedded ID).
pub(crate) fn root_id(this: &RnaPtr) -> Result<IdAny, RnaError> {
    match this.root {
        Root::Id(a) => Ok(a),
        Root::Embedded { owner, .. } => Ok(owner),
        _ => Err(internal(format!(
            "{} is not rooted at a datablock",
            this.srna.identifier
        ))),
    }
}

/// `rna_enum_id_type_items` identifier of an ID code (`'MESH'`, `'NODETREE'`).
pub(crate) fn id_type_identifier(code: IdCode) -> &'static str {
    match code {
        IdCode::OB => "OBJECT",
        IdCode::ME => "MESH",
        IdCode::MA => "MATERIAL",
        IdCode::CA => "CAMERA",
        IdCode::LA => "LIGHT",
        IdCode::WO => "WORLD",
        IdCode::IM => "IMAGE",
        IdCode::AR => "ARMATURE",
        IdCode::AC => "ACTION",
        IdCode::SC => "SCENE",
        IdCode::GR => "COLLECTION",
        IdCode::KE => "KEY",
        IdCode::NT => "NODETREE",
        IdCode::TE => "TEXTURE",
        IdCode::LI => "LIBRARY",
        IdCode::PT => "POINTCLOUD",
        _ => "ID",
    }
}

/// `IDTypeInfo.name` of an ID code ("Object", "Mesh", …), used in reports.
pub(crate) fn id_type_name(code: IdCode) -> &'static str {
    match code {
        IdCode::OB => "Object",
        IdCode::ME => "Mesh",
        IdCode::MA => "Material",
        IdCode::CA => "Camera",
        IdCode::LA => "Light",
        IdCode::WO => "World",
        IdCode::IM => "Image",
        IdCode::AR => "Armature",
        IdCode::AC => "Action",
        IdCode::SC => "Scene",
        IdCode::GR => "Collection",
        IdCode::KE => "Key",
        IdCode::NT => "NodeTree",
        IdCode::TE => "Texture",
        IdCode::LI => "Library",
        IdCode::PT => "PointCloud",
        _ => "ID",
    }
}

/// True for the ID types an object can use as its data (`OB_DATA_SUPPORT_ID`, restricted to
/// the types frap has).
pub(crate) fn is_obdata(code: IdCode) -> bool {
    matches!(
        code,
        IdCode::ME | IdCode::CA | IdCode::LA | IdCode::AR | IdCode::PT
    )
}

/// Runs `$body` with `$m` bound to the mutable `IdMap` of `$code`; `$other` for codes frap
/// has no map for.
macro_rules! on_map_mut {
    ($main:expr, $code:expr, |$m:ident| $body:expr, $other:expr) => {
        match $code {
            IdCode::OB => {
                let $m = &mut $main.objects;
                $body
            }
            IdCode::ME => {
                let $m = &mut $main.meshes;
                $body
            }
            IdCode::MA => {
                let $m = &mut $main.materials;
                $body
            }
            IdCode::CA => {
                let $m = &mut $main.cameras;
                $body
            }
            IdCode::LA => {
                let $m = &mut $main.lights;
                $body
            }
            IdCode::WO => {
                let $m = &mut $main.worlds;
                $body
            }
            IdCode::IM => {
                let $m = &mut $main.images;
                $body
            }
            IdCode::AR => {
                let $m = &mut $main.armatures;
                $body
            }
            IdCode::AC => {
                let $m = &mut $main.actions;
                $body
            }
            IdCode::SC => {
                let $m = &mut $main.scenes;
                $body
            }
            IdCode::GR => {
                let $m = &mut $main.collections;
                $body
            }
            IdCode::KE => {
                let $m = &mut $main.shape_keys;
                $body
            }
            IdCode::NT => {
                let $m = &mut $main.node_groups;
                $body
            }
            IdCode::TE => {
                let $m = &mut $main.textures;
                $body
            }
            IdCode::LI => {
                let $m = &mut $main.libraries;
                $body
            }
            IdCode::PT => {
                let $m = &mut $main.pointclouds;
                $body
            }
            #[allow(unreachable_patterns)]
            _ => $other,
        }
    };
}

/// Clears `use_fake_user` of an ID (logged only when it was set). Returns false when the ID
/// does not exist.
pub(crate) fn clear_fake_user(main: &mut Main, a: IdAny, log: &mut ChangeLog) -> bool {
    fn go<T: IdType>(m: &mut IdMap<T>, a: IdAny, log: &mut ChangeLog) -> bool {
        let Some(r) = a.typed::<T::Kind>() else {
            return false;
        };
        match m.get(r).map(|t| t.header().use_fake_user) {
            None => false,
            Some(false) => true,
            Some(true) => {
                if let Some(t) = m.get_mut(r, log) {
                    t.header_mut().use_fake_user = false;
                }
                true
            }
        }
    }
    on_map_mut!(main, a.code, |m| go(m, a, log), false)
}

/// A local copy of an ID in its own map (`ID.copy()`): same name made unique among local IDs,
/// `use_fake_user` cleared, library cleared.
fn dup<T: IdType>(
    m: &mut IdMap<T>,
    a: IdAny,
    log: &mut ChangeLog,
    adjust: impl FnOnce(&mut T),
) -> Option<IdAny> {
    let r = a.typed::<T::Kind>()?;
    let mut v = m.get(r)?.clone();
    let name = v.header().name.clone();
    {
        let h = v.header_mut();
        h.use_fake_user = false;
        h.library = None;
        h.is_linked_packed = false;
    }
    adjust(&mut v);
    Some(m.insert(&name, v, log).any())
}

/// `ID.copy()` of a datablock of `bpy.data` (see the module notes). Scenes and libraries are
/// not copied by frap yet.
pub(crate) fn copy_id(main: &mut Main, a: IdAny, log: &mut ChangeLog) -> Result<IdAny, RnaError> {
    let missing = || internal("ID.copy(): the datablock was removed");
    match a.code {
        IdCode::SC | IdCode::LI => Err(not_implemented(format!(
            "ID.copy() of a {} is not implemented yet (scenes: M3)",
            id_type_name(a.code)
        ))),
        IdCode::OB => {
            dup(&mut main.objects, a, log, |o| o.mode = ObjectMode::Object).ok_or_else(missing)
        }
        IdCode::ME => {
            let new = dup(&mut main.meshes, a, log, |_| ()).ok_or_else(missing)?;
            // A mesh copy owns a copy of its shape keys (`BKE_id_copy` of the `Key`).
            let r: IdRef<MeshId> = new.typed().ok_or_else(missing)?;
            if let Some(k) = main.meshes.get(r).and_then(|m| m.shape_keys)
                && let Some(k2) = dup(&mut main.shape_keys, k.any(), log, |_| ())
                && let Some(m) = main.meshes.get_mut(r, log)
            {
                m.shape_keys = k2.typed();
            }
            Ok(new)
        }
        code => on_map_mut!(
            main,
            code,
            |m| dup(m, a, log, |_| ()).ok_or_else(missing),
            Err(missing())
        ),
    }
}

/// Deletes an ID the way `bpy.data.<coll>.remove(id)` does (`BKE_id_delete`): objects using
/// it as their data go with it (never-null usage), a mesh's shape keys go with the mesh, and
/// every other reference is cleared (`Main::remove`). Returns the removed mesh keys (their
/// edit-mode state must be dropped).
pub(crate) fn delete_id(main: &mut Main, a: IdAny, log: &mut ChangeLog) -> Vec<IdRef<MeshId>> {
    let mut meshes = Vec::new();
    if is_obdata(a.code) {
        let users: Vec<IdAny> = main
            .objects
            .iter()
            .filter(|(_, o)| o.data == Some(a))
            .map(|(r, _)| r.any())
            .collect();
        for o in users {
            main.remove(o, log);
        }
    }
    if let Some(me) = a.typed::<MeshId>() {
        let key = main.meshes.get(me).and_then(|m| m.shape_keys);
        main.remove(a, log);
        if let Some(k) = key {
            main.remove(k.any(), log);
        }
        meshes.push(me);
    } else {
        main.remove(a, log);
    }
    meshes
}

/// `bpy.data.<coll>.remove(<param>, do_unlink=True, do_id_user=True, do_ui_user=True)`.
pub(crate) fn remove(h: &mut dyn Host, args: &FuncArgs, param: &str) -> Result<Value, RnaError> {
    let a = arg_id(args, param).ok_or_else(|| internal("remove(): no ID argument"))?;
    let do_unlink = arg_bool(args, "do_unlink", true);
    let (main, log) = h.main_mut();
    let header = main
        .header(a)
        .ok_or_else(|| internal("remove(): the datablock was already removed"))?;
    if !do_unlink {
        // `ID_REAL_USERS`: the fake user does not count.
        let real = main
            .users(a)
            .saturating_sub(u32::from(header.use_fake_user));
        if real > 0 {
            return Err(report_error(format!(
                "{} '{}' must have zero users to be removed, found {real} (try with \
                 do_unlink=True parameter)",
                id_type_name(a.code),
                header.name
            )));
        }
    }
    let removed = delete_id(main, a, log);
    let edit = h.edit_mut();
    for me in removed {
        edit.meshes.remove(&me.key());
    }
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------- collections

/// The collection a `CollectionObjects`/`CollectionChildren` call is made on: a
/// `bpy.data.collections` ID, or a scene's embedded master collection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CollTarget {
    /// `bpy.data.collections[…]`
    Id(IdRef<CollectionId>),
    /// `scene.collection`
    Master(IdRef<SceneId>),
}

impl CollTarget {
    /// The collection of `this` (`….objects` / `….children` of a collection).
    pub(crate) fn of(this: &RnaPtr) -> Result<CollTarget, RnaError> {
        match this.root {
            Root::Id(a) if a.code == IdCode::GR => a
                .typed()
                .map(CollTarget::Id)
                .ok_or_else(|| internal("not a collection")),
            Root::Embedded {
                owner,
                slot: EmbeddedSlot::MasterCollection,
            } => owner
                .typed()
                .map(CollTarget::Master)
                .ok_or_else(|| internal("not a scene")),
            _ => Err(internal(format!(
                "{} is not rooted at a collection",
                this.srna.identifier
            ))),
        }
    }
    /// The collection.
    pub(crate) fn get(self, main: &Main) -> Result<&Collection, RnaError> {
        match self {
            CollTarget::Id(r) => main.collections.get(r),
            CollTarget::Master(s) => main.scenes.get(s).map(|s| &s.collection),
        }
        .ok_or_else(|| internal("the collection was removed"))
    }
    /// The collection, mutably (through its owner's `get_mut`).
    pub(crate) fn get_mut<'m>(
        self,
        main: &'m mut Main,
        log: &mut ChangeLog,
    ) -> Result<&'m mut Collection, RnaError> {
        match self {
            CollTarget::Id(r) => main.collections.get_mut(r, log),
            CollTarget::Master(s) => main.scenes.get_mut(s, log).map(|s| &mut s.collection),
        }
        .ok_or_else(|| internal("the collection was removed"))
    }
}

/// The name of an object, for reports.
pub(crate) fn object_name(main: &Main, ob: IdRef<ObjectId>) -> SmolStr {
    main.objects
        .name_of(ob)
        .map(SmolStr::new)
        .unwrap_or_default()
}

/// True when `ancestor` is `c` or reachable from `c` through `children` (cycle test of
/// `BKE_collection_child_add` → `collection_find_child_recursive`).
pub(crate) fn reaches(main: &Main, c: IdRef<CollectionId>, ancestor: IdRef<CollectionId>) -> bool {
    let mut stack = vec![c];
    let mut seen = Vec::new();
    while let Some(x) = stack.pop() {
        if x == ancestor {
            return true;
        }
        if seen.contains(&x) {
            continue;
        }
        seen.push(x);
        if let Some(col) = main.collections.get(x) {
            stack.extend(col.children.iter().copied());
        }
    }
    false
}

// ---------------------------------------------------------------------------------- materials

/// Resizes the slot overrides of every object using `data` after its material count changed
/// from `old_len` to `new_len` (see the module notes), and clamps each object's
/// `active_material_index` to the new slots like `BKE_object_material_resize` clamps
/// `ob->actcol` (`actcol > totcol → totcol`; the 0-based index reads `max(totcol - 1, 0)`).
/// Verified on 5.2.4: Cube with 3 slots and `active_material_index = 2`, `pop()` → 1,
/// `clear()` → 0.
pub(crate) fn resize_object_slots(
    main: &mut Main,
    data: IdAny,
    old_len: usize,
    new_len: usize,
    log: &mut ChangeLog,
) {
    let max_index = new_len.saturating_sub(1) as i32;
    let obs: Vec<IdRef<ObjectId>> = main
        .objects
        .iter()
        .filter(|(_, o)| o.data == Some(data))
        .filter(|(_, o)| {
            let n = o.material_slot_overrides.len();
            (new_len == 0 && n > 0)
                || (new_len > old_len && n > old_len)
                || o.active_material_index > max_index
        })
        .map(|(r, _)| r)
        .collect();
    for r in obs {
        if let Some(o) = main.objects.get_mut(r, log) {
            if new_len == 0 {
                o.material_slot_overrides.clear();
            } else if new_len > old_len {
                o.material_slot_overrides.truncate(old_len);
            }
            if o.active_material_index > max_index {
                o.active_material_index = max_index;
            }
        }
    }
}

/// `BKE_mesh_material_index_remove`: every non-zero face `material_index` ≥ `index` is
/// decremented.
pub(crate) fn material_index_remove(mesh: &mut Mesh, index: usize) {
    let needs = matches!(
        mesh.attributes().get("material_index"),
        Some(a) if a.domain == Domain::Face
            && matches!(&a.data, AttrData::Int(v) if v.iter().any(|&m| m > 0 && m as usize >= index))
    );
    if !needs {
        return;
    }
    if let Some(a) = mesh.attributes_mut().get_mut("material_index")
        && let AttrData::Int(v) = &mut a.data
    {
        for m in std::sync::Arc::make_mut(v) {
            if *m > 0 && *m as usize >= index {
                *m -= 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------------- node trees

/// The node tree a `Nodes`/`NodeLinks` call is made on (`material.node_tree`,
/// `world.node_tree`, or a `bpy.data.node_groups` ID).
pub(crate) fn tree(main: &Main, root: Root) -> Result<&NodeTree, RnaError> {
    match root {
        Root::Id(a) if a.code == IdCode::NT => a.typed().and_then(|r| main.node_groups.get(r)),
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => owner
                .typed()
                .and_then(|r| main.materials.get(r))
                .map(|m| &m.node_tree),
            IdCode::WO => owner
                .typed()
                .and_then(|r| main.worlds.get(r))
                .map(|w| &w.node_tree),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| internal("not a node tree"))
}

/// The node tree of `root`, mutably (through the owning ID's `get_mut`).
pub(crate) fn tree_mut<'m>(
    main: &'m mut Main,
    root: Root,
    log: &mut ChangeLog,
) -> Result<&'m mut NodeTree, RnaError> {
    match root {
        Root::Id(a) if a.code == IdCode::NT => {
            a.typed().and_then(|r| main.node_groups.get_mut(r, log))
        }
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => owner
                .typed()
                .and_then(|r| main.materials.get_mut(r, log))
                .map(|m| &mut m.node_tree),
            IdCode::WO => owner
                .typed()
                .and_then(|r| main.worlds.get_mut(r, log))
                .map(|w| &mut w.node_tree),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| internal("not a node tree"))
}

/// The RNA struct of the tree at `root` (for result pointers).
pub(crate) fn tree_srna(main: &Main, root: Root) -> &'static StructDef {
    match root {
        Root::Id(a) => main.id(a).map(|r| r.srna()),
        Root::Embedded { owner, slot } => main.embedded(owner, slot).map(|r| r.srna()),
        _ => None,
    }
    .unwrap_or(&frap_spec_rna::NODE_TREE)
}

/// Index of the node named by an item segment (`nodes["Name"]` or `nodes[i]`).
fn node_at(t: &NodeTree, seg: &Seg) -> Option<usize> {
    match seg {
        Seg::Key(k) => t.nodes.iter().position(|n| n.name == k.as_str()),
        Seg::Index(i) => usize::try_from(*i).ok().filter(|i| *i < t.nodes.len()),
        Seg::Prop(_) => None,
    }
}

/// Index in `t.nodes` of the node `p` points at, when `p` is a node of the tree at `root`.
pub(crate) fn node_of(t: &NodeTree, root: Root, p: &RnaPtr) -> Option<usize> {
    if p.root != root {
        return None;
    }
    match p.path.as_slice() {
        [Seg::Prop("nodes"), seg] => node_at(t, seg),
        _ => None,
    }
}

/// The socket `p` points at, when it is a socket of a node of the tree at `root`.
pub(crate) fn socket_of(t: &NodeTree, root: Root, p: &RnaPtr) -> Option<SocketAddr> {
    if p.root != root {
        return None;
    }
    let [Seg::Prop("nodes"), node, Seg::Prop(dir), seg] = p.path.as_slice() else {
        return None;
    };
    let n = &t.nodes[node_at(t, node)?];
    let is_output = match *dir {
        "inputs" => false,
        "outputs" => true,
        _ => return None,
    };
    let sockets = if is_output { &n.outputs } else { &n.inputs };
    let index = match seg {
        Seg::Index(i) => usize::try_from(*i).ok().filter(|i| *i < sockets.len())?,
        Seg::Key(k) => frap_nodes::find_socket(sockets, k)?,
        Seg::Prop(_) => return None,
    };
    Some(SocketAddr {
        node: n.uid,
        is_output,
        index,
    })
}

/// Index in `t.links` of the link `p` points at, when it is a link of the tree at `root`.
pub(crate) fn link_of(t: &NodeTree, root: Root, p: &RnaPtr) -> Option<usize> {
    if p.root != root {
        return None;
    }
    match p.path.as_slice() {
        [Seg::Prop("links"), Seg::Index(i)] => {
            usize::try_from(*i).ok().filter(|i| *i < t.links.len())
        }
        _ => None,
    }
}

/// Canonical pointer of node `i` of the tree at `root`: `….nodes["Name"]`.
pub(crate) fn node_ptr(main: &Main, root: Root, t: &NodeTree, i: usize) -> RnaPtr {
    let srna = frap_nodes::rna::node_srna(&t.nodes[i]);
    RnaPtr::new(root, tree_srna(main, root))
        .child(Seg::Prop("nodes"), srna)
        .child(Seg::Key(t.nodes[i].name.clone()), srna)
}

/// Canonical pointer of link `i` of the tree at `root`: `….links[i]`.
pub(crate) fn link_ptr(main: &Main, root: Root, i: usize) -> RnaPtr {
    let srna = &frap_spec_rna::NODE_LINK;
    RnaPtr::new(root, tree_srna(main, root))
        .child(Seg::Prop("links"), srna)
        .child(Seg::Index(i as i64), srna)
}

/// The mesh a `Mesh`/`IDMaterials` call is made on.
pub(crate) fn mesh_of(this: &RnaPtr) -> Result<IdRef<MeshId>, RnaError> {
    root_id(this)?
        .typed()
        .ok_or_else(|| internal("the pointer is not rooted at a Mesh"))
}
