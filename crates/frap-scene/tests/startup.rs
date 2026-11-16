// SPDX-License-Identifier: GPL-3.0-or-later
//! Factory and empty startup equal Blender's dumps, compared **through RNA** (§5.7): every
//! dumped property of every datablock that frap binds is read with the bindings and hooks and
//! compared with the dump (`spec/blender-5.2/startup.json`; the empty startup with
//! `tests/data/empty_startup.json`, a `dump_startup.py` run after
//! `wm.read_homefile(use_empty=True)` on Blender 5.2.4).

mod support;

use frap_rna::{EulerOrder, IdAny, MathTag, PropKind, RnaErrorKind, RnaPtr, Seg, Value};
use frap_scene::HostRead;
use frap_scene::startup::{OMITTED_IMAGES, OMITTED_MATERIALS, factory_dump};
use serde_json::Value as J;
use support::{TestHost, read, with_struct};

const EMPTY_JSON: &str = include_str!("data/empty_startup.json");

/// Properties whose value legitimately differs from a `--factory-startup` dump.
/// `(struct, property)` frap does not model (documented gaps).
const KNOWN_GAPS: &[(&str, &str)] = &[
    // Cycles light shader trees are not modelled (`Light.node_tree` reads None).
    ("PointLight", "node_tree"),
];

const NOT_COMPARED: &[&str] = &[
    // Per-session values.
    "session_uid",
    // Derived values Blender reports stale (D-04: frap's are fresh and equal anyway, but the
    // dumped `users` counts depend on UI users frap does not model, e.g. images).
    "is_editable",
];

fn approx(a: f64, b: f64) -> bool {
    let tol = 1e-5 * a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= tol
}

fn flat(j: &J, out: &mut Vec<J>) {
    match j {
        J::Array(a) => a.iter().for_each(|x| flat(x, out)),
        other => out.push(other.clone()),
    }
}

/// `None` when equal; the mismatch otherwise.
fn same(h: &TestHost, got: &Value, want: &J) -> Option<String> {
    let num = |j: &J| j.as_f64();
    let ok = match (got, want) {
        (Value::None, J::Null) => true,
        (Value::Bool(a), J::Bool(b)) => a == b,
        (Value::Int(a), _) => num(want).is_some_and(|b| *a as f64 == b),
        (Value::Float(a), _) => num(want).is_some_and(|b| approx(f64::from(*a), b)),
        (Value::Double(a), _) => num(want).is_some_and(|b| approx(*a, b)),
        (Value::Str(a) | Value::Enum(a), J::String(b)) => a == b,
        (Value::EnumSet(a), J::Array(b)) => {
            let mut b: Vec<&str> = b.iter().filter_map(J::as_str).collect();
            b.sort_unstable();
            a.iter().map(|x| x.as_str()).collect::<Vec<_>>() == b
        }
        (Value::FloatArray { data, .. }, J::Array(_)) => {
            let mut w = Vec::new();
            flat(want, &mut w);
            w.len() == data.len()
                && data
                    .iter()
                    .zip(&w)
                    .all(|(a, b)| b.as_f64().is_some_and(|b| approx(f64::from(*a), b)))
        }
        (Value::IntArray(a), J::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| y.as_i64() == Some(*x))
        }
        (Value::BoolArray(a), J::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| y.as_bool() == Some(*x))
        }
        (Value::Id(a), _) => {
            let want = want.get("$ref").and_then(J::as_str).map(str::to_owned);
            a.and_then(|a| repr(h, a)) == want
        }
        (Value::Ptr(p), _) => {
            want.get("$ref").and_then(J::as_str) == Some(p.repr(&h.main).as_str())
        }
        _ => false,
    };
    (!ok).then(|| format!("frap {got:?}, Blender {want}"))
}

fn repr(h: &TestHost, a: IdAny) -> Option<String> {
    Some(RnaPtr::id(a, &frap_spec_rna::ID).repr(&h.main))
}

#[derive(Default)]
struct Tally {
    compared: usize,
    unbound: usize,
    mismatches: Vec<String>,
}

/// Compares every dumped property of the struct at `ptr` with `j`, recursing into nested
/// plain structs.
fn compare(h: &TestHost, ptr: &RnaPtr, j: &J, at: &str, t: &mut Tally) {
    let Some(obj) = j.as_object() else { return };
    let srna = with_struct(h, ptr, &mut |s| Ok(s.srna())).expect("struct reachable");
    for (k, v) in obj {
        if k.starts_with('$') || NOT_COMPARED.contains(&k.as_str()) {
            continue;
        }
        let Some(prop) = srna.find_prop(k) else {
            continue;
        };
        if KNOWN_GAPS.contains(&(srna.identifier, k.as_str())) {
            continue;
        }
        let here = format!("{at}.{k}");
        match prop.kind {
            PropKind::Pointer { is_id: false, .. } if v.get("$type").is_some() => {
                let child = ptr.child(Seg::Prop(prop.identifier), srna);
                let reachable = with_struct(h, &child, &mut |_| Ok(())).is_ok();
                if reachable {
                    compare(h, &child, v, &here, t);
                } else {
                    t.unbound += 1;
                }
            }
            PropKind::Pointer { is_id: false, .. } | PropKind::Collection { .. } => {}
            _ => {
                let mut p = ptr.clone();
                p.srna = srna;
                match read(h, &p, k) {
                    Ok(got) => {
                        t.compared += 1;
                        if let Some(m) = same(h, &got, v) {
                            t.mismatches.push(format!("{here}: {m}"));
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind,
                            RnaErrorKind::NoAttribute | RnaErrorKind::NotImplemented
                        ) =>
                    {
                        t.unbound += 1
                    }
                    Err(e) => t
                        .mismatches
                        .push(format!("{here}: read failed: {}", e.message)),
                }
            }
        }
    }
}

fn check_state(h: &TestHost, dump: &J, label: &str) -> Tally {
    let mut t = Tally::default();
    for (coll, items) in dump["ids"].as_object().expect("ids") {
        let want: Vec<&str> = items
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap())
            .filter(|n| !OMITTED_MATERIALS.contains(n) && !OMITTED_IMAGES.contains(n))
            .collect();
        let got = h.main.names(coll).unwrap_or_default();
        assert_eq!(got, want, "{label}: bpy.data.{coll}");
        for item in items.as_array().unwrap() {
            let name = item["name"].as_str().unwrap();
            let Some(a) = h.main.find(coll, name) else {
                continue; // omitted
            };
            let srna = h.main.id(a).unwrap();
            let ptr = RnaPtr::id(a, srna.srna());
            compare(h, &ptr, item, &format!("{coll}['{name}']"), &mut t);
        }
    }
    // Context and window.
    let ctx = h.ctx();
    let active = ctx
        .active_object()
        .map(|r| h.main.objects.name_of(r).unwrap().to_owned());
    assert_eq!(
        active.as_deref(),
        dump["context"]["active_object"].as_str(),
        "{label}"
    );
    let selected: Vec<String> = ctx
        .selected_objects()
        .into_iter()
        .map(|r| h.main.objects.name_of(r).unwrap().to_owned())
        .collect();
    let want: Vec<&str> = dump["context"]["selected_objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(J::as_str)
        .collect();
    assert_eq!(selected, want, "{label}: selected_objects");
    assert_eq!(
        ctx.mode().as_str(),
        dump["context"]["mode"].as_str().unwrap()
    );
    assert_eq!(
        ctx.scene().header.name,
        dump["window"]["scene"].as_str().unwrap()
    );
    assert_eq!(
        ctx.view_layer().name,
        dump["window"]["view_layer"].as_str().unwrap()
    );
    t
}

#[test]
fn factory_startup_equals_the_blender_dump_through_rna() {
    let h = TestHost::factory();
    let t = check_state(&h, factory_dump(), "factory");
    assert!(
        t.mismatches.is_empty(),
        "factory startup differs from startup.json:\n  {}",
        t.mismatches.join("\n  ")
    );
    // Sanity: the comparison really reads most of the dump.
    assert!(t.compared > 550, "only {} properties compared", t.compared);
    eprintln!(
        "factory: {} properties compared through RNA, {} not bound",
        t.compared, t.unbound
    );
}

#[test]
fn empty_startup_equals_the_blender_dump_through_rna() {
    let dump: J = serde_json::from_str(EMPTY_JSON).unwrap();
    let h = TestHost::new(frap_scene::startup::empty());
    let t = check_state(&h, &dump, "empty");
    assert!(
        t.mismatches.is_empty(),
        "empty startup differs from empty_startup.json:\n  {}",
        t.mismatches.join("\n  ")
    );
    assert!(t.compared > 250, "only {} properties compared", t.compared);
    assert!(h.main.objects.is_empty() && h.main.collections.is_empty());
    let scene = h.ctx().scene();
    assert_eq!(scene.camera, None);
    assert_eq!(scene.world, None);
    assert!(
        h.ctx().collection().is_none(),
        "new objects go to the master collection"
    );
}

/// The factory `World` exists (closes the deviations.md §2 gap): `scene.world` is `World`,
/// its colour 0.0509, and its embedded tree is Background → World Output.
#[test]
fn factory_world_and_material_graphs() {
    let h = TestHost::factory();
    let w = h.ctx().scene().world.expect("factory scene has a world");
    let world = h.main.worlds.get(w).unwrap();
    assert_eq!(world.header.name, "World");
    assert!((world.color[0] - 0.050_876_09).abs() < 1e-7);
    let names: Vec<&str> = world
        .node_tree
        .nodes
        .iter()
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(names, ["World Output", "Background"]);
    assert_eq!(world.node_tree.links.len(), 1);
    assert!(world.node_tree.header.is_embedded_data);
    let mat = h
        .main
        .materials
        .get(h.main.materials.find("Material").unwrap())
        .unwrap();
    let names: Vec<&str> = mat
        .node_tree
        .nodes
        .iter()
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(names, ["Material Output", "Principled BSDF"]);
    let bsdf = &mat.node_tree.nodes[1];
    assert_eq!(bsdf.location, [-200.0, 100.0]);
    assert!(bsdf.inputs.len() > 20, "every Principled socket");
    // The factory Layout viewport (virtual 3D View, §8).
    let vm = h.win.view3d.view_matrix.transpose().to_cols_array();
    let want = &factory_dump()["view3d"]["region_3d"]["view_matrix"];
    for (i, x) in vm.iter().enumerate() {
        let w = want[i / 4][i % 4].as_f64().unwrap() as f32;
        assert!((x - w).abs() < 1e-6, "view_matrix[{i}]");
    }
}

/// Mesh data of the factory cube is exactly the dump (vertex order differs from
/// `primitive_cube_add`).
#[test]
fn factory_cube_mesh_is_exact() {
    let h = TestHost::factory();
    let me = h
        .main
        .meshes
        .get(h.main.meshes.find("Cube").unwrap())
        .unwrap();
    let d = &factory_dump()["meshes_exact"][0];
    assert_eq!(me.positions().len(), 8);
    assert_eq!(me.corner_verts().len(), 24);
    let ce: Vec<i64> = d["corner_edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    assert_eq!(
        me.corner_edges()
            .iter()
            .map(|x| i64::from(*x))
            .collect::<Vec<_>>(),
        ce
    );
    let names: Vec<&str> = me.attributes().iter().map(|a| a.name.as_str()).collect();
    let want: Vec<&str> = d["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, want);
    assert_eq!(me.uv_active.as_deref(), Some("UVMap"));
    let _ = (MathTag::None, EulerOrder::XYZ);
}
