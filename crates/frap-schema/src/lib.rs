// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-schema` (layer L11): `frap --describe`: Blender-RNA-shaped manifest, JSON Schema 2020-12,
//! LLM tool formats (ARCHITECTURE §14). Built from the `frap-spec-*` tables plus the registry,
//! so it cannot drift; `schemars` only for the static parts (envelope, errors, `.frap`).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.18.
#![deny(unsafe_code)]

pub mod jsonschema;
pub mod manifest;
pub mod tools;
