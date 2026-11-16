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
use frap_rna::naming::unique_name;
use frap_rna::{
    IdAny, IdCode, IdHeader, IdKey, IdKind, IdNames, IdRef, IdType, LibraryId, Rna, RnaMut,
    StructDef,
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
    /// `header.generation` bump, logged as modified.
    pub fn get_mut(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<&mut T> {
        let arc = self.items.get_mut(&r.key())?;
        let t = Arc::make_mut(arc);
        t.header_mut().generation = next_generation();
        log.modified.insert(r.any());
        Some(t)
    }
    /// `coll['Name']`, `coll.get()`: the local ID first, then linked IDs in library order.
    pub fn find(&self, name: &str) -> Option<IdRef<T::Kind>> {
        if let Some(k) = self.by_name.get(&(SmolStr::new(name), None)) {
            return Some(IdRef::from_key(*k));
        }
        self.items
            .iter()
            .find(|(_, v)| v.header().name == name)
            .map(|(k, _)| IdRef::from_key(*k))
    }
    /// `coll['Name', <lib>]`: one library explicitly. The exact Blender rule is pinned by
    /// T0 goldens in WP 1.10 before the tuple form is exposed (§5.2).
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
    /// Inserts a local ID with a unique name (lowest free `.NNN`) at its sorted position.
    pub fn insert(&mut self, requested: &str, mut value: T, log: &mut ChangeLog) -> IdRef<T::Kind> {
        let lib = lib_key(value.header());
        let name = unique_name(
            &|n| self.by_name.contains_key(&(SmolStr::new(n), lib)),
            requested,
            255,
            T::Kind::DEFAULT_NAME,
        );
        value.header_mut().name = name.clone();
        value.header_mut().generation = next_generation();
        let key = IdKey::next();
        // Sorted ListBase insert: after every ID whose name is <= the new one (stable).
        let pos = self
            .items
            .values()
            .position(|v| strcasecmp(&v.header().name, &name) == Ordering::Greater)
            .unwrap_or(self.items.len());
        self.items.shift_insert(pos, key, Arc::new(value));
        self.by_name.insert((name, lib), key);
        let r = IdRef::from_key(key);
        log.added.push(r.any());
        r
    }
    /// Renames with Blender's collision rules and re-sorts (§5.4).
    pub fn rename(
        &mut self,
        r: IdRef<T::Kind>,
        requested: &str,
        mode: RenameMode,
        log: &mut ChangeLog,
    ) -> RenameResult {
        let _ = (r, requested, mode, log);
        todo!("WP 1.10: ID rename with RenameMode and re-sort")
    }
    /// Removes an ID.
    pub fn remove(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<Arc<T>> {
        let v = self.items.shift_remove(&r.key())?;
        self.by_name
            .remove(&(v.header().name.clone(), lib_key(v.header())));
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

impl IdNames for Main {
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        let h = self.header(id)?;
        let lib = h
            .library
            .and_then(|l| self.libraries.get(l))
            .map(|l| SmolStr::new(&l.filepath));
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
