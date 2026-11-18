// SPDX-License-Identifier: GPL-3.0-or-later
//! Renders show modifiers (M4, WP 4.7; stub created by M4 W0): the builder already extracts
//! each object's `EvalMode::Render` evaluated mesh (`mod.rs`), so the modifier stack reaches
//! the render through `frap-eval`. WP 4.7 owns this file for what is specific to evaluated
//! geometry: render levels (`render_levels`, `show_render`), material indices created by
//! modifiers beyond the object's slots, and the mesh-key sharing of linked duplicates whose
//! evaluated meshes differ.
