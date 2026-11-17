// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.25 acceptance through the real `frap` binary (ARCHITECTURE §13.1, §14, §22.3):
//! flags, exit codes, `--fail-fast`, ready/end events (validated against
//! `--describe=protocol`), raw mode never hanging on an unclosed bracket, `frap serve --jsonl`
//! answering each request without waiting for EOF, and `frap mcp` (handshake, `tools/list` equal
//! to `--describe=tools-anthropic`, state kept across `tools/call`s).
//!
//! The interactive tests keep stdin open and read with a timeout: a response that only came
//! at EOF would fail them.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use frap_schema::jsonschema::validate::validate;
use serde_json::{Value as J, json};

const TIMEOUT: Duration = Duration::from_secs(60);

fn frap() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_frap"));
    c.env_remove("FRAP_LOG");
    c
}

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = frap()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn json_lines(o: &Output) -> Vec<J> {
    String::from_utf8(o.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

fn protocol() -> J {
    let o = run(&["--describe=protocol"], "");
    assert!(o.status.success());
    serde_json::from_slice(&o.stdout).unwrap()
}

fn assert_valid(schema: &J, v: &J) {
    if let Err(e) = validate(schema, v) {
        panic!("{v} does not match the protocol schema: {e:?}");
    }
}

/// A running frap whose stdout lines arrive on a channel.
struct Live {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
}

impl Live {
    fn spawn(args: &[&str]) -> Live {
        let mut child = frap()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines() {
                let Ok(l) = l else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        Live {
            child,
            stdin,
            lines: rx,
        }
    }

    fn send(&mut self, s: &str) {
        let w = self.stdin.as_mut().unwrap();
        w.write_all(s.as_bytes()).unwrap();
        w.flush().unwrap();
    }

    /// The next stdout line, while stdin is still open.
    fn next(&self) -> J {
        let l = self
            .lines
            .recv_timeout(TIMEOUT)
            .expect("frap answered without waiting for more input");
        serde_json::from_str(&l).unwrap()
    }

    fn finish(mut self) -> (std::process::ExitStatus, Vec<J>) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        let rest = self
            .lines
            .iter()
            .map(|l| serde_json::from_str(&l).unwrap())
            .collect();
        (status, rest)
    }
}

#[test]
fn ready_and_end_events_follow_the_protocol() {
    let p = protocol();
    let o = run(&[], "print(1)\nbpy.ops.mesh.nope()\n");
    assert!(o.status.success());
    let v = json_lines(&o);
    assert_eq!(v.len(), 4);
    assert_eq!(
        v[0],
        json!({"event": "ready", "frap": env!("CARGO_PKG_VERSION"), "protocol": 1,
               "blender_api": "5.2", "startup": "factory"})
    );
    assert_valid(&p["events"]["ready"], &v[0]);
    assert_valid(&p["response"], &v[1]);
    assert_valid(&p["response"], &v[2]);
    assert_eq!(
        v[3],
        json!({"event": "end", "ok": false, "stats": {"statements": 2, "errors": 1, "cancelled": 0}})
    );
    assert_valid(&p["events"]["end"], &v[3]);

    let o = run(&["--no-ready", "--no-end-event"], "print(1)\n");
    let v = json_lines(&o);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0]["kind"], "print");

    let o = run(&["--empty", "-c", "bpy.data.objects"], "");
    let v = json_lines(&o);
    assert_eq!(v[0]["startup"], "empty");
    assert_eq!(v[1]["value"]["len"], 0);
}

#[test]
fn exit_codes() {
    // 0: errors are reported in-band.
    let o = run(&["-c", "bpy.ops.mesh.nope()"], "");
    assert_eq!(o.status.code(), Some(0));
    // 1: --fail-fast stopped after the failed statement (the end event still closes the stream).
    let o = run(
        &["--fail-fast"],
        "print(1)\nbpy.ops.mesh.nope()\nprint(3)\n",
    );
    assert_eq!(o.status.code(), Some(1));
    let v = json_lines(&o);
    assert_eq!(v.len(), 4);
    assert_eq!(v[2]["error"]["code"], "OP_UNKNOWN");
    assert_eq!(v[3]["event"], "end");
    assert_eq!(v[3]["stats"]["statements"], 2);
    // --fail-fast without a failure: 0.
    let o = run(&["--fail-fast", "-c", "print(1)"], "");
    assert_eq!(o.status.code(), Some(0));
    // 2: usage errors and failure to start; nothing on stdout.
    for args in [
        &["--bogus"][..],
        &["--verbosity", "loud"],
        &["--max-items", "-3"],
        &["--stmt-timeout", "soon"],
        &["--empty", "--file", "a.frap"],
        &["-c", "print(1)", "run", "a.py"],
        &["--describe", "-c", "print(1)"],
        &["--filter", "mesh.*"],
        &["--tool-namespaces", "mesh"],
        &["mcp", "--fail-fast"],
        &["mcp", "--tool-namespaces", "nope"],
        &["--describe", "op:mesh.nope"],
        &["--describe=tools-anthropic", "--tool-namespaces", "meshh"],
        &["--log-level", "x=["],
        &["run", "/nonexistent/frap/script.py"],
        &["--file", "/nonexistent/frap/scene.frap"],
    ] {
        let o = run(args, "");
        assert_eq!(o.status.code(), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
        assert!(!o.stderr.is_empty(), "{args:?}");
    }
    let o = run(&["--describe", "op:mesh.bevl"], "");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("mesh.bevel"),
        "did_you_mean on stderr"
    );
    // --help and --version: stdout, 0.
    for a in ["--help", "--version"] {
        let o = run(&[a], "");
        assert_eq!(o.status.code(), Some(0));
        assert!(String::from_utf8_lossy(&o.stdout).contains("frap"));
    }
}

#[test]
fn session_flags_reach_the_session() {
    let o = run(
        &[
            "--verbosity",
            "minimal",
            "--max-items",
            "2",
            "--max-array",
            "6",
            "--no-ready",
            "--no-end-event",
            "-c",
            "bpy.data.objects\nbpy.data.meshes['Cube'].vertices.foreach_get('co')",
        ],
        "",
    );
    let v = json_lines(&o);
    assert!(v[0].get("context").is_none(), "minimal drops context");
    assert_eq!(v[0]["value"]["items"], json!(["Camera", "Cube"]));
    assert_eq!(v[0]["value"]["truncated"], true);
    assert_eq!(v[1]["value"]["shape"], json!([2, 3]));
    assert_eq!(v[1]["value"]["truncated"], true);
    let o = run(
        &[
            "--max-elements",
            "10",
            "--no-ready",
            "--no-end-event",
            "-c",
            "bpy.ops.mesh.primitive_cube_add()",
        ],
        "",
    );
    let v = json_lines(&o);
    assert_eq!(v[0]["error"]["code"], "LIMIT_EXCEEDED");
}

#[test]
fn describe_targets() {
    let o = run(&["--describe"], "");
    assert!(o.status.success());
    let m: J = serde_json::from_slice(&o.stdout).unwrap();
    assert!(m["operators"].as_array().unwrap().len() > 500);
    let o = run(
        &[
            "--describe",
            "--filter",
            "mesh.primitive_*",
            "--implemented-only",
        ],
        "",
    );
    let m: J = serde_json::from_slice(&o.stdout).unwrap();
    let ops = m["operators"].as_array().unwrap();
    assert!(!ops.is_empty());
    assert!(ops.iter().all(|o| {
        o["idname"].as_str().unwrap().starts_with("mesh.primitive_")
            && o["frap"]["status"] == "implemented"
    }));
    let o = run(&["--describe", "op:mesh.primitive_cube_add"], "");
    let m: J = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(m["idname"], "mesh.primitive_cube_add");
    let o = run(&["--describe=errors"], "");
    let m = String::from_utf8(o.stdout).unwrap();
    assert!(
        m.contains("REQUEST_INVALID"),
        "frap-cli's own code is listed"
    );
    for t in [
        "jsonschema",
        "tools-openai",
        "tools-mcp",
        "struct:Object",
        "Object.location",
    ] {
        let o = run(&["--describe", t], "");
        assert!(o.status.success(), "{t}");
        let _: J = serde_json::from_slice(&o.stdout).unwrap();
    }
}

#[test]
fn raw_mode_never_hangs_on_an_unclosed_bracket() {
    let mut f = Live::spawn(&[]);
    assert_eq!(f.next()["event"], "ready");
    // A blank line ends the pending continuation while stdin stays open.
    f.send("bpy.ops.mesh.primitive_cube_add(size=2,\n\n");
    let r = f.next();
    assert_eq!(r["kind"], "invalid");
    assert_eq!(r["error"]["exception"], "SyntaxError");
    assert_eq!(r["error"]["message"], "'(' was never closed");
    // A continuation that closes runs as one statement.
    f.send("print(1,\n  2)\n");
    let r = f.next();
    assert_eq!(
        (r["line"].clone(), r["value"].clone()),
        (json!(3), json!([1, 2]))
    );
    // A complete statement is answered at once.
    f.send("print(3)\n");
    assert_eq!(f.next()["value"], json!([3]));
    // EOF ends the last continuation.
    f.send("print(4,");
    let (status, rest) = f.finish();
    assert!(status.success());
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0]["error"]["message"], "'(' was never closed");
    assert_eq!(rest[1]["event"], "end");
}

#[test]
fn serve_answers_each_request_without_waiting() {
    let p = protocol();
    let mut f = Live::spawn(&["serve", "--jsonl"]);
    let ready = f.next();
    assert_valid(&p["events"]["ready"], &ready);
    f.send(
        "{\"id\": 7, \"src\": \"bpy.ops.mesh.primitive_cube_add()\\nbpy.ops.mesh.nope()\\nprint(1)\"}\n",
    );
    let r: Vec<J> = (0..4).map(|_| f.next()).collect();
    for x in &r[..3] {
        assert_eq!(x["id"], 7);
        assert_valid(&p["response"], x);
    }
    assert_eq!(r[0]["status"], "FINISHED");
    assert_eq!(r[1]["status"], "ERROR");
    assert_eq!(r[2]["status"], "SKIPPED");
    assert_eq!(r[2]["state"], "unchanged");
    assert_eq!(
        r[3],
        json!({"id": 7, "event": "done", "ok": false,
               "counts": {"finished": 1, "error": 1, "cancelled": 0, "skipped": 1}})
    );
    assert_valid(&p["events"]["done"], &r[3]);
    // An unclosed bracket in a request is an immediate SyntaxError, never a wait.
    f.send("{\"id\": \"q\", \"src\": \"print(1,\", \"on_error\": \"continue\"}\n");
    let r = f.next();
    assert_eq!(r["id"], "q");
    assert_eq!(r["error"]["message"], "'(' was never closed");
    assert_eq!(f.next()["event"], "done");
    // State persists across requests: the cube from request 7 is there.
    f.send("{\"id\": 9, \"src\": \"bpy.data.objects['Cube.001'].name\"}\n");
    assert_eq!(f.next()["value"], "Cube.001");
    assert_eq!(f.next()["ok"], true);
    // A bad line is answered in-band and the session goes on.
    f.send("{\"id\": 10, \"src\": \"print(1)\", \"on_eror\": \"continue\"}\n");
    let r = f.next();
    assert_eq!(r["kind"], "invalid");
    assert_eq!(r["error"]["code"], "REQUEST_INVALID");
    assert_eq!(r["error"]["detail"]["did_you_mean"], json!(["on_error"]));
    assert_valid(&p["response"], &r);
    assert_eq!(f.next()["id"], 10);
    let (status, rest) = f.finish();
    assert!(status.success());
    assert_eq!(rest.len(), 1);
    assert_valid(&p["events"]["end"], &rest[0]);
    assert_eq!(rest[0]["stats"]["statements"], 6);
}

#[test]
fn serve_fail_fast_stops_after_the_failed_request() {
    let o = run(
        &[
            "serve",
            "--jsonl",
            "--fail-fast",
            "--no-ready",
            "--no-end-event",
        ],
        "{\"id\": 1, \"src\": \"bpy.ops.mesh.nope()\"}\n{\"id\": 2, \"src\": \"print(1)\"}\n",
    );
    assert_eq!(o.status.code(), Some(1));
    let v = json_lines(&o);
    assert_eq!(v.len(), 2);
    assert_eq!(
        v[1],
        json!({"id": 1, "event": "done", "ok": false,
                            "counts": {"finished": 0, "error": 1, "cancelled": 0, "skipped": 0}})
    );
}

fn rpc(f: &mut Live, id: i64, method: &str, params: J) -> J {
    f.send(&format!(
        "{}\n",
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    ));
    let r = f.next();
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], id);
    r
}

#[test]
fn mcp_handshake_tools_list_and_persistent_state() {
    for extra in [
        &[][..],
        &["--tool-namespaces", "mesh,object", "--implemented-only"],
    ] {
        let mut args = vec!["mcp"];
        args.extend_from_slice(extra);
        let mut f = Live::spawn(&args);
        // stdout carries JSON-RPC only: no ready event.
        let r = rpc(
            &mut f,
            1,
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "m1_cli", "version": "1"}}),
        );
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "frap");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        f.send("{\"jsonrpc\": \"2.0\", \"method\": \"notifications/initialized\"}\n");

        // tools/list == --describe=tools-anthropic (same flags), with MCP's key name.
        let listed = rpc(&mut f, 2, "tools/list", json!({}))["result"]["tools"].clone();
        let mut dargs = vec!["--describe=tools-anthropic"];
        dargs.extend_from_slice(extra);
        let o = run(&dargs, "");
        assert!(o.status.success());
        let anthropic: J = serde_json::from_slice(&o.stdout).unwrap();
        let renamed: Vec<J> = anthropic
            .as_array()
            .unwrap()
            .iter()
            .map(|t| json!({"name": t["name"], "description": t["description"], "inputSchema": t["input_schema"]}))
            .collect();
        assert_eq!(listed, J::Array(renamed), "{extra:?}");
        let o = run(&[&["--describe=tools-mcp"][..], extra].concat(), "");
        let mcp: J = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(listed, mcp["tools"]);

        // tools/call keeps the session alive across calls.
        let r = rpc(
            &mut f,
            3,
            "tools/call",
            json!({"name": "frap_exec", "arguments": {"statements": "bpy.ops.mesh.primitive_cube_add(location=(0, 0, 2))"}}),
        );
        assert_eq!(r["result"]["isError"], false);
        let r = rpc(
            &mut f,
            4,
            "tools/call",
            json!({"name": "frap_exec", "arguments": {"statements": "bpy.data.objects['Cube.001'].location"}}),
        );
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        let lines: Vec<J> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[0]["value"], json!([0, 0, 2]));
        assert_eq!(lines[0]["seq"], 2);
        assert_eq!(lines[1]["event"], "done");
        assert_eq!(lines[1]["id"], 4);

        let r = rpc(
            &mut f,
            5,
            "tools/call",
            json!({"name": "frap_describe", "arguments": {"target": "Object.location"}}),
        );
        assert_eq!(r["result"]["isError"], false);
        let r = rpc(&mut f, 6, "nope/nope", json!({}));
        assert_eq!(r["error"]["code"], -32601);
        let (status, rest) = f.finish();
        assert!(status.success());
        assert!(rest.is_empty());
    }
}
