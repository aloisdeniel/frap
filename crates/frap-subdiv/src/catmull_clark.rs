// SPDX-License-Identifier: GPL-3.0-or-later
//! frap's own Catmull-Clark backend (WP 4.4): uniform refinement with Blender's crease rules
//! (`use_creases`), boundary interpolation and face-varying UVs, matching what Blender computes
//! through OpenSubdiv (`subdiv_mesh.cc`, `subdiv_eval.cc`). Stub until WP 4.4.

use crate::{SubdivError, SubdivResult, SubdivSettings, SubdivTopology, Subdivider};

/// frap's Catmull-Clark subdivider.
#[derive(Copy, Clone, Debug, Default)]
pub struct CatmullClark;

impl Subdivider for CatmullClark {
    fn name(&self) -> &'static str {
        "frap"
    }

    fn subdivide(
        &self,
        topo: &SubdivTopology<'_>,
        settings: &SubdivSettings,
    ) -> Result<SubdivResult, SubdivError> {
        let _ = (topo, settings);
        todo!("WP 4.4: Catmull-Clark subdivision")
    }
}
