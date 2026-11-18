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
        /// `BM_ELEM_SELECT_UV`: UV selection of a loop (vertex) or a face (5.x UV select).
        const SELECT_UV = 64;
        /// `BM_ELEM_TAG_ALT` (second scratch tag, owned by the running op)
        const TAG_ALT = 128;
    }
}

impl ElemFlag {
    /// `BM_ELEM_SELECT_UV_EDGE`: the UV edge selection of a loop (Blender reuses the seam bit,
    /// which loops never carry).
    pub const SELECT_UV_EDGE: ElemFlag = ElemFlag::SEAM;
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
    /// Header flags (`SELECT_UV`, `SELECT_UV_EDGE`, `TAG`, …).
    pub flag: ElemFlag,
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

impl From<VertId> for BMElem {
    fn from(v: VertId) -> BMElem {
        BMElem::Vert(v)
    }
}
impl From<EdgeId> for BMElem {
    fn from(e: EdgeId) -> BMElem {
        BMElem::Edge(e)
    }
}
impl From<FaceId> for BMElem {
    fn from(f: FaceId) -> BMElem {
        BMElem::Face(f)
    }
}

impl BMElem {
    /// The element type bit (`htype`).
    pub fn htype(self) -> ElemTypes {
        match self {
            BMElem::Vert(_) => ElemTypes::VERT,
            BMElem::Edge(_) => ElemTypes::EDGE,
            BMElem::Face(_) => ElemTypes::FACE,
        }
    }
    /// The pool slot.
    pub fn slot(self) -> usize {
        match self {
            BMElem::Vert(v) => v.slot(),
            BMElem::Edge(e) => e.slot(),
            BMElem::Face(f) => f.slot(),
        }
    }
}

/// `BMHeader.index` of every element, by slot (`BM_elem_index_get/set`), and
/// `bm->elem_index_dirty`. New elements get -1; `BMesh::elem_index_ensure` fills dense
/// iteration-order indices of the dirty types.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElemIndex {
    /// Vertex indices by slot.
    pub vert: Vec<i32>,
    /// Edge indices by slot.
    pub edge: Vec<i32>,
    /// Loop indices by slot.
    pub loops: Vec<i32>,
    /// Face indices by slot.
    pub face: Vec<i32>,
    /// Types whose indices are stale (`bm->elem_index_dirty`).
    pub dirty: ElemTypes,
}

/// BMesh operator tool flags (`BMFlagLayer` / `oflags`, `BMO_elem_flag_*`): one layer per
/// running operator (`BMO_push`/`BMO_pop`), each holding a `u16` per vertex, edge and face
/// slot. New elements start with every layer cleared, as `BLI_mempool_calloc` does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolFlags {
    /// `[verts, edges, faces]` columns per layer; the last layer is the active one.
    pub layers: Vec<[Vec<u16>; 3]>,
}

impl ToolFlags {
    fn kind(e: BMElem) -> usize {
        match e {
            BMElem::Vert(_) => 0,
            BMElem::Edge(_) => 1,
            BMElem::Face(_) => 2,
        }
    }
    /// `BMO_push`: a new cleared layer for the given slot capacities.
    pub fn push(&mut self, caps: [usize; 3]) {
        self.layers
            .push([vec![0; caps[0]], vec![0; caps[1]], vec![0; caps[2]]]);
    }
    /// `BMO_pop`.
    pub fn pop(&mut self) {
        self.layers.pop();
    }
    /// Nesting depth (`bm->toolflag_index`).
    pub fn depth(&self) -> usize {
        self.layers.len()
    }
    /// Clears slot `slot` of element kind `kind` (0 vert, 1 edge, 2 face) in every layer.
    pub(crate) fn clear_slot(&mut self, kind: usize, slot: usize) {
        for l in &mut self.layers {
            let col = &mut l[kind];
            if col.len() <= slot {
                col.resize(slot + 1, 0);
            }
            col[slot] = 0;
        }
    }
    /// The active layer's flags of `e` (0 without a running operator).
    pub fn get(&self, e: BMElem) -> u16 {
        self.layers
            .last()
            .and_then(|l| l[Self::kind(e)].get(e.slot()).copied())
            .unwrap_or(0)
    }
    /// `BMO_elem_flag_test`.
    pub fn test(&self, e: BMElem, flag: u16) -> bool {
        self.get(e) & flag != 0
    }
    /// `BMO_elem_flag_enable`.
    pub fn enable(&mut self, e: BMElem, flag: u16) {
        self.update(e, |x| x | flag);
    }
    /// `BMO_elem_flag_disable`.
    pub fn disable(&mut self, e: BMElem, flag: u16) {
        self.update(e, |x| x & !flag);
    }
    /// `BMO_elem_flag_set`.
    pub fn set(&mut self, e: BMElem, flag: u16, value: bool) {
        if value {
            self.enable(e, flag);
        } else {
            self.disable(e, flag);
        }
    }
    fn update(&mut self, e: BMElem, f: impl FnOnce(u16) -> u16) {
        let Some(l) = self.layers.last_mut() else {
            return;
        };
        let col = &mut l[Self::kind(e)];
        let s = e.slot();
        if col.len() <= s {
            col.resize(s + 1, 0);
        }
        col[s] = f(col[s]);
    }
}

bitflags! {
    /// `BMOpTypeFlag`: what `bmesh_edit_end` does after a top-level operator.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct OpTypeFlag: u8 {
        /// `BMO_OPTYPE_FLAG_NORMALS_CALC`: `BM_mesh_normals_update`.
        const NORMALS_CALC = 1;
        /// `BMO_OPTYPE_FLAG_SELECT_FLUSH`: `BM_mesh_select_mode_flush`.
        const SELECT_FLUSH = 2;
        /// `BMO_OPTYPE_FLAG_SELECT_VALIDATE`: let the flush drop unselected history entries
        /// (otherwise the history is kept as it was).
        const SELECT_VALIDATE = 4;
        /// `BMO_OPTYPE_FLAG_INVALIDATE_CLNOR_ALL` (custom normal spaces; no-op in frap).
        const INVALIDATE_CLNOR_ALL = 8;
    }
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
    /// Element indices (`BM_elem_index_get/set`) and their dirty flags.
    pub index: ElemIndex,
    /// Operator tool flags (`BMO_elem_flag_*`), one layer per running operator.
    pub oflags: ToolFlags,
}

impl BMesh {
    /// An empty BMesh (`BM_mesh_create(&bm_mesh_allocsize_default)`): pools with Blender's
    /// edit-mode chunk sizes, one preallocated chunk each.
    pub fn new() -> BMesh {
        BMesh::with_alloc_size([512, 1024, 2048, 512])
    }
    /// `BM_mesh_create(&allocsize)` with `allocsize` = `{totvert, totedge, totloop, totface}`
    /// (`BMALLOC_TEMPLATE_FROM_ME` when converting a mesh): only the chunk bookkeeping of the
    /// pools depends on it (see `pool.rs`).
    pub fn with_alloc_size(allocsize: [u32; 4]) -> BMesh {
        use crate::pool::ChunkSizes as C;
        let r = C::REQUESTED;
        BMesh {
            verts: Pool::with_chunks(C::VERT, allocsize[0], r[0]),
            edges: Pool::with_chunks(C::EDGE, allocsize[1], r[1]),
            loops: Pool::with_chunks(C::LOOP, allocsize[2], r[2]),
            faces: Pool::with_chunks(C::FACE, allocsize[3], r[3]),
            ..BMesh::default()
        }
    }

    /// The vertex `v` (panics on a dead id: a kernel invariant violation).
    #[inline]
    #[track_caller]
    pub fn vert(&self, v: VertId) -> &BMVert {
        self.verts.get(v.0).expect("dead BMVert id")
    }
    /// The vertex `v`, mutably.
    #[inline]
    #[track_caller]
    pub fn vert_mut(&mut self, v: VertId) -> &mut BMVert {
        self.verts.get_mut(v.0).expect("dead BMVert id")
    }
    /// The edge `e`.
    #[inline]
    #[track_caller]
    pub fn edge(&self, e: EdgeId) -> &BMEdge {
        self.edges.get(e.0).expect("dead BMEdge id")
    }
    /// The edge `e`, mutably.
    #[inline]
    #[track_caller]
    pub fn edge_mut(&mut self, e: EdgeId) -> &mut BMEdge {
        self.edges.get_mut(e.0).expect("dead BMEdge id")
    }
    /// The loop `l`.
    #[inline]
    #[track_caller]
    pub fn lp(&self, l: LoopId) -> &BMLoop {
        self.loops.get(l.0).expect("dead BMLoop id")
    }
    /// The loop `l`, mutably.
    #[inline]
    #[track_caller]
    pub fn lp_mut(&mut self, l: LoopId) -> &mut BMLoop {
        self.loops.get_mut(l.0).expect("dead BMLoop id")
    }
    /// The face `f`.
    #[inline]
    #[track_caller]
    pub fn face(&self, f: FaceId) -> &BMFace {
        self.faces.get(f.0).expect("dead BMFace id")
    }
    /// The face `f`, mutably.
    #[inline]
    #[track_caller]
    pub fn face_mut(&mut self, f: FaceId) -> &mut BMFace {
        self.faces.get_mut(f.0).expect("dead BMFace id")
    }
    /// Whether `e` refers to a live element.
    pub fn elem_alive(&self, e: BMElem) -> bool {
        match e {
            BMElem::Vert(v) => self.verts.contains(v.0),
            BMElem::Edge(x) => self.edges.contains(x.0),
            BMElem::Face(f) => self.faces.contains(f.0),
        }
    }
    /// Header flags of any element.
    pub fn elem_flag(&self, e: BMElem) -> ElemFlag {
        match e {
            BMElem::Vert(v) => self.vert(v).flag,
            BMElem::Edge(x) => self.edge(x).flag,
            BMElem::Face(f) => self.face(f).flag,
        }
    }
    /// Header flags of any element, mutably.
    pub fn elem_flag_mut(&mut self, e: BMElem) -> &mut ElemFlag {
        match e {
            BMElem::Vert(v) => &mut self.vert_mut(v).flag,
            BMElem::Edge(x) => &mut self.edge_mut(x).flag,
            BMElem::Face(f) => &mut self.face_mut(f).flag,
        }
    }
    /// Live vertex ids in iteration order (`BM_VERTS_OF_MESH`).
    pub fn vert_ids(&self) -> Vec<VertId> {
        self.verts.ids().map(VertId).collect()
    }
    /// Live edge ids in iteration order (`BM_EDGES_OF_MESH`).
    pub fn edge_ids(&self) -> Vec<EdgeId> {
        self.edges.ids().map(EdgeId).collect()
    }
    /// Live face ids in iteration order (`BM_FACES_OF_MESH`).
    pub fn face_ids(&self) -> Vec<FaceId> {
        self.faces.ids().map(FaceId).collect()
    }
    /// `BMO_elem_flag_test` on the running operator's layer.
    pub fn oflag_test(&self, e: impl Into<BMElem>, flag: u16) -> bool {
        self.oflags.test(e.into(), flag)
    }
    /// `BMO_elem_flag_enable`.
    pub fn oflag_enable(&mut self, e: impl Into<BMElem>, flag: u16) {
        self.oflags.enable(e.into(), flag);
    }
    /// `BMO_elem_flag_disable`.
    pub fn oflag_disable(&mut self, e: impl Into<BMElem>, flag: u16) {
        self.oflags.disable(e.into(), flag);
    }
    /// `BMO_elem_flag_set`.
    pub fn oflag_set(&mut self, e: impl Into<BMElem>, flag: u16, value: bool) {
        self.oflags.set(e.into(), flag, value);
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
