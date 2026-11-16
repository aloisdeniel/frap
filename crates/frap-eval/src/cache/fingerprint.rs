// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation fingerprints and parent relations (§7.3).
//!
//! The fingerprint of an object hashes its own `gen`, its data's `gen`, its shape-key `gen`,
//! the edit BMesh generation (Edit Mode), the context scene's frame and, recursively, the
//! fingerprint of its parent. Every write bumps a `gen` (§5.3), so a cached result is valid
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

use frap_rna::{IdKey, IdRef, ObjectId};
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
            let node = Node {
                fp: self.hash(c, parent, None),
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
            let node = Node {
                fp: self.hash(c, parent, Some(&cycle)),
                parent,
                cycle: Some(cycle.clone()),
            };
            self.memo.insert(c.key(), node);
        }
    }

    fn hash(
        &self,
        ob: IdRef<ObjectId>,
        parent: Option<IdRef<ObjectId>>,
        cycle: Option<&Cycle>,
    ) -> u64 {
        let main = self.host.main();
        let o = self.object(ob).expect("invariant: resolved objects exist");
        let mut h = FxHasher::default();
        ob.key().hash(&mut h);
        o.header.generation.hash(&mut h);
        self.mode.hash(&mut h);
        self.frame.hash(&mut h);
        match o.data.and_then(|d| main.header(d).map(|hd| (d, hd))) {
            Some((d, hd)) => {
                d.hash(&mut h);
                hd.generation.hash(&mut h);
                if let Some(me) = d.typed().and_then(|m| main.meshes.get(m)) {
                    // Shape keys and the edit BMesh of the mesh.
                    match me.shape_keys.and_then(|k| main.shape_keys.get(k)) {
                        Some(k) => (1u8, k.header.generation).hash(&mut h),
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
