// SPDX-License-Identifier: GPL-3.0-or-later
//! Element ids, element structs and `BMesh` (ARCHITECTURE §6.2, CONTRACT frozen by M2 W0;
//! implemented by WP 2.1).
//!
//! Ids are pool slots: iteration order is slot order, and a freed slot is reused first (LIFO),
//! as `BLI_mempool` does, so element indices after an edit match Blender's ([MESH §2.4,
//! App. B.6]).

use bitflags::bitflags;
use smol_str::SmolStr;

use crate::customdata::CustomData;
use crate::pool::Pool;

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
        pub struct $name(pub u32);

        impl $name {
            /// The pool slot.
            #[inline]
            pub fn slot(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(
    /// A vertex (`BMVert *`): its pool slot.
    VertId
);
id_type!(
    /// An edge (`BMEdge *`).
    EdgeId
);
id_type!(
    /// A loop / face corner (`BMLoop *`).
    LoopId
);
id_type!(
    /// A face (`BMFace *`).
    FaceId
);

bitflags! {
    /// Element header flags (`BMHeader.hflag`: `BM_ELEM_SELECT`, `BM_ELEM_HIDDEN`,
    /// `BM_ELEM_SMOOTH`, `BM_ELEM_SEAM`, `BM_ELEM_TAG`, `BM_ELEM_INTERNAL_TAG`).
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ElemFlag: u16 {
        /// `BM_ELEM_SELECT`
        const SELECT = 1;
        /// `BM_ELEM_HIDDEN`
        const HIDDEN = 2;
        /// `BM_ELEM_SMOOTH` (faces: smooth shading; edges: not sharp)
        const SMOOTH = 4;
        /// `BM_ELEM_SEAM`
        const SEAM = 8;
        /// `BM_ELEM_TAG` (scratch, owned by the running op)
        const TAG = 16;
        /// `BM_ELEM_INTERNAL_TAG` (scratch of the kernel)
        const INTERNAL_TAG = 32;
    }
}

bitflags! {
    /// `bm->selectmode` (`SCE_SELECT_VERTEX | SCE_SELECT_EDGE | SCE_SELECT_FACE`); several bits
    /// may be set (`tool_settings.mesh_select_mode`).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct SelectMode: u8 {
        /// Vertex select mode.
        const VERT = 1;
        /// Edge select mode.
        const EDGE = 2;
        /// Face select mode.
        const FACE = 4;
    }
}

impl Default for SelectMode {
    /// Blender's factory default: vertex mode.
    fn default() -> Self {
        SelectMode::VERT
    }
}

bitflags! {
    /// Element types (`BM_VERT | BM_EDGE | BM_LOOP | BM_FACE`), e.g. for `elem_index_ensure`.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ElemTypes: u8 {
        /// Vertices.
        const VERT = 1;
        /// Edges.
        const EDGE = 2;
        /// Loops.
        const LOOP = 4;
        /// Faces.
        const FACE = 8;
    }
}

bitflags! {
    /// `eBMCreateFlag`.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct CreateFlag: u8 {
        /// `BM_CREATE_NO_DOUBLE`: return the existing element instead of a duplicate.
        const NO_DOUBLE = 1;
        /// `BM_CREATE_SKIP_CD`: leave custom data uninitialised (the caller copies it).
        const SKIP_CD = 2;
    }
}

/// A vertex.
#[derive(Clone, Debug, PartialEq)]
pub struct BMVert {
    /// Position.
    pub co: [f32; 3],
    /// Normal.
    pub no: [f32; 3],
    /// One edge of the disk cycle (`v->e`).
    pub e: Option<EdgeId>,
    /// Header flags.
    pub flag: ElemFlag,
}

/// The disk-cycle links of an edge at one of its vertices.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DiskLink {
    /// Next edge around the vertex.
    pub next: EdgeId,
    /// Previous edge around the vertex.
    pub prev: EdgeId,
}

/// An edge.
#[derive(Clone, Debug, PartialEq)]
pub struct BMEdge {
    /// `v1`, `v2`.
    pub v: [VertId; 2],
    /// One loop of the radial cycle (`e->l`).
    pub l: Option<LoopId>,
    /// Disk links at `v[0]` and `v[1]`.
    pub disk: [DiskLink; 2],
    /// Header flags.
    pub flag: ElemFlag,
}

/// A loop (face corner).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BMLoop {
    /// The corner's vertex.
    pub v: VertId,
    /// The edge from `v` to `next.v`.
    pub e: EdgeId,
    /// The face.
    pub f: FaceId,
    /// Next loop of the face.
    pub next: LoopId,
    /// Previous loop of the face.
    pub prev: LoopId,
    /// Next loop around `e`.
    pub radial_next: LoopId,
    /// Previous loop around `e`.
    pub radial_prev: LoopId,
}

/// A face.
#[derive(Clone, Debug, PartialEq)]
pub struct BMFace {
    /// First loop (`l_first`; `loops[0]` of the mesh face after conversion).
    pub l_first: LoopId,
    /// Number of corners.
    pub len: u32,
    /// Normal.
    pub no: [f32; 3],
    /// Material index.
    pub mat_nr: i16,
    /// Header flags.
    pub flag: ElemFlag,
}

/// An element of any kind (`BMElem *` + `htype`): `select_history` entries and `geom` slots.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum BMElem {
    /// A vertex.
    Vert(VertId),
    /// An edge.
    Edge(EdgeId),
    /// A face.
    Face(FaceId),
}

/// One shape key while editing (`CD_SHAPEKEY` layer): the key block's name and its
/// coordinates by vertex slot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeLayer {
    /// Key block name.
    pub name: SmolStr,
    /// Coordinates by vertex slot (free slots hold garbage).
    pub co: Vec<[f32; 3]>,
}

/// The edit-mode mesh (`BMesh`).
#[derive(Clone, Debug, Default)]
pub struct BMesh {
    /// Vertices.
    pub verts: Pool<BMVert>,
    /// Edges.
    pub edges: Pool<BMEdge>,
    /// Loops.
    pub loops: Pool<BMLoop>,
    /// Faces.
    pub faces: Pool<BMFace>,
    /// Vertex custom data, columns by vertex slot.
    pub vdata: CustomData,
    /// Edge custom data.
    pub edata: CustomData,
    /// Loop custom data.
    pub ldata: CustomData,
    /// Face custom data.
    pub pdata: CustomData,
    /// `bm->selectmode`.
    pub select_mode: SelectMode,
    /// `bm->selected` (`BMEditSelection` list); the last entry is the active element.
    pub select_history: Vec<BMElem>,
    /// `bm->act_face`.
    pub act_face: Option<FaceId>,
    /// One per key block while editing.
    pub shape_layers: Vec<ShapeLayer>,
}

impl BMesh {
    /// An empty BMesh (`BM_mesh_create`).
    pub fn new() -> BMesh {
        BMesh::default()
    }
    /// Live vertex count (`bm->totvert`).
    pub fn verts_num(&self) -> usize {
        self.verts.live()
    }
    /// Live edge count (`bm->totedge`).
    pub fn edges_num(&self) -> usize {
        self.edges.live()
    }
    /// Live loop count (`bm->totloop`).
    pub fn loops_num(&self) -> usize {
        self.loops.live()
    }
    /// Live face count (`bm->totface`).
    pub fn faces_num(&self) -> usize {
        self.faces.live()
    }
}

/// Errors of `bmesh.ops` and kernel operations. Messages are Blender's operator error strings
/// where Blender reports one (`BMO_error_raise`), e.g. `Select at least two edge loops`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BmError {
    /// `BMO_error_raise(..., BMO_ERROR_CANCEL, msg)`: the operator is cancelled with this
    /// message (an ERROR report, no exception).
    Cancel(String),
    /// `BMO_ERROR_FATAL`: the operator fails (`RuntimeError`).
    Fatal(String),
    /// Invalid input (a stale element id, an out-of-range slot value).
    Invalid(String),
}

impl std::fmt::Display for BmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BmError::Cancel(m) | BmError::Fatal(m) | BmError::Invalid(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for BmError {}
