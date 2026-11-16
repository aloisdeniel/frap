// SPDX-License-Identifier: GPL-3.0-or-later
//! Drift test (ARCHITECTURE §4.2): the hand-written M0 statics must equal the committed
//! Blender 5.2 dumps in spec/m0/ (re-dumped from a live Blender by the frap-cli live test).
//! Operator specs are checked by the same test in frap-spec-ops-mesh and frap-spec-ops-io.

use frap_rna::*;
use frap_spec_ops_io::export_scene::ALL_OPS;
use serde_json::Value as J;

fn load(rel: &str) -> J {
    let p = format!("{}/../../spec/m0/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}")))
        .unwrap()
}

fn f(j: &J) -> f64 {
    j.as_f64().unwrap()
}

fn check_prop(ctx: &str, p: &PropDef, j: &J, overlay: &J) {
    let ctx = format!("{ctx}.{}", p.identifier);
    assert_eq!(j["identifier"], p.identifier, "{ctx}");
    assert_eq!(j["name"], p.name, "{ctx} name");
    assert_eq!(j["description"], p.description, "{ctx} description");
    assert_eq!(j["type"], p.type_str(), "{ctx} type");
    assert_eq!(j["subtype"], p.subtype.as_str(), "{ctx} subtype");
    let arr: Vec<u64> = j["array"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let expect_arr: Vec<u64> = match p.array {
        ArrayShape::Scalar => vec![],
        ArrayShape::Fixed(n) => vec![n as u64],
        ArrayShape::Matrix(r, c) => vec![r as u64, c as u64],
        ArrayShape::Dynamic => vec![0],
    };
    assert_eq!(arr, expect_arr, "{ctx} array");
    let flags: Vec<&str> = j["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(flags, p.flags.names(), "{ctx} flags");
    match &p.kind {
        PropKind::Float {
            default,
            hard,
            soft,
            precision,
            ..
        } => {
            assert_eq!(j["unit"], p.unit.as_str(), "{ctx} unit");
            assert_eq!((f(&j["hard_min"]), f(&j["hard_max"])), *hard, "{ctx} hard");
            assert_eq!((f(&j["soft_min"]), f(&j["soft_max"])), *soft, "{ctx} soft");
            assert_eq!(
                j["precision"].as_u64().unwrap(),
                *precision as u64,
                "{ctx} precision"
            );
            match default {
                Defaults::Scalar(d) => assert_eq!(f(&j["default"]), *d, "{ctx} default"),
                Defaults::Array(a) => {
                    let got: Vec<f64> = j["default"].as_array().unwrap().iter().map(f).collect();
                    assert_eq!(got, a.to_vec(), "{ctx} default");
                }
            }
        }
        PropKind::Bool {
            default: Defaults::Scalar(d),
        } => assert_eq!(j["default"], *d, "{ctx} default"),
        PropKind::String { default, max_bytes } => {
            assert_eq!(j["default"], *default, "{ctx} default");
            assert_eq!(
                j["max_bytes"].as_u64().unwrap(),
                *max_bytes as u64,
                "{ctx} max_bytes"
            );
        }
        PropKind::Enum {
            items,
            default,
            dynamic,
        } => {
            let (src_items, src_default) = match dynamic {
                Some(key) => {
                    assert_eq!(j["dynamic"], true, "{ctx} dynamic");
                    let o = &overlay["dynamic_enums"][key];
                    (o["items"].clone(), o["default"].clone())
                }
                None => (j["items"].clone(), j["default"].clone()),
            };
            let got: Vec<(String, String, String)> = src_items
                .as_array()
                .unwrap()
                .iter()
                .map(|i| {
                    (
                        i["identifier"].as_str().unwrap().into(),
                        i["name"].as_str().unwrap().into(),
                        i["description"].as_str().unwrap().into(),
                    )
                })
                .collect();
            let want: Vec<(String, String, String)> = items
                .iter()
                .map(|i| (i.identifier.into(), i.name.into(), i.description.into()))
                .collect();
            assert_eq!(got, want, "{ctx} items");
            match default {
                EnumDefault::Item(d) => assert_eq!(src_default, *d, "{ctx} default"),
                EnumDefault::Flags(_) => unreachable!(),
            }
        }
        other => panic!("{ctx}: unexpected kind {other:?}"),
    }
}

#[test]
fn operator_specs_match_dumps() {
    let overlay = load("overlay.json");
    for op in ALL_OPS {
        let j = load(&format!("ops/{}.json", op.idname));
        assert_eq!(j["idname"], op.idname);
        assert_eq!(j["bl_idname"], op.bl_idname);
        assert_eq!(j["label"], op.label);
        assert_eq!(j["description"], op.description);
        let opts: Vec<&str> = j["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(opts, op.options.names(), "{} options", op.idname);
        let jp = j["props"].as_array().unwrap();
        if j["props_subset"] == false {
            assert_eq!(jp.len(), op.props.len(), "{} prop count", op.idname);
        }
        for p in op.props {
            let jprop = jp
                .iter()
                .find(|x| x["identifier"] == p.identifier)
                .unwrap_or_else(|| panic!("{}.{} not in dump", op.idname, p.identifier));
            check_prop(op.bl_idname, p, jprop, &overlay);
        }
        // RNA order is preserved.
        let order: Vec<&str> = jp
            .iter()
            .map(|x| x["identifier"].as_str().unwrap())
            .collect();
        let mine: Vec<&str> = op.props.iter().map(|p| p.identifier).collect();
        assert_eq!(order, mine, "{} prop order", op.idname);
    }
}
