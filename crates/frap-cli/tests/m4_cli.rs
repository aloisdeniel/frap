// SPDX-License-Identifier: GPL-3.0-or-later
//! M4 through the real `frap` binary: `--verbosity full` evaluates the active object for
//! `context.evaluated` / `context.modifier_status` after every statement (§13.2). An
//! evaluation over `--max-elements` or past `--stmt-timeout` must be reported there, never
//! unwind out of the session (M3+M4 stabilization round 2: it killed the process with exit
//! 101 and no output, also under `frap serve`).

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value as J;

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_frap"))
        .env_remove("FRAP_LOG")
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

/// The statement responses (no `event` lines, no serve `done` events).
fn responses(lines: &[J]) -> Vec<&J> {
    lines.iter().filter(|l| l.get("seq").is_some()).collect()
}

fn subsurf_status(r: &J) -> (&str, &str) {
    let st = &r["context"]["modifier_status"][0];
    assert_eq!(st["name"], "Subdivision", "{r}");
    (
        st["status"].as_str().unwrap(),
        st["message"].as_str().unwrap_or(""),
    )
}

const SUBSURF_11: &str = "object.modifier_add type=SUBSURF\n\
    context.object.modifiers['Subdivision'].levels = 11\n\
    context.object.name\n";

#[test]
fn verbosity_full_reports_a_subsurf_over_max_elements() {
    let o = run(&["--verbosity", "full"], SUBSURF_11);
    assert_eq!(
        o.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let lines = json_lines(&o);
    assert_eq!(lines.last().unwrap()["event"], "end", "{lines:?}");
    let rs = responses(&lines);
    assert_eq!(rs.len(), 3);
    for r in &rs[1..] {
        assert_eq!(r["status"], "FINISHED", "{r}");
        let (status, msg) = subsurf_status(r);
        assert_eq!(status, "ERROR", "{r}");
        assert!(msg.contains("limit exceeded (max_elements)"), "{msg}");
        // The modifier passes the mesh through (the cube's own counts).
        assert_eq!(r["context"]["evaluated"]["vertices"], 8, "{r}");
    }
}

#[test]
fn serve_verbosity_full_survives_a_subsurf_over_max_elements() {
    let stdin = "{\"id\":1,\"src\":\"object.modifier_add type=SUBSURF\"}\n\
        {\"id\":2,\"src\":\"context.object.modifiers['Subdivision'].levels = 11\"}\n\
        {\"id\":3,\"src\":\"context.object.name\"}\n";
    let o = run(&["serve", "--jsonl", "--verbosity", "full"], stdin);
    assert_eq!(
        o.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let lines = json_lines(&o);
    let rs = responses(&lines);
    assert_eq!(rs.len(), 3, "{lines:?}");
    assert_eq!(rs[2]["value"], "Cube");
    let (status, msg) = subsurf_status(rs[2]);
    assert_eq!(status, "ERROR");
    assert!(msg.contains("limit exceeded (max_elements)"), "{msg}");
}

#[test]
fn verbosity_full_reports_an_evaluation_past_stmt_timeout() {
    let stdin = "object.modifier_add type=SUBSURF\n\
        context.object.modifiers['Subdivision'].levels = 10\n\
        context.object.name\n";
    for args in [
        &["--verbosity", "full", "--stmt-timeout", "2"][..],
        &[
            "serve",
            "--jsonl",
            "--verbosity",
            "full",
            "--stmt-timeout",
            "2",
        ][..],
    ] {
        let input = if args[0] == "serve" {
            stdin
                .lines()
                .enumerate()
                .map(|(i, l)| format!("{}\n", serde_json::json!({"id": i, "src": l})))
                .collect::<String>()
        } else {
            stdin.to_owned()
        };
        let o = run(args, &input);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{args:?} stderr: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let lines = json_lines(&o);
        let rs = responses(&lines);
        assert_eq!(rs.len(), 3, "{args:?}: {lines:?}");
        let r = rs[2];
        assert_eq!(r["status"], "FINISHED", "{r}");
        let (status, msg) = subsurf_status(r);
        assert_eq!(status, "ERROR", "{r}");
        assert!(msg.contains("stmt_timeout"), "{msg}");
        assert!(r["context"].get("evaluated").is_none(), "{r}");
    }
}
