// SPDX-License-Identifier: GPL-3.0-or-later
//! Lazy decoding of the catalog blob into leaked `&'static` tables (WP 1.1).

use frap_rna::OperatorSpec;

/// Every catalog operator, decoded once per process. Empty until WP 1.1 generates the blob.
pub fn catalog() -> &'static [&'static OperatorSpec] {
    &[]
}
