// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the differential fuzzer that need the catalogue and frap (WP 8.7), and the
//! regression inputs of the `cargo fuzz` targets whose reader fixes landed outside this crate
//! (`regress/`: the crash inputs, first byte = the target's format selector).

use std::collections::BTreeSet;

use frap_session::{Session, SessionOptions};

use super::catalog::{Catalog, Ctx, Kind, OPERATORS, RNA};
use super::sequence::generate;
use super::*;

#[test]
fn catalogue_covers_150_operators() {
    let c = Catalog::get();
    let ids = c.operator_ids();
    assert!(ids.len() >= 150, "{} operators", ids.len());
    assert_eq!(ids.len(), c.ops.len(), "an operator is listed twice");
    assert!(c.rna.len() >= 40, "{} RNA statements", c.rna.len());
    assert!(c.ops.iter().any(|o| o.ctx == Ctx::Edit));
    assert!(c.ops.iter().any(|o| o.ctx == Ctx::Object));
}

#[test]
fn catalogue_errors_are_reported() {
    for (bad, why) in [
        ("X mesh.subdivide", "unknown context"),
        ("E mesh.nope", "not a Blender 5.2.4 operator"),
        ("E mesh.subdivide; nope", "no property"),
        ("E mesh.subdivide; ngon=1:2", "a range for a BOOLEAN"),
        (
            "E mesh.extrude_region_move; TRANSFORM_OT_translate",
            "needs literals",
        ),
    ] {
        let e = catalog::parse_op(bad).unwrap_err();
        assert!(e.0.contains(why), "{bad}: {e}");
    }
    let e = catalog::parse_op("E mesh.subdivide; number_cuts=1:2; quadcorner; seed").unwrap();
    assert_eq!(e.props[0].kind, Kind::Int { lo: 1, hi: 2, n: 0 });
    assert!(matches!(&e.props[1].kind, Kind::Enum(items) if items.len() == 4));
    // Unlisted ranges come from the soft range, clamped.
    assert_eq!(e.props[2].kind, Kind::Int { lo: 0, hi: 5, n: 0 });
    let e = catalog::parse_op("E mesh.select_linked; !delimit").unwrap();
    assert!(e.props[0].always);
    assert!(
        matches!(&e.props[0].kind, Kind::EnumFlag(items) if items.contains(&"SEAM".to_owned()))
    );
    assert!(OPERATORS.len() + RNA.len() > 190);
}

/// Every catalogue operator is implemented in frap: `bpy.ops.<id>.poll()` answers (in the
/// factory scene a poll may be False, but never `OP_NOT_IMPLEMENTED` or an unknown name).
#[test]
fn catalogue_operators_are_implemented_in_frap() {
    let mut s = Session::new(SessionOptions::default()).expect("factory session");
    // The check sees unimplemented operators (`object.voxel_remesh` is planned after M8).
    let control = s.exec_source("bpy.ops.object.voxel_remesh.poll()");
    assert!(
        control.iter().any(|r| r.error.is_some()),
        "poll() of an unimplemented operator answered {control:?}"
    );
    let mut missing = Vec::new();
    for id in Catalog::get().operator_ids() {
        let r = s.exec_source(&format!("bpy.ops.{id}.poll()"));
        if let Some(e) = r.iter().find_map(|r| r.error.as_ref()) {
            missing.push(format!("{id}: {} {}", e.code.0, e.message));
        }
    }
    assert!(
        missing.is_empty(),
        "not implemented:\n  {}",
        missing.join("\n  ")
    );
}

/// Generated statements are in the subset frap's parser accepts, cover the catalogue, and
/// are reproducible from the seed.
#[test]
fn generated_sequences_parse_and_cover_the_catalogue() {
    let mut seen = BTreeSet::new();
    for i in 0..400u64 {
        let st = generate(rng::Rng::sequence_seed(1, i), DEFAULT_LEN);
        assert_eq!(st, generate(rng::Rng::sequence_seed(1, i), DEFAULT_LEN));
        let src = st.join("\n");
        for r in frap_lang::parse_complete(&src) {
            if let Err(e) = r {
                panic!("trial {i}: {e:?}\n{src}");
            }
        }
        for s in &st {
            seen.insert(driver::callee(s));
        }
    }
    let ops: BTreeSet<&str> = Catalog::get().operator_ids().into_iter().collect();
    let missed: Vec<&&str> = ops.iter().filter(|o| !seen.contains(**o)).collect();
    // 400 trials of 14 statements draw every operator with near certainty.
    assert!(missed.len() <= 3, "never drawn: {missed:?}");
}

/// The committed fuzz cases still come out of the generator: a case named
/// `seed_<hex>.py` holds the statements of `replay(<hex>)`.
#[test]
fn committed_seed_cases_replay() {
    let repo = crate::blender::Repo::locate();
    let dir = repo.cases_dir().join("fuzz");
    let mut n = 0;
    for p in crate::case::discover(&dir, None) {
        let stem = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Some(hex) = stem.strip_prefix("seed_") else {
            continue;
        };
        let seed = u64::from_str_radix(hex, 16).expect("seed_<hex>.py");
        let text = std::fs::read_to_string(&p).unwrap();
        let body = case_statements(&text);
        assert_eq!(
            body,
            generate(seed, body.len()),
            "{} no longer matches the generator",
            p.display()
        );
        n += 1;
    }
    assert!(n >= 20, "{n} seed cases under tests/golden/cases/fuzz");
}

// --- regression inputs of the cargo-fuzz targets ---------------------------------------------

macro_rules! regress {
    ($name:literal) => {
        &include_bytes!(concat!("regress/", $name))[1..]
    };
}

/// Inputs that made `imbuf_decode` abort (panics in `ktx2`, allocations sized from headers):
/// now plain errors, without allocating what the header asks for.
#[test]
fn imbuf_crash_inputs_are_errors() {
    for (name, bytes) in [
        ("png_huge_header", regress!("imbuf_png_huge_header.bin")),
        ("png_huge_rows", regress!("imbuf_png_huge_rows.bin")),
        ("exr_huge_window", regress!("imbuf_exr_huge_window.bin")),
        ("exr_dwa_counter", regress!("imbuf_exr_dwa_counter.bin")),
        ("ktx2_dfd_dim", regress!("imbuf_ktx2_dfd_dim.bin")),
        ("ktx2_zstd_length", regress!("imbuf_ktx2_zstd_length.bin")),
        (
            "basis_size_overflow",
            regress!("imbuf_basis_size_overflow.bin"),
        ),
    ] {
        assert!(frap_imbuf::decode(bytes).is_err(), "{name} decoded");
        let _ = frap_imbuf::probe(bytes);
        let _ = frap_imbuf::decode_exr_channels(bytes);
    }
}

/// The DWA guard keeps real files: Blender 5.2.4's DWAA, DWAB and PIZ EXRs (37×21 RGBA
/// gradient, `regress/grad_*.exr`) decode.
#[test]
fn exr_guards_keep_real_files() {
    for name in ["dwaa", "dwab", "piz"] {
        let bytes: &[u8] = match name {
            "dwaa" => include_bytes!("regress/grad_dwaa.exr"),
            "dwab" => include_bytes!("regress/grad_dwab.exr"),
            _ => include_bytes!("regress/grad_piz.exr"),
        };
        let img = frap_imbuf::decode(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((img.width, img.height, img.channels), (37, 21, 4), "{name}");
        let frap_imbuf::PixelData::F32(px) = &img.pixels else {
            panic!("{name}: not float");
        };
        // Pixel (10, 0) of the top row: x / 37 in red, alpha 1 (DWA is lossy on colour).
        let i = 10 * 4;
        assert!((px[i] - 10.0 / 37.0).abs() < 0.02, "{name}: {}", px[i]);
        assert!((px[i + 3] - 1.0).abs() < 1e-3, "{name}");
    }
}
