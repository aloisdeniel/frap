// SPDX-License-Identifier: GPL-3.0-or-later
//! GPU tests of the EEVEE-like engine (WP 5.1) on synthetic render scenes: light units on a
//! diffuse plane (against the values Blender 5.2.4's EEVEE renders, the same scenes as the
//! `tests/golden/cases/render_eevee/core/light_units_*` goldens), shadow placement, the single
//! uber-shader, the `Rgba32Float` film, `film_transparent`, determinism.
//!
//! Without an adapter the tests fail, or print a notice and pass under `FRAP_GPU_OPTIONAL=1`.

use std::sync::Arc;

use frap_material::{Channel, LoweredMaterial, Specular};
use frap_render_scene::{
    Film, GpuOptions, MatKey, MeshKey, RCamera, RLight, RLightEevee, RLightKind, RMesh, RObject,
    RWorld, RenderScene,
};
use glam::{Mat4, Vec3};

use super::*;
use crate::eevee::shadows::{look_to, orthographic};
use crate::engine::Engine;

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(g) => {
            eprintln!("adapter: {:?}", g.summary());
            Some(g)
        }
        Err(e) if std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1") => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter: {e:?}");
            None
        }
        Err(e) => panic!("no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip): {e:?}"),
    }
}

/// A `size`-sided plane in the XY plane (normal +Z), as `primitive_plane_add`.
fn plane(size: f32) -> RMesh {
    let h = size / 2.0;
    RMesh {
        positions: vec![[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]],
        uv_names: vec!["UVMap".into()],
        uv_active_render: Some(0),
        indices: vec![0, 1, 2, 0, 2, 3],
        submeshes: vec![(0, 0..6)],
        ..RMesh::default()
    }
}

/// A unit cube of half size `h` centred at the origin.
fn cube(h: f32) -> RMesh {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut m = RMesh::default();
    for (n, u, v) in faces {
        let base = m.positions.len() as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            m.positions
                .push([0, 1, 2].map(|i| h * (n[i] + su * u[i] + sv * v[i])));
            m.normals.push(n);
        }
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m.submeshes = vec![(0, 0..36)];
    m
}

/// White-ish diffuse material: base 0.8, roughness 1, `Specular IOR Level` 0.
fn diffuse() -> LoweredMaterial {
    let mut m = LoweredMaterial::gltf_default();
    m.base_color.factor = [0.8, 0.8, 0.8, 1.0];
    m.metallic.factor = 0.0;
    m.roughness.factor = 1.0;
    m.ext.specular = Some(Specular {
        factor: Channel {
            factor: 0.0,
            texture: None,
        },
        color: Channel {
            factor: [1.0; 3],
            texture: None,
        },
    });
    m
}

/// Orthographic camera at height 10 looking down, `ortho_scale` 10.
fn top_camera() -> RCamera {
    RCamera {
        view: look_to(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z, Vec3::Y),
        projection: orthographic(-5.0, 5.0, -5.0, 5.0, 0.1, 100.0),
        is_perspective: false,
        clip: [0.1, 100.0],
        pixel_aspect: 1.0,
    }
}

fn object(name: &str, mesh: u64, m: Mat4) -> RObject {
    RObject {
        name: name.into(),
        matrix_world: m,
        mesh: Some(MeshKey(mesh)),
        materials: vec![Some(MatKey(1))],
        color: [1.0; 4],
        random_hash: 0,
        display_type: "TEXTURED".into(),
    }
}

fn light(kind: RLightKind, energy: f32, at: Vec3) -> RLight {
    RLight {
        kind,
        color: [1.0; 3],
        energy,
        radius: 0.25,
        use_shadow: true,
        matrix_world: Mat4::from_translation(at),
        eevee: RLightEevee::default(),
    }
}

/// The light-unit scene of `render_eevee/core/light_units_*`: a 10 m plane, black world,
/// one light 2 m above the centre, 64 x 64 pixels.
fn plane_scene(l: RLight) -> RenderScene {
    let mut sc = RenderScene {
        resolution: [64, 64],
        camera: top_camera(),
        world: RWorld::Color {
            rgb: [0.0; 3],
            strength: 0.0,
        },
        ..RenderScene::default()
    };
    sc.settings.engine = "BLENDER_EEVEE".into();
    sc.meshes.insert(MeshKey(1), Arc::new(plane(10.0)));
    sc.materials.insert(MatKey(1), Arc::new(diffuse()));
    sc.objects.push(object("Plane", 1, Mat4::IDENTITY));
    sc.lights.push(l);
    sc
}

fn render(gpu: &Gpu, eevee: &mut crate::eevee::Eevee, sc: &RenderScene) -> Film {
    let mut cache = GpuCache::default();
    let film = eevee.render(gpu, &mut cache, sc).expect("render");
    assert_eq!(
        film.rgba.len(),
        (sc.resolution[0] * sc.resolution[1]) as usize
    );
    assert!(film.rgba.iter().all(|p| p.iter().all(|c| c.is_finite())));
    film
}

fn px(f: &Film, x: u32, y: u32) -> [f32; 4] {
    f.rgba[(y * f.width + x) as usize]
}

/// Mean of the red channel over a `r`-pixel box around `(x, y)`.
fn mean(f: &Film, x: u32, y: u32, r: u32) -> f32 {
    let mut s = 0.0;
    let mut n = 0.0;
    for yy in y - r..=y + r {
        for xx in x - r..=x + r {
            s += px(f, xx, yy)[0];
            n += 1.0;
        }
    }
    s / n
}

/// Blender 5.2.4 EEVEE, same scenes (64 x 64, Standard view, linear EXR): the red channel at
/// pixel (32, 32), (48, 32) and (63, 32) (`/private/tmp` probe, see the light-unit goldens).
const BLENDER_POINT: [f32; 3] = [0.500_977, 0.113_953, 0.022_995];
const BLENDER_SUN: [f32; 3] = [0.764_160, 0.763_672, 0.755_371];
const BLENDER_AREA: [f32; 3] = [1.846_68, 0.287_109, 0.035_431];

fn check(label: &str, f: &Film, want: [f32; 3]) {
    let got = [px(f, 32, 32)[0], px(f, 48, 32)[0], px(f, 63, 32)[0]];
    eprintln!("{label}: frap {got:?} Blender {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g - w).abs() <= 0.05 * w + 1e-3,
            "{label}: {got:?} vs Blender {want:?}"
        );
    }
}

#[test]
fn light_units_match_blender() {
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let up = Vec3::new(0.0, 0.0, 2.0);
    let f = render(
        &gpu,
        &mut e,
        &plane_scene(light(RLightKind::Point, 100.0, up)),
    );
    check("POINT", &f, BLENDER_POINT);
    let mut sun = light(RLightKind::Sun { angle: 0.00918043 }, 3.0, up);
    sun.radius = 0.25;
    let f = render(&gpu, &mut e, &plane_scene(sun));
    check("SUN", &f, BLENDER_SUN);
    let spot = light(
        RLightKind::Spot {
            size: 90f32.to_radians(),
            blend: 0.15,
        },
        100.0,
        up,
    );
    let f = render(&gpu, &mut e, &plane_scene(spot));
    check("SPOT", &f, [BLENDER_POINT[0], 0.0, 0.0]);
    let area = light(
        RLightKind::Area {
            shape: "SQUARE".into(),
            size: [1.0, 1.0],
        },
        100.0,
        up,
    );
    let f = render(&gpu, &mut e, &plane_scene(area));
    check("AREA", &f, BLENDER_AREA);
    // One uber-shader: the pipelines were built once for the four renders.
    assert_eq!(e.state.pipeline_builds(), 1);
}

#[test]
fn shadow_placement() {
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    // A 1 m cube (z 1..2) above the 10 m plane, seen from the top (25.6 pixels per metre).
    // `shadowed`/`lit`: plane points (x, y) the shadow must cover / must not cover.
    type Case = (
        &'static str,
        RLightKind,
        Mat4,
        &'static [(f32, f32)],
        &'static [(f32, f32)],
    );
    let down = Mat4::IDENTITY;
    let tilt = Mat4::from_rotation_y(45f32.to_radians());
    let at3 = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0));
    let cases: [Case; 5] = [
        // Sun straight down: the shadow hides under the cube; beside it everything is lit.
        (
            "sun",
            RLightKind::Sun { angle: 0.0 },
            down,
            &[],
            &[(0.7, 0.0), (0.0, -0.7)],
        ),
        // Sun 45° towards +X: the cube (x -0.5..0.5, z 1..2) shadows x in [-2.5, -0.5].
        (
            "sun 45",
            RLightKind::Sun { angle: 0.0 },
            tilt,
            &[(-1.5, 0.0), (-1.0, 0.3), (-2.2, -0.3)],
            &[(-2.8, 0.0), (0.8, 0.0), (-1.5, 0.8)],
        ),
        // Point light 3 m up: the cube's shadow spans 0.75 .. 1.5 m from the centre.
        (
            "point",
            RLightKind::Point,
            at3,
            &[(1.0, 0.0), (0.0, -1.0), (1.0, 1.0)],
            &[(1.9, 0.0), (0.0, 2.0)],
        ),
        (
            "spot",
            RLightKind::Spot {
                size: 120f32.to_radians(),
                blend: 0.0,
            },
            at3,
            &[(1.0, 0.0), (-1.0, 0.0)],
            &[(1.9, 0.0), (0.0, -2.0)],
        ),
        (
            "area",
            RLightKind::Area {
                shape: "SQUARE".into(),
                size: [0.01, 0.01],
            },
            at3,
            &[(1.0, 0.0), (0.0, 1.0)],
            &[(1.9, 0.0), (-2.0, 0.0)],
        ),
    ];
    for (label, kind, m, shadowed, lit) in cases {
        let mut l = light(kind, 300.0, Vec3::ZERO);
        if matches!(l.kind, RLightKind::Sun { .. }) {
            l.energy = 3.0;
        }
        l.radius = 0.0;
        l.matrix_world = m;
        let mut sc = plane_scene(l);
        sc.resolution = [256, 256];
        sc.meshes.insert(MeshKey(2), Arc::new(cube(0.5)));
        sc.objects.push(object(
            "Cube",
            2,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 1.5)),
        ));
        sc.settings.taa_render_samples = 4;
        let f = render(&gpu, &mut e, &sc);
        let at = |x: f32, y: f32| px(&f, (128.0 + x * 25.6) as u32, (128.0 - y * 25.6) as u32)[0];
        // Unshadowed reference: the same scene without the cube.
        let mut open = sc.clone();
        open.objects.pop();
        let g = render(&gpu, &mut e, &open);
        let free = |x: f32, y: f32| px(&g, (128.0 + x * 25.6) as u32, (128.0 - y * 25.6) as u32)[0];
        for &(x, y) in shadowed {
            assert!(
                at(x, y) < 0.05 * free(x, y),
                "{label}: ({x}, {y}) should be in shadow: {} (unshadowed {})",
                at(x, y),
                free(x, y)
            );
        }
        for &(x, y) in lit {
            assert!(
                (at(x, y) - free(x, y)).abs() < 0.02 * free(x, y) + 1e-4,
                "{label}: ({x}, {y}) should be lit: {} (unshadowed {})",
                at(x, y),
                free(x, y)
            );
        }
    }
}

#[test]
fn shadows_off_and_soft_shadows() {
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut l = light(RLightKind::Sun { angle: 0.0 }, 3.0, Vec3::ZERO);
    l.matrix_world = Mat4::from_rotation_y(45f32.to_radians());
    let mut sc = plane_scene(l);
    sc.meshes.insert(MeshKey(2), Arc::new(cube(0.5)));
    sc.objects.push(object(
        "Cube",
        2,
        Mat4::from_translation(Vec3::new(0.0, 0.0, 1.5)),
    ));
    sc.settings.taa_render_samples = 16;
    let hard = render(&gpu, &mut e, &sc);
    let at = |f: &Film, x: f32| px(f, (32.0 + x * 6.4) as u32, 32)[0];
    assert!(at(&hard, -1.5) < 0.05);
    // `use_shadows = False`: no shadow at all.
    sc.settings.use_shadows = false;
    let off = render(&gpu, &mut e, &sc);
    assert!((at(&off, -1.5) - at(&off, 3.5)).abs() < 0.02);
    // A wide sun (angle 30°): the jittered shadow origins soften the edge — the shadow edge
    // pixel takes intermediate values.
    sc.settings.use_shadows = true;
    sc.lights[0].kind = RLightKind::Sun {
        angle: 30f32.to_radians(),
    };
    sc.settings.taa_render_samples = 32;
    let soft = render(&gpu, &mut e, &sc);
    let edge: Vec<f32> = (0..20).map(|i| at(&soft, -2.3 + i as f32 * 0.1)).collect();
    let lit = at(&soft, 3.5);
    assert!(
        edge.iter().any(|v| *v > 0.15 * lit && *v < 0.85 * lit),
        "no penumbra: {edge:?} (lit {lit})"
    );
}

/// `n` point lights (hard shadows) on a ring of radius 3 at height 3 around a 1 m cube on the
/// plane: each light casts the cube's shadow outwards, on the far side of the ring.
fn ring_scene(n: usize) -> RenderScene {
    let mut sc = plane_scene(light(RLightKind::Point, 100.0, Vec3::ZERO));
    sc.lights.clear();
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let mut l = light(
            RLightKind::Point,
            100.0,
            Vec3::new(3.0 * a.cos(), 3.0 * a.sin(), 3.0),
        );
        l.radius = 0.0;
        sc.lights.push(l);
    }
    sc.meshes.insert(MeshKey(2), Arc::new(cube(0.5)));
    sc.objects.push(object(
        "Cube",
        2,
        Mat4::from_translation(Vec3::new(0.0, 0.0, 0.5)),
    ));
    sc.settings.taa_render_samples = 1;
    sc
}

/// Sum of the red channel of a film.
fn total(f: &Film) -> f64 {
    f.rgba.iter().map(|p| f64::from(p[0])).sum()
}

/// Every shadowed light casts its shadow, however many there are (no per-render light limit):
/// turning one light's `use_shadow` off brightens the plane by that light's shadow, the same
/// amount for every light of the symmetric ring.
fn check_every_shadow(gpu: &Gpu, n: usize, probe: &[usize]) {
    let mut e = crate::eevee::Eevee::new();
    let sc = ring_scene(n);
    let all = total(&render(gpu, &mut e, &sc));
    let mut unshadowed = sc.clone();
    for l in &mut unshadowed.lights {
        l.use_shadow = false;
    }
    let none = total(&render(gpu, &mut e, &unshadowed));
    // Each light's share of the shadowed light, if all were present.
    let share = (none - all) / n as f64;
    assert!(
        share > 0.0,
        "{n} lights: no shadow at all ({all} vs {none})"
    );
    for &i in probe {
        let mut one = sc.clone();
        one.lights[i].use_shadow = false;
        let gain = total(&render(gpu, &mut e, &one)) - all;
        eprintln!("{n} lights: light {i} shadow {gain:.3} (mean {share:.3})");
        assert!(
            (gain - share).abs() < 0.25 * share,
            "{n} lights: light {i}'s shadow is missing or wrong: {gain} vs {share}"
        );
    }
}

#[test]
fn every_light_casts_its_shadow() {
    let Some(gpu) = gpu() else { return };
    // 10 point lights: 60 cube faces, past the full-size texel budget (smaller maps).
    check_every_shadow(&gpu, 10, &(0..10).collect::<Vec<_>>());
    // 50 point lights: 300 maps, more than the 256 array layers wgpu guarantees (tiled layers).
    let sc = ring_scene(50);
    let records: Vec<lights::LightRecord> = sc
        .lights
        .iter()
        .map(|l| lights::sync(l, sc.settings.eevee.light_threshold))
        .collect();
    let limits = gpu.device.limits();
    let plan = shadows::plan(
        &records,
        true,
        1024,
        limits.max_texture_array_layers,
        limits.max_texture_dimension_2d,
    );
    eprintln!(
        "50 lights: {} maps of {} in {} layers of {} x {} tiles",
        plan.maps, plan.size, plan.layers, plan.tiles, plan.tiles
    );
    assert_eq!(plan.maps, 300);
    if limits.max_texture_array_layers < 300 {
        assert!(plan.tiles > 1);
    }
    check_every_shadow(&gpu, 50, &[0, 7, 13, 24, 31, 42, 45, 49]);
}

#[test]
fn film_is_rgba32_and_transparent() {
    assert_eq!(film::ACCUM_FORMAT, wgpu::TextureFormat::Rgba32Float);
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut sc = plane_scene(light(RLightKind::Point, 100.0, Vec3::new(0.0, 0.0, 2.0)));
    // Shrink the plane so the background shows.
    sc.objects[0].matrix_world = Mat4::from_scale(Vec3::splat(0.5));
    sc.world = RWorld::Color {
        rgb: [0.2, 0.3, 0.4],
        strength: 2.0,
    };
    let f = render(&gpu, &mut e, &sc);
    let bg = px(&f, 1, 1);
    assert!(
        (bg[0] - 0.4).abs() < 1e-3 && (bg[2] - 0.8).abs() < 1e-3 && bg[3] == 1.0,
        "{bg:?}"
    );
    sc.settings.film_transparent = true;
    let f = render(&gpu, &mut e, &sc);
    assert_eq!(px(&f, 1, 1)[3], 0.0);
    assert!((px(&f, 32, 32)[3] - 1.0).abs() < 1e-5);
    // Anti-aliased plane border: the 5 m plane spans pixels 16..48 of 64 (borders on pixel
    // edges); moved a third of a pixel, its border cuts pixel 15, whose coverage is partial.
    // Lit by the uniform world only, the plane has one colour, which the straight-alpha border
    // pixel keeps.
    sc.lights.clear();
    sc.objects[0].matrix_world = Mat4::from_translation(Vec3::new(-10.0 / 64.0 / 3.0, 0.0, 0.0))
        * Mat4::from_scale(Vec3::splat(0.5));
    let f = render(&gpu, &mut e, &sc);
    let edge = px(&f, 15, 32);
    let inner = px(&f, 24, 32);
    assert!((inner[3] - 1.0).abs() < 1e-4, "{inner:?}");
    assert!(
        edge[3] > 0.05 && edge[3] < 0.95,
        "partial coverage: {edge:?}"
    );
    for c in 0..3 {
        assert!(
            (edge[c] - inner[c]).abs() < 0.01 * inner[c] + 1e-4,
            "straight colour at the border: {edge:?} vs {inner:?}"
        );
    }
}

#[test]
fn world_lights_the_plane() {
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut sc = plane_scene(light(RLightKind::Point, 0.0, Vec3::new(0.0, 0.0, 2.0)));
    sc.lights.clear();
    sc.world = RWorld::Color {
        rgb: [0.5, 0.5, 0.5],
        strength: 1.0,
    };
    let f = render(&gpu, &mut e, &sc);
    // A uniform world of radiance L lights a Lambertian surface to albedo × L.
    let c = mean(&f, 32, 32, 3);
    assert!((c - 0.4).abs() < 0.01, "{c}");
}

#[test]
fn deterministic_and_cached() {
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut sc = plane_scene(light(RLightKind::Point, 100.0, Vec3::new(0.5, 0.0, 2.0)));
    sc.meshes.insert(MeshKey(2), Arc::new(cube(0.5)));
    sc.objects.push(object(
        "Cube",
        2,
        Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0)),
    ));
    sc.settings.taa_render_samples = 8;
    let a = render(&gpu, &mut e, &sc);
    let b = render(&gpu, &mut e, &sc);
    assert_eq!(a, b);
    assert_eq!(e.state.pipeline_builds(), 1);
}

/// Cold start (new device, pipelines, first render) of the factory-like scene at the M5
/// golden size, within the CI budget (`FRAP_EEVEE_COLD_BUDGET_S`; default 60 s on a hardware
/// adapter and 180 s on a software one, wgpu's `DeviceType::Cpu`: WARP's FXC compile of the
/// uber-shader plus 64 samples rasterised on the CPU take 85-105 s on the Windows runner;
/// decision log M56CI-3). The adapter, cold and warm times are printed and appended to
/// `FRAP_RENDER_TIMINGS` (gpu.yml's per-adapter timings artifact), as Workbench's
/// `cold_first_render_time` does.
#[test]
fn cold_start_within_budget() {
    let start = std::time::Instant::now();
    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut sc = plane_scene(light(RLightKind::Point, 1000.0, Vec3::new(4.0, 1.0, 6.0)));
    sc.resolution = [320, 180];
    sc.meshes.insert(MeshKey(2), Arc::new(cube(1.0)));
    sc.objects.push(object(
        "Cube",
        2,
        Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0)),
    ));
    sc.settings.taa_render_samples = 64;
    let _ = render(&gpu, &mut e, &sc);
    let cold = start.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();
    let _ = render(&gpu, &mut e, &sc);
    let warm = t1.elapsed().as_secs_f64();
    let a = gpu.summary();
    let software = gpu.info.device_type == wgpu::DeviceType::Cpu;
    let budget: f64 = std::env::var("FRAP_EEVEE_COLD_BUDGET_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if software { 180.0 } else { 60.0 });
    eprintln!(
        "FRAP_RENDER_TIMING engine=BLENDER_EEVEE adapter={:?} backend={} cold_first_render_ms={:.1} warm_render_ms={:.1} (device + pipelines + 64 samples, 320x180)",
        a.name,
        a.backend,
        cold * 1e3,
        warm * 1e3
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
            "{{\"engine\":\"BLENDER_EEVEE\",\"adapter\":{:?},\"backend\":{:?},\"cold_first_render_ms\":{:.1},\"warm_render_ms\":{:.1}}}",
            a.name,
            a.backend,
            cold * 1e3,
            warm * 1e3
        )
        .expect("FRAP_RENDER_TIMINGS write");
    }
    assert!(
        cold < budget,
        "cold start {cold:.1} s > budget {budget} s ({} adapter)",
        a.device_type
    );
}

// A test-only shading feature: proves the WP 5.5 extension point (flag bit, `pack`, WGSL
// snippet called by the generated dispatcher) end to end.
fn test_tint_applies(m: &LoweredMaterial) -> bool {
    m.ext.dispersion == Some(-7.0)
}

fn test_tint_pack(_m: &LoweredMaterial, _s: &RenderScene, p: &mut MaterialParams) {
    p.extra[0] = [0.0, 1.0, 0.5, 0.0];
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static TEST_TINT: crate::eevee::EeveeFeature = crate::eevee::EeveeFeature {
    name: "test_tint",
    order: u16::MAX,
    wgsl: "fn feature_test_tint(s: ptr<function, Surface>, v: ptr<function, Inputs>) {\n    \
           (*s).base_color = (*s).base_color * material.extra[0].rgb;\n}\n",
    applies: test_tint_applies,
    pack: test_tint_pack,
};

#[test]
fn shading_features_extension_point() {
    let src = forward_source();
    assert!(src.contains("fn feature_test_tint"));
    assert!(src.contains("fn eevee_features_apply"));
    let bit = crate::eevee::features()
        .iter()
        .position(|f| f.name == "test_tint")
        .expect("registered");
    assert!(src.contains(&format!(
        "& {}u) != 0u) {{ feature_test_tint(s, v); }}",
        1u32 << bit
    )));
    let mut m = diffuse();
    let sc = plane_scene(light(RLightKind::Point, 100.0, Vec3::new(0.0, 0.0, 2.0)));
    assert_eq!(MaterialParams::for_material(&m, &sc).flags, 0);
    m.ext.dispersion = Some(-7.0);
    let p = MaterialParams::for_material(&m, &sc);
    assert_eq!(p.flags, 1 << bit);
    assert_eq!(p.gpu().extra[0], [0.0, 1.0, 0.5, 0.0]);

    let Some(gpu) = gpu() else { return };
    let mut e = crate::eevee::Eevee::new();
    let mut sc = sc;
    let plain = render(&gpu, &mut e, &sc);
    sc.materials.insert(MatKey(1), Arc::new(m));
    let tinted = render(&gpu, &mut e, &sc);
    let (a, b) = (px(&plain, 32, 32), px(&tinted, 32, 32));
    assert!(b[0] < 1e-4, "{b:?}");
    assert!(
        (b[1] - a[1]).abs() < 1e-4 && (b[2] - 0.5 * a[2]).abs() < 1e-3,
        "{a:?} {b:?}"
    );
}

#[test]
fn material_params_from_lowered() {
    let sc = plane_scene(light(RLightKind::Point, 100.0, Vec3::new(0.0, 0.0, 2.0)));
    // Default surface: Principled defaults with base colour 0.8.
    let d = MaterialParams::default();
    assert_eq!(d.base_color, [0.8, 0.8, 0.8, 1.0]);
    assert_eq!((d.roughness, d.ior, d.specular_ior_level), (0.5, 1.5, 0.5));
    let mut m = diffuse();
    let p = MaterialParams::core(&m, &sc);
    assert_eq!(p.specular_ior_level, 0.0);
    assert_eq!(p.blend, BlendMode::Opaque);
    m.alpha_mode = frap_material::AlphaMode::Mask { cutoff: 0.3 };
    let p = MaterialParams::core(&m, &sc);
    assert_eq!((p.blend, p.alpha_cutoff), (BlendMode::Clip, 0.3));
    m.alpha_mode = frap_material::AlphaMode::Blend;
    assert_eq!(MaterialParams::core(&m, &sc).blend, BlendMode::Dithered);
    // Texture transforms: identity, and the glTF rotation of the Blender add-on.
    assert_eq!(uv_matrix(None), [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
    let r = uv_matrix(Some(frap_material::TexTransform {
        offset: [0.1, 0.2],
        rotation: std::f32::consts::FRAC_PI_2,
        scale: [2.0, 3.0],
    }));
    // (1, 0) → (offset.x + sx·cos, offset.y − sx·sin)
    assert!((r[0][0] * 1.0 + r[0][2] - 0.1).abs() < 1e-6);
    assert!((r[1][0] * 1.0 + r[1][2] - (0.2 - 2.0)).abs() < 1e-6);
    // GPU layout: 16-byte rows, as `Material` in WGSL.
    assert_eq!(std::mem::size_of::<GpuMaterial>() % 16, 0);
    assert_eq!(
        std::mem::size_of::<GpuMaterial>(),
        8 * 16 + 4 * 16 + 4 * 16 + 16 + 8 * 32
    );
}

#[test]
fn image_decoding() {
    let img = frap_imbuf::DecodedImage {
        width: 2,
        height: 1,
        channels: 3,
        pixels: frap_imbuf::PixelData::U8(vec![255, 128, 0, 0, 0, 255]),
    };
    let lin = image_linear(&img, false);
    assert_eq!(lin[0][0], 1.0);
    assert!((lin[0][1] - 0.2158605).abs() < 1e-4, "{:?}", lin[0]);
    assert_eq!(lin[1][3], 1.0);
    let data = image_linear(&img, true);
    assert!((data[0][1] - 128.0 / 255.0).abs() < 1e-6);
    let (d, w, h) = downsample(&[[1.0; 4], [0.0; 4], [1.0; 4], [0.0; 4]], 2, 2);
    assert_eq!((w, h), (1, 1));
    assert_eq!(d[0], [0.5; 4]);
}

// DIAG (temporary): cube-face depths of a hard point light above the plane against the
// analytic depth of the plane (z = 0), with the interpolated depth (old) and the plane depth.
fn diag_depth_layer(gpu: &Gpu, tex: &wgpu::Texture, size: u32, layer: u32) -> Vec<f32> {
    let bpr = (size * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(bpr * size),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(size),
            },
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = slice.get_mapped_range().unwrap();
    let mut out = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        let row = &data[(y * bpr) as usize..(y * bpr + size * 4) as usize];
        out.extend(
            row.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
        );
    }
    out
}

fn diag_analytic(m: &Mat4, size: u32, i: u32, j: u32) -> Option<f64> {
    let inv = m.as_dmat4().inverse();
    let u = (f64::from(i) + 0.5) / f64::from(size);
    let v = (f64::from(j) + 0.5) / f64::from(size);
    let ndc = glam::DVec2::new(u * 2.0 - 1.0, 1.0 - v * 2.0);
    let a = inv * glam::DVec4::new(ndc.x, ndc.y, 1.0, 1.0);
    let b = inv * glam::DVec4::new(ndc.x, ndc.y, 0.0, 1.0);
    let a = a.truncate() / a.w;
    let b = b.truncate() / b.w;
    let dir = b - a;
    if dir.z.abs() < 1e-12 {
        return None;
    }
    let t = -a.z / dir.z;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let x = a + dir * t;
    if x.x.abs() > 4.9 || x.y.abs() > 4.9 {
        return None;
    }
    let c = m.as_dmat4() * x.extend(1.0);
    Some(c.z / c.w)
}

#[test]
fn diag_plane_depth() {
    if std::env::var("FRAP_DIAG").is_err() {
        return;
    }
    let Some(gpu) = gpu() else { return };
    for at in [
        Vec3::new(0.0, 0.0, 1.5),
        Vec3::new(0.13, -0.07, 1.5),
        Vec3::new(-0.21, 0.17, 1.5),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(0.31, 0.22, 2.0),
    ] {
        for old in [true, false] {
            DIAG_OLD_DEPTH.with(|c| c.set(old));
            let mut e = crate::eevee::Eevee::new();
            let mut l = light(RLightKind::Point, 100.0, at);
            l.radius = 0.0;
            let mut sc = plane_scene(l);
            sc.settings.taa_render_samples = 1;
            let lit = render(&gpu, &mut e, &sc);
            let (mats, size, tiles) = DIAG_MATS.with(|d| d.borrow().clone());
            let (lsize, layers, tex) = e.state.shadow_atlas.as_ref().unwrap();
            eprintln!(
                "DIAG at={at:?} old={old} map size {size} tiles {tiles} layer {lsize} x {layers}; probes {:.4} {:.4} {:.4} {:.4}",
                px(&lit, 32, 32)[0],
                px(&lit, 48, 32)[0],
                px(&lit, 16, 32)[0],
                px(&lit, 32, 48)[0]
            );
            let per = tiles * tiles;
            let layer_data: Vec<Vec<f32>> = (0..*layers)
                .map(|l| diag_depth_layer(&gpu, tex, *lsize, l))
                .collect();
            for (f, m) in mats.iter().enumerate().take(6) {
                let f = f as u32;
                let (layer, t) = (f / per, f % per);
                let (ox, oy) = (t % tiles * size, t / tiles * size);
                let d = &layer_data[layer as usize];
                let (mut n, mut lo, mut hi, mut closer, mut far3, mut worst_rel) =
                    (0u32, f64::MAX, f64::MIN, 0u32, 0u32, 0.0f64);
                for j in 1..size - 1 {
                    for i in 1..size - 1 {
                        let Some(want) = diag_analytic(m, size, i, j) else {
                            continue;
                        };
                        let (Some(r), Some(dn)) = (
                            diag_analytic(m, size, i + 1, j),
                            diag_analytic(m, size, i, j + 1),
                        ) else {
                            continue;
                        };
                        let slope = (r - want).abs().max((dn - want).abs());
                        let got = f64::from(d[((oy + j) * lsize + ox + i) as usize]);
                        let diff = got - want;
                        n += 1;
                        lo = lo.min(diff);
                        hi = hi.max(diff);
                        if diff > 0.25 * slope + 1e-9 {
                            closer += 1;
                        }
                        if diff < -3.0 * slope - 1e-9 {
                            far3 += 1;
                        }
                        worst_rel = worst_rel.max(diff.abs() / want.max(1e-12));
                    }
                }
                if n > 0 {
                    eprintln!(
                        "  face {f}: {n} texels, depth - analytic in [{:.3e}, {:.3e}], {closer} closer than -0.25 slope.. (acne), {far3} beyond 3 slopes, worst |rel| {worst_rel:.3e}",
                        lo, hi
                    );
                }
            }
        }
    }
    DIAG_OLD_DEPTH.with(|c| c.set(false));
}
