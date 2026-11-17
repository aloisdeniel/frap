// SPDX-License-Identifier: GPL-3.0-or-later
//! wgpu clear-and-readback on the real adapter (Metal locally; lavapipe / WARP in
//! `.github/workflows/gpu.yml`). Fails when no adapter exists, unless
//! `FRAP_GPU_OPTIONAL=1` (then each test prints a visible SKIP notice). Moved from the WP 0.2
//! contract probe by M1 W0; WP 1.7 owns it.

use frap_render::readback::TRIANGLE_RGBA;
use frap_render::{Gpu, GpuOptions};

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
                assert!(
                    all.contains(&want),
                    "expected adapter containing {want:?}, got {all:?}"
                );
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
        .clear_and_readback(
            w,
            h,
            wgpu::TextureFormat::Rgba32Float,
            [0.25, 0.5, 0.75, 1.0],
            false,
        )
        .unwrap();
    assert_eq!(px.len(), (w * h) as usize);
    assert!(
        px.iter().all(|p| *p == [0.25, 0.5, 0.75, 1.0]),
        "first={:?}",
        px[0]
    );
}

#[test]
fn clear_and_readback_rgba8unorm_padding() {
    let Some(g) = gpu() else { return };
    let (w, h) = (13, 7);
    let px = g
        .clear_and_readback(
            w,
            h,
            wgpu::TextureFormat::Rgba8Unorm,
            [1.0, 0.0, 0.0, 1.0],
            false,
        )
        .unwrap();
    assert!(
        px.iter().all(|p| *p == [1.0, 0.0, 0.0, 1.0]),
        "first={:?}",
        px[0]
    );
}

#[test]
fn wgsl_pipeline_draw_and_readback() {
    let Some(g) = gpu() else { return };
    let (w, h) = (64, 32);
    let t = std::time::Instant::now();
    let px = g
        .clear_and_readback(
            w,
            h,
            wgpu::TextureFormat::Rgba32Float,
            [0.0, 0.0, 0.0, 1.0],
            true,
        )
        .unwrap();
    eprintln!("first pipeline + draw + readback: {:?}", t.elapsed());
    let at = |x: u32, y: u32| px[(y * w + x) as usize];
    assert_eq!(
        at(1, h - 2),
        TRIANGLE_RGBA,
        "inside the lower-left triangle"
    );
    assert_eq!(
        at(w - 2, 1),
        [0.0, 0.0, 0.0, 1.0],
        "outside the triangle (clear colour)"
    );
}

#[test]
fn band_split_readback_at_small_forced_limit() {
    let Some(g) = gpu() else { return };
    // 67 px Rgba32Float rows pad to 1280 bytes; a 3.5-row limit gives bands of 3 rows (the
    // last band of 41 rows holds 2), so every band boundary and the short tail are exercised.
    let (w, h) = (67u32, 41u32);
    let format = wgpu::TextureFormat::Rgba32Float;
    let tex = g.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("band test"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let expect: Vec<[f32; 4]> = (0..h)
        .flat_map(|y| (0..w).map(move |x| [x as f32, y as f32, (x * y) as f32, 1.0]))
        .collect();
    let bytes: Vec<u8> = expect
        .iter()
        .flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes()))
        .collect();
    g.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 16),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    let bpr = u64::from(frap_render::readback::padded_bytes_per_row(w, format));
    let banded = g
        .read_texture_in_bands(&tex, w, h, format, bpr * 7 / 2)
        .unwrap();
    assert_eq!(banded, expect);
    let whole = g.read_texture(&tex, w, h, format).unwrap();
    assert_eq!(whole, expect);
    // Not even one padded row fits: an error, not a panic.
    assert!(
        g.read_texture_in_bands(&tex, w, h, format, bpr - 1)
            .is_err()
    );
}
