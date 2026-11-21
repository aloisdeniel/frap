// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_use_gltfpack` (WP 7.1; M7 W0 stub): the add-on runs the external `gltfpack` binary
//! on the written file (`blender/exp/export.py::__postprocess_with_gltfpack`, options from the
//! `export_gltfpack_*` props); frap maps those props onto in-process meshopt and quantization
//! with gltfpack-equivalent semantics and an INFO report (§17.2), no external binary.
//! `export()` calls [`apply`] on the finished output.
//!
//! Until WP 7.1 the output is returned unchanged (the operator warns about the props).

use crate::export::{ExportError, ExportOutput, ExportSettings};

/// Post-processes the written output as gltfpack would. The stub returns it unchanged.
pub fn apply(_settings: &ExportSettings, out: ExportOutput) -> Result<ExportOutput, ExportError> {
    Ok(out)
}
