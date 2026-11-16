// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[serde(with = "frap_rna::serde_buf")]` for `Arc<Vec<T>>` fields (§15): goes through a
//! scoped `BufferSink`/`BufferSource` (thread-local during `.frap` save/load), so data crates
//! never depend on `frap-project`. WP 1.2.
