// SPDX-License-Identifier: GPL-3.0-or-later
//! spec-refresh tests (`cargo test -p xtask spec_refresh`). Hermetic ones run everywhere; the
//! Blender ones are `#[ignore]`d and run with `cargo test -p xtask spec_refresh -- --ignored`
//! (the `spec-refresh` workflow), finding Blender like the golden harness.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::blender::{self, parse_version};
use super::compare::{Family, Files, Subject, compare};
use super::diff::{Change, Seg, diff};
use super::followups::{self, Codegen};
use super::report::BlenderInfo;
use super::*;

const EXPECTED: &str = "src/spec_refresh/fixtures/seeded.expected.md";

fn root() -> PathBuf {
    crate::workspace_root()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("frap-spec-refresh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn spec_files() -> Files {
    load(&root().join("spec/blender-5.2")).expect("spec loads")
}

fn write_files(dir: &Path, files: &Files) {
    for (rel, b) in files {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b).unwrap();
    }
}

// ------------------------------------------------------------------------------------- diff

#[test]
fn keyed_arrays_report_insertions_not_shifts() {
    let a = json!({"props": [{"identifier": "a", "v": 1}, {"identifier": "b", "v": 2}]});
    let b = json!({"props": [{"identifier": "x", "v": 9}, {"identifier": "a", "v": 1}, {"identifier": "b", "v": 3}]});
    let d = diff(&a, &b);
    assert_eq!(
        d,
        vec![
            Change::Changed {
                at: vec![
                    Seg::Key("props".into()),
                    Seg::Id("b".into()),
                    Seg::Key("v".into())
                ],
                old: json!(2),
                new: json!(3)
            },
            Change::Added {
                at: vec![Seg::Key("props".into()), Seg::Id("x".into())],
                value: json!({"identifier": "x", "v": 9})
            },
        ]
    );
}

#[test]
fn renames_strict_and_loose() {
    let a = json!([{"identifier": "old", "name": "N", "type": "INT"}, {"identifier": "k", "type": "BOOLEAN"}]);
    let strict = json!([{"identifier": "new", "name": "N", "type": "INT"}, {"identifier": "k", "type": "BOOLEAN"}]);
    match &diff(&a, &strict)[..] {
        [Change::Renamed { from, to, .. }] => {
            assert_eq!((from.as_str(), to.as_str()), ("old", "new"))
        }
        d => panic!("{d:?}"),
    }
    // Loose: the UI name changes too; reported as a rename plus the name change.
    let loose = json!([{"identifier": "new", "name": "M", "type": "INT"}, {"identifier": "k", "type": "BOOLEAN"}]);
    let d = diff(&a, &loose);
    assert!(matches!(&d[0], Change::Renamed { from, to, .. } if from == "old" && to == "new"));
    assert!(
        matches!(&d[1], Change::Changed { at, .. } if at == &vec![Seg::Id("new".into()), Seg::Key("name".into())])
    );
    // A different type is not a rename.
    let other = json!([{"identifier": "new", "name": "N", "type": "FLOAT"}, {"identifier": "k", "type": "BOOLEAN"}]);
    let d = diff(&a, &other);
    assert!(matches!(&d[0], Change::Removed { .. }) && matches!(&d[1], Change::Added { .. }));
}

#[test]
fn string_sets_and_order() {
    let a = json!({"flags": ["A", "B", "C"]});
    let b = json!({"flags": ["C", "A", "D"]});
    let d = diff(&a, &b);
    assert_eq!(d.len(), 3, "{d:?}");
    assert!(matches!(&d[0], Change::Removed { at, .. } if at[1] == Seg::Id("B".into())));
    assert!(matches!(&d[1], Change::Added { at, .. } if at[1] == Seg::Id("D".into())));
    assert!(
        matches!(&d[2], Change::Reordered { old, new, .. } if old == &["A", "C"] && new == &["C", "A"])
    );
    // Numeric arrays are one value.
    let d = diff(
        &json!({"default": [0.0, 1.0]}),
        &json!({"default": [0.0, 2.0]}),
    );
    assert!(matches!(&d[..], [Change::Changed { .. }]));
}

// ---------------------------------------------------------------------------------- compare

fn files(pairs: &[(&str, Value)]) -> Files {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), serde_json::to_vec_pretty(v).unwrap()))
        .collect()
}

#[test]
fn version_fields_are_not_drift() {
    let meta =
        |v: &str, h: &str| json!({"blender_version": v, "build_hash": h, "namespaces": ["mesh"]});
    let old = files(&[
        ("ops/_meta.json", meta("5.2.4 LTS", "cd5dc428cce1")),
        (
            "startup.json",
            json!({"blender": {"blender_version": "5.2.4 LTS", "build_hash": "cd5dc428cce1"}, "ids": {}}),
        ),
    ]);
    let new = files(&[
        ("ops/_meta.json", meta("5.2.7", "0123456789ab")),
        (
            "startup.json",
            json!({"blender": {"blender_version": "5.2.7", "build_hash": "0123456789ab"}, "ids": {}}),
        ),
    ]);
    let c = compare(&old, &new);
    assert!(!c.has_drift(), "{:?}", c.items);
    assert!(c.differs());
    assert_eq!(c.version_only, ["ops/_meta.json", "startup.json"]);
    assert_eq!(c.old_version.unwrap().version, "5.2.4 LTS");
    assert_eq!(c.new_version.unwrap().build_hash, "0123456789ab");
}

#[test]
fn files_added_removed_and_formatting() {
    let s = json!({"identifier": "Foo", "props": []});
    let old = files(&[
        ("rna/Foo.json", s.clone()),
        ("modifiers/BEVEL.json", json!({"values": {"width": 0.1}})),
    ]);
    let mut new = files(&[("rna/Bar.json", s), ("nodes/ShaderNodeX.json", json!({}))]);
    new.insert(
        "modifiers/BEVEL.json".into(),
        serde_json::to_vec(&json!({"values": {"width": 0.1}})).unwrap(),
    );
    let c = compare(&old, &new);
    let cats: Vec<(&str, String)> = c
        .items
        .iter()
        .map(|i| (i.category.as_str(), i.subject.label()))
        .collect();
    assert_eq!(
        cats,
        [
            ("formatting changed", "BEVEL".to_owned()),
            ("node type added", "ShaderNodeX".into()),
            ("struct added", "Bar".into()),
            ("struct removed", "Foo".into()),
        ]
    );
}

#[test]
fn rna_nodes_modifiers_classification() {
    let prop = |id: &str| {
        json!({"identifier": id, "name": "Loc", "type": "FLOAT", "default": [0.0, 0.0, 0.0],
        "items": [], "flags": ["ANIMATABLE"]})
    };
    let st = |p: Value, f: Value| {
        json!({"identifier": "Object", "props": [p, prop("keep")], "funcs": [f],
        "fresh": {"recipe": "r", "differs_from_default": {"up_axis": "Z"}}})
    };
    let func = |pid: &str| json!({"identifier": "select_get", "params": [{"identifier": pid, "type": "POINTER"}], "returns": []});
    let old = files(&[
        ("rna/Object.json", st(prop("location"), func("view_layer"))),
        (
            "nodes/ShaderNodeMath.json",
            json!({"inputs": [{"identifier": "Value", "default_value": 0.5, "enabled": true}]}),
        ),
        (
            "modifiers/BEVEL.json",
            json!({"values": {"width": 0.1, "segments": 1}}),
        ),
    ]);
    let mut new_st = st(prop("loc"), func("layer"));
    new_st["fresh"]["differs_from_default"]["up_axis"] = json!("Y");
    new_st["props"][1]["flags"] = json!(["ANIMATABLE", "LIBRARY_EDITABLE"]);
    let new = files(&[
        ("rna/Object.json", new_st),
        (
            "nodes/ShaderNodeMath.json",
            json!({"inputs": [{"identifier": "Value", "default_value": 0.25, "enabled": false}]}),
        ),
        (
            "modifiers/BEVEL.json",
            json!({"values": {"width": 0.2, "segments": 1, "harden": false}}),
        ),
    ]);
    let c = compare(&old, &new);
    let got: Vec<String> = c
        .items
        .iter()
        .map(|i| format!("{} | {} | {}", i.category, i.locus(), i.detail))
        .collect();
    assert_eq!(
        got,
        [
            "default changed | BEVEL.width | `0.1` → `0.2`",
            "property added | BEVEL.harden | `false`",
            "default changed | ShaderNodeMath inputs[Value] | `default_value`: `0.5` → `0.25`",
            "socket availability changed | ShaderNodeMath inputs[Value] | `enabled`: `true` → `false`",
            "property renamed | Object | `location` → `loc`",
            "flag added | Object.keep | `flags` + `LIBRARY_EDITABLE`",
            "parameter renamed | Object.select_get() | `view_layer` → `layer`",
            "fresh value changed | Object.up_axis | `\"Z\"` → `\"Y\"`",
        ],
        "{got:#?}"
    );
    // RNA renames are `.frap` migrations; function parameters are not.
    let m = followups::migrations(&c, Some(1));
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].starts_with("`Object.location` → `Object.loc`: "),
        "{m:?}"
    );
    assert!(m[0].contains("migrate/steps/v1_to_v2.rs"), "{m:?}");
}

#[test]
fn enum_item_renames_and_removals_are_migrations() {
    let item = |id: &str, v: i64| json!({"identifier": id, "name": "Same", "description": "", "icon": "NONE", "value": v});
    let st = |items: Value| json!({"identifier": "Object", "props": [{"identifier": "rotation_mode", "type": "ENUM", "items": items}], "funcs": []});
    let old = files(&[(
        "rna/Object.json",
        st(json!([
            item("XYZ", 1),
            item("QUATERNION", 0),
            item("AXIS_ANGLE", -1)
        ])),
    )]);
    let new = files(&[(
        "rna/Object.json",
        st(json!([item("XYZ", 1), item("QUAT", 0)])),
    )]);
    let c = compare(&old, &new);
    let got: Vec<String> = c
        .items
        .iter()
        .map(|i| format!("{} | {} | {}", i.category, i.locus(), i.detail))
        .collect();
    assert_eq!(
        got,
        [
            "enum item renamed | Object.rotation_mode | `QUATERNION` → `QUAT` (\"Same\")",
            "enum item removed | Object.rotation_mode | − `AXIS_ANGLE` (\"Same\", value -1)",
        ],
        "{got:#?}"
    );
    let m = followups::migrations(&c, None);
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(
        m[0].starts_with("`Object.rotation_mode` value `'QUATERNION'` → `'QUAT'`"),
        "{m:?}"
    );
    assert!(
        m[1].starts_with("`Object.rotation_mode` value removed"),
        "{m:?}"
    );
    assert!(m[1].contains("v<N>_to_v<N+1>.rs"), "{m:?}");
}

#[test]
fn new_unlisted_operator_is_out_by_rule_3() {
    let op = |id: &str| json!({"idname": id, "label": "L", "props": [], "poll": {"A": "Y"}});
    let old = files(&[(
        "ops/object.json",
        json!({"namespace": "object", "count": 1, "operators": [op("object.select_pattern")]}),
    )]);
    let new = files(&[(
        "ops/object.json",
        json!({"namespace": "object", "count": 2, "operators": [op("object.select_pattern"), op("object.frap_new_thing")]}),
    )]);
    let c = compare(&old, &new);
    assert_eq!(c.items.len(), 1, "{:?}", c.items);
    assert_eq!(c.items[0].category, "operator added");
    let md = std::fs::read_to_string(root().join("docs/OPERATORS.md")).unwrap();
    let f = followups::operators_md(&md, &c, 8);
    assert_eq!(
        f,
        [
            "`object.frap_new_thing` (operator added): not listed, so OUT by rule (3); add a row \
          to the `object.*` table if it is in scope"
        ]
    );
}

// ---------------------------------------------------------------------------------- blender

#[test]
fn version_parsing_accepts_any_5_2_x() {
    let out = "Blender 5.2.4 LTS\n\tbuild date: 2026-11-10\n\tbuild time: 01:31:20\n\tbuild commit date: 2026-11-09\n\tbuild hash: cd5dc428cce1\n\tbuild branch: blender-v5.2-release\n";
    let v = parse_version(out).unwrap();
    assert_eq!(v.version, "5.2.4 LTS");
    assert_eq!(v.number, (5, 2, 4));
    assert_eq!(v.build_hash, "cd5dc428cce1");
    let v = parse_version("Blender 5.2.11\nbuild hash: 0123456789ab\n").unwrap();
    assert_eq!(
        (v.number, v.build_hash.as_str()),
        ((5, 2, 11), "0123456789ab")
    );
    for bad in [
        "Blender 5.3.0\nbuild hash: x\n",
        "Blender 4.5.3 LTS\nbuild hash: x\n",
    ] {
        let e = parse_version(bad).unwrap_err();
        assert!(e.contains("needs Blender 5.2.x"), "{e}");
    }
    assert!(
        parse_version("Blender 5.2.4\n")
            .unwrap_err()
            .contains("build hash")
    );
    assert!(parse_version("hello\n").is_err());
}

#[test]
fn locate_order_and_dump_command_line() {
    let d = scratch("locate");
    // The PATH lookup wants `blender.exe` on Windows.
    let exe = d.join(if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    });
    std::fs::write(&exe, "").unwrap();
    let missing = d.join("nope");
    assert_eq!(
        blender::locate_from(Some(exe.clone()), None, None, None).unwrap(),
        exe
    );
    assert!(
        blender::locate_from(Some(missing.clone()), Some(exe.clone().into()), None, None)
            .unwrap_err()
            .contains("no such file")
    );
    assert_eq!(
        blender::locate_from(None, Some(missing.into()), Some(d.clone().into()), None).unwrap(),
        exe
    );
    let r = Path::new("/r");
    let a = blender::dump_args(r, Family::Ops, Some(Path::new("/s.py")), Path::new("/o"));
    let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    assert_eq!(
        a,
        [
            "-b",
            "--factory-startup",
            "--python-exit-code",
            "1",
            "--python",
            "/s.py",
            "--python",
            &r.join("tools/blender")
                .join("dump_ops.py")
                .to_string_lossy(),
            "--",
            "--out",
            "/o"
        ]
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn args() {
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    let o = parse_args(&s(&["--check", "--blender", "/b", "--seed", "x.py"])).unwrap();
    assert!(o.check);
    assert_eq!(o.blender.as_deref(), Some(Path::new("/b")));
    assert_eq!(o.seed.as_deref(), Some(Path::new("x.py")));
    assert!(parse_args(&s(&["--blender"])).is_err());
    // A seeded run never writes spec/blender-5.2/.
    let e = refresh(&root(), &parse_args(&s(&["--seed", "x.py"])).unwrap()).unwrap_err();
    assert!(e.contains("never into spec/blender-5.2/"), "{e}");
    assert!(parse_args(&s(&["--frobnicate"])).is_err());
}

// ----------------------------------------------------------------------------- OPERATORS.md

#[test]
fn operators_md_rows() {
    let md = std::fs::read_to_string(root().join("docs/OPERATORS.md")).unwrap();
    let rows = followups::parse_rows(&md);
    let r = followups::rows_for(&rows, &Subject::Operator("object.select_pattern".into()));
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].section, 'A');
    assert!(
        md.lines()
            .nth(r[0].line - 1)
            .unwrap()
            .contains("`object.select_pattern`")
    );
    assert_eq!(
        followups::rows_for(&rows, &Subject::Modifier("SUBSURF".into())).len(),
        1
    );
    assert!(!followups::rows_for(&rows, &Subject::Struct("Object".into())).is_empty());
    assert!(followups::rows_for(&rows, &Subject::Operator("object.nope".into())).is_empty());
}

// ------------------------------------------------------------------------ seeded drift fixture

fn ops_mut<'a>(doc: &'a mut Value, idname: &str) -> &'a mut Value {
    doc["operators"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| o["idname"] == idname)
        .unwrap_or_else(|| panic!("{idname} not dumped"))
}

fn prop_mut<'a>(op: &'a mut Value, id: &str) -> &'a mut Value {
    op["props"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["identifier"] == id)
        .unwrap()
}

/// The five changes of `tools/blender/spec_refresh_seed.py`, applied to the committed dumps.
fn seeded(spec: &Files) -> Files {
    let mut out = spec.clone();
    let mut obj: Value = serde_json::from_slice(&spec["ops/object.json"]).unwrap();
    {
        let sp = ops_mut(&mut obj, "object.select_pattern");
        prop_mut(sp, "case_sensitive")["identifier"] = json!("match_case");
        sp["poll"]["D"] = json!("N");
        sp["poll"]["DV"] = json!("N");
        let ss = ops_mut(&mut obj, "object.subdivision_set");
        prop_mut(ss, "level")["default"] = json!(2);
    }
    let mut uv: Value = serde_json::from_slice(&spec["ops/uv.json"]).unwrap();
    let ops = uv["operators"].as_array_mut().unwrap();
    ops.retain(|o| o["idname"] != "uv.lightmap_pack");
    let n = ops.len();
    uv["count"] = json!(n);
    {
        let fq = ops_mut(&mut uv, "uv.follow_active_quads");
        prop_mut(fq, "mode")["items"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "identifier": "UNIFORM",
                "name": "Uniform",
                "description": "Seeded drift (spec-refresh fixture)",
                "icon": "NONE",
                "value": 3
            }));
    }
    out.insert(
        "ops/object.json".into(),
        serde_json::to_vec_pretty(&obj).unwrap(),
    );
    out.insert(
        "ops/uv.json".into(),
        serde_json::to_vec_pretty(&uv).unwrap(),
    );
    out
}

fn spec_blender() -> BlenderInfo {
    BlenderInfo {
        path: "<blender>".into(),
        version: "5.2.4 LTS".into(),
        build_hash: "cd5dc428cce1".into(),
    }
}

/// The report without what other work packages change in passing: OPERATORS.md line numbers
/// (rows added above) and the file count (structs added to the dump set). The line numbers
/// themselves are checked by `operators_md_rows`.
fn normalised(md: &str) -> String {
    let mut out = String::new();
    for line in md.lines() {
        if line.starts_with("- Files: ") {
            out.push_str("- Files: <counts>\n");
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find("OPERATORS.md:") {
            let (head, tail) = rest.split_at(i + "OPERATORS.md:".len());
            out.push_str(head);
            let digits = tail.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 {
                out.push_str("<line>");
            }
            rest = &tail[digits..];
        }
        out.push_str(rest);
        out.push('\n');
    }
    out
}

fn check_expected(md: &str) {
    let p = root().join("xtask").join(EXPECTED);
    if std::env::var_os("FRAP_BLESS").is_some() {
        std::fs::write(&p, md).unwrap();
    }
    let want = std::fs::read_to_string(&p).unwrap_or_default();
    assert!(
        normalised(md) == normalised(&want),
        "report differs from {EXPECTED} (FRAP_BLESS=1 rewrites it):\n{md}"
    );
}

/// The seeded drift gives the expected report, with the codegen and OPERATORS.md follow-ups.
#[test]
fn seeded_drift_gives_the_expected_report() {
    let spec = spec_files();
    let new = seeded(&spec);
    let cmp = compare(&spec, &new);
    let cats: Vec<&str> = cmp.items.iter().map(|i| i.category.as_str()).collect();
    for want in [
        "property renamed",
        "enum item added",
        "operator removed",
        "default changed",
        "poll cell changed",
    ] {
        assert!(cats.contains(&want), "{want} missing: {cats:?}");
    }
    let d = scratch("seeded");
    write_files(&d.join("dumps"), &new);
    let r = build_report(
        &root(),
        spec_blender(),
        Some("tools/blender/spec_refresh_seed.py".into()),
        cmp,
        &d.join("dumps"),
        &d.join("shadow"),
    );
    assert!(
        !matches!(r.codegen, Codegen::Skipped | Codegen::Clean),
        "{:?}",
        r.codegen
    );
    assert!(
        r.operators_md
            .iter()
            .any(|l| l.contains("`uv.lightmap_pack`")),
        "{:?}",
        r.operators_md
    );
    let md = r.markdown();
    let _ = std::fs::remove_dir_all(&d);
    check_expected(&md);
    // JSON form carries the same items.
    let j = r.json();
    assert_eq!(j["drift"], json!(true));
    assert_eq!(j["items"].as_array().unwrap().len(), r.cmp.items.len());
}

/// The committed dumps through the codegen dry run generate the committed code.
#[test]
fn codegen_dry_run_on_unchanged_dumps_is_clean() {
    let d = scratch("clean");
    write_files(&d.join("dumps"), &spec_files());
    let (c, dropped) = followups::codegen(&root(), &d.join("dumps"), &d.join("shadow"), &[]);
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(c, Codegen::Clean);
    assert!(dropped.is_empty());
}

#[test]
fn no_drift_report() {
    let spec = spec_files();
    let cmp = compare(&spec, &spec);
    assert!(!cmp.has_drift() && !cmp.differs());
    assert_eq!(cmp.identical, spec.len());
    let d = scratch("nodrift");
    let r = build_report(&root(), spec_blender(), None, cmp, &d, &d.join("shadow"));
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(r.codegen, Codegen::Skipped);
    assert!(r.markdown().contains("**no drift**"), "{}", r.markdown());
}

// ---------------------------------------------------------- fake Blender (any 5.2.x, version)

/// A shell script standing in for Blender `version`: `--version` prints it, a dump copies the
/// family from `src` and rewrites the recorded version.
#[cfg(unix)]
fn fake_blender(dir: &Path, src: &Path, version: &str, hash: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("fake-blender");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'Blender {version}\n\tbuild hash: {hash}\n'
  exit 0
fi
script=""; out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --python) script="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
fam=$(basename "$script" .py | sed 's/^dump_//')
mkdir -p "$out"
if [ "$fam" = startup ]; then
  cp "{src}/startup.json" "$out/startup.json"
  f="$out/startup.json"
  sed -e 's/5\.2\.4 LTS/{version}/' -e 's/cd5dc428cce1/{hash}/' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
else
  cp -R "{src}/$fam" "$out/$fam"
  f="$out/$fam/_meta.json"
  sed -e 's/5\.2\.4 LTS/{version}/' -e 's/cd5dc428cce1/{hash}/' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
fi
"#,
        src = src.display()
    );
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// `--blender` accepts any 5.2.x: no API drift passes `--check`, the version and hash are
/// recorded in the report and, by a refresh, in the dumps' `_meta.json`.
#[cfg(unix)]
#[test]
fn any_5_2_x_is_accepted_and_its_version_recorded() {
    let d = scratch("fake");
    let spec = d.join("spec");
    write_files(&spec, &spec_files());
    let bl = fake_blender(
        &d,
        &root().join("spec/blender-5.2"),
        "5.2.7",
        "0123456789ab",
    );
    let mut o = Options {
        check: true,
        blender: Some(bl.clone()),
        out: Some(d.join("out")),
        seed: None,
        spec: Some(spec.clone()),
    };
    let r = refresh(&root(), &o).unwrap();
    assert!(!r.report.cmp.has_drift(), "{:?}", r.report.cmp.items);
    assert_eq!(r.report.blender.version, "5.2.7");
    assert_eq!(r.report.blender.build_hash, "0123456789ab");
    assert_eq!(
        r.report.cmp.version_only.len(),
        5,
        "{:?}",
        r.report.cmp.version_only
    );
    assert!(r.applied.is_none());
    let md = std::fs::read_to_string(d.join("out/report.md")).unwrap();
    assert!(
        md.contains("Blender: 5.2.7 (build hash `0123456789ab`)"),
        "{md}"
    );
    assert!(
        md.contains("records: 5.2.4 LTS (build hash `cd5dc428cce1`)"),
        "{md}"
    );
    assert!(md.contains("**no API drift**"), "{md}");
    let j: Value =
        serde_json::from_slice(&std::fs::read(d.join("out/report.json")).unwrap()).unwrap();
    assert_eq!(j["blender"]["version"], "5.2.7");
    assert_eq!(j["dumps_record"]["build_hash"], "0123456789ab");

    // A refresh writes the new version into the five metadata files, nothing else.
    o.check = false;
    let r = refresh(&root(), &o).unwrap();
    let (written, removed) = r.applied.unwrap();
    assert_eq!(
        written,
        [
            "modifiers/_meta.json",
            "nodes/_meta.json",
            "ops/_meta.json",
            "rna/_meta.json",
            "startup.json"
        ]
    );
    assert!(removed.is_empty());
    let meta: Value =
        serde_json::from_slice(&std::fs::read(spec.join("ops/_meta.json")).unwrap()).unwrap();
    assert_eq!(meta["blender_version"], "5.2.7");
    assert_eq!(meta["build_hash"], "0123456789ab");
    // A second check against the refreshed directory is byte-identical.
    o.check = true;
    let r = refresh(&root(), &o).unwrap();
    assert!(!r.report.cmp.differs());

    // Other versions are refused before anything is dumped.
    let bad = fake_blender(
        &d,
        &root().join("spec/blender-5.2"),
        "5.3.0",
        "0123456789ab",
    );
    o.blender = Some(bad);
    let e = refresh(&root(), &o).unwrap_err();
    assert!(e.contains("needs Blender 5.2.x"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

// -------------------------------------------------------------------------- live (Blender 5.2)

fn live_blender() -> PathBuf {
    blender::locate(None).expect("these tests need Blender 5.2.x")
}

/// `spec-refresh --check` with Blender 5.2.4: the re-dump is byte-identical to the spec.
#[test]
#[ignore = "needs Blender 5.2.4 (cargo test -p xtask spec_refresh -- --ignored)"]
fn live_check_reports_no_drift() {
    let d = scratch("live");
    let o = Options {
        check: true,
        blender: Some(live_blender()),
        out: Some(d.clone()),
        ..Options::default()
    };
    let r = refresh(&root(), &o).unwrap();
    let c = &r.report.cmp;
    assert!(
        !c.differs(),
        "drift against spec/blender-5.2/:\n{}",
        r.report.markdown()
    );
    assert_eq!(c.identical, spec_files().len());
    let _ = std::fs::remove_dir_all(&d);
}

/// The seeded drift run through Blender gives the same expected report as the hermetic test.
#[test]
#[ignore = "needs Blender 5.2.4 (cargo test -p xtask spec_refresh -- --ignored)"]
fn live_seeded_drift_gives_the_expected_report() {
    let d = scratch("live-seeded");
    let bl = live_blender();
    let o = Options {
        check: true,
        blender: Some(bl.clone()),
        out: Some(d.clone()),
        seed: Some("tools/blender/spec_refresh_seed.py".into()),
        spec: None,
    };
    let r = refresh(&root(), &o).unwrap();
    assert!(r.applied.is_none(), "--check never writes the spec");
    let md = r
        .report
        .markdown()
        .replace(&bl.display().to_string(), "<blender>");
    let _ = std::fs::remove_dir_all(&d);
    check_expected(&md);
}
