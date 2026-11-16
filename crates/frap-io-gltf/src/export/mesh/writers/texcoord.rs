// SPDX-License-Identifier: GPL-3.0-or-later
//! TEXCOORD_n writer, V flipped (M1, WP 1.16): key = `(u, 1 - v)` of every exported UV map as
//! f32 bits (2 words per map per corner); `run` fills `PrimitiveDraft::texcoords`.
