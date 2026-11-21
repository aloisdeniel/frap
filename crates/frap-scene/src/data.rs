// SPDX-License-Identifier: GPL-3.0-or-later
//! `Main` (= `bpy.data`), `IdMap<T>` and `ChangeLog` (ARCHITECTURE §5.2–§5.4, CONTRACT frozen
//! at M1 W0, amended by RFC 0001 F-MAIN-1/2). The file is `data.rs`, not `main.rs`, because
//! Cargo would build `src/main.rs` as a binary target (F-ED-2).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use frap_anim::Action;
use frap_mesh::Mesh;
use frap_nodes::NodeTree;
use frap_rna::id::next_generation;
use frap_rna::naming::{split_suffix, truncate_utf8, unique_name};
use frap_rna::{
    EmbeddedSlot, IdAny, IdCode, IdHeader, IdKey, IdKind, IdNames, IdRef, IdType, LibraryId, PyExc,
    Rna, RnaError, RnaErrorKind, RnaMut, StructDef,
};
use indexmap::{IndexMap, IndexSet};
use smol_str::SmolStr;

use crate::id::{
    Armature, Camera, Collection, Image, Key, Library, Light, Material, Object, PointCloud, Scene,
    Texture, World,
};

/// Per-statement change set; feeds `Response.changes`.
#[derive(Clone, Debug, Default)]
pub struct ChangeLog {
    /// IDs created.
    pub added: Vec<IdAny>,
    /// IDs removed, with their last name (they are gone from `Main` when the response is built).
    pub removed: Vec<(IdAny, SmolStr)>,
    /// IDs mutated (insertion order).
    pub modified: IndexSet<IdAny>,
    /// IDs renamed, with their old name.
    pub renamed: Vec<(IdAny, SmolStr)>,
    /// Selection / active / mode changed.
    pub context_changed: bool,
}

/// `ID.rename(name, mode=…)` modes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenameMode {
    /// `'NEVER'`: never rename another ID to make room.
    Never,
    /// `'ALWAYS'`: take the name, renaming the other ID.
    Always,
    /// `'SAME_ROOT'`: take the name if the other shares the root name.
    SameRoot,
}

/// `ID.rename` return values (Blender's status enum).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenameResult {
    /// `'UNCHANGED'`
    Unchanged,
    /// `'UNCHANGED_COLLISION'`
    UnchangedCollision,
    /// `'RENAMED_NO_COLLISION'`
    RenamedNoCollision,
    /// `'RENAMED_COLLISION_ADJUSTED'`
    RenamedCollisionAdjusted,
    /// `'RENAMED_COLLISION_FORCED'`
    RenamedCollisionForced,
}

/// Blender's ListBase order key: `BLI_strcasecmp` (ASCII case-insensitive, non-natural).
fn strcasecmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

/// One `bpy.data` collection: `Arc`'d datablocks in Blender order (§5.4).
#[derive(Debug)]
pub struct IdMap<T: IdType> {
    items: IndexMap<IdKey, Arc<T>>,
    by_name: BTreeMap<(SmolStr, Option<IdKey>), IdKey>,
}

impl<T: IdType> Clone for IdMap<T> {
    /// O(#IDs): data is shared until written (copy-on-write).
    fn clone(&self) -> Self {
        IdMap {
            items: self.items.clone(),
            by_name: self.by_name.clone(),
        }
    }
}

impl<T: IdType> Default for IdMap<T> {
    fn default() -> Self {
        IdMap {
            items: IndexMap::new(),
            by_name: BTreeMap::new(),
        }
    }
}

fn lib_key(h: &IdHeader) -> Option<IdKey> {
    h.library.map(|l| l.key())
}

/// Sort key of an ID in its ListBase (§5.4): local IDs first, then linked IDs grouped by
/// library (in library creation order), each group sorted by `BLI_strcasecmp`, equal keys in
/// insertion order (verified: a linked `Smooth by Angle` sorts after the local `Zzz`).
fn list_cmp(a: (&str, Option<IdKey>), b: (&str, Option<IdKey>)) -> Ordering {
    match (a.1, b.1) {
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (la, lb) => la.cmp(&lb).then_with(|| strcasecmp(a.0, b.0)),
    }
}

impl<T: IdType> IdMap<T> {
    /// Read access.
    pub fn get(&self, r: IdRef<T::Kind>) -> Option<&T> {
        self.items.get(&r.key()).map(|a| a.as_ref())
    }
    /// Shared handle (e.g. `HostRead::mesh_view` in object mode; F-MAIN-2).
    pub fn get_arc(&self, r: IdRef<T::Kind>) -> Option<Arc<T>> {
        self.items.get(&r.key()).cloned()
    }
    /// The only mutable access path: `Arc::make_mut` (clones only if a snapshot shares it),
    /// `header.generation` bump, logged as modified. Embedded IDs (`Material.node_tree`,
    /// `Scene.collection`) are reached through their owner, so the owner is bumped (§5.1).
    pub fn get_mut(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<&mut T> {
        let arc = self.items.get_mut(&r.key())?;
        let t = Arc::make_mut(arc);
        t.header_mut().generation = next_generation();
        log.modified.insert(r.any());
        Some(t)
    }
    /// True when `r` is in this map.
    pub fn contains(&self, r: IdRef<T::Kind>) -> bool {
        self.items.contains_key(&r.key())
    }
    /// `coll['Name']`, `coll.get()`: the local ID first, then linked IDs in library order
    /// (Blender scans its ListBase, where local IDs sort first; verified with a local and a
    /// linked `Smooth by Angle`).
    pub fn find(&self, name: &str) -> Option<IdRef<T::Kind>> {
        if let Some(k) = self.by_name.get(&(SmolStr::new(name), None)) {
            return Some(IdRef::from_key(*k));
        }
        self.items
            .iter()
            .find(|(_, v)| v.header().name == name)
            .map(|(k, _)| IdRef::from_key(*k))
    }
    /// `coll['Name', lib]`: exactly the ID named `name` in `library` (`None` = local only).
    ///
    /// Pinned on Blender 5.2.4 (`pyrna_prop_collection_subscript_str_lib_pair`): `None` finds
    /// local IDs only (`objects['Cube', None]` works; a linked group is not found by `None`);
    /// a string names a library by **filepath**, and the first library with that filepath is
    /// used ([`Main::library_by_filepath`]). See [`Main::find_key_lib`] for the errors.
    pub fn find_in(&self, name: &str, library: Option<IdRef<LibraryId>>) -> Option<IdRef<T::Kind>> {
        self.by_name
            .get(&(SmolStr::new(name), library.map(|l| l.key())))
            .map(|k| IdRef::from_key(*k))
    }
    /// `coll.find(name)` position.
    pub fn index_of(&self, r: IdRef<T::Kind>) -> Option<usize> {
        self.items.get_index_of(&r.key())
    }
    /// `coll[i]`, negative allowed.
    pub fn at(&self, i: isize) -> Option<IdRef<T::Kind>> {
        let n = self.items.len() as isize;
        let i = if i < 0 { i + n } else { i };
        if !(0..n).contains(&i) {
            return None;
        }
        self.items
            .get_index(i as usize)
            .map(|(k, _)| IdRef::from_key(*k))
    }
    /// Number of IDs.
    pub fn len(&self) -> usize {
        self.items.len()
    }
    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Sorted ListBase position for a new entry `(name, lib)`: after every ID that sorts
    /// before or equal to it (stable).
    fn sorted_pos(&self, name: &str, lib: Option<IdKey>, skip: Option<IdKey>) -> usize {
        self.items
            .iter()
            .filter(|(k, _)| Some(**k) != skip)
            .position(|(_, v)| {
                list_cmp((&v.header().name, lib_key(v.header())), (name, lib)) == Ordering::Greater
            })
            .unwrap_or(self.items.len() - usize::from(skip.is_some()))
    }
    fn unique(&self, requested: &str, lib: Option<IdKey>) -> SmolStr {
        unique_name(
            &|n| self.by_name.contains_key(&(SmolStr::new(n), lib)),
            requested,
            255,
            T::Kind::DEFAULT_NAME,
        )
    }
    /// Inserts an ID (local, or linked when its header names a library) with a unique name
    /// among the IDs of the same library (lowest free `.NNN`), at its sorted position.
    pub fn insert(&mut self, requested: &str, mut value: T, log: &mut ChangeLog) -> IdRef<T::Kind> {
        let lib = lib_key(value.header());
        let name = self.unique(requested, lib);
        let key = IdKey::next();
        {
            let h = value.header_mut();
            h.name = name.clone();
            h.generation = next_generation();
            h.session_uid = key.get() as u32;
        }
        let pos = if self.items.is_empty() {
            0
        } else {
            self.sorted_pos(&name, lib, None)
        };
        self.items.shift_insert(pos, key, Arc::new(value));
        self.by_name.insert((name, lib), key);
        let r = IdRef::from_key(key);
        log.added.push(r.any());
        r
    }
    /// Moves `key` to its sorted position after a name change.
    fn resort(&mut self, key: IdKey) {
        let Some(from) = self.items.get_index_of(&key) else {
            return;
        };
        let (name, lib) = {
            let h = self.items[from].header();
            (h.name.clone(), lib_key(h))
        };
        let to = self.sorted_pos(&name, lib, Some(key));
        self.items.move_index(from, to);
    }
    /// Sets the name of `key` (already unique), re-indexes and re-sorts, logging the rename.
    fn set_name(&mut self, key: IdKey, name: SmolStr, log: &mut ChangeLog) {
        let Some(arc) = self.items.get_mut(&key) else {
            return;
        };
        let t = Arc::make_mut(arc);
        let lib = lib_key(t.header());
        let old = std::mem::replace(&mut t.header_mut().name, name.clone());
        t.header_mut().generation = next_generation();
        if self.by_name.get(&(old.clone(), lib)) == Some(&key) {
            self.by_name.remove(&(old.clone(), lib));
        }
        self.by_name.insert((name, lib), key);
        let r: IdRef<T::Kind> = IdRef::from_key(key);
        log.modified.insert(r.any());
        log.renamed.push((r.any(), old));
        self.resort(key);
    }
    /// `ID.rename(name, mode=…)` and `ID.name = …` (mode `Never`), with Blender's collision
    /// rules (`BKE_id_rename`, verified on 5.2.4): the requested name is truncated to 255 bytes
    /// on a UTF-8 boundary (empty → the type's default name); a free name is taken as is;
    /// on a collision `Never` takes the lowest free `.NNN` (`UnchangedCollision` when that is
    /// the current name), `Always` takes the name and renames the other ID to its own lowest
    /// free name, `SameRoot` does the same only when the current and requested names share
    /// their root (`Foo.002` → `Foo`), otherwise behaves as `Never`. The renamed ID moves to
    /// its sorted ListBase position (§5.4).
    pub fn rename(
        &mut self,
        r: IdRef<T::Kind>,
        requested: &str,
        mode: RenameMode,
        log: &mut ChangeLog,
    ) -> RenameResult {
        let key = r.key();
        let Some(cur) = self.items.get(&key) else {
            return RenameResult::Unchanged;
        };
        let lib = lib_key(cur.header());
        let old = cur.header().name.clone();
        let requested = if requested.is_empty() {
            T::Kind::DEFAULT_NAME
        } else {
            requested
        };
        let wanted = SmolStr::new(truncate_utf8(requested, 255));
        if wanted == old {
            return RenameResult::Unchanged;
        }
        // Take the ID out of the namespace while deciding (its old name is free again).
        self.by_name.remove(&(old.clone(), lib));
        let conflict = self.by_name.get(&(wanted.clone(), lib)).copied();
        let Some(other) = conflict else {
            self.by_name.insert((old, lib), key);
            self.set_name(key, wanted, log);
            return RenameResult::RenamedNoCollision;
        };
        let force = match mode {
            RenameMode::Never => false,
            RenameMode::Always => true,
            RenameMode::SameRoot => split_suffix(&old).0 == split_suffix(&wanted).0,
        };
        if force {
            // The renamed ID takes the name; the other gets the lowest free one (the renamed
            // ID's old name is free again).
            self.by_name.insert((wanted.clone(), lib), key);
            let other_name = self.unique(&wanted, lib);
            self.by_name.insert((other_name.clone(), lib), other);
            self.set_name(other, other_name, log);
            self.set_name(key, wanted, log);
            return RenameResult::RenamedCollisionForced;
        }
        let adjusted = self.unique(&wanted, lib);
        self.by_name.insert((old.clone(), lib), key);
        if adjusted == old {
            return RenameResult::UnchangedCollision;
        }
        self.set_name(key, adjusted, log);
        RenameResult::RenamedCollisionAdjusted
    }
    /// Removes an ID (references to it are not touched: see [`Main::remove`]).
    pub fn remove(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<Arc<T>> {
        let v = self.items.shift_remove(&r.key())?;
        let k = (v.header().name.clone(), lib_key(v.header()));
        if self.by_name.get(&k) == Some(&r.key()) {
            self.by_name.remove(&k);
        }
        log.removed.push((r.any(), v.header().name.clone()));
        Some(v)
    }
    /// Iterates in Blender order.
    pub fn iter(&self) -> impl Iterator<Item = (IdRef<T::Kind>, &T)> {
        self.items
            .iter()
            .map(|(k, v)| (IdRef::from_key(*k), v.as_ref()))
    }
    /// Name of an ID.
    pub fn name_of(&self, r: IdRef<T::Kind>) -> Option<&str> {
        self.get(r).map(|t| t.header().name.as_str())
    }
    /// Mutable access to every ID that `pred` selects (each one bumped and logged).
    pub(crate) fn update_where(
        &mut self,
        mut pred: impl FnMut(&T) -> bool,
        mut f: impl FnMut(&mut T),
        log: &mut ChangeLog,
    ) {
        let keys: Vec<IdKey> = self
            .items
            .iter()
            .filter(|(_, v)| pred(v))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            if let Some(t) = self.get_mut(IdRef::from_key(k), log) {
                f(t);
            }
        }
    }
}

/// `bpy.data`.
#[derive(Clone, Debug, Default)]
pub struct Main {
    /// `bpy.data.objects`
    pub objects: IdMap<Object>,
    /// `bpy.data.meshes`
    pub meshes: IdMap<Mesh>,
    /// `bpy.data.materials`
    pub materials: IdMap<Material>,
    /// `bpy.data.cameras`
    pub cameras: IdMap<Camera>,
    /// `bpy.data.lights`
    pub lights: IdMap<Light>,
    /// `bpy.data.worlds`
    pub worlds: IdMap<World>,
    /// `bpy.data.images`
    pub images: IdMap<Image>,
    /// `bpy.data.armatures`
    pub armatures: IdMap<Armature>,
    /// `bpy.data.actions`
    pub actions: IdMap<Action>,
    /// `bpy.data.scenes`
    pub scenes: IdMap<Scene>,
    /// `bpy.data.collections`
    pub collections: IdMap<Collection>,
    /// `bpy.data.shape_keys`
    pub shape_keys: IdMap<Key>,
    /// `bpy.data.node_groups`
    pub node_groups: IdMap<NodeTree>,
    /// `bpy.data.textures`
    pub textures: IdMap<Texture>,
    /// `bpy.data.libraries`
    pub libraries: IdMap<Library>,
    /// `bpy.data.pointclouds`
    pub pointclouds: IdMap<PointCloud>,
    /// `bpy.data.filepath` (`None` = unsaved). A plain string: `camino` is not a workspace
    /// dependency (Decision log, M1 W0).
    pub filepath: Option<String>,
    /// `bpy.data.is_dirty`
    pub is_dirty: bool,
    /// Runtime: the `BASE_SELECTED` bit of each object's `ob->base_flag`, which seeds the
    /// selection of a new base (`object_base_new`, e.g. an object linked into another scene).
    /// Kept by `frap_scene::id::layers` (WP 3.10); not saved, rebuilt from the bases.
    pub base_selected: crate::id::layers::BaseSelectedMemory,
    /// Runtime: the layer-collection trees Blender's lazy resync still starts from (the trees
    /// at the last point Blender would have resynced; `frap_scene::id::layers::LazyResync`);
    /// not saved.
    pub lazy_resync: crate::id::layers::LazyResync,
    /// Runtime: the depsgraphs (scene, view layer name) the window has shown before its
    /// current one ([`crate::window::ShownDepsgraphs`]); not saved.
    pub shown_depsgraphs: crate::window::ShownDepsgraphs,
    /// Runtime: the invisible depsgraph nodes with an animation update pending
    /// ([`crate::window::InvisiblePending`]); not saved.
    pub invisible_pending: crate::window::InvisiblePending,
}

/// Runs `$body` with `$m` bound to the `IdMap` of `$code` (every `IdCode`).
macro_rules! with_map {
    ($main:expr, $code:expr, |$m:ident| $body:expr) => {
        match $code {
            IdCode::OB => {
                let $m = &$main.objects;
                $body
            }
            IdCode::ME => {
                let $m = &$main.meshes;
                $body
            }
            IdCode::MA => {
                let $m = &$main.materials;
                $body
            }
            IdCode::CA => {
                let $m = &$main.cameras;
                $body
            }
            IdCode::LA => {
                let $m = &$main.lights;
                $body
            }
            IdCode::WO => {
                let $m = &$main.worlds;
                $body
            }
            IdCode::IM => {
                let $m = &$main.images;
                $body
            }
            IdCode::AR => {
                let $m = &$main.armatures;
                $body
            }
            IdCode::AC => {
                let $m = &$main.actions;
                $body
            }
            IdCode::SC => {
                let $m = &$main.scenes;
                $body
            }
            IdCode::GR => {
                let $m = &$main.collections;
                $body
            }
            IdCode::KE => {
                let $m = &$main.shape_keys;
                $body
            }
            IdCode::NT => {
                let $m = &$main.node_groups;
                $body
            }
            IdCode::TE => {
                let $m = &$main.textures;
                $body
            }
            IdCode::LI => {
                let $m = &$main.libraries;
                $body
            }
            IdCode::PT => {
                let $m = &$main.pointclouds;
                $body
            }
            #[allow(unreachable_patterns)]
            _ => unreachable!("IdCode without an IdMap"),
        }
    };
}

/// Mutable variant of [`with_map!`].
macro_rules! with_map_mut {
    ($main:expr, $code:expr, |$m:ident| $body:expr) => {
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
            _ => unreachable!("IdCode without an IdMap"),
        }
    };
}

fn id_ref<T: IdType>(m: &IdMap<T>, a: IdAny) -> Option<&T> {
    m.get(a.typed::<T::Kind>()?)
}

/// Names for `repr` (D-15). A linked ID gets Blender's plain `['Name']` key whenever that key
/// resolves to it (no local ID of the same name shadows it, the common case and Blender's own
/// repr); only a shadowed linked ID gets the `['Name', '<library filepath>']` tuple key. The
/// tuple key follows Blender's lookup (the first library with that filepath, see
/// [`IdMap::find_in`]), so it does not resolve back for an ID of a second library with the same
/// filepath (the packed archive library `object.shade_auto_smooth` creates) when it is also
/// shadowed by a local ID.
impl IdNames for Main {
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        let h = self.header(id)?;
        let lib = h.library.and_then(|l| {
            let plain = with_map!(self, id.code, |m| m.find(&h.name).map(|r| r.any()));
            if plain == Some(id) {
                return None;
            }
            self.libraries.get(l).map(|l| SmolStr::new(&l.filepath))
        });
        Some((h.name.clone(), lib))
    }
}

impl Main {
    /// `bpy.data` collection attribute names, sorted (as `dir(bpy.data)` lists them).
    pub const COLLECTIONS: &'static [&'static str] = &[
        "actions",
        "armatures",
        "cameras",
        "collections",
        "images",
        "libraries",
        "lights",
        "materials",
        "meshes",
        "node_groups",
        "objects",
        "pointclouds",
        "scenes",
        "shape_keys",
        "textures",
        "worlds",
    ];

    /// The `IdCode` of a `bpy.data` collection attribute.
    pub fn code_of(collection: &str) -> Option<IdCode> {
        IdCode::ALL
            .into_iter()
            .find(|c| c.collection() == collection)
    }

    /// The ID as an RNA struct (F-MAIN-1).
    pub fn id(&self, a: IdAny) -> Option<&dyn Rna> {
        with_map!(self, a.code, |m| id_ref(m, a).map(|t| t as &dyn Rna))
    }
    /// The ID as a mutable RNA struct, through `IdMap::get_mut` (COW, generation bump, logged).
    pub fn id_mut(&mut self, a: IdAny, log: &mut ChangeLog) -> Option<&mut dyn RnaMut> {
        fn g<'a, T: IdType + RnaMut>(
            m: &'a mut IdMap<T>,
            a: IdAny,
            log: &mut ChangeLog,
        ) -> Option<&'a mut dyn RnaMut> {
            m.get_mut(a.typed::<T::Kind>()?, log)
                .map(|t| t as &mut dyn RnaMut)
        }
        with_map_mut!(self, a.code, |m| g(m, a, log))
    }
    /// The ID's header.
    pub fn header(&self, a: IdAny) -> Option<&IdHeader> {
        with_map!(self, a.code, |m| id_ref(m, a).map(|t| t.header()))
    }
    /// The ID's header, mutably, through `IdMap::get_mut` (COW, generation bump, logged):
    /// ID properties (`ID["key"] = value`).
    pub fn header_mut(&mut self, a: IdAny, log: &mut ChangeLog) -> Option<&mut IdHeader> {
        fn g<'a, T: IdType>(
            m: &'a mut IdMap<T>,
            a: IdAny,
            log: &mut ChangeLog,
        ) -> Option<&'a mut IdHeader> {
            m.get_mut(a.typed::<T::Kind>()?, log)
                .map(|t| t.header_mut())
        }
        with_map_mut!(self, a.code, |m| g(m, a, log))
    }
    /// `bpy.data.<attr>`: the IDs in Blender order and the item struct (F-MAIN-1).
    pub fn collection(&self, attr: &str) -> Option<(Vec<IdAny>, &'static StructDef)> {
        fn ids<T: IdType>(m: &IdMap<T>) -> Vec<IdAny> {
            m.iter().map(|(r, _)| r.any()).collect()
        }
        use frap_spec_rna as s;
        let code = Main::code_of(attr)?;
        let srna: &'static StructDef = match code {
            IdCode::OB => &s::OBJECT,
            IdCode::ME => &s::MESH,
            IdCode::MA => &s::MATERIAL,
            IdCode::CA => &s::CAMERA,
            IdCode::LA => &s::LIGHT,
            IdCode::WO => &s::WORLD,
            IdCode::IM => &s::IMAGE,
            IdCode::AR => &s::ARMATURE,
            IdCode::AC => &s::ACTION,
            IdCode::SC => &s::SCENE,
            IdCode::GR => &s::COLLECTION,
            IdCode::KE => &s::KEY,
            IdCode::NT => &s::NODE_TREE,
            IdCode::TE => &s::TEXTURE,
            IdCode::LI => &s::LIBRARY,
            IdCode::PT => &s::POINT_CLOUD,
            _ => return None,
        };
        Some((with_map!(self, code, |m| ids(m)), srna))
    }

    /// Looks up a local-first ID by collection attribute and name.
    pub fn find(&self, collection: &str, name: &str) -> Option<IdAny> {
        let code = Main::code_of(collection)?;
        with_map!(self, code, |m| m.find(name).map(|r| r.any()))
    }
    /// Names of a collection, in Blender order.
    pub fn names(&self, collection: &str) -> Option<Vec<SmolStr>> {
        let code = Main::code_of(collection)?;
        Some(with_map!(self, code, |m| m
            .iter()
            .map(|(_, t)| t.header().name.clone())
            .collect()))
    }
    /// The `i`-th ID of a collection (negative allowed).
    pub fn at(&self, collection: &str, i: isize) -> Option<IdAny> {
        let code = Main::code_of(collection)?;
        with_map!(self, code, |m| m.at(i).map(|r| r.any()))
    }
}

/// Runs `$body` with `$m` bound to every `IdMap` of `Main`.
macro_rules! for_each_map {
    ($main:expr, |$m:ident| $body:expr) => {{
        {
            let $m = &$main.objects;
            $body
        }
        {
            let $m = &$main.meshes;
            $body
        }
        {
            let $m = &$main.materials;
            $body
        }
        {
            let $m = &$main.cameras;
            $body
        }
        {
            let $m = &$main.lights;
            $body
        }
        {
            let $m = &$main.worlds;
            $body
        }
        {
            let $m = &$main.images;
            $body
        }
        {
            let $m = &$main.armatures;
            $body
        }
        {
            let $m = &$main.actions;
            $body
        }
        {
            let $m = &$main.scenes;
            $body
        }
        {
            let $m = &$main.collections;
            $body
        }
        {
            let $m = &$main.shape_keys;
            $body
        }
        {
            let $m = &$main.node_groups;
            $body
        }
        {
            let $m = &$main.textures;
            $body
        }
        {
            let $m = &$main.libraries;
            $body
        }
        {
            let $m = &$main.pointclouds;
            $body
        }
    }};
}

/// Mutable variant of [`for_each_map!`].
macro_rules! for_each_map_mut {
    ($main:expr, |$m:ident| $body:expr) => {{
        {
            let $m = &mut $main.objects;
            $body
        }
        {
            let $m = &mut $main.meshes;
            $body
        }
        {
            let $m = &mut $main.materials;
            $body
        }
        {
            let $m = &mut $main.cameras;
            $body
        }
        {
            let $m = &mut $main.lights;
            $body
        }
        {
            let $m = &mut $main.worlds;
            $body
        }
        {
            let $m = &mut $main.images;
            $body
        }
        {
            let $m = &mut $main.armatures;
            $body
        }
        {
            let $m = &mut $main.actions;
            $body
        }
        {
            let $m = &mut $main.scenes;
            $body
        }
        {
            let $m = &mut $main.collections;
            $body
        }
        {
            let $m = &mut $main.shape_keys;
            $body
        }
        {
            let $m = &mut $main.node_groups;
            $body
        }
        {
            let $m = &mut $main.textures;
            $body
        }
        {
            let $m = &mut $main.libraries;
            $body
        }
        {
            let $m = &mut $main.pointclouds;
            $body
        }
    }};
}

fn key_lib_err(msg: String) -> RnaError {
    RnaError::new(RnaErrorKind::KeyNotFound, PyExc::KeyError, msg)
}

impl Main {
    /// The first library (in `bpy.data.libraries` order) whose `filepath` is `path`, as
    /// Blender's `BLI_findstring(&bmain->libraries, path, offsetof(Library, filepath))`.
    pub fn library_by_filepath(&self, path: &str) -> Option<IdRef<LibraryId>> {
        self.libraries
            .iter()
            .find(|(_, l)| l.filepath == path)
            .map(|(r, _)| r)
    }

    /// `bpy.data.<collection>[name, lib]` (the tuple key; `PathSeg::KeyLib`), pinned on Blender
    /// 5.2.4:
    ///
    /// - `lib = None` finds the **local** ID `name` only;
    /// - `lib = Some(path)` uses the first library whose `filepath` is `path`
    ///   ([`Main::library_by_filepath`]); no such library raises `KeyError:
    ///   "bpy_prop_collection[id, lib]: lib filepath '<path>' does not reference a valid library"`;
    /// - a missing ID raises `KeyError: 'bpy_prop_collection[key, lib]: not found'`.
    ///
    /// `coll.get((name, lib))` maps both errors to its default. An unknown collection attribute
    /// gives `Ok(None)`.
    pub fn find_key_lib(
        &self,
        collection: &str,
        name: &str,
        lib: Option<&str>,
    ) -> Result<Option<IdAny>, RnaError> {
        let Some(code) = Main::code_of(collection) else {
            return Ok(None);
        };
        let lib = match lib {
            None => None,
            Some(path) => Some(self.library_by_filepath(path).ok_or_else(|| {
                key_lib_err(format!(
                    "bpy_prop_collection[id, lib]: lib filepath '{path}' does not reference a valid library"
                ))
            })?),
        };
        with_map!(self, code, |m| m.find_in(name, lib).map(|r| r.any()))
            .map(Some)
            .ok_or_else(|| key_lib_err("bpy_prop_collection[key, lib]: not found".into()))
    }

    /// `ID.rename(name, mode)` on any ID (see [`IdMap::rename`]).
    pub fn rename(
        &mut self,
        a: IdAny,
        requested: &str,
        mode: RenameMode,
        log: &mut ChangeLog,
    ) -> RenameResult {
        fn go<T: IdType>(
            m: &mut IdMap<T>,
            a: IdAny,
            requested: &str,
            mode: RenameMode,
            log: &mut ChangeLog,
        ) -> RenameResult {
            match a.typed::<T::Kind>() {
                Some(r) => m.rename(r, requested, mode, log),
                None => RenameResult::Unchanged,
            }
        }
        with_map_mut!(self, a.code, |m| go(m, a, requested, mode, log))
    }

    /// Every ID of `bpy.data`, map by map in `IdCode::ALL` order, each in Blender order.
    pub fn all_ids(&self) -> Vec<IdAny> {
        let mut out = Vec::new();
        for code in IdCode::ALL {
            with_map!(self, code, |m| out.extend(m.iter().map(|(r, _)| r.any())));
        }
        out
    }

    /// Calls `f(used)` for every outgoing ID reference of `a` (`IdType::for_each_ref`:
    /// user-counting references and plain pointers such as `Object.parent` alike).
    pub fn for_each_ref_of(&self, a: IdAny, f: &mut dyn FnMut(IdAny)) {
        with_map!(self, a.code, |m| if let Some(t) = id_ref(m, a) {
            t.for_each_ref(f);
        })
    }

    /// Calls `f(user, used)` for every **user-counting** reference (`IDWALK_CB_USER`):
    /// `Object.parent` and `Scene.camera` are references but not users (verified on 5.2.4:
    /// parenting the Cube to the Camera leaves `Camera.users == 1`).
    pub fn for_each_user_ref(&self, f: &mut dyn FnMut(IdAny, IdAny)) {
        for (r, ob) in self.objects.iter() {
            let me = r.any();
            if let Some(d) = ob.data {
                f(me, d);
            }
            for o in &ob.material_slot_overrides {
                if let Some(m) = o.material {
                    f(me, m.any());
                }
            }
            if let Some(a) = ob.animation_data.as_ref().and_then(|a| a.action) {
                f(me, a.any());
            }
            // Modifier pointers (M4 W0): node groups, textures, collections are users
            // (`IDWALK_CB_USER` in every `MOD_*.cc` `foreach_ID_link`); objects are not
            // (`IDWALK_CB_NOP`: Boolean operands, mirror objects, …).
            for m in &ob.modifiers {
                for (_, x) in m.settings.id_refs() {
                    if let Some(x) = x.filter(|x| x.code != IdCode::OB) {
                        f(me, x);
                    }
                }
            }
        }
        for (r, sc) in self.scenes.iter() {
            let me = r.any();
            sc.collection.for_each_ref(&mut |x| f(me, x));
            if let Some(w) = sc.world {
                f(me, w.any());
            }
            if let Some(a) = sc.animation_data.as_ref().and_then(|a| a.action) {
                f(me, a.any());
            }
        }
        fn generic<T: IdType>(m: &IdMap<T>, f: &mut dyn FnMut(IdAny, IdAny)) {
            for (r, t) in m.iter() {
                let me = r.any();
                t.for_each_ref(&mut |x| f(me, x));
            }
        }
        generic(&self.meshes, f);
        generic(&self.materials, f);
        // `Camera.dof.focus_object` is not a user (`IDWALK_CB_NOP`); a camera references no
        // other object.
        for (r, t) in self.cameras.iter() {
            let me = r.any();
            t.for_each_ref(&mut |x| {
                if x.code != IdCode::OB {
                    f(me, x);
                }
            });
        }
        generic(&self.lights, f);
        generic(&self.worlds, f);
        generic(&self.images, f);
        generic(&self.armatures, f);
        generic(&self.actions, f);
        generic(&self.collections, f);
        generic(&self.shape_keys, f);
        generic(&self.node_groups, f);
        generic(&self.textures, f);
        generic(&self.pointclouds, f);
        // NLA strips use their actions (`BKE_animdata_foreach_id`: `IDWALK_CB_USER` for every
        // strip's action; WP 6.4). The `AnimData.action` users are counted above.
        fn strips(me: IdAny, adt: &Option<frap_anim::AnimData>, f: &mut dyn FnMut(IdAny, IdAny)) {
            for s in adt
                .iter()
                .flat_map(|a| &a.nla_tracks)
                .flat_map(|t| &t.strips)
            {
                if let Some(a) = s.action {
                    f(me, a.any());
                }
            }
        }
        macro_rules! nla_users {
            ($($m:ident),*) => {$(
                for (r, t) in self.$m.iter() {
                    strips(r.any(), &t.animation_data, f);
                }
            )*};
        }
        nla_users!(
            objects,
            scenes,
            materials,
            cameras,
            lights,
            worlds,
            images,
            armatures,
            shape_keys,
            node_groups,
            textures,
            pointclouds
        );
        for (r, m) in self.materials.iter() {
            strips(r.any(), &m.node_tree.animation_data, f);
        }
        for (r, w) in self.worlds.iter() {
            strips(r.any(), &w.node_tree.animation_data, f);
        }
    }

    /// `ID.users` of every ID with at least one user (§5.5): user-counting references
    /// ([`Main::for_each_user_ref`]) plus one for `use_fake_user`. Libraries always have one
    /// user (verified). The window's user of its scene is added by the `ID.users` hook.
    pub fn user_counts(&self) -> BTreeMap<IdAny, u32> {
        let mut out: BTreeMap<IdAny, u32> = BTreeMap::new();
        self.for_each_user_ref(&mut |_, used| *out.entry(used).or_default() += 1);
        for_each_map!(self, |m| for (r, t) in m.iter() {
            if t.header().use_fake_user {
                *out.entry(r.any()).or_default() += 1;
            }
        });
        for (r, _) in self.libraries.iter() {
            out.insert(r.any(), 1);
        }
        out
    }

    /// `ID.users` of one ID (without the window's scene user, see [`Main::user_counts`]).
    pub fn users(&self, a: IdAny) -> u32 {
        self.user_counts().get(&a).copied().unwrap_or(0)
    }

    /// IDs with no user and no fake user, in [`Main::all_ids`] order: what a save drops (§5.5,
    /// §15). Scenes and libraries are never orphans.
    pub fn orphans(&self) -> Vec<IdAny> {
        let counts = self.user_counts();
        self.all_ids()
            .into_iter()
            .filter(|a| !matches!(a.code, IdCode::SC | IdCode::LI) && !counts.contains_key(a))
            .collect()
    }

    /// Clears every reference to `a` (`bpy.data.<coll>.remove(id, do_unlink=True)`), or
    /// retargets it to `to` (`ID.user_remap`). Only the IDs that reference `a` are touched
    /// (bumped and logged). View layers are resynced afterwards (§8).
    pub fn remap(&mut self, a: IdAny, to: Option<IdAny>, log: &mut ChangeLog) {
        fn refs<T: IdType>(t: &T, a: IdAny) -> bool {
            let mut hit = false;
            t.for_each_ref(&mut |x| hit |= x == a);
            hit
        }
        for_each_map_mut!(self, |m| m.update_where(
            |t| refs(t, a),
            |t| t.remap_ref(a, to),
            log
        ));
        crate::id::layers::sync_all(self, log);
    }

    /// Unlinks and removes an ID (`bpy.data.<coll>.remove(id)`, `do_unlink=True`).
    pub fn remove(&mut self, a: IdAny, log: &mut ChangeLog) -> bool {
        fn go<T: IdType>(m: &mut IdMap<T>, a: IdAny, log: &mut ChangeLog) -> bool {
            a.typed::<T::Kind>()
                .and_then(|r| m.remove(r, log))
                .is_some()
        }
        if self.header(a).is_none() {
            return false;
        }
        self.remap(a, None, log);
        let removed = with_map_mut!(self, a.code, |m| go(m, a, log));
        crate::id::layers::sync_all(self, log);
        removed
    }

    /// An embedded ID as an RNA struct (`Root::Embedded { owner, slot }`, §5.1).
    pub fn embedded(&self, owner: IdAny, slot: EmbeddedSlot) -> Option<&dyn Rna> {
        match (owner.code, slot) {
            (IdCode::MA, EmbeddedSlot::NodeTree) => self
                .materials
                .get(owner.typed()?)
                .map(|m| &m.node_tree as &dyn Rna),
            (IdCode::WO, EmbeddedSlot::NodeTree) => self
                .worlds
                .get(owner.typed()?)
                .map(|w| &w.node_tree as &dyn Rna),
            (IdCode::SC, EmbeddedSlot::MasterCollection) => self
                .scenes
                .get(owner.typed()?)
                .map(|s| &s.collection as &dyn Rna),
            _ => None,
        }
    }

    /// The header of an embedded ID.
    pub fn embedded_header(&self, owner: IdAny, slot: EmbeddedSlot) -> Option<&IdHeader> {
        match (owner.code, slot) {
            (IdCode::MA, EmbeddedSlot::NodeTree) => self
                .materials
                .get(owner.typed()?)
                .map(|m| &m.node_tree.header),
            (IdCode::WO, EmbeddedSlot::NodeTree) => {
                self.worlds.get(owner.typed()?).map(|w| &w.node_tree.header)
            }
            (IdCode::SC, EmbeddedSlot::MasterCollection) => self
                .scenes
                .get(owner.typed()?)
                .map(|s| &s.collection.header),
            _ => None,
        }
    }

    /// An embedded ID, mutably: through the **owner's** `IdMap::get_mut`, so the owner's
    /// `generation` bumps and the owner is logged as modified (§5.1).
    pub fn embedded_mut(
        &mut self,
        owner: IdAny,
        slot: EmbeddedSlot,
        log: &mut ChangeLog,
    ) -> Option<&mut dyn RnaMut> {
        match (owner.code, slot) {
            (IdCode::MA, EmbeddedSlot::NodeTree) => self
                .materials
                .get_mut(owner.typed()?, log)
                .map(|m| &mut m.node_tree as &mut dyn RnaMut),
            (IdCode::WO, EmbeddedSlot::NodeTree) => self
                .worlds
                .get_mut(owner.typed()?, log)
                .map(|w| &mut w.node_tree as &mut dyn RnaMut),
            (IdCode::SC, EmbeddedSlot::MasterCollection) => self
                .scenes
                .get_mut(owner.typed()?, log)
                .map(|s| &mut s.collection as &mut dyn RnaMut),
            _ => None,
        }
    }
}
