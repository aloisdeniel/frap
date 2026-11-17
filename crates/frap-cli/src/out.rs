// SPDX-License-Identifier: GPL-3.0-or-later
//! The stdout writer (rule 2.2.7: only `frap-cli` writes to stdout): JSON Lines, flushed after
//! every line so a driving process sees each response as soon as it exists (§13.1), plus the
//! ready/end events and the per-session statement counts of the end event.

use std::io::{self, Write};

use frap_rna::FrapError;
use frap_session::{DoneRecord, Response, Status};
use serde_json::{Map, Value as J, json};

/// Statement counts of the end event (`{"statements", "errors", "cancelled"}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Responses written (every status, SKIPPED included).
    pub statements: u64,
    /// `ERROR` responses.
    pub errors: u64,
    /// `CANCELLED` responses.
    pub cancelled: u64,
}

impl Stats {
    /// Counts one response.
    pub fn count(&mut self, r: &Response) {
        self.statements += 1;
        match r.status {
            Status::Error => self.errors += 1,
            Status::Cancelled => self.cancelled += 1,
            Status::Finished | Status::Skipped => {}
        }
    }

    /// True when nothing failed.
    pub fn ok(&self) -> bool {
        self.errors == 0 && self.cancelled == 0
    }
}

/// The ready event (§13.1), first line of the stream unless `--no-ready`.
pub fn ready_event(startup: &str) -> J {
    json!({
        "event": "ready",
        "frap": env!("CARGO_PKG_VERSION"),
        "protocol": 1,
        "blender_api": frap_schema::manifest::BLENDER_API,
        "startup": startup,
    })
}

/// The end event (§13.1), last line at EOF unless `--no-end-event`.
pub fn end_event(s: &Stats) -> J {
    json!({
        "event": "end",
        "ok": s.ok(),
        "stats": {"statements": s.statements, "errors": s.errors, "cancelled": s.cancelled},
    })
}

/// A `FrapError` in the §13.4 JSON shape (the `error` member of a response), for errors that
/// are not statement responses (`frap_describe`, MCP tool errors).
pub fn error_json(e: &FrapError) -> J {
    let mut m = Map::new();
    m.insert("exception".into(), J::from(e.exception.as_str()));
    m.insert("code".into(), J::from(e.code.0));
    m.insert("message".into(), J::from(e.message.as_str()));
    m.insert(
        "span".into(),
        match e.span {
            Some(s) => {
                json!({"line": s.line, "col": s.col, "col_chars": s.col_chars, "len": s.len})
            }
            None => J::Null,
        },
    );
    m.insert("detail".into(), J::Object(e.detail.clone()));
    m.insert(
        "hint".into(),
        e.hint.as_deref().map(J::from).unwrap_or(J::Null),
    );
    J::Object(m)
}

/// A JSON Lines writer that counts the responses it writes.
pub struct Out<W: Write> {
    w: W,
    /// Counts for the end event.
    pub stats: Stats,
}

impl<W: Write> Out<W> {
    /// Wraps a writer.
    pub fn new(w: W) -> Self {
        Out {
            w,
            stats: Stats::default(),
        }
    }

    /// Writes one JSON value as a line and flushes.
    pub fn line(&mut self, v: &J) -> io::Result<()> {
        serde_json::to_writer(&mut self.w, v)?;
        self.w.write_all(b"\n")?;
        self.w.flush()
    }

    /// Writes one statement response and counts it.
    pub fn response(&mut self, r: &Response) -> io::Result<()> {
        self.stats.count(r);
        tracing::debug!(
            seq = r.seq,
            line = r.line,
            kind = r.kind,
            status = r.status.as_str(),
            "statement"
        );
        self.line(&r.to_json())
    }

    /// Writes a serve request's done record.
    pub fn done(&mut self, d: &DoneRecord) -> io::Result<()> {
        self.line(&d.to_json())
    }

    /// The underlying writer (tests).
    #[cfg(test)]
    pub fn into_inner(self) -> W {
        self.w
    }
}
