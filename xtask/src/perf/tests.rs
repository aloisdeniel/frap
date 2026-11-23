// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of `cargo xtask perf` (WP 8.8).

use super::*;

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        pairs
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| (*v).to_owned())
    }
}

#[test]
fn runner_classes() {
    assert_eq!(
        runner_class(&env_of(&[
            ("GITHUB_ACTIONS", "true"),
            ("ImageOS", "ubuntu24")
        ])),
        "ubuntu-24.04"
    );
    assert_eq!(
        runner_class(&env_of(&[
            ("GITHUB_ACTIONS", "true"),
            ("ImageOS", "macos15")
        ])),
        "macos-15"
    );
    assert_eq!(
        runner_class(&env_of(&[("GITHUB_ACTIONS", "true"), ("ImageOS", "win22")])),
        "windows-2022"
    );
    assert_eq!(
        runner_class(&env_of(&[
            ("FRAP_RUNNER_CLASS", "ubuntu-24.04"),
            ("ImageOS", "macos15")
        ])),
        "ubuntu-24.04"
    );
    // `ImageOS` alone (no GitHub Actions) is not trusted.
    assert!(runner_class(&env_of(&[("ImageOS", "ubuntu24")])).starts_with("local-"));
}

#[test]
fn measurements_parse_and_keep_the_last_value() {
    let ms = parse_measurements(
        r#"{"name":"cold_start","ms":12.5,"budget_ms":30.0,"release":true,"extra":{}}

{"name":"snapshot.ids.x","ms":0.5,"budget_ms":null}
{"name":"cold_start","ms":13.0,"budget_ms":30.0}
"#,
    )
    .expect("parses");
    assert_eq!(
        ms,
        vec![
            Measurement {
                name: "cold_start".into(),
                ms: 13.0,
                budget_ms: Some(30.0)
            },
            Measurement {
                name: "snapshot.ids.x".into(),
                ms: 0.5,
                budget_ms: None
            },
        ]
    );
    assert!(parse_measurements("{\"ms\": 1}").is_err());
}

fn cfg() -> PerfConfig {
    let doc: toml::Table = r#"
regression_factor = 1.5
[budgets_ms]
cold_start = 30
"edit_op_100k.subdivide" = 200.0
[recorded."ubuntu-24.04"]
cold_start = 10.0
"snapshot.ids.x" = 0.2
"#
    .parse()
    .expect("toml");
    PerfConfig::parse(&doc, "ubuntu-24.04").expect("config")
}

fn m(name: &str, ms: f64, budget_ms: Option<f64>) -> Measurement {
    Measurement {
        name: name.into(),
        ms,
        budget_ms,
    }
}

#[test]
fn judge_checks_budgets_both_ways_and_warns_on_regressions() {
    let c = cfg();
    assert_eq!(c.budgets_ms["cold_start"], 30.0);
    let ok = judge(
        &[
            m("cold_start", 12.0, Some(30.0)),
            m("edit_op_100k.subdivide", 50.0, Some(200.0)),
            m("snapshot.ids.x", 0.9, None),
        ],
        &c,
    );
    assert!(ok.errors.is_empty(), "{:?}", ok.errors);
    // 0.9 ms vs 0.2 recorded: more than 1.5x, but under a millisecond apart (noise).
    assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
    assert_eq!(ok.rows.len(), 3);

    let bad = judge(
        &[
            m("cold_start", 31.0, Some(30.0)),
            m("snapshot.ids.x", 0.1, Some(5.0)),
        ],
        &c,
    );
    assert_eq!(bad.errors.len(), 3, "{:?}", bad.errors);
    assert!(bad.errors[0].contains("over its 30 ms budget"));
    assert!(bad.errors[1].contains("differs from perf.toml"));
    assert!(bad.errors[2].contains("edit_op_100k.subdivide: a perf.toml budget no test measured"));
    // 31 ms vs 10 recorded: a regression warning as well.
    assert_eq!(bad.warnings.len(), 1);
}

#[test]
fn recording_writes_the_runner_class_table() {
    let mut doc: toml::Table = "regression_factor = 1.5\n".parse().expect("toml");
    record_into(
        &mut doc,
        "ubuntu-24.04",
        &[m("cold_start", 12.34567, Some(30.0))],
    );
    let c = PerfConfig::parse(
        &{
            let mut d = doc.clone();
            d.insert(
                "budgets_ms".into(),
                toml::Value::Table(
                    [("cold_start".to_owned(), toml::Value::Float(30.0))]
                        .into_iter()
                        .collect(),
                ),
            );
            d
        },
        "ubuntu-24.04",
    )
    .expect("config");
    assert_eq!(c.recorded["cold_start"], 12.346);
}

#[test]
fn build_budgets_gate_only_once_recorded_on_the_ci_class() {
    assert!(!build_budgets_gate(Some("provisional"), "ubuntu-24.04"));
    assert!(!build_budgets_gate(None, "ubuntu-24.04"));
    assert!(build_budgets_gate(Some("ubuntu-24.04"), "ubuntu-24.04"));
    assert!(!build_budgets_gate(
        Some("ubuntu-24.04"),
        "local-macos-aarch64"
    ));
    // `budgets --record` writes the key at the top of budgets.toml; none means provisional.
    let doc: toml::Table =
        "factor = 2.0\nrecorded_on = \"ubuntu-24.04\"\n[baseline_secs]\nx = 1.0\n"
            .parse()
            .expect("toml");
    assert_eq!(
        build_baselines_recorded_on(&doc).as_deref(),
        Some("ubuntu-24.04")
    );
    let doc: toml::Table = "factor = 2.0\n[baseline_secs]\nx = 1.0\n"
        .parse()
        .expect("toml");
    assert_eq!(build_baselines_recorded_on(&doc), None);
}

/// The committed `xtask/perf.toml` parses and has every budget of §21.7; the build-time
/// baselines of `xtask/budgets.toml` are provisional (no `recorded_on`) or recorded on a
/// runner class.
#[test]
fn committed_perf_toml() {
    let text = std::fs::read_to_string(perf_toml()).expect("perf.toml");
    let doc: toml::Table = text.parse().expect("perf.toml parses");
    let c = PerfConfig::parse(&doc, CI_CLASS).expect("config");
    for (name, budget) in [
        ("cold_start", 30.0),
        ("edit_op_100k.subdivide", 200.0),
        ("edit_op_100k.extrude_region", 200.0),
        ("edit_op_100k.bevel", 200.0),
        ("edit_op_100k_all.extrude_region", 200.0),
        ("glb_export_1m_corners", 2000.0),
    ] {
        assert_eq!(c.budgets_ms.get(name), Some(&budget), "{name}");
    }
    assert!(doc.get("build_budgets").is_none(), "moved to budgets.toml");
    let text = std::fs::read_to_string(budgets_toml()).expect("budgets.toml");
    let doc: toml::Table = text.parse().expect("budgets.toml parses");
    if let Some(on) = build_baselines_recorded_on(&doc) {
        assert!(!on.is_empty());
    }
}
