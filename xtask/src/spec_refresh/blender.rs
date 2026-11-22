// SPDX-License-Identifier: GPL-3.0-or-later
//! Locating, identifying and running Blender 5.2.x for the spec refresh (WP 8.9).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::compare::Family;

/// Same lookup as the golden harness (`frap-golden` `locate_from`).
pub const BLENDER_ENV: &str = "BLENDER_BIN";
const MACOS_DEFAULT: &str = "Applications/Blender-5.2.app/Contents/MacOS/Blender";

/// `--blender <path>`, else `$BLENDER_BIN` (if it names a file), else `blender` on `PATH`, else
/// `~/Applications/Blender-5.2.app` (macOS).
pub fn locate_from(
    explicit: Option<PathBuf>,
    blender_bin: Option<OsString>,
    path: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return if p.is_file() {
            Ok(p)
        } else {
            Err(format!("--blender {}: no such file", p.display()))
        };
    }
    if let Some(p) = blender_bin.map(PathBuf::from)
        && p.is_file()
    {
        return Ok(p);
    }
    let exe = if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    };
    if let Some(path) = path {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(exe);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    if let Some(h) = home {
        let p = PathBuf::from(h).join(MACOS_DEFAULT);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(format!(
        "Blender 5.2 not found: pass --blender <path>, set {BLENDER_ENV}, put `blender` on PATH \
         or install ~/{MACOS_DEFAULT}"
    ))
}

/// [`locate_from`] for this process.
pub fn locate(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    locate_from(
        explicit,
        std::env::var_os(BLENDER_ENV),
        std::env::var_os("PATH"),
        std::env::var_os("HOME"),
    )
}

/// What `blender --version` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// `5.2.4 LTS` (as `bpy.app.version_string`).
    pub version: String,
    /// (major, minor, patch)
    pub number: (u32, u32, u32),
    /// `cd5dc428cce1`
    pub build_hash: String,
}

/// Parses `blender --version`: a `Blender 5.2.4 LTS` line and a `build hash: …` line. Only
/// 5.2.x is accepted (the spec directory is `spec/blender-5.2/`).
pub fn parse_version(out: &str) -> Result<Version, String> {
    let line = out
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("Blender "))
        .ok_or_else(|| format!("`--version` printed no `Blender <version>` line:\n{out}"))?;
    let version = line.trim().to_owned();
    let num = version.split_whitespace().next().unwrap_or("");
    let parts: Vec<u32> = num
        .split('.')
        .map(|p| p.parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("cannot read the Blender version `{version}`"))?;
    let number = match parts[..] {
        [a, b, c] => (a, b, c),
        [a, b] => (a, b, 0),
        _ => return Err(format!("cannot read the Blender version `{version}`")),
    };
    if (number.0, number.1) != (5, 2) {
        return Err(format!(
            "spec-refresh needs Blender 5.2.x (spec/blender-5.2/), this is Blender {version}"
        ));
    }
    let build_hash = out
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("build hash:"))
        .map(|h| h.trim().to_owned())
        .ok_or_else(|| format!("`--version` printed no `build hash:` line:\n{out}"))?;
    Ok(Version {
        version,
        number,
        build_hash,
    })
}

/// Runs `blender --version`.
pub fn probe(blender: &Path) -> Result<Version, String> {
    let out = Command::new(blender)
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", blender.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} --version failed ({}): {}",
            blender.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    parse_version(&String::from_utf8_lossy(&out.stdout))
}

/// The command line of one dump: `-b --factory-startup --python-exit-code 1 [--python <seed>]
/// --python tools/blender/dump_<family>.py -- --out <out>/`.
pub fn dump_args(root: &Path, family: Family, seed: Option<&Path>, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-b", "--factory-startup", "--python-exit-code", "1"]
        .iter()
        .map(OsString::from)
        .collect();
    if let Some(s) = seed {
        a.push("--python".into());
        a.push(s.into());
    }
    a.push("--python".into());
    a.push(
        root.join("tools/blender")
            .join(format!("dump_{}.py", family.script()))
            .into(),
    );
    a.push("--".into());
    a.push("--out".into());
    // The scripts join `<out>/<family>`; a trailing separator is accepted too.
    a.push(out.into());
    a
}

/// Runs one dump script, writing Blender's output to `log`.
pub fn run_dump(
    blender: &Path,
    root: &Path,
    family: Family,
    seed: Option<&Path>,
    out: &Path,
    log: &Path,
) -> Result<(), String> {
    let args = dump_args(root, family, seed, out);
    let res = Command::new(blender)
        .args(&args)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", blender.display()))?;
    let mut text = res.stdout.clone();
    text.extend_from_slice(&res.stderr);
    let _ = std::fs::write(log, &text);
    if !res.status.success() {
        let t = String::from_utf8_lossy(&text);
        let tail: Vec<&str> = t.lines().rev().take(30).collect();
        return Err(format!(
            "dump_{}.py failed ({}); log {}:\n{}",
            family.script(),
            res.status,
            log.display(),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    Ok(())
}
