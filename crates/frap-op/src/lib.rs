// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-op` (layer L9): the operator framework (ARCHITECTURE §10): `OperatorDef` and the
//! `OPERATORS` slice, the registry, argument conversion with Blender's messages, declarative
//! poll, `OpCtx`, `Services`, shared helpers, and the extension-point slices `MODE_HANDLERS` and
//! `TRANSFORM_MODE_HANDLERS`.
//!
//! Ownership (M1): WP 1.12 owns everything except this file. CONTRACT files (frozen at M1 W0):
//! `def.rs`, `ctx.rs`, `services.rs`, `modes.rs`.
#![deny(unsafe_code)]

mod ctx;
mod def;
pub mod describe;
pub mod errors;
pub mod helpers;
mod modes;
pub mod planned;
pub mod poll;
mod registry;
mod services;

pub use ctx::{OpCtx, OpOutput, Placement};
pub use def::*;
pub use describe::{DESCRIBE_PROVIDERS, DescribeProvider, describe_provider};
pub use modes::{
    MODE_HANDLERS, ModeHandler, TRANSFORM_MODE_HANDLERS, TransformModeHandler, TransformOp,
};
pub use registry::{
    Lookup, OP_GATES, OpGate, Registry, check_poll, convert_args, gate, lookup, lookup_in, suggest,
};
pub use services::{ImageCache, PathResolver, Services, UndoRequest};

/// Operator-tier proc-macro (WP 1.12).
pub use frap_macros::operator;
/// Re-export for registration sites: `#[linkme::distributed_slice(frap_op::OPERATORS)]`.
pub use linkme;

/// What an operator file usually needs.
pub mod prelude {
    pub use crate::ctx::{OpCtx, Placement};
    pub use crate::def::{
        OPERATORS, OpError, OpKind, OpResult, OpStatus, OperatorDef, Support, Tier,
    };
    pub use crate::helpers;
    pub use crate::poll;
    pub use frap_rna::{OpArgs, OpProps, ReportType, Value};
}
