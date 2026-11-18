// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSubdiv backend over `opensubdiv-rs` (feature `osd`, off until the M4 gate, §3.6; WP 4.4).
//! Stub until WP 4.4.

use crate::{SubdivError, SubdivResult, SubdivSettings, SubdivTopology, Subdivider};

/// The `opensubdiv-rs` subdivider.
#[derive(Copy, Clone, Debug, Default)]
pub struct OpenSubdiv;

impl Subdivider for OpenSubdiv {
    fn name(&self) -> &'static str {
        "opensubdiv"
    }

    fn subdivide(
        &self,
        topo: &SubdivTopology<'_>,
        settings: &SubdivSettings,
    ) -> Result<SubdivResult, SubdivError> {
        let _ = (topo, settings);
        Err(SubdivError::Unavailable(
            "frap: the OpenSubdiv backend is not implemented yet (WP 4.4)".into(),
        ))
    }
}
