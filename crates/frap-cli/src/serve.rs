// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap serve --jsonl` (§13.1): one JSON request per input line,
//! `{"id": <string or number>, "src": "<statements>", "on_error": "skip_rest" | "continue"}`.
//! `src` is parsed as one complete unit (`Session::exec_request`), so frap never waits for more
//! input; every response carries the request's `id`, and each request ends with
//! `{"id": …, "event": "done", "ok": …, "counts": {…}}`.
//!
//! A line that is not a valid request (not JSON, not an object, a missing or mistyped member,
//! an unknown member) is answered with one `invalid` response, `ValueError` code
//! `REQUEST_INVALID`, followed by a done record when the request's `id` could be read. The
//! session goes on. `on_error: null` means the default (`skip_rest`), as OpenAI strict-mode
//! tool calls send it.

use std::io::{self, BufRead, Write};

use frap_rna::{ERROR_CODES, ErrorCode, ErrorCodeDoc, FrapError, PyExc};
use frap_session::{DoneRecord, OnError, Request, Session};
use serde_json::Value as J;

use crate::out::Out;
use crate::raw::Flow;

/// A serve line that is not a valid request.
pub const REQUEST_INVALID: ErrorCode = ErrorCode("REQUEST_INVALID");

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(ERROR_CODES)]
#[linkme(crate = frap_rna::linkme)]
static DOC_REQUEST_INVALID: ErrorCodeDoc = ErrorCodeDoc {
    code: REQUEST_INVALID,
    exception: PyExc::ValueError,
    summary: "frap serve --jsonl: a line that is not a request {\"id\": string|number, \"src\": string, \"on_error\"?: \"skip_rest\"|\"continue\"}",
};

const REQUEST_HINT: &str = r#"send one JSON object per line: {"id": 1, "src": "bpy.ops.mesh.primitive_cube_add()", "on_error": "skip_rest"}"#;

const MEMBERS: [&str; 3] = ["id", "src", "on_error"];

/// A request line that could not be turned into a [`Request`].
#[derive(Debug)]
pub struct BadRequest {
    /// The request's id when it could be read (then a done record follows the error).
    pub id: Option<J>,
    /// `ValueError` / `REQUEST_INVALID`.
    pub err: FrapError,
}

fn bad(id: Option<J>, msg: impl Into<String>) -> BadRequest {
    BadRequest {
        id,
        err: FrapError::new(
            PyExc::ValueError,
            REQUEST_INVALID,
            format!("frap serve: invalid request: {}", msg.into()),
        )
        .with_hint(REQUEST_HINT),
    }
}

/// Levenshtein distance (for `did_you_mean` over the three request members).
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Parses one request line.
pub fn parse_request(line: &[u8]) -> Result<Request, BadRequest> {
    let v: J = serde_json::from_slice(line).map_err(|e| bad(None, e.to_string()))?;
    let J::Object(mut m) = v else {
        return Err(bad(None, "expected a JSON object"));
    };
    let id = match m.remove("id") {
        Some(id @ (J::String(_) | J::Number(_))) => id,
        Some(_) => return Err(bad(None, "'id' must be a string or a number")),
        None => return Err(bad(None, "missing 'id'")),
    };
    let src = match m.remove("src") {
        Some(J::String(s)) => s,
        Some(_) => return Err(bad(Some(id), "'src' must be a string")),
        None => return Err(bad(Some(id), "missing 'src'")),
    };
    let on_error = match m.remove("on_error") {
        None | Some(J::Null) => OnError::SkipRest,
        Some(J::String(s)) if s == "skip_rest" => OnError::SkipRest,
        Some(J::String(s)) if s == "continue" => OnError::Continue,
        Some(other) => {
            return Err(bad(
                Some(id),
                format!("'on_error' must be \"skip_rest\" or \"continue\", not {other}"),
            ));
        }
    };
    if let Some(k) = m.keys().next() {
        let mut e = bad(
            Some(id),
            format!("unknown member '{k}'; a request has id, src and on_error"),
        );
        let dym: Vec<J> = MEMBERS
            .into_iter()
            .filter(|m| edit_distance(k, m) <= 2)
            .map(J::from)
            .collect();
        e.err = e.err.with_detail("did_you_mean", dym);
        return Err(e);
    }
    Ok(Request { id, src, on_error })
}

/// Answers one request line; returns whether every statement finished (a line that is not a
/// request counts as failed).
pub fn handle_line<W: Write>(
    session: &mut Session,
    out: &mut Out<W>,
    line: &[u8],
) -> io::Result<bool> {
    match parse_request(line) {
        Ok(req) => {
            let id = req.id.clone();
            let _span = tracing::debug_span!("request", id = %id).entered();
            let rs = session.exec_request(req);
            for r in &rs {
                out.response(r)?;
            }
            let done = DoneRecord::from_responses(id, &rs);
            out.done(&done)?;
            Ok(done.ok)
        }
        Err(BadRequest { id, err }) => {
            let mut r = session.reject(1, err);
            r.id.clone_from(&id);
            out.response(&r)?;
            if let Some(id) = id {
                out.done(&DoneRecord::from_responses(id, std::slice::from_ref(&r)))?;
            }
            Ok(false)
        }
    }
}

/// The serve loop: one request per line until EOF. Blank lines are ignored. With
/// `fail_fast`, stops after the done record of the first request that did not finish.
pub fn run<R: BufRead, W: Write>(
    session: &mut Session,
    out: &mut Out<W>,
    mut input: R,
    fail_fast: bool,
) -> io::Result<Flow> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if input.read_until(b'\n', &mut buf)? == 0 {
            return Ok(Flow::Done);
        }
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let ok = handle_line(session, out, &buf)?;
        if fail_fast && !ok {
            return Ok(Flow::Stopped);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_session::SessionOptions;
    use serde_json::json;

    fn run_str(input: &str, fail_fast: bool) -> (Flow, Vec<J>) {
        let mut s = Session::new(SessionOptions::default()).unwrap();
        let mut out = Out::new(Vec::new());
        let flow = run(&mut s, &mut out, input.as_bytes(), fail_fast).unwrap();
        let v = String::from_utf8(out.into_inner())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        (flow, v)
    }

    #[test]
    fn parses_requests() {
        let r = parse_request(br#"{"id": 3, "src": "print(1)"}"#).unwrap();
        assert_eq!((r.id, r.on_error), (json!(3), OnError::SkipRest));
        let r = parse_request(br#"{"id": "a", "src": "", "on_error": null}"#).unwrap();
        assert_eq!(r.on_error, OnError::SkipRest);
        let r = parse_request(br#"{"id": 1.5, "src": "", "on_error": "continue"}"#).unwrap();
        assert_eq!(r.on_error, OnError::Continue);
        for (line, id, msg) in [
            (&b"nope"[..], None, "expected ident"),
            (b"[1]", None, "expected a JSON object"),
            (br#"{"src": "x"}"#, None, "missing 'id'"),
            (br#"{"id": true, "src": "x"}"#, None, "'id' must be"),
            (br#"{"id": 1}"#, Some(json!(1)), "missing 'src'"),
            (br#"{"id": 1, "src": 2}"#, Some(json!(1)), "'src' must be"),
            (
                br#"{"id": 1, "src": "", "on_error": "stop"}"#,
                Some(json!(1)),
                "'on_error' must be",
            ),
        ] {
            let e = parse_request(line).unwrap_err();
            assert_eq!(e.id, id, "{line:?}");
            assert!(e.err.message.contains(msg), "{}", e.err.message);
            assert_eq!(e.err.code, REQUEST_INVALID);
        }
        let e = parse_request(br#"{"id": 1, "src": "", "onerror": "continue"}"#).unwrap_err();
        assert_eq!(e.err.detail["did_you_mean"], json!(["on_error"]));
    }

    #[test]
    fn ids_done_records_and_on_error() {
        let (flow, v) = run_str(
            concat!(
                r#"{"id": 7, "src": "print(1)\nbpy.ops.mesh.nope()\nprint(3)"}"#,
                "\n\n",
                r#"{"id": "b", "src": "print(1)\nbpy.ops.mesh.nope()\nprint(3)", "on_error": "continue"}"#,
                "\n",
            ),
            false,
        );
        assert_eq!(flow, Flow::Done);
        assert_eq!(v.len(), 8);
        assert!(v[..4].iter().all(|r| r["id"] == 7));
        assert_eq!(v[2]["status"], "SKIPPED");
        assert_eq!(v[2]["line"], 3);
        assert_eq!(
            v[3],
            json!({"id": 7, "event": "done", "ok": false,
                   "counts": {"finished": 1, "error": 1, "cancelled": 0, "skipped": 1}})
        );
        assert!(v[4..].iter().all(|r| r["id"] == "b"));
        assert_eq!(v[6]["status"], "FINISHED");
        assert_eq!(v[6]["seq"], 6, "seq counts per session, skipped included");
        assert_eq!(v[7]["counts"]["finished"], 2);
    }

    #[test]
    fn unclosed_bracket_is_answered_at_once() {
        let (_, v) = run_str("{\"id\": 1, \"src\": \"print(1,\"}\n", false);
        assert_eq!(v[0]["error"]["message"], "'(' was never closed");
        assert_eq!(v[1]["event"], "done");
    }

    #[test]
    fn bad_lines_and_fail_fast() {
        let (flow, v) = run_str(
            "not json\n{\"id\": 2}\n{\"id\": 3, \"src\": \"print(1)\"}\n",
            false,
        );
        assert_eq!(flow, Flow::Done);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0]["kind"], "invalid");
        assert_eq!(v[0]["error"]["exception"], "ValueError");
        assert!(v[0].get("id").is_none());
        assert_eq!(v[1]["id"], 2);
        assert_eq!(v[2]["event"], "done");
        assert_eq!(v[2]["counts"]["error"], 1);
        assert_eq!(v[4]["ok"], true);

        let (flow, v) = run_str(
            "{\"id\": 1, \"src\": \"bpy.ops.mesh.nope()\"}\n{\"id\": 2, \"src\": \"print(1)\"}\n",
            true,
        );
        assert_eq!(flow, Flow::Stopped);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1]["event"], "done");
    }
}
