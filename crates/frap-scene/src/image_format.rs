// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.ImageFormatSettings` (`scene.render.image_settings`, `render.bake.image_settings`):
//! the `Rna`/`RnaMut` impls of the generated plain struct, hand-written for the setters with
//! side effects (M5+M6 stabilization; `impl_by_hand` in `xtask/src/codegen/gen_plain.rs`).
//!
//! Ported from blender/source/blender/blenkernel/intern/image_format.cc
//! (`BKE_image_format_set`, `BKE_image_format_media_type_set`, `BKE_imtype_valid_channels`,
//! `BKE_imtype_valid_depths`, `BKE_imtype_first_valid_depth`) and
//! blender/source/blender/makesrna/intern/rna_scene.cc (`rna_ImageFormatSettings_*_itemf`):
//!
//! - `file_format` offers the formats of the current `media_type` (`IMAGE`: the image formats;
//!   `MULTI_LAYER_IMAGE`: `OPEN_EXR_MULTILAYER`; `VIDEO`: `FFMPEG`); setting it makes
//!   `color_mode` and `color_depth` valid for the format (`BW` → `RGBA` → `RGB`, the deepest
//!   valid depth) and fixes `views_format`;
//! - `media_type` switches `file_format` to `PNG`, `OPEN_EXR_MULTILAYER` or `FFMPEG` when the
//!   current one is of another kind;
//! - `color_mode` offers `BW` (render output convention), `RGB` and, where the format has
//!   alpha, `RGBA`; `color_depth` offers the format's depths.
//!
//! Every refused value is Blender's `TypeError` listing the current items (verified on 5.2.4:
//! `CINEON` depths `('10')`, colour modes `('BW', 'RGB')`). frap's settings always belong to a
//! scene (Blender's `is_render`). The colour space update of `BKE_image_format_set`
//! (`BKE_image_format_update_color_space_for_type`) and the video codec checks are not made.

use frap_rna::{
    Field, PropDef, PyExc, Rna, RnaCollMutGuard, RnaError, RnaErrorKind, RnaMut, RnaMutGuard,
    SetOutcome, StructDef, Value,
};
use smol_str::SmolStr;

use crate::generated::plain::ImageFormatSettings;
use crate::generated::{not_a_struct, unstored_get, unstored_set};

/// `file_format` items of `media_type='IMAGE'` (`IMAGE_TYPE_ITEMS_IMAGE`, 5.2.4 order).
pub const IMAGE_FORMATS: [&str; 14] = [
    "AVIF",
    "JPEG",
    "OPEN_EXR",
    "PNG",
    "WEBP",
    "BMP",
    "CINEON",
    "DPX",
    "IRIS",
    "JPEG2000",
    "HDR",
    "TARGA",
    "TARGA_RAW",
    "TIFF",
];

/// The `file_format` items of a media type.
pub fn file_format_items(media_type: &str) -> &'static [&'static str] {
    match media_type {
        "MULTI_LAYER_IMAGE" => &["OPEN_EXR_MULTILAYER"],
        "VIDEO" => &["FFMPEG"],
        _ => &IMAGE_FORMATS,
    }
}

/// The media type of a file format (`BKE_imtype_is_multi_layer_image`, `_is_movie`).
fn media_of(format: &str) -> &'static str {
    match format {
        "OPEN_EXR_MULTILAYER" => "MULTI_LAYER_IMAGE",
        "FFMPEG" => "VIDEO",
        _ => "IMAGE",
    }
}

/// `BKE_imtype_valid_channels`: (`RGBA`, `BW`) support, besides `RGB`.
fn channels_of(format: &str) -> (bool, bool) {
    let rgba = matches!(
        format,
        "BMP"
            | "TARGA"
            | "TARGA_RAW"
            | "IRIS"
            | "PNG"
            | "TIFF"
            | "OPEN_EXR"
            | "OPEN_EXR_MULTILAYER"
            | "DDS"
            | "JPEG2000"
            | "DPX"
            | "WEBP"
            | "AVIF"
    );
    let bw = matches!(
        format,
        "BMP"
            | "PNG"
            | "JPEG"
            | "TARGA"
            | "TARGA_RAW"
            | "TIFF"
            | "IRIS"
            | "OPEN_EXR"
            | "JPEG2000"
            | "AVIF"
    );
    (rgba, bw)
}

/// `color_mode` items for render output (`rna_ImageFormatSettings_color_mode_itemf` with a
/// scene owner: `BW` always; `RGBA` where the format has alpha).
pub fn color_mode_items(format: &str) -> Vec<&'static str> {
    let (rgba, _) = channels_of(format);
    let mut v = vec!["BW", "RGB"];
    if rgba {
        v.push("RGBA");
    }
    v
}

/// `BKE_imtype_valid_depths`, as the `color_depth` items (shallowest first).
pub fn color_depth_items(format: &str) -> &'static [&'static str] {
    match format {
        "HDR" => &["32"],
        "TIFF" | "PNG" => &["8", "16"],
        "OPEN_EXR" | "OPEN_EXR_MULTILAYER" => &["16", "32"],
        "DPX" => &["8", "10", "12", "16"],
        "CINEON" => &["10"],
        "JPEG2000" => &["8", "12", "16"],
        "AVIF" => &["8", "10", "12"],
        _ => &["8"],
    }
}

fn enum_error(name: &str, items: &[&str]) -> RnaError {
    let listed = items
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    RnaError::new(
        RnaErrorKind::Enum,
        PyExc::TypeError,
        format!("bpy_struct: item.attr = val: enum \"{name}\" not found in ({listed})"),
    )
    .with_detail(
        "did_you_mean",
        frap_rna::error::suggest(name, items.iter().copied()),
    )
    .with_detail("valid", items.to_vec())
}

/// `BKE_image_format_set` on render output settings (`format` already valid).
pub fn set_format(s: &mut ImageFormatSettings, format: &str) {
    s.file_format = SmolStr::new(format);
    s.media_type = SmolStr::new_static(media_of(format));
    let (rgba, _) = channels_of(format);
    // A render's settings accept BW for every format (`is_render`).
    if s.color_mode == "RGBA" && !rgba {
        s.color_mode = SmolStr::new_static("RGB");
    }
    let depths = color_depth_items(format);
    if !depths.contains(&s.color_depth.as_str()) {
        // `BKE_imtype_first_valid_depth`: the deepest one.
        s.color_depth = SmolStr::new_static(depths[depths.len() - 1]);
    }
    if format == "OPEN_EXR_MULTILAYER" {
        if s.views_format == "STEREO_3D" {
            s.views_format = SmolStr::new_static("MULTIVIEW");
        }
    } else if format != "OPEN_EXR" && s.views_format == "MULTIVIEW" {
        s.views_format = SmolStr::new_static("INDIVIDUAL");
    }
}

/// `image_settings.file_format = name`.
pub fn set_file_format(s: &mut ImageFormatSettings, name: &str) -> Result<(), RnaError> {
    let items = file_format_items(&s.media_type);
    if !items.contains(&name) {
        return Err(enum_error(name, items));
    }
    set_format(s, name);
    Ok(())
}

/// `image_settings.media_type = name` (`BKE_image_format_media_type_set`).
pub fn set_media_type(s: &mut ImageFormatSettings, name: &str) {
    s.media_type = SmolStr::new(name);
    if media_of(&s.file_format) != name {
        match name {
            "MULTI_LAYER_IMAGE" => set_format(s, "OPEN_EXR_MULTILAYER"),
            "VIDEO" => set_format(s, "FFMPEG"),
            _ => set_format(s, "PNG"),
        }
    }
}

impl Rna for ImageFormatSettings {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::IMAGE_FORMAT_SETTINGS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match self.rna_get(prop) {
            Some(f) => Ok(f),
            None => unstored_get(self.srna(), prop),
        }
    }
}

impl RnaMut for ImageFormatSettings {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        if let Value::Enum(name) = &v {
            match prop.identifier {
                "file_format" => {
                    set_file_format(self, name)?;
                    return Ok(SetOutcome::Done);
                }
                "media_type" => {
                    set_media_type(self, name);
                    return Ok(SetOutcome::Done);
                }
                "color_mode" => {
                    let items = color_mode_items(&self.file_format);
                    if !items.contains(&name.as_str()) {
                        return Err(enum_error(name, &items));
                    }
                }
                "color_depth" => {
                    let items = color_depth_items(&self.file_format);
                    if !items.contains(&name.as_str()) {
                        return Err(enum_error(name, items));
                    }
                }
                _ => {}
            }
        }
        let srna = self.srna();
        match self.rna_set(prop, v) {
            Some(r) => r,
            None => unstored_set(srna, prop),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        self.rna_struct_mut(prop)
            .ok_or_else(|| not_a_struct(srna, prop))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(not_a_struct(self.srna(), prop))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_switches_follow_blender() {
        // The 5.2.4 sequence of `fmt_probe.py` (M5+M6 stabilization).
        let mut s = ImageFormatSettings::default();
        set_file_format(&mut s, "TIFF").unwrap();
        assert_eq!(
            (s.color_depth.as_str(), s.color_mode.as_str()),
            ("8", "RGBA")
        );
        set_file_format(&mut s, "CINEON").unwrap();
        assert_eq!(
            (s.color_depth.as_str(), s.color_mode.as_str()),
            ("10", "RGB")
        );
        set_file_format(&mut s, "JPEG2000").unwrap();
        assert_eq!(s.color_depth, "16");
        set_file_format(&mut s, "AVIF").unwrap();
        assert_eq!(s.color_depth, "12");
        let e = set_file_format(&mut s, "OPEN_EXR_MULTILAYER").unwrap_err();
        assert!(
            e.message.contains("not found in ('AVIF', 'JPEG',"),
            "{}",
            e.message
        );
        set_media_type(&mut s, "MULTI_LAYER_IMAGE");
        assert_eq!(
            (s.file_format.as_str(), s.color_depth.as_str()),
            ("OPEN_EXR_MULTILAYER", "32")
        );
        let e = set_file_format(&mut s, "PNG").unwrap_err();
        assert!(
            e.message.ends_with("not found in ('OPEN_EXR_MULTILAYER')"),
            "{}",
            e.message
        );
        set_media_type(&mut s, "IMAGE");
        assert_eq!(
            (s.file_format.as_str(), s.color_depth.as_str()),
            ("PNG", "16")
        );
        assert_eq!(color_mode_items("CINEON"), ["BW", "RGB"]);
        assert_eq!(color_depth_items("IRIS"), ["8"]);
    }
}
