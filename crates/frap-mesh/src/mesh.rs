// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh` (ARCHITECTURE §6.1, CONTRACT frozen at M1 W0): struct-of-arrays attributes on four domains,
//! Blender attribute names, copy-on-write columns, runtime caches cleared on every write.
//!
//! Attribute order is Blender 5.2's `mesh.attributes` order: **creation order** (5.x keeps
//! attributes in one `AttributeStorage`; verified on 5.2.4: a primitive lists `UVMap`,
//! `.select_*`, `sharp_face`, `.uv_select_*` and then `position`, `.edge_verts`, `.corner_*`;
//! removing and re-adding an attribute moves it to the end). Replacing an attribute keeps its
//! position.

mod calc_edges;
mod layers;
mod lookup;

pub use layers::{AttributeError, MAX_UV_MAPS, default_column};

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use frap_rna::{IdHeader, IdRef, KeyId, MaterialId};
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::normals;

/// Attribute domain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    /// Vertices.
    Point,
    /// Edges.
    Edge,
    /// Faces.
    Face,
    /// Face corners (loops).
    Corner,
}

impl Domain {
    /// Blender's identifier (`'POINT'`, `'EDGE'`, `'FACE'`, `'CORNER'`).
    pub fn identifier(self) -> &'static str {
        match self {
            Domain::Point => "POINT",
            Domain::Edge => "EDGE",
            Domain::Face => "FACE",
            Domain::Corner => "CORNER",
        }
    }
}

/// Typed attribute data (each column `Arc`'d for copy-on-write); Blender's data types.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum AttrData {
    /// FLOAT
    Float(Arc<Vec<f32>>),
    /// INT
    Int(Arc<Vec<i32>>),
    /// INT8
    Int8(Arc<Vec<i8>>),
    /// BOOLEAN
    Bool(Arc<Vec<bool>>),
    /// FLOAT2
    Float2(Arc<Vec<[f32; 2]>>),
    /// FLOAT_VECTOR
    Float3(Arc<Vec<[f32; 3]>>),
    /// FLOAT4 (not a colour)
    Float4(Arc<Vec<[f32; 4]>>),
    /// FLOAT_COLOR (linear)
    FloatColor(Arc<Vec<[f32; 4]>>),
    /// BYTE_COLOR (sRGB bytes)
    ByteColor(Arc<Vec<[u8; 4]>>),
    /// INT16_2D (`custom_normal`)
    Int16x2(Arc<Vec<[i16; 2]>>),
    /// INT32_2D
    Int32x2(Arc<Vec<[i32; 2]>>),
    /// QUATERNION (w-first)
    Quaternion(Arc<Vec<[f32; 4]>>),
    /// FLOAT4X4
    Float4x4(Arc<Vec<[[f32; 4]; 4]>>),
    /// STRING
    String(Arc<Vec<SmolStr>>),
}

/// Applies `$body` to the column of any variant, rebuilding the same variant.
macro_rules! map_column {
    ($data:expr, |$v:ident| $body:expr) => {
        match $data {
            AttrData::Float($v) => AttrData::Float(Arc::new($body)),
            AttrData::Int($v) => AttrData::Int(Arc::new($body)),
            AttrData::Int8($v) => AttrData::Int8(Arc::new($body)),
            AttrData::Bool($v) => AttrData::Bool(Arc::new($body)),
            AttrData::Float2($v) => AttrData::Float2(Arc::new($body)),
            AttrData::Float3($v) => AttrData::Float3(Arc::new($body)),
            AttrData::Float4($v) => AttrData::Float4(Arc::new($body)),
            AttrData::FloatColor($v) => AttrData::FloatColor(Arc::new($body)),
            AttrData::ByteColor($v) => AttrData::ByteColor(Arc::new($body)),
            AttrData::Int16x2($v) => AttrData::Int16x2(Arc::new($body)),
            AttrData::Int32x2($v) => AttrData::Int32x2(Arc::new($body)),
            AttrData::Quaternion($v) => AttrData::Quaternion(Arc::new($body)),
            AttrData::Float4x4($v) => AttrData::Float4x4(Arc::new($body)),
            AttrData::String($v) => AttrData::String(Arc::new($body)),
        }
    };
}

impl AttrData {
    /// Element count.
    pub fn len(&self) -> usize {
        match self {
            AttrData::Float(v) => v.len(),
            AttrData::Int(v) => v.len(),
            AttrData::Int8(v) => v.len(),
            AttrData::Bool(v) => v.len(),
            AttrData::Float2(v) => v.len(),
            AttrData::Float3(v) => v.len(),
            AttrData::Float4(v) => v.len(),
            AttrData::FloatColor(v) => v.len(),
            AttrData::ByteColor(v) => v.len(),
            AttrData::Int16x2(v) => v.len(),
            AttrData::Int32x2(v) => v.len(),
            AttrData::Quaternion(v) => v.len(),
            AttrData::Float4x4(v) => v.len(),
            AttrData::String(v) => v.len(),
        }
    }
    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Blender's `data_type` string.
    pub fn data_type(&self) -> &'static str {
        match self {
            AttrData::Float(_) => "FLOAT",
            AttrData::Int(_) => "INT",
            AttrData::Int8(_) => "INT8",
            AttrData::Bool(_) => "BOOLEAN",
            AttrData::Float2(_) => "FLOAT2",
            AttrData::Float3(_) => "FLOAT_VECTOR",
            AttrData::Float4(_) => "FLOAT4",
            AttrData::FloatColor(_) => "FLOAT_COLOR",
            AttrData::ByteColor(_) => "BYTE_COLOR",
            AttrData::Int16x2(_) => "INT16_2D",
            AttrData::Int32x2(_) => "INT32_2D",
            AttrData::Quaternion(_) => "QUATERNION",
            AttrData::Float4x4(_) => "FLOAT4X4",
            AttrData::String(_) => "STRING",
        }
    }
    /// A column of `len` default values of the same type (`0`, `false`, identity quaternion
    /// and matrix as Blender's `CPPType::default_value`).
    pub fn default_of_len(&self, len: usize) -> AttrData {
        let ident4 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        match self {
            AttrData::Float(_) => AttrData::Float(Arc::new(vec![0.0; len])),
            AttrData::Int(_) => AttrData::Int(Arc::new(vec![0; len])),
            AttrData::Int8(_) => AttrData::Int8(Arc::new(vec![0; len])),
            AttrData::Bool(_) => AttrData::Bool(Arc::new(vec![false; len])),
            AttrData::Float2(_) => AttrData::Float2(Arc::new(vec![[0.0; 2]; len])),
            AttrData::Float3(_) => AttrData::Float3(Arc::new(vec![[0.0; 3]; len])),
            AttrData::Float4(_) => AttrData::Float4(Arc::new(vec![[0.0; 4]; len])),
            AttrData::FloatColor(_) => AttrData::FloatColor(Arc::new(vec![[0.0; 4]; len])),
            AttrData::ByteColor(_) => AttrData::ByteColor(Arc::new(vec![[0; 4]; len])),
            AttrData::Int16x2(_) => AttrData::Int16x2(Arc::new(vec![[0; 2]; len])),
            AttrData::Int32x2(_) => AttrData::Int32x2(Arc::new(vec![[0; 2]; len])),
            AttrData::Quaternion(_) => {
                AttrData::Quaternion(Arc::new(vec![[1.0, 0.0, 0.0, 0.0]; len]))
            }
            AttrData::Float4x4(_) => AttrData::Float4x4(Arc::new(vec![ident4; len])),
            AttrData::String(_) => AttrData::String(Arc::new(vec![SmolStr::default(); len])),
        }
    }
    /// `dst[i] = self[src[i]]` for `i < src.len()`, defaults up to `len`.
    #[allow(clippy::clone_on_copy)] // the macro instantiates the body for every column type
    pub fn gather_with_default(&self, src: &[usize], len: usize) -> AttrData {
        let def = self.default_of_len(1);
        map_column!(self, |v| {
            let fill = first_of(&def, v.as_slice());
            let mut out: Vec<_> = src.iter().map(|&i| v[i].clone()).collect();
            out.resize(len, fill);
            out
        })
    }
}

/// The default element of `def` as the element type of `_like` (helper of
/// [`AttrData::gather_with_default`]).
fn first_of<T: Clone + 'static>(def: &AttrData, _like: &[T]) -> T {
    let any: &dyn std::any::Any = match def {
        AttrData::Float(v) => &v[0],
        AttrData::Int(v) => &v[0],
        AttrData::Int8(v) => &v[0],
        AttrData::Bool(v) => &v[0],
        AttrData::Float2(v) => &v[0],
        AttrData::Float3(v) => &v[0],
        AttrData::Float4(v) => &v[0],
        AttrData::FloatColor(v) => &v[0],
        AttrData::ByteColor(v) => &v[0],
        AttrData::Int16x2(v) => &v[0],
        AttrData::Int32x2(v) => &v[0],
        AttrData::Quaternion(v) => &v[0],
        AttrData::Float4x4(v) => &v[0],
        AttrData::String(v) => &v[0],
    };
    any.downcast_ref::<T>()
        .expect("same attribute type")
        .clone()
}

/// One named attribute.
#[derive(Clone, Debug)]
pub struct Attribute {
    /// Name ("position", "UVMap", ".corner_vert").
    pub name: SmolStr,
    /// Domain.
    pub domain: Domain,
    /// Data.
    pub data: AttrData,
}

/// Attributes in Blender 5.2 order: creation order (see the module docs).
#[derive(Clone, Debug, Default)]
pub struct AttributeSet {
    attrs: Vec<Attribute>,
}

impl AttributeSet {
    /// Looks up by name.
    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.attrs.iter().find(|a| a.name == name)
    }
    /// Looks up by name, mutably.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Attribute> {
        self.attrs.iter_mut().find(|a| a.name == name)
    }
    /// Inserts at the end, or replaces in place when the name exists.
    pub fn insert(&mut self, attr: Attribute) {
        if let Some(slot) = self.attrs.iter_mut().find(|a| a.name == attr.name) {
            *slot = attr;
            return;
        }
        self.attrs.push(attr);
    }
    /// Removes by name.
    pub fn remove(&mut self, name: &str) -> Option<Attribute> {
        let i = self.attrs.iter().position(|a| a.name == name)?;
        Some(self.attrs.remove(i))
    }
    /// All attributes.
    pub fn iter(&self) -> impl Iterator<Item = &Attribute> {
        self.attrs.iter()
    }
    /// All attributes, mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Attribute> {
        self.attrs.iter_mut()
    }
    /// Number of attributes.
    pub fn len(&self) -> usize {
        self.attrs.len()
    }
    /// True without attributes.
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }
}

/// Per-mesh runtime caches. Never persisted, compared or cloned: a clone starts cold.
#[derive(Default, Debug)]
struct MeshRuntime {
    face_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    corner_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    vert_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
    looptris: OnceLock<Arc<Vec<[u32; 3]>>>,
    bounds: OnceLock<Option<[[f32; 3]; 2]>>,
}
impl Clone for MeshRuntime {
    fn clone(&self) -> Self {
        MeshRuntime::default()
    }
}

/// `MDeformVert`: vertex-group weights of one vertex (group index into
/// `Mesh::vertex_group_names`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeformVert {
    /// `(group, weight)` pairs.
    pub weights: SmallVec<[(u32, f32); 4]>,
}

/// Texture space (`Mesh.texspace_location/size`, `use_auto_texspace`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TexSpace {
    /// `use_auto_texspace`
    pub auto: bool,
    /// `texspace_location`
    pub location: [f32; 3],
    /// `texspace_size`
    pub size: [f32; 3],
}

impl Default for TexSpace {
    fn default() -> Self {
        TexSpace {
            auto: true,
            location: [0.0; 3],
            size: [1.0; 3],
        }
    }
}

/// `Mesh.normals_domain`: where the mesh's normals live.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NormalsDomain {
    /// Smooth everywhere: vertex normals.
    Point,
    /// Flat everywhere: face normals.
    Face,
    /// Mixed sharp/smooth or custom normals: corner normals.
    Corner,
}

impl NormalsDomain {
    /// Blender's identifier (`'POINT'`, `'FACE'`, `'CORNER'`).
    pub fn identifier(self) -> &'static str {
        match self {
            NormalsDomain::Point => "POINT",
            NormalsDomain::Face => "FACE",
            NormalsDomain::Corner => "CORNER",
        }
    }
}

/// A mesh datablock.
///
/// Serialization (`.frap`, §15) is added on top of WP 1.2's `frap_rna::serde_buf` and `IdRef`
/// serde once they land: the §15 attribute shape (`name`/`domain`/`data_type`/`data`) is not
/// what a plain derive produces, and `runtime` is never serialized (Decision log W0-18).
#[derive(Clone, Debug)]
pub struct Mesh {
    /// ID header.
    pub header: IdHeader,
    /// `len = faces + 1` (`polygons[i].loop_start` / `loop_total`).
    face_offsets: Arc<Vec<i32>>,
    /// Incl. "position", ".edge_verts", ".corner_vert", ".corner_edge".
    attributes: AttributeSet,
    /// `MDeformVert` per vertex, when any vertex group has weights.
    deform_verts: Option<Arc<Vec<DeformVert>>>,
    /// Material slots (`mesh.materials`); marker kind from frap-rna, no dependency on frap-scene.
    pub materials: Vec<Option<IdRef<MaterialId>>>,
    /// Vertex-group names (DNA-level on the Mesh; RNA exposes them via `Object.vertex_groups`).
    pub vertex_group_names: Vec<SmolStr>,
    /// `Mesh.shape_keys`
    pub shape_keys: Option<IdRef<KeyId>>,
    /// Active UV map name.
    pub uv_active: Option<SmolStr>,
    /// Active-render UV map name (exported as TEXCOORD_0 first by glTF when it is the only one).
    pub uv_active_render: Option<SmolStr>,
    /// `color_attributes.active_color_name`
    pub color_active: Option<SmolStr>,
    /// `color_attributes.default_color_name`
    pub color_default: Option<SmolStr>,
    /// Texture space.
    pub texspace: TexSpace,
    /// OnceLock caches; never persisted, compared or cloned.
    runtime: MeshRuntime,
}

const POSITION: &str = "position";
const EDGE_VERTS: &str = ".edge_verts";
const CORNER_VERT: &str = ".corner_vert";
const CORNER_EDGE: &str = ".corner_edge";
/// Blender's flat-shading attribute (removed when every value is false).
pub const SHARP_FACE: &str = "sharp_face";
/// Blender's sharp-edge attribute (removed when every value is false).
pub const SHARP_EDGE: &str = "sharp_edge";
/// Blender's UV-seam attribute (removed when every value is false).
pub const UV_SEAM: &str = "uv_seam";
/// Custom split normals (`INT16_2D` on corners, or `FLOAT_VECTOR` on any domain).
pub const CUSTOM_NORMAL: &str = "custom_normal";
/// Internal selection attributes, in the order Blender's `bm_to_me` creates them.
pub const SELECT_VERT: &str = ".select_vert";
/// See [`SELECT_VERT`].
pub const SELECT_EDGE: &str = ".select_edge";
/// See [`SELECT_VERT`].
pub const SELECT_POLY: &str = ".select_poly";
/// Internal UV selection attributes (5.x), created with a UV map.
pub const UV_SELECT_VERT: &str = ".uv_select_vert";
/// See [`UV_SELECT_VERT`].
pub const UV_SELECT_EDGE: &str = ".uv_select_edge";
/// See [`UV_SELECT_VERT`].
pub const UV_SELECT_FACE: &str = ".uv_select_face";

/// Flag attributes that `BM_mesh_bm_to_me` (leaving edit mode, and every BMesh round trip)
/// drops when every value is false; see [`Mesh::remove_all_false_flags`].
///
/// Only that conversion trims them (plus `Mesh.shade_smooth()`, which removes `sharp_face`
/// outright). Everything else writes with a plain insert and keeps an all-false attribute, as
/// Blender 5.2.4 does: the RNA proxies `MeshPolygon.use_smooth`, `MeshEdge.use_seam` and
/// `MeshEdge.use_edge_sharp` (setting every polygon smooth, or clearing the last sharp edge or
/// seam, keeps the attribute), `attributes[...].data.foreach_set`, and attribute value
/// assignments. Use [`Mesh::set_bool_attribute`] for those.
pub const REMOVED_WHEN_ALL_FALSE: [&str; 3] = [SHARP_FACE, SHARP_EDGE, UV_SEAM];

impl Mesh {
    /// Empty mesh (`bpy.data.meshes.new`): no attributes at all, like Blender.
    pub fn new(name: impl Into<SmolStr>) -> Mesh {
        Mesh {
            header: IdHeader::new(name),
            face_offsets: Arc::new(vec![0]),
            attributes: AttributeSet::default(),
            deform_verts: None,
            materials: Vec::new(),
            vertex_group_names: Vec::new(),
            shape_keys: None,
            uv_active: None,
            uv_active_render: None,
            color_active: None,
            color_default: None,
            texspace: TexSpace::default(),
            runtime: MeshRuntime::default(),
        }
    }

    /// Builds a mesh from face offsets and a complete attribute set (crate use: generators).
    pub(crate) fn from_parts(
        name: impl Into<SmolStr>,
        face_offsets: Vec<i32>,
        attributes: AttributeSet,
    ) -> Mesh {
        let mut m = Mesh::new(name);
        m.face_offsets = Arc::new(face_offsets);
        m.attributes = attributes;
        m
    }

    /// Attributes (read).
    pub fn attributes(&self) -> &AttributeSet {
        &self.attributes
    }
    /// Attributes (write): clears runtime caches first.
    pub fn attributes_mut(&mut self) -> &mut AttributeSet {
        self.runtime = MeshRuntime::default();
        &mut self.attributes
    }
    /// `polygons[i].loop_start` table, length `faces + 1`.
    pub fn face_offsets(&self) -> &[i32] {
        &self.face_offsets
    }

    /// Vertex-group weights (`MDeformVert`), if any.
    pub fn deform_verts(&self) -> Option<&[DeformVert]> {
        self.deform_verts.as_deref().map(|v| v.as_slice())
    }

    /// Mutable vertex-group weights (created on demand, one entry per vertex); clears caches.
    pub fn deform_verts_mut(&mut self) -> &mut Vec<DeformVert> {
        self.runtime = MeshRuntime::default();
        let n = self.verts_num();
        Arc::make_mut(
            self.deform_verts
                .get_or_insert_with(|| Arc::new(vec![DeformVert::default(); n])),
        )
    }

    /// Replaces vertex positions (vertex count changes are the caller's responsibility).
    pub fn set_positions(&mut self, positions: Vec<[f32; 3]>) {
        self.attributes_mut().insert(Attribute {
            name: POSITION.into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(positions)),
        });
    }

    /// Mutable positions (copy-on-write); clears caches. Creates `position` when missing.
    pub fn positions_mut(&mut self) -> &mut Vec<[f32; 3]> {
        if !matches!(
            self.attributes.get(POSITION).map(|a| &a.data),
            Some(AttrData::Float3(_))
        ) {
            self.set_positions(Vec::new());
        }
        match self.attributes_mut().get_mut(POSITION).map(|a| &mut a.data) {
            Some(AttrData::Float3(v)) => Arc::make_mut(v),
            _ => unreachable!("position was just ensured"),
        }
    }

    /// Sets faces (offsets + corner verts) and edges; derives `.corner_edge`.
    pub fn set_topology(
        &mut self,
        face_offsets: Vec<i32>,
        corner_verts: Vec<i32>,
        edges: Vec<[i32; 2]>,
    ) {
        let mut lookup: HashMap<(i32, i32), i32> = HashMap::with_capacity(edges.len());
        for (i, e) in edges.iter().enumerate() {
            lookup
                .entry((e[0].min(e[1]), e[0].max(e[1])))
                .or_insert(i as i32);
        }
        let mut corner_edges = vec![-1; corner_verts.len()];
        for f in 0..face_offsets.len().saturating_sub(1) {
            let (s, e) = (face_offsets[f] as usize, face_offsets[f + 1] as usize);
            for c in s..e {
                let a = corner_verts[c];
                let b = corner_verts[if c + 1 == e { s } else { c + 1 }];
                corner_edges[c] = *lookup.get(&(a.min(b), a.max(b))).unwrap_or(&-1);
            }
        }
        self.face_offsets = Arc::new(face_offsets);
        let attrs = self.attributes_mut();
        attrs.insert(Attribute {
            name: EDGE_VERTS.into(),
            domain: Domain::Edge,
            data: AttrData::Int32x2(Arc::new(edges)),
        });
        attrs.insert(Attribute {
            name: CORNER_VERT.into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_verts)),
        });
        attrs.insert(Attribute {
            name: CORNER_EDGE.into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_edges)),
        });
    }

    /// Builds a mesh from positions and faces; edges are created in BMesh face-creation order
    /// (`BM_edges_from_verts_ensure`: for each face, edge `(v[i-1], v[i])`), the order Blender's
    /// primitive operators produce.
    pub fn from_faces_bmesh_order(
        name: impl Into<SmolStr>,
        positions: Vec<[f32; 3]>,
        faces: &[&[i32]],
    ) -> Mesh {
        let mut edges: Vec<[i32; 2]> = Vec::new();
        let mut seen: HashMap<(i32, i32), ()> = HashMap::new();
        let mut offsets = vec![0i32];
        let mut corner_verts = Vec::new();
        for f in faces {
            let n = f.len();
            for i in 0..n {
                let (a, b) = (f[(i + n - 1) % n], f[i]);
                if seen.insert((a.min(b), a.max(b)), ()).is_none() {
                    edges.push([a, b]);
                }
            }
            corner_verts.extend_from_slice(f);
            offsets.push(corner_verts.len() as i32);
        }
        let mut m = Mesh::new(name);
        m.set_positions(positions);
        m.set_topology(offsets, corner_verts, edges);
        m
    }

    // Ported from blender/scripts/modules/_bpy_types.py (Mesh.from_pydata) and the RNA calls it
    // makes (`vertices.add`, `edges.add`, `loops.add`, `polygons.add`, `shade_flat`,
    // `Mesh.update(calc_edges=bool(faces), calc_edges_loose=bool(edges))`).
    /// `Mesh.from_pydata(vertices, edges, faces, shade_flat)`: replaces the geometry (Blender
    /// requires an empty mesh) and creates the attributes in Blender's order: `.select_vert`,
    /// `position`, [`.select_edge`, `.edge_verts`], [`.corner_vert`, `.corner_edge`,
    /// `.select_poly`], [`sharp_face`], then `mesh_calc_edges` re-creates `.edge_verts` and
    /// `.select_edge` at the end when there are faces (given edges unselected, derived edges
    /// selected).
    pub fn from_pydata(
        &mut self,
        vertices: &[[f32; 3]],
        edges: &[[i32; 2]],
        faces: &[Vec<i32>],
        shade_flat: bool,
    ) {
        self.attributes_mut().attrs.clear();
        self.face_offsets = Arc::new(vec![0]);
        self.deform_verts = None;
        let attrs = &mut self.attributes;
        if !vertices.is_empty() {
            attrs.insert(bool_attr(
                SELECT_VERT,
                Domain::Point,
                vec![true; vertices.len()],
            ));
            attrs.insert(Attribute {
                name: POSITION.into(),
                domain: Domain::Point,
                data: AttrData::Float3(Arc::new(vertices.to_vec())),
            });
        }
        if !edges.is_empty() {
            attrs.insert(bool_attr(
                SELECT_EDGE,
                Domain::Edge,
                vec![true; edges.len()],
            ));
            attrs.insert(Attribute {
                name: EDGE_VERTS.into(),
                domain: Domain::Edge,
                data: AttrData::Int32x2(Arc::new(edges.to_vec())),
            });
        }
        let corners: usize = faces.iter().map(|f| f.len()).sum();
        if corners > 0 {
            let cv: Vec<i32> = faces.iter().flatten().copied().collect();
            attrs.insert(Attribute {
                name: CORNER_VERT.into(),
                domain: Domain::Corner,
                data: AttrData::Int(Arc::new(cv)),
            });
            attrs.insert(Attribute {
                name: CORNER_EDGE.into(),
                domain: Domain::Corner,
                data: AttrData::Int(Arc::new(vec![0; corners])),
            });
        }
        if !faces.is_empty() {
            attrs.insert(bool_attr(
                SELECT_POLY,
                Domain::Face,
                vec![true; faces.len()],
            ));
            let mut offs = Vec::with_capacity(faces.len() + 1);
            let mut acc = 0i32;
            offs.push(0);
            for f in faces {
                acc += f.len() as i32;
                offs.push(acc);
            }
            self.face_offsets = Arc::new(offs);
        }
        if shade_flat {
            self.shade_flat();
        }
        if !edges.is_empty() || !faces.is_empty() {
            // `Mesh.update(calc_edges=bool(faces))`; with faces but no edges Blender computes
            // them anyway (`ED_mesh_update`).
            if !faces.is_empty() {
                self.calc_edges(true, true);
            }
        }
    }

    /// `Mesh.shade_flat()` (Python): ensures `sharp_face` and sets it true on every face.
    pub fn shade_flat(&mut self) {
        let n = self.faces_num();
        self.attributes_mut()
            .insert(bool_attr(SHARP_FACE, Domain::Face, vec![true; n]));
    }

    /// `Mesh.shade_smooth()` (Python): removes `sharp_face`.
    pub fn shade_smooth(&mut self) {
        self.attributes_mut().remove(SHARP_FACE);
    }

    /// Writes a boolean attribute with a plain insert (the RNA proxies `use_smooth`,
    /// `use_seam`, `use_edge_sharp` and attribute data writes): an existing attribute keeps its
    /// position, a new one is appended, and all-false values are kept, flags included (Blender
    /// 5.2.4 never trims on these paths; see [`REMOVED_WHEN_ALL_FALSE`]).
    pub fn set_bool_attribute(&mut self, name: &str, domain: Domain, values: Vec<bool>) {
        self.attributes_mut()
            .insert(bool_attr(name, domain, values));
    }

    /// Removes every [`REMOVED_WHEN_ALL_FALSE`] attribute whose values are all false: the
    /// clean-up `BM_mesh_bm_to_me` performs, and only that (callers: the BMesh-to-mesh
    /// conversion). Returns the removed names.
    pub fn remove_all_false_flags(&mut self) -> Vec<&'static str> {
        let mut removed = Vec::new();
        for name in REMOVED_WHEN_ALL_FALSE {
            let all_false = matches!(self.attributes.get(name).map(|a| &a.data),
                Some(AttrData::Bool(v)) if v.iter().all(|b| !b));
            if all_false {
                self.attributes_mut().remove(name);
                removed.push(name);
            }
        }
        removed
    }

    fn float3(&self, name: &str) -> &[[f32; 3]] {
        match self.attributes.get(name).map(|a| &a.data) {
            Some(AttrData::Float3(v)) => v,
            _ => &[],
        }
    }
    fn int(&self, name: &str) -> &[i32] {
        match self.attributes.get(name).map(|a| &a.data) {
            Some(AttrData::Int(v)) => v,
            _ => &[],
        }
    }
    /// Boolean attribute values on `domain`, if present with that type and domain.
    pub fn bool_attribute(&self, name: &str, domain: Domain) -> Option<&[bool]> {
        match self.attributes.get(name) {
            Some(Attribute {
                domain: d,
                data: AttrData::Bool(v),
                ..
            }) if *d == domain => Some(v),
            _ => None,
        }
    }

    /// Vertex positions.
    pub fn positions(&self) -> &[[f32; 3]] {
        self.float3(POSITION)
    }
    /// Edge vertex pairs.
    pub fn edges(&self) -> &[[i32; 2]] {
        match self.attributes.get(EDGE_VERTS).map(|a| &a.data) {
            Some(AttrData::Int32x2(v)) => v,
            _ => &[],
        }
    }
    /// Corner → vertex.
    pub fn corner_verts(&self) -> &[i32] {
        self.int(CORNER_VERT)
    }
    /// Corner → edge.
    pub fn corner_edges(&self) -> &[i32] {
        self.int(CORNER_EDGE)
    }
    /// Number of vertices.
    pub fn verts_num(&self) -> usize {
        self.positions().len()
    }
    /// Number of edges.
    pub fn edges_num(&self) -> usize {
        self.edges().len()
    }
    /// Number of faces.
    pub fn faces_num(&self) -> usize {
        self.face_offsets.len() - 1
    }
    /// Number of corners.
    pub fn corners_num(&self) -> usize {
        self.corner_verts().len()
    }
    /// Corner range of face `i`.
    pub fn face(&self, i: usize) -> Range<usize> {
        self.face_offsets[i] as usize..self.face_offsets[i + 1] as usize
    }
    /// The stored `sharp_face` values when the attribute is BOOLEAN on FACE (the shape Blender
    /// itself writes). For the values Blender *reads* (any type or domain, converted), use
    /// [`Mesh::sharp_face_flags`].
    pub fn sharp_faces(&self) -> Option<&[bool]> {
        self.bool_attribute(SHARP_FACE, Domain::Face)
    }
    /// The stored `sharp_edge` values when the attribute is BOOLEAN on EDGE; see
    /// [`Mesh::sharp_edge_flags`].
    pub fn sharp_edges(&self) -> Option<&[bool]> {
        self.bool_attribute(SHARP_EDGE, Domain::Edge)
    }
    /// `sharp_face` as Blender reads it (`lookup_or_default<bool>("sharp_face", FACE)`):
    /// converted from any type and domain ([`Mesh::lookup_bool`]); `None` means all false.
    pub fn sharp_face_flags(&self) -> Option<Cow<'_, [bool]>> {
        self.lookup_bool(SHARP_FACE, Domain::Face)
    }
    /// `sharp_edge` as Blender reads it (`lookup_or_default<bool>("sharp_edge", EDGE)`); see
    /// [`Mesh::sharp_face_flags`].
    pub fn sharp_edge_flags(&self) -> Option<Cow<'_, [bool]>> {
        self.lookup_bool(SHARP_EDGE, Domain::Edge)
    }
    /// UV maps (FLOAT2 corner attributes without a leading `.`), in attribute order.
    pub fn uv_maps(&self) -> Vec<(&str, &[[f32; 2]])> {
        self.attributes
            .iter()
            .filter(|a| a.domain == Domain::Corner && !a.name.starts_with('.'))
            .filter_map(|a| match &a.data {
                AttrData::Float2(v) => Some((a.name.as_str(), v.as_slice())),
                _ => None,
            })
            .collect()
    }

    // Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (Mesh::normals_domain).
    /// `Mesh.normals_domain`: POINT without faces; the custom normals' domain when present
    /// (`custom_normal`); FACE when every face or every edge is sharp; POINT when nothing is
    /// sharp; CORNER otherwise. The flags are read like Blender reads them
    /// ([`Mesh::sharp_face_flags`], [`Mesh::sharp_edge_flags`]: any type and domain).
    pub fn normals_domain(&self) -> NormalsDomain {
        if self.faces_num() == 0 {
            return NormalsDomain::Point;
        }
        if let Some(custom) = self.attributes.get(CUSTOM_NORMAL) {
            match custom.domain {
                Domain::Point => return NormalsDomain::Point,
                Domain::Face => return NormalsDomain::Face,
                Domain::Corner => return NormalsDomain::Corner,
                Domain::Edge => {}
            }
        }
        let mix = |v: Option<&[bool]>| -> (bool, bool) {
            match v {
                None => (false, true),
                Some([]) => (true, true),
                Some(v) => (v.iter().all(|b| *b), v.iter().all(|b| !b)),
            }
        };
        let (face_all_true, face_all_false) = mix(self.sharp_face_flags().as_deref());
        if face_all_true {
            return NormalsDomain::Face;
        }
        let (edge_all_true, edge_all_false) = mix(self.sharp_edge_flags().as_deref());
        if edge_all_true {
            return NormalsDomain::Face;
        }
        if edge_all_false && face_all_false {
            return NormalsDomain::Point;
        }
        NormalsDomain::Corner
    }

    /// Face normals (cached; `Mesh.polygon_normals`): Newell normals, or derived from custom
    /// normals when the mesh has them.
    pub fn face_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .face_normals
            .get_or_init(|| Arc::new(normals::face_normals(self)))
            .clone()
    }
    /// Corner normals (cached; `Mesh.corner_normals`), following [`Mesh::normals_domain`].
    pub fn corner_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .corner_normals
            .get_or_init(|| Arc::new(normals::corner_normals(self)))
            .clone()
    }
    /// Vertex normals (cached; `Mesh.vertex_normals`).
    pub fn vert_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .vert_normals
            .get_or_init(|| Arc::new(normals::vert_normals(self)))
            .clone()
    }
    /// Axis-aligned bounds of the positions, `[min, max]` (cached); `None` without vertices
    /// (`Mesh::bounds_min_max`).
    pub fn bounds(&self) -> Option<[[f32; 3]; 2]> {
        *self.runtime.bounds.get_or_init(|| {
            let mut it = self.positions().iter();
            let first = *it.next()?;
            Some(it.fold([first, first], |[lo, hi], p| {
                [
                    [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                    [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
                ]
            }))
        })
    }
    /// Loop triangles as corner indices (cached; `Mesh.loop_triangles`).
    pub fn looptris(&self) -> Arc<Vec<[u32; 3]>> {
        self.runtime
            .looptris
            .get_or_init(|| Arc::new(normals::looptris(self)))
            .clone()
    }
    /// Face index of each loop triangle (`loop_triangle_polygons`).
    pub fn looptri_faces(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.corners_num());
        for f in 0..self.faces_num() {
            let n = self.face(f).len();
            out.extend(std::iter::repeat_n(f as u32, n.saturating_sub(2)));
        }
        out
    }
}

/// A BOOLEAN attribute.
pub(crate) fn bool_attr(name: &str, domain: Domain, values: Vec<bool>) -> Attribute {
    Attribute {
        name: name.into(),
        domain,
        data: AttrData::Bool(Arc::new(values)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warm(m: &Mesh) {
        let _ = (
            m.face_normals(),
            m.vert_normals(),
            m.corner_normals(),
            m.looptris(),
            m.bounds(),
        );
    }

    fn is_cold(m: &Mesh) -> bool {
        let r = &m.runtime;
        r.face_normals.get().is_none()
            && r.vert_normals.get().is_none()
            && r.corner_normals.get().is_none()
            && r.looptris.get().is_none()
            && r.bounds.get().is_none()
    }

    #[test]
    fn clone_starts_cold() {
        let m = crate::primitives::cube(2.0, true, [1.0; 3]);
        warm(&m);
        assert!(!is_cold(&m));
        assert!(is_cold(&m.clone()));
    }

    #[test]
    fn every_mut_accessor_clears_caches() {
        type Write = fn(&mut Mesh);
        let writes: [(&str, Write); 10] = [
            ("attributes_mut", |m| {
                m.attributes_mut();
            }),
            ("deform_verts_mut", |m| {
                m.deform_verts_mut();
            }),
            ("set_positions", |m| m.set_positions(m.positions().to_vec())),
            ("positions_mut", |m| {
                m.positions_mut();
            }),
            ("set_topology", |m| {
                m.set_topology(
                    m.face_offsets().to_vec(),
                    m.corner_verts().to_vec(),
                    m.edges().to_vec(),
                )
            }),
            ("calc_edges", |m| m.calc_edges(false, true)),
            ("shade_flat", |m| m.shade_flat()),
            ("shade_smooth", |m| m.shade_smooth()),
            ("set_bool_attribute", |m| {
                m.set_bool_attribute("x", Domain::Point, vec![true; 8])
            }),
            ("from_pydata", |m| {
                m.from_pydata(
                    &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    &[],
                    &[vec![0, 1, 2]],
                    true,
                )
            }),
        ];
        for (name, w) in writes {
            let mut m = crate::primitives::cube(2.0, true, [1.0; 3]);
            warm(&m);
            w(&mut m);
            assert!(is_cold(&m), "{name} left a cache behind");
        }
        // remove_all_false_flags writes (and clears) only when something is removed.
        let mut m = crate::primitives::cube(2.0, true, [1.0; 3]);
        m.attributes_mut()
            .insert(bool_attr(SHARP_EDGE, Domain::Edge, vec![false; 12]));
        warm(&m);
        assert_eq!(m.remove_all_false_flags(), [SHARP_EDGE]);
        assert!(is_cold(&m));
    }

    #[test]
    fn attribute_order_is_creation_order() {
        let mut s = AttributeSet::default();
        for (n, d) in [
            ("b", Domain::Corner),
            ("a", Domain::Point),
            ("c", Domain::Edge),
        ] {
            s.insert(bool_attr(n, d, vec![]));
        }
        s.insert(bool_attr("a", Domain::Point, vec![true]));
        let names: Vec<&str> = s.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["b", "a", "c"], "replacing keeps the position");
        s.remove("b");
        s.insert(bool_attr("b", Domain::Corner, vec![]));
        let names: Vec<&str> = s.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["a", "c", "b"], "re-adding appends");
    }
}
