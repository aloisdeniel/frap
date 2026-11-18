// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench cavity (`shading.show_cavity`, `cavity_type`, ridge/valley factors) and depth of
//! field (`shading.use_dof`): the screen-space effects frap does not render yet (priority P2,
//! ARCHITECTURE §19.3; recorded in `docs/deviations.md`).
//!
//! Blender 5.2.4 (`workbench_state.cc`, `SceneState::init`) draws the SSAO cavity for
//! `cavity_type` WORLD or BOTH and the curvature for SCREEN or BOTH when `show_cavity` is on,
//! and depth of field when `use_dof` is on and the camera's `dof.use_dof` is set; X-ray mode
//! disables all three. frap renders such scenes without these effects; [`unsupported`] names
//! what a render leaves out, so callers can report it.

use frap_render_scene::RWorkbenchShading;

/// The Workbench effects a render with these settings asks for and frap leaves out (empty when
/// none): `"cavity"` (`show_cavity`) and `"depth of field"` (`use_dof`). In X-ray mode
/// (`xray_alpha < 1`) Blender disables both, so nothing is left out.
pub fn unsupported(w: &RWorkbenchShading, xray_alpha: f32) -> Vec<&'static str> {
    if xray_alpha != 1.0 {
        return vec![];
    }
    let mut out = vec![];
    if w.show_cavity {
        out.push("cavity");
    }
    if w.use_dof {
        out.push("depth of field");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_effects_left_out() {
        let mut w = RWorkbenchShading::default();
        assert!(unsupported(&w, 1.0).is_empty());
        w.show_cavity = true;
        w.use_dof = true;
        assert_eq!(unsupported(&w, 1.0), ["cavity", "depth of field"]);
        assert!(unsupported(&w, 0.5).is_empty(), "X-ray disables them");
    }
}
