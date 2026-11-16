// SPDX-License-Identifier: GPL-3.0-or-later
//! Serde for ID references (§15, decision W0-18): `IdRef<K>` and `IdAny` are written as
//! `{"$id": "ME", "name": "Cube"}`, plus `"library": "<library name>"` for linked IDs. `IdKey`s
//! are session-only and never written; they are regenerated on load.
//!
//! Like [`crate::serde_buf`], the names come from a scoped, thread-local context, so data
//! crates derive `Serialize`/`Deserialize` on fields such as `Vec<Option<IdRef<MaterialId>>>`
//! without depending on `frap-project`:
//!
//! * save: [`with_namer`] installs an [`IdRefNamer`] (`IdAny` → name and library name);
//! * load: [`with_resolver`] installs an [`IdRefResolver`] (`(code, name, library)` → `IdKey`).
//!
//! Serializing or deserializing a reference with nothing installed is an error, never a silent
//! fallback. `Option<IdRef<K>>` uses serde's `Option` impl (`None` is `null`).
//!
//! The library element is the library **name** (`bpy.data.libraries` key), not its filepath:
//! library names are unique while two libraries may share a filepath (verified: the two
//! `geometry_nodes_essentials.blend` libraries of `shade_auto_smooth`, §5.4). This is why the
//! namer is its own trait rather than [`crate::ptr::IdNames`], whose second element is the
//! library path used by `repr`.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use serde::de::Error as _;
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

use crate::id::{IdAny, IdCode, IdKey, IdKind, IdRef};

/// Names IDs while a `.frap` is being written (implemented by `frap-project` over `Main`).
pub trait IdRefNamer {
    /// `(name, library name)` of `id`; the library is `None` for local IDs. `None` if the ID
    /// does not exist (a dangling reference, which fails the save).
    fn ref_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)>;
}

/// Resolves references while a `.frap` is being read (implemented by `frap-project`).
pub trait IdRefResolver {
    /// The key of the ID of kind `code` called `name` in library `library` (`None` = local).
    /// The loader may allocate keys on demand so that forward references resolve before the
    /// referenced ID is inserted. `None` fails the load with an unresolved-reference error.
    fn resolve(&mut self, code: IdCode, name: &str, library: Option<&str>) -> Option<IdKey>;
}

/// A ready-made name table usable as both [`IdRefNamer`] and [`IdRefResolver`] (tests, and
/// any caller that collects names up front).
#[derive(Clone, Debug, Default)]
pub struct IdNameTable {
    by_id: HashMap<IdAny, (SmolStr, Option<SmolStr>)>,
    by_name: HashMap<(IdCode, SmolStr, Option<SmolStr>), IdKey>,
}

impl IdNameTable {
    /// Empty table.
    pub fn new() -> Self {
        Self::default()
    }
    /// Records `id` as `name` (in library `library`, by library name).
    pub fn insert(&mut self, id: IdAny, name: impl Into<SmolStr>, library: Option<&str>) {
        let name = name.into();
        let library = library.map(SmolStr::new);
        self.by_name
            .insert((id.code, name.clone(), library.clone()), id.key);
        self.by_id.insert(id, (name, library));
    }
}

impl IdRefNamer for IdNameTable {
    fn ref_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        self.by_id.get(&id).cloned()
    }
}

impl IdRefResolver for IdNameTable {
    fn resolve(&mut self, code: IdCode, name: &str, library: Option<&str>) -> Option<IdKey> {
        self.by_name
            .get(&(code, SmolStr::new(name), library.map(SmolStr::new)))
            .copied()
    }
}

trait ErasedNamer {
    fn ref_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<N: IdRefNamer + 'static> ErasedNamer for N {
    fn ref_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        IdRefNamer::ref_name(self, id)
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

trait ErasedResolver {
    fn resolve(&mut self, code: IdCode, name: &str, library: Option<&str>) -> Option<IdKey>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<R: IdRefResolver + 'static> ErasedResolver for R {
    fn resolve(&mut self, code: IdCode, name: &str, library: Option<&str>) -> Option<IdKey> {
        IdRefResolver::resolve(self, code, name, library)
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

thread_local! {
    static NAMER: RefCell<Option<Box<dyn ErasedNamer>>> = const { RefCell::new(None) };
    static RESOLVER: RefCell<Option<Box<dyn ErasedResolver>>> = const { RefCell::new(None) };
}

/// Restores the previously installed namer/resolver on drop (also on panic).
struct Restore<F: FnMut()>(F);
impl<F: FnMut()> Drop for Restore<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Runs `f` with `namer` installed for this thread, then returns `f`'s result and the namer.
/// Nested calls restore the outer namer.
pub fn with_namer<N: IdRefNamer + 'static, R>(namer: N, f: impl FnOnce() -> R) -> (R, N) {
    let prev = NAMER.with(|c| c.borrow_mut().replace(Box::new(namer)));
    let mut prev = Some(prev);
    let mut taken: Option<Box<dyn ErasedNamer>> = None;
    let r = {
        let _guard = Restore(|| {
            let mine = NAMER.with(|c| {
                let mut slot = c.borrow_mut();
                let mine = slot.take();
                *slot = prev.take().flatten();
                mine
            });
            if taken.is_none() {
                taken = mine;
            }
        });
        f()
    };
    let namer = taken
        .expect("invariant: the namer installed by with_namer is still there")
        .into_any()
        .downcast::<N>()
        .expect("invariant: with_namer gets back the namer type it installed");
    (r, *namer)
}

/// Runs `f` with `resolver` installed for this thread, then returns `f`'s result and the
/// resolver. Nested calls restore the outer resolver.
pub fn with_resolver<Q: IdRefResolver + 'static, R>(resolver: Q, f: impl FnOnce() -> R) -> (R, Q) {
    let prev = RESOLVER.with(|c| c.borrow_mut().replace(Box::new(resolver)));
    let mut prev = Some(prev);
    let mut taken: Option<Box<dyn ErasedResolver>> = None;
    let r = {
        let _guard = Restore(|| {
            let mine = RESOLVER.with(|c| {
                let mut slot = c.borrow_mut();
                let mine = slot.take();
                *slot = prev.take().flatten();
                mine
            });
            if taken.is_none() {
                taken = mine;
            }
        });
        f()
    };
    let resolver = taken
        .expect("invariant: the resolver installed by with_resolver is still there")
        .into_any()
        .downcast::<Q>()
        .expect("invariant: with_resolver gets back the resolver type it installed");
    (r, *resolver)
}

/// The wire form of a reference.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    #[serde(rename = "$id")]
    code: String,
    name: String,
    #[serde(default)]
    library: Option<String>,
}

fn serialize_any<S: Serializer>(id: IdAny, s: S) -> Result<S::Ok, S::Error> {
    let named = NAMER
        .with(|c| c.borrow().as_ref().map(|n| n.ref_name(id)))
        .ok_or_else(|| {
            S::Error::custom(format!(
                "reference to a {} serialized with no IdRefNamer installed \
                 (frap_rna::serde_id::with_namer)",
                id.code.as_str()
            ))
        })?;
    let (name, library) = named.ok_or_else(|| {
        S::Error::custom(format!(
            "dangling reference to a {} (key {}) during save",
            id.code.as_str(),
            id.key.get()
        ))
    })?;
    let mut m = s.serialize_map(Some(if library.is_some() { 3 } else { 2 }))?;
    m.serialize_entry("$id", id.code.as_str())?;
    m.serialize_entry("name", name.as_str())?;
    if let Some(lib) = &library {
        m.serialize_entry("library", lib.as_str())?;
    }
    m.end()
}

fn resolve_wire(w: &Wire, code: IdCode) -> Result<IdKey, String> {
    RESOLVER
        .with(|c| {
            c.borrow_mut()
                .as_mut()
                .map(|r| r.resolve(code, &w.name, w.library.as_deref()))
        })
        .ok_or_else(|| {
            format!(
                "reference {{\"$id\": \"{}\", \"name\": {:?}}} read with no IdRefResolver \
                 installed (frap_rna::serde_id::with_resolver)",
                w.code, w.name
            )
        })?
        .ok_or_else(|| match &w.library {
            None => format!("unresolved reference: {} {:?}", w.code, w.name),
            Some(lib) => format!(
                "unresolved reference: {} {:?} in library {lib:?}",
                w.code, w.name
            ),
        })
}

fn parse_code(code: &str) -> Result<IdCode, String> {
    IdCode::from_code(code).ok_or_else(|| format!("unknown ID code {code:?} in \"$id\""))
}

impl<K: IdKind> Serialize for IdRef<K> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_any(self.any(), s)
    }
}

impl<'de, K: IdKind> Deserialize<'de> for IdRef<K> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = Wire::deserialize(d)?;
        let code = parse_code(&w.code).map_err(D::Error::custom)?;
        if code != K::CODE {
            return Err(D::Error::custom(format!(
                "expected a reference to a {} ({}), got \"$id\": \"{}\"",
                K::CODE.as_str(),
                K::COLLECTION,
                w.code
            )));
        }
        resolve_wire(&w, code)
            .map(IdRef::from_key)
            .map_err(D::Error::custom)
    }
}

impl Serialize for IdAny {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_any(*self, s)
    }
}

impl<'de> Deserialize<'de> for IdAny {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = Wire::deserialize(d)?;
        let code = parse_code(&w.code).map_err(D::Error::custom)?;
        resolve_wire(&w, code)
            .map(|key| IdAny { code, key })
            .map_err(D::Error::custom)
    }
}
