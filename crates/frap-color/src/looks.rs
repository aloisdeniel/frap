// SPDX-License-Identifier: GPL-3.0-or-later
//! Looks (`view_settings.look`): which looks each view transform offers in Blender 5.2.4's
//! OCIO config, and how a look combines with the view transform (baked into the view LUTs, or
//! applied as a grading step before them). Owned by WP 5.3 (M5); `display::look_applies` keeps
//! the M1 rules until it lands.
