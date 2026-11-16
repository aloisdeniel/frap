// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask check-deps` (ARCHITECTURE §3.2, rules 2.2.3 and 2.2.11). Reads `cargo metadata`
//! and fails when:
//!
//! 1. a workspace crate is missing from the layer table below, or a crate of §3.2 is missing
//!    from the workspace;
//! 2. a normal or build dependency between frap crates does not point to a strictly lower layer
//!    (the same-layer edges §3.2 itself draws are allow-listed in `SAME_LAYER_EDGES`);
//! 3. a dev-dependency closes a cycle: `A` dev-depends on `B` while `B` depends on `A` through
//!    normal/build dependencies (rule 2.2.11: two copies of `A`, duplicate `linkme` entries);
//! 4. an external dependency is used outside the crates allowed to use it (`wgpu` only in
//!    frap-render, `anyhow` only in frap-cli), or frap-render depends on frap-scene directly;
//! 5. an external dependency is declared in a member without `workspace = true` (rule 2.2.3).

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value as J;

/// Every crate of ARCHITECTURE §3.2 with its layer, plus the test-only `frap-ops-<x>-tests`
/// crates (§3.1). `xtask` is tooling and has no layer.
pub const LAYERS: &[(&str, u8)] = &[
    ("frap-math", 0),
    ("frap-macros", 0),
    ("frap-rna", 1),
    ("frap-geom", 1),
    ("frap-color", 1),
    ("frap-lang", 1),
    ("frap-spec-rna", 2),
    ("frap-spec-ops-object", 2),
    ("frap-spec-ops-transform", 2),
    ("frap-spec-ops-mesh", 2),
    ("frap-spec-ops-uv", 2),
    ("frap-spec-ops-anim", 2),
    ("frap-spec-ops-data", 2),
    ("frap-spec-ops-io", 2),
    ("frap-spec-ops-render", 2),
    ("frap-spec-ops-frap", 2),
    ("frap-spec-catalog", 2),
    ("frap-anim", 3),
    ("frap-gltf", 3),
    ("frap-imbuf", 3),
    ("frap-codec-mesh", 3),
    ("frap-uv", 3),
    ("frap-subdiv", 3),
    ("frap-boolean", 3),
    ("frap-mesh", 4),
    ("frap-nodes", 4),
    ("frap-bmesh", 5),
    ("frap-scene", 6),
    ("frap-modifiers", 7),
    ("frap-material", 7),
    ("frap-eval", 8),
    ("frap-render-scene", 8),
    ("frap-op", 9),
    ("frap-render", 9),
    ("frap-io-gltf", 9),
    ("frap-project", 9),
    ("frap-ops-object", 10),
    ("frap-ops-transform", 10),
    ("frap-ops-mesh", 10),
    ("frap-ops-uv", 10),
    ("frap-ops-anim", 10),
    ("frap-ops-data", 10),
    ("frap-ops-io", 10),
    ("frap-ops-render", 10),
    ("frap-ops-frap", 10),
    ("frap-ops-all", 10),
    ("frap-session", 11),
    ("frap-schema", 11),
    ("frap-cli", 12),
    ("frap-golden", 12),
    // Test-only crates (§3.1, rule 2.2.11): they depend on frap-session; nothing depends on them.
    ("frap-ops-io-tests", 12),
];

/// Same-layer edges that §3.2 itself draws: `frap-lang` "depends only on frap-rna" (both L1),
/// and the `frap-ops-all` aggregator depends on every `frap-ops-*` (all L10).
const SAME_LAYER_EDGES: &[(&str, &str)] = &[
    ("frap-lang", "frap-rna"),
    ("frap-ops-all", "frap-ops-object"),
    ("frap-ops-all", "frap-ops-transform"),
    ("frap-ops-all", "frap-ops-mesh"),
    ("frap-ops-all", "frap-ops-uv"),
    ("frap-ops-all", "frap-ops-anim"),
    ("frap-ops-all", "frap-ops-data"),
    ("frap-ops-all", "frap-ops-io"),
    ("frap-ops-all", "frap-ops-render"),
    ("frap-ops-all", "frap-ops-frap"),
];

/// External crates restricted to a set of frap crates.
const RESTRICTED: &[(&str, &[&str], &str)] = &[
    (
        "wgpu",
        &["frap-render"],
        "frap-render is the only crate that depends on wgpu (§3.2)",
    ),
    (
        "anyhow",
        &["frap-cli", "xtask"],
        "only frap-cli uses anyhow (§13.4)",
    ),
];

/// Crate pairs that must not be direct dependencies.
const FORBIDDEN_EDGES: &[(&str, &str, &str)] = &[(
    "frap-render",
    "frap-scene",
    "frap-render uses only frap_render_scene::types (§3.2)",
)];

#[derive(Debug, Default)]
struct Pkg {
    deps: BTreeSet<String>,
    dev_deps: BTreeSet<String>,
    external: BTreeSet<String>,
}

fn metadata() -> Result<J, String> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(crate::workspace_root())
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))
}

/// Raw `[dependencies]` tables of a member manifest, to check `workspace = true` (rule 2.2.3).
fn non_workspace_deps(manifest: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let doc: toml::Table = text.parse().map_err(|e| format!("{manifest}: {e}"))?;
    let mut bad = Vec::new();
    for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = doc.get(table).and_then(|t| t.as_table()) else {
            continue;
        };
        for (name, spec) in deps {
            let inherited = spec
                .as_table()
                .and_then(|t| t.get("workspace"))
                .and_then(|w| w.as_bool())
                == Some(true);
            if !inherited {
                bad.push(format!("{table}.{name}"));
            }
        }
    }
    Ok(bad)
}

/// Runs every check; returns the list of violations.
pub fn violations() -> Result<Vec<String>, String> {
    let meta = metadata()?;
    let layers: BTreeMap<&str, u8> = LAYERS.iter().copied().collect();
    let members: BTreeSet<String> = meta["workspace_members"]
        .as_array()
        .ok_or("cargo metadata: no workspace_members")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let mut pkgs: BTreeMap<String, Pkg> = BTreeMap::new();
    let mut manifests: BTreeMap<String, String> = BTreeMap::new();
    for p in meta["packages"]
        .as_array()
        .ok_or("cargo metadata: no packages")?
    {
        let id = p["id"].as_str().unwrap_or_default();
        if !members.contains(id) {
            continue;
        }
        let name = p["name"].as_str().unwrap_or_default().to_owned();
        let mut pkg = Pkg::default();
        for d in p["dependencies"].as_array().into_iter().flatten() {
            let dn = d["name"].as_str().unwrap_or_default().to_owned();
            let is_dev = d["kind"].as_str() == Some("dev");
            if dn.starts_with("frap-") {
                if is_dev {
                    pkg.dev_deps.insert(dn);
                } else {
                    pkg.deps.insert(dn);
                }
            } else if !is_dev {
                pkg.external.insert(dn);
            }
        }
        manifests.insert(
            name.clone(),
            p["manifest_path"].as_str().unwrap_or_default().to_owned(),
        );
        pkgs.insert(name, pkg);
    }

    let mut v = Vec::new();
    // 1. Layer table <-> workspace.
    for name in pkgs.keys() {
        if name != "xtask" && !layers.contains_key(name.as_str()) {
            v.push(format!(
                "{name}: workspace crate missing from the §3.2 layer table"
            ));
        }
    }
    for (name, _) in LAYERS {
        if !pkgs.contains_key(*name) {
            v.push(format!("{name}: crate of §3.2 is not a workspace member"));
        }
    }
    // 2. Normal/build deps point to strictly lower layers.
    for (name, pkg) in &pkgs {
        let Some(&l) = layers.get(name.as_str()) else {
            continue;
        };
        for d in &pkg.deps {
            let Some(&dl) = layers.get(d.as_str()) else {
                v.push(format!("{name} depends on {d}, which has no layer"));
                continue;
            };
            let same_ok = SAME_LAYER_EDGES.contains(&(name.as_str(), d.as_str()));
            if dl > l || (dl == l && !same_ok) {
                v.push(format!(
                    "{name} (L{l}) depends on {d} (L{dl}); only lower layers are allowed (§3.2)"
                ));
            }
        }
    }
    // 3. Dev-dependency cycles: B must not reach A through normal deps when A dev-depends on B.
    let reaches = |from: &str, to: &str| -> bool {
        let mut stack = vec![from.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(c) = stack.pop() {
            if c == to {
                return true;
            }
            if seen.insert(c.clone())
                && let Some(p) = pkgs.get(&c)
            {
                stack.extend(p.deps.iter().cloned());
            }
        }
        false
    };
    for (name, pkg) in &pkgs {
        for d in &pkg.dev_deps {
            if reaches(d, name) {
                v.push(format!(
                    "{name} dev-depends on {d}, which depends on {name}: dev-dependency cycle (rule 2.2.11)"
                ));
            }
        }
    }
    // 4. Restricted externals and forbidden edges.
    for (ext, allowed, why) in RESTRICTED {
        for (name, pkg) in &pkgs {
            if pkg.external.contains(*ext) && !allowed.contains(&name.as_str()) {
                v.push(format!("{name} depends on {ext}: {why}"));
            }
        }
    }
    for (a, b, why) in FORBIDDEN_EDGES {
        if pkgs.get(*a).is_some_and(|p| p.deps.contains(*b)) {
            v.push(format!("{a} depends on {b}: {why}"));
        }
    }
    // 5. Every member dependency is inherited from [workspace.dependencies].
    for (name, manifest) in &manifests {
        for d in non_workspace_deps(manifest)? {
            v.push(format!(
                "{name}: `{d}` is not `workspace = true` (rule 2.2.3: declare it in the root Cargo.toml)"
            ));
        }
    }
    Ok(v)
}

/// `cargo xtask check-deps`.
pub fn run(args: &[String]) -> Result<(), String> {
    if let Some(a) = args.first() {
        return Err(format!("unexpected argument `{a}`"));
    }
    let v = violations()?;
    if v.is_empty() {
        println!(
            "check-deps: {} crates, layer rules and dev-dependency rules OK",
            LAYERS.len()
        );
        Ok(())
    } else {
        for x in &v {
            eprintln!("  {x}");
        }
        Err(format!("{} violation(s)", v.len()))
    }
}

#[cfg(test)]
mod tests {
    /// `cargo test --workspace` runs the same check as CI's `cargo xtask check-deps`.
    #[test]
    fn workspace_obeys_dependency_rules() {
        let v = super::violations().expect("cargo metadata");
        assert!(
            v.is_empty(),
            "dependency rule violations:\n{}",
            v.join("\n")
        );
    }
}
