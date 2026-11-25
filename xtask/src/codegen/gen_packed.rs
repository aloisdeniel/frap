// SPDX-License-Identifier: GPL-3.0-or-later
//! Deflate-packed copies of the large data files the release binary embeds (decision log
//! BIN-1): the colour-management LUTs, the factory startup dump and the EEVEE lookup textures.
//! The sources stay readable and authoritative in the repository (the bake scripts write them,
//! the tests and goldens read them); the crates embed only the packed copy and inflate it on
//! first use. `--check` compares the inflated content, so a different flate2 backend is fine.

use std::path::Path;

use super::Files;
use super::gen_ops::deflate;
use super::model::Res;

/// Generated directories owned by this generator (besides `gen_plain::DIR`, which also holds
/// the packed startup dump).
pub const COLOR_DIR: &str = "crates/frap-color/src/generated";
/// See [`COLOR_DIR`].
pub const RENDER_DIR: &str = "crates/frap-render/src/generated";

/// `(source, packed output)`, both relative to the repository root.
pub const PACKED: &[(&str, &str)] = &[
    (
        "assets/luts/agx.cube",
        "crates/frap-color/src/generated/agx.cube.deflate",
    ),
    (
        "assets/luts/agx_base_p3.cube",
        "crates/frap-color/src/generated/agx_base_p3.cube.deflate",
    ),
    (
        "assets/luts/agx_base_rec2020.cube",
        "crates/frap-color/src/generated/agx_base_rec2020.cube.deflate",
    ),
    (
        "assets/luts/luminance_compensation_bt2020.cube",
        "crates/frap-color/src/generated/luminance_compensation_bt2020.cube.deflate",
    ),
    (
        "assets/luts/pbr_neutral.cube",
        "crates/frap-color/src/generated/pbr_neutral.cube.deflate",
    ),
    (
        "assets/luts/filmic_desat_33.cube",
        "crates/frap-color/src/generated/filmic_desat_33.cube.deflate",
    ),
    (
        "assets/luts/filmic_to_0_35_1_30.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_35_1_30.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_0_48_1_09.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_48_1_09.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_0_60_1_04.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_60_1_04.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_0_70_1_03.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_70_1_03.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_0_85_1_011.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_85_1_011.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_0_99_1_0075.lut1d",
        "crates/frap-color/src/generated/filmic_to_0_99_1_0075.lut1d.deflate",
    ),
    (
        "assets/luts/filmic_to_1_20_1_00.lut1d",
        "crates/frap-color/src/generated/filmic_to_1_20_1_00.lut1d.deflate",
    ),
    (
        "assets/luts/agx_false_color.lut1d",
        "crates/frap-color/src/generated/agx_false_color.lut1d.deflate",
    ),
    (
        "spec/blender-5.2/startup.json",
        "crates/frap-scene/src/generated/startup.json.deflate",
    ),
    (
        "crates/frap-render/src/eevee/pipeline/utility_tx.bin",
        "crates/frap-render/src/generated/utility_tx.bin.deflate",
    ),
    (
        "assets/brdf_lut.bin",
        "crates/frap-render/src/generated/brdf_lut.bin.deflate",
    ),
];

/// Packs every [`PACKED`] source.
pub fn generate(root: &Path, files: &mut Files) -> Res<()> {
    for (src, out) in PACKED {
        let data = std::fs::read(root.join(src)).map_err(|e| format!("{src}: {e}"))?;
        files.insert(root.join(out), deflate(&data));
    }
    Ok(())
}
