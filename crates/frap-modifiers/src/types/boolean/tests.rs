// SPDX-License-Identifier: GPL-3.0-or-later
//! BOOLEAN against Blender 5.2.4: `blender_5_2_4.json` holds the inputs and evaluated results of
//! `dump_blender.py` (reduced by `compact.py`). MANIFOLD results compare at T2 (vertices on a
//! 1e-4 grid, faces as rotation-normalised cycles, UVs per face corner, material indices and
//! slots); EXACT results at T3 (closed and consistently wound, the same volume, element counts
//! within a band, the same material slots).

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_rna::{IdRef, ImageId, MaterialId, ObjectId, Report};
use frap_scene::id::{Collection, Material, Modifier, Object};
use frap_scene::{ChangeLog, EvalMode, EvaluatedPose, Main};
use glam::Mat4;
use serde_json::Value;

use crate::{ModError, ModEvalCx, ModifierSettings};

const DATA: &str = include_str!("blender_5_2_4.json");

fn f32s(v: &Value) -> Vec<f32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap() as f32)
        .collect()
}

fn mat4_rows(v: &Value) -> Mat4 {
    let rows: Vec<Vec<f32>> = v.as_array().unwrap().iter().map(f32s).collect();
    let mut cols = [[0.0f32; 4]; 4];
    for (r, row) in rows.iter().enumerate() {
        for (c, x) in row.iter().enumerate() {
            cols[c][r] = *x;
        }
    }
    Mat4::from_cols_array_2d(&cols)
}

/// A flat-shaded mesh from the dump (positions, faces, UV maps, material indices and slots).
fn mesh_from(j: &Value, name: &str, main: &mut Main, log: &mut ChangeLog) -> Mesh {
    let positions: Vec<[f32; 3]> = j["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let v = f32s(p);
            [v[0], v[1], v[2]]
        })
        .collect();
    let faces: Vec<Vec<i32>> = j["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect()
        })
        .collect();
    let mut m = Mesh::new(name);
    m.from_pydata(&positions, &[], &faces, true);
    for (uv_name, data) in j["uv"].as_object().unwrap() {
        let uv: Vec<[f32; 2]> = data
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let v = f32s(p);
                [v[0], v[1]]
            })
            .collect();
        m.attributes_mut().insert(Attribute {
            name: uv_name.as_str().into(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uv)),
        });
        m.uv_active = Some(uv_name.as_str().into());
    }
    let mi: Vec<i32> = j["material_index"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap() as i32)
        .collect();
    if mi.iter().any(|&i| i != 0) {
        m.attributes_mut().insert(Attribute {
            name: "material_index".into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(mi)),
        });
    }
    for ma in j["materials"].as_array().unwrap() {
        m.materials
            .push(ma.as_str().map(|n| material(main, log, n)));
    }
    m
}

fn material(main: &mut Main, log: &mut ChangeLog, name: &str) -> IdRef<MaterialId> {
    if let Some((r, _)) = main.materials.iter().find(|(_, m)| m.header.name == name) {
        return r;
    }
    main.materials.insert(name, Material::new(name), log)
}

/// The scene of one case: the base object (with the modifier) and its operands.
struct Scene {
    main: Main,
    base: IdRef<ObjectId>,
    matrices: Vec<(IdRef<ObjectId>, Mat4)>,
    modifier: Modifier,
}

fn scene(case: &Value) -> Scene {
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    let mut add = |j: &Value, name: &str, main: &mut Main| -> IdRef<ObjectId> {
        let me = mesh_from(&j["mesh"], name, main, &mut log);
        let me_ref = main.meshes.insert(name, me, &mut log);
        let ob = Object::new(name, Some(me_ref.any()));
        main.objects.insert(name, ob, &mut log)
    };
    let base = add(&case["base"], "Cube", &mut main);
    let mut matrices = vec![(base, mat4_rows(&case["base"]["matrix_world"]))];
    let mut ops = Vec::new();
    for o in case["operands"].as_array().unwrap() {
        let r = add(o, o["name"].as_str().unwrap(), &mut main);
        matrices.push((r, mat4_rows(&o["matrix_world"])));
        ops.push(r);
    }
    let mut settings = frap_scene::generated::modifier_settings::BooleanSettings {
        solver: case["solver"].as_str().unwrap().into(),
        operation: case["operation"].as_str().unwrap().into(),
        material_mode: case["material_mode"].as_str().unwrap().into(),
        ..Default::default()
    };
    if ops.len() > 1 {
        let mut log = ChangeLog::default();
        let mut col = Collection::new("Cutters");
        col.objects = ops.clone();
        let c = main.collections.insert("Cutters", col, &mut log);
        settings.operand_type = "COLLECTION".into();
        settings.collection = Some(c.any());
    } else {
        settings.object = Some(ops[0].any());
    }
    let modifier = Modifier {
        name: "Boolean".into(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: true,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid: 1,
        settings: ModifierSettings::Boolean(settings),
    };
    Scene {
        main,
        base,
        matrices,
        modifier,
    }
}

struct Cx<'a> {
    s: &'a Scene,
    reports: Vec<Report>,
}

impl ModEvalCx for Cx<'_> {
    fn main(&self) -> &Main {
        &self.s.main
    }
    fn object(&self) -> &Object {
        self.s.main.objects.get(self.s.base).unwrap()
    }
    fn object_ref(&self) -> IdRef<ObjectId> {
        self.s.base
    }
    fn evaluated_mesh(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<Mesh>, ModError> {
        let o = self.s.main.objects.get(ob).unwrap();
        let me = o.data.and_then(|d| d.typed()).unwrap();
        Ok(Arc::new(self.s.main.meshes.get(me).unwrap().clone()))
    }
    fn object_matrix_world(&mut self, ob: IdRef<ObjectId>) -> Mat4 {
        self.s
            .matrices
            .iter()
            .find(|(r, _)| *r == ob)
            .map(|(_, m)| *m)
            .unwrap()
    }
    fn pose(&mut self, _: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError> {
        Err(ModError::Dependency("no pose".into()))
    }
    fn image(&mut self, _: IdRef<ImageId>) -> Option<Arc<frap_imbuf::DecodedImage>> {
        None
    }
    fn mode(&self) -> EvalMode {
        EvalMode::Viewport
    }
    fn frame(&self) -> f32 {
        1.0
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn subdivider(&self) -> &dyn frap_subdiv::Subdivider {
        frap_subdiv::default_subdivider()
    }
    fn boolean_solver(&self) -> &dyn frap_boolean::BooleanSolver {
        frap_boolean::default_solver()
    }
}

/// Evaluates the case's modifier on the base mesh.
fn evaluate(s: &Scene) -> (Result<Mesh, ModError>, Vec<Report>) {
    let mut cx = Cx {
        s,
        reports: Vec::new(),
    };
    let base_mesh = cx.evaluated_mesh(s.base).unwrap();
    let info = crate::registry::find(crate::ModifierType::Boolean).expect("BOOLEAN registered");
    let r = (info.eval)(&s.modifier, (*base_mesh).clone(), &mut cx);
    (r, cx.reports)
}

fn positions(j: &Value) -> Vec<[f32; 3]> {
    j.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let v = f32s(p);
            [v[0], v[1], v[2]]
        })
        .collect()
}

/// Maps every frap vertex to the Blender vertex within 1e-4 (each used once).
fn match_verts(frap: &[[f32; 3]], blender: &[[f32; 3]]) -> Result<Vec<usize>, String> {
    let mut used = vec![false; blender.len()];
    let mut map = Vec::with_capacity(frap.len());
    for (i, p) in frap.iter().enumerate() {
        let j = (0..blender.len())
            .find(|&j| !used[j] && (0..3).all(|k| (blender[j][k] - p[k]).abs() <= 1e-4))
            .ok_or_else(|| format!("frap vertex {i} {p:?} has no Blender match"))?;
        used[j] = true;
        map.push(j);
    }
    Ok(map)
}

fn rotate_min(c: &[usize]) -> Vec<usize> {
    let k = (0..c.len()).min_by_key(|&i| c[i]).unwrap();
    let mut v = c.to_vec();
    v.rotate_left(k);
    v
}

/// T2: canonical geometry, UVs, material indices and slots equal Blender's.
#[allow(clippy::needless_range_loop)] // faces index three parallel arrays
fn compare_t2(name: &str, s: &Scene, out: &Mesh, res: &Value) {
    let bpos = positions(&res["positions"]);
    assert_eq!(
        out.verts_num(),
        bpos.len(),
        "{name}: vertex count (frap {:?})",
        out.positions()
    );
    let vmap = match_verts(out.positions(), &bpos).unwrap_or_else(|e| panic!("{name}: {e}"));
    let bfaces: Vec<Vec<usize>> = res["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect()
        })
        .collect();
    assert_eq!(out.faces_num(), bfaces.len(), "{name}: face count");
    assert_eq!(
        out.edges_num(),
        res["edges"].as_array().unwrap().len(),
        "{name}: edge count"
    );
    let bkey: Vec<Vec<usize>> = bfaces.iter().map(|f| rotate_min(f)).collect();
    let bmi: Vec<i64> = res["material_index"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    let fmi: Vec<i32> = match out.attributes().get("material_index").map(|a| &a.data) {
        Some(AttrData::Int(v)) => v.to_vec(),
        _ => vec![0; out.faces_num()],
    };
    let buv: Vec<[f32; 2]> = res["uv"]["UVMap"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| {
                    let v = f32s(p);
                    [v[0], v[1]]
                })
                .collect()
        })
        .unwrap_or_default();
    let fuv: Vec<[f32; 2]> = match out.attributes().get("UVMap").map(|a| &a.data) {
        Some(AttrData::Float2(v)) => v.to_vec(),
        _ => Vec::new(),
    };
    let bstart: Vec<usize> = bfaces
        .iter()
        .scan(0usize, |acc, f| {
            let s = *acc;
            *acc += f.len();
            Some(s)
        })
        .collect();
    let mut used = vec![false; bfaces.len()];
    for f in 0..out.faces_num() {
        let cyc: Vec<usize> = out
            .face(f)
            .map(|c| vmap[out.corner_verts()[c] as usize])
            .collect();
        let key = rotate_min(&cyc);
        let bf = (0..bkey.len())
            .find(|&b| !used[b] && bkey[b] == key)
            .unwrap_or_else(|| panic!("{name}: frap face {f} {cyc:?} not in Blender's"));
        used[bf] = true;
        assert_eq!(
            i64::from(fmi[f]),
            bmi[bf],
            "{name}: material_index of face {f}"
        );
        if !buv.is_empty() {
            for (k, c) in out.face(f).enumerate() {
                let v = cyc[k];
                let bk = bfaces[bf].iter().position(|&x| x == v).unwrap();
                let b = buv[bstart[bf] + bk];
                let u = fuv[c];
                assert!(
                    (u[0] - b[0]).abs() < 1e-4 && (u[1] - b[1]).abs() < 1e-4,
                    "{name}: UV of face {f} corner {k}: frap {u:?} blender {b:?}"
                );
            }
        }
    }
    let names: Vec<Option<String>> = out
        .materials
        .iter()
        .map(|m| m.map(|r| s.main.materials.get(r).unwrap().header.name.to_string()))
        .collect();
    let bnames: Vec<Option<String>> = res["materials"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().map(String::from))
        .collect();
    assert_eq!(names, bnames, "{name}: materials");
}

/// Signed volume of a closed mesh.
fn volume(m: &Mesh) -> f64 {
    let p = m.positions();
    let mut v = 0.0f64;
    for f in 0..m.faces_num() {
        let c: Vec<usize> = m.face(f).map(|c| m.corner_verts()[c] as usize).collect();
        let a = p[c[0]].map(f64::from);
        for i in 1..c.len() - 1 {
            let b = p[c[i]].map(f64::from);
            let d = p[c[i + 1]].map(f64::from);
            v += (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0])
                + a[2] * (b[0] * d[1] - b[1] * d[0]))
                / 6.0;
        }
    }
    v
}

/// Every edge has exactly two faces, using it in opposite directions.
fn is_closed_manifold(m: &Mesh) -> bool {
    let mut dir: std::collections::HashMap<(i32, i32), i32> = Default::default();
    for f in 0..m.faces_num() {
        let r = m.face(f);
        let cv = &m.corner_verts()[r];
        for i in 0..cv.len() {
            let (a, b) = (cv[i], cv[(i + 1) % cv.len()]);
            *dir.entry((a, b)).or_default() += 1;
        }
    }
    dir.iter()
        .all(|(&(a, b), &n)| n == 1 && dir.get(&(b, a)) == Some(&1))
}

/// T3: closed, consistently wound, Blender's volume and slots, counts within a band.
fn compare_t3(name: &str, s: &Scene, out: &Mesh, res: &Value) {
    let bv = res["volume"].as_f64().unwrap();
    let fv = volume(out);
    assert!(
        (fv - bv).abs() <= 1e-4 * bv.abs().max(1.0),
        "{name}: volume frap {fv} blender {bv}"
    );
    if out.faces_num() > 0 {
        assert!(is_closed_manifold(out), "{name}: not closed");
    }
    let bf = res["faces_num"].as_u64().unwrap() as usize;
    assert!(
        out.faces_num() * 2 >= bf && out.faces_num() <= bf * 2,
        "{name}: {} faces vs Blender's {bf}",
        out.faces_num()
    );
    let names: Vec<Option<String>> = out
        .materials
        .iter()
        .map(|m| m.map(|r| s.main.materials.get(r).unwrap().header.name.to_string()))
        .collect();
    let bnames: Vec<Option<String>> = res["materials"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().map(String::from))
        .collect();
    assert_eq!(names, bnames, "{name}: materials");
}

fn cases() -> serde_json::Map<String, Value> {
    serde_json::from_str::<Value>(DATA)
        .unwrap()
        .as_object()
        .unwrap()
        .clone()
}

#[test]
fn manifold_matches_blender_t2() {
    for (name, case) in cases() {
        if case["solver"] != "MANIFOLD" || name.contains("nonmanifold") {
            continue;
        }
        let s = scene(&case);
        let (r, reports) = evaluate(&s);
        let out = r.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(reports.is_empty(), "{name}: {reports:?}");
        compare_t2(&name, &s, &out, &case["result"]);
    }
}

#[test]
fn exact_matches_blender_t3() {
    for (name, case) in cases() {
        if case["solver"] != "EXACT" || name.contains("nonmanifold") {
            continue;
        }
        let s = scene(&case);
        let (r, _) = evaluate(&s);
        let out = r.unwrap_or_else(|e| panic!("{name}: {e}"));
        compare_t3(&name, &s, &out, &case["result"]);
    }
}

#[test]
fn manifold_non_manifold_operand_is_blenders_error() {
    let case = &cases()["manifold_nonmanifold"];
    let s = scene(case);
    let (r, _) = evaluate(&s);
    assert_eq!(
        r.unwrap_err(),
        ModError::Error("Cannot execute, object(s) 'Grid' have non-manifold geometry".into())
    );
}

#[test]
fn exact_open_operand_is_a_frap_error() {
    // Blender's exact mesh arrangement cuts with open sheets; frap's stand-in cannot (WP 8.6).
    let case = &cases()["exact_nonmanifold"];
    let s = scene(case);
    let (r, _) = evaluate(&s);
    let ModError::Error(msg) = r.unwrap_err() else {
        panic!("expected a modifier error");
    };
    assert!(msg.starts_with("frap: the EXACT solver needs closed operands; object(s) 'Grid'"));
}

#[test]
fn float_runs_exact_with_a_warning() {
    let mut case = cases()["exact_cube_cube_difference"].clone();
    case["solver"] = "FLOAT".into();
    let s = scene(&case);
    let (r, reports) = evaluate(&s);
    let out = r.unwrap();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].message.contains("FLOAT solver runs as EXACT"));
    compare_t3(
        "float",
        &s,
        &out,
        &cases()["exact_cube_cube_difference"]["result"],
    );
}

#[test]
fn float_collection_intersect_is_refused() {
    let mut case = cases()["manifold_collection"].clone();
    case["solver"] = "FLOAT".into();
    case["operation"] = "INTERSECT".into();
    let s = scene(&case);
    let (r, _) = evaluate(&s);
    assert_eq!(
        r.unwrap_err(),
        ModError::Error("Cannot execute, intersect only available using exact solver".into())
    );
}

#[test]
fn disabled_without_operand() {
    let s = ModifierSettings::Boolean(Default::default());
    assert!(super::is_disabled(&s).is_some());
    let mut b = frap_scene::generated::modifier_settings::BooleanSettings {
        operand_type: "COLLECTION".into(),
        ..Default::default()
    };
    // EXACT tolerates an empty collection.
    assert!(super::is_disabled(&ModifierSettings::Boolean(b.clone())).is_none());
    b.solver = "MANIFOLD".into();
    assert!(super::is_disabled(&ModifierSettings::Boolean(b)).is_some());
}

#[test]
fn index_based_remap_is_blenders() {
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    let red = material(&mut main, &mut log, "Red");
    let green = material(&mut main, &mut log, "Green");
    let blue = material(&mut main, &mut log, "Blue");
    // Blender's BKE_object_material_remap_calc: found slot, else same slot, else 0.
    assert_eq!(
        super::material_remap_index_based(&[Some(red), Some(green)], &[Some(blue), Some(red)]),
        vec![0, 0]
    );
    assert_eq!(
        super::material_remap_index_based(&[Some(red)], &[Some(green), Some(blue), Some(red)]),
        vec![0, 0, 0]
    );
    assert_eq!(
        super::material_remap_index_based(&[Some(red), Some(green)], &[]),
        vec![0]
    );
}

// --------------------------------------------- the golden fixtures, until WP 4.1 enables them

/// A scene of primitives placed by a golden fixture's `matrix_world`s; `Cube` carries the
/// modifier, operand `operand` (or the collection of `collection`).
fn fixture_scene(
    fixture: &Value,
    objects: Vec<(&str, Mesh, Vec<&str>)>,
    settings: frap_scene::generated::modifier_settings::BooleanSettings,
    operand: &str,
    collection: &[&str],
) -> Scene {
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    let mut matrices = Vec::new();
    let mut refs = Vec::new();
    for (name, mut me, mats) in objects {
        for m in mats {
            let r = material(&mut main, &mut log, m);
            me.materials.push(Some(r));
        }
        let me_ref = main.meshes.insert(name, me, &mut log);
        let ob = main
            .objects
            .insert(name, Object::new(name, Some(me_ref.any())), &mut log);
        matrices.push((ob, mat4_rows(&fixture["objects"][name]["matrix_world"])));
        refs.push((name, ob));
    }
    let find = |n: &str| refs.iter().find(|(m, _)| *m == n).unwrap().1;
    let mut settings = settings;
    if collection.is_empty() {
        settings.object = Some(find(operand).any());
    } else {
        let mut col = Collection::new("Cutters");
        col.objects = collection.iter().map(|n| find(n)).collect();
        let c = main.collections.insert("Cutters", col, &mut log);
        settings.operand_type = "COLLECTION".into();
        settings.collection = Some(c.any());
    }
    Scene {
        main,
        base: find("Cube"),
        matrices,
        modifier: Modifier {
            name: "Boolean".into(),
            show_viewport: true,
            show_render: true,
            show_in_editmode: true,
            show_on_cage: false,
            show_expanded: true,
            is_active: true,
            use_pin_to_last: false,
            persistent_uid: 1,
            settings: ModifierSettings::Boolean(settings),
        },
    }
}

/// A fixture's evaluated mesh in the dump format [`compare_t2`] reads.
fn fixture_result(g: &Value, materials: &[Option<&str>]) -> Value {
    let faces = g["faces"].clone();
    let n = faces.as_array().unwrap().len();
    let mi = g["attributes"]
        .get("material_index")
        .map(|a| a["data"].clone())
        .unwrap_or_else(|| Value::from(vec![0; n]));
    serde_json::json!({
        "positions": g["positions"],
        "edges": g["edges"],
        "faces": faces,
        "material_index": mi,
        "uv": {"UVMap": g["uv_layers"]["UVMap"]},
        "materials": materials,
    })
}

fn settings(
    solver: &str,
    operation: &str,
    material_mode: &str,
) -> frap_scene::generated::modifier_settings::BooleanSettings {
    frap_scene::generated::modifier_settings::BooleanSettings {
        solver: solver.into(),
        operation: operation.into(),
        material_mode: material_mode.into(),
        ..Default::default()
    }
}

fn cube(size: f32) -> Mesh {
    frap_mesh::primitives::cube(size, true, [1.0, 1.0, 1.0])
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../../tests/golden/fixtures/modifiers/boolean/{name}.blender.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn golden_manifold_difference_t2() {
    let fx = fixture("manifold_difference");
    let s = fixture_scene(
        &fx,
        vec![
            ("Cube", cube(2.0), vec!["Material"]),
            (
                "Cube.001",
                frap_mesh::primitives::cube(1.0, true, [1.2, 0.8, 1.0]),
                vec![],
            ),
        ],
        settings("MANIFOLD", "DIFFERENCE", "INDEX"),
        "Cube.001",
        &[],
    );
    let out = evaluate(&s).0.unwrap();
    let res = fixture_result(&fx["evaluated"]["Cube"], &[Some("Material"), None]);
    compare_t2("manifold_difference", &s, &out, &res);
}

#[test]
fn golden_manifold_union_transfer_t2() {
    let fx = fixture("manifold_union_transfer");
    let cyl =
        frap_mesh::primitives::cylinder::cylinder(&frap_mesh::primitives::cylinder::Cylinder {
            vertices: 10,
            radius: 0.5,
            depth: 3.0,
            ..Default::default()
        });
    let s = fixture_scene(
        &fx,
        vec![
            ("Cube", cube(2.0), vec!["Material", "Red"]),
            ("Cylinder", cyl, vec!["Blue"]),
        ],
        settings("MANIFOLD", "UNION", "TRANSFER"),
        "Cylinder",
        &[],
    );
    let out = evaluate(&s).0.unwrap();
    let res = fixture_result(
        &fx["evaluated"]["Cube"],
        &[Some("Material"), Some("Red"), Some("Blue")],
    );
    compare_t2("manifold_union_transfer", &s, &out, &res);
}

#[test]
fn golden_manifold_collection_t2() {
    let fx = fixture("manifold_collection");
    let ico = frap_mesh::primitives::ico_sphere::ico_sphere(
        &frap_mesh::primitives::ico_sphere::IcoSphere {
            subdivisions: 1,
            radius: 0.6,
            ..Default::default()
        },
    );
    let s = fixture_scene(
        &fx,
        vec![
            ("Cube", cube(2.0), vec!["Material"]),
            ("Icosphere", ico, vec![]),
            ("Cube.001", cube(0.8), vec![]),
        ],
        settings("MANIFOLD", "DIFFERENCE", "INDEX"),
        "",
        &["Icosphere", "Cube.001"],
    );
    let out = evaluate(&s).0.unwrap();
    let res = fixture_result(&fx["evaluated"]["Cube"], &[Some("Material"), None]);
    compare_t2("manifold_collection", &s, &out, &res);
}
