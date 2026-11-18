// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the `frap-render` GPU tests: adapter selection with the
//! `FRAP_GPU_OPTIONAL` / `FRAP_EXPECT_ADAPTER` policy of `.github/workflows/gpu.yml`, synthetic
//! `RenderScene` builders, SSIM.
#![allow(dead_code)]

use std::sync::Arc;

use frap_material::{AlphaMode, Channel, LoweredMaterial, MaterialExts};
use frap_render::{Gpu, GpuOptions};
use frap_render_scene::{MatKey, MeshKey, RCamera, RMesh, RObject, RenderScene};
use glam::{Mat4, Vec3};

/// Whether a missing adapter is a skip (`FRAP_GPU_OPTIONAL=1`) rather than a failure.
pub fn gpu_optional() -> bool {
    std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1")
}

/// Checks the adapter against `FRAP_EXPECT_ADAPTER`.
pub fn check_adapter(s: &frap_render_scene::AdapterSummary) {
    eprintln!(
        "adapter: {} | backend {} | type {} | driver {}",
        s.name, s.backend, s.device_type, s.driver
    );
    if let Ok(want) = std::env::var("FRAP_EXPECT_ADAPTER") {
        let all = format!("{} {} {}", s.name, s.backend, s.driver);
        assert!(
            all.contains(&want),
            "expected adapter containing {want:?}, got {all:?}"
        );
    }
}

/// The test device, or `None` (with a visible SKIP notice) when there is no adapter and
/// `FRAP_GPU_OPTIONAL=1`.
pub fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(g) => {
            check_adapter(&g.summary());
            Some(g)
        }
        Err(e) if gpu_optional() => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter: {e:?}");
            None
        }
        Err(e) => panic!("no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip): {e:?}"),
    }
}

/// A unit cube (`size=2` like `primitive_cube_add`), corner-split with face normals.
pub fn cube_mesh() -> RMesh {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // (normal, u axis, v axis): u × v = normal, so the quad winds counter-clockwise.
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
            let p = [0, 1, 2].map(|i| n[i] + su * u[i] + sv * v[i]);
            m.positions.push(p);
            m.normals.push(n);
        }
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m.submeshes = vec![(0, 0..36)];
    m
}

/// A smooth UV sphere of radius 1 (`segments × rings`), corner-split.
pub fn sphere_mesh(segments: u32, rings: u32) -> RMesh {
    let mut m = RMesh::default();
    let at = |s: u32, r: u32| {
        let theta = std::f32::consts::PI * r as f32 / rings as f32;
        let phi = 2.0 * std::f32::consts::PI * s as f32 / segments as f32;
        [
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        ]
    };
    for r in 0..rings {
        for s in 0..segments {
            let q = [at(s, r), at(s + 1, r), at(s + 1, r + 1), at(s, r + 1)];
            // Outward winding seen from outside: (0, 3, 2), (0, 2, 1).
            for tri in [[0usize, 3, 2], [0, 2, 1]] {
                let base = m.positions.len() as u32;
                for i in tri {
                    m.positions.push(q[i]);
                    m.normals.push(q[i]);
                }
                m.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
    }
    let n = m.indices.len() as u32;
    m.submeshes = vec![(0, 0..n)];
    m
}

/// A material with constant factors.
pub fn material(rgba: [f32; 4], metallic: f32, roughness: f32) -> LoweredMaterial {
    LoweredMaterial {
        base_color: Channel {
            factor: rgba,
            texture: None,
        },
        metallic: Channel {
            factor: metallic,
            texture: None,
        },
        roughness: Channel {
            factor: roughness,
            texture: None,
        },
        normal: None,
        occlusion: None,
        emissive: Channel {
            factor: [0.0; 3],
            texture: None,
        },
        emissive_strength: 1.0,
        alpha_mode: AlphaMode::Opaque,
        double_sided: true,
        unlit: false,
        ext: MaterialExts::default(),
        vertex_color: None,
        diagnostics: vec![],
    }
}

/// A perspective camera at `eye` looking at `target` (Z up), 50 mm-ish field of view.
pub fn camera(eye: Vec3, target: Vec3, w: u32, h: u32) -> RCamera {
    RCamera {
        view: look_at(eye, target, Vec3::Z),
        projection: perspective(0.69, w as f32 / h as f32, 0.1, 100.0),
        is_perspective: true,
        clip: [0.1, 100.0],
        pixel_aspect: 1.0,
    }
}

/// Right-handed world -> view (camera looks along -Z).
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = (target - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(
        glam::Vec4::new(s.x, u.x, -f.x, 0.0),
        glam::Vec4::new(s.y, u.y, -f.y, 0.0),
        glam::Vec4::new(s.z, u.z, -f.z, 0.0),
        glam::Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed perspective projection with wgpu's [0, 1] depth range.
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (0.5 * fov_y).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        glam::Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, r, -1.0),
        glam::Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// An object.
pub fn object(name: &str, mesh: u64, m: Mat4, materials: Vec<Option<MatKey>>) -> RObject {
    RObject {
        name: name.into(),
        matrix_world: m,
        mesh: Some(MeshKey(mesh)),
        materials,
        color: [1.0, 1.0, 1.0, 1.0],
        random_hash: 0,
        display_type: "TEXTURED".into(),
    }
}

/// A scene with one cube (mesh key 1) at the origin, material key 1, seen from the front-right.
pub fn cube_scene(w: u32, h: u32) -> RenderScene {
    let mut sc = RenderScene {
        resolution: [w, h],
        camera: camera(Vec3::new(4.0, -5.0, 3.0), Vec3::ZERO, w, h),
        ..Default::default()
    };
    sc.meshes.insert(MeshKey(1), Arc::new(cube_mesh()));
    sc.materials.insert(
        MatKey(1),
        Arc::new(material([0.8, 0.1, 0.1, 1.0], 0.0, 0.5)),
    );
    sc.objects
        .push(object("Cube", 1, Mat4::IDENTITY, vec![Some(MatKey(1))]));
    sc
}

/// Pixel accessor.
pub fn px(f: &frap_render_scene::Film, x: u32, y: u32) -> [f32; 4] {
    f.rgba[(y * f.width + x) as usize]
}

/// SSIM (image-compare's MSSIM over 8×8 windows, minimum over R, G, B) of two films after a
/// clamp to [0, 1] and 8-bit quantisation (what a PNG self-golden stores).
pub fn ssim(a: &frap_render_scene::Film, b: &frap_render_scene::Film) -> f64 {
    use image_compare::prelude::RgbImage;
    assert_eq!((a.width, a.height), (b.width, b.height));
    let to_img = |f: &frap_render_scene::Film| {
        let raw: Vec<u8> = f
            .rgba
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        RgbImage::from_raw(f.width, f.height, raw).expect("buffer size")
    };
    image_compare::rgb_similarity_structure(
        &image_compare::Algorithm::MSSIMSimple,
        &to_img(a),
        &to_img(b),
    )
    .expect("same dimensions")
    .score
}
