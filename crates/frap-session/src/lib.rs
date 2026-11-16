// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-session` (layer L11): the `Session` (executor IR → calls, path resolver, transactions,
//! response builder, JSON value encoding, request framing for `serve`/MCP) and the `Host` and
//! `Services` implementations (ARCHITECTURE §11.6, §12, §13). Never writes to stdout (rule
//! 2.2.7). With feature `gpu` it owns the lazily created `RenderService` from `frap-render`.
//!
//! Ownership (M1): WP 1.14 owns everything except this file. CONTRACT (frozen at M1 W0): the
//! `Response` JSON shape (`response.rs`) and the serve framing (`serve.rs`).
#![deny(unsafe_code)]

pub mod errors;
pub mod resolve;
pub mod response;
pub mod serve;
mod session;

pub use response::{Response, Status};
pub use serve::{DoneRecord, OnError, Request};
pub use session::{Limits, Session, SessionOptions, Startup, Verbosity};
