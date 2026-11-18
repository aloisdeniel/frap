// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.describe(target=…)` reaches the `--describe` manifest through this slice
//! (M2 W0 contract delta). The manifest lives in `frap-schema` (L11), above the operator
//! crates, so `frap-schema` registers the provider and the operator (`frap-ops-frap`, L10)
//! calls it: dependency inversion through `linkme`, like `MODE_HANDLERS`. WP 2.13 implements
//! both sides (`crates/frap-schema/src/provider.rs`, `crates/frap-ops-frap/src/ops/describe.rs`).

use frap_rna::FrapError;

/// The `--describe` manifest, as a callable.
#[derive(Debug)]
pub struct DescribeProvider {
    /// The manifest fragment of `target` (`op:mesh.bevel`, `mesh.bevel`, `struct:Object`,
    /// `Object.location`, `errors`, …), or `DESCRIBE_TARGET` (`KeyError`, `did_you_mean`).
    pub describe: fn(target: &str) -> Result<serde_json::Value, FrapError>,
}

/// The registered provider (`frap-schema`; at most one).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static DESCRIBE_PROVIDERS: [DescribeProvider];

/// The registered provider, if the binary links `frap-schema`.
pub fn describe_provider() -> Option<&'static DescribeProvider> {
    DESCRIBE_PROVIDERS.first()
}
