// SPDX-License-Identifier: GPL-3.0-or-later
//! The `image` crate's 8-bit channel layouts, for the encoders that go through it (WebP; WP 5.4).

pub(crate) fn color_type_8(channels: u8) -> image::ExtendedColorType {
    match channels {
        1 => image::ExtendedColorType::L8,
        2 => image::ExtendedColorType::La8,
        3 => image::ExtendedColorType::Rgb8,
        _ => image::ExtendedColorType::Rgba8,
    }
}
