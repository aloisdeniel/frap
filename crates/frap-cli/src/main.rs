// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap`: reads statements (bpy-line or short form) and answers with one JSON object per
//! statement on stdout (ARCHITECTURE §13.1). Only this crate writes to stdout (rule 2.2.7).
//!
//! M0 modes: raw stdin (default), `frap run <file>`, `frap -c "<statements>"`.
//! `frap serve --jsonl` and `frap mcp` arrive in M1.
#![forbid(unsafe_code)]

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use frap_rna::{ErrorCode, FrapError, PyExc};
use frap_session::{Response, Session, SessionOptions, Status};
use serde_json::json;

const USAGE: &str = "\
frap: create and edit 3D models headlessly with Blender 5.2 terminology

USAGE:
    frap [OPTIONS]                  read statements from stdin
    frap [OPTIONS] run <FILE>       run a script file
    frap [OPTIONS] -c <STATEMENTS>  run statements given on the command line

OPTIONS:
    --no-ready        do not print the ready event
    --no-end-event    do not print the end event
    --fail-fast       stop after the first failing statement (exit code 1)
    -h, --help        print this help
    -V, --version     print the version
";

#[derive(Default)]
struct Opts {
    no_ready: bool,
    no_end: bool,
    fail_fast: bool,
    source: Source,
}

#[derive(Default)]
enum Source {
    #[default]
    Stdin,
    File(String),
    Inline(String),
}

fn parse_args() -> Result<Option<Opts>, String> {
    let mut o = Opts::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--no-ready" => o.no_ready = true,
            "--no-end-event" => o.no_end = true,
            "--fail-fast" => o.fail_fast = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("frap {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-c" => o.source = Source::Inline(it.next().ok_or("-c needs an argument")?),
            "run" => o.source = Source::File(it.next().ok_or("run needs a file argument")?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(Some(o))
}

struct Out {
    stdout: io::StdoutLock<'static>,
    statements: u64,
    errors: u64,
    cancelled: u64,
}

impl Out {
    fn emit(&mut self, r: &Response) -> io::Result<()> {
        self.statements += 1;
        match r.status {
            Status::Error => self.errors += 1,
            Status::Cancelled => self.cancelled += 1,
            _ => {}
        }
        serde_json::to_writer(&mut self.stdout, &r.to_json())?;
        self.stdout.write_all(b"\n")?;
        self.stdout.flush()
    }
    fn event(&mut self, v: serde_json::Value) -> io::Result<()> {
        serde_json::to_writer(&mut self.stdout, &v)?;
        self.stdout.write_all(b"\n")?;
        self.stdout.flush()
    }
}

/// Feeds `src` (starting at `first_line`) and emits responses; returns false to stop (fail-fast).
fn run_chunk(
    session: &mut Session,
    out: &mut Out,
    src: &str,
    first_line: u32,
    fail_fast: bool,
) -> io::Result<bool> {
    for r in session.exec_source_at(src, first_line) {
        out.emit(&r)?;
        if fail_fast && !r.ok() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn run(o: Opts) -> io::Result<ExitCode> {
    let mut session = match Session::new(SessionOptions::default()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("frap: cannot start: {e}");
            return Ok(ExitCode::from(2));
        }
    };
    let mut out = Out {
        stdout: io::stdout().lock(),
        statements: 0,
        errors: 0,
        cancelled: 0,
    };
    if !o.no_ready {
        out.event(
            json!({"event": "ready", "frap": env!("CARGO_PKG_VERSION"), "protocol": 1,
                         "blender_api": "5.2", "startup": "factory"}),
        )?;
    }
    let mut keep_going = true;
    match o.source {
        Source::Inline(src) => {
            keep_going = run_chunk(&mut session, &mut out, &src, 1, o.fail_fast)?
        }
        Source::File(path) => match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(src) => keep_going = run_chunk(&mut session, &mut out, &src, 1, o.fail_fast)?,
                Err(_) => {
                    eprintln!("frap: {path}: not valid UTF-8");
                    return Ok(ExitCode::from(2));
                }
            },
            Err(e) => {
                eprintln!("frap: cannot read {path}: {e}");
                return Ok(ExitCode::from(2));
            }
        },
        Source::Stdin => {
            // Raw mode (§11.1): a statement continues while brackets are open; a blank line
            // or EOF ends a pending continuation (→ SyntaxError rather than a hang).
            let stdin = io::stdin();
            let mut reader = stdin.lock();
            let mut buf = String::new();
            let mut buf_line = 1u32;
            let mut line_no = 0u32;
            let mut raw = Vec::new();
            loop {
                raw.clear();
                let n = reader.read_until(b'\n', &mut raw)?;
                if n == 0 {
                    break;
                }
                line_no += 1;
                let line = match String::from_utf8(raw.clone()) {
                    Ok(l) => l,
                    Err(_) => {
                        let e = FrapError::new(
                            PyExc::SyntaxError,
                            ErrorCode("SYNTAX"),
                            "invalid UTF-8 in input line",
                        );
                        let r = session.reject(line_no, e);
                        out.emit(&r)?;
                        if o.fail_fast {
                            keep_going = false;
                            break;
                        }
                        continue;
                    }
                };
                let blank = line.trim().is_empty();
                if buf.is_empty() {
                    if blank {
                        continue;
                    }
                    buf_line = line_no;
                }
                buf.push_str(&line);
                if blank || frap_lang::is_complete(&buf) {
                    let chunk = std::mem::take(&mut buf);
                    if !run_chunk(&mut session, &mut out, &chunk, buf_line, o.fail_fast)? {
                        keep_going = false;
                        break;
                    }
                }
            }
            if keep_going && !buf.trim().is_empty() {
                keep_going = run_chunk(&mut session, &mut out, &buf, buf_line, o.fail_fast)?;
            }
        }
    }
    if !o.no_end {
        out.event(json!({"event": "end", "ok": out.errors == 0 && out.cancelled == 0,
                         "stats": {"statements": out.statements, "errors": out.errors, "cancelled": out.cancelled}}))?;
    }
    Ok(if o.fail_fast && !keep_going {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(Some(o)) => o,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("frap: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(opts) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("frap: I/O error: {e}");
            ExitCode::from(2)
        }
    }
}
