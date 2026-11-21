// SPDX-License-Identifier: GPL-3.0-or-later
//! The spec-complete writer (WP 7.4; M7 W0 stub): `frap_compat='SPEC'` (drop the add-on's
//! quirks, e.g. `KHR_texture_transform` in `extensionsRequired`), `frap_interleave`,
//! UNSIGNED_BYTE indices, `frap_node_visibility` (`KHR_node_visibility` from `hide_render`) and
//! `frap_diffuse_transmission`; the output passes the Khronos validator.
//!
//! `write::finish` calls [`finish`] once, on the gathered document and buffer, before the
//! extension order is fixed and the buffer is finished. Until WP 7.4 it changes nothing (the
//! operator warns about the `frap_*` props as not supported).

use crate::export::ExportError;
use crate::export::gather::Gather;

/// Rewrites the gathered document for the spec-complete options. The stub changes nothing.
pub fn finish(_g: &mut Gather) -> Result<(), ExportError> {
    Ok(())
}
