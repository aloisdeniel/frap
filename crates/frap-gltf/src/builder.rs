// SPDX-License-Identifier: GPL-3.0-or-later
//! `BufferBuilder`: aligned buffer views, interleave option (§17.1). Named by the frozen
//! `PRIMITIVE_POST_PROCESS` contract (frap-io-gltf, §17.2). WP 1.5.

/// Accumulates the BIN chunk and its buffer views.
#[derive(Clone, Debug, Default)]
pub struct BufferBuilder {
    /// BIN chunk bytes so far.
    pub bytes: Vec<u8>,
}
