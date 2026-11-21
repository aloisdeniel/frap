// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_texture_basisu` export (WP 7.3): `frap_texture_format='KTX2'` and `frap_basisu_mode`
//! (frap extensions; the add-on has no KTX2 export).
//!
//! Every texture gets a KTX2 image as its `KHR_texture_basisu` `source`, made from the same
//! channels as its other images ([`images::encode`]): the file's bytes when the image is an
//! unmodified KTX2 file (one imported from a `KHR_texture_basisu` glTF), else the pixels
//! encoded with Basis Universal (`frap_imbuf::basis`, feature `basisu-encode`): ETC1S or UASTC
//! (`frap_basisu_mode`), the encoder's unified quality from `export_image_quality`, a full mip
//! chain wrapped at the borders unless both sampler wraps clamp, and the sRGB transfer
//! function for colour textures (base colour, emission, sheen and specular colour) and linear
//! for data (normal, metallic-roughness, occlusion, the other extension textures).
//!
//! Every KTX2 image written keeps the rules of the extension's specification that the
//! JSON cannot show: `pixelWidth` and `pixelHeight` are multiples of 4 (an image of another
//! size is resampled to the next multiple of 4 in each dimension before encoding, so a 5x3
//! image is written 8x4 and a 1x1 image 4x4; frap does not round to a power of two as
//! `toktx --resize` or `gltfpack -tp` can, see deviation D-130), colour data has the BT709
//! primaries and the sRGB transfer function, non-colour data the primaries
//! `KHR_DF_PRIMARIES_UNSPECIFIED` and the linear transfer function (`frap_imbuf::basis`). A
//! KTX2 file is copied as it is only when it keeps these rules for the texture's colour
//! space (`frap_gltf::ext::khr_texture_basisu::ktx2_issues`); another is re-encoded.
//!
//! The texture keeps the images `export_image_format` and the WebP options give it, so the
//! file stays readable by viewers without KTX2 support — Blender among them, which cannot load
//! KTX2 (deviation D-130) — and the extension is required only when the texture has no core
//! `source` (`export_image_format='WEBP'`, or a WebP image without fallback), as the
//! extension's specification demands. A build without the encoder answers NOT_IMPLEMENTED
//! for a texture whose image must be encoded.

use frap_imbuf::basis::{BasisMode, BasisOptions};
use frap_material::{TexRef, Wrap};
use frap_spec_ops_io::export_scene::gltf::{FrapBasisuMode, FrapTextureFormat};

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{self, ExportImage, KTX2, PendingImage, Target, TextureSlot};

/// The props this file implements.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["frap_texture_format", "frap_basisu_mode"];

/// Whether a texture of `slot` holds colour (sRGB) rather than data.
pub fn is_colour(slot: TextureSlot) -> bool {
    matches!(
        slot,
        TextureSlot::BaseColor
            | TextureSlot::Emissive
            | TextureSlot::Ext("sheenColorTexture" | "specularColorTexture")
    )
}

/// The Basis Universal settings of a texture of `slot` sampled as `tex`.
pub fn options(g: &Gather, slot: TextureSlot, tex: &TexRef) -> BasisOptions {
    let mode = match g.settings.props.frap_basisu_mode.value {
        FrapBasisuMode::Uastc => BasisMode::Uastc,
        _ => BasisMode::Etc1s,
    };
    let clamp = |w: Wrap| w == Wrap::Clamp;
    BasisOptions::new(mode)
        .with_srgb(is_colour(slot))
        .with_wrap(!(clamp(tex.sampler.wrap_s) && clamp(tex.sampler.wrap_t)))
        .with_quality(g.settings.export_image_quality)
}

/// The `KHR_texture_basisu` image of a texture gathered from `ex` for `slot` (`None` unless
/// `frap_texture_format='KTX2'`, or with a WARNING when the image cannot be written).
pub(crate) fn gather(
    g: &mut Gather,
    ex: &ExportImage,
    slot: TextureSlot,
    tex: &TexRef,
    name: String,
) -> Result<Option<PendingImage>, ExportError> {
    if g.settings.props.frap_texture_format.value != FrapTextureFormat::Ktx2 {
        return Ok(None);
    }
    let target = Target::Ktx2(options(g, slot, tex));
    let Some(bytes) = images::encode(g, ex, target)? else {
        g.warn(format!(
            "image '{name}': Image data is empty, not exporting image"
        ));
        return Ok(None);
    };
    Ok(Some(images::make_image(g, bytes, KTX2, name)))
}
