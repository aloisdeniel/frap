// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation-fingerprint cache (§7.3). Never iterated for output (rule 2.2.8).

use std::sync::Arc;

use frap_rna::{IdKey, IdRef, ObjectId};
use frap_scene::{EvalError, EvalMode, EvaluatedObject, Evaluator, HostRead};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug)]
#[allow(dead_code)] // filled by WP 1.13
struct CacheEntry {
    fingerprint: u64,
    result: Arc<EvaluatedObject>,
}

/// The evaluator of a session (or of a render/export snapshot).
#[derive(Clone, Debug, Default)]
pub struct EvalCache {
    objects: FxHashMap<(IdKey, EvalMode), CacheEntry>,
}

impl EvalCache {
    /// Empty cache.
    pub fn new() -> Self {
        EvalCache::default()
    }
}

impl Evaluator for EvalCache {
    fn object(
        &mut self,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError> {
        let _ = (host, ob, mode, &self.objects);
        todo!("WP 1.13: fingerprint, world matrix, bounds, mesh pipeline")
    }
    fn depsgraph_update(&mut self, host: &dyn HostRead) {
        let _ = host;
        todo!("WP 1.13")
    }
}
