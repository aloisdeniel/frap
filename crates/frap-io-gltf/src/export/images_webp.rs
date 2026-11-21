// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_texture_webp` export (WP 7.3; M7 W0 stub): `export_image_format='WEBP'`,
//! `export_image_add_webp`, `export_image_webp_fallback`, and WebP sources, as the add-on's
//! `blender/exp/material/{texture,image}.py` write them (the WebP image as the extension's
//! `source`, an optional PNG/JPEG fallback as the texture's own `source`). WP 7.3 wires it into
//! `images.rs` (which it owns in M7); until then WebP is written as PNG with a WARNING.
