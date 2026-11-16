// SPDX-License-Identifier: GPL-3.0-or-later
//! One generator per primitive (ARCHITECTURE §6.1, rule 2.2.1): `primitives/<name>.rs`, included
//! with `automod`, so adding a generator needs no `mod` edit. Callers name the file module
//! (`primitives::uv_sphere::uv_sphere(…)`); the M0 re-export of `cube` below stays for the
//! existing call sites.

automod::dir!(pub "src/primitives");

pub use cube::{UV_MAP_NAME, cube};
