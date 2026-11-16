// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes raised by `frap-lang` (§13.4).

use frap_rna::ErrorCode;

/// Python-level syntax error (`'(' was never closed`, `invalid syntax`).
pub const SYNTAX: ErrorCode = ErrorCode("SYNTAX");
/// Valid Python that frap does not accept (variables, loops, ...), or not yet (M0 subset).
pub const UNSUPPORTED_SYNTAX: ErrorCode = ErrorCode("UNSUPPORTED_SYNTAX");
