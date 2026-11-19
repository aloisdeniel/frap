// SPDX-License-Identifier: GPL-3.0-or-later
//! Output formats (M3, WP 3.7): `GLB` (JSON chunk + BIN chunk, no BIN chunk without binary
//! data), `GLTF_SEPARATE` (`<stem>.gltf`, `<stem>.bin` and the texture files, the texture
//! folder from `export_texture_dir`) and `GLTF_EMBEDDED` (one `.gltf` with the buffer and the
//! images as base64 data URIs; offered by the operator only with the add-on preference
//! `allow_embedded_format`).
//!
//! The serialization is the last step of the export and lives in `write.rs` (`finish`), the
//! image placement (file next to the `.gltf`, buffer view, data URI) in `images.rs`. WP 3.7
//! verified the three formats against Blender 5.2.4: `tests/golden/cases/gltf/attributes/
//! formats.py` (GLB and GLTF_EMBEDDED, with and without the `.gltf` extension, with a texture),
//! `gltf/m1/gltf_separate.py` and `gltf/m1/glb_extension.py`.

pub use crate::export::ExportFormat;

/// The file extension the add-on gives an export of `format` (`.glb` / `.gltf`).
pub fn extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Glb => ".glb",
        ExportFormat::GltfSeparate | ExportFormat::GltfEmbedded => ".gltf",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions() {
        assert_eq!(extension(ExportFormat::Glb), ".glb");
        assert_eq!(extension(ExportFormat::GltfSeparate), ".gltf");
        assert_eq!(extension(ExportFormat::GltfEmbedded), ".gltf");
    }
}
