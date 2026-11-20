// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenEXR half/float (`exr` crate; WP 5.4).

use std::io::Cursor;

use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_exr(image: &DecodedImage, bit_depth: u8) -> Result<Vec<u8>, ImageError> {
    use exr::prelude::{
        AnyChannel, AnyChannels, Encoding, FlatSamples, Image, Layer, LayerAttributes,
        WritableImage, f16,
    };
    if bit_depth != 16 && bit_depth != 32 {
        return Err(ImageError::Unsupported(format!(
            "OpenEXR bit depth {bit_depth}"
        )));
    }
    let names: &[&str] = match image.channels {
        1 => &["Y"],
        2 => &["Y", "A"],
        3 => &["R", "G", "B"],
        _ => &["R", "G", "B", "A"],
    };
    let data = image.to_f32();
    let c = image.channels as usize;
    let list = names
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let plane = data.iter().skip(k).step_by(c).copied();
            let samples = if bit_depth == 16 {
                FlatSamples::F16(plane.map(f16::from_f32).collect())
            } else {
                FlatSamples::F32(plane.collect())
            };
            AnyChannel::new(*name, samples)
        })
        .collect();
    let layer = Layer::new(
        (image.width as usize, image.height as usize),
        LayerAttributes::default(),
        Encoding::SMALL_LOSSLESS,
        AnyChannels::sort(list),
    );
    let mut out = Cursor::new(Vec::new());
    Image::from_layer(layer)
        .write()
        .non_parallel()
        .to_buffered(&mut out)
        .map_err(|e| ImageError::Encode(format!("OpenEXR: {e}")))?;
    Ok(out.into_inner())
}
