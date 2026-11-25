// SPDX-License-Identifier: GPL-3.0-or-later
//! The split-sum BxDF look-up tables (`assets/brdf_lut.bin`, WP 5.2).
//!
//! Ported from blender/source/blender/draw/engines/eevee/eevee_lut.cc (tables `brdf_ggx`,
//! `bsdf_ggx`, `btdf_ggx`) and laid out like the BRDF/BSDF layers of EEVEE's `UtilityTexture`
//! (blender/source/blender/draw/engines/eevee/eevee_pipeline.hh, `UTIL_BRDF_LAYER` and the 16
//! `UTIL_BSDF_LAYER`s), which Blender uploads as `SFLOAT_16_16_16_16`:
//!
//! | Layer | R | G | B | A |
//! |---|---|---|---|---|
//! | 0 (`brdf_ggx`) | `scale` | `bias` | `metal_bias` | 0 |
//! | 1 + i (`bsdf_ggx[i]`, `btdf_ggx[i]`), i < 16 | `scale` | `bias` | `transmission_factor` | `btdf` transmission factor (IOR > 1) |
//!
//! Each layer is 64 × 64; texel `(x, y)` holds `table[y][x]` (Blender's upload order). The file
//! is `LAYERS × 64 × 64 × 4` little-endian IEEE half floats, rounded to nearest from the
//! tables' `f32` values as the GPU upload does. Sampling (`eevee_bxdf_lut_lib.bsl.hh`,
//! `eevee_utility_tx.bsl.hh`): BRDF coordinates are `(roughness, sqrt(1 - cos_theta))`, scaled
//! by `63/64` and biased by `0.5/64` for a filtered lookup; the BSDF layers add the IOR axis
//! with a manual linear interpolation between layers.
//!
//! `cargo test -p frap-render lut -- --nocapture` with `FRAP_BLENDER_SRC` (a Blender 5.2.4
//! source tree) checks the asset against `eevee_lut.cc`; `FRAP_IBL_REGEN_LUT=1` rewrites it.

use half::f16;

/// Side of each LUT layer (`UTIL_TEX_SIZE`).
pub const SIZE: usize = 64;
/// Number of BSDF layers (`UTIL_BSDF_LAYER_COUNT`).
pub const BSDF_LAYERS: usize = 16;
/// Layers in the asset: the BRDF layer, then the BSDF layers.
pub const LAYERS: usize = 1 + BSDF_LAYERS;

/// Bytes of the asset.
pub const ASSET_LEN: usize = LAYERS * SIZE * SIZE * 4 * 2;

/// The asset (`assets/brdf_lut.bin`), deflate-packed by `cargo xtask codegen` (decision log
/// BIN-1).
static ASSET_DEFLATE: &[u8] = include_bytes!("../../generated/brdf_lut.bin.deflate");

/// The asset (`assets/brdf_lut.bin`), inflated on first use.
pub fn asset() -> &'static [u8] {
    static ASSET: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    ASSET.get_or_init(|| crate::eevee::unpack(ASSET_DEFLATE))
}

/// The decoded tables.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut {
    /// `LAYERS × SIZE × SIZE` RGBA texels (`[layer][y][x]`), as the GPU sees them (half
    /// precision, widened to `f32`).
    pub texels: Vec<[f32; 4]>,
}

/// The data of one `GGXBrdfData` lookup: `integral = F0 * scale + F90 * bias - F82_tint *
/// metal_bias`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrdfData {
    /// `scale`
    pub scale: f32,
    /// `bias`
    pub bias: f32,
    /// `metal_bias`
    pub metal_bias: f32,
}

impl Lut {
    /// Decodes [`asset`].
    pub fn load() -> Lut {
        Lut::from_bytes(asset())
    }

    /// Decodes an asset image (see the module documentation).
    pub fn from_bytes(bytes: &[u8]) -> Lut {
        assert_eq!(bytes.len(), ASSET_LEN, "brdf_lut.bin: wrong size");
        let texels = bytes
            .chunks_exact(8)
            .map(|t| {
                let c = |i: usize| f16::from_le_bytes([t[2 * i], t[2 * i + 1]]).to_f32();
                [c(0), c(1), c(2), c(3)]
            })
            .collect();
        Lut { texels }
    }

    /// Texel `(x, y)` of `layer`.
    pub fn texel(&self, layer: usize, x: usize, y: usize) -> [f32; 4] {
        self.texels[(layer * SIZE + y) * SIZE + x]
    }

    /// Bilinear lookup of `layer` at `uv` (texture coordinates in `[0, 1]`, clamp to edge), as
    /// `UtilityTexture::sample_lut` does after its scale and bias.
    pub fn sample(&self, layer: usize, uv: [f32; 2]) -> [f32; 4] {
        let s = SIZE as f32;
        let fx = (uv[0] * s - 0.5).clamp(0.0, s - 1.0);
        let fy = (uv[1] * s - 0.5).clamp(0.0, s - 1.0);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(SIZE - 1), (y0 + 1).min(SIZE - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let (a, b, c, d) = (
            self.texel(layer, x0, y0),
            self.texel(layer, x1, y0),
            self.texel(layer, x0, y1),
            self.texel(layer, x1, y1),
        );
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bot = c[i] + (d[i] - c[i]) * tx;
            top + (bot - top) * ty
        })
    }

    /// `GGXBrdfData::sample_utility_tx(cos_theta, roughness)`.
    pub fn brdf(&self, cos_theta: f32, roughness: f32) -> BrdfData {
        let uv = [roughness, (1.0 - cos_theta).clamp(0.0, 1.0).sqrt()];
        let t = self.sample(0, scale_bias(uv));
        BrdfData {
            scale: t[0],
            bias: t[1],
            metal_bias: t[2],
        }
    }

    /// `GGXBtdfGt1Data::sample_utility_tx(cos_theta, roughness, f0)`: the transmission factor
    /// for IOR > 1 (`sample_bsdf_lut`'s manual interpolation across the roughness layers).
    pub fn btdf_gt1(&self, cos_theta: f32, roughness: f32, f0: f32) -> f32 {
        let uv = scale_bias([f0.max(0.0).sqrt(), (1.0 - cos_theta).max(0.0).sqrt()]);
        self.sample_bsdf(uv, roughness.clamp(0.0, 1.0))[3]
    }

    /// `UtilityTexture::sample_bsdf_lut(uv, layer)` (`uv` already scaled and biased).
    pub fn sample_bsdf(&self, uv: [f32; 2], layer: f32) -> [f32; 4] {
        let l = layer * BSDF_LAYERS as f32;
        let lf = l.floor();
        let interp = l - lf;
        let lo = (lf as usize).min(BSDF_LAYERS - 1);
        // `layer_floored + 1` reads past the BSDF layers at roughness 1 in Blender (the next
        // utility layer); its weight is then 0, so clamping is equivalent.
        let hi = (lo + 1).min(BSDF_LAYERS - 1);
        let a = self.sample(1 + lo, uv);
        let b = self.sample(1 + hi, uv);
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * interp)
    }
}

/// `uv * UTIL_TEX_UV_SCALE + UTIL_TEX_UV_BIAS`.
pub fn scale_bias(uv: [f32; 2]) -> [f32; 2] {
    let s = SIZE as f32;
    uv.map(|c| c * ((s - 1.0) / s) + 0.5 / s)
}

/// Parses one `const float <name>[..] = { ... };` table of `eevee_lut.cc` (`count` values).
pub fn parse_table(source: &str, name: &str, count: usize) -> Result<Vec<f32>, String> {
    let head = format!("const float {name}[");
    let start = source
        .find(&head)
        .ok_or_else(|| format!("eevee_lut.cc: no table {name}"))?;
    let body = &source[start..];
    let open = body.find('{').ok_or("no table body")?;
    let mut out = Vec::with_capacity(count);
    let mut cur = String::new();
    for ch in body[open..].chars() {
        match ch {
            '0'..='9' | '.' | '-' | 'e' | '+' => cur.push(ch),
            _ => {
                if ch == 'f' && !cur.is_empty() {
                    out.push(
                        cur.parse::<f32>()
                            .map_err(|e| format!("{name}: {cur:?}: {e}"))?,
                    );
                }
                cur.clear();
                if ch == ';' {
                    break;
                }
            }
        }
    }
    if out.len() != count {
        return Err(format!("{name}: {} values, expected {count}", out.len()));
    }
    Ok(out)
}

/// Builds the asset bytes from `eevee_lut.cc`'s source text.
pub fn asset_from_blender_source(source: &str) -> Result<Vec<u8>, String> {
    let n = SIZE * SIZE;
    let brdf = parse_table(source, "brdf_ggx", n * 3)?;
    let bsdf = parse_table(source, "bsdf_ggx", BSDF_LAYERS * n * 3)?;
    let btdf = parse_table(source, "btdf_ggx", BSDF_LAYERS * n)?;
    let mut out = Vec::with_capacity(ASSET_LEN);
    let mut push = |v: f32| out.extend_from_slice(&f16::from_f32(v).to_le_bytes());
    for i in 0..n {
        push(brdf[3 * i]);
        push(brdf[3 * i + 1]);
        push(brdf[3 * i + 2]);
        push(0.0);
    }
    for l in 0..BSDF_LAYERS {
        for i in 0..n {
            let j = l * n + i;
            push(bsdf[3 * j]);
            push(bsdf[3 * j + 1]);
            push(bsdf[3 * j + 2]);
            push(btdf[j]);
        }
    }
    Ok(out)
}

/// Uploads the tables as one `Rgba16Float` 2D-array texture of [`LAYERS`] layers.
pub(crate) fn upload(gpu: &crate::gpu::Gpu) -> wgpu::Texture {
    use wgpu::util::DeviceExt;
    gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("frap eevee bxdf lut"),
            size: wgpu::Extent3d {
                width: SIZE as u32,
                height: SIZE as u32,
                depth_or_array_layers: LAYERS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        asset(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_shape_and_known_texels() {
        let lut = Lut::load();
        assert_eq!(lut.texels.len(), LAYERS * SIZE * SIZE);
        // brdf_ggx[0][0] = {0.999996, 0, 0} (roughness 0, normal incidence: scale 1).
        let t = lut.texel(0, 0, 0);
        assert!(
            (t[0] - 1.0).abs() < 1e-3 && t[1] == 0.0 && t[3] == 0.0,
            "{t:?}"
        );
        // bsdf_ggx[0][0][0] = {0, 0.999996, 0}, btdf_ggx[0][0][0] = 0.999996.
        let t = lut.texel(1, 0, 0);
        assert!(t[0] == 0.0 && (t[1] - 1.0).abs() < 1e-3 && (t[3] - 1.0).abs() < 1e-3);
        // The split sum stays within 1 (single-scatter GGX) up to the fit error of Blender's table
        // near roughness 0 at grazing angles (1.003).
        for y in 0..SIZE {
            for x in 0..SIZE {
                let t = lut.texel(0, x, y);
                assert!(t[0] + t[1] <= 1.005, "({x}, {y}): {t:?}");
            }
        }
        // Grazing, rough: much lower directional albedo than at normal incidence.
        let b = lut.brdf(0.05, 0.9);
        let n = lut.brdf(1.0, 0.0);
        assert!(b.scale + b.bias < n.scale + n.bias);
    }

    #[test]
    fn asset_matches_blender_source() {
        let Ok(root) = std::env::var("FRAP_BLENDER_SRC") else {
            eprintln!(
                "SKIP: FRAP_BLENDER_SRC not set (a Blender 5.2.4 source tree to check \
                 assets/brdf_lut.bin against eevee_lut.cc)"
            );
            return;
        };
        let p = std::path::Path::new(&root).join("source/blender/draw/engines/eevee/eevee_lut.cc");
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let bytes = asset_from_blender_source(&src).unwrap();
        if std::env::var("FRAP_IBL_REGEN_LUT").is_ok_and(|v| v == "1") {
            let out =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/brdf_lut.bin");
            std::fs::write(&out, &bytes).unwrap();
            eprintln!("wrote {}", out.display());
            return;
        }
        assert!(
            bytes == asset(),
            "assets/brdf_lut.bin differs from {}",
            p.display()
        );
    }

    #[test]
    fn parser_reads_nested_tables() {
        let src = "const float a[2][1] = {{1.5f}, {-2e-3f}};\nconst float b[1] = {{0.25f}};";
        assert_eq!(parse_table(src, "a", 2).unwrap(), vec![1.5, -0.002]);
        assert_eq!(parse_table(src, "b", 1).unwrap(), vec![0.25]);
        assert!(parse_table(src, "a", 3).is_err());
        assert!(parse_table(src, "c", 1).is_err());
    }
}
