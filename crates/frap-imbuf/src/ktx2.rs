// SPDX-License-Identifier: GPL-3.0-or-later
//! KTX2 container read (WP 7.3; M7 W0 stub, decision log M7W0-7): `KHR_texture_basisu`
//! images and `.ktx2` files decode to RGBA8 through the `ktx2` parser and the Basis Universal
//! transcoder ([`crate::basis`], feature `basisu`). [`crate::decode`] and [`crate::probe`]
//! dispatch here on the KTX2 identifier. Blender 5.2 cannot load KTX2 at all, so this is a
//! frap extension (deviations §2, WP 7.3).

use crate::codec::{DecodedImage, ImageError};
use crate::formats::ImageInfo;

/// The 12-byte KTX2 file identifier (`«KTX 20»\r\n\x1A\n`).
pub const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// Whether `bytes` start with the KTX2 identifier.
pub fn is_ktx2(bytes: &[u8]) -> bool {
    bytes.starts_with(&IDENTIFIER)
}

fn unsupported() -> ImageError {
    ImageError::Unsupported("KTX2 images are decoded from M7 (WP 7.3)".into())
}

/// Decodes the base mip level of a KTX2 file (WP 7.3). The stub answers `Unsupported`.
pub fn decode(_bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    Err(unsupported())
}

/// Reads a KTX2 header (WP 7.3). The stub answers `Unsupported`.
pub fn probe(_bytes: &[u8]) -> Result<ImageInfo, ImageError> {
    Err(unsupported())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier() {
        assert!(is_ktx2(&IDENTIFIER));
        assert!(!is_ktx2(b"\x89PNG\r\n\x1a\n"));
    }
}
