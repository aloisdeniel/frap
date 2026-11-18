// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation-fingerprint cache (§7.3). Never iterated for output (rule 2.2.8).
//!
//! `EvalCache::object` resolves the object's fingerprint (`fingerprint.rs`), returns the cached
//! result while it is unchanged, and otherwise evaluates the stale ancestors parents first:
//! world matrix (`transform.rs`), geometry (`mesh.rs`) and local bounds. Evaluation only reads
//! the host state, so reads never change a fingerprint, and the cache lives outside the
//! session's snapshot: a rollback restores older generations, whose fingerprints simply miss.

mod bounds;
mod fingerprint;
mod mesh;
mod transform;

use std::sync::Arc;

use frap_rna::{IdKey, IdRef, ObjectId, Report, ReportType};
use frap_scene::{EvalError, EvalMode, EvaluatedObject, Evaluator, HostRead};
use rustc_hash::FxHashMap;

pub use bounds::{bound_box, dimensions, scale_for_dimensions};
pub use transform::{matrix_basis, object_transform, parent_matrix, world_matrix};

use fingerprint::{Cycle, FpCx};

#[derive(Clone, Debug)]
struct CacheEntry {
    fingerprint: u64,
    result: Arc<EvaluatedObject>,
}

/// The evaluator of a session (or of a render/export snapshot).
#[derive(Clone, Debug, Default)]
pub struct EvalCache {
    objects: FxHashMap<(IdKey, EvalMode), CacheEntry>,
    /// Objects whose modifier stacks are being evaluated (operand cycle guard, `geometry.rs`).
    pub(crate) in_progress: crate::geometry::InProgress,
}

/// `Dependency cycle detected` report of a parent cycle, worded like Blender's depsgraph log.
fn cycle_report(host: &dyn HostRead, cycle: &Cycle) -> Report {
    let objects = &host.main().objects;
    let name = |ob: IdRef<ObjectId>| objects.name_of(ob).unwrap_or("").to_owned();
    let mut msg = format!(
        "Dependency cycle detected:\n  OB{}/TRANSFORM_PARENT() depends on",
        name(cycle[0])
    );
    for &m in cycle.iter().skip(1).chain(std::iter::once(&cycle[0])) {
        msg.push_str(&format!("\n  OB{}/TRANSFORM_FINAL() via 'Parent'", name(m)));
    }
    Report::new(ReportType::Warning, msg)
}

fn missing(ob: IdRef<ObjectId>) -> EvalError {
    EvalError {
        message: format!("frap: object {:?} does not exist", ob.key()),
    }
}

impl EvalCache {
    /// Empty cache.
    pub fn new() -> Self {
        EvalCache::default()
    }

    /// The current fingerprint of `ob` in `mode` (`None` if the object does not exist).
    /// Computing it only reads `host`.
    pub fn fingerprint(host: &dyn HostRead, ob: IdRef<ObjectId>, mode: EvalMode) -> Option<u64> {
        FpCx::new(host, mode).node(ob).map(|n| n.fp)
    }

    /// Number of cached results (tests and diagnostics).
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// True when nothing is cached.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Drops every cached result.
    pub fn clear(&mut self) {
        self.objects.clear();
    }

    /// The cached result of `(ob, mode)` when it is still valid.
    fn hit(&self, ob: IdRef<ObjectId>, mode: EvalMode, fp: u64) -> Option<Arc<EvaluatedObject>> {
        self.objects
            .get(&(ob.key(), mode))
            .filter(|e| e.fingerprint == fp)
            .map(|e| e.result.clone())
    }

    fn eval(
        &mut self,
        cx: &mut FpCx<'_>,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError> {
        // The stale part of the ancestor chain, child first.
        let mut stale = Vec::new();
        let mut parent_result = None;
        let mut cur = Some(ob);
        while let Some(c) = cur {
            let node = cx.node(c).ok_or_else(|| missing(c))?;
            if let Some(r) = self.hit(c, mode, node.fp) {
                parent_result = Some(r);
                break;
            }
            let next = node.parent;
            stale.push((c, node.clone()));
            cur = next;
        }
        // `parent_result` belongs to the first fresh ancestor, which is the parent of the last
        // stale object; with no stale object it is `ob`'s own cached result.
        if stale.is_empty() {
            return parent_result.ok_or_else(|| missing(ob));
        }
        for (c, node) in stale.into_iter().rev() {
            let o = host.main().objects.get(c).ok_or_else(|| missing(c))?;
            let parent = parent_result.as_deref().filter(|_| node.parent.is_some());
            let matrix_world = match (node.parent, parent) {
                (Some(_), Some(p)) => {
                    let pm = parent_matrix(o, &p.matrix_world, p.mesh.as_deref());
                    world_matrix(o, Some(&pm))
                }
                // The member of a cycle evaluated first: its parent is not evaluated yet.
                (None, _) if node.cycle.is_some() && o.parent.is_some() => {
                    world_matrix(o, Some(&glam::Mat4::IDENTITY))
                }
                _ => world_matrix(o, None),
            };
            let geo = mesh::evaluate(self, host, c, o, mode);
            let bounds = geo.mesh.as_ref().and_then(|m| m.bounds());
            let mut warnings = geo.warnings;
            if let Some(cycle) = &node.cycle {
                warnings.insert(0, cycle_report(host, cycle));
            }
            let result = Arc::new(EvaluatedObject {
                matrix_world,
                mesh: geo.mesh,
                bounds,
                pose: None, // M6 (`pose.rs`)
                modifier_status: geo.modifier_status,
                warnings,
            });
            self.objects.insert(
                (c.key(), mode),
                CacheEntry {
                    fingerprint: node.fp,
                    result: result.clone(),
                },
            );
            parent_result = Some(result);
        }
        parent_result.ok_or_else(|| missing(ob))
    }

    /// Forgets results of objects that no longer exist.
    fn prune(&mut self, host: &dyn HostRead) {
        let objects = &host.main().objects;
        self.objects
            .retain(|(k, _), _| objects.get(IdRef::<ObjectId>::from_key(*k)).is_some());
    }
}

impl Evaluator for EvalCache {
    fn object(
        &mut self,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError> {
        let mut cx = FpCx::new(host, mode);
        self.eval(&mut cx, host, ob, mode)
    }

    /// `view_layer.update()` / `evaluated_depsgraph_get()`: evaluates every object in
    /// `bpy.data` order (viewport) and drops the results of removed objects. With always-fresh
    /// reads this only warms the cache; it never changes what a read returns.
    fn depsgraph_update(&mut self, host: &dyn HostRead) {
        let mut cx = FpCx::new(host, EvalMode::Viewport);
        let obs: Vec<_> = host.main().objects.iter().map(|(r, _)| r).collect();
        for ob in obs {
            // Every listed object exists, so evaluation cannot fail.
            let _ = self.eval(&mut cx, host, ob, EvalMode::Viewport);
        }
        self.prune(host);
    }
}
