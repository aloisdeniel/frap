// SPDX-License-Identifier: GPL-3.0-or-later
//! §5.1 Identity (`frap-rna/src/id.rs`, CONTRACT).

use core::fmt;
use core::marker::PhantomData;
use core::num::NonZeroU64;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::traits::Rna;

/// Session-unique, never persisted, never reused (not even after rollback).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdKey(NonZeroU64);

impl IdKey {
    /// RFC 0001 F-ID-3: the contract gives no constructor; `Main` needs one (process-global counter).
    pub fn next() -> IdKey {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        IdKey(NonZeroU64::new(NEXT.fetch_add(1, Ordering::Relaxed)).expect("invariant: counter starts at 1"))
    }
}

/// Blender's 2-letter ID codes; also the prefix of action slot identifiers ("OBCube", "KEKey").
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[non_exhaustive]
pub enum IdCode {
    OB,
    ME,
    MA,
    CA,
    LA,
    WO,
    IM,
    AR,
    AC,
    SC,
    GR,
    KE,
    NT,
    TE,
    LI,
    PT,
}

impl IdCode {
    /// RFC 0001 F-ID-1: `IdAny` only carries the code, but `repr` (§9.3) needs the `bpy.data`
    /// collection name for a dynamic ID. `IdKind::COLLECTION` is only reachable statically.
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
}

/// Zero-sized marker per ID type, declared here (L1) so that any crate can name an ID type
/// without depending on the crate that defines its data.
pub trait IdKind: Copy + Send + Sync + 'static {
    const CODE: IdCode;
    const COLLECTION: &'static str;
    const DEFAULT_NAME: &'static str;
}

macro_rules! id_kinds {
    ($($name:ident = $code:ident, $coll:literal, $dflt:literal;)*) => {$(
        #[derive(Copy, Clone, Debug)]
        pub enum $name {}
        impl IdKind for $name {
            const CODE: IdCode = IdCode::$code;
            const COLLECTION: &'static str = $coll;
            const DEFAULT_NAME: &'static str = $dflt;
        }
    )*};
}
id_kinds! {
    ObjectId = OB, "objects", "Object";
    MeshId = ME, "meshes", "Mesh";
    MaterialId = MA, "materials", "Material";
    CameraId = CA, "cameras", "Camera";
    LightId = LA, "lights", "Light";
    WorldId = WO, "worlds", "World";
    ImageId = IM, "images", "Image";
    ArmatureId = AR, "armatures", "Armature";
    ActionId = AC, "actions", "Action";
    SceneId = SC, "scenes", "Scene";
    CollectionId = GR, "collections", "Collection";
    KeyId = KE, "shape_keys", "Key";
    NodeTreeId = NT, "node_groups", "NodeTree";
    TextureId = TE, "textures", "Texture";
    LibraryId = LI, "libraries", "Library";
    PointCloudId = PT, "pointclouds", "PointCloud";
}

/// Typed pointer between datablocks. Never a name, never an Arc.
pub struct IdRef<K: IdKind>(IdKey, PhantomData<fn() -> K>);

// Manual impls: `#[derive]` would add `K: Trait` bounds (harmless here because markers derive
// Copy/Clone, but `Eq`/`Hash`/`Ord` on an uninhabited enum must not be required).
impl<K: IdKind> Clone for IdRef<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: IdKind> Copy for IdRef<K> {}
impl<K: IdKind> PartialEq for IdRef<K> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<K: IdKind> Eq for IdRef<K> {}
impl<K: IdKind> core::hash::Hash for IdRef<K> {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}
impl<K: IdKind> PartialOrd for IdRef<K> {
    fn partial_cmp(&self, o: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<K: IdKind> Ord for IdRef<K> {
    fn cmp(&self, o: &Self) -> core::cmp::Ordering {
        self.0.cmp(&o.0)
    }
}
impl<K: IdKind> fmt::Debug for IdRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IdRef<{:?}>({:?})", K::CODE, self.0)
    }
}

impl<K: IdKind> IdRef<K> {
    /// RFC 0001 F-ID-2: needed by `IdMap::insert`; not in the contract.
    pub fn from_key(key: IdKey) -> Self {
        IdRef(key, PhantomData)
    }
    pub fn key(self) -> IdKey {
        self.0
    }
    /// RFC 0001 F-ID-2: `IdAny` -> `IdRef<K>` (checked) and back; needed by every resolver step
    /// that re-roots at an ID and by hooks that receive `RnaPtr { root: Id(IdAny) }`.
    pub fn from_any(a: IdAny) -> Option<Self> {
        (a.code == K::CODE).then_some(IdRef(a.key, PhantomData))
    }
    pub fn any(self) -> IdAny {
        IdAny {
            code: K::CODE,
            key: self.0,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct IdAny {
    pub code: IdCode,
    pub key: IdKey,
}

/// Implemented by the data type, in whatever crate defines it.
pub trait IdType: Rna + Clone + Send + Sync + 'static {
    type Kind: IdKind;
    fn header(&self) -> &IdHeader;
    fn header_mut(&mut self) -> &mut IdHeader;
    /// Outgoing references (BKE_library_foreach_ID_link).
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny));
    /// Retarget references (remove(do_unlink=True), user_remap).
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>);
    /// Embedded IDs owned by this ID (Material/World.node_tree, Scene.collection).
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[]
    }
}

/// Embedded IDs have an IdHeader (is_embedded_data = True) but no IdKey and no IdMap entry.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum EmbeddedSlot {
    NodeTree,         /* Material, World */
    MasterCollection, /* Scene */
}

impl EmbeddedSlot {
    /// RFC 0001 F-ID-4: the RNA property of the owner that holds the embedded ID; `repr` and the
    /// mutable walk (`RnaMut::struct_mut(owner, <this prop>)`) both need it.
    pub const fn prop_identifier(self) -> &'static str {
        match self {
            EmbeddedSlot::NodeTree => "node_tree",
            EmbeddedSlot::MasterCollection => "collection",
        }
    }
}

/// ID property value (`obj["foo"]`); minimal for the probe.
#[derive(Clone, Debug, PartialEq)]
pub enum IdProp {
    Int(i32),
    Float(f64),
    Str(SmolStr),
}

#[derive(Clone, Debug, Default)]
pub struct IdHeader {
    pub name: SmolStr,
    pub use_fake_user: bool,
    pub library: Option<IdRef<LibraryId>>,
    pub is_linked_packed: bool,
    pub is_embedded_data: bool,
    /// §5.1 names this field `gen`, a reserved keyword in edition 2024 (RFC 0001 F-ED-1).
    pub generation: u64,
    pub session_uid: u32,
    pub custom: IndexMap<SmolStr, IdProp>,
}

impl IdHeader {
    pub fn named(name: &str) -> Self {
        IdHeader {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// Process-global generation counter (§5.3).
pub fn next_gen() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static GEN: AtomicU64 = AtomicU64::new(1);
    GEN.fetch_add(1, Ordering::Relaxed)
}
