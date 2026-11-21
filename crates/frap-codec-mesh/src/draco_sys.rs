// SPDX-License-Identifier: GPL-3.0-or-later
//! The `frap-draco-sys` backend (feature `draco-sys`, WP 7.2).
//!
//! Spike gate (§22.9): draco-core passed (byte-identical to Blender's Google Draco 1.5.7 on the
//! add-on's inputs, decodes Blender's files; tests in `draco_core.rs` and the `gltf/draco`
//! goldens), so the cc-built fallback is not filled: `frap_draco_sys::AVAILABLE` is `false` and
//! this backend answers `None` even with the feature on. `frap_draco_backend='SYS'` then fails
//! with [`crate::DracoBackend::unavailable`]'s message.

use crate::draco::DracoCodec;

/// Whether this build enables the `draco-sys` feature (the fallback may still be empty).
pub const BUILT_WITH_FEATURE: bool = cfg!(feature = "draco-sys");

/// The Google Draco codec: `None` (the spike gate passed, see the module docs).
pub fn codec() -> Option<&'static dyn DracoCodec> {
    #[cfg(feature = "draco-sys")]
    {
        // The stub crate carries no encoder; a filled one would set `AVAILABLE` and export
        // the codec here.
        const { assert!(!frap_draco_sys::AVAILABLE) };
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_fallback_is_not_built() {
        assert!(super::codec().is_none());
        #[cfg(feature = "draco-sys")]
        const {
            assert!(!frap_draco_sys::AVAILABLE)
        };
    }
}
