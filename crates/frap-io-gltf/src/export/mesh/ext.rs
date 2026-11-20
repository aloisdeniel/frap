// SPDX-License-Identifier: GPL-3.0-or-later
//! Export extension points (ARCHITECTURE §17.2, CONTRACT frozen at M1 W0). The driver
//! (`primitives.rs`) runs every slice in ascending `order`; later milestones add files, never
//! edit the driver.
//!
//! Driver protocol (the add-on's `PrimitiveCreator`, `primitive_extract.py`):
//! 1. **Split key.** Every attribute writer with a `key` contributes per-corner words to one
//!    [`SplitKey`] (the add-on's structured `dots` array: vertex index, then normals, tangents,
//!    UVs, colours, morph normals, in writer `order`).
//! 2. **Extract.** Extractors build primitives, deduplicating corners on
//!    `(vertex index, every key column)` — so a writer added later (COLOR_0, TANGENT) splits
//!    vertices exactly as the add-on does, without editing an extractor — and fill
//!    [`PrimitiveDraft::corners`] with one representative Blender corner per glTF vertex.
//! 3. **Write.** Writers fill their attribute(s) from the representative corners (or the
//!    vertex index for per-vertex data such as JOINTS/WEIGHTS, which take no key).
//! 4. **Post-process** (morph targets, quantization, meshopt, Draco).

use frap_gltf::BufferBuilder;
use frap_mesh::Mesh;
use frap_rna::{IdRef, ObjectId};
use frap_scene::Main;
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::export::{ExportError, ExportSettings};

/// What extractors and writers see for one mesh. `#[non_exhaustive]` and built only by the
/// driver ([`MeshExportCx::new`]), so later milestones add fields without an RFC (decision
/// W0-28: e.g. the evaluated armature for M6 skinning).
#[non_exhaustive]
pub struct MeshExportCx<'a> {
    /// `bpy.data` (materials, vertex groups, shape keys).
    pub main: &'a Main,
    /// The (evaluated, when `export_apply`) mesh.
    pub mesh: &'a Mesh,
    /// The object the mesh is exported for: its vertex groups and armature (JOINTS/WEIGHTS,
    /// M6) and object-linked material slots. `None` when one glTF mesh is shared by several
    /// objects and no per-object data applies (the add-on's mesh cache key, §17.2).
    pub object: Option<IdRef<ObjectId>>,
    /// Export settings.
    pub settings: &'a ExportSettings,
    /// The skin of `object`'s mesh (M6, WP 6.8: joints, vertex groups, armature-space
    /// transforms; `export/skins.rs`), `None` when the mesh is not skinned.
    pub skin: Option<&'a crate::export::skins::MeshSkin>,
}

impl<'a> MeshExportCx<'a> {
    /// Context for exporting `mesh` (for `object`, when per-object data applies).
    pub fn new(
        main: &'a Main,
        mesh: &'a Mesh,
        object: Option<IdRef<ObjectId>>,
        settings: &'a ExportSettings,
    ) -> Self {
        MeshExportCx {
            main,
            mesh,
            object,
            settings,
            skin: None,
        }
    }

    /// The same context with the mesh skinned by `skin` (WP 6.8).
    pub fn with_skin(mut self, skin: Option<&'a crate::export::skins::MeshSkin>) -> Self {
        self.skin = skin;
        self
    }
}

/// One column of the split key: `stride` words per Blender corner, `words.len() ==
/// stride * mesh.corners_num()`. Floats go in as `f32::to_bits` with `-0.0` folded into `0.0`
/// (the add-on compares the bytes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyColumn {
    /// Contributing writer (`PrimitiveAttributeWriter::name`).
    pub writer: &'static str,
    /// Words per corner.
    pub stride: usize,
    /// Corner-major words.
    pub words: Vec<u32>,
}

/// Per-corner split key (the add-on's `dots` minus `vertex_index`, which extractors prepend).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SplitKey {
    /// Number of Blender corners every column covers.
    pub corners: usize,
    /// Columns in writer `order`.
    pub columns: Vec<KeyColumn>,
}

impl SplitKey {
    /// Empty key for a mesh with `corners` corners.
    pub fn new(corners: usize) -> Self {
        SplitKey {
            corners,
            columns: Vec::new(),
        }
    }
    /// Appends a column. Panics when `words.len() != stride * corners` (a writer bug).
    pub fn push(&mut self, writer: &'static str, stride: usize, words: Vec<u32>) {
        assert_eq!(
            words.len(),
            stride * self.corners,
            "split-key column of writer {writer}: {} words for {} corners × stride {stride}",
            words.len(),
            self.corners,
        );
        self.columns.push(KeyColumn {
            writer,
            stride,
            words,
        });
    }
    /// Words per corner over all columns.
    pub fn stride(&self) -> usize {
        self.columns.iter().map(|c| c.stride).sum()
    }
    /// The key words of one corner, column by column.
    pub fn corner(&self, corner: usize) -> impl Iterator<Item = u32> + '_ {
        self.columns.iter().flat_map(move |c| {
            c.words[corner * c.stride..(corner + 1) * c.stride]
                .iter()
                .copied()
        })
    }
    /// The column a writer contributed, if any.
    pub fn column(&self, writer: &str) -> Option<&KeyColumn> {
        self.columns.iter().find(|c| c.writer == writer)
    }
}

/// Typed per-vertex data of one glTF attribute (accessor `componentType` × `type`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AttrValues {
    /// FLOAT VEC2
    F32x2(Vec<[f32; 2]>),
    /// FLOAT VEC3
    F32x3(Vec<[f32; 3]>),
    /// FLOAT VEC4
    F32x4(Vec<[f32; 4]>),
    /// UNSIGNED_BYTE VEC4
    U8x4(Vec<[u8; 4]>),
    /// UNSIGNED_SHORT VEC4
    U16x4(Vec<[u16; 4]>),
}

/// One glTF vertex attribute of a draft primitive.
#[derive(Clone, Debug, PartialEq)]
pub struct AttrColumn {
    /// Values, one per glTF vertex.
    pub values: AttrValues,
    /// Accessor `normalized` (COLOR_0 as normalized UNSIGNED_SHORT, WEIGHTS_0 as bytes/shorts).
    pub normalized: bool,
}

/// One glTF primitive under construction, attribute data already in glTF space.
///
/// `positions`, `normals` and `texcoords` are the M1 attributes; every other attribute
/// (`COLOR_n`, `TANGENT`, `JOINTS_n`, `WEIGHTS_n`, `_CUSTOM`) goes into `attributes` under its
/// glTF semantic, in insertion order, and morph targets into `targets`. `#[non_exhaustive]`:
/// outside this crate, build it with `PrimitiveDraft::default()` and assign fields.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct PrimitiveDraft {
    /// glTF primitive mode (4 = TRIANGLES, 1 = LINES, 0 = POINTS).
    pub mode: u32,
    /// Material slot index (into `mesh.materials`).
    pub material_slot: usize,
    /// POSITION (Y-up).
    pub positions: Vec<[f32; 3]>,
    /// NORMAL (Y-up).
    pub normals: Vec<[f32; 3]>,
    /// TEXCOORD_n, one entry per exported UV map.
    pub texcoords: Vec<Vec<[f32; 2]>>,
    /// Indices.
    pub indices: Vec<u32>,
    /// Representative Blender corner of each glTF vertex (writers gather corner data from it;
    /// all corners merged into one vertex have equal split keys). `u32::MAX` for vertices that
    /// have no corner (loose edges/points: use `vertices`).
    pub corners: Vec<u32>,
    /// Blender vertex index of each glTF vertex (per-vertex data: skin weights, shape keys).
    pub vertices: Vec<u32>,
    /// Further attributes by glTF semantic (`COLOR_0`, `TANGENT`, `JOINTS_0`, `WEIGHTS_0`, …).
    pub attributes: IndexMap<SmolStr, AttrColumn>,
    /// Morph targets (M6): per target, semantic (`POSITION`, `NORMAL`, `TANGENT`) → column.
    pub targets: Vec<IndexMap<SmolStr, AttrColumn>>,
}

/// A writer's split-key contribution (`PrimitiveAttributeWriter::key`).
pub type KeyFn = fn(&MeshExportCx, &mut SplitKey) -> Result<(), ExportError>;

/// Produces primitives from a mesh (one file each in `extract/`).
#[derive(Debug)]
pub struct PrimitiveExtractor {
    /// Unique name.
    pub name: &'static str,
    /// Run order (ascending).
    pub order: u16,
    /// Appends the primitives it extracts, deduplicating on `(vertex, key)`.
    pub run: fn(&MeshExportCx, &SplitKey, &mut Vec<PrimitiveDraft>),
}

/// Adds attributes to a primitive (one file each in `writers/`).
#[derive(Debug)]
pub struct PrimitiveAttributeWriter {
    /// Unique name.
    pub name: &'static str,
    /// Run order (ascending); also the order of its key column.
    pub order: u16,
    /// Contributes this writer's per-corner words to the split key before extraction
    /// (`None` for per-vertex attributes, which never split a vertex).
    pub key: Option<KeyFn>,
    /// Writes its attribute(s) for the extracted vertices.
    pub run: fn(&MeshExportCx, &SplitKey, &mut PrimitiveDraft) -> Result<(), ExportError>,
}

/// Transforms a finished primitive (one file each in `post/`).
#[derive(Debug)]
pub struct PrimitivePostProcess {
    /// Unique name.
    pub name: &'static str,
    /// Run order (ascending).
    pub order: u16,
    /// Post-processes, writing compressed data into the buffer if needed.
    pub run: fn(&MeshExportCx, &mut PrimitiveDraft, &mut BufferBuilder) -> Result<(), ExportError>,
}

/// Every primitive extractor.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static PRIMITIVE_EXTRACTORS: [PrimitiveExtractor];

/// Every attribute writer.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static PRIMITIVE_ATTRIBUTE_WRITERS: [PrimitiveAttributeWriter];

/// Every post-processor.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static PRIMITIVE_POST_PROCESS: [PrimitivePostProcess];
