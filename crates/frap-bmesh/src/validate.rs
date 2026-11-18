// SPDX-License-Identifier: GPL-3.0-or-later
//! Structural validation (`BM_mesh_validate`; ARCHITECTURE §6.2): disk and radial cycle
//! consistency, face loop cycles, pool bookkeeping. Used by the proptests of every op (random
//! kernel and op sequences keep `validate` green). CONTRACT frozen by M2 W0, implemented by
//! WP 2.1.

use crate::types::BMesh;

impl BMesh {
    /// Every inconsistency found, as messages (empty when valid).
    pub fn validate(&self) -> Vec<String> {
        todo!("WP 2.1: BM_mesh_validate")
    }
}
