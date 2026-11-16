// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-io-tests`: a test-only crate (ARCHITECTURE rule 2.2.11, §3.1). Behaviour tests of
//! `frap-ops-io` (`wm.save_as_mainfile`/`open_mainfile`, `export_scene.gltf`,
//! `import_scene.gltf`) that need a full `Session` live in `tests/*.rs` here, one file per
//! topic. Owned by WP 1.23 in M1. This library is intentionally empty.
#![deny(unsafe_code)]
