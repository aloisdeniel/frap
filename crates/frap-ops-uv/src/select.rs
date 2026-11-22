// SPDX-License-Identifier: GPL-3.0-or-later
//! UV selection, to be ported from `blender/source/blender/editors/uvedit/uvedit_select.cc`
//! (Blender 5.2.4): the per-corner `.uv_select_vert` / `.uv_select_edge` / `.uv_select_face`
//! attributes, `ED_uvedit_select_sync_flush`, the sticky modes (`uv_sticky_select_mode`,
//! factory `SHARED_LOCATION`) and the UV select mode (`uv_select_mode`), and
//! `use_uv_select_sync` (deviation D-111: `uv_select_sync_valid`), for the `uv.select_*`,
//! `uv.hide` / `uv.reveal` operators (WP 8.3.8) and the UV editing operators that act on the
//! UV selection (`uv.weld`, `stitch`, `align`, … WP 8.3.9).
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.3.8 owns this file (and `select/**`).
