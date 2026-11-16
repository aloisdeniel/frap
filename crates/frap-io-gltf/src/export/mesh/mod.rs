// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh → glTF primitives (§17.2). `primitives.rs` is the driver; extractors, attribute writers
//! and post-processors are one file each in `extract/`, `writers/`, `post/` (automod) and
//! register into the `ext.rs` slices. Later milestones add files, never edit `primitives.rs`.

pub mod ext;
pub mod primitives;

/// `PRIMITIVE_EXTRACTORS` entries (`triangles.rs` M1; `loose_edges.rs`, `loose_points.rs`,
/// `pointcloud.rs` M3).
pub mod extract {
    automod::dir!(pub "src/export/mesh/extract");
}
/// `PRIMITIVE_ATTRIBUTE_WRITERS` entries (`normal.rs`, `texcoord.rs` M1; `color.rs`,
/// `tangent.rs` M3; `skin.rs` M6).
pub mod writers {
    automod::dir!(pub "src/export/mesh/writers");
}
/// `PRIMITIVE_POST_PROCESS` entries (`morph_targets.rs` M6; `quantize.rs`, `meshopt.rs`,
/// `draco.rs` M7).
pub mod post {
    automod::dir!(pub "src/export/mesh/post");
}
