// SPDX-License-Identifier: GPL-3.0-or-later
//! wgpu clear-and-readback on the real adapter (Metal locally; lavapipe / WARP in
//! `.github/workflows/gpu-probe.yml`). Fails when no adapter exists, unless
//! `FRAP_GPU_OPTIONAL=1` (then each test prints a visible SKIP notice).

use frap_probe_render::{Gpu, GpuOptions, TRIANGLE_RGBA, new_service};
use frap_probe_render_scene::RenderScene;

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(g) => {
            let s = g.summary();
            eprintln!(
                "adapter: {} | backend {} | type {} | driver {}",
                s.name, s.backend, s.device_type, s.driver
            );
            if let Ok(want) = std::env::var("FRAP_EXPECT_ADAPTER") {
                let all = format!("{} {} {}", s.name, s.backend, s.driver);
                assert!(all.contains(&want), "expected adapter containing {want:?}, got {all:?}");
            }
            Some(g)
        }
        Err(e) if std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1") => {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter: {e:?}");
            None
        }
        Err(e) => panic!("no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip): {e:?}"),
    }
}

#[test]
fn clear_and_readback_rgba32float_odd_width() {
    let Some(g) = gpu() else { return };
    // 67 * 16 = 1072 bytes per row, padded to 1280: exercises the row padding.
    let (w, h) = (67, 33);
    let px = g
        .clear_and_readback(w, h, wgpu::TextureFormat::Rgba32Float, [0.25, 0.5, 0.75, 1.0], false)
        .unwrap();
    assert_eq!(px.len(), (w * h) as usize);
    assert!(px.iter().all(|p| *p == [0.25, 0.5, 0.75, 1.0]), "first={:?}", px[0]);
}

#[test]
fn clear_and_readback_rgba8unorm_padding() {
    let Some(g) = gpu() else { return };
    let (w, h) = (13, 7);
    let px = g
        .clear_and_readback(w, h, wgpu::TextureFormat::Rgba8Unorm, [1.0, 0.0, 0.0, 1.0], false)
        .unwrap();
    assert!(px.iter().all(|p| *p == [1.0, 0.0, 0.0, 1.0]), "first={:?}", px[0]);
}

#[test]
fn wgsl_pipeline_draw_and_readback() {
    let Some(g) = gpu() else { return };
    let (w, h) = (64, 32);
    let t = std::time::Instant::now();
    let px = g
        .clear_and_readback(w, h, wgpu::TextureFormat::Rgba32Float, [0.0, 0.0, 0.0, 1.0], true)
        .unwrap();
    eprintln!("first pipeline + draw + readback: {:?}", t.elapsed());
    let at = |x: u32, y: u32| px[(y * w + x) as usize];
    assert_eq!(at(1, h - 2), TRIANGLE_RGBA, "inside the lower-left triangle");
    assert_eq!(at(w - 2, 1), [0.0, 0.0, 0.0, 1.0], "outside the triangle (clear colour)");
}

#[test]
fn render_service_box_dyn() {
    if gpu().is_none() {
        return;
    }
    let mut svc = new_service(&GpuOptions::from_env()).unwrap();
    let film = svc
        .render(&RenderScene {
            resolution: [8, 4],
            world_rgb: [0.5, 0.25, 0.125],
            objects: vec![],
        })
        .unwrap();
    assert_eq!((film.width, film.height, film.rgba.len()), (8, 4, 32));
    assert!(film.rgba.iter().all(|p| *p == [0.5, 0.25, 0.125, 1.0]));
    eprintln!("service adapter: {:?}", svc.adapter_info());
}
