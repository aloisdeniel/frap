// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose-mode `TRANSFORM_MODE_HANDLERS` entry (WP 6.2, `crates/frap-ops-transform/src/pose/**`):
//! `transform.translate/rotate/resize` on the selected pose bones through the shared core
//! (orientation, pivot), writing `PoseBone.location` / rotation / `scale` in bone space as
//! `transform_convert_armature.cc` does. The dispatcher (`core/`) stays WP 1.21's; this module
//! registers the handler once WP 6.2 lands (M5+M6 W0 stub: no handler, so pose-mode transforms
//! answer `OP_NOT_IMPLEMENTED` naming the mode).
