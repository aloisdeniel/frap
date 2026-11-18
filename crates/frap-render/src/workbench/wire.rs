// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench `shading.type = 'WIREFRAME'` and `Object.display_type` in final renders.
//!
//! Wireframes are drawn by Blender's overlay engine, which final renders do not run. What the
//! Workbench render engine itself does (Blender 5.2.4, GPL-2.0-or-later, verified with F12
//! renders of the factory scene):
//!
//! - `workbench_state.cc` (`SceneState::init`): a shading type below SOLID (WIREFRAME) forces
//!   `light = FLAT`, `color_type = OBJECT` and an X-ray alpha of 0, so every surface goes to the
//!   transparent passes with zero alpha: the render shows the background only. The object ids
//!   are still written by the transparent depth pass, so `show_object_outline` draws the
//!   objects' outlines over the background (see [`super::outline`]).
//! - `workbench_engine.cc` (`object_sync`): in a scene render, objects draw their surface
//!   whatever their `display_type` (`ob->dt >= OB_SOLID || is_scene_render()`), so `WIRE` and
//!   `BOUNDS` objects render solid.

/// Whether `shading.type` is below SOLID (`OB_WIRE`).
pub fn is_wireframe(shading_type: &str) -> bool {
    shading_type == "WIREFRAME"
}

/// Whether an object with this `display_type` draws its surface in a final render (always:
/// `is_scene_render()`).
pub fn draws_surface(display_type: &str) -> bool {
    let _ = display_type;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wireframe_types() {
        assert!(is_wireframe("WIREFRAME"));
        for t in ["SOLID", "MATERIAL", "RENDERED"] {
            assert!(!is_wireframe(t));
        }
        for d in ["TEXTURED", "SOLID", "WIRE", "BOUNDS"] {
            assert!(draws_surface(d));
        }
    }
}
