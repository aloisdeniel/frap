// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 7.3: a KHR_texture_basisu (KTX2) texture renders — the one GPU test of M7 (§22.9).
//!
//! `ktx2_only.gltf` (tests/golden/cases/gltf/textures/ktx2_import.inputs/, written by
//! crates/frap-ops-io-tests `write_ktx2_inputs`) is the factory cube whose base colour and
//! metallic-roughness textures have only their UASTC KTX2 image; `ktx2_fallback.glb` is the
//! same scene with ETC1S KTX2 images and the PNG fallbacks the importer prefers (as Blender
//! does). Blender 5.2 cannot load KTX2 (deviation D-133), so the reference is frap's render of the
//! PNG version: both are imported into the factory scene (its cube removed) and rendered with
//! Workbench in `TEXTURE` colour mode and with EEVEE; the KTX2 render must match the PNG one
//! (SSIM ≥ 0.95, mean ΔE ≤ 3: UASTC is near lossless on the grid) and differ from a render
//! without the image (so the texture is what shows, not a fallback colour). The lossy ETC1S
//! path renders too: `ktx2_fallback.glb` with its PNG images removed from the textures (so the
//! ETC1S KTX2 images are imported) against the same PNG render, within ETC1S's looser
//! tolerance ([`ETC1S_MIN_SSIM`], [`ETC1S_MAX_DELTA_E`]). Without an adapter the tests fail,
//! or print a notice and pass under `FRAP_GPU_OPTIONAL=1`.

mod common;

use std::path::Path;

use common::Env;
use frap_golden::compare::image_ssim;

const MIN_SSIM: f64 = 0.95;
const MAX_DELTA_E: f64 = 3.0;
/// ETC1S (BasisLZ, lossy: endpoint / selector codebooks at quality 75) against the PNG render
/// (measured on Metal: SSIM 0.913 Workbench, 0.938 EEVEE; mean ΔE 1.2 and 1.8).
const ETC1S_MIN_SSIM: f64 = 0.88;
const ETC1S_MAX_DELTA_E: f64 = 3.0;

fn inputs() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/cases/gltf/textures/ktx2_import.inputs")
}

/// A session with the inputs in its cwd, the factory cube removed and `file` imported, set up
/// for `engine`.
fn scene(tag: &str, file: &str, engine: &str) -> Env {
    let mut e = Env::new(tag);
    for f in std::fs::read_dir(inputs())
        .expect("ktx2_import.inputs")
        .flatten()
    {
        std::fs::copy(f.path(), e.path(&f.file_name().to_string_lossy())).unwrap();
    }
    e.ok("bpy.data.objects.remove(bpy.data.objects['Cube'])");
    e.ok(&format!("bpy.ops.import_scene.gltf(filepath='{file}')"));
    e.ok(&format!("bpy.context.scene.render.engine = '{engine}'"));
    e.ok("bpy.context.scene.display.shading.color_type = 'TEXTURE'");
    e.ok("bpy.context.scene.render.resolution_x = 96");
    e.ok("bpy.context.scene.render.resolution_y = 72");
    e.ok("bpy.context.scene.render.resolution_percentage = 100");
    e
}

/// Renders to `out/<name>.png` and decodes it (`None`: no GPU, skipped).
fn render(e: &mut Env, name: &str) -> Option<frap_imbuf::DecodedImage> {
    e.ok(&format!("bpy.context.scene.render.filepath = 'out/{name}'"));
    let r = e.render("bpy.ops.render.render(write_still=True)")?;
    assert_eq!(r["status"], "FINISHED", "{r}");
    Some(image_ssim::load_png(&e.path(&format!("out/{name}.png"))).expect("render written"))
}

#[test]
fn ktx2_textures_render_like_their_png_fallbacks() {
    for engine in ["BLENDER_WORKBENCH", "BLENDER_EEVEE"] {
        let mut k = scene("ktx2_render", "ktx2_only.gltf", engine);
        assert_eq!(
            k.run("bpy.data.images['Grid'].file_format")["value"],
            "KTX2",
            "the KTX2 image is the one imported"
        );
        let Some(ktx2) = render(&mut k, "ktx2") else {
            return;
        };
        let mut p = scene("png_render", "ktx2_fallback.glb", engine);
        assert_eq!(p.run("bpy.data.images['Grid'].file_format")["value"], "PNG");
        let png = render(&mut p, "png").expect("GPU available above");
        let m = image_ssim::compare(engine, &ktx2, &png, MIN_SSIM, MAX_DELTA_E);
        assert!(m.is_empty(), "{engine}: KTX2 vs PNG render: {m:#?}");
        eprintln!(
            "{engine}: KTX2 vs PNG {:?}",
            image_ssim::score(&ktx2, &png).unwrap()
        );
        // Without the image the cube renders differently.
        k.ok("bpy.data.objects['Cube'].active_material.node_tree.nodes['Image Texture'].image = None");
        let plain = render(&mut k, "plain").expect("GPU available above");
        let s = image_ssim::score(&plain, &ktx2).unwrap();
        assert!(
            s.ssim < MIN_SSIM || s.mean_delta_e > MAX_DELTA_E,
            "{engine}: the texture does not show ({s:?})"
        );
    }
}

/// `ktx2_fallback.glb` with every texture's core `source` removed (KHR_texture_basisu
/// required), so its ETC1S KTX2 images are the ones imported.
fn etc1s_only(glb: &[u8]) -> Vec<u8> {
    let u32_at = |at: usize| u32::from_le_bytes(glb[at..at + 4].try_into().unwrap()) as usize;
    let json_len = u32_at(12);
    let mut j: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
    for t in j["textures"].as_array_mut().unwrap() {
        assert!(
            t["extensions"]["KHR_texture_basisu"]["source"].is_u64(),
            "{t}"
        );
        t.as_object_mut().unwrap().remove("source");
    }
    j["extensionsRequired"] = serde_json::json!(["KHR_texture_basisu"]);
    let mut json = serde_json::to_vec(&j).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let bin = &glb[20 + json_len..];
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((12 + 8 + json.len() + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(bin);
    out
}

#[test]
fn etc1s_textures_render_within_their_tolerance() {
    let glb = std::fs::read(inputs().join("ktx2_fallback.glb")).unwrap();
    for engine in ["BLENDER_WORKBENCH", "BLENDER_EEVEE"] {
        let mut k = Env::new("ktx2_etc1s_render");
        std::fs::write(k.path("etc1s_only.glb"), etc1s_only(&glb)).unwrap();
        k.ok("bpy.data.objects.remove(bpy.data.objects['Cube'])");
        k.ok("bpy.ops.import_scene.gltf(filepath='etc1s_only.glb')");
        assert_eq!(
            k.run("bpy.data.images['Grid'].file_format")["value"],
            "KTX2"
        );
        for line in [
            format!("bpy.context.scene.render.engine = '{engine}'"),
            "bpy.context.scene.display.shading.color_type = 'TEXTURE'".into(),
            "bpy.context.scene.render.resolution_x = 96".into(),
            "bpy.context.scene.render.resolution_y = 72".into(),
            "bpy.context.scene.render.resolution_percentage = 100".into(),
        ] {
            k.ok(&line);
        }
        let Some(etc1s) = render(&mut k, "etc1s") else {
            return;
        };
        let mut p = scene("png_render_etc1s", "ktx2_fallback.glb", engine);
        let png = render(&mut p, "png").expect("GPU available above");
        let score = image_ssim::score(&etc1s, &png).unwrap();
        eprintln!("{engine}: ETC1S vs PNG {score:?}");
        let m = image_ssim::compare(engine, &etc1s, &png, ETC1S_MIN_SSIM, ETC1S_MAX_DELTA_E);
        assert!(m.is_empty(), "{engine}: ETC1S vs PNG render: {m:#?}");
        // Lossy: the ETC1S image is not the PNG's pixels, so the renders are not identical.
        assert!(
            score.ssim < 1.0 && score.mean_delta_e > 0.0,
            "{engine}: the ETC1S render equals the PNG one ({score:?})"
        );
    }
}
