// SPDX-License-Identifier: GPL-3.0-or-later
//! GPU tests of the world lighting (WP 5.2) and its Blender goldens: every case of
//! `tests/golden/cases/render_eevee/world/` is parsed into a preview scene ([`super::preview`]),
//! lit by [`super::world_lighting`] of its HDRI and compared with Blender 5.2.4's EEVEE render
//! (`tests/golden/fixtures/render_eevee/world/<case>.blender.png`): SSIM ≥ the case's
//! `min_ssim` (0.90, §20.4 "EEVEE vs Blender").
//!
//! GPU policy (§22.8 platform rules): without an adapter the tests fail, or print a SKIP notice
//! and pass under `FRAP_GPU_OPTIONAL=1`. `FRAP_IBL_KEEP=<dir>` writes every preview as PNG.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frap_imbuf::{DecodedImage, PixelData};
use frap_render_scene::{GpuOptions, ImageKey, RWorld, RenderScene};

use super::bake::read_rgba16f;
use super::equirect::{self, LinearImage};
use super::preview::{self, Material, Shape};
use super::sh::ShL1;
use super::{WorldProbeSettings, build, octahedral, world_lighting};
use crate::gpu::Gpu;

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(g) => Some(g),
        Err(e) if std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1") => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter: {e:?}");
            None
        }
        Err(e) => panic!("no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip): {e:?}"),
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Direction of the centre of equirect pixel `(x, y)` (top row first), inverse of
/// `equirect::uv` without rotation.
fn pixel_direction(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let u = (x as f32 + 0.5) / w as f32;
    let v = 1.0 - (y as f32 + 0.5) / h as f32;
    let phi = (0.5 - u) * std::f32::consts::TAU;
    let theta = (v - 0.5) * std::f32::consts::PI;
    [
        theta.cos() * phi.cos(),
        theta.cos() * phi.sin(),
        theta.sin(),
    ]
}

fn image_from(w: u32, h: u32, f: impl Fn([f32; 3]) -> [f32; 3]) -> LinearImage {
    let mut rgb = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            rgb.push(f(pixel_direction(x, y, w, h)));
        }
    }
    LinearImage {
        width: w,
        height: h,
        rgb,
    }
}

/// Brute-force L1 projection of an equirect image (solid angle per pixel).
fn cpu_sh(img: &LinearImage, threshold: f32) -> ShL1 {
    let mut sh = ShL1::default();
    let (w, h) = (img.width, img.height);
    for y in 0..h {
        let theta0 = (0.5 - y as f64 / h as f64) * std::f64::consts::PI;
        let theta1 = (0.5 - (y + 1) as f64 / h as f64) * std::f64::consts::PI;
        let omega = ((theta0.sin() - theta1.sin()) * std::f64::consts::TAU / w as f64) as f32;
        for x in 0..w {
            let c = img.rgb[(y * w + x) as usize];
            let m = c[0].abs().max(c[1].abs()).max(c[2].abs());
            let k = (threshold / m.max(1e-8)).min(1.0);
            sh.encode_signal_sample(pixel_direction(x, y, w, h), c.map(|v| v * k * omega));
        }
    }
    sh
}

fn close3(a: [f32; 3], b: [f32; 3], rel: f32) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (x - y).abs() <= rel * y.abs().max(1e-3))
}

#[test]
fn constant_world() {
    let Some(gpu) = gpu() else { return };
    let img = image_from(64, 32, |_| [0.5, 1.0, 2.0]);
    let s = WorldProbeSettings {
        probe_extent: 256,
        ..Default::default()
    };
    let wl = build(&gpu, &img, 0.3, &s).unwrap();
    assert!(wl.sun.is_none(), "nothing passes the threshold");
    assert_eq!(wl.specular_mips, 5);
    for n in [[0.0, 0.0, 1.0], [0.6, -0.8, 0.0], [0.0, 0.0, -1.0]] {
        let e = wl.sh().evaluate_lambert(n);
        assert!(close3(e, [0.5, 1.0, 2.0], 5e-3), "{n:?}: {e:?}");
    }
    assert!(wl.sh9[4..].iter().all(|c| *c == [0.0; 4]));
    // Every probe mip and every cube mip hold the constant.
    for mip in 0..5 {
        for layer in read_rgba16f(&gpu, &wl.octahedral, mip).unwrap() {
            for t in layer {
                assert!(
                    close3([t[0], t[1], t[2]], [0.5, 1.0, 2.0], 2e-3),
                    "oct {mip}: {t:?}"
                );
            }
        }
    }
    let spec = wl.specular.texture().clone();
    for mip in 0..5 {
        for layer in read_rgba16f(&gpu, &spec, mip).unwrap() {
            for t in layer {
                assert!(
                    close3([t[0], t[1], t[2]], [0.5, 1.0, 2.0], 2e-3),
                    "cube {mip}: {t:?}"
                );
            }
        }
    }
    assert_eq!(spec.size().width, 128, "cube face = probe extent / 2");
}

#[test]
fn cube_faces_and_rotation_follow_blender_mapping() {
    let Some(gpu) = gpu() else { return };
    // Colour = positive direction components: +X red, +Y green, +Z blue.
    let img = image_from(512, 256, |d| d.map(|c| c.max(0.0)));
    let s = WorldProbeSettings {
        probe_extent: 256,
        ..Default::default()
    };
    // Face centres (+X, -X, +Y, -Y, +Z, -Z), rotation 0 then 90°: the lookup is `Rz(rot) · d`,
    // so the +X face sees the image's +Y.
    let expect = |rot: bool| -> [[f32; 3]; 6] {
        if rot {
            [
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0; 3],
            ]
        } else {
            [
                [1.0, 0.0, 0.0],
                [0.0; 3],
                [0.0, 1.0, 0.0],
                [0.0; 3],
                [0.0, 0.0, 1.0],
                [0.0; 3],
            ]
        }
    };
    for rot in [false, true] {
        let r = if rot {
            std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };
        let wl = build(&gpu, &img, r, &s).unwrap();
        let spec = wl.specular.texture().clone();
        let faces = read_rgba16f(&gpu, &spec, 0).unwrap();
        let size = spec.size().width as usize;
        for (f, want) in expect(rot).iter().enumerate() {
            // Average the 2 x 2 centre texels (the exact axis hits the `x == 0 || y == 0`
            // guard of Blender's mapping).
            let mut c = [0.0f32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let t = faces[f][(size / 2 - 1 + dy) * size + size / 2 - 1 + dx];
                for i in 0..3 {
                    c[i] += t[i] / 4.0;
                }
            }
            for i in 0..3 {
                assert!(
                    (c[i] - want[i]).abs() < 0.03,
                    "rot {rot} face {f}: {c:?} vs {want:?}"
                );
            }
        }
        // Background: the image as uploaded, rotation stored for the lookup.
        assert_eq!(wl.rotation, r);
        assert_eq!(wl.background.texture().size().width, 512);
    }
}

#[test]
fn harmonics_match_a_cpu_projection_and_are_deringed() {
    let Some(gpu) = gpu() else { return };
    let img = image_from(256, 128, |d| {
        let sky = (d[2] * 0.5 + 0.5).max(0.0);
        [
            0.2 + 3.0 * sky * sky,
            0.3 + 2.0 * sky,
            0.5 + d[0].max(0.0) * 4.0,
        ]
    });
    let s = WorldProbeSettings {
        probe_extent: 512,
        ..Default::default()
    };
    let wl = build(&gpu, &img, 0.0, &s).unwrap();
    let want = cpu_sh(&img, 1e20).dering();
    let got = wl.sh();
    for (g, w) in got.coefs().iter().zip(want.coefs()) {
        for i in 0..3 {
            assert!(
                (g[i] - w[i]).abs() <= 0.01 * want.l0[i].abs(),
                "{:?} vs {:?}",
                got,
                want
            );
        }
    }
}

#[test]
fn sun_extraction() {
    let Some(gpu) = gpu() else { return };
    // A 0.2 sky with a 2.5°-radius sun of radiance 2000 towards `sun`.
    let sun = glam::Vec3::new(0.3, -0.5, 0.6).normalize();
    let cos_r = (2.5f32).to_radians().cos();
    let img = image_from(1024, 512, |d| {
        if glam::Vec3::from(d).dot(sun) > cos_r {
            [2000.0, 1800.0, 1500.0]
        } else {
            [0.2, 0.2, 0.2]
        }
    });
    let s = WorldProbeSettings::default();
    let wl = build(&gpu, &img, 0.0, &s).unwrap();
    let l = wl.sun.as_ref().expect("a sun passes the threshold");
    let dir = l.matrix_world.z_axis.truncate();
    assert!(dir.dot(sun) > (0.5f32).to_radians().cos(), "{dir} vs {sun}");
    // Irradiance: (L - clamp(L)) over the disk; clamp keeps a radiance of max 10.
    let omega = std::f32::consts::TAU * (1.0 - cos_r);
    let k = 10.0 / 2000.0;
    let want = [2000.0, 1800.0, 1500.0].map(|c: f32| c * (1.0 - k) * omega);
    let got = l.color.map(|c| c * l.energy);
    assert!(close3(got, want, 0.03), "{got:?} vs {want:?}");
    assert!(l.use_shadow);
    // The probe keeps only the clamped radiance: the diffuse world is ~0.2 plus the clamped disk.
    let e = wl.sh().evaluate_lambert([0.0, 0.0, -1.0]);
    assert!(e[0] < 0.3, "{e:?}");
    // No extraction when the threshold is off: the sun stays in the harmonics.
    let off = build(
        &gpu,
        &img,
        0.0,
        &WorldProbeSettings {
            sun_threshold: 0.0,
            ..WorldProbeSettings::default()
        },
    )
    .unwrap();
    assert!(off.sun.is_none());
    // E ≈ 11.9: the deringed L1 reconstruction towards the sun gains ≈ 0.08 E + 0.53 · 0.16 E.
    let (with, without) = (
        wl.sh().evaluate_lambert(sun.into())[0],
        off.sh().evaluate_lambert(sun.into())[0],
    );
    assert!(with < 0.5 && without > 1.5, "{with} {without}");
}

#[test]
fn probe_mips_blur_with_roughness() {
    let Some(gpu) = gpu() else { return };
    // A checker of 16 x 8 cells: each mip has lower contrast than the one before.
    let img = image_from(512, 256, |d| {
        let phi = d[1].atan2(d[0]);
        let th = d[2].asin();
        let a = ((phi / std::f32::consts::TAU * 16.0).floor()
            + (th / std::f32::consts::PI * 8.0).floor()) as i32;
        if a.rem_euclid(2) == 0 {
            [1.0; 3]
        } else {
            [0.0; 3]
        }
    });
    let s = WorldProbeSettings {
        probe_extent: 512,
        ..Default::default()
    };
    let wl = build(&gpu, &img, 0.0, &s).unwrap();
    let mut prev = f32::INFINITY;
    for mip in 0..5 {
        let t = &read_rgba16f(&gpu, &wl.octahedral, mip).unwrap()[0];
        let mean = t.iter().map(|p| p[0]).sum::<f32>() / t.len() as f32;
        let var = t.iter().map(|p| (p[0] - mean).powi(2)).sum::<f32>() / t.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "mip {mip}: mean {mean}");
        assert!(var < prev, "mip {mip}: variance {var} >= {prev}");
        prev = var;
    }
    assert!(prev < 0.01, "the roughest mip is nearly uniform: {prev}");
}

#[test]
fn world_lighting_from_scenes() {
    let Some(gpu) = gpu() else { return };
    let mut sc = RenderScene::default();
    assert!(
        world_lighting(&gpu, &sc).unwrap().is_none(),
        "colour worlds are WP 5.1's"
    );
    // An HDRI world: strength multiplies the image; a missing image is magenta.
    sc.world = RWorld::Hdri {
        image: ImageKey(7),
        rotation: 0.0,
        strength: 2.0,
    };
    sc.images.insert(
        ImageKey(7),
        Arc::new(DecodedImage {
            width: 4,
            height: 2,
            channels: 4,
            pixels: PixelData::F32(vec![0.25; 32]),
        }),
    );
    let wl = world_lighting(&gpu, &sc).unwrap().expect("hdri");
    assert!(close3(
        wl.sh().evaluate_lambert([0.0, 0.0, 1.0]),
        [0.5; 3],
        5e-3
    ));
    sc.images.clear();
    let wl = world_lighting(&gpu, &sc).unwrap().expect("hdri");
    let e = wl.sh().evaluate_lambert([0.0, 0.0, 1.0]);
    assert!(close3(e, [2.0, 0.0, 2.0], 5e-3), "{e:?}");
}

// --- Blender goldens -----------------------------------------------------------------------

/// A `render_eevee/world` case as the preview sees it.
struct Case {
    name: String,
    min_ssim: f64,
    image: String,
    hdri: String,
    rotation: f32,
    strength: f32,
    scene: preview::Scene,
    view_transform: String,
}

fn floats(s: &str) -> Vec<f32> {
    s.trim_matches(|c| c == '(' || c == ')' || c == ' ')
        .split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse().unwrap_or_else(|e| panic!("{t:?}: {e}")))
        .collect()
}

fn kwarg(line: &str, key: &str) -> Option<String> {
    let start = line.find(&format!("{key}="))? + key.len() + 1;
    let rest = &line[start..];
    if rest.starts_with('(') {
        return Some(rest[..=rest.find(')')?].to_owned());
    }
    let end = rest.find([',', ')']).unwrap_or(rest.len());
    Some(rest[..end].trim().to_owned())
}

fn quoted_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    Some(&rest[..rest.find('\'')?])
}

/// Blender's next free name (`Sphere`, `Sphere.001`, …).
fn next_name(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|n| n == base) {
        return base.to_owned();
    }
    (1..)
        .map(|i| format!("{base}.{i:03}"))
        .find(|n| !taken.iter().any(|t| t == n))
        .unwrap()
}

fn parse_case(path: &Path) -> Case {
    let text = std::fs::read_to_string(path).unwrap();
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    let directive = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix("# render-golden:"))
            .and_then(|l| l.split_whitespace().find_map(|kv| kv.strip_prefix(key)))
            .map(str::to_owned)
    };
    let mut objects: Vec<(String, Shape)> = vec![];
    let mut names: Vec<String> = vec!["Camera".into(), "Light".into()];
    let mut materials: Vec<(String, Material)> = vec![];
    let mut assigned: Vec<(String, String)> = vec![];
    let mut c = Case {
        min_ssim: directive("min_ssim=").map_or(0.90, |s| s.parse().unwrap()),
        image: directive("image=").expect("image="),
        name,
        hdri: String::new(),
        rotation: 0.0,
        strength: 1.0,
        scene: preview::Scene {
            width: 1920,
            height: 1080,
            cam_location: [7.358_891, -6.925_791, 4.958_309],
            cam_rotation: [1.109_319, 0.0, 0.814_928],
            lens: 50.0,
            sensor: 36.0,
            shapes: vec![],
        },
        view_transform: "AgX".into(),
    };
    for line in text
        .lines()
        .map(|l| l.strip_prefix("#@render ").unwrap_or(l))
    {
        let rhs = || {
            line.split_once(" = ")
                .map(|(_, r)| r.trim().to_owned())
                .unwrap_or_default()
        };
        if line.starts_with("bpy.ops.mesh.primitive_uv_sphere_add(") {
            let r = kwarg(line, "radius").map_or(1.0, |s| s.parse().unwrap());
            let p = floats(&kwarg(line, "location").unwrap());
            let n = next_name("Sphere", &names);
            names.push(n.clone());
            objects.push((n, Shape::Sphere([p[0], p[1], p[2]], r)));
        } else if line.starts_with("bpy.ops.mesh.primitive_plane_add(") {
            let s: f32 = kwarg(line, "size").map_or(2.0, |s| s.parse().unwrap());
            let p = floats(&kwarg(line, "location").unwrap());
            let n = next_name("Plane", &names);
            names.push(n.clone());
            objects.push((n, Shape::Quad([p[0], p[1], p[2]], s / 2.0)));
        } else if line.starts_with("bpy.ops.object.delete()") {
            names.retain(|n| n != "Cube");
        } else if line.starts_with("bpy.data.images.load(") {
            c.hdri = quoted_after(line, "load('").unwrap().to_owned();
        } else if line.starts_with("bpy.data.materials.new(") {
            materials.push((
                quoted_after(line, "new('").unwrap().to_owned(),
                Material::default(),
            ));
        } else if line.starts_with("bpy.data.materials['") && line.contains(".inputs['") {
            let mat = quoted_after(line, "materials['").unwrap();
            let input = quoted_after(line, ".inputs['").unwrap();
            let v = floats(&rhs());
            let m = &mut materials.iter_mut().find(|(n, _)| n == mat).unwrap().1;
            match input {
                "Base Color" => m.base_color = [v[0], v[1], v[2]],
                "Metallic" => m.metallic = v[0],
                "Roughness" => m.roughness = v[0],
                "IOR" => m.ior = v[0],
                other => panic!("{}: unsupported input {other}", path.display()),
            }
        } else if line.contains(".data.materials.append(") {
            let ob = quoted_after(line, "objects['").unwrap().to_owned();
            let mat = quoted_after(line, "append(bpy.data.materials['")
                .unwrap()
                .to_owned();
            assigned.push((ob, mat));
        } else if line.starts_with("bpy.data.objects['Camera'].location = ") {
            let v = floats(&rhs());
            c.scene.cam_location = [v[0], v[1], v[2]];
        } else if line.starts_with("bpy.data.objects['Camera'].rotation_euler = ") {
            let v = floats(&rhs());
            c.scene.cam_rotation = [v[0], v[1], v[2]];
        } else if line.contains("nodes['Mapping'].inputs['Rotation'].default_value = ") {
            c.rotation = floats(&rhs())[2];
        } else if line.contains("nodes['Background'].inputs['Strength'].default_value = ") {
            c.strength = floats(&rhs())[0];
        } else if line.starts_with("bpy.context.scene.render.resolution_x = ") {
            c.scene.width = rhs().parse().unwrap();
        } else if line.starts_with("bpy.context.scene.render.resolution_y = ") {
            c.scene.height = rhs().parse().unwrap();
        } else if line.starts_with("bpy.context.scene.view_settings.view_transform = ") {
            c.view_transform = rhs().trim_matches('\'').to_owned();
        }
    }
    for (ob, shape) in objects {
        let mat = assigned
            .iter()
            .find(|(o, _)| *o == ob)
            .and_then(|(_, m)| materials.iter().find(|(n, _)| n == m))
            .map_or(Material::default(), |(_, m)| *m);
        c.scene.shapes.push((shape, mat));
    }
    c
}

fn world_cases() -> Vec<PathBuf> {
    let dir = repo().join("tests/golden/cases/render_eevee/world");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    v.sort();
    v
}

#[test]
fn case_parser_reads_the_world_scenes() {
    let cases: Vec<Case> = world_cases().iter().map(|p| parse_case(p)).collect();
    assert!(cases.len() >= 4, "{} HDRI scenes", cases.len());
    for c in &cases {
        assert_eq!(c.image, format!("out/{}.png", c.name));
    }
    let city = cases
        .iter()
        .find(|c| c.name == "city_rotation_strength")
        .unwrap();
    assert_eq!(city.hdri, "city.exr");
    assert_eq!((city.rotation, city.strength), (2.0, 0.7));
    assert_eq!(city.scene.shapes.len(), 4);
    assert!(matches!(city.scene.shapes[0].0, Shape::Quad(_, h) if h == 6.0));
    assert_eq!(city.scene.shapes[2].1.roughness, 0.05);
    assert_eq!(city.view_transform, "Standard");
    let interior = cases.iter().find(|c| c.name == "interior_agx").unwrap();
    assert_eq!(interior.view_transform, "AgX");
    assert_eq!(interior.scene.shapes[3].1, Material::default());
    for c in &cases {
        assert!(c.min_ssim >= 0.90, "{}: EEVEE goldens gate at 0.90", c.name);
        assert!(!c.hdri.is_empty(), "{}: an HDRI world", c.name);
    }
}

fn to_rgba8(c: &Case, film: &[[f32; 4]]) -> Vec<u8> {
    let view = frap_color::ViewSettings {
        view_transform: frap_color::ViewTransform::from_name(&c.view_transform).unwrap(),
        ..Default::default()
    };
    let d = frap_color::display_transform(
        film,
        c.scene.width,
        c.scene.height,
        &view,
        &frap_color::DisplaySettings::default(),
    )
    .unwrap();
    d.to_rgba8(1.0)
}

fn ssim_rgb(a: &[u8], b: &DecodedImage, w: u32, h: u32) -> f64 {
    use image_compare::prelude::RgbImage;
    let rgb = |v: &[u8], ch: usize| -> Vec<u8> {
        v.chunks_exact(ch)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect()
    };
    let bb = b.to_u8();
    let ia = RgbImage::from_raw(w, h, rgb(a, 4)).unwrap();
    let ib = RgbImage::from_raw(w, h, rgb(&bb, b.channels as usize)).unwrap();
    image_compare::rgb_similarity_structure(&image_compare::Algorithm::MSSIMSimple, &ia, &ib)
        .unwrap()
        .score
}

/// CIE76 ΔE of two sRGB colours (as `frap_golden::compare::image_ssim::delta_e`).
fn delta_e(a: [u8; 3], b: [u8; 3]) -> f64 {
    let lab = |c: [u8; 3]| {
        let lin = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = c.map(lin);
        let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
        let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
        let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
        let f = |t: f64| {
            if t > 216.0 / 24389.0 {
                t.cbrt()
            } else {
                (24389.0 / 27.0 * t + 16.0) / 116.0
            }
        };
        let (fx, fy, fz) = (f(x), f(y), f(z));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    };
    let (p, q) = (lab(a), lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

/// Mean ΔE over the centre half of the image (§20.4 `image_ssim`).
fn centre_delta_e(a: &[u8], b: &DecodedImage, w: u32, h: u32) -> f64 {
    let bb = b.to_u8();
    let ch = b.channels as usize;
    let (cw, chh) = (w / 2, h / 2);
    let (x0, y0) = ((w - cw) / 2, (h - chh) / 2);
    let mut sum = 0.0;
    for y in y0..y0 + chh {
        for x in x0..x0 + cw {
            let i = (y * w + x) as usize;
            sum += delta_e(
                [a[4 * i], a[4 * i + 1], a[4 * i + 2]],
                [bb[ch * i], bb[ch * i + 1], bb[ch * i + 2]],
            );
        }
    }
    sum / (cw * chh) as f64
}

#[test]
fn hdri_scenes_match_blender_eevee() {
    let Some(gpu) = gpu() else { return };
    let keep = std::env::var("FRAP_IBL_KEEP").ok();
    let mut failures = vec![];
    for path in world_cases() {
        let c = parse_case(&path);
        let input = path.with_extension("inputs").join(&c.hdri);
        let decoded =
            frap_imbuf::load(&input).unwrap_or_else(|e| panic!("{}: {e:?}", input.display()));
        let img = LinearImage::from_decoded(&decoded, c.strength);
        let wl = build(&gpu, &img, c.rotation, &WorldProbeSettings::default()).unwrap();
        let film = preview::render(&gpu, &wl, &c.scene).unwrap();
        let ours = to_rgba8(&c, &film);
        if let Some(dir) = &keep {
            let _ = std::fs::create_dir_all(dir);
            let out = DecodedImage {
                width: c.scene.width,
                height: c.scene.height,
                channels: 4,
                pixels: PixelData::U8(ours.clone()),
            };
            let _ = frap_imbuf::save(
                &Path::new(dir).join(format!("{}.png", c.name)),
                &out,
                &frap_imbuf::EncodeFormat::Png {
                    bit_depth: 8,
                    compression: 15,
                },
            );
        }
        let fixture = repo()
            .join("tests/golden/fixtures/render_eevee/world")
            .join(format!("{}.blender.png", c.name));
        let blender =
            frap_imbuf::load(&fixture).unwrap_or_else(|e| panic!("{}: {e:?}", fixture.display()));
        assert_eq!(
            (blender.width, blender.height),
            (c.scene.width, c.scene.height)
        );
        let s = ssim_rgb(&ours, &blender, c.scene.width, c.scene.height);
        let de = centre_delta_e(&ours, &blender, c.scene.width, c.scene.height);
        eprintln!(
            "{}: vs Blender 5.2.4 EEVEE: SSIM {s:.4} (gate {}), centre mean ΔE {de:.3} (gate 2)",
            c.name, c.min_ssim
        );
        if s < c.min_ssim {
            failures.push(format!("{}: SSIM {s:.4} < {}", c.name, c.min_ssim));
        }
        if de > 2.0 {
            failures.push(format!("{}: centre mean ΔE {de:.3} > 2", c.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn equirect_pixel_directions_invert_the_mapping() {
    for (x, y) in [(10, 5), (100, 60), (255, 127), (3, 120)] {
        let d = pixel_direction(x, y, 256, 128);
        let uv = equirect::uv(d, 0.0);
        let (u, v) = ((x as f32 + 0.5) / 256.0, 1.0 - (y as f32 + 0.5) / 128.0);
        assert!(
            (uv[0] - u).abs() < 1e-4 && (uv[1] - v).abs() < 1e-4,
            "{uv:?} vs {u} {v}"
        );
    }
    // The octahedral texel solid angles at the bake's resolution cover the sphere.
    let sum: f64 = (0..64u32)
        .flat_map(|y| (0..64u32).map(move |x| octahedral::texel_solid_angle([x, y], 64) as f64))
        .sum();
    assert!((sum - 4.0 * std::f64::consts::PI).abs() < 1e-2);
}
