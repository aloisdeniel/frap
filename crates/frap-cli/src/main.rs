// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap`: reads statements (bpy-line or short form) and answers with one JSON object per
//! statement on stdout (ARCHITECTURE §13.1). Only this crate writes to stdout (rule 2.2.7);
//! logs go to stderr (§21.5).
//!
//! Transports over the same `Session`:
//! * raw stdin (default), `frap run <file>`, `frap -c "<statements>"` ([`raw`]);
//! * `frap serve --jsonl` ([`serve`]);
//! * `frap mcp` ([`mcp`]).
//!
//! `frap --describe[=<target>]` prints the API description (§14) without starting a session.
//!
//! Exit codes (§13.1): 0 normally (errors are reported in-band); 1 when `--fail-fast` stopped
//! after a failed statement (serve: a failed request); 2 on usage errors or failure to start.
#![deny(unsafe_code)]

mod cli;
mod mcp;
mod out;
mod raw;
mod serve;

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use clap::Parser;
use frap_schema::manifest::{Target, describe};
use frap_session::{Session, Startup};

use crate::cli::{Cli, LogFormat, Mode};
use crate::out::{Out, end_event, ready_event};
use crate::raw::Flow;

/// Exit code for usage errors and failure to start.
const EXIT_USAGE: u8 = 2;

fn init_logging(cli: &Cli) -> Result<(), String> {
    use tracing_subscriber::EnvFilter;
    let filter = match &cli.log.log_level {
        Some(f) => EnvFilter::try_new(f).map_err(|e| format!("invalid --log-level '{f}': {e}"))?,
        // wgpu-hal logs adapter probing failures (e.g. "vkCreateInstance: Found no drivers!")
        // at ERROR level; the GPU_UNAVAILABLE response already says it, so they are off by
        // default (`--log-level`/`FRAP_LOG` can turn them back on).
        None => EnvFilter::try_from_env("FRAP_LOG")
            .unwrap_or_else(|_| EnvFilter::new("warn,wgpu_hal=off")),
    };
    let b = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_ansi(false);
    let r = match cli.log.log_format {
        LogFormat::Text => b.try_init(),
        LogFormat::Json => b.json().try_init(),
    };
    r.map_err(|e| format!("cannot initialise logging: {e}"))
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("frap: {msg}\n\nFor more information, try '--help'.");
    ExitCode::from(EXIT_USAGE)
}

/// `frap --describe[=target]`: the JSON on stdout (pretty-printed), exit 0; an unknown
/// target is a usage error (message and suggestions on stderr, exit 2).
fn run_describe(cli: &Cli, target: &str) -> io::Result<ExitCode> {
    frap_ops_all::link();
    let opts = cli.describe_options();
    match Target::parse(target).and_then(|t| describe(&t, &opts)) {
        Ok(v) => {
            let mut w = BufWriter::new(io::stdout().lock());
            serde_json::to_writer_pretty(&mut w, &v)?;
            w.write_all(b"\n")?;
            w.flush()?;
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            let msg = e.to_string();
            eprintln!(
                "frap: --describe: {}",
                msg.strip_prefix("frap: ").unwrap_or(&msg)
            );
            Ok(ExitCode::from(EXIT_USAGE))
        }
    }
}

/// Starts the session; on failure prints why and returns the exit code (2).
fn start_session(cli: &Cli) -> Result<Session, ExitCode> {
    let opts = cli.session_options();
    if let Startup::File(path) = &opts.startup
        && let Err(e) = std::fs::File::open(path)
    {
        eprintln!("frap: cannot read --file {}: {e}", path.display());
        return Err(ExitCode::from(EXIT_USAGE));
    }
    Session::new(opts).map_err(|e| {
        eprintln!(
            "frap: cannot start: {}: {}",
            e.exception.as_str(),
            e.message
        );
        ExitCode::from(EXIT_USAGE)
    })
}

fn run(cli: &Cli, mode: Mode) -> io::Result<ExitCode> {
    if let Mode::Describe(t) = &mode {
        return run_describe(cli, t);
    }
    // A script that cannot be read is a failure to start: nothing is printed on stdout.
    let script = match &mode {
        Mode::File(path) => match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("frap: cannot read {}: {e}", path.display());
                return Ok(ExitCode::from(EXIT_USAGE));
            }
        },
        _ => None,
    };
    let mut session = match start_session(cli) {
        Ok(s) => s,
        Err(code) => return Ok(code),
    };
    if mode == Mode::Mcp {
        let mut server = match mcp::Server::new(session, cli.describe_options()) {
            Ok(s) => s,
            Err(e) => return Ok(usage_error(&format!("--tool-namespaces: {e}"))),
        };
        mcp::run(&mut server, io::stdin().lock(), io::stdout().lock())?;
        return Ok(ExitCode::SUCCESS);
    }
    let fail_fast = cli.session.fail_fast;
    let mut out = Out::new(io::stdout().lock());
    if !cli.output.no_ready {
        out.line(&ready_event(&cli.startup_label()))?;
    }
    let flow = match mode {
        Mode::Stdin => raw::run_lines(&mut session, &mut out, io::stdin().lock(), fail_fast)?,
        Mode::Inline(src) => raw::run_unit(&mut session, &mut out, &src, 1, fail_fast)?,
        Mode::File(_) => raw::run_bytes(
            &mut session,
            &mut out,
            script.as_deref().unwrap_or_default(),
            fail_fast,
        )?,
        Mode::Serve => serve::run(&mut session, &mut out, io::stdin().lock(), fail_fast)?,
        Mode::Describe(_) | Mode::Mcp => unreachable!("handled above"),
    };
    if !cli.output.no_end_event {
        let end = end_event(&out.stats);
        out.line(&end)?;
    }
    Ok(match flow {
        Flow::Stopped => ExitCode::from(1),
        Flow::Done => ExitCode::SUCCESS,
    })
}

fn main() -> ExitCode {
    // clap prints --help/--version on stdout (exit 0) and usage errors on stderr (exit 2).
    let cli = Cli::parse();
    let mode = match cli.mode() {
        Ok(m) => m,
        Err(e) => return usage_error(&e),
    };
    if let Err(e) = init_logging(&cli) {
        return usage_error(&e);
    }
    match run(&cli, mode) {
        Ok(c) => c,
        // The reader of stdout went away: nothing left to say.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("frap: I/O error: {e}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}
