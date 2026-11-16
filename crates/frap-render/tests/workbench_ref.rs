// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench vs Blender 5.2.4, per pixel, at one sample per pixel (`render_aa = 'OFF'`).
//!
//! `tests/data/workbench_ref.txt` is written by `tests/data/make_workbench_ref.py` (camera,
//! corner-split meshes, materials and scene-linear EXR pixels of six Workbench renders). The
//! scene is rebuilt as a `RenderScene` and rendered with `Workbench::with_samples(1)`. Pixels
//! on silhouettes may legitimately differ between adapters (rasterisation rules), so the test
//! allows 2 % of pixels off by more than 2e-3 (Metal, M1 Max: none; max error 8e-4). Set
//! `FRAP_REF_DEBUG=1` to list mismatching pixels.
//! This pins the ports (studio lighting, roughness/metallic packing, RANDOM hash, background,
//! `film_transparent`, mirrored objects) to Blender; the M2 SSIM gate with anti-aliasing is
//! WP 2.3's.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::*;
use frap_render::workbench::Workbench;
use frap_render::{Engine, GpuCache};
use frap_render_scene::{Film, MatKey, MeshKey, RMesh, RObject, RenderScene};
use glam::Mat4;

const FIXTURE: &str = include_str!("data/workbench_ref.txt");

struct Case {
    name: String,
    light: String,
    color_type: String,
    transparent: bool,
    single: [f32; 3],
    px: Vec<[f32; 4]>,
}

fn floats(s: &[&str]) -> Vec<f32> {
    s.iter().map(|v| v.parse::<f32>().expect("float")).collect()
}

fn mat(v: &[f32]) -> Mat4 {
    Mat4::from_cols_slice(v)
}

/// Parses the fixture into a base scene and its cases.
fn load() -> (RenderScene, Vec<Case>) {
    let mut sc = RenderScene::default();
    let mut cases = vec![];
    let mut meshes: BTreeMap<String, RMesh> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut mat_key = 0u64;
    for line in FIXTURE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("size") => sc.resolution = [t[1].parse().unwrap(), t[2].parse().unwrap()],
            Some("camera") if t[1] == "view" => sc.camera.view = mat(&floats(&t[2..])),
            Some("camera") if t[1] == "proj" => {
                // OpenGL clip depth [-1, 1] -> wgpu [0, 1].
                let gl_to_wgpu = Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
                ]);
                sc.camera.projection = gl_to_wgpu * mat(&floats(&t[2..]));
                sc.camera.is_perspective = true;
            }
            Some("object") => {
                let v = floats(&t[3..]);
                let key = sc.objects.len() as u64 + 1;
                sc.objects.push(RObject {
                    name: t[1].into(),
                    matrix_world: mat(&v[..16]),
                    mesh: Some(MeshKey(key)),
                    materials: vec![],
                    color: [v[16], v[17], v[18], v[19]],
                    random_hash: t[2].parse().unwrap(),
                });
                current = Some(t[1].to_string());
            }
            Some("material") => {
                let v = floats(&t[2..]);
                mat_key += 1;
                sc.materials.insert(
                    MatKey(mat_key),
                    Arc::new(material([v[0], v[1], v[2], v[3]], v[4], v[5])),
                );
                sc.objects
                    .last_mut()
                    .unwrap()
                    .materials
                    .push(Some(MatKey(mat_key)));
            }
            Some("tri") => {
                let v = floats(&t[1..]);
                let m = meshes.entry(current.clone().unwrap()).or_default();
                let base = m.positions.len() as u32;
                for i in 0..3 {
                    m.positions.push([v[3 * i], v[3 * i + 1], v[3 * i + 2]]);
                    m.normals
                        .push([v[9 + 3 * i], v[9 + 3 * i + 1], v[9 + 3 * i + 2]]);
                }
                m.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
            Some("case") => cases.push(Case {
                name: t[1].into(),
                light: t[2].into(),
                color_type: t[3].into(),
                transparent: t[4] == "1",
                single: [t[5], t[6], t[7]].map(|s| s.parse().unwrap()),
                px: vec![],
            }),
            Some("px") => {
                let v = floats(&t[1..]);
                cases.last_mut().unwrap().px =
                    v.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
            }
            _ => {}
        }
    }
    for (i, o) in sc.objects.iter().enumerate() {
        let mut m = meshes.remove(o.name.as_str()).unwrap();
        let n = m.indices.len() as u32;
        m.submeshes = vec![(0, 0..n)];
        sc.meshes.insert(MeshKey(i as u64 + 1), Arc::new(m));
    }
    (sc, cases)
}

/// `BLI_hash_mm2(data, len, 0)` (MurmurHash2, seed 0).
fn murmur2(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let r = chunks.remainder();
    if r.len() >= 3 {
        h ^= u32::from(r[2]) << 16;
    }
    if r.len() >= 2 {
        h ^= u32::from(r[1]) << 8;
    }
    if !r.is_empty() {
        h ^= u32::from(r[0]);
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

#[test]
fn fixture_random_hash_is_murmur_of_id_name() {
    let (sc, _) = load();
    for o in &sc.objects {
        assert_eq!(
            o.random_hash,
            // `BLI_ghashutil_strhash_p_murmur` hashes `strlen + 1` bytes: the NUL is included.
            murmur2(format!("OB{}\0", o.name).as_bytes()),
            "{}",
            o.name
        );
    }
}

struct Stats {
    bad: usize,
    total: usize,
    max_good: f32,
}

fn compare(frap: &Film, blender: &[[f32; 4]], tol: f32) -> Stats {
    let mut s = Stats {
        bad: 0,
        total: blender.len(),
        max_good: 0.0,
    };
    for (a, b) in frap.rgba.iter().zip(blender) {
        let d = (0..4).map(|i| (a[i] - b[i]).abs()).fold(0.0f32, f32::max);
        if d > tol {
            s.bad += 1;
        } else {
            s.max_good = s.max_good.max(d);
        }
    }
    s
}

#[test]
fn single_sample_renders_match_blender_workbench() {
    let Some(g) = gpu() else { return };
    let (base, cases) = load();
    assert_eq!(cases.len(), 6);
    let mut wb = Workbench::with_samples(1);
    let mut cache = GpuCache::default();
    let mut failures = vec![];
    for c in &cases {
        let mut sc = base.clone();
        sc.settings.shading_light = c.light.as_str().into();
        sc.settings.shading_color_type = c.color_type.as_str().into();
        sc.settings.shading_single_color = c.single;
        sc.settings.film_transparent = c.transparent;
        let film = wb.render(&g, &mut cache, &sc).expect("render");
        assert_eq!(film.rgba.len(), c.px.len());
        // Blender stores colours in half floats and normals as RG16F octahedral pairs; on
        // Metal every pixel of all six cases is within 1e-3 of Blender (max 8e-4).
        let tol = 2e-3;
        let s = compare(&film, &c.px, tol);
        let share = s.bad as f64 / s.total as f64;
        eprintln!(
            "{:<24} mismatching {:>4}/{} ({:.2}%), max error elsewhere {:.5}",
            c.name,
            s.bad,
            s.total,
            share * 100.0,
            s.max_good
        );
        if std::env::var("FRAP_REF_DEBUG").is_ok() {
            for (i, (a, b)) in film.rgba.iter().zip(&c.px).enumerate() {
                let d = (0..4).map(|k| (a[k] - b[k]).abs()).fold(0.0f32, f32::max);
                if d > tol {
                    eprintln!("  ({}, {}) frap {:?} blender {:?}", i % 40, i / 40, a, b);
                }
            }
        }
        if share > 0.02 {
            failures.push(format!(
                "{}: {:.2}% of pixels off by > {tol}",
                c.name,
                share * 100.0
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
