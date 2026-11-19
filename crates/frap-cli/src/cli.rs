// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line arguments (ARCHITECTURE §13.1 transports and flags, §12 limits, §14
//! `--describe`, §21.5 logging) and their translation into `SessionOptions` and
//! `DescribeOptions`. Usage errors exit with code 2 (§13.1).

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use frap_schema::manifest::DescribeOptions;
use frap_session::{SessionOptions, Startup, Verbosity};

/// frap: create and edit 3D models headlessly with Blender 5.2 terminology.
///
/// Reads statements (bpy-style `bpy.ops.mesh.primitive_cube_add(size=2)` or short form
/// `mesh.primitive_cube_add size=2`) and answers each one with a JSON object on stdout.
#[derive(Parser, Debug)]
#[command(
    name = "frap",
    // Usage lines say `frap` on every OS (not `frap.exe` from argv[0] on Windows).
    bin_name = "frap",
    version,
    after_help = "\
Transports:
  frap                       statements on stdin, one JSON line per statement
  frap run <FILE>            run a script file
  frap -c <STATEMENTS>       run statements given on the command line
  frap serve --jsonl         one JSON request per line: {\"id\": 1, \"src\": \"...\", \"on_error\": \"skip_rest\"}
  frap mcp                   MCP server on stdio (tools frap_exec, frap_describe)

Describe the API (no scene needed):
  frap --describe                          manifest
  frap --describe op:mesh.bevel            one operator (also struct:Object, Object.location)
  frap --describe --filter 'mesh.*' --implemented-only
  frap --describe=tools-anthropic          also tools-openai, tools-mcp, jsonschema, protocol, errors

Exit codes: 0 normally (errors are reported in-band), 1 when --fail-fast stopped after a
failed statement, 2 on usage errors or failure to start."
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,

    /// Run these statements (newline- or ';'-separated) instead of reading stdin.
    #[arg(short = 'c', value_name = "STATEMENTS")]
    pub command: Option<String>,

    #[command(flatten)]
    pub session: SessionArgs,

    #[command(flatten)]
    pub output: OutputArgs,

    #[command(flatten)]
    pub describe: DescribeArgs,

    #[command(flatten)]
    pub log: LogArgs,
}

/// Subcommands.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// Run a script file (a complete unit: brackets may span lines, blank lines included).
    Run {
        /// The script (UTF-8 statements).
        #[arg(id = "script", value_name = "FILE")]
        script: PathBuf,
    },
    /// Serve requests from a long-lived process (agents, harnesses).
    Serve {
        /// JSON Lines framing: one request {"id", "src", "on_error"} per input line, one
        /// response per statement, then {"id", "event": "done", ...} (the only framing).
        #[arg(long)]
        jsonl: bool,
    },
    /// MCP server over stdio (JSON-RPC 2.0): initialize, tools/list, tools/call.
    Mcp,
}

/// Session options (§12, §13.1, §13.3).
#[derive(Args, Debug, Clone)]
pub struct SessionArgs {
    /// Start from an empty scene (wm.read_homefile(use_empty=True)) instead of the factory
    /// startup.
    #[arg(long, global = true, conflicts_with = "file")]
    pub empty: bool,

    /// Start from a .frap project.
    #[arg(long, global = true, value_name = "PATH.frap")]
    pub file: Option<PathBuf>,

    /// Response verbosity: minimal drops context and edit_mesh; full adds evaluated counts.
    #[arg(long, global = true, value_enum, default_value_t = VerbosityArg::Normal)]
    pub verbosity: VerbosityArg,

    /// Names listed in a collection value ($coll items).
    #[arg(long, global = true, value_name = "N", default_value_t = 100)]
    pub max_items: usize,

    /// Scalars returned by a bulk read ($array).
    #[arg(long, global = true, value_name = "N", default_value_t = 65_536)]
    pub max_array: usize,

    /// Largest mesh a statement may create, in face corners (LIMIT_EXCEEDED above it).
    #[arg(long, global = true, value_name = "N", default_value_t = 50_000_000)]
    pub max_elements: u64,

    /// Statement time limit (`30`, `30s`, `500ms`, `1.5`); none by default.
    #[arg(long, global = true, value_name = "DURATION", value_parser = parse_duration)]
    pub stmt_timeout: Option<Duration>,

    /// Stop after the first failing statement (serve: after the first failing request) with
    /// exit code 1.
    #[arg(long, global = true)]
    pub fail_fast: bool,
}

/// Stream events (§13.1).
#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    /// Do not print the ready event.
    #[arg(long, global = true)]
    pub no_ready: bool,

    /// Do not print the end event.
    #[arg(long, global = true)]
    pub no_end_event: bool,
}

/// `--describe` (§14).
#[derive(Args, Debug, Clone)]
pub struct DescribeArgs {
    /// Print the API description and exit: manifest (default), jsonschema, tools-anthropic,
    /// tools-openai, tools-mcp, protocol, errors, op:IDNAME (op:mesh.bevel), struct:STRUCT
    /// (struct:Object) or STRUCT.PROP (Object.location).
    #[arg(
        long,
        value_name = "TARGET",
        num_args = 0..=1,
        default_missing_value = "manifest",
        require_equals = false
    )]
    pub describe: Option<String>,

    /// Comma-separated globs over operator idnames ('mesh.*').
    #[arg(long, value_name = "GLOBS", requires = "describe")]
    pub filter: Option<String>,

    /// Only operators frap implements (manifest, jsonschema, and the per-namespace tools).
    #[arg(long, global = true)]
    pub implemented_only: bool,

    /// Also emit one tool per operator namespace (frap_ops_mesh, ...) in the tools-* targets
    /// and in `frap mcp`'s tools/list.
    #[arg(
        long,
        global = true,
        value_name = "NS,...",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub tool_namespaces: Vec<String>,
}

/// Logging (§21.5): stderr only.
#[derive(Args, Debug, Clone)]
pub struct LogArgs {
    /// Log filter (EnvFilter syntax, e.g. `info` or `frap_session=debug`); overrides FRAP_LOG
    /// (default warn).
    #[arg(long, global = true, value_name = "FILTER")]
    pub log_level: Option<String>,

    /// Log line format on stderr.
    #[arg(long, global = true, value_enum, default_value_t = LogFormat::Text)]
    pub log_format: LogFormat,
}

/// `--verbosity`.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityArg {
    /// No context or edit_mesh.
    Minimal,
    /// The default envelope.
    Normal,
    /// Adds evaluated element counts and modifier status.
    Full,
}

/// `--log-format`.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable lines.
    Text,
    /// One JSON object per log line.
    Json,
}

/// Parses `--stmt-timeout`: seconds (`30`, `1.5`, `30s`) or milliseconds (`500ms`).
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let t = s.trim();
    let (num, scale) = if let Some(n) = t.strip_suffix("ms") {
        (n, 1e-3)
    } else if let Some(n) = t.strip_suffix('s') {
        (n, 1.0)
    } else {
        (t, 1.0)
    };
    let v: f64 = num
        .trim()
        .parse()
        .map_err(|_| format!("invalid duration '{s}' (expected e.g. 30, 30s, 500ms)"))?;
    if !v.is_finite() || v <= 0.0 {
        return Err(format!("invalid duration '{s}' (must be positive)"));
    }
    Duration::try_from_secs_f64(v * scale).map_err(|e| format!("invalid duration '{s}': {e}"))
}

/// What the process does, after the cross-flag checks clap cannot express.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `--describe[=target]`.
    Describe(String),
    /// Raw stdin.
    Stdin,
    /// `frap run <file>`.
    File(PathBuf),
    /// `frap -c <statements>`.
    Inline(String),
    /// `frap serve --jsonl`.
    Serve,
    /// `frap mcp`.
    Mcp,
}

impl Cli {
    /// The selected mode, or a usage error (exit code 2).
    pub fn mode(&self) -> Result<Mode, String> {
        if let Some(t) = &self.describe.describe {
            if self.cmd.is_some() || self.command.is_some() {
                return Err("--describe cannot be combined with -c, run, serve or mcp".into());
            }
            return Ok(Mode::Describe(t.clone()));
        }
        if !self.describe.tool_namespaces.is_empty() && self.cmd != Some(Cmd::Mcp) {
            return Err("--tool-namespaces needs --describe=tools-* or frap mcp".into());
        }
        match (&self.cmd, &self.command) {
            (Some(_), Some(_)) => Err("-c cannot be combined with run, serve or mcp".into()),
            (None, Some(c)) => Ok(Mode::Inline(c.clone())),
            (None, None) => Ok(Mode::Stdin),
            (Some(Cmd::Run { script }), None) => Ok(Mode::File(script.clone())),
            (Some(Cmd::Serve { .. }), None) => Ok(Mode::Serve),
            (Some(Cmd::Mcp), None) => {
                if self.session.fail_fast {
                    Err("--fail-fast has no effect with frap mcp (every tools/call reports its own outcome)".into())
                } else {
                    Ok(Mode::Mcp)
                }
            }
        }
    }

    /// The session options these flags select.
    pub fn session_options(&self) -> SessionOptions {
        let a = &self.session;
        let mut o = SessionOptions::default();
        o.startup = if a.empty {
            Startup::Empty
        } else if let Some(f) = &a.file {
            Startup::File(f.clone())
        } else {
            Startup::Factory
        };
        o.verbosity = match a.verbosity {
            VerbosityArg::Minimal => Verbosity::Minimal,
            VerbosityArg::Normal => Verbosity::Normal,
            VerbosityArg::Full => Verbosity::Full,
        };
        o.max_items = a.max_items;
        o.max_array = a.max_array;
        o.limits.max_elements = a.max_elements;
        o.limits.stmt_timeout = a.stmt_timeout;
        o.fail_fast = a.fail_fast;
        o
    }

    /// The `startup` field of the ready event: `factory`, `empty` or the `--file` path.
    pub fn startup_label(&self) -> String {
        if self.session.empty {
            "empty".into()
        } else if let Some(f) = &self.session.file {
            f.display().to_string()
        } else {
            "factory".into()
        }
    }

    /// The describe options these flags select (also `frap mcp`'s tools/list and
    /// frap_describe).
    pub fn describe_options(&self) -> DescribeOptions {
        DescribeOptions {
            filter: self.describe.filter.clone(),
            implemented_only: self.describe.implemented_only,
            tool_namespaces: self.describe.tool_namespaces.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("frap").chain(args.iter().copied()))
    }

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn modes() {
        assert_eq!(parse(&[]).unwrap().mode().unwrap(), Mode::Stdin);
        assert_eq!(
            parse(&["-c", "print(1)"]).unwrap().mode().unwrap(),
            Mode::Inline("print(1)".into())
        );
        let run = parse(&["run", "a.py", "--no-ready"]).unwrap();
        assert_eq!(run.mode().unwrap(), Mode::File("a.py".into()));
        // The script is not the global `--file` startup project.
        assert_eq!(run.session_options().startup, Startup::Factory);
        let both = parse(&["run", "a.py", "--file", "b.frap"]).unwrap();
        assert_eq!(both.mode().unwrap(), Mode::File("a.py".into()));
        assert_eq!(both.startup_label(), "b.frap");
        assert_eq!(
            parse(&["serve", "--jsonl"]).unwrap().mode().unwrap(),
            Mode::Serve
        );
        assert_eq!(parse(&["mcp"]).unwrap().mode().unwrap(), Mode::Mcp);
        assert!(parse(&["-c", "x", "run", "a.py"]).unwrap().mode().is_err());
        assert!(parse(&["mcp", "--fail-fast"]).unwrap().mode().is_err());
        assert!(
            parse(&["--tool-namespaces", "mesh"])
                .unwrap()
                .mode()
                .is_err()
        );
        assert!(parse(&["--empty", "--file", "x.frap"]).is_err());
        assert!(parse(&["--filter", "mesh.*"]).is_err());
        assert!(parse(&["--verbosity", "loud"]).is_err());
    }

    #[test]
    fn describe_target_forms() {
        let m = |a: &[&str]| parse(a).unwrap().mode().unwrap();
        assert_eq!(m(&["--describe"]), Mode::Describe("manifest".into()));
        assert_eq!(
            m(&["--describe=tools-anthropic"]),
            Mode::Describe("tools-anthropic".into())
        );
        assert_eq!(
            m(&["--describe", "op:mesh.bevel"]),
            Mode::Describe("op:mesh.bevel".into())
        );
        assert_eq!(
            m(&["--describe", "--filter", "mesh.*", "--implemented-only"]),
            Mode::Describe("manifest".into())
        );
        let c = parse(&[
            "--describe=tools-mcp",
            "--tool-namespaces",
            "mesh,object",
            "--implemented-only",
        ])
        .unwrap();
        let o = c.describe_options();
        assert_eq!(o.tool_namespaces, vec!["mesh", "object"]);
        assert!(o.implemented_only);
        assert!(parse(&["--describe", "-c", "x"]).unwrap().mode().is_err());
    }

    #[test]
    fn session_flags() {
        let c = parse(&[
            "--verbosity",
            "minimal",
            "--max-items",
            "3",
            "--max-array",
            "9",
            "--max-elements",
            "100",
            "--stmt-timeout",
            "250ms",
            "--fail-fast",
            "--empty",
        ])
        .unwrap();
        let o = c.session_options();
        assert_eq!(o.verbosity, Verbosity::Minimal);
        assert_eq!((o.max_items, o.max_array), (3, 9));
        assert_eq!(o.limits.max_elements, 100);
        assert_eq!(o.limits.stmt_timeout, Some(Duration::from_millis(250)));
        assert!(o.fail_fast);
        assert_eq!(o.startup, Startup::Empty);
        assert_eq!(c.startup_label(), "empty");
        let d = parse(&[]).unwrap().session_options();
        assert_eq!(d.verbosity, Verbosity::Normal);
        assert_eq!((d.max_items, d.max_array), (100, 65_536));
        assert_eq!(d.limits.max_elements, 50_000_000);
        assert_eq!(d.startup, Startup::Factory);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("1.5").unwrap(), Duration::from_millis(1500));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert!(parse_duration("-1").is_err());
        assert!(parse_duration("soon").is_err());
        assert!(parse_duration("0").is_err());
    }
}
