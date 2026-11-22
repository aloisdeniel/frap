// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 7.3: frap's glTF texture formats against Blender 5.2.4's files.
//!
//! Every case of `tests/golden/cases/gltf/textures/` that lists output files
//! (`# gltf-output: out/a.glb out/sep/b.gltf …`) is run through frap (as the golden runner does)
//! and each output is compared with Blender's run of the same case
//! (`tests/golden/fixtures/gltf/textures/<case>/<path below out/>`, the `.gltf` of
//! `GLTF_SEPARATE` with its `.bin` and texture files):
//!
//! - the canonical glTF (`gltf_canonical`: textures with their samplers and extensions resolved,
//!   images by name and MIME type, `extensionsUsed` / `extensionsRequired`, materials, meshes)
//!   with the image bytes left out, at the case's tolerance;
//! - the `images` array in file order: name, MIME type, URI;
//! - each image's pixels, decoded: equal for PNG and lossless WebP (`VP8L`), within a mean
//!   absolute difference of [`LOSSY_MEAN`] (and the same lossy / lossless kind) for JPEG and
//!   lossy WebP, whose encoders differ (frap's JPEG port and libwebp 1.3 vs OIIO's libwebp);
//! - `GLTF_SEPARATE`: the same files next to the `.gltf` (WebP images take their names in the
//!   add-on's order, and images replaced by the WebP rules are not written).
//!
//! The golden comparator (`glb:`) hashes non-PNG image bytes, which two WebP encoders never
//! share, so these cases compare only their line outcomes and materials there and their files
//! here. Blessing: `FRAP_TEXTURES_BLESS=1 cargo test -p frap-ops-io-tests --test gltf_textures
//! -- --nocapture` rewrites the Blender outputs when Blender 5.2 is found
//! (`FRAP_TEXTURES_BLESS=<case name>`: that case only).

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::case::Case;
use frap_golden::compare::gltf_canonical::{self, GltfDoc};
use frap_golden::runner::{Props, RunOptions, run_frap};
use frap_imbuf::DecodedImage;

const AREA: &str = "gltf/textures";
const DIRECTIVE: &str = "# gltf-output:";
/// Largest mean absolute difference (8-bit samples) between frap's and Blender's decoded
/// JPEG or lossy WebP images.
const LOSSY_MEAN: f64 = 6.0;

struct TexCase {
    case: Case,
    outputs: Vec<String>,
}

fn cases(repo: &Repo) -> Vec<TexCase> {
    let root = repo.cases_dir();
    let dir = root.join(AREA);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let outputs: Vec<String> = text
                .lines()
                .find_map(|l| l.strip_prefix(DIRECTIVE))?
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            let case = Case::load(&p, &root).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
            Some(TexCase { case, outputs })
        })
        .collect()
}

/// `out/sep/b.gltf` → `<fixtures>/gltf/textures/<case>/sep/b.gltf`.
fn fixture(repo: &Repo, case: &Case, out: &str) -> PathBuf {
    let rel = out.strip_prefix("out/").unwrap_or(out);
    repo.fixtures_dir().join(AREA).join(&case.name).join(rel)
}

/// The output file and, for a `.gltf`, the files its URIs name (relative to its folder).
fn output_files(path: &Path) -> Vec<String> {
    let mut v = vec![path.file_name().unwrap().to_string_lossy().into_owned()];
    if path.extension().is_some_and(|x| x == "gltf") {
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for k in ["buffers", "images"] {
            for e in json[k].as_array().into_iter().flatten() {
                if let Some(u) = e["uri"].as_str()
                    && !u.starts_with("data:")
                {
                    v.push(u.to_owned());
                }
            }
        }
    }
    v
}

fn bless(repo: &Repo, tc: &TexCase) {
    let Some(blender) = frap_golden::locate_blender() else {
        eprintln!(
            "{}: Blender 5.2 not found; fixtures not refreshed",
            tc.case.name
        );
        return;
    };
    let work = ScratchDir::new(&format!("bless_tex_{}", tc.case.name)).unwrap();
    let run = work.path().join("run");
    let state = run_blender(&blender, repo, &tc.case.path, &run).unwrap();
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        panic!(
            "{}: Blender failed on line {}: {:?} {:?}",
            tc.case.name, bad.line, bad.exception, bad.message
        );
    }
    let dir = repo.fixtures_dir().join(AREA).join(&tc.case.name);
    let _ = std::fs::remove_dir_all(&dir);
    for out in &tc.outputs {
        let src = run.join(out);
        let dst = fixture(repo, &tc.case, out);
        for f in output_files(&src) {
            let (s, d) = (src.with_file_name(&f), dst.with_file_name(&f));
            std::fs::create_dir_all(d.parent().unwrap()).unwrap();
            std::fs::copy(&s, &d).unwrap_or_else(|e| panic!("{}: {e}", s.display()));
        }
    }
    eprintln!(
        "{}: Blender fixtures written to {}",
        tc.case.name,
        dir.display()
    );
}

fn load(path: &Path) -> GltfDoc {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    gltf_canonical::load(&bytes, path.parent()).unwrap()
}

/// The encoded bytes of image `i`.
fn image_bytes(doc: &GltfDoc, dir: &Path, i: usize) -> Vec<u8> {
    let img = &doc.json["images"][i];
    if let Some(v) = img["bufferView"].as_u64() {
        let bv = &doc.json["bufferViews"][v as usize];
        let b = &doc.buffers[bv["buffer"].as_u64().unwrap_or(0) as usize];
        let o = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let n = bv["byteLength"].as_u64().unwrap() as usize;
        return b[o..o + n].to_vec();
    }
    let uri = img["uri"]
        .as_str()
        .expect("an image with a uri or a bufferView");
    std::fs::read(dir.join(uri)).unwrap()
}

/// `(name, mimeType, uri)` of every image, in file order.
fn image_list(doc: &GltfDoc) -> Vec<(String, String, String)> {
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_owned();
    doc.json["images"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| (s(&i["name"]), s(&i["mimeType"]), s(&i["uri"])))
        .collect()
}

/// RGBA 8-bit samples of an encoded image.
fn rgba(bytes: &[u8]) -> DecodedImage {
    let d = frap_imbuf::decode(bytes).unwrap();
    let d = d.with_channels(4).unwrap();
    DecodedImage::new(d.width, d.height, 4, frap_imbuf::PixelData::U8(d.to_u8())).unwrap()
}

fn is_lossy(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.get(12..16) == Some(b"VP8 ")
}

fn compare_images(what: &str, f: (&GltfDoc, &Path), b: (&GltfDoc, &Path)) {
    let (fl, bl) = (image_list(f.0), image_list(b.0));
    assert_eq!(fl, bl, "{what}: images (name, mimeType, uri)");
    for (i, (name, mime, _)) in bl.iter().enumerate() {
        let (fb, bb) = (image_bytes(f.0, f.1, i), image_bytes(b.0, b.1, i));
        let label = format!("{what}: image {i} '{name}' ({mime})");
        assert_eq!(
            is_lossy(&fb),
            is_lossy(&bb),
            "{label}: lossy / lossless kind"
        );
        let (fd, bd) = (rgba(&fb), rgba(&bb));
        assert_eq!(
            (fd.width, fd.height),
            (bd.width, bd.height),
            "{label}: size"
        );
        let (fp, bp) = (fd.to_u8(), bd.to_u8());
        if is_lossy(&bb) {
            let mean = fp
                .iter()
                .zip(&bp)
                .map(|(x, y)| f64::from(x.abs_diff(*y)))
                .sum::<f64>()
                / bp.len() as f64;
            assert!(mean <= LOSSY_MEAN, "{label}: mean difference {mean}");
        } else {
            assert_eq!(fp, bp, "{label}: pixels");
        }
    }
}

/// The canonical form without image bytes (compared by [`compare_images`]) and without empty
/// texture `extensions` objects: the add-on writes `"extensions": {}` on a texture whose
/// EXT_texture_webp image could not be written (`export_image_format='WEBP'`), frap leaves the
/// member out; both mean no extension.
fn canonical_without_image_bytes(mut doc: GltfDoc) -> serde_json::Value {
    if let Some(images) = doc.json["images"].as_array_mut() {
        for i in images {
            if let Some(o) = i.as_object_mut() {
                o.remove("bufferView");
                o.remove("uri");
            }
        }
    }
    if let Some(textures) = doc.json["textures"].as_array_mut() {
        for t in textures {
            if let Some(o) = t.as_object_mut()
                && o.get("extensions")
                    .is_some_and(|e| e.as_object().is_some_and(|m| m.is_empty()))
            {
                o.remove("extensions");
            }
        }
    }
    gltf_canonical::canonicalize(&doc).unwrap()
}

fn files_next_to(path: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// The scene of the KTX2 inputs: the factory cube's material samples a UV grid (Base Color)
/// and a flat Non-Color image (Roughness).
const KTX2_SCENE: &[&str] = &[
    "bpy.ops.image.new(name='Grid', width=32, height=32, generated_type='UV_GRID')",
    "bpy.data.images.new('Rough', 16, 16)",
    "bpy.data.images['Rough'].generated_color = (0.2, 0.6, 0.9, 1.0)",
    "bpy.data.images['Rough'].colorspace_settings.name = 'Non-Color'",
    "bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')",
    "bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']",
    "bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])",
    "bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')",
    "bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Rough']",
    "bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])",
    "bpy.ops.export_scene.gltf(filepath='ktx2_fallback.glb', frap_texture_format='KTX2')",
    "bpy.ops.export_scene.gltf(filepath='uastc/ktx2_uastc.gltf', export_format='GLTF_SEPARATE', frap_texture_format='KTX2', frap_basisu_mode='UASTC')",
];

/// `ktx2_uastc.gltf` without its PNG images: every texture keeps only its
/// `KHR_texture_basisu` source, the extension required (a file Blender refuses).
fn ktx2_only(gltf: &[u8]) -> serde_json::Value {
    let mut j: serde_json::Value = serde_json::from_slice(gltf).unwrap();
    let images = j["images"].as_array().unwrap().clone();
    let keep: Vec<usize> = (0..images.len())
        .filter(|&i| images[i]["mimeType"] == "image/ktx2")
        .collect();
    j["images"] = keep.iter().map(|&i| images[i].clone()).collect();
    for t in j["textures"].as_array_mut().unwrap() {
        let o = t.as_object_mut().unwrap();
        o.remove("source");
        let s = &mut o["extensions"]["KHR_texture_basisu"]["source"];
        let old = s.as_u64().unwrap() as usize;
        *s = keep.iter().position(|&k| k == old).unwrap().into();
    }
    j["buffers"][0]["uri"] = "ktx2_only.bin".into();
    j["extensionsRequired"] = serde_json::json!(["KHR_texture_basisu"]);
    j
}

/// The JSON chunk of a GLB.
fn glb_json(bytes: &[u8]) -> serde_json::Value {
    let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[20..20 + len]).unwrap()
}

/// `frap_texture_format='KTX2'` (frap's extension, no Blender reference): every texture gets a
/// `KHR_texture_basisu` source next to the images `export_image_format` gives it, the
/// extension required only when no core `source` is left; the KTX2 images decode to the PNG
/// fallback's pixels (UASTC, within the codec's error) with the sRGB transfer function for
/// the base colour and linear for the metallic-roughness data; a file with fallbacks imports
/// its PNG images (as Blender would). In a build without the encoder the export answers
/// NOT_IMPLEMENTED.
#[test]
fn ktx2_export_structure() {
    let work = ScratchDir::new("ktx2_export").unwrap();
    let mut opts = frap_session::SessionOptions::default();
    opts.cwd = work.path().to_owned();
    let mut s = frap_session::Session::new(opts).unwrap();
    let run = |s: &mut frap_session::Session, line: &str| -> serde_json::Value {
        let r = s.exec_source(line);
        r[0].to_json()
    };
    let n = KTX2_SCENE.len() - 2;
    for line in &KTX2_SCENE[..n] {
        let j = run(&mut s, line);
        assert_eq!(j["ok"], true, "{line}: {j}");
    }
    let j = run(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='k.glb', frap_texture_format='KTX2', frap_basisu_mode='UASTC', export_image_quality=90)",
    );
    if !frap_imbuf::basis::CAN_ENCODE {
        assert_eq!(j["ok"], false, "{j}");
        assert_eq!(j["error"]["code"], "OP_NOT_IMPLEMENTED", "{j}");
        assert!(
            j["error"]["message"]
                .as_str()
                .unwrap()
                .contains("`basisu-encode` feature"),
            "{j}"
        );
        return;
    }
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(
        j["result"]["extensions_used"],
        serde_json::json!(["KHR_texture_basisu"])
    );
    // The PNG stays the texture's source: KHR_texture_basisu is not required, no WARNING (M8
    // W0, decision log M8W0-7).
    let ktx2_required = |j: &serde_json::Value| {
        j["reports"].as_array().is_some_and(|rs| {
            rs.iter().any(|r| {
                r["type"] == "WARNING"
                    && r["message"] == frap_io_gltf::export::images::KTX2_REQUIRED_WARNING
            })
        })
    };
    assert!(!ktx2_required(&j), "{j}");
    let bytes = std::fs::read(work.path().join("k.glb")).unwrap();
    let json = glb_json(&bytes);
    assert!(json.get("extensionsRequired").is_none(), "{json}");
    let doc = gltf_canonical::load(&bytes, None).unwrap();
    let mimes: Vec<String> = image_list(&doc).into_iter().map(|i| i.1).collect();
    // The traversal order: each texture's extension image before its own source.
    assert_eq!(
        mimes,
        ["image/ktx2", "image/png", "image/ktx2", "image/png"]
    );
    for (t, srgb) in [(0, true), (1, false)] {
        let tex = &json["textures"][t];
        let (k, p) = (
            tex["extensions"]["KHR_texture_basisu"]["source"]
                .as_u64()
                .unwrap() as usize,
            tex["source"].as_u64().unwrap() as usize,
        );
        let (kb, pb) = (
            image_bytes(&doc, work.path(), k),
            image_bytes(&doc, work.path(), p),
        );
        let d = rgba(&kb);
        let want = rgba(&pb);
        let mean = d
            .to_u8()
            .iter()
            .zip(want.to_u8())
            .map(|(a, b)| f64::from(a.abs_diff(b)))
            .sum::<f64>()
            / (d.to_u8().len() as f64);
        assert!(
            mean <= 2.0,
            "texture {t}: KTX2 vs PNG mean difference {mean}"
        );
        let info = frap_imbuf::probe(&kb).unwrap();
        assert_eq!(info.format, frap_imbuf::FileFormat::Ktx2);
        // DFD basic descriptor block word 2 (after the 4-byte DFD size and the 8-byte block
        // header): colour primaries (1 BT709, 0 unspecified: KHR_texture_basisu requires
        // unspecified for non-colour data) and transfer function (1 linear, 2 sRGB).
        let dfd = u32::from_le_bytes(kb[48..52].try_into().unwrap()) as usize;
        assert_eq!(
            (kb[dfd + 4 + 8 + 1], kb[dfd + 4 + 8 + 2]),
            if srgb { (1, 2) } else { (0, 1) },
            "texture {t}"
        );
        let usage = if srgb {
            frap_gltf::ext::khr_texture_basisu::Usage::Colour
        } else {
            frap_gltf::ext::khr_texture_basisu::Usage::Data
        };
        assert_eq!(
            frap_gltf::ext::khr_texture_basisu::ktx2_issues(&kb, Some(usage)),
            [],
            "texture {t}"
        );
    }
    // Validates as glTF (the extension's rules included, the embedded KTX2 images too).
    let r = frap_gltf::validate::validate_file(&bytes).unwrap();
    assert!(r.is_ok(), "{r}");
    // KHR_texture_basisu: pixelWidth and pixelHeight MUST be multiples of 4. A 5x3 image is
    // written 8x4 (and decodes at that size); its PNG fallback keeps 5x3.
    for line in [
        "bpy.data.images.new('Odd', 5, 3)",
        "bpy.data.images['Odd'].generated_color = (0.9, 0.3, 0.1, 1.0)",
        "bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Odd']",
        "bpy.ops.export_scene.gltf(filepath='odd.glb', frap_texture_format='KTX2')",
    ] {
        let j = run(&mut s, line);
        assert_eq!(j["ok"], true, "{line}: {j}");
    }
    let odd = std::fs::read(work.path().join("odd.glb")).unwrap();
    let (oj, od) = (glb_json(&odd), gltf_canonical::load(&odd, None).unwrap());
    let tex = &oj["textures"][0];
    let k = tex["extensions"]["KHR_texture_basisu"]["source"]
        .as_u64()
        .unwrap() as usize;
    let p = tex["source"].as_u64().unwrap() as usize;
    let kb = image_bytes(&od, work.path(), k);
    assert_eq!(
        (
            u32::from_le_bytes(kb[20..24].try_into().unwrap()),
            u32::from_le_bytes(kb[24..28].try_into().unwrap())
        ),
        (8, 4),
        "KTX2 pixelWidth / pixelHeight"
    );
    let d = rgba(&kb);
    assert_eq!((d.width, d.height), (8, 4));
    let png = rgba(&image_bytes(&od, work.path(), p));
    assert_eq!((png.width, png.height), (5, 3));
    // The resampled image keeps the (uniform) colour.
    let want = &png.to_u8()[..4];
    for px in d.to_u8().chunks_exact(4) {
        assert!(
            px.iter().zip(want).all(|(a, b)| a.abs_diff(*b) <= 4),
            "{px:?} vs {want:?}"
        );
    }
    let r = frap_gltf::validate::validate_file(&odd).unwrap();
    assert!(r.is_ok(), "{r}");
    // WEBP: no core source left, so KHR_texture_basisu is required (with EXT_texture_webp).
    let j = run(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='kw.glb', frap_texture_format='KTX2', export_image_format='WEBP')",
    );
    assert_eq!(j["ok"], true, "{j}");
    let kw = glb_json(&std::fs::read(work.path().join("kw.glb")).unwrap());
    assert_eq!(
        kw["extensionsRequired"],
        serde_json::json!(["EXT_texture_webp", "KHR_texture_basisu"]),
        "{kw}"
    );
    assert!(kw["textures"][0].get("source").is_none());
    assert!(ktx2_required(&j), "{j}");
    // Imported back, the PNG fallbacks are used, as Blender's add-on does.
    for line in [
        "bpy.ops.wm.read_homefile(use_empty=True)",
        "bpy.ops.import_scene.gltf(filepath='k.glb')",
    ] {
        let j = run(&mut s, line);
        assert_eq!(j["ok"], true, "{line}: {j}");
    }
    let j = run(&mut s, "bpy.data.images['Grid'].file_format");
    assert_eq!(j["value"], "PNG", "{j}");
}

/// Writes the KTX2 inputs of `tests/golden/cases/gltf/textures/ktx2_import.inputs/`, the KTX2
/// CLI transcript (`tests/cli/m7/textures/ktx2.in/`) and the KTX2 fixtures of `frap-imbuf`
/// (`tests/golden/fixtures/gltf/textures/ktx2/`), exported by frap: run with
/// `cargo test -p frap-ops-io-tests --features frap-ops-all/basisu-encode --test gltf_textures
/// -- --ignored write_ktx2_inputs`.
#[test]
#[ignore = "writes the KTX2 inputs; needs the basisu-encode feature"]
fn write_ktx2_inputs() {
    if !frap_imbuf::basis::CAN_ENCODE {
        panic!("build with --features frap-ops-all/basisu-encode");
    }
    let repo = Repo::locate();
    let work = ScratchDir::new("ktx2_inputs").unwrap();
    let mut opts = frap_session::SessionOptions::default();
    opts.cwd = work.path().to_owned();
    let mut s = frap_session::Session::new(opts).unwrap();
    for line in KTX2_SCENE {
        for r in s.exec_source(line) {
            assert!(r.ok(), "{line}: {}", r.to_json());
        }
    }
    let w = work.path();
    let only = ktx2_only(&std::fs::read(w.join("uastc/ktx2_uastc.gltf")).unwrap());
    let mut files: Vec<(String, Vec<u8>)> = vec![
        (
            "ktx2_fallback.glb".into(),
            std::fs::read(w.join("ktx2_fallback.glb")).unwrap(),
        ),
        (
            "ktx2_only.gltf".into(),
            serde_json::to_vec_pretty(&only).unwrap(),
        ),
        (
            "ktx2_only.bin".into(),
            std::fs::read(w.join("uastc/ktx2_uastc.bin")).unwrap(),
        ),
    ];
    for img in only["images"].as_array().unwrap() {
        let uri = img["uri"].as_str().unwrap().to_owned();
        files.push((
            uri.clone(),
            std::fs::read(w.join("uastc").join(&uri)).unwrap(),
        ));
    }
    let tests = repo
        .cases_dir()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    for dir in [
        repo.cases_dir().join(AREA).join("ktx2_import.inputs"),
        tests.join("cli/m7/textures/ktx2.in"),
    ] {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in &files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
    // frap-imbuf's fixtures: the UASTC grid (RGB, sRGB), its PNG, and an ETC1S RGBA image.
    let fx = repo.fixtures_dir().join(AREA).join("ktx2");
    std::fs::create_dir_all(&fx).unwrap();
    std::fs::copy(w.join("uastc/Grid.ktx2"), fx.join("grid_uastc.ktx2")).unwrap();
    std::fs::copy(w.join("uastc/Grid.png"), fx.join("grid.png")).unwrap();
    let grid = frap_imbuf::decode(&std::fs::read(w.join("uastc/Grid.png")).unwrap()).unwrap();
    let mut rgba = grid.with_channels(4).unwrap();
    if let frap_imbuf::PixelData::U8(v) = &mut rgba.pixels {
        for (i, p) in v.chunks_exact_mut(4).enumerate() {
            p[3] = if (i % 32) < 16 { 255 } else { 64 };
        }
    }
    let etc1s = frap_imbuf::basis::encode_ktx2(&rgba, frap_imbuf::basis::BasisMode::Etc1s).unwrap();
    std::fs::write(fx.join("grid_alpha_etc1s.ktx2"), etc1s).unwrap();
    frap_imbuf::save(
        &fx.join("grid_alpha.png"),
        &rgba,
        &frap_imbuf::EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        },
    )
    .unwrap();
    eprintln!("KTX2 inputs written");
}

#[test]
fn texture_outputs_match_blender() {
    let repo = Repo::locate();
    let props = Props::load(&repo.props_path()).expect("props.toml");
    let all = cases(&repo);
    assert!(!all.is_empty(), "no texture cases with outputs");
    // `FRAP_TEXTURES_BLESS=1` blesses every case, `=<case name>` that case only.
    let blessing = std::env::var("FRAP_TEXTURES_BLESS").ok();
    for tc in &all {
        if blessing
            .as_deref()
            .is_some_and(|b| b == "1" || tc.case.name.ends_with(b))
        {
            bless(&repo, tc);
        }
        let work = ScratchDir::new(&tc.case.name).unwrap();
        let run = run_frap(
            &tc.case,
            &props,
            &RunOptions {
                workdir: work.path().to_owned(),
                strict_rna: false,
            },
        )
        .unwrap_or_else(|e| panic!("{}: {e}", tc.case.name));
        if let Some(bad) = run.state.lines.iter().find(|l| !l.ok) {
            panic!(
                "{}: frap failed on line {}: {:?} {:?}",
                tc.case.name, bad.line, bad.exception, bad.message
            );
        }
        for out in &tc.outputs {
            let what = format!("{} {out}", tc.case.name);
            let (fpath, bpath) = (work.path().join(out), fixture(&repo, &tc.case, out));
            let (f, b) = (load(&fpath), load(&bpath));
            compare_images(
                &what,
                (&f, fpath.parent().unwrap()),
                (&b, bpath.parent().unwrap()),
            );
            let m = gltf_canonical::compare(
                &what,
                &canonical_without_image_bytes(f),
                &canonical_without_image_bytes(b),
                tc.case.tol,
            );
            assert!(m.is_empty(), "{what}: {m:#?}");
            if fpath.extension().is_some_and(|x| x == "gltf") {
                let mut want = output_files(&bpath);
                want.sort();
                assert_eq!(files_next_to(&fpath), want, "{what}: files");
            }
        }
    }
}
