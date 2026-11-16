// SPDX-License-Identifier: GPL-3.0-or-later
//! Schema migrations, one file per step (`v{N}_to_v{N+1}.rs`), on `serde_json::Value` before
//! typed deserialization; newer versions are refused (§15). WP 1.9.

/// Migration steps (automod: adding a step needs no `mod` edit).
pub mod steps {
    automod::dir!(pub "src/migrate/steps");
}
