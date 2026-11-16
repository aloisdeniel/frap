// SPDX-License-Identifier: GPL-3.0-or-later
//! TRIANGLES extractor: one primitive per used slot, looptris, corner split on
//! `(vertex index, SplitKey words)` byte-sorted unique, `corners`/`vertices` filled (M1, WP 1.16).
//! Today the M0 logic lives in `primitives.rs`.
