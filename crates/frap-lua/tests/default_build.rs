// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 8.10 acceptance (§22.10): the default build and the `cc`-only container are unaffected
//! by the spike. Runs with and without the `lua` feature.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn manifest(path: &Path) -> toml::Table {
    std::fs::read_to_string(path).unwrap().parse().unwrap()
}

#[test]
fn has_lua_follows_the_feature() {
    assert_eq!(frap_lua::HAS_LUA, cfg!(feature = "lua"));
}

#[test]
fn the_lua_feature_is_off_by_default_and_owns_every_native_dependency() {
    let m = manifest(&Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"));
    let features = m["features"].as_table().unwrap();
    assert_eq!(features["default"].as_array().unwrap().len(), 0);
    let lua: Vec<&str> = features["lua"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(lua.contains(&"dep:mlua"), "{lua:?}");
    // Every normal dependency but the session is optional (pulled in by `lua` only).
    for (name, dep) in m["dependencies"].as_table().unwrap() {
        let optional = dep
            .as_table()
            .and_then(|t| t.get("optional"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        assert!(
            optional || name == "frap-session",
            "{name} is not optional: the default build must stay frap-session + one constant"
        );
    }
}

#[test]
fn no_workspace_crate_depends_on_frap_lua() {
    let root = manifest(&repo().join("Cargo.toml"));
    for member in root["workspace"]["members"].as_array().unwrap() {
        let member = member.as_str().unwrap();
        if member.ends_with("frap-lua") {
            continue;
        }
        let path = repo().join(member).join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let m: toml::Table = text.parse().unwrap();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(deps) = m.get(section).and_then(toml::Value::as_table) {
                assert!(
                    !deps.contains_key("frap-lua"),
                    "{member} [{section}] names frap-lua; nothing may depend on the spike"
                );
            }
        }
    }
}

/// `cargo tree` of the default `frap` binary and of `frap-lua` without features: no Lua, no
/// `cc`-built Lua sources (the cc-only container builds exactly these graphs).
#[test]
fn the_default_graphs_have_no_lua() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    for (pkg, extra) in [
        ("frap-cli", &[][..]),
        ("frap-lua", &["--no-default-features"][..]),
    ] {
        let out = Command::new(&cargo)
            .current_dir(repo())
            .args(["tree", "--locked", "--offline", "-e", "normal,build"])
            .args(["--prefix", "none", "-p", pkg])
            .args(extra)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "cargo tree -p {pkg}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let tree = String::from_utf8(out.stdout).unwrap();
        for crate_ in ["mlua ", "mlua-sys ", "lua-src ", "luau0-src "] {
            assert!(
                !tree.lines().any(|l| l.starts_with(crate_)),
                "{pkg}'s default graph contains {crate_}"
            );
        }
        if pkg == "frap-cli" {
            assert!(
                !tree.lines().any(|l| l.starts_with("frap-lua ")),
                "frap-cli links frap-lua"
            );
        }
    }
}
