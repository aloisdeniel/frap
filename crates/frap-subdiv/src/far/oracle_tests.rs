// SPDX-License-Identifier: GPL-3.0-or-later
//! Development cross-check against Blender's own OpenSubdiv: with `FRAP_OSD_ORACLE` set to the
//! program of `tests/fvar/oracle.cpp` (linked against Blender 5.2.4's `libosdCPU.dylib`; it
//! builds the refiner, stencil and patch tables exactly as `evaluator_impl.cc` does and dumps
//! them), every stage of this port is compared bit for bit on the meshes of `tests/data`.
//! Without the variable the test returns immediately; `tests/fvar_dumps.rs` checks the end
//! result against Blender's committed dumps.

use std::fmt::Write as _;
use std::path::PathBuf;

use super::*;
use crate::layout;

struct Case {
    name: String,
    positions: Vec<[f32; 3]>,
    edges: Vec<[u32; 2]>,
    face_offsets: Vec<u32>,
    corner_verts: Vec<u32>,
    corner_edges: Vec<u32>,
    uvs: Vec<Vec<[f32; 2]>>,
    crease_edge: Option<Vec<f32>>,
    crease_vert: Option<Vec<f32>>,
    settings: SubdivSettings,
}

fn load(path: &std::path::Path) -> Case {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let inp = &v["input"];
    let f = |x: &serde_json::Value| x.as_f64().unwrap() as f32;
    let u = |x: &serde_json::Value| x.as_u64().unwrap() as u32;
    let mut face_offsets = vec![0u32];
    let mut corner_verts = Vec::new();
    for face in inp["faces"].as_array().unwrap() {
        for c in face.as_array().unwrap() {
            corner_verts.push(u(c));
        }
        face_offsets.push(corner_verts.len() as u32);
    }
    let floats = |x: &serde_json::Value| x.as_array().map(|a| a.iter().map(f).collect::<Vec<_>>());
    let st = &v["settings"];
    let s = SubdivSettings {
        level: st["levels"].as_u64().unwrap() as u32,
        quality: st["quality"].as_u64().unwrap() as u32,
        use_limit_surface: st["use_limit_surface"].as_bool().unwrap(),
        is_simple: st["subdivision_type"] == "SIMPLE",
        use_creases: st["use_creases"].as_bool().unwrap(),
        vtx_boundary: VtxBoundary::from_boundary_smooth(st["boundary_smooth"].as_str().unwrap()),
        fvar_linear: FVarLinear::from_uv_smooth(st["uv_smooth"].as_str().unwrap()),
        ..SubdivSettings::default()
    };
    Case {
        name: path.file_stem().unwrap().to_string_lossy().into(),
        positions: inp["positions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [f(&p[0]), f(&p[1]), f(&p[2])])
            .collect(),
        edges: inp["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| [u(&e[0]), u(&e[1])])
            .collect(),
        face_offsets,
        corner_verts,
        corner_edges: inp["corner_edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(u)
            .collect(),
        uvs: inp["uvs"]
            .as_array()
            .map(|ls| {
                ls.iter()
                    .map(|l| {
                        l.as_array()
                            .unwrap()
                            .iter()
                            .map(|p| [f(&p[0]), f(&p[1])])
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default(),
        crease_edge: floats(&inp["crease_edge"]),
        crease_vert: floats(&inp["crease_vert"]),
        settings: s,
    }
}

/// Everything the oracle dumps, from this port.
fn dump(case: &Case) -> Option<(String, String)> {
    let mut topo = SubdivTopology::new(
        &case.positions,
        &case.edges,
        &case.face_offsets,
        &case.corner_verts,
        &case.corner_edges,
    );
    topo.edge_creases = case.crease_edge.as_deref();
    topo.vert_creases = case.crease_vert.as_deref();
    topo.uv_layers = case.uvs.iter().map(|l| l.as_slice()).collect();
    if topo.uv_layers.is_empty() {
        return None;
    }
    let s = &case.settings;
    let (mesh, coarse) = base_mesh(&topo, s);
    let opts = sdc_options(s);
    let mut r = refiner::TopologyRefiner::create(&mesh, opts).ok()?;
    let (adaptive, level) = refine_level(s);
    let res = (1u32 << s.level) + 1;
    let lay = layout::compute(topo.coarse(), res);

    // descriptor
    let base = &r.levels[0];
    let mut d = String::new();
    let vb = match opts.vtx_boundary {
        sdc::VtxBoundary::None => 0,
        sdc::VtxBoundary::EdgeOnly => 1,
        sdc::VtxBoundary::EdgeAndCorner => 2,
    };
    let fl = match opts.fvar_linear {
        sdc::FVarLinear::None => 0,
        sdc::FVarLinear::CornersOnly => 1,
        sdc::FVarLinear::CornersPlus1 => 2,
        sdc::FVarLinear::CornersPlus2 => 3,
        sdc::FVarLinear::Boundaries => 4,
        sdc::FVarLinear::All => 5,
    };
    writeln!(d, "{} {}", mesh.num_vertices, mesh.face_sizes.len()).unwrap();
    writeln!(d, "{vb} {fl} {} {level}", adaptive as i32).unwrap();
    for x in &mesh.face_sizes {
        write!(d, "{x} ").unwrap();
    }
    writeln!(d).unwrap();
    for x in &mesh.face_verts {
        write!(d, "{x} ").unwrap();
    }
    writeln!(d).unwrap();
    let creases: Vec<(i32, i32, f32)> = (0..base.edge_count)
        .filter(|&e| base.edge_sharpness(e) > 0.0)
        .map(|e| {
            let ev = base.edge_vertices(e);
            (ev[0], ev[1], base.edge_sharpness(e))
        })
        .collect();
    writeln!(d, "{}", creases.len()).unwrap();
    for (a, b, w) in &creases {
        writeln!(d, "{a} {b} {:08x}", w.to_bits()).unwrap();
    }
    let corners: Vec<(i32, f32)> = (0..base.vert_count)
        .filter(|&v| base.vertex_sharpness(v) > 0.0)
        .map(|v| (v, base.vertex_sharpness(v)))
        .collect();
    writeln!(d, "{}", corners.len()).unwrap();
    for (v, w) in &corners {
        writeln!(d, "{v} {:08x}", w.to_bits()).unwrap();
    }
    writeln!(d, "{}", mesh.fvar.len()).unwrap();
    for (c, (idx, n)) in mesh.fvar.iter().enumerate() {
        writeln!(d, "{n}").unwrap();
        for x in idx {
            write!(d, "{x} ").unwrap();
        }
        writeln!(d).unwrap();
        for p in &coarse[c] {
            write!(d, "{:08x} {:08x} ", p[0].to_bits(), p[1].to_bits()).unwrap();
        }
        writeln!(d).unwrap();
    }
    writeln!(d, "{}", lay.corner_ptex.len()).unwrap();
    for &(p, u, v) in &lay.corner_ptex {
        writeln!(d, "{p} {:08x} {:08x}", u.to_bits(), v.to_bits()).unwrap();
    }

    // this port's dump, in the oracle's format
    if adaptive {
        r.refine_adaptive(level, true);
    } else {
        r.refine_uniform(level);
    }
    let mut o = String::new();
    writeln!(o, "levels {}", r.levels.len()).unwrap();
    for l in &r.levels {
        write!(o, "{} {} {}", l.vert_count, l.edge_count, l.face_count).unwrap();
        for c in 0..l.num_fvar_channels() {
            write!(o, " {}", l.num_fvar_values(c)).unwrap();
        }
        writeln!(o).unwrap();
    }
    let tables: Vec<stencil::StencilTable> = (0..mesh.fvar.len())
        .map(|c| stencil::create_fvar_table(&r, c, adaptive))
        .collect();
    for t in &tables {
        dump_table(&mut o, "stencils", Some(t));
    }
    let pt = patch_table::create(&r);
    let nreg = pt.params.iter().filter(|p| p.regular).count();
    let arrays = (nreg > 0) as usize + (pt.params.len() > nreg) as usize;
    writeln!(
        o,
        "patches {} arrays {}",
        pt.params.len(),
        if pt.uniform { 1 } else { arrays }
    )
    .unwrap();
    for (i, p) in pt.params.iter().enumerate() {
        let a = if pt.uniform || (i < nreg) || nreg == 0 {
            0
        } else {
            1
        };
        write!(
            o,
            "P a{a} f{} u{} v{} d{} nq{} b{} t{} r{}",
            p.face_id,
            p.u,
            p.v,
            p.depth,
            p.non_quad_root as i32,
            p.boundary,
            p.transition,
            p.regular as i32
        )
        .unwrap();
        for c in 0..mesh.fvar.len() {
            let fp = &pt.fvar[c][i];
            write!(
                o,
                " | b{} r{} :",
                fp.param.boundary, fp.param.regular as i32
            )
            .unwrap();
            for x in &fp.points {
                write!(o, " {x}").unwrap();
            }
        }
        writeln!(o).unwrap();
    }
    let map = eval::PatchMap::new(&pt.params);
    for c in 0..mesh.fvar.len() {
        let local = &pt.local[c];
        dump_table(&mut o, "local", (local.num_stencils() > 0).then_some(local));
        let app = stencil::append_local_point_table(&r, &tables[c], local, c).unwrap();
        dump_table(&mut o, "appended", app.as_ref());
        let full = app.unwrap_or_else(|| tables[c].clone());
        let nc = full.num_control as usize;
        let mut data = vec![[0.0f32; 2]; nc + full.num_stencils()];
        data[..coarse[c].len()].copy_from_slice(&coarse[c]);
        full.eval_into(&mut data);
        writeln!(o, "values {}", data.len()).unwrap();
        for p in &data {
            writeln!(o, "{:08x} {:08x}", p[0].to_bits(), p[1].to_bits()).unwrap();
        }
        writeln!(o, "uvs {}", lay.corner_ptex.len()).unwrap();
        let base = if pt.uniform { nc } else { 0 };
        for &(p, u, v) in &lay.corner_ptex {
            let h = map.find(p as i32, u as f64, v as f64).unwrap();
            let r = eval::eval_patch(&pt.fvar[c][h], u, v, &data, base);
            writeln!(o, "{:08x} {:08x} {h}", r[0].to_bits(), r[1].to_bits()).unwrap();
        }
    }
    Some((d, o))
}

fn dump_table(o: &mut String, name: &str, t: Option<&stencil::StencilTable>) {
    let Some(t) = t else {
        writeln!(o, "{name} 0 0").unwrap();
        return;
    };
    writeln!(o, "{name} {} {}", t.num_stencils(), t.num_control).unwrap();
    for i in 0..t.num_stencils() {
        let (idx, w) = t.stencil(i);
        write!(o, "{}", idx.len()).unwrap();
        for j in 0..idx.len() {
            write!(o, " {}:{:08x}", idx[j], w[j].to_bits()).unwrap();
        }
        writeln!(o).unwrap();
    }
}

/// The first differing line of two dumps, with its section.
fn first_difference(a: &str, b: &str) -> Option<String> {
    let mut section = String::new();
    for (i, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && !x.starts_with('P') {
            section = x.to_string();
        }
        if x != y {
            return Some(format!(
                "line {} (after `{section}`):\n  oracle: {x}\n  port:   {y}",
                i + 1
            ));
        }
    }
    let (na, nb) = (a.lines().count(), b.lines().count());
    (na != nb).then(|| format!("line counts {na} vs {nb}"))
}

#[test]
fn matches_blender_opensubdiv_oracle() {
    let Ok(oracle) = std::env::var("FRAP_OSD_ORACLE") else {
        return;
    };
    let work =
        PathBuf::from(std::env::var("FRAP_OSD_ORACLE_DIR").unwrap_or_else(|_| "/tmp".into()));
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let filter = std::env::var("FRAP_OSD_ORACLE_FILTER").unwrap_or_default();
    let mut names: Vec<PathBuf> = std::fs::read_dir(&data)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    let mut failures = Vec::new();
    for p in names {
        let case = load(&p);
        if !case.name.contains(&filter) {
            continue;
        }
        let Some((desc, port)) = dump(&case) else {
            continue;
        };
        let dp = work.join(format!("{}.desc", case.name));
        let op = work.join(format!("{}.oracle", case.name));
        std::fs::write(&dp, desc).unwrap();
        std::fs::write(work.join(format!("{}.port", case.name)), &port).unwrap();
        let st = std::process::Command::new(&oracle)
            .arg(&dp)
            .arg(&op)
            .status()
            .unwrap();
        assert!(st.success(), "{}: oracle failed", case.name);
        let want = std::fs::read_to_string(&op).unwrap();
        match first_difference(&want, &port) {
            None => eprintln!("{}: identical", case.name),
            Some(d) => {
                eprintln!("{}: {d}", case.name);
                failures.push(case.name.clone());
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// `xorshift64*`
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random mesh: a `w` x `h` torus grid (closed, or opened along a seam), some faces removed,
/// some quads split into triangles, UV islands per face (seams where neighbours differ),
/// random creases and settings.
fn random_case(seed: u64) -> Case {
    let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let w = 3 + r.below(5) as u32;
    let h = 3 + r.below(4) as u32;
    let open = r.below(3) == 0;
    let vid = |i: u32, j: u32| (j % h) * w + (i % w);
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let jmax = if open { h - 1 } else { h };
    for j in 0..jmax {
        let mut i = 0;
        while i < w {
            let q = [vid(i, j), vid(i + 1, j), vid(i + 1, j + 1), vid(i, j + 1)];
            match r.below(14) {
                0 => {}
                1 | 2 => {
                    faces.push(vec![q[0], q[1], q[2]]);
                    faces.push(vec![q[0], q[2], q[3]]);
                }
                3 if i + 2 < w => {
                    // A hexagon over two cells.
                    faces.push(vec![
                        q[0],
                        q[1],
                        vid(i + 2, j),
                        vid(i + 2, j + 1),
                        q[2],
                        q[3],
                    ]);
                    i += 1;
                }
                _ => faces.push(q.to_vec()),
            }
            i += 1;
        }
    }
    // Non-manifold fins: an extra quad on an existing edge, with two new vertices.
    let mut extra: Vec<(u32, u32)> = Vec::new();
    let fins = if r.below(4) == 0 { 1 + r.below(2) } else { 0 };
    for _ in 0..fins {
        if faces.is_empty() {
            break;
        }
        let f = r.below(faces.len() as u64) as usize;
        let k = r.below(faces[f].len() as u64) as usize;
        let (a, b) = (faces[f][k], faces[f][(k + 1) % faces[f].len()]);
        let base = w * h + extra.len() as u32;
        extra.push((100 + base, 0));
        extra.push((101 + base, 0));
        faces.push(vec![a, b, base, base + 1]);
    }
    let nv0 = (w * h) as usize + extra.len();
    let mut used = vec![u32::MAX; nv0];
    let mut n = 0;
    for f in &faces {
        for &v in f {
            if used[v as usize] == u32::MAX {
                used[v as usize] = n;
                n += 1;
            }
        }
    }
    for f in &mut faces {
        for v in f.iter_mut() {
            *v = used[*v as usize];
        }
    }
    let positions = vec![[0.0f32; 3]; n as usize];
    let mut edges: Vec<[u32; 2]> = Vec::new();
    let mut emap = std::collections::HashMap::new();
    let mut face_offsets = vec![0u32];
    let mut corner_verts = Vec::new();
    let mut corner_edges = Vec::new();
    for f in &faces {
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            let key = (a.min(b), a.max(b));
            let e = *emap.entry(key).or_insert_with(|| {
                edges.push([a, b]);
                edges.len() as u32 - 1
            });
            corner_verts.push(a);
            corner_edges.push(e);
        }
        face_offsets.push(corner_verts.len() as u32);
    }
    let mut inv = vec![(0u32, 0u32); n as usize];
    for (orig, &nw) in used.iter().enumerate() {
        if nw != u32::MAX {
            inv[nw as usize] = if orig < (w * h) as usize {
                (orig as u32 % w, orig as u32 / w)
            } else {
                extra[orig - (w * h) as usize]
            };
        }
    }
    let islands = 1 + r.below(3);
    let mut uvs = Vec::new();
    for f in &faces {
        let isl = r.below(islands) as f32;
        let (si, sj) = inv[f[0] as usize];
        for &v in f {
            let (mut i, mut j) = inv[v as usize];
            if i + 1 < si {
                i += w;
            }
            if j + 1 < sj {
                j += h;
            }
            uvs.push([0.1 * i as f32 + 3.0 * isl, 0.1 * j as f32]);
        }
    }
    let crease_edge = (r.below(2) == 0).then(|| {
        (0..edges.len())
            .map(|_| match r.below(6) {
                0 => 1.0,
                1 => 0.3 + 0.1 * r.below(6) as f32,
                _ => 0.0,
            })
            .collect()
    });
    let crease_vert = (r.below(3) == 0).then(|| {
        (0..n)
            .map(|_| if r.below(8) == 0 { 0.5 } else { 0.0 })
            .collect()
    });
    let modes = [
        "NONE",
        "PRESERVE_CORNERS",
        "PRESERVE_CORNERS_AND_JUNCTIONS",
        "PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE",
        "PRESERVE_BOUNDARIES",
        "SMOOTH_ALL",
    ];
    let settings = SubdivSettings {
        level: 1 + r.below(3) as u32,
        quality: 1 + r.below(3) as u32,
        use_limit_surface: r.below(4) != 0,
        is_simple: r.below(5) == 0,
        use_creases: true,
        vtx_boundary: if r.below(2) == 0 {
            VtxBoundary::EdgeOnly
        } else {
            VtxBoundary::EdgeAndCorner
        },
        fvar_linear: FVarLinear::from_uv_smooth(modes[r.below(6) as usize]),
        ..SubdivSettings::default()
    };
    Case {
        name: format!("random_{seed}"),
        positions,
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
        uvs: vec![uvs],
        crease_edge,
        crease_vert,
        settings,
    }
}

/// `FRAP_OSD_ORACLE_FUZZ=<count>`: random meshes through both, stage by stage.
#[test]
fn random_meshes_match_blender_opensubdiv_oracle() {
    let (Ok(oracle), Ok(count)) = (
        std::env::var("FRAP_OSD_ORACLE"),
        std::env::var("FRAP_OSD_ORACLE_FUZZ"),
    ) else {
        return;
    };
    let work =
        PathBuf::from(std::env::var("FRAP_OSD_ORACLE_DIR").unwrap_or_else(|_| "/tmp".into()));
    let count: u64 = count.parse().unwrap();
    let mut failures = Vec::new();
    for seed in 1..=count {
        let case = random_case(seed);
        let Some((desc, port)) = dump(&case) else {
            continue;
        };
        let dp = work.join(format!("{}.desc", case.name));
        let op = work.join(format!("{}.oracle", case.name));
        std::fs::write(&dp, desc).unwrap();
        std::fs::write(work.join(format!("{}.port", case.name)), &port).unwrap();
        let st = std::process::Command::new(&oracle)
            .arg(&dp)
            .arg(&op)
            .status()
            .unwrap();
        if !st.success() {
            eprintln!("{}: oracle failed", case.name);
            continue;
        }
        let want = std::fs::read_to_string(&op).unwrap();
        if let Some(d) = first_difference(&want, &port) {
            eprintln!("{}: {d}", case.name);
            failures.push(case.name.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {count}: {failures:?}",
        failures.len()
    );
}
