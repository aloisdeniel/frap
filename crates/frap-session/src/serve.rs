// SPDX-License-Identifier: GPL-3.0-or-later
//! Request framing for `frap serve --jsonl` and `frap mcp` (ARCHITECTURE §13.1, CONTRACT frozen
//! at M1 W0). One request per input line; `src` is parsed as one complete unit, so frap never
//! waits for more input; every response echoes the request's `id`; each request ends with a
//! done record.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::response::{Response, Status};
use crate::session::Session;

/// What happens to the rest of a request after a failing statement.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// Answer every remaining statement with `status: "SKIPPED"` (default for requests).
    #[default]
    SkipRest,
    /// Run them.
    Continue,
}

/// `{"id": <string or number>, "src": "<statements>", "on_error": "skip_rest" | "continue"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Echoed on every response of the request (string or number).
    pub id: Json,
    /// Statements, parsed as one complete unit.
    pub src: String,
    /// Batch error policy.
    #[serde(default)]
    pub on_error: OnError,
}

/// Counts of a request's outcomes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneCounts {
    /// `FINISHED`
    pub finished: u32,
    /// `ERROR`
    pub error: u32,
    /// `CANCELLED`
    pub cancelled: u32,
    /// `SKIPPED`
    pub skipped: u32,
}

/// `{"id": …, "event": "done", "ok": <all ok>, "counts": {...}}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DoneRecord {
    /// The request's id.
    pub id: Json,
    /// Always `"done"`.
    pub event: String,
    /// True iff every statement finished.
    pub ok: bool,
    /// Outcome counts.
    pub counts: DoneCounts,
}

impl DoneRecord {
    /// The done record of a request's responses: `ok` is true iff every statement finished
    /// (an empty request is ok).
    pub fn from_responses(id: Json, responses: &[Response]) -> DoneRecord {
        let mut counts = DoneCounts::default();
        for r in responses {
            match r.status {
                Status::Finished => counts.finished += 1,
                Status::Error => counts.error += 1,
                Status::Cancelled => counts.cancelled += 1,
                Status::Skipped => counts.skipped += 1,
            }
        }
        DoneRecord {
            id,
            event: "done".to_owned(),
            ok: responses.iter().all(Response::ok),
            counts,
        }
    }

    /// The JSON line of the record.
    pub fn to_json(&self) -> Json {
        serde_json::to_value(self).expect("invariant: DoneRecord serializes")
    }
}

impl Session {
    /// Runs one serve/MCP request (§11.6, §13.1): `src` is parsed as one complete unit (an
    /// unclosed bracket is an immediate `SyntaxError`, frap never waits for more input); one
    /// response per statement, each carrying the request's `id`, with line numbers relative to
    /// the request. With `on_error: skip_rest` (the default) every statement after a failing
    /// one (`ERROR` or `CANCELLED`) answers `SKIPPED` without running; with `continue` they
    /// run. The caller writes [`DoneRecord::from_responses`] after them.
    pub fn exec_request(&mut self, req: Request) -> Vec<Response> {
        let mut out = Vec::new();
        let mut failed = false;
        for parsed in frap_lang::parse_complete(&req.src) {
            let mut r = if failed && req.on_error == OnError::SkipRest {
                match &parsed {
                    Ok(stmt) => self.skipped(
                        crate::session::stmt_kind(&stmt.cmd),
                        crate::session::stmt_op(&stmt.cmd),
                        stmt.line,
                    ),
                    Err(e) => self.skipped("invalid", None, e.span.map_or(1, |s| s.line)),
                }
            } else {
                match parsed {
                    Ok(stmt) => self.exec(&stmt),
                    Err(e) => {
                        let line = e.span.map_or(1, |s| s.line);
                        self.reject(line, e)
                    }
                }
            };
            failed |= matches!(r.status, Status::Error | Status::Cancelled);
            r.id = Some(req.id.clone());
            out.push(r);
        }
        out
    }
}
