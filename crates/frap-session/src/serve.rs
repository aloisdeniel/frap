// SPDX-License-Identifier: GPL-3.0-or-later
//! Request framing for `frap serve --jsonl` and `frap mcp` (ARCHITECTURE §13.1, CONTRACT frozen
//! at M1 W0). One request per input line; `src` is parsed as one complete unit, so frap never
//! waits for more input; every response echoes the request's `id`; each request ends with a
//! done record.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::response::Response;
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
    /// The done record of a request's responses.
    pub fn from_responses(id: Json, responses: &[Response]) -> DoneRecord {
        let _ = (id, responses);
        todo!("WP 1.14")
    }
}

impl Session {
    /// Runs one serve/MCP request (§11.6, §13.1): one response per statement, each carrying
    /// the request's `id`; with `on_error: skip_rest` the statements after a failure answer
    /// `SKIPPED`. The caller writes `DoneRecord::from_responses` after them.
    pub fn exec_request(&mut self, req: Request) -> Vec<Response> {
        let _ = req;
        todo!("WP 1.14: on_error, SKIPPED, id echo")
    }
}
