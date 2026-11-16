// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of the format layer. Callers (frap-io-gltf) map them onto Blender's messages.

/// Everything that can go wrong while reading, decoding or encoding glTF data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GltfError {
    /// The JSON is not a valid glTF document (syntax or a property of the wrong type).
    #[error("invalid glTF JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A typed extension could not be decoded from its JSON value.
    #[error("invalid {name} extension: {source}")]
    Extension {
        /// Extension name.
        name: String,
        /// Decoding error.
        source: serde_json::Error,
    },
    /// The GLB container is malformed.
    #[error("invalid GLB: {0}")]
    Glb(String),
    /// A reference points past the end of its array.
    #[error("{what} index {index} out of range (only {len})")]
    Index {
        /// What was referenced (`accessor`, `bufferView`, `buffer`, …).
        what: &'static str,
        /// The offending index.
        index: u64,
        /// The array length.
        len: usize,
    },
    /// Accessor or buffer-view bytes lie outside the data they reference.
    #[error("{0}")]
    OutOfBounds(String),
    /// Data that cannot be decoded or encoded as requested (wrong component type, bad stride,
    /// unsupported URI, …).
    #[error("{0}")]
    Data(String),
    /// A buffer view uses a compression extension this crate cannot decode.
    #[error("buffer view {view} is compressed with {extension}, which frap-gltf does not decode")]
    Compressed {
        /// Buffer view index.
        view: u32,
        /// Extension name.
        extension: String,
    },
    /// Loading an external resource failed (the caller's loader returned this).
    #[error("cannot load {uri:?}: {message}")]
    Load {
        /// The URI as written in the document.
        uri: String,
        /// Loader message.
        message: String,
    },
}
