// SPDX-License-Identifier: GPL-3.0-or-later
//! The primitive driver (ARCHITECTURE §17.2): runs the three extension-point slices of
//! [`super::ext`] in ascending `order` (ties broken by `name`, so the run order never depends
//! on link order). Later milestones add files to `extract/`, `writers/` and `post/`; this file
//! names none of them.
//!
//! Driver protocol, the add-on's `PrimitiveCreator` (blender/exp/primitive_extract.py):
//! 1. every writer's `key` contributes its per-corner words to one [`SplitKey`]
//!    (`create_dots_data_structure` + `populate_dots_data`);
//! 2. every extractor builds its primitives, deduplicating on `(vertex index, key)`
//!    (`primitive_split` + `fast_structured_np_unique`);
//! 3. every writer's `run` fills its attribute(s) of every primitive;
//! 4. every post-processor transforms each primitive just before its accessors are written
//!    ([`post_process`], called by the mesh gatherer with the open `BufferBuilder`).

use frap_gltf::BufferBuilder;

pub use super::convert::yup;
pub use super::ext::PrimitiveDraft;
use super::ext::{
    MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PRIMITIVE_EXTRACTORS, PRIMITIVE_POST_PROCESS,
    PrimitiveAttributeWriter, PrimitiveExtractor, PrimitivePostProcess, SplitKey,
};
use crate::export::ExportError;

/// `material_slot` of the single primitive written with `export_materials='NONE'` (the
/// add-on's material index `-1`): it never maps to a material.
pub const NO_MATERIAL_SLOT: usize = usize::MAX;

/// Extractors in run order.
pub fn extractors() -> Vec<&'static PrimitiveExtractor> {
    let mut v: Vec<_> = PRIMITIVE_EXTRACTORS.iter().collect();
    v.sort_by_key(|e| (e.order, e.name));
    v
}

/// Attribute writers in run order (also the order of their split-key columns).
pub fn writers() -> Vec<&'static PrimitiveAttributeWriter> {
    let mut v: Vec<_> = PRIMITIVE_ATTRIBUTE_WRITERS.iter().collect();
    v.sort_by_key(|w| (w.order, w.name));
    v
}

/// Post-processors in run order.
pub fn post_processors() -> Vec<&'static PrimitivePostProcess> {
    let mut v: Vec<_> = PRIMITIVE_POST_PROCESS.iter().collect();
    v.sort_by_key(|p| (p.order, p.name));
    v
}

/// Step 1: the split key of the mesh (every writer's column, in writer order).
pub fn split_key(cx: &MeshExportCx) -> Result<SplitKey, ExportError> {
    let mut key = SplitKey::new(cx.mesh.corners_num());
    for w in writers() {
        if let Some(f) = w.key {
            f(cx, &mut key)?;
        }
    }
    Ok(key)
}

/// Steps 1–3: the primitives of a mesh with every attribute written (glTF space), before
/// post-processing.
pub fn extract(cx: &MeshExportCx) -> Result<Vec<PrimitiveDraft>, ExportError> {
    let key = split_key(cx)?;
    let mut drafts = Vec::new();
    for e in extractors() {
        (e.run)(cx, &key, &mut drafts);
    }
    let writers = writers();
    for d in &mut drafts {
        for w in &writers {
            (w.run)(cx, &key, d)?;
        }
    }
    Ok(drafts)
}

/// Step 4: every post-processor, in order, on one primitive.
pub fn post_process(
    cx: &MeshExportCx,
    draft: &mut PrimitiveDraft,
    buffer: &mut BufferBuilder,
) -> Result<(), ExportError> {
    for p in post_processors() {
        (p.run)(cx, draft, buffer)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m1_slices_are_registered_in_order() {
        let names: Vec<_> = extractors().iter().map(|e| e.name).collect();
        assert!(names.contains(&"triangles"));
        let w: Vec<_> = writers().iter().map(|w| w.name).collect();
        let n = w.iter().position(|n| *n == "normal").unwrap();
        let t = w.iter().position(|n| *n == "texcoord").unwrap();
        // The add-on's dots: vertex_index, NORMAL, TEXCOORD_n, …
        assert!(n < t);
    }
}
