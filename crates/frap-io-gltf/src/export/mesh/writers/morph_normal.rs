// SPDX-License-Identifier: GPL-3.0-or-later
//! Morph-normal split key (WP 6.8, `export_morph_normal`): the `MORPH_NORMAL_n` fields of the
//! add-on's `dots`, so two corners of a vertex that share their base normal but not a shape
//! key's normal become two glTF vertices. Three words per exported key and corner (the delta
//! of `post/morph_targets.rs`, glTF space). The targets themselves are written by the
//! `morph_targets` post-processor; this writer adds no attribute.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/primitive_extract.py` (`define_attributes`:
//! `MORPH_NORMAL_x` after `TANGENT`; `__get_normal_attribute` fills them; Apache-2.0, The
//! glTF-Blender-IO authors).

use crate::export::ExportError;
use crate::export::mesh::convert::key_word;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter, PrimitiveDraft, SplitKey,
};
use crate::export::mesh::post::morph_targets::morph_normal_deltas;

/// This writer's split-key column.
pub const NAME: &str = "morph_normal";

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    let Some(deltas) = morph_normal_deltas(cx) else {
        return Ok(());
    };
    if deltas.is_empty() {
        return Ok(());
    }
    let n = deltas.len();
    let corners = cx.mesh.corners_num();
    let mut words = Vec::with_capacity(3 * n * corners);
    for c in 0..corners {
        for d in &deltas {
            words.extend(d[c].map(key_word));
        }
    }
    k.push(NAME, 3 * n, words);
    Ok(())
}

fn run(_: &MeshExportCx, _: &SplitKey, _: &mut PrimitiveDraft) -> Result<(), ExportError> {
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static MORPH_NORMAL: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 320,
    key: Some(key),
    run,
};
