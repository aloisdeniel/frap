// SPDX-License-Identifier: GPL-3.0-or-later
//! Codec errors (CONTRACT, M7 W0).

/// Why a codec call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CodecError {
    /// The backend is not part of this build (its cargo feature is off).
    #[error("{codec} is not available: frap was built without the `{feature}` feature")]
    Unavailable {
        /// Codec name (`Draco`, `meshopt`).
        codec: &'static str,
        /// The cargo feature that provides it.
        feature: &'static str,
    },
    /// The input cannot be encoded (unsupported attribute layout, too many vertices, …).
    #[error("{0}")]
    Encode(String),
    /// Corrupt or truncated compressed data, or data that does not match the declared layout.
    #[error("{0}")]
    Decode(String),
    /// Not implemented yet (a W0 stub of a W1 work package).
    #[error("{0}")]
    NotImplemented(String),
}
