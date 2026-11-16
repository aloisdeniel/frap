// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-lang` (layer L1): hand-written lexer, bpy-line parser, short-form parser and the
//! semantic IR (ARCHITECTURE §11). Depends only on `frap-rna` (`Span`, `FrapError`).
//! It knows nothing about operators, RNA or the scene: both syntaxes lower to one `Command`.
#![deny(unsafe_code)]

pub mod errors;
pub mod ir;
pub mod lexer;
mod parse;
pub mod split;

pub use ir::*;
pub use parse::{parse_complete, parse_complete_at};
pub use split::is_complete;
