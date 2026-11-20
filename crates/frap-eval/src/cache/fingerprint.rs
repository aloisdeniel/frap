// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation fingerprints and parent relations (§7.3).
//!
//! The fingerprint of an object hashes its own `gen`, its data's `gen`, its shape-key `gen`,
//! the edit BMesh generation (Edit Mode), the context scene's frame and, recursively, the
//! fingerprint of its parent and of every `depends_on` target of its modifiers (WP 4.1):
//! operand objects (their fingerprints, so a moved or edited operand invalidates), the objects
//! of operand collections, and the `gen` of other IDs (node groups, textures and their images).
//! A dependency cycle through modifiers (A's Boolean uses B, B's uses A) is cut: the member
//! being resolved contributes only its own and its data's `gen`. Every write bumps a `gen` (§5.3), so a cached result is valid
//! exactly while its fingerprint is unchanged; computing one only reads.
//!
//! Parent cycles cannot be created through RNA (Blender's `Object.parent` setter refuses them,
//! verified on 5.2.4), but a loaded or Rust-built state may hold one. A cycle is reported as
//! Blender's depsgraph does (`Dependency cycle detected`) and evaluated in `bpy.data` order:
//! the member that comes first in `bpy.data.objects` is evaluated first, before its parent, so
//! its parent link contributes the identity (a never-evaluated object's `matrix_world`).
//! Everything is computed iteratively, so deep hierarchies do not grow the stack.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use frap_modifiers::{ModifierType, registry};
use frap_rna::{IdAny, IdCode, IdKey, IdRef, IdType, ObjectId};
use frap_scene::id::Object;
use frap_scene::{EvalMode, HostRead};
use rustc_hash::{FxHashMap, FxHasher};

/// A parent cycle: its members in parent order, starting with the member evaluated first.
pub(crate) type Cycle = Arc<[IdRef<ObjectId>]>;

/// The resolved relations and fingerprint of one object.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    /// Fingerprint (§7.3).
    pub fp: u64,
    /// The parent whose evaluation this object waits for (`None` at a root and at the member
    /// of a cycle that is evaluated first).
    pub parent: Option<IdRef<ObjectId>>,
    /// The cycle this object is a member of.
    pub cycle: Option<Cycle>,
}

/// Fingerprints of one host state, memoized for the duration of one evaluator call.
pub(crate) struct FpCx<'h> {
    host: &'h dyn HostRead,
    mode: EvalMode,
    frame: i32,
    memo: FxHashMap<IdKey, Node>,
    /// Objects whose dependency hash is being computed (modifier dependency cycle guard).
    resolving: Vec<IdKey>,
}

impl<'h> FpCx<'h> {
    pub fn new(host: &'h dyn HostRead, mode: EvalMode) -> Self {
        let frame = host
            .main()
            .scenes
            .get(host.win().scene)
            .map_or(0, |s| s.frame_current);
        FpCx {
            host,
            mode,
            frame,
            memo: FxHashMap::default(),
            resolving: Vec::new(),
        }
    }

    fn object(&self, ob: IdRef<ObjectId>) -> Option<&'h Object> {
        self.host.main().objects.get(ob)
    }

    /// `ob.parent` when it exists (a dangling reference reads as no parent).
    fn parent_of(&self, ob: IdRef<ObjectId>) -> Option<IdRef<ObjectId>> {
        let p = self.object(ob)?.parent?;
        self.object(p).is_some().then_some(p)
    }

    /// The node of `ob` (`None` when the object does not exist).
    pub fn node(&mut self, ob: IdRef<ObjectId>) -> Option<&Node> {
        self.object(ob)?;
        if !self.memo.contains_key(&ob.key()) {
            self.resolve(ob);
        }
        self.memo.get(&ob.key())
    }

    /// Resolves `ob` and every unresolved ancestor, parents first.
    fn resolve(&mut self, ob: IdRef<ObjectId>) {
        // Walk up until a resolved object, a root, or a repeat (a cycle).
        let mut chain: Vec<IdRef<ObjectId>> = Vec::new();
        let mut cycle_start = None;
        let mut cur = Some(ob);
        while let Some(c) = cur {
            if self.memo.contains_key(&c.key()) {
                break;
            }
            if let Some(i) = chain.iter().position(|&x| x == c) {
                cycle_start = Some(i);
                break;
            }
            chain.push(c);
            cur = self.parent_of(c);
        }
        let mut prefix_len = chain.len();
        if let Some(start) = cycle_start {
            self.resolve_cycle(&chain[start..]);
            prefix_len = start;
        }
        // The acyclic part, parents first: chain[i]'s parent is chain[i + 1] (or resolved).
        for &c in chain[..prefix_len].iter().rev() {
            let parent = self.parent_of(c);
            let deps = self.deps_hash(c);
            let node = Node {
                fp: self.hash(c, parent, None, deps),
                parent,
                cycle: None,
            };
            self.memo.insert(c.key(), node);
        }
    }

    /// Resolves a cycle given in parent order (`members[i].parent == members[i + 1]`, the last
    /// member's parent is the first).
    fn resolve_cycle(&mut self, members: &[IdRef<ObjectId>]) {
        let objects = &self.host.main().objects;
        let first = (0..members.len())
            .min_by_key(|&i| objects.index_of(members[i]).unwrap_or(usize::MAX))
            .expect("invariant: a cycle has members");
        // Parent order starting at the member evaluated first.
        let cycle: Cycle = (0..members.len())
            .map(|k| members[(first + k) % members.len()])
            .collect();
        // Evaluation order: the first member, then each member whose parent was just done
        // (the reverse of parent order).
        let order = std::iter::once(cycle[0]).chain(cycle[1..].iter().rev().copied());
        for (k, c) in order.enumerate() {
            let parent = if k == 0 { None } else { self.parent_of(c) };
            let deps = self.deps_hash(c);
            let node = Node {
                fp: self.hash(c, parent, Some(&cycle), deps),
                parent,
                cycle: Some(cycle.clone()),
            };
            self.memo.insert(c.key(), node);
        }
    }

    /// The IDs the modifiers of `ob` depend on (`ModifierTypeInfo::depends_on` of the types
    /// with an evaluator), in stack order.
    fn deps_of(&self, ob: IdRef<ObjectId>) -> Vec<IdAny> {
        let Some(o) = self.object(ob) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for m in &o.modifiers {
            let info = ModifierType::from_identifier(m.settings.type_identifier())
                .and_then(registry::find);
            if let Some(info) = info {
                out.extend((info.depends_on)(&m.settings));
            }
        }
        out
    }

    /// Own and data `gen` of an object (the shallow hash of a dependency-cycle member).
    fn shallow(&self, ob: IdRef<ObjectId>, h: &mut FxHasher) {
        let main = self.host.main();
        match self.object(ob) {
            Some(o) => {
                o.header.generation.hash(h);
                o.data
                    .and_then(|d| main.header(d))
                    .map_or(0, |hd| hd.generation)
                    .hash(h);
            }
            None => 0u8.hash(h),
        }
    }

    /// The hash of an object dependency: its fingerprint, or its shallow hash inside a cycle.
    fn object_dep(&mut self, ob: IdRef<ObjectId>, h: &mut FxHasher) {
        if self.resolving.contains(&ob.key()) {
            self.shallow(ob, h);
            return;
        }
        match self.node(ob).map(|n| n.fp) {
            Some(fp) => (1u8, fp).hash(h),
            None => 0u8.hash(h),
        }
    }

    /// The hash of every `depends_on` target of `ob`'s modifiers (module docs).
    fn deps_hash(&mut self, ob: IdRef<ObjectId>) -> u64 {
        let deps = self.deps_of(ob);
        // Empty, or `ob` is already being resolved further up (a dependency cycle).
        if deps.is_empty() || self.resolving.contains(&ob.key()) {
            return 0;
        }
        self.resolving.push(ob.key());
        let mut h = FxHasher::default();
        for d in deps {
            d.hash(&mut h);
            let main = self.host.main();
            match d.code {
                IdCode::OB => match IdRef::<ObjectId>::from_any(d) {
                    Some(o) => self.object_dep(o, &mut h),
                    None => 0u8.hash(&mut h),
                },
                IdCode::GR => {
                    // The collection and, recursively, the objects of it and its children.
                    let mut objects = Vec::new();
                    let mut stack = vec![d];
                    let mut seen = Vec::new();
                    while let Some(c) = stack.pop() {
                        if seen.contains(&c) {
                            continue;
                        }
                        seen.push(c);
                        let Some(col) = c.typed().and_then(|r| main.collections.get(r)) else {
                            continue;
                        };
                        col.header.generation.hash(&mut h);
                        objects.extend(col.objects.iter().copied());
                        stack.extend(col.children.iter().rev().map(|r| r.any()));
                    }
                    for o in objects {
                        self.object_dep(o, &mut h);
                    }
                }
                _ => {
                    main.header(d).map_or(0, |hd| hd.generation).hash(&mut h);
                    // A texture's image (DISPLACE): pixels change the image's `gen`.
                    if let Some(t) = d.typed().and_then(|r| main.textures.get(r)) {
                        t.for_each_ref(&mut |r| {
                            main.header(r).map_or(0, |hd| hd.generation).hash(&mut h);
                        });
                    }
                }
            }
        }
        self.resolving.retain(|k| *k != ob.key());
        h.finish()
    }

    fn hash(
        &self,
        ob: IdRef<ObjectId>,
        parent: Option<IdRef<ObjectId>>,
        cycle: Option<&Cycle>,
        deps: u64,
    ) -> u64 {
        let main = self.host.main();
        let o = self.object(ob).expect("invariant: resolved objects exist");
        let mut h = FxHasher::default();
        deps.hash(&mut h);
        ob.key().hash(&mut h);
        o.header.generation.hash(&mut h);
        self.mode.hash(&mut h);
        self.frame.hash(&mut h);
        match o.data.and_then(|d| main.header(d).map(|hd| (d, hd))) {
            Some((d, hd)) => {
                d.hash(&mut h);
                hd.generation.hash(&mut h);
                if let Some(me) = d.typed().and_then(|m| main.meshes.get(m)) {
                    // Shape keys (the `Key` ID and its generation: block data, values,
                    // relative keys, vertex-group names, WP 6.3) and the edit BMesh of the
                    // mesh.
                    match me
                        .shape_keys
                        .and_then(|k| Some((k, main.shape_keys.get(k)?)))
                    {
                        Some((r, k)) => (1u8, r.key(), k.header.generation).hash(&mut h),
                        None => 0u8.hash(&mut h),
                    }
                    match self.host.edit().meshes.get(&d.key) {
                        Some(em) => (1u8, em.bm_gen).hash(&mut h),
                        None => 0u8.hash(&mut h),
                    }
                }
            }
            None => 0u8.hash(&mut h),
        }
        match parent.and_then(|p| self.memo.get(&p.key())) {
            Some(p) => (1u8, p.fp).hash(&mut h),
            None => 0u8.hash(&mut h),
        }
        // A cycle member depends on every member's relations (any of them can break it).
        if let Some(cycle) = cycle {
            for m in cycle.iter() {
                m.key().hash(&mut h);
                self.object(*m)
                    .map_or(0, |x| x.header.generation)
                    .hash(&mut h);
            }
        }
        h.finish()
    }
}
