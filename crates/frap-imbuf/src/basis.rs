// SPDX-License-Identifier: GPL-3.0-or-later
//! Basis Universal (WP 7.3; M7 W0 stub, decision log M7W0-7): transcoding of ETC1S / UASTC
//! payloads to RGBA8 (the pure-Rust `basisu` crate, feature `basisu`) for [`crate::ktx2`], and
//! encoding (`basisu_c_sys`, feature `basisu-encode`, off by default: it needs libclang) for
//! `frap_texture_format='KTX2'`.

use crate::codec::{DecodedImage, ImageError};

/// `frap_basisu_mode`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BasisMode {
    /// `ETC1S`: smaller files, lower quality.
    #[default]
    Etc1s,
    /// `UASTC`: larger files, higher quality.
    Uastc,
}

/// Whether this build can encode Basis Universal KTX2 files (feature `basisu-encode`).
pub const CAN_ENCODE: bool = cfg!(feature = "basisu-encode");

/// Encodes `image` as a KTX2 file with Basis Universal supercompression (WP 7.3). The stub
/// answers `Unsupported`.
pub fn encode_ktx2(_image: &DecodedImage, _mode: BasisMode) -> Result<Vec<u8>, ImageError> {
    Err(ImageError::Unsupported(if CAN_ENCODE {
        "KTX2 encoding arrives in M7 (WP 7.3)".into()
    } else {
        "KTX2 encoding needs frap built with the `basisu-encode` feature".into()
    }))
}
