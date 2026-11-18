// SPDX-License-Identifier: GPL-3.0-or-later
//! Contact-sheet compositor (`frap.render_contact_sheet`, ARCHITECTURE §19.6): tiles placed by
//! a `frap_render_scene::SheetLayout` on the CPU, labels drawn with the embedded bitmap font of
//! `assets/font/`. Reached through `RenderService::composite_sheet`.
//!
//! **Stub** created by M2 W0 (WP 2.0); implemented by WP 2.3 (Workbench completion and contact
//! sheet, §22.4), which also implements `composite_sheet` for `Renderer`.
