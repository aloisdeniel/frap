// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask golden bless [--filter <glob>] | live | report` (ARCHITECTURE §20.5). WP 1.8.
//!
//! xtask depends on no frap crate (it must build in seconds), so every subcommand forwards to
//! the harness itself: the `frap-golden` binary (`bless`, `check`, `report`, `diff`) or the
//! golden test suite with `FRAP_GOLDEN_LIVE=1` (`live`).
//!
//! ```text
//! cargo xtask golden bless  [--filter <glob>]  Blender -> tests/golden/fixtures (needs Blender 5.2)
//! cargo xtask golden check  [--filter <glob>]  frap vs the committed fixtures
//! cargo xtask golden live   [--filter <glob>]  `cargo test -p frap-golden` with FRAP_GOLDEN_LIVE=1
//! cargo xtask golden report [--filter <glob>]  operator x tier coverage matrix
//! cargo xtask golden diff <case.py>            frap and Blender side by side
//! cargo xtask golden fuzz [options]             the differential fuzzer (WP 8.7), release build
//! ```

use std::process::Command;

const USAGE: &str = "usage: cargo xtask golden <bless|check|live|report> [--filter <glob>] | \
                     diff <case.py> | fuzz [options]";

/// What a `golden` invocation runs: cargo arguments plus extra environment.
#[derive(Debug, PartialEq, Eq)]
struct Plan {
    cargo_args: Vec<String>,
    env: Vec<(String, String)>,
}

fn filter(rest: &[String]) -> Result<Option<String>, String> {
    match rest {
        [] => Ok(None),
        [f, g] if f == "--filter" => Ok(Some(g.clone())),
        _ => Err(USAGE.into()),
    }
}

fn plan(args: &[String]) -> Result<Plan, String> {
    let (cmd, rest) = args.split_first().ok_or(USAGE)?;
    let bin = |sub: &str, rest: &[String]| {
        let mut a: Vec<String> = [
            "run",
            "--quiet",
            "--package",
            "frap-golden",
            "--bin",
            "frap-golden",
            "--",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        a.push(sub.to_owned());
        a.extend(rest.iter().cloned());
        a
    };
    match cmd.as_str() {
        "bless" | "check" | "report" => {
            filter(rest)?;
            Ok(Plan {
                cargo_args: bin(cmd, rest),
                env: vec![],
            })
        }
        "diff" if rest.len() == 1 => Ok(Plan {
            cargo_args: bin(cmd, rest),
            env: vec![],
        }),
        // WP 8.7: thousands of frap runs per hour, so a release build; the options are
        // checked by `frap-golden fuzz` itself.
        "fuzz" => {
            let mut a = bin(cmd, rest);
            a.insert(1, "--release".to_owned());
            Ok(Plan {
                cargo_args: a,
                env: vec![],
            })
        }
        "live" => {
            let mut env = vec![("FRAP_GOLDEN_LIVE".to_owned(), "1".to_owned())];
            if let Some(f) = filter(rest)? {
                env.push(("FRAP_GOLDEN_FILTER".to_owned(), f));
            }
            Ok(Plan {
                cargo_args: [
                    "test",
                    "--package",
                    "frap-golden",
                    "--test",
                    "golden",
                    "--",
                    "--include-ignored",
                    "--nocapture",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                env,
            })
        }
        _ => Err(USAGE.into()),
    }
}

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    let p = plan(args)?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args(&p.cargo_args)
        .envs(p.env.iter().map(|(k, v)| (k, v)))
        .current_dir(crate::workspace_root())
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed ({status})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn plans() {
        let p = plan(&s(&["bless", "--filter", "harness/*"])).unwrap();
        assert_eq!(
            &p.cargo_args[..6],
            &s(&[
                "run",
                "--quiet",
                "--package",
                "frap-golden",
                "--bin",
                "frap-golden"
            ])[..]
        );
        assert_eq!(
            &p.cargo_args[6..],
            &s(&["--", "bless", "--filter", "harness/*"])[..]
        );
        let p = plan(&s(&["live", "--filter", "x/**"])).unwrap();
        assert_eq!(p.cargo_args[0], "test");
        assert!(p.cargo_args.contains(&"--include-ignored".to_owned()));
        assert!(p.env.contains(&("FRAP_GOLDEN_LIVE".into(), "1".into())));
        assert!(
            p.env
                .contains(&("FRAP_GOLDEN_FILTER".into(), "x/**".into()))
        );
        assert!(plan(&s(&["report"])).is_ok());
        assert!(plan(&s(&["diff", "a.py"])).is_ok());
        let p = plan(&s(&["fuzz", "--minutes", "60", "--seed", "7"])).unwrap();
        assert_eq!(
            p.cargo_args,
            s(&[
                "run",
                "--release",
                "--quiet",
                "--package",
                "frap-golden",
                "--bin",
                "frap-golden",
                "--",
                "fuzz",
                "--minutes",
                "60",
                "--seed",
                "7"
            ])
        );
        assert!(plan(&s(&["fuzz"])).is_ok());
        for bad in [
            &[][..],
            &["nope"][..],
            &["bless", "--filter"][..],
            &["diff"][..],
        ] {
            assert!(plan(&s(bad)).is_err(), "{bad:?}");
        }
    }
}
