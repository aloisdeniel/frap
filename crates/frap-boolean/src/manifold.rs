// SPDX-License-Identifier: GPL-3.0-or-later
//! `manifold-rust` backend (feature `manifold`, WP 4.6): MANIFOLD at T2, EXACT at T3 until M8.
//! Stub until WP 4.6.

use crate::{BoolError, BoolMesh, BoolOptions, BoolResult, BooleanSolver, SolverKind};

/// The `manifold-rust` solver.
#[derive(Copy, Clone, Debug, Default)]
pub struct ManifoldSolver;

impl BooleanSolver for ManifoldSolver {
    fn name(&self) -> &'static str {
        "manifold-rust"
    }

    fn supports(&self, kind: SolverKind) -> bool {
        matches!(kind, SolverKind::Manifold | SolverKind::Exact)
    }

    fn run(&self, operands: &[BoolMesh<'_>], opts: &BoolOptions) -> Result<BoolResult, BoolError> {
        let _ = (operands, opts);
        #[cfg(not(feature = "manifold"))]
        return Err(BoolError::Unavailable(
            "frap: built without the boolean-manifold feature".into(),
        ));
        #[cfg(feature = "manifold")]
        todo!("WP 4.6: manifold-rust boolean")
    }
}
