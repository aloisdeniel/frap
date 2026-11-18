// SPDX-License-Identifier: GPL-3.0-or-later
//! Placeholder while the complex mode is ported (not committed in this state).
use frap_mesh::Mesh;
use frap_scene::generated::modifier_settings::SolidifySettings;

pub(crate) fn solidify_nonmanifold(s: &SolidifySettings, mesh: &Mesh, totcol: usize) -> Mesh {
    super::extrude::solidify_extrude(s, mesh, totcol)
}
