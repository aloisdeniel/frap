// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit-mode transforms (`TRANSFORM_MODE_HANDLERS` entry for `EDIT_MESH`; ARCHITECTURE §10.3,
//! §22.4): `transform.translate/rotate/resize` on the selected elements of every mesh in Edit
//! Mode, in world space, with the shared orientation/pivot/proportional core (`crate::core`;
//! pivots incl. `ACTIVE_ELEMENT` and `INDIVIDUAL_ORIGINS`, `orient_type='NORMAL'`, proportional
//! falloffs), plus the element data the edit-only transform operators (`shrink_fatten`,
//! `edge_slide`, `vert_slide`, `tosphere`, `edge_crease`, `edge_bevelweight`, `vert_crease`,
//! `push_pull`, `mirror`, `vertex_random`, `transform.transform`) share.
//!
//! **Stub** created by M2 W0 (WP 2.0); implemented by WP 2.12 (Edit-mode transforms), which
//! owns `src/edit/**` and adds submodules from this file. The dispatchers (`core::dispatch`,
//! `ops/{translate,rotate,resize}.rs`) stay with WP 1.21: the handler is found through the
//! `TRANSFORM_MODE_HANDLERS` slice.
