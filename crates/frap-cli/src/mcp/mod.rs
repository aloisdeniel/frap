// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap mcp` (§13.1): a minimal, hand-written Model Context Protocol server on stdio, without
//! an SDK dependency (risk table §23: MCP churn stays inside this directory, and
//! `frap serve --jsonl` is the stable fallback).
//!
//! * Transport: newline-delimited JSON-RPC 2.0 messages on stdin/stdout (MCP stdio transport);
//!   logs go to stderr only.
//! * Lifecycle: `initialize` (protocol version negotiation, `tools` capability),
//!   `notifications/initialized`, `ping`. Calls before `initialize` are served too.
//! * Tools: `tools/list` returns exactly `frap --describe=tools-mcp` for the same flags (the
//!   tools of `--describe=tools-anthropic` with MCP's `inputSchema` key); `tools/call` runs
//!   `frap_exec`, `frap_describe` and any `frap_ops_<ns>` tool enabled with
//!   `--tool-namespaces`.
//! * One `Session` per connection (= process), alive across tool calls: the scene built by one
//!   `tools/call` is there for the next.
//!
//! A `frap_exec` result is one text block holding the responses (one JSON line per statement,
//! each carrying the JSON-RPC id as its `id`) followed by the request's done record, exactly
//! as `frap serve --jsonl` would print them; `isError` is true when a statement did not
//! finish. Bad tool arguments are tool execution errors (`isError: true`, a message the model
//! can act on), not protocol errors, as MCP 2025-11-25 asks; an unknown tool is JSON-RPC
//! `-32602`.

mod rpc;

use std::io::{self, BufRead, Write};

use frap_rna::FrapError;
use frap_schema::manifest::{DescribeError, DescribeOptions, Target, describe};
use frap_schema::tools::{DESCRIBE, EXEC, NS_PREFIX, call_to_statement};
use frap_session::{DoneRecord, OnError, Request, Session};
use serde_json::{Map, Value as J, json};

use crate::out::error_json;
pub use rpc::{INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR};

/// MCP protocol revisions this server speaks, newest first. A client asking for one of them
/// gets it; any other request is answered with the newest (the client then decides).
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "\
frap edits a persistent 3D scene headlessly with Blender 5.2's Python API names (bpy.ops, bpy.data, bpy.context). \
Use frap_exec to run statements (one per line; no variables: use full paths such as bpy.data.objects['Cube']); \
the scene stays alive between calls and starts as Blender's factory scene (Cube, Camera, Light). \
Use frap_describe ('mesh.bevel', 'Object', 'Object.location') to check names, arguments, units (angles are radians) and poll requirements. \
A failing statement leaves the scene unchanged; read error.hint and detail.did_you_mean.";

/// The MCP server state: the connection's session and its fixed tool list.
pub struct Server {
    session: Session,
    describe_opts: DescribeOptions,
    tools: J,
    tool_names: Vec<String>,
    initialized: bool,
    protocol_version: Option<String>,
}

impl Server {
    /// A server for `session`; `opts` selects the tool list (`--tool-namespaces`,
    /// `--implemented-only`). Fails on an unknown namespace.
    pub fn new(session: Session, opts: DescribeOptions) -> Result<Server, DescribeError> {
        let tools = frap_schema::tools::mcp(&opts)?;
        let tool_names = tools["tools"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Server {
            session,
            describe_opts: opts,
            tools,
            tool_names,
            initialized: false,
            protocol_version: None,
        })
    }

    /// The negotiated protocol version (after `initialize`).
    #[cfg(test)]
    pub fn protocol_version(&self) -> Option<&str> {
        self.protocol_version.as_deref()
    }

    /// True once the client sent `notifications/initialized`.
    #[cfg(test)]
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// Answers one input line (a message or a batch); `None` when nothing is to be sent back
    /// (notifications, client responses).
    pub fn handle_line(&mut self, line: &[u8]) -> Option<J> {
        match serde_json::from_slice::<J>(line) {
            Ok(J::Array(batch)) => {
                if batch.is_empty() {
                    return Some(rpc::error(
                        J::Null,
                        INVALID_REQUEST,
                        "Invalid Request: empty batch",
                    ));
                }
                let out: Vec<J> = batch
                    .into_iter()
                    .filter_map(|m| self.handle_message(m))
                    .collect();
                (!out.is_empty()).then_some(J::Array(out))
            }
            Ok(m) => self.handle_message(m),
            Err(e) => Some(rpc::error(
                J::Null,
                PARSE_ERROR,
                &format!("Parse error: {e}"),
            )),
        }
    }

    /// Answers one JSON-RPC message.
    pub fn handle_message(&mut self, msg: J) -> Option<J> {
        let J::Object(mut m) = msg else {
            return Some(rpc::error(
                J::Null,
                INVALID_REQUEST,
                "Invalid Request: expected an object",
            ));
        };
        let id = m.remove("id");
        let Some(method) = m.get("method").and_then(J::as_str).map(str::to_owned) else {
            // A response from the client (we send no requests) is ignored.
            if m.contains_key("result") || m.contains_key("error") {
                return None;
            }
            return Some(rpc::error(
                id.unwrap_or(J::Null),
                INVALID_REQUEST,
                "Invalid Request: missing 'method'",
            ));
        };
        if m.get("jsonrpc").and_then(J::as_str) != Some("2.0") {
            return id.map(|id| {
                rpc::error(
                    id,
                    INVALID_REQUEST,
                    "Invalid Request: 'jsonrpc' must be \"2.0\"",
                )
            });
        }
        let params = m.remove("params");
        let Some(id) = id else {
            self.notification(&method);
            return None;
        };
        if !matches!(id, J::String(_) | J::Number(_)) {
            return Some(rpc::error(
                J::Null,
                INVALID_REQUEST,
                "Invalid Request: 'id' must be a string or a number",
            ));
        }
        let params = match params {
            None | Some(J::Null) => Map::new(),
            Some(J::Object(p)) => p,
            Some(_) => {
                return Some(rpc::error(
                    id,
                    INVALID_PARAMS,
                    "Invalid params: expected an object",
                ));
            }
        };
        let _span = tracing::debug_span!("mcp", method = %method, id = %id).entered();
        Some(match self.request(&method, &id, params) {
            Ok(result) => rpc::result(id, result),
            Err((code, msg)) => rpc::error(id, code, &msg),
        })
    }

    fn notification(&mut self, method: &str) {
        match method {
            "notifications/initialized" => {
                self.initialized = true;
                tracing::info!(protocol = ?self.protocol_version, "mcp client initialized");
            }
            other => tracing::debug!(method = other, "mcp notification ignored"),
        }
    }

    fn request(
        &mut self,
        method: &str,
        id: &J,
        params: Map<String, J>,
    ) -> Result<J, (i64, String)> {
        match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools.clone()),
            "tools/call" => self.tools_call(id, params),
            other => Err((METHOD_NOT_FOUND, format!("Method not found: {other}"))),
        }
    }

    fn initialize(&mut self, params: &Map<String, J>) -> J {
        let asked = params.get("protocolVersion").and_then(J::as_str);
        let version = asked
            .filter(|v| PROTOCOL_VERSIONS.contains(v))
            .unwrap_or(PROTOCOL_VERSIONS[0])
            .to_owned();
        self.protocol_version = Some(version.clone());
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {
                "name": "frap",
                "title": "frap (headless Blender 5.2 API)",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": INSTRUCTIONS,
        })
    }

    fn tools_call(&mut self, id: &J, mut params: Map<String, J>) -> Result<J, (i64, String)> {
        let name = match params.remove("name") {
            Some(J::String(s)) => s,
            _ => {
                return Err((
                    INVALID_PARAMS,
                    "Invalid params: 'name' must be a string".into(),
                ));
            }
        };
        if !self.tool_names.contains(&name) {
            return Err((INVALID_PARAMS, format!("Unknown tool: {name}")));
        }
        let args = match params.remove("arguments") {
            None | Some(J::Null) => Map::new(),
            Some(J::Object(a)) => a,
            Some(_) => {
                return Ok(tool_error(&format!(
                    "{name}: 'arguments' must be an object"
                )));
            }
        };
        Ok(match name.as_str() {
            EXEC => self.exec_tool(id, &args),
            DESCRIBE => self.describe_tool(&args),
            ns if ns.starts_with(NS_PREFIX) => match call_to_statement(ns, &J::Object(args)) {
                Ok(stmt) => self.exec(id, stmt, OnError::SkipRest),
                Err(e) => tool_error(&format!("{name}: {e}")),
            },
            _ => return Err((INVALID_PARAMS, format!("Unknown tool: {name}"))),
        })
    }

    fn exec_tool(&mut self, id: &J, args: &Map<String, J>) -> J {
        if let Some(k) = args
            .keys()
            .find(|k| !matches!(k.as_str(), "statements" | "on_error"))
        {
            return tool_error(&format!(
                "{EXEC}: unknown argument '{k}'; arguments are statements and on_error"
            ));
        }
        let Some(statements) = args.get("statements").and_then(J::as_str) else {
            return tool_error(&format!("{EXEC}: 'statements' must be a string"));
        };
        let on_error = match args.get("on_error") {
            None | Some(J::Null) => OnError::SkipRest,
            Some(J::String(s)) if s == "skip_rest" => OnError::SkipRest,
            Some(J::String(s)) if s == "continue" => OnError::Continue,
            Some(other) => {
                return tool_error(&format!(
                    "{EXEC}: 'on_error' must be \"skip_rest\" or \"continue\", not {other}"
                ));
            }
        };
        self.exec(id, statements.to_owned(), on_error)
    }

    fn exec(&mut self, id: &J, src: String, on_error: OnError) -> J {
        let rs = self.session.exec_request(Request {
            id: id.clone(),
            src,
            on_error,
        });
        let done = DoneRecord::from_responses(id.clone(), &rs);
        let mut text = String::new();
        for r in &rs {
            tracing::debug!(
                seq = r.seq,
                line = r.line,
                kind = r.kind,
                status = r.status.as_str(),
                "statement"
            );
            text.push_str(&r.to_json().to_string());
            text.push('\n');
        }
        text.push_str(&done.to_json().to_string());
        json!({"content": [{"type": "text", "text": text}], "isError": !done.ok})
    }

    fn describe_tool(&self, args: &Map<String, J>) -> J {
        if let Some(k) = args.keys().find(|k| k.as_str() != "target") {
            return tool_error(&format!(
                "{DESCRIBE}: unknown argument '{k}'; the only argument is target"
            ));
        }
        let Some(target) = args.get("target").and_then(J::as_str) else {
            return tool_error(&format!("{DESCRIBE}: 'target' must be a string"));
        };
        let opts = DescribeOptions {
            filter: None,
            implemented_only: self.describe_opts.implemented_only,
            tool_namespaces: Vec::new(),
        };
        match Target::parse(target).and_then(|t| describe(&t, &opts)) {
            Ok(v) => {
                json!({"content": [{"type": "text", "text": v.to_string()}], "isError": false})
            }
            Err(e) => {
                let e: FrapError = e.into_frap();
                json!({
                    "content": [{"type": "text", "text": json!({"error": error_json(&e)}).to_string()}],
                    "isError": true,
                })
            }
        }
    }
}

fn tool_error(msg: &str) -> J {
    json!({"content": [{"type": "text", "text": msg}], "isError": true})
}

/// The stdio loop: one message per line until EOF.
pub fn run<R: BufRead, W: Write>(
    server: &mut Server,
    mut input: R,
    mut output: W,
) -> io::Result<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if input.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if let Some(reply) = server.handle_line(&buf) {
            serde_json::to_writer(&mut output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests;
