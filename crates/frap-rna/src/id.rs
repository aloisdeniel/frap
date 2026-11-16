// SPDX-License-Identifier: GPL-3.0-or-later
//! Identity (ARCHITECTURE §5.1, CONTRACT frozen at M1 W0, amended by RFC 0001 F-ED-1, F-ID-1..4).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::id_kinds::LibraryId;
use crate::traits::Rna;

/// Session-unique, never persisted, never reused (not even after rollback).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdKey(NonZeroU64);

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
static GEN: AtomicU64 = AtomicU64::new(1);

impl IdKey {
    /// Allocates a fresh key from a process-global counter (RFC 0001 F-ID-3).
    pub fn next() -> IdKey {
        let v = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
        IdKey(NonZeroU64::new(v).expect("invariant: key counter starts at 1"))
    }
    /// Raw value (debugging only; never appears in output, §21.4).
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Next value of the process-global generation counter `GEN` (§5.3). It lives outside any
/// snapshot, so a rollback never reuses a generation.
pub fn next_generation() -> u64 {
    GEN.fetch_add(1, Ordering::Relaxed)
}

/// Blender's 2-letter ID codes; also the prefix of action slot identifiers ("OBCube", "KEKey").
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[non_exhaustive]
pub enum IdCode {
    /// Object
    OB,
    /// Mesh
    ME,
    /// Material
    MA,
    /// Camera
    CA,
    /// Light
    LA,
    /// World
    WO,
    /// Image
    IM,
    /// Armature
    AR,
    /// Action
    AC,
    /// Scene
    SC,
    /// Collection
    GR,
    /// Key (shape keys)
    KE,
    /// NodeTree
    NT,
    /// Texture
    TE,
    /// Library
    LI,
    /// PointCloud
    PT,
}

impl IdCode {
    /// Every code, in declaration order.
    pub const ALL: [IdCode; 16] = [
        IdCode::OB,
        IdCode::ME,
        IdCode::MA,
        IdCode::CA,
        IdCode::LA,
        IdCode::WO,
        IdCode::IM,
        IdCode::AR,
        IdCode::AC,
        IdCode::SC,
        IdCode::GR,
        IdCode::KE,
        IdCode::NT,
        IdCode::TE,
        IdCode::LI,
        IdCode::PT,
    ];

    /// The `bpy.data` collection attribute for this code (repr of a dynamic `IdAny`, F-ID-1).
    pub const fn collection(self) -> &'static str {
        match self {
            IdCode::OB => "objects",
            IdCode::ME => "meshes",
            IdCode::MA => "materials",
            IdCode::CA => "cameras",
            IdCode::LA => "lights",
            IdCode::WO => "worlds",
            IdCode::IM => "images",
            IdCode::AR => "armatures",
            IdCode::AC => "actions",
            IdCode::SC => "scenes",
            IdCode::GR => "collections",
            IdCode::KE => "shape_keys",
            IdCode::NT => "node_groups",
            IdCode::TE => "textures",
            IdCode::LI => "libraries",
            IdCode::PT => "pointclouds",
        }
    }
    /// The two-letter code as a string ("OB").
    pub const fn as_str(self) -> &'static str {
        match self {
            IdCode::OB => "OB",
            IdCode::ME => "ME",
            IdCode::MA => "MA",
            IdCode::CA => "CA",
            IdCode::LA => "LA",
            IdCode::WO => "WO",
            IdCode::IM => "IM",
            IdCode::AR => "AR",
            IdCode::AC => "AC",
            IdCode::SC => "SC",
            IdCode::GR => "GR",
            IdCode::KE => "KE",
            IdCode::NT => "NT",
            IdCode::TE => "TE",
            IdCode::LI => "LI",
            IdCode::PT => "PT",
        }
    }
}

/// Zero-sized marker per ID type, declared in `frap-rna` (L1) so that any crate can name an ID
/// type without depending on the crate that defines its data. Markers are in [`crate::id_kinds`].
pub trait IdKind: Copy + Send + Sync + 'static {
    /// Blender ID code.
    const CODE: IdCode;
    /// `bpy.data` attribute ("objects", "meshes", …).
    const COLLECTION: &'static str;
    /// Default name for new datablocks of this kind ("Object", "Mesh", "Material", …).
    const DEFAULT_NAME: &'static str;
}

/// Typed pointer between datablocks. Never a name, never an `Arc`. `Clone/Copy/Eq/Hash/Ord/Debug`
/// are implemented by hand so they carry no `K:` bounds.
pub struct IdRef<K: IdKind>(IdKey, PhantomData<fn() -> K>);

impl<K: IdKind> IdRef<K> {
    /// Wraps a key (the caller guarantees it belongs to an ID of kind `K`).
    pub fn from_key(key: IdKey) -> Self {
        IdRef(key, PhantomData)
    }
    /// The underlying key.
    pub fn key(self) -> IdKey {
        self.0
    }
    /// Narrows a type-erased reference, checked by code (F-ID-2).
    pub fn from_any(a: IdAny) -> Option<Self> {
        (a.code == K::CODE).then(|| IdRef::from_key(a.key))
    }
    /// Type-erased form.
    pub fn any(self) -> IdAny {
        IdAny {
            code: K::CODE,
            key: self.0,
        }
    }
}
impl<K: IdKind> Copy for IdRef<K> {}
impl<K: IdKind> Clone for IdRef<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: IdKind> PartialEq for IdRef<K> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<K: IdKind> Eq for IdRef<K> {}
impl<K: IdKind> Hash for IdRef<K> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}
impl<K: IdKind> PartialOrd for IdRef<K> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<K: IdKind> Ord for IdRef<K> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.cmp(&o.0)
    }
}
impl<K: IdKind> fmt::Debug for IdRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IdRef<{:?}>({})", K::CODE, self.0.get())
    }
}
impl<K: IdKind> From<IdRef<K>> for IdAny {
    fn from(r: IdRef<K>) -> IdAny {
        r.any()
    }
}

/// Type-erased ID reference.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdAny {
    /// Kind of the ID.
    pub code: IdCode,
    /// Its key.
    pub key: IdKey,
}

impl IdAny {
    /// Narrows to a typed reference when the code matches (same as [`IdRef::from_any`]).
    pub fn typed<K: IdKind>(self) -> Option<IdRef<K>> {
        IdRef::from_any(self)
    }
}

/// An ID property value (`obj["foo"]`), exported as glTF `extras`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum IdProp {
    /// int
    Int(i64),
    /// float (Blender stores doubles)
    Float(f64),
    /// bool
    Bool(bool),
    /// str
    Str(SmolStr),
    /// int array
    IntArray(Vec<i64>),
    /// float array
    FloatArray(Vec<f64>),
    /// nested group (dict)
    Group(IndexMap<SmolStr, IdProp>),
    /// list of groups or mixed values (IDP_IDPARRAY)
    List(Vec<IdProp>),
}

/// Common datablock header.
#[derive(Clone, Debug)]
pub struct IdHeader {
    /// `ID.name`: ≤255 bytes UTF-8, unique within its `IdMap` per library.
    pub name: SmolStr,
    /// `ID.use_fake_user`
    pub use_fake_user: bool,
    /// `ID.library`; `None` = local.
    pub library: Option<IdRef<LibraryId>>,
    /// `ID.is_linked_packed` (linked from a packed library).
    pub is_linked_packed: bool,
    /// `ID.is_embedded_data`
    pub is_embedded_data: bool,
    /// Bumped on every mutable access (§5.3). Renamed from `gen`: reserved keyword in edition
    /// 2024 (RFC 0001 F-ED-1).
    pub generation: u64,
    /// Deterministic per session, exposed as RNA `session_uid`.
    pub session_uid: u32,
    /// ID properties (`obj["foo"]`) → glTF extras.
    pub custom: IndexMap<SmolStr, IdProp>,
}

impl IdHeader {
    /// Local, non-embedded header with the given name and a fresh generation.
    pub fn new(name: impl Into<SmolStr>) -> Self {
        IdHeader {
            name: name.into(),
            use_fake_user: false,
            library: None,
            is_linked_packed: false,
            is_embedded_data: false,
            generation: next_generation(),
            session_uid: 0,
            custom: IndexMap::new(),
        }
    }
    /// Header of an embedded ID (`Material.node_tree`, `Scene.collection`).
    pub fn embedded(name: impl Into<SmolStr>) -> Self {
        IdHeader {
            is_embedded_data: true,
            ..IdHeader::new(name)
        }
    }
}

/// Implemented by the data type, in whatever crate defines it. Exactly one data type per kind
/// (checked by a test in frap-ops-all that walks every `IdMap` of `Main`).
pub trait IdType: Rna + Clone + Send + Sync + 'static {
    /// The marker kind tying data to `IdRef<Kind>`.
    type Kind: IdKind;
    /// Header access.
    fn header(&self) -> &IdHeader;
    /// Mutable header access.
    fn header_mut(&mut self) -> &mut IdHeader;
    /// Outgoing references (`BKE_library_foreach_ID_link`): users, orphan purge, save, deps.
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny));
    /// Retarget references (`remove(do_unlink=True)`, `user_remap`).
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>);
    /// Embedded IDs owned by this ID (`Material`/`World.node_tree`, `Scene.collection`).
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[]
    }
}

/// Embedded IDs have an `IdHeader` (`is_embedded_data = True`) but no `IdKey` and no `IdMap`
/// entry; they are addressed as `Root::Embedded { owner, slot }` (§9.3).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum EmbeddedSlot {
    /// `Material.node_tree`, `World.node_tree`
    NodeTree,
    /// `Scene.collection`
    MasterCollection,
}

impl EmbeddedSlot {
    /// RNA property of the owner that holds the embedded ID (F-ID-4).
    pub const fn prop_identifier(self) -> &'static str {
        match self {
            EmbeddedSlot::NodeTree => "node_tree",
            EmbeddedSlot::MasterCollection => "collection",
        }
    }
}
