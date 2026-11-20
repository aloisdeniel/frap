// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenEXR half/float (`exr` crate; WP 5.4), with the rules of Blender 5.2.4's writers
//! (`imbuf/intern/openexr/openexr_api.cpp`, `blenkernel/intern/image_save.cc`,
//! GPL-2.0-or-later):
//!
//! - one scanline part, increasing Y, the codec of `image_settings.exr_codec` (ZIP by default;
//!   DWAA/DWAB at `openexr_jpg_like_quality_to_dwa_compression_level(quality)`);
//! - channels `R`, `G`, `B` (+ `A`); a grey image is one channel (`V` in render output); a
//!   multilayer file (`OPEN_EXR_MULTILAYER`, `WriteOptions::exr_layer`) names them after the
//!   pass, `ViewLayer.Combined.R`…, with the part name and Blender's `BlenderMultiChannel`
//!   attribute;
//! - half samples: NaN → 0, clamped to ±`HALF_MAX` (less a margin for DWA, so its overshoot
//!   stays finite: `compression_half_max`), then rounded to nearest even
//!   (`float_to_half_clamp_array`); float samples are written as they are;
//! - `xDensity` (pixels per inch) and `pixelAspectRatio` from the render's pixels per metre
//!   (`openexr_header_metadata_pixelinfo`) and the `colorInteropID` of the colour space.
//!
//! The compressed bytes come from the `exr` crate, not OpenEXR: files equal Blender's by
//! samples. Blender's `Software` attribute and stamp metadata are not written.

use std::io::Cursor;

use super::{ExrCodec, WriteOptions};
use crate::codec::{DecodedImage, ImageError};

/// `HALF_MAX`.
const HALF_MAX: f32 = 65504.0;

/// `openexr_jpg_like_quality_to_dwa_compression_level`: JPEG quality 90 → DWA level 45, 100 →
/// 0, linear in between (integer arithmetic).
pub(crate) fn dwa_level(quality: u8) -> i32 {
    let q = i32::from(quality).clamp(0, 100);
    (q - 100) * 45 / (90 - 100)
}

/// `compression_half_max`.
fn half_max(codec: ExrCodec) -> f32 {
    match codec {
        ExrCodec::Dwaa(q) | ExrCodec::Dwab(q) => {
            let level = dwa_level(q);
            let margin = if level <= 50 {
                2048.0 + 32.0 * level as f32
            } else {
                3624.0 + 128.0 * (level - 50) as f32
            };
            HALF_MAX - margin
        }
        _ => HALF_MAX,
    }
}

/// `float_to_half_clamp_array` for one sample.
fn to_half(v: f32, max: f32) -> half::f16 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(-max, max) };
    half::f16::from_f32(v)
}

pub(crate) fn encode_exr(
    image: &DecodedImage,
    bit_depth: u8,
    options: &WriteOptions,
) -> Result<Vec<u8>, ImageError> {
    use exr::prelude::{
        AnyChannel, AnyChannels, AttributeValue, Compression, Encoding, FlatSamples, Image,
        ImageAttributes, IntegerBounds, Layer, LayerAttributes, LineOrder, Text, WritableImage,
    };
    if bit_depth != 16 && bit_depth != 32 {
        return Err(ImageError::Unsupported(format!(
            "OpenEXR bit depth {bit_depth}"
        )));
    }
    let grey = options.exr_grey_channel;
    let names: &[&str] = match image.channels {
        1 => &[grey],
        2 => &[grey, "A"],
        3 => &["R", "G", "B"],
        _ => &["R", "G", "B", "A"],
    };
    let codec = options.exr_codec;
    let compression = match codec {
        ExrCodec::None => Compression::Uncompressed,
        ExrCodec::Zip => Compression::ZIP16,
        ExrCodec::Zips => Compression::ZIP1,
        ExrCodec::Piz => Compression::PIZ,
        ExrCodec::Rle => Compression::RLE,
        ExrCodec::Pxr24 => Compression::PXR24,
        ExrCodec::B44 => Compression::B44,
        ExrCodec::B44a => Compression::B44A,
        ExrCodec::Dwaa(q) => Compression::DWAA(Some(dwa_level(q) as f32)),
        ExrCodec::Dwab(q) => Compression::DWAB(Some(dwa_level(q) as f32)),
    };
    let max = half_max(codec);
    let data = image.to_f32();
    let c = image.channels as usize;
    let list = names
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let plane = data.iter().skip(k).step_by(c).copied();
            let samples = if bit_depth == 16 {
                FlatSamples::F16(plane.map(|v| to_half(v, max)).collect())
            } else {
                FlatSamples::F32(plane.collect())
            };
            match &options.exr_layer {
                Some(layer) => AnyChannel::new(format!("{layer}.{name}").as_str(), samples),
                None => AnyChannel::new(*name, samples),
            }
        })
        .collect();
    let size = (image.width as usize, image.height as usize);
    let mut layer_attributes = LayerAttributes::default();
    let mut image_attributes = ImageAttributes::new(IntegerBounds::from_dimensions(size));
    if let Some([x, y]) = options.ppm.filter(|p| p[0] > 0.0 && p[1] > 0.0) {
        layer_attributes.horizontal_density = Some((x * 0.0254) as f32);
        image_attributes.pixel_aspect = (y / x) as f32;
    }
    if let Some(id) = options.exr_interop_id {
        layer_attributes.other.insert(
            Text::from("colorInteropID"),
            AttributeValue::Text(Text::from(id)),
        );
    }
    if let Some(layer) = &options.exr_layer {
        // `IMB_exr_begin_write` of a multilayer file: the part name and Blender's marker.
        layer_attributes.layer_name = Some(Text::from(layer.as_str()));
        layer_attributes.other.insert(
            Text::from("BlenderMultiChannel"),
            AttributeValue::Text(Text::from("Blender V2.55.1 and newer")),
        );
    }
    let layer = Layer::new(
        size,
        layer_attributes,
        Encoding {
            compression,
            blocks: exr::prelude::Blocks::ScanLines,
            line_order: LineOrder::Increasing,
        },
        AnyChannels::sort(list),
    );
    let mut out = Cursor::new(Vec::new());
    Image::new(image_attributes, layer)
        .write()
        .non_parallel()
        .to_buffered(&mut out)
        .map_err(|e| ImageError::Encode(format!("OpenEXR: {e}")))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dwa_levels_and_half_clamp() {
        assert_eq!(dwa_level(90), 45);
        assert_eq!(dwa_level(100), 0);
        assert_eq!(dwa_level(0), 450);
        assert_eq!(half_max(ExrCodec::Zip), 65504.0);
        assert_eq!(
            half_max(ExrCodec::Dwaa(90)),
            65504.0 - (2048.0 + 32.0 * 45.0)
        );
        assert_eq!(to_half(f32::NAN, HALF_MAX).to_f32(), 0.0);
        assert_eq!(to_half(1e9, HALF_MAX).to_f32(), 65504.0);
        assert_eq!(to_half(-1e9, HALF_MAX).to_f32(), -65504.0);
        assert_eq!(to_half(f32::INFINITY, HALF_MAX).to_f32(), 65504.0);
    }
}
