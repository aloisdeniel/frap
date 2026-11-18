// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the Blender golden tests: the fixture (`tests/fixtures/uv.blender.json`,
//! written by `tests/blender/dump_uv.py`), the edit-mode mesh as plain arrays, and replays of
//! the `uvedit_unwrap_ops.cc` operator pipelines on top of `frap_uv` (what `frap-ops-uv`,
//! WP 3.8, does on the edit BMesh): `construct_param_handle`, `uvedit_unwrap`,
//! `uvedit_pack_islands_multi` with `bm_mesh_calc_uv_islands`, `smart_project_exec`,
//! `cube/cylinder/sphere_project_exec` and `average_islands_scale_exec`. Every face is selected
//! and there is no image, so the aspect ratio is 1 (as in the fixture).
#![allow(dead_code)]

use std::path::PathBuf;

use frap_uv::pack::{
    self, MarginMethod, PackIsland, PackParams, PinMethod, RotationMethod, ShapeMethod,
};
use frap_uv::project::{self, CurvedProjection, ProjectAlign, ProjectDirection, ProjectMesh};
use frap_uv::{ConstructOpts, ParamFace, ParamMesh, SolveCount};
use serde_json::Value as J;

/// The fixture file.
pub fn fixture() -> J {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/uv.blender.json");
    serde_json::from_str(&std::fs::read_to_string(p).expect("read fixture")).expect("parse")
}

/// A case's edit mesh: its own `before` (pinned cases) or its setup's.
pub fn before<'a>(fx: &'a J, case: &'a J) -> &'a J {
    match case.get("before") {
        Some(b) => b,
        None => &fx["setups"][case["setup"].as_str().expect("setup")],
    }
}

fn f32s<const N: usize>(v: &J) -> [f32; N] {
    let a = v.as_array().expect("array");
    core::array::from_fn(|i| a[i].as_f64().expect("number") as f32)
}

/// The edit-mode mesh of a case (`before`).
#[derive(Clone, Debug)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub edges: Vec<[usize; 2]>,
    pub faces: Vec<Vec<usize>>,
    pub face_offsets: Vec<usize>,
    pub corner_edges: Vec<usize>,
    pub seams: Vec<bool>,
    pub face_normals: Vec<[f32; 3]>,
    pub face_areas: Vec<f32>,
    pub uvs: Vec<[f32; 2]>,
    /// `.pn.UVMap` per corner.
    pub pins: Vec<bool>,
    /// Corners of each edge in creation order (the radial cycle).
    pub edge_corners: Vec<Vec<usize>>,
    pub corner_face: Vec<usize>,
}

impl Mesh {
    pub fn from_json(b: &J) -> Mesh {
        let positions: Vec<[f32; 3]> = b["positions"]
            .as_array()
            .unwrap()
            .iter()
            .map(f32s::<3>)
            .collect();
        let edges: Vec<[usize; 2]> = b["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                [
                    e[0].as_u64().unwrap() as usize,
                    e[1].as_u64().unwrap() as usize,
                ]
            })
            .collect();
        let faces: Vec<Vec<usize>> = b["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize)
                    .collect()
            })
            .collect();
        let mut face_offsets = vec![0usize];
        for f in &faces {
            face_offsets.push(face_offsets.last().unwrap() + f.len());
        }
        let corner_edges: Vec<usize> = b["corner_edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        let mut seams = vec![false; edges.len()];
        for s in b["seams"].as_array().unwrap() {
            seams[s.as_u64().unwrap() as usize] = true;
        }
        let mut edge_corners = vec![Vec::new(); edges.len()];
        let mut corner_face = vec![0; corner_edges.len()];
        for (f, w) in face_offsets.windows(2).enumerate() {
            for c in w[0]..w[1] {
                edge_corners[corner_edges[c]].push(c);
                corner_face[c] = f;
            }
        }
        Mesh {
            positions,
            edges,
            face_offsets,
            corner_edges,
            seams,
            face_normals: b["face_normals"]
                .as_array()
                .unwrap()
                .iter()
                .map(f32s::<3>)
                .collect(),
            face_areas: b["face_areas"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect(),
            uvs: b["uvs"].as_array().unwrap().iter().map(f32s::<2>).collect(),
            pins: b["pins"]
                .as_array()
                .map(|a| a.iter().map(|v| v.as_bool().unwrap()).collect())
                .unwrap_or_else(|| vec![false; corner_face.len()]),
            faces,
            edge_corners,
            corner_face,
        }
    }

    pub fn corners(&self, f: usize) -> core::ops::Range<usize> {
        self.face_offsets[f]..self.face_offsets[f + 1]
    }

    /// `l->next`.
    pub fn corner_next(&self, c: usize) -> usize {
        let f = self.corner_face[c];
        let r = self.corners(f);
        if c + 1 == r.end { r.start } else { c + 1 }
    }

    pub fn corner_vert(&self, c: usize) -> usize {
        let f = self.corner_face[c];
        self.faces[f][c - self.face_offsets[f]]
    }

    /// `l->radial_next` (`bmesh_radial_loop_append` keeps creation order cyclic).
    pub fn radial_next(&self, c: usize) -> usize {
        let list = &self.edge_corners[self.corner_edges[c]];
        let i = list.iter().position(|&x| x == c).unwrap();
        list[(i + 1) % list.len()]
    }
}

impl ProjectMesh for Mesh {
    fn face_count(&self) -> usize {
        self.faces.len()
    }
    fn face_corners(&self, f: usize) -> core::ops::Range<usize> {
        self.corners(f)
    }
    fn corner_co(&self, c: usize) -> [f32; 3] {
        self.positions[self.corner_vert(c)]
    }
    fn corner_edge(&self, c: usize) -> usize {
        self.corner_edges[c]
    }
    fn edge_is_seam(&self, e: usize) -> bool {
        self.seams[e]
    }
    fn edge_faces(&self, e: usize, out: &mut Vec<usize>) {
        // `BM_FACES_OF_EDGE` starts at `e->l`, the last loop appended.
        let list = &self.edge_corners[e];
        if let Some(&last) = list.last() {
            out.push(self.corner_face[last]);
            out.extend(list[..list.len() - 1].iter().map(|&c| self.corner_face[c]));
        }
    }
    fn face_selected(&self, _f: usize) -> bool {
        true
    }
}

/// `bm_mesh_calc_uv_islands` (`BM_mesh_calc_face_groups` over UV-connected, non-seam edges).
pub fn calc_uv_islands(mesh: &Mesh, uvs: &[[f32; 2]], use_seams: bool) -> Vec<Vec<usize>> {
    let shared = |la: usize, lb: usize| -> bool {
        if use_seams && mesh.seams[mesh.corner_edges[la]] {
            return false;
        }
        let (mut b_curr, mut b_next) = (uvs[lb], uvs[mesh.corner_next(lb)]);
        if mesh.corner_vert(la) != mesh.corner_vert(lb) {
            core::mem::swap(&mut b_curr, &mut b_next);
        }
        uvs[la] == b_curr && uvs[mesh.corner_next(la)] == b_next
    };
    let n = mesh.faces.len();
    let mut tag = vec![false; n];
    let mut groups = Vec::new();
    let mut f_next = 0;
    while f_next < n {
        if tag[f_next] {
            f_next += 1;
            continue;
        }
        tag[f_next] = true;
        let mut stack = vec![f_next];
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            group.push(f);
            for l in mesh.corners(f) {
                let mut lr = mesh.radial_next(l);
                while lr != l {
                    if shared(l, lr) {
                        let fo = mesh.corner_face[lr];
                        if !tag[fo] {
                            tag[fo] = true;
                            stack.push(fo);
                        }
                    }
                    lr = mesh.radial_next(lr);
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// `uvedit_pack_islands_multi` for one mesh (`base_offset` 0; islands with pinned UVs are
/// skipped with `PIN_IGNORE`, `island_has_pins`).
pub fn pack_islands_multi(mesh: &Mesh, uvs: &mut [[f32; 2]], params: &PackParams) {
    let mut islands = calc_uv_islands(mesh, uvs, params.use_seams);
    let pinned: Vec<bool> = islands
        .iter()
        .map(|faces| faces.iter().any(|&f| mesh.corners(f).any(|c| mesh.pins[c])))
        .collect();
    let mut pinned_kept = Vec::new();
    let mut kept = Vec::new();
    for (faces, p) in islands.drain(..).zip(pinned) {
        if params.pin_method == PinMethod::Ignore && p {
            continue;
        }
        kept.push(faces);
        pinned_kept.push(p);
    }
    let islands = kept;
    if islands.is_empty() {
        return;
    }
    let mut packs: Vec<PackIsland> = islands
        .iter()
        .enumerate()
        .map(|(i, faces)| {
            let mut p = PackIsland::new();
            p.caller_index = i as i32;
            p.aspect_y = 1.0;
            p.pinned = pinned_kept[i];
            for &f in faces {
                let poly: Vec<[f32; 2]> = mesh.corners(f).map(|c| uvs[c]).collect();
                p.add_polygon(&poly);
            }
            p
        })
        .collect();
    let scale = pack::pack_islands(&mut packs, params);
    for p in &packs {
        let faces = &islands[p.caller_index as usize];
        for &f in faces {
            pack::apply_to_uvs(p, scale, params, &mut uvs[mesh.corners(f)]);
        }
    }
}

/// `UVPackIsland_Params` as `unwrap_exec` sets them.
pub fn unwrap_pack_params(margin: f32, margin_method: MarginMethod) -> PackParams {
    let mut p = PackParams::default();
    p.only_selected_uvs = false;
    p.only_selected_faces = true;
    p.use_seams = true;
    p.correct_aspect = true;
    p.pin_unselected = false;
    p.rotate_method = RotationMethod::Any;
    p.pin_method = PinMethod::Ignore;
    p.margin_method = margin_method;
    p.margin = margin;
    p
}

/// `construct_param_handle` for all faces (all selected, pins from `mesh.pins`).
pub fn construct_param_handle(
    mesh: &Mesh,
    uvs: &[[f32; 2]],
    opts: ConstructOpts,
) -> (ParamMesh, u32) {
    let mut pm = ParamMesh::new();
    pm.set_aspect_ratio(1.0);
    // `uvedit_prepare_pinned_indices`.
    for f in 0..mesh.faces.len() {
        for c in mesh.corners(f) {
            if mesh.pins[c] {
                pm.prepare_pin_index(mesh.corner_vert(c) as i32, uvs[c]);
            }
        }
    }
    for f in 0..mesh.faces.len() {
        let r = mesh.corners(f);
        let vkeys: Vec<u64> = r
            .clone()
            .map(|c| pm.find_pin_index(mesh.corner_vert(c) as i32, uvs[c]))
            .collect();
        let co: Vec<[f32; 3]> = r
            .clone()
            .map(|c| mesh.positions[mesh.corner_vert(c)])
            .collect();
        let uv: Vec<u32> = r.clone().map(|c| c as u32).collect();
        let uv_init: Vec<[f32; 2]> = r.clone().map(|c| uvs[c]).collect();
        let weight = vec![1.0f32; r.len()];
        let pin: Vec<bool> = r.clone().map(|c| mesh.pins[c]).collect();
        let select = vec![true; r.len()];
        let mut face = ParamFace::new(f as u64, &vkeys, &co, &uv, &uv_init, &pin, &select);
        face.weight = Some(&weight);
        pm.face_add(&face);
    }
    // `construct_param_edge_set_seams`.
    if !opts.topology_from_uvs {
        for (e, &seam) in mesh.seams.iter().enumerate() {
            if !seam {
                continue;
            }
            for &l in &mesh.edge_corners[e] {
                let ln = mesh.corner_next(l);
                let k0 = pm.find_pin_index(mesh.corner_vert(l) as i32, uvs[l]);
                let k1 = pm.find_pin_index(mesh.corner_vert(ln) as i32, uvs[ln]);
                pm.edge_set_seam([k0, k1]);
            }
        }
    }
    let failed = pm.construct_end(opts);
    (pm, failed)
}

/// `unwrap_exec` (CONFORMAL, or ANGLE_BASED through the LSCM fallback) followed by its pack.
pub fn unwrap(
    mesh: &Mesh,
    uvs: &mut [[f32; 2]],
    abf: bool,
    fill_holes: bool,
    margin: f32,
) -> (SolveCount, u32) {
    let (mut pm, failed) = construct_param_handle(mesh, uvs, ConstructOpts::new(fill_holes, false));
    pm.lscm_begin(false, abf, false);
    let count = pm.lscm_solve();
    pm.lscm_end();
    pm.average(true, false, false);
    pm.flush(uvs);
    pack_islands_multi(mesh, uvs, &unwrap_pack_params(margin, MarginMethod::Scaled));
    (count, failed)
}

/// `smart_project_exec` with the operator defaults (`angle_limit` 66°, `island_margin` 0,
/// `area_weight` 0, `rotate_method='AXIS_ALIGNED_Y'`, `margin_method='SCALED'`).
pub fn smart_project(mesh: &Mesh, uvs: &mut [[f32; 2]]) {
    // `DEG2RADF(66.0f)`.
    let angle_limit = 66.0f32 * (core::f64::consts::PI / 180.0) as f32;
    let groups =
        frap_uv::smart::project_groups(&mesh.face_normals, &mesh.face_areas, angle_limit, 0.0);
    // Zero-area faces get zero UVs.
    for (f, &a) in mesh.face_areas.iter().enumerate() {
        // `!(area > ignore)`: NaN areas count as zero.
        if a.partial_cmp(&frap_uv::smart::AREA_IGNORE) != Some(core::cmp::Ordering::Greater) {
            for c in mesh.corners(f) {
                uvs[c] = [0.0, 0.0];
            }
        }
    }
    for g in &groups {
        for &f in &g.faces {
            let r = mesh.corners(f as usize);
            let cos: Vec<[f32; 3]> = r
                .clone()
                .map(|c| mesh.positions[mesh.corner_vert(c)])
                .collect();
            frap_uv::smart::project_face(g.normal, &cos, &mut uvs[r]);
        }
    }
    let mut params = PackParams::default();
    params.rotate_method = RotationMethod::AxisAlignedY;
    params.only_selected_uvs = false;
    params.only_selected_faces = true;
    params.correct_aspect = true;
    params.use_seams = true;
    params.margin_method = MarginMethod::Scaled;
    params.margin = 0.0;
    pack_islands_multi(mesh, uvs, &params);
}

/// `pack_islands_exec` with the operator properties in `kw` (missing ones take the operator
/// defaults: `rotate=True`, `rotate_method='ANY'`, `shape_method='CONCAVE'`,
/// `margin_method='SCALED'`, `scale=True`, `merge_overlap=False`, `pin=False`; an unset `margin`
/// is `ts_margin`, the scene tool settings' `uvcalc_margin`).
pub fn pack_islands_op(mesh: &Mesh, uvs: &mut [[f32; 2]], kw: &J, ts_margin: f32) {
    let s = |k: &str, d: &str| kw.get(k).and_then(J::as_str).unwrap_or(d).to_owned();
    let b = |k: &str, d: bool| kw.get(k).and_then(J::as_bool).unwrap_or(d);
    let mut params = PackParams::default();
    params.only_selected_uvs = true;
    params.only_selected_faces = true;
    params.use_seams = false;
    params.correct_aspect = true;
    params.rotate_method = if b("rotate", true) {
        match s("rotate_method", "ANY").as_str() {
            "ANY" => RotationMethod::Any,
            "CARDINAL" => RotationMethod::Cardinal,
            "AXIS_ALIGNED" => RotationMethod::AxisAligned,
            "AXIS_ALIGNED_X" => RotationMethod::AxisAlignedX,
            "AXIS_ALIGNED_Y" => RotationMethod::AxisAlignedY,
            m => panic!("rotate_method {m}"),
        }
    } else {
        RotationMethod::None
    };
    params.scale_to_fit = b("scale", true);
    params.merge_overlap = b("merge_overlap", false);
    params.pin_method = PinMethod::None;
    params.margin_method = match s("margin_method", "SCALED").as_str() {
        "SCALED" => MarginMethod::Scaled,
        "ADD" => MarginMethod::Add,
        "FRACTION" => MarginMethod::Fraction,
        m => panic!("margin_method {m}"),
    };
    // An unset `margin` is the tool settings' `uvcalc_margin`, which `smart_project` overwrites
    // with its `island_margin` (`ts_margin`).
    params.margin = kw
        .get("margin")
        .and_then(J::as_f64)
        .map_or(ts_margin, |m| m as f32);
    params.shape_method = match s("shape_method", "CONCAVE").as_str() {
        "CONCAVE" => ShapeMethod::Concave,
        "CONVEX" => ShapeMethod::Convex,
        "AABB" => ShapeMethod::Aabb,
        m => panic!("shape_method {m}"),
    };
    pack_islands_multi(mesh, uvs, &params);
}

/// `cube_project_exec` with `cube_size` unset (from the selection bounds).
pub fn cube_project(mesh: &Mesh, uvs: &mut [[f32; 2]]) {
    let (min, max) = project::selected_bounds(mesh);
    let center = project::bounds_center(min, max);
    let dims = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let cube_size = dims[0].max(dims[1]).max(dims[2]);
    for f in 0..mesh.faces.len() {
        let r = mesh.corners(f);
        let cos: Vec<[f32; 3]> = r
            .clone()
            .map(|c| mesh.positions[mesh.corner_vert(c)])
            .collect();
        project::cube_project_face(mesh.face_normals[f], &cos, center, cube_size, &mut uvs[r]);
    }
}

/// `sphere_project_exec` / `cylinder_project_exec` headless (identity view, identity object
/// matrix), centre = selection bounds centre.
pub fn curved_project(
    mesh: &Mesh,
    uvs: &mut [[f32; 2]],
    kind: CurvedProjection,
    direction: ProjectDirection,
    fan: bool,
) {
    let rotmat = project::uv_map_transform(
        direction,
        ProjectAlign::PolarZx,
        1.0,
        None,
        &project::M4_IDENTITY,
    );
    let (min, max) = project::selected_bounds(mesh);
    let center = project::bounds_center(min, max);
    project::curved_project(mesh, kind, center, &rotmat, fan, false, uvs);
}

/// `average_islands_scale_exec`.
pub fn average_islands_scale(mesh: &Mesh, uvs: &mut [[f32; 2]], scale_uv: bool, shear: bool) {
    let (mut pm, _) = construct_param_handle(mesh, uvs, ConstructOpts::new(false, true));
    pm.average(false, scale_uv, shear);
    pm.flush(uvs);
}

/// Max absolute difference between two UV arrays.
pub fn max_diff(a: &[[f32; 2]], b: &[[f32; 2]]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x[0] - y[0]).abs().max((x[1] - y[1]).abs()))
        .fold(0.0, f32::max)
}

/// The case's expected UVs.
pub fn after(case: &J) -> Vec<[f32; 2]> {
    case["after_uvs"]
        .as_array()
        .unwrap()
        .iter()
        .map(f32s::<2>)
        .collect()
}
