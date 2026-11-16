// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-mesh` (§6.1 Mesh accessor API) and of the §9.4 SoA proxies
//! `MeshVertexRef` / `MeshVertexMut` with their collections.
#![forbid(unsafe_code)]

pub mod rna;

use std::sync::{Arc, OnceLock};

use frap_probe_rna::{IdAny, IdHeader, IdRef, IdType, KeyId, MaterialId, MeshId};
use smol_str::SmolStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Domain {
    Point,
    Edge,
    Face,
    Corner,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttrData {
    Float(Arc<Vec<f32>>),
    Int(Arc<Vec<i32>>),
    Int8(Arc<Vec<i8>>),
    Bool(Arc<Vec<bool>>),
    Float2(Arc<Vec<[f32; 2]>>),
    Float3(Arc<Vec<[f32; 3]>>),
    Float4(Arc<Vec<[f32; 4]>>),
    FloatColor(Arc<Vec<[f32; 4]>>),
    ByteColor(Arc<Vec<[u8; 4]>>),
    Int16x2(Arc<Vec<[i16; 2]>>),
    Int32x2(Arc<Vec<[i32; 2]>>),
    Quaternion(Arc<Vec<[f32; 4]>>),
    Float4x4(Arc<Vec<[[f32; 4]; 4]>>),
    String(Arc<Vec<SmolStr>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub name: SmolStr,
    pub domain: Domain,
    pub data: AttrData,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeSet {
    attrs: Vec<Attribute>,
}

impl AttributeSet {
    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.attrs.iter().find(|a| a.name == name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Attribute> {
        self.attrs.iter_mut().find(|a| a.name == name)
    }
    /// Blender order: by domain, then creation.
    pub fn add(&mut self, a: Attribute) {
        let pos = self
            .attrs
            .iter()
            .position(|x| (x.domain as u8) > (a.domain as u8))
            .unwrap_or(self.attrs.len());
        self.attrs.insert(pos, a);
    }
    pub fn remove(&mut self, name: &str) -> Option<Attribute> {
        let i = self.attrs.iter().position(|a| a.name == name)?;
        Some(self.attrs.remove(i))
    }
    pub fn names(&self) -> Vec<&str> {
        self.attrs.iter().map(|a| a.name.as_str()).collect()
    }
}

/// MDeformVert.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeformVert {
    pub weights: smallvec::SmallVec<[(u32, f32); 4]>,
}

/// Private caches; `Clone` returns an empty cache (§5.3, §6.1).
#[derive(Default, Debug)]
pub struct MeshRuntime {
    vert_normals: OnceLock<Arc<Vec<[f32; 3]>>>,
}
impl Clone for MeshRuntime {
    fn clone(&self) -> Self {
        MeshRuntime::default()
    }
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub header: IdHeader,
    face_offsets: Arc<Vec<i32>>,
    attributes: AttributeSet,
    deform_verts: Option<Arc<Vec<DeformVert>>>,
    pub materials: Vec<Option<IdRef<MaterialId>>>,
    pub vertex_group_names: Vec<SmolStr>,
    pub shape_keys: Option<IdRef<KeyId>>,
    runtime: MeshRuntime,
}

impl Mesh {
    pub fn new(name: &str) -> Mesh {
        Mesh {
            header: IdHeader::named(name),
            face_offsets: Arc::new(vec![0]),
            attributes: AttributeSet::default(),
            deform_verts: None,
            materials: vec![],
            vertex_group_names: vec![],
            shape_keys: None,
            runtime: MeshRuntime::default(),
        }
    }

    /// Rust-level `from_pydata` (subset: verts + faces).
    pub fn from_pydata(name: &str, verts: &[[f32; 3]], faces: &[&[i32]]) -> Mesh {
        let mut m = Mesh::new(name);
        let mut offsets = vec![0];
        let mut corners = vec![];
        for f in faces {
            corners.extend_from_slice(f);
            offsets.push(corners.len() as i32);
        }
        m.set_topology(offsets, corners, vec![]);
        m.attributes_mut().add(Attribute {
            name: "position".into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(verts.to_vec())),
        });
        m
    }

    pub fn attributes(&self) -> &AttributeSet {
        &self.attributes
    }
    pub fn face_offsets(&self) -> &[i32] {
        &self.face_offsets
    }
    pub fn deform_verts(&self) -> Option<&[DeformVert]> {
        self.deform_verts.as_deref().map(|v| v.as_slice())
    }

    /// Every geometry write goes through a `&mut self` method that first clears `runtime`.
    pub fn attributes_mut(&mut self) -> &mut AttributeSet {
        self.runtime = MeshRuntime::default();
        &mut self.attributes
    }
    pub fn set_topology(&mut self, face_offsets: Vec<i32>, corner_verts: Vec<i32>, edges: Vec<[i32; 2]>) {
        self.runtime = MeshRuntime::default();
        self.face_offsets = Arc::new(face_offsets);
        let a = &mut self.attributes;
        a.remove(".corner_vert");
        a.remove(".edge_verts");
        a.add(Attribute {
            name: ".corner_vert".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_verts)),
        });
        a.add(Attribute {
            name: ".edge_verts".into(),
            domain: Domain::Edge,
            data: AttrData::Int32x2(Arc::new(edges)),
        });
    }
    pub fn deform_verts_mut(&mut self) -> &mut Vec<DeformVert> {
        self.runtime = MeshRuntime::default();
        Arc::make_mut(self.deform_verts.get_or_insert_with(Default::default))
    }

    pub fn positions(&self) -> &[[f32; 3]] {
        match self.attributes.get("position").map(|a| &a.data) {
            Some(AttrData::Float3(v)) => v,
            _ => &[],
        }
    }
    pub fn positions_mut(&mut self) -> &mut Vec<[f32; 3]> {
        match self.attributes_mut().get_mut("position").map(|a| &mut a.data) {
            Some(AttrData::Float3(v)) => Arc::make_mut(v),
            _ => panic!("invariant: mesh has a position attribute"),
        }
    }
    pub fn verts_len(&self) -> usize {
        self.positions().len()
    }

    /// `.select_vert` etc.: read a bool POINT attribute, `false` when absent.
    pub fn point_bool(&self, name: &str, i: usize) -> bool {
        match self.attributes.get(name).map(|a| &a.data) {
            Some(AttrData::Bool(v)) => v.get(i).copied().unwrap_or(false),
            _ => false,
        }
    }
    /// Write a bool POINT attribute, creating it on first write (verified: `vertex.hide = True`
    /// creates `.hide_vert`, and setting it back to False keeps it).
    pub fn set_point_bool(&mut self, name: &str, i: usize, v: bool) {
        let n = self.verts_len();
        let set = self.attributes_mut();
        if set.get(name).is_none() {
            set.add(Attribute {
                name: name.into(),
                domain: Domain::Point,
                data: AttrData::Bool(Arc::new(vec![false; n])),
            });
        }
        if let Some(Attribute {
            data: AttrData::Bool(col), ..
        }) = set.get_mut(name)
        {
            Arc::make_mut(col)[i] = v;
        }
    }

    /// Cached vertex normals (area-weighted face normals; probe-quality only).
    pub fn vert_normals(&self) -> Arc<Vec<[f32; 3]>> {
        self.runtime
            .vert_normals
            .get_or_init(|| Arc::new(self.compute_vert_normals()))
            .clone()
    }
    pub fn vert_normals_cached(&self) -> bool {
        self.runtime.vert_normals.get().is_some()
    }

    fn compute_vert_normals(&self) -> Vec<[f32; 3]> {
        let pos = self.positions();
        let corners: &[i32] = match self.attributes.get(".corner_vert").map(|a| &a.data) {
            Some(AttrData::Int(v)) => v,
            _ => &[],
        };
        let mut acc = vec![[0.0f32; 3]; pos.len()];
        for w in self.face_offsets.windows(2) {
            let f = &corners[w[0] as usize..w[1] as usize];
            let mut n = [0.0f32; 3];
            for k in 0..f.len() {
                let a = pos[f[k] as usize];
                let b = pos[f[(k + 1) % f.len()] as usize];
                n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                n[2] += (a[0] - b[0]) * (a[1] + b[1]);
            }
            for &v in f {
                for c in 0..3 {
                    acc[v as usize][c] += n[c];
                }
            }
        }
        for n in &mut acc {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 0.0 {
                for c in n.iter_mut() {
                    *c /= l;
                }
            }
        }
        acc
    }
}

impl IdType for Mesh {
    type Kind = MeshId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        for m in self.materials.iter().flatten() {
            f(m.any());
        }
        if let Some(k) = self.shape_keys {
            f(k.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        for m in self.materials.iter_mut() {
            if m.is_some_and(|r| r.any() == from) {
                *m = to.and_then(IdRef::from_any);
            }
        }
    }
}

/// Test helper: the default cube (positions in Blender's `primitive_cube_add` order, [MESH §1.4]).
pub fn test_cube(name: &str) -> Mesh {
    let p = [
        [-1., -1., -1.],
        [-1., -1., 1.],
        [-1., 1., -1.],
        [-1., 1., 1.],
        [1., -1., -1.],
        [1., -1., 1.],
        [1., 1., -1.],
        [1., 1., 1.],
    ];
    Mesh::from_pydata(
        name,
        &p,
        &[
            &[0, 1, 3, 2],
            &[2, 3, 7, 6],
            &[6, 7, 5, 4],
            &[4, 5, 1, 0],
            &[2, 6, 4, 0],
            &[7, 3, 1, 5],
        ],
    )
}
