// SPDX-License-Identifier: GPL-3.0-or-later
//! Read access along canonical pointers (ARCHITECTURE §9.3, §9.4): walking an `RnaPtr` from
//! its root through `Rna::get`, collection items and get hooks, without ever borrowing across
//! datablocks. Walks are continuation-passing because a proxy (`StructRef::Owned`) lives only
//! as long as the field it came from.

use std::ops::Range;

use frap_rna::{
    Bulk, CollRef, Field, FrapError, IdAny, IdNames, Item, PropDef, PyExc, Rna, RnaError,
    RnaErrorKind, RnaPtr, Root, Seg, StructDef,
};
use frap_scene::{HostRead, find_hook};
use smol_str::SmolStr;

use crate::errors::*;

/// An internal inconsistency between a canonical pointer and the data (an ID removed while a
/// pointer to it was held, a binding that changed shape): `RuntimeError`, code `INTERNAL`.
pub fn stale(what: impl std::fmt::Display) -> FrapError {
    FrapError::new(
        PyExc::RuntimeError,
        frap_rna::ErrorCode::INTERNAL,
        format!("frap internal: {what}"),
    )
}

/// An `RnaError` with its own message and the default code of its kind.
pub fn rna_err(e: RnaError) -> FrapError {
    FrapError::from(e)
}

/// The refined RNA struct of an ID (`SunLight` for a sun lamp).
pub fn id_srna(host: &dyn HostRead, id: IdAny) -> Result<&'static StructDef, FrapError> {
    host.main()
        .id(id)
        .map(|r| r.srna())
        .ok_or_else(|| stale(format!("{:?} no longer exists", id.code)))
}

/// Reads `prop` of `cur`, calling the registered get hook when the binding asks for it
/// (`Field::NeedsHook`, F-TR-3). `this` is the canonical pointer of `cur`.
pub fn get_field<'a>(
    host: &'a dyn HostRead,
    cur: &'a dyn Rna,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'a>, FrapError> {
    match cur.get(prop) {
        Ok(Field::NeedsHook) => call_get_hook(host, this, prop),
        Ok(f) => Ok(f),
        // A binding that does not know the property yet: a registered hook may still serve it
        // (Python-only members, hook-only collections).
        Err(e) if e.kind == RnaErrorKind::NoAttribute => {
            match find_hook(this.srna, prop.identifier).and_then(|h| h.get) {
                Some(get) => get(host, this, prop).map_err(rna_err),
                None => Err(not_bound(this.srna, prop)),
            }
        }
        Err(e) => Err(rna_err(e)),
    }
}

/// The get hook of `(this.srna, prop)`.
pub fn call_get_hook<'a>(
    host: &'a dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'a>, FrapError> {
    match find_hook(this.srna, prop.identifier).and_then(|h| h.get) {
        Some(get) => get(host, this, prop).map_err(rna_err),
        None => Err(FrapError::new(
            PyExc::NotImplementedError,
            NOT_IMPLEMENTED,
            format!(
                "frap: {}.{} needs an RNA get hook that is not implemented yet",
                this.srna.identifier, prop.identifier
            ),
        )),
    }
}

/// A property that exists in Blender 5.2 but has no frap binding yet (`NotImplementedError`,
/// code `RNA_NOT_IMPLEMENTED`; the golden dumper treats it as "unbound").
pub fn not_bound(srna: &'static StructDef, prop: &PropDef) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        frap_rna::codes::RNA_NOT_IMPLEMENTED,
        format!(
            "frap: {}.{} exists in Blender 5.2 but is not bound by frap yet",
            srna.identifier, prop.identifier
        ),
    )
}

/// The evaluated object (viewport), from the cache of this thread's reads
/// ([`frap_eval::EvalCache::evaluated_for_read`]: `with_struct` sees only `&dyn HostRead`,
/// not the session's evaluator). The RNA get hooks of evaluated data (`Object.dimensions`,
/// `Object.bound_box`) share the same cache (M8 WP 8.8), so a change is evaluated once
/// whichever read comes first.
pub(crate) fn evaluated_for_read(
    host: &dyn HostRead,
    ob: frap_rna::IdRef<frap_rna::ObjectId>,
) -> Result<std::sync::Arc<frap_scene::EvaluatedObject>, FrapError> {
    frap_eval::EvalCache::evaluated_for_read(host, ob, frap_scene::EvalMode::Viewport)
        .map_err(|e| stale(e.message))
}

/// Runs `f` on the struct at `ptr` (an ID, an embedded ID or a struct below one).
pub fn with_struct<R>(
    host: &dyn HostRead,
    ptr: &RnaPtr,
    f: &mut dyn FnMut(&dyn Rna) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    let main = host.main();
    match ptr.root {
        // WP 4.1: `Object.to_mesh()` and `ob.evaluated_get(dg).data` of an object with
        // modifiers are `objects['X'].data` (a path through an ID pointer never occurs
        // otherwise, the resolver re-roots there): they read the evaluated mesh, always fresh.
        Root::Id(id)
            if id.code == frap_rna::IdCode::OB
                && matches!(ptr.path.first(), Some(Seg::Prop("data"))) =>
        {
            let ob = id
                .typed()
                .ok_or_else(|| stale("an object pointer of another type"))?;
            let eo = evaluated_for_read(host, ob)?;
            let mesh = eo
                .mesh
                .clone()
                .ok_or_else(|| stale("the object has no evaluated mesh"))?;
            let mut root =
                RnaPtr::id(id, &frap_spec_rna::OBJECT).child(Seg::Prop("data"), mesh.srna());
            root.detached = ptr.detached;
            walk(host, &*mesh, &root, &ptr.path[1..], f)
        }
        Root::Id(id) => {
            // §6.3: a mesh in Edit Mode reads through the cached view of its BMesh (WP 2.2).
            if let Some(view) = crate::edit::edit_view(host, id) {
                let root = RnaPtr::id(id, view.srna());
                return walk(host, &*view, &root, &ptr.path, f);
            }
            let r = main
                .id(id)
                .ok_or_else(|| stale(format!("{:?} no longer exists", id.code)))?;
            let root = RnaPtr::id(id, r.srna());
            walk(host, r, &root, &ptr.path, f)
        }
        Root::Embedded { owner, slot } => {
            let o = main
                .id(owner)
                .ok_or_else(|| stale(format!("{:?} no longer exists", owner.code)))?;
            let prop = o
                .srna()
                .find_prop(slot.prop_identifier())
                .ok_or_else(|| stale("embedded slot without a property"))?;
            let owner_ptr = RnaPtr::id(owner, o.srna());
            match get_field(host, o, &owner_ptr, prop)? {
                Field::Embedded(_, s) | Field::Struct(s) => {
                    let root = RnaPtr::new(ptr.root, s.srna());
                    walk(host, &*s, &root, &ptr.path, f)
                }
                _ => Err(stale("embedded slot is not a struct")),
            }
        }
        // `bpy.context.window` / `space_data` / `preferences` and below (WP 2.3).
        Root::Context if !ptr.path.is_empty() => {
            let root = super::context::ui::root(host);
            let here = RnaPtr::new(Root::Context, &frap_spec_rna::CONTEXT);
            walk(host, &root, &here, &ptr.path, f)
        }
        _ => Err(stale("bpy.data / bpy.context are not RNA structs")),
    }
}

fn walk<R>(
    host: &dyn HostRead,
    cur: &dyn Rna,
    here: &RnaPtr,
    rest: &[Seg],
    f: &mut dyn FnMut(&dyn Rna) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    let Some((seg, rest)) = rest.split_first() else {
        return f(cur);
    };
    let Seg::Prop(name) = seg else {
        return Err(stale(format!("{seg:?} without a collection")));
    };
    let prop = cur
        .srna()
        .find_prop(name)
        .ok_or_else(|| stale(format!("{}.{name} vanished", cur.srna().identifier)))?;
    match get_field(host, cur, here, prop)? {
        Field::Struct(s) | Field::Embedded(_, s) => {
            let next = here.child(Seg::Prop(prop.identifier), s.srna());
            walk(host, &*s, &next, rest, f)
        }
        Field::Coll(c) => {
            let Some((key, rest)) = rest.split_first() else {
                return Err(stale("pointer ends on a collection"));
            };
            let view = CollView::Rna {
                coll: c,
                main: host,
            };
            let i = match key {
                Seg::Index(i) => usize::try_from(*i).map_err(|_| stale("negative index"))?,
                Seg::Key(k) => view
                    .find(k)
                    .ok_or_else(|| stale(format!("key {k:?} vanished")))?,
                Seg::Prop(_) => return Err(stale("collection followed by a property")),
            };
            match view.item(i).map_err(rna_err)? {
                Item::Struct(s) => {
                    let next = here
                        .child(Seg::Prop(prop.identifier), s.srna())
                        .child(view.item_seg(i), s.srna());
                    walk(host, &*s, &next, rest, f)
                }
                Item::Id(_) => Err(stale("canonical pointers never cross an ID")),
            }
        }
        _ => Err(stale(format!(
            "{}.{name} is not a struct",
            here.srna.identifier
        ))),
    }
}

/// A collection seen through one interface: the IDs of a `bpy.data` collection, or an RNA
/// collection (whose ID items are named through `IdNames`, F-TR-4).
pub enum CollView<'a> {
    /// `bpy.data.<coll>`: IDs in Blender order.
    Ids {
        /// The IDs.
        ids: Vec<IdAny>,
        /// Item struct.
        item_type: &'static StructDef,
        /// Name lookup.
        main: &'a dyn HostRead,
    },
    /// An RNA collection.
    Rna {
        /// The collection.
        coll: CollRef<'a>,
        /// Name lookup for ID items.
        main: &'a dyn HostRead,
    },
}

impl CollView<'_> {
    /// Item struct.
    pub fn item_type(&self) -> &'static StructDef {
        match self {
            CollView::Ids { item_type, .. } => item_type,
            CollView::Rna { coll, .. } => coll.item_type(),
        }
    }
    /// Number of items.
    pub fn len(&self) -> usize {
        match self {
            CollView::Ids { ids, .. } => ids.len(),
            CollView::Rna { coll, .. } => coll.len(),
        }
    }
    /// True when the collection has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// True for collections whose items are IDs.
    pub fn id_items(&self) -> bool {
        match self {
            CollView::Ids { .. } => true,
            CollView::Rna { coll, .. } => coll.item_type().is_id,
        }
    }
    /// True when items can be looked up by name (`coll['Name']`).
    pub fn named(&self) -> bool {
        self.id_items() || self.item_type().name_property.is_some()
    }
    fn id_at(&self, i: usize) -> Option<IdAny> {
        match self {
            CollView::Ids { ids, .. } => ids.get(i).copied(),
            CollView::Rna { coll, .. } => match coll.item(i) {
                Ok(Item::Id(id)) => id,
                _ => None,
            },
        }
    }
    fn host(&self) -> &dyn HostRead {
        match self {
            CollView::Ids { main, .. } | CollView::Rna { main, .. } => *main,
        }
    }
    /// Name of item `i` (`None` for nameless items).
    pub fn key_of(&self, i: usize) -> Option<SmolStr> {
        if let CollView::Rna { coll, .. } = self
            && let Some(k) = coll.key_of(i)
        {
            return Some(k);
        }
        if self.id_items() {
            let id = self.id_at(i)?;
            return self.host().main().id_name(id).map(|(n, _)| n);
        }
        None
    }
    /// Index of the item named `key` (node sockets: the first enabled match).
    pub fn find(&self, key: &str) -> Option<usize> {
        if let CollView::Rna { coll, .. } = self
            && let Some(i) = coll.find(key)
        {
            return Some(i);
        }
        if self.id_items() {
            // Local IDs first, as `bpy_prop_collection[key]` does (§5.2).
            let n = self.len();
            let names: Vec<Option<(SmolStr, Option<SmolStr>)>> = (0..n)
                .map(|i| self.id_at(i).and_then(|id| self.host().main().id_name(id)))
                .collect();
            return names
                .iter()
                .position(|x| matches!(x, Some((name, None)) if name == key))
                .or_else(|| {
                    names
                        .iter()
                        .position(|x| matches!(x, Some((name, _)) if name == key))
                });
        }
        None
    }
    /// Item `i`.
    pub fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match self {
            CollView::Ids { ids, .. } => ids
                .get(i)
                .map(|id| Item::Id(Some(*id)))
                .ok_or_else(|| frap_rna::traits::index_out_of_range(i, ids.len())),
            CollView::Rna { coll, .. } => coll.item(i),
        }
    }
    /// Canonical path segment of item `i` (F-TR-5).
    pub fn item_seg(&self, i: usize) -> Seg {
        match self {
            CollView::Ids { .. } => Seg::Index(i as i64),
            CollView::Rna { coll, .. } => coll.item_seg(i),
        }
    }
    /// Bulk read (`foreach_get`, slices).
    pub fn foreach_get(
        &self,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        match self {
            CollView::Ids { main, ids, .. } => {
                let mut out = match prop.type_str() {
                    "FLOAT" => Bulk::F32(Vec::new()),
                    "INT" => Bulk::I32(Vec::new()),
                    "BOOLEAN" => Bulk::Bool(Vec::new()),
                    _ => return Err(bulk_type()),
                };
                for id in &ids[range] {
                    let r = main.main().id(*id).ok_or_else(bulk_type)?;
                    match r.get(prop)? {
                        Field::Value(v) => push_bulk(&mut out, &v)?,
                        _ => return Err(bulk_type()),
                    }
                }
                Ok(out)
            }
            CollView::Rna { coll, .. } => coll.foreach_get(prop, range),
        }
    }
}

fn bulk_type() -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        "foreach_get/foreach_set: property type does not match the buffer type",
    )
}

fn push_bulk(b: &mut Bulk, v: &frap_rna::Value) -> Result<(), RnaError> {
    use frap_rna::Value;
    match (b, v) {
        (Bulk::F32(b), Value::Float(f)) => b.push(*f),
        (Bulk::F32(b), Value::Double(f)) => b.push(*f as f32),
        (Bulk::F32(b), Value::FloatArray { data, .. }) => b.extend_from_slice(data),
        (Bulk::I32(b), Value::Int(i)) => b.push(*i as i32),
        (Bulk::I32(b), Value::IntArray(a)) => b.extend(a.iter().map(|i| *i as i32)),
        (Bulk::Bool(b), Value::Bool(x)) => b.push(*x),
        (Bulk::Bool(b), Value::BoolArray(a)) => b.extend_from_slice(a),
        _ => return Err(bulk_type()),
    }
    Ok(())
}

/// Runs `f` on the collection `prop` of the struct at `owner` (`bpy.data.<coll>` when the
/// owner is `bpy.data`).
pub fn with_coll<R>(
    host: &dyn HostRead,
    owner: &RnaPtr,
    prop: &'static PropDef,
    f: &mut dyn FnMut(&CollView) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    if owner.root == Root::Data && owner.path.is_empty() {
        let (ids, item_type) = host
            .main()
            .collection(prop.identifier)
            .ok_or_else(|| not_bound(&frap_spec_rna::BLEND_DATA, prop))?;
        return f(&CollView::Ids {
            ids,
            item_type,
            main: host,
        });
    }
    with_struct(
        host,
        owner,
        &mut |s| match get_field(host, s, owner, prop)? {
            Field::Coll(c) => f(&CollView::Rna {
                coll: c,
                main: host,
            }),
            _ => Err(stale(format!(
                "{}.{} is not a collection",
                owner.srna.identifier, prop.identifier
            ))),
        },
    )
}
