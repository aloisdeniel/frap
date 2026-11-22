// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_gltf::glb::read` (GLB or glTF JSON), the accessor and primitive readers and the validator on arbitrary bytes: never panic, errors only (ARCHITECTURE §20.1, nightly from M8).
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.7 owns this file and writes the target.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data;
});
