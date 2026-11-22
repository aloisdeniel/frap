// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.rip` against interactive Blender 5.2.4 recordings
//! (`tests/golden/cases/edit/tools_m8/rip_recordings/*.json`, recorded with
//! `--enable-event-simulate` as `tests/golden/cases/edit/tools_m8/README.md` describes): the
//! mesh before the call (positions, edges, faces, sharpness, custom normals, selection and
//! history), the region, view and window matrices and the mouse position of the interactive
//! session, and the mesh, selection, history and encoded custom normals after it, or the
//! error it reported.
//!
//! - every recording replays exactly with the recorded region, matrices and mouse (the port);
//! - the recordings made in an orthographic view along a direction replay exactly with frap's
//!   own region for `frap_direction`/`frap_location` ([`RegionView::ortho_along`]);
//! - the cursor recordings replay exactly in frap's default region (the factory Layout 3D View
//!   of `blender -b`, D-153).

use std::path::PathBuf;

use frap_bmesh::{BMElem, BMesh, SelectMode, ToMeshParams};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use serde_json::Value as J;

use super::view::RegionView;
use super::{MeshRip, rip_mesh, rip_report};

fn recordings() -> Vec<(String, J)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/cases/edit/tools_m8/rip_recordings");
    let mut out: Vec<(String, J)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("readable directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let s = std::fs::read_to_string(&p).expect("readable recording");
            let j: J = serde_json::from_str(&s).expect("recording JSON");
            (p.file_stem().unwrap().to_string_lossy().into_owned(), j)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(out.len() >= 20, "{} recordings", out.len());
    out
}

fn f32s(v: &J) -> Vec<f32> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_f64().expect("number") as f32)
        .collect()
}
fn v3(v: &J) -> [f32; 3] {
    let a = f32s(v);
    [a[0], a[1], a[2]]
}
fn m4(v: &J) -> [[f32; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for (c, col) in v.as_array().expect("matrix").iter().enumerate() {
        let a = f32s(col);
        m[c].copy_from_slice(&a);
    }
    m
}
fn ints(v: &J) -> Vec<i32> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_i64().expect("int") as i32)
        .collect()
}
fn bools(v: &J) -> Option<Vec<bool>> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_bool().expect("bool")).collect())
}

/// The recorded mesh state as a frap mesh.
fn mesh_of(state: &J) -> Mesh {
    let mut me = Mesh::new("Rip");
    me.set_positions(state["verts"].as_array().unwrap().iter().map(v3).collect());
    let edges: Vec<[i32; 2]> = state["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let e = ints(e);
            [e[0], e[1]]
        })
        .collect();
    let mut offsets = vec![0];
    let mut corner_verts = Vec::new();
    for f in state["faces"].as_array().unwrap() {
        corner_verts.extend(ints(f));
        offsets.push(corner_verts.len() as i32);
    }
    me.set_topology(offsets, corner_verts, edges);
    for (key, name, domain) in [
        ("sharp_edge", frap_mesh::SHARP_EDGE, Domain::Edge),
        ("sharp_face", frap_mesh::SHARP_FACE, Domain::Face),
        ("select_vert", frap_mesh::SELECT_VERT, Domain::Point),
        ("select_edge", frap_mesh::SELECT_EDGE, Domain::Edge),
        ("select_poly", frap_mesh::SELECT_POLY, Domain::Face),
    ] {
        if let Some(v) = bools(&state[key]) {
            me.set_bool_attribute(name, domain, v);
        }
    }
    if let Some(cn) = state["custom_normal"].as_array() {
        let data = cn
            .iter()
            .map(|x| {
                let a = ints(x);
                [a[0] as i16, a[1] as i16]
            })
            .collect();
        me.attributes_mut().insert(Attribute {
            name: frap_mesh::CUSTOM_NORMAL.into(),
            domain: Domain::Corner,
            data: AttrData::Int16x2(std::sync::Arc::new(data)),
        });
    }
    me
}

fn history(bm: &BMesh) -> Vec<(String, usize)> {
    let (vi, ei, fi) = (bm.vert_ids(), bm.edge_ids(), bm.face_ids());
    bm.select_history
        .iter()
        .map(|h| match *h {
            BMElem::Vert(v) => ("VERT".into(), vi.iter().position(|&x| x == v).unwrap()),
            BMElem::Edge(e) => ("EDGE".into(), ei.iter().position(|&x| x == e).unwrap()),
            BMElem::Face(f) => ("FACE".into(), fi.iter().position(|&x| x == f).unwrap()),
        })
        .collect()
}

fn recorded_history(state: &J) -> Vec<(String, usize)> {
    state["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            (
                h[0].as_str().unwrap().to_owned(),
                h[1].as_u64().unwrap() as usize,
            )
        })
        .collect()
}

/// The edit BMesh of the recorded state before the call.
fn bmesh_before(rec: &J) -> (Mesh, BMesh) {
    let before = &rec["before"];
    let me = mesh_of(before);
    let sm = bools(&rec["select_mode"]).unwrap();
    let mut mode = SelectMode::empty();
    for (on, m) in sm
        .iter()
        .zip([SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE])
    {
        if *on {
            mode |= m;
        }
    }
    let mut bm = BMesh::edit_mesh_make(&me, mode);
    let (vi, ei, fi) = (bm.vert_ids(), bm.edge_ids(), bm.face_ids());
    for (kind, i) in recorded_history(before) {
        let e = match kind.as_str() {
            "VERT" => BMElem::Vert(vi[i]),
            "EDGE" => BMElem::Edge(ei[i]),
            _ => BMElem::Face(fi[i]),
        };
        bm.select_history_store(e);
    }
    (me, bm)
}

/// Runs the rip on the recorded mesh in `region` with the mouse at `mval` and checks the
/// outcome against the recording; `Err` describes the first difference.
fn replay(rec: &J, region: &RegionView, mval: [f32; 2]) -> Result<(), String> {
    let (me, mut bm) = bmesh_before(rec);
    let obmat = m4(&rec["obmat"]);
    let use_fill = rec["kw"]["use_fill"].as_bool().unwrap_or(false);
    let outcome = rip_mesh(&mut bm, region, &obmat, mval, use_fill);
    let report = rip_report(&[outcome]);
    match rec["result"].as_str().unwrap() {
        "FINISHED" => {
            if report != Some(Ok(())) {
                return Err(format!("FINISHED recorded, frap {outcome:?} / {report:?}"));
            }
        }
        "ERROR" => {
            let msg = rec["error"].as_str().unwrap().trim();
            let msg = msg.strip_prefix("Error: ").unwrap_or(msg);
            if report != Some(Err(msg)) {
                return Err(format!("error {msg:?} recorded, frap {report:?}"));
            }
            // Nothing observable changed: the operator raises and frap rolls the call back.
            return Ok(());
        }
        r => return Err(format!("unexpected recorded result {r}")),
    }
    assert_eq!(outcome, MeshRip::Ripped);
    let after = &rec["after"];
    let out = bm.to_mesh(&me, &ToMeshParams::default());
    let want = mesh_of(after);
    if out.positions() != want.positions() {
        return Err(format!(
            "positions: frap {:?}, Blender {:?}",
            out.positions(),
            want.positions()
        ));
    }
    if out.edges() != want.edges() {
        return Err(format!(
            "edges: frap {:?}, Blender {:?}",
            out.edges(),
            want.edges()
        ));
    }
    if out.face_offsets() != want.face_offsets() || out.corner_verts() != want.corner_verts() {
        return Err(format!(
            "faces: frap {:?} {:?}, Blender {:?} {:?}",
            out.face_offsets(),
            out.corner_verts(),
            want.face_offsets(),
            want.corner_verts()
        ));
    }
    for (name, domain, n) in [
        (frap_mesh::SELECT_VERT, Domain::Point, out.verts_num()),
        (frap_mesh::SELECT_EDGE, Domain::Edge, out.edges_num()),
        (frap_mesh::SELECT_POLY, Domain::Face, out.faces_num()),
    ] {
        let got = out
            .bool_attribute(name, domain)
            .map_or(vec![false; n], <[bool]>::to_vec);
        let exp = want
            .bool_attribute(name, domain)
            .map_or(vec![false; n], <[bool]>::to_vec);
        if got != exp {
            return Err(format!("{name}: frap {got:?}, Blender {exp:?}"));
        }
    }
    let (h, hw) = (history(&bm), recorded_history(after));
    if h != hw {
        return Err(format!("history: frap {h:?}, Blender {hw:?}"));
    }
    let cn = |m: &Mesh| match m
        .attributes()
        .get(frap_mesh::CUSTOM_NORMAL)
        .map(|a| &a.data)
    {
        Some(AttrData::Int16x2(v)) => Some(v.to_vec()),
        _ => None,
    };
    let (got, exp) = (cn(&out), cn(&want));
    if got != exp {
        let diff: Vec<String> = match (&got, &exp) {
            (Some(g), Some(e)) => g
                .iter()
                .zip(e)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| format!("corner {i}: frap {a:?}, Blender {b:?}"))
                .collect(),
            _ => vec![format!("frap {got:?}, Blender {exp:?}")],
        };
        return Err(format!("custom_normal: {}", diff.join("; ")));
    }
    Ok(())
}

fn check_all(cases: impl Iterator<Item = (String, Result<(), String>)>) {
    let failures: Vec<String> = cases
        .filter_map(|(name, r)| r.err().map(|e| format!("{name}: {e}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn recordings_replay_in_the_recorded_region() {
    check_all(recordings().into_iter().map(|(name, rec)| {
        let r = &rec["region"];
        let region = RegionView {
            win: [r[0].as_i64().unwrap() as i32, r[1].as_i64().unwrap() as i32],
            winmat: m4(&rec["winmat"]),
            viewmat: m4(&rec["viewmat"]),
        };
        let mval = f32s(&rec["mval"]);
        let res = replay(&rec, &region, [mval[0], mval[1]]);
        (name, res)
    }));
}

#[test]
fn direction_recordings_replay_in_frap_ortho_region() {
    let mut n = 0;
    check_all(recordings().into_iter().filter_map(|(name, rec)| {
        let view = rec.get("view").filter(|v| !v.is_null())?;
        n += 1;
        let (dir, loc) = (v3(&view["direction"]), v3(&view["location"]));
        let region = RegionView::ortho_along(dir, loc);
        let mval = region.mouse_over(loc);
        Some((name, replay(&rec, &region, mval)))
    }));
    assert!(n >= 5, "{n} direction recordings");
}

#[test]
fn cursor_recordings_replay_in_the_factory_region() {
    // frap's virtual view matrix (D-10): the factory Layout `view_matrix` of `startup.json`.
    let viewmat = frap_scene::startup::factory()
        .1
        .view3d
        .view_matrix
        .to_cols_array_2d();
    let factory = RegionView::factory(viewmat);
    check_all(recordings().into_iter().filter_map(|(name, rec)| {
        if rec.get("view").is_some_and(|v| !v.is_null()) {
            return None;
        }
        let cursor = v3(&rec["cursor"]);
        let mval = factory.mouse_over(cursor);
        Some((name, replay(&rec, &factory, mval)))
    }));
}
