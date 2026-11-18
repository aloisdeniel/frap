// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench: MATCAP lighting (`display.shading.light = 'MATCAP'`, `studio_light` names the matcap; the matcaps of `assets/matcaps/` baked from Blender's `datafiles/studiolights/matcap`, provenance recorded) and the matcap SSAO of `display.matcap_ssao_*`.
//!
//! **Stub** created by M2 W0 (WP 2.0); implemented by WP 2.3 (Workbench completion, ARCHITECTURE
//! §19.3, §22.4). Feature switches are uniform flags of the one Workbench uber-shader, never
//! pipeline permutations (§19.2). Inputs: `RSettings::workbench` (`RWorkbenchShading`).
