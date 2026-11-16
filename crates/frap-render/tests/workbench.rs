// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.7 acceptance: synthetic `RenderScene`s through the Workbench engine on the real adapter
//! (Metal locally; lavapipe / WARP in `.github/workflows/gpu.yml`). Fails without an adapter
//! unless `FRAP_GPU_OPTIONAL=1`.

mod common;

use std::sync::Arc;
use std::time::Instant;

use common::*;
use frap_render::workbench::{Workbench, color};
use frap_render::{Engine, GpuCache, GpuOptions, Renderer, new_service};
use frap_render_scene::{Film, MatKey, MeshKey, RWorld, RenderError, RenderScene, RenderService};
use glam::{Mat4, Vec3};

const BG: [f32; 3] = [0.050876, 0.050876, 0.050876];

fn close(a: &[f32], b: &[f32], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

/// Renders with a fresh engine (`samples` per pixel) on the shared test device.
fn render_with(samples: u32, scene: &RenderScene) -> Option<Result<Film, RenderError>> {
    let g = gpu()?;
    let mut wb = Workbench::with_samples(samples);
    let mut cache = GpuCache::default();
    Some(wb.render(&g, &mut cache, scene))
}

fn render(scene: &RenderScene) -> Option<Film> {
    render_with(8, scene).map(|r| r.expect("render"))
}

fn centre(f: &Film) -> [f32; 4] {
    px(f, f.width / 2, f.height / 2)
}

#[test]
fn service_renders_solid_studio_material() {
    // Through the public contract: `new_service` + `RenderService::render`.
    let mut svc = new_service(&GpuOptions::from_env()).expect("service");
    let scene = cube_scene(96, 64);
    let film = match svc.render(&scene) {
        Ok(f) => f,
        Err(e @ RenderError::NoAdapter(_)) if gpu_optional() => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    check_adapter(&svc.adapter_info());
    assert_eq!((film.width, film.height), (96, 64));
    assert_eq!(film.rgba.len(), 96 * 64);
    assert!(film.rgba.iter().flatten().all(|c| c.is_finite()));
    // Corners: the factory world colour, opaque.
    for (x, y) in [(0, 0), (95, 0), (0, 63), (95, 63)] {
        let p = px(&film, x, y);
        assert!(close(&p, &[BG[0], BG[1], BG[2], 1.0], 1e-6), "{p:?}");
    }
    // Centre: the red cube, lit (neither the flat base colour nor black).
    let c = centre(&film);
    assert_eq!(c[3], 1.0);
    assert!(c[0] > c[1] * 3.0 && c[0] > 0.05, "{c:?}");
    assert!(!close(&c[..3], &[0.8, 0.1, 0.1], 1e-3), "{c:?}");
    // Every pixel is opaque without film_transparent.
    assert!(film.rgba.iter().all(|p| (p[3] - 1.0).abs() < 1e-6));
}

#[test]
fn flat_lighting_reproduces_each_colour_source_exactly() {
    let mut scene = cube_scene(64, 48);
    scene.settings.shading_light = "FLAT".into();
    scene.objects[0].color = [0.2, 0.6, 0.3, 1.0];
    scene.objects[0].random_hash = 0x1234_5678;
    scene.settings.shading_single_color = [0.25, 0.5, 0.75];
    let cases: [(&str, [f32; 3]); 4] = [
        ("MATERIAL", [0.8, 0.1, 0.1]),
        ("OBJECT", [0.2, 0.6, 0.3]),
        ("SINGLE", [0.25, 0.5, 0.75]),
        ("RANDOM", color::random_color(0x1234_5678)),
    ];
    for (ct, want) in cases {
        scene.settings.shading_color_type = ct.into();
        let Some(f) = render(&scene) else { return };
        let c = centre(&f);
        assert!(
            close(&c, &[want[0], want[1], want[2], 1.0], 1e-6),
            "{ct}: {c:?} != {want:?}"
        );
    }
    // A slot without a material: BKE_material_default_empty() grey.
    scene.settings.shading_color_type = "MATERIAL".into();
    scene.objects[0].materials = vec![None];
    let Some(f) = render(&scene) else { return };
    assert!(close(&centre(&f), &[0.8, 0.8, 0.8, 1.0], 1e-6));
}

#[test]
fn material_slots_draw_their_own_colours() {
    let mut scene = cube_scene(64, 64);
    scene.settings.shading_light = "FLAT".into();
    scene.materials.insert(
        MatKey(2),
        Arc::new(material([0.1, 0.2, 0.9, 1.0], 0.0, 0.5)),
    );
    // First 3 faces (18 indices) slot 0, the rest slot 1. Faces +Z (top) is index 4 -> slot 1.
    let mut mesh = cube_mesh();
    mesh.submeshes = vec![(0, 0..18), (1, 18..36)];
    scene.meshes.insert(MeshKey(1), Arc::new(mesh));
    scene.objects[0].materials = vec![Some(MatKey(1)), Some(MatKey(2))];
    let Some(f) = render(&scene) else { return };
    let mut seen_red = false;
    let mut seen_blue = false;
    for p in &f.rgba {
        seen_red |= close(p, &[0.8, 0.1, 0.1, 1.0], 1e-6);
        seen_blue |= close(p, &[0.1, 0.2, 0.9, 1.0], 1e-6);
    }
    assert!(seen_red && seen_blue);
}

#[test]
fn background_and_film_transparent() {
    let mut scene = cube_scene(64, 48);
    scene.settings.shading_light = "FLAT".into();
    scene.settings.shading_color_type = "SINGLE".into();
    scene.settings.shading_single_color = [0.25, 0.5, 0.75];
    scene.world = RWorld::Color {
        rgb: [0.3, 0.2, 0.1],
        strength: 4.0,
    };
    let Some(opaque) = render(&scene) else { return };
    assert!(close(&px(&opaque, 0, 0), &[0.3, 0.2, 0.1, 1.0], 1e-6));

    scene.settings.film_transparent = true;
    let Some(f) = render(&scene) else { return };
    assert_eq!(px(&f, 0, 0), [0.0; 4]);
    assert!(close(&centre(&f), &[0.25, 0.5, 0.75, 1.0], 1e-6));
    // Anti-aliased silhouette: partial coverage in straight alpha keeps the surface colour.
    let edges: Vec<_> = f.rgba.iter().filter(|p| p[3] > 0.0 && p[3] < 1.0).collect();
    assert!(!edges.is_empty(), "no partially covered pixel");
    for p in edges {
        assert!(close(&p[..3], &[0.25, 0.5, 0.75], 1e-5), "{p:?}");
        // 8 samples, each fully in or out: coverage is a multiple of 1/8.
        let k = p[3] * 8.0;
        assert!((k - k.round()).abs() < 1e-5, "{p:?}");
    }
}

#[test]
fn rgba32float_accumulation_keeps_full_precision() {
    // 64 samples of a colour that is not representable in f16: an Rgba16Float accumulator
    // drifts by > 1e-4 here, Rgba32Float stays within float rounding.
    let mut scene = cube_scene(32, 32);
    scene.settings.shading_light = "FLAT".into();
    scene.settings.shading_color_type = "SINGLE".into();
    scene.settings.shading_single_color = [0.123_457, 0.987_654, 0.333_333];
    let Some(f) = render_with(64, &scene) else {
        return;
    };
    let f = f.unwrap();
    let c = centre(&f);
    assert!(
        close(&c, &[0.123_457, 0.987_654, 0.333_333, 1.0], 2e-6),
        "{c:?}"
    );
}

#[test]
fn studio_lighting_shades_and_specular_varies() {
    let mut scene = cube_scene(96, 96);
    scene
        .meshes
        .insert(MeshKey(1), Arc::new(sphere_mesh(32, 16)));
    scene.camera = camera(Vec3::new(0.0, -5.0, 0.0), Vec3::ZERO, 96, 96);
    scene.settings.shading_color_type = "SINGLE".into();
    let Some(studio) = render(&scene) else { return };
    scene.settings.shading_light = "FLAT".into();
    let Some(flat) = render(&scene) else { return };
    let lum = |p: [f32; 4]| p[0] + p[1] + p[2];
    // FLAT is uniform over the sphere; STUDIO is not.
    let inside: Vec<(u32, u32)> = (20..76)
        .flat_map(|y| (20..76).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (dx, dy) = (x as f32 - 47.5, y as f32 - 47.5);
            dx * dx + dy * dy < 20.0 * 20.0
        })
        .collect();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &(x, y) in &inside {
        assert!(close(&px(&flat, x, y), &[0.8, 0.8, 0.8, 1.0], 1e-6));
        let l = lum(px(&studio, x, y));
        lo = lo.min(l);
        hi = hi.max(l);
    }
    assert!(hi - lo > 0.2, "studio shading too uniform: {lo}..{hi}");
    // The Default rig's key light comes from the upper left (view space (-0.41, 0.35, 0.84)).
    assert!(lum(px(&studio, 34, 34)) > lum(px(&studio, 62, 62)));
}

#[test]
fn mirrored_object_shades_like_its_unmirrored_twin() {
    // A cube is symmetric under x -> -x, so scale (-1, 1, 1) must render identically: the
    // front-face test follows the object's handedness.
    let scene = cube_scene(64, 64);
    let Some(a) = render(&scene) else { return };
    let mut mirrored = scene.clone();
    mirrored.objects[0].matrix_world = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let Some(b) = render(&mirrored) else { return };
    let max = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .flat_map(|(p, q)| (0..4).map(move |i| (p[i] - q[i]).abs()))
        .fold(0.0f32, f32::max);
    assert!(max < 2e-2, "max diff {max}");
    assert!(ssim(&a, &b) >= 0.99);
}

#[test]
fn odd_width_readback_is_unpadded_and_symmetric() {
    // 67 × 16 B = 1072 B per row, padded to 1280 B for the copy: the film must come back
    // tightly packed. A centred, front-facing symmetric object is left-right symmetric.
    let (w, h) = (67, 33);
    let mut scene = cube_scene(w, h);
    scene.camera = camera(Vec3::new(0.0, -6.0, 0.0), Vec3::ZERO, w, h);
    scene.settings.shading_light = "FLAT".into();
    let Some(f) = render_with(1, &scene) else {
        return;
    };
    let f = f.unwrap();
    assert_eq!(f.rgba.len(), (w * h) as usize);
    for y in 0..h {
        for x in 0..w / 2 {
            assert_eq!(px(&f, x, y)[3], 1.0);
            let (l, r) = (px(&f, x, y), px(&f, w - 1 - x, y));
            assert!(close(&l, &r, 1e-6), "row {y}: {x} {l:?} vs {r:?}");
        }
    }
    assert!(close(&centre(&f), &[0.8, 0.1, 0.1, 1.0], 1e-6));
    assert!(close(&px(&f, 0, h / 2), &[BG[0], BG[1], BG[2], 1.0], 1e-6));
}

#[test]
fn same_adapter_determinism() {
    // Two independent renderers (cold and warm devices) on the same adapter.
    let mut scene = cube_scene(128, 96);
    scene
        .meshes
        .insert(MeshKey(2), Arc::new(sphere_mesh(24, 12)));
    scene.objects.push(object(
        "Sphere",
        2,
        Mat4::from_translation(Vec3::new(1.5, 1.5, 0.5)),
        vec![],
    ));
    let opts = GpuOptions::from_env();
    let mut a = Renderer::new(&opts);
    let first = match a.render(&scene) {
        Ok(f) => f,
        Err(e @ RenderError::NoAdapter(_)) if gpu_optional() => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let second = a.render(&scene).unwrap();
    let mut b = Renderer::new(&opts);
    let third = b.render(&scene).unwrap();
    for other in [&second, &third] {
        let s = ssim(&first, other);
        assert!(s >= 0.995, "SSIM {s}");
    }
    assert_eq!(first, second, "same device, same scene: bit-identical");
    eprintln!(
        "determinism: SSIM(run1, run2 fresh device) = {:.6}, identical = {}",
        ssim(&first, &third),
        first == third
    );
}

#[test]
fn one_uber_shader_and_cached_meshes() {
    let opts = GpuOptions::from_env();
    let mut r = Renderer::new(&opts);
    let mut scene = cube_scene(48, 48);
    for (i, ct) in ["MATERIAL", "OBJECT", "SINGLE", "RANDOM"]
        .iter()
        .enumerate()
    {
        for light in ["STUDIO", "FLAT"] {
            scene.settings.shading_color_type = (*ct).into();
            scene.settings.shading_light = light.into();
            scene.materials.insert(
                MatKey(10 + i as u64),
                Arc::new(material([0.1 * i as f32, 0.5, 0.5, 1.0], 0.5, 0.2)),
            );
            scene.objects[0].materials.push(Some(MatKey(10 + i as u64)));
            match r.render(&scene) {
                Ok(_) => {}
                Err(e @ RenderError::NoAdapter(_)) if gpu_optional() => {
                    eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): {e}");
                    return;
                }
                Err(e) => panic!("{e}"),
            }
        }
    }
    assert_eq!(r.workbench().pipeline_builds(), 1);
    assert_eq!(r.cache().upload_count(), 1, "one mesh, uploaded once");
    assert_eq!(r.cache().mesh_count(), 1);
    // render_tiles: one film per scene.
    let tiles = r
        .render_tiles(&[cube_scene(16, 16), cube_scene(24, 8)])
        .unwrap();
    assert_eq!(
        tiles
            .iter()
            .map(|t| (t.width, t.height))
            .collect::<Vec<_>>(),
        [(16, 16), (24, 8)]
    );
}

#[test]
fn invalid_scenes_are_errors_not_panics() {
    let Some(r) = render_with(1, &cube_scene(0, 10)) else {
        return;
    };
    assert!(matches!(r, Err(RenderError::Unsupported(_))), "{r:?}");
    let mut scene = cube_scene(8, 8);
    scene.objects[0].mesh = Some(MeshKey(99));
    let r = render_with(1, &scene).unwrap();
    assert!(matches!(r, Err(RenderError::Unsupported(_))), "{r:?}");
    // Objects without meshes, empty meshes and zero scale draw nothing.
    let mut scene = cube_scene(8, 8);
    scene.objects[0].mesh = None;
    scene
        .meshes
        .insert(MeshKey(5), Arc::new(Default::default()));
    scene
        .objects
        .push(object("Empty", 5, Mat4::IDENTITY, vec![]));
    scene
        .objects
        .push(object("Flat", 1, Mat4::from_scale(Vec3::ZERO), vec![]));
    let f = render_with(1, &scene).unwrap().unwrap();
    assert!(
        f.rgba
            .iter()
            .all(|p| close(p, &[BG[0], BG[1], BG[2], 1.0], 1e-6))
    );
}

#[test]
fn cold_first_render_time() {
    // Recorded per CI adapter (`cargo test -- --nocapture` in gpu.yml prints it).
    let t0 = Instant::now();
    let mut svc = new_service(&GpuOptions::from_env()).expect("service");
    let scene = cube_scene(256, 256);
    let first = svc.render(&scene);
    let cold = t0.elapsed();
    match first {
        Ok(_) => {}
        Err(e @ RenderError::NoAdapter(_)) if gpu_optional() => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    }
    let t1 = Instant::now();
    svc.render(&scene).unwrap();
    let warm = t1.elapsed();
    let a = svc.adapter_info();
    eprintln!(
        "FRAP_RENDER_TIMING adapter={:?} backend={} cold_first_render_ms={:.1} warm_render_ms={:.1} (256x256, 8 samples)",
        a.name,
        a.backend,
        cold.as_secs_f64() * 1e3,
        warm.as_secs_f64() * 1e3
    );
    if let Ok(path) = std::env::var("FRAP_RENDER_TIMINGS") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("FRAP_RENDER_TIMINGS file");
        writeln!(
            f,
            "{{\"adapter\":{:?},\"backend\":{:?},\"cold_first_render_ms\":{:.1},\"warm_render_ms\":{:.1}}}",
            a.name,
            a.backend,
            cold.as_secs_f64() * 1e3,
            warm.as_secs_f64() * 1e3
        )
        .unwrap();
    }
}
