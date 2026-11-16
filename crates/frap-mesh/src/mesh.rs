// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh` (ARCHITECTURE §6.1, CONTRACT frozen at M1 W0): struct-of-arrays attributes on four domains,
//! Blender attribute names, copy-on-write columns, runtime caches cleared on every write.

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

/// Attributes in Blender order: by domain, then creation.
#[derive(Clone, Debug, Default)]
pub struct AttributeSet {
    attrs: Vec<Attribute>,
}

impl AttributeSet {
    /// Looks up by name.
    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.attrs.iter().find(|a| a.name == name)
    }
    /// Inserts or replaces (replacing keeps the position).
    pub fn insert(&mut self, attr: Attribute) {
        if let Some(slot) = self.attrs.iter_mut().find(|a| a.name == attr.name) {
            *slot = attr;
            return;
        }
        let at = self
            .attrs
            .iter()
            .position(|a| a.domain > attr.domain)
            .unwrap_or(self.attrs.len());
        self.attrs.insert(at, attr);
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

/// A mesh datablock.
///
/// Serialization (`.frap`, §15) is added by WP 1.4 on top of WP 1.2's `frap_rna::serde_buf`
/// and `IdRef` serde: the §15 attribute shape (`name`/`domain`/`data_type`/`data`) is not what
/// a plain derive produces, and `runtime` is never serialized (Decision log W0-18).
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

impl Mesh {
    /// Empty mesh.
    pub fn new(name: impl Into<SmolStr>) -> Mesh {
        let mut m = Mesh {
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
        };
        m.set_positions(Vec::new());
        m.set_topology(vec![0], Vec::new(), Vec::new());
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

    /// Sets faces (offsets + corner verts) and edges; derives `.corner_edge`.
    pub fn set_topology(
        &mut self,
        face_offsets: Vec<i32>,
        corner_verts: Vec<i32>,
        edges: Vec<[i32; 2]>,
    ) {
        let mut lookup: HashMap<(i32, i32), i32> = HashMap::with_capacity(edges.len());
        for (i, e) in edges.iter().enumerate() {
            lookup.insert((e[0].min(e[1]), e[0].max(e[1])), i as i32);
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
    /// `sharp_face` values, if the attribute exists.
    pub fn sharp_faces(&self) -> Option<&[bool]> {
        match self.attributes.get(SHARP_FACE).map(|a| &a.data) {
            Some(AttrData::Bool(v)) => Some(v),
            _ => None,
        }
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

    /// Face normals (cached).
    pub fn face_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .face_normals
            .get_or_init(|| Arc::new(normals::face_normals(self)))
            .clone()
    }
    /// Corner normals (cached). See [`normals::corner_normals`] for the M0 limits.
    pub fn corner_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .corner_normals
            .get_or_init(|| Arc::new(normals::corner_normals(self, &self.face_normals())))
            .clone()
    }
    /// Vertex normals (cached; `Mesh.vertex_normals`, `normals_domain == 'POINT'`).
    pub fn vert_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .vert_normals
            .get_or_init(|| Arc::new(normals::vert_normals(self, &self.face_normals())))
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
    /// Loop triangles as corner indices (cached).
    pub fn looptris(&self) -> Arc<Vec<[u32; 3]>> {
        self.runtime
            .looptris
            .get_or_init(|| Arc::new(normals::looptris(self)))
            .clone()
    }
}
