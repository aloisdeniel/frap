// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh data-API helpers behind the RNA functions `Mesh.update/validate/transform/
//! clear_geometry`, `attributes.new/remove` (also `color_attributes`) and `uv_layers.new`.
//! Ports of `BKE_attribute_new` / `BKE_attribute_calc_unique_name`
//! (`blenkernel/intern/attribute.cc`), `ED_mesh_uv_add` / `mesh_uv_reset_array`
//! (`editors/mesh/mesh_data.cc`), `rna_Mesh_update` (`makesrna/intern/rna_mesh_api.cc`),
//! `BKE_mesh_transform` and `BKE_mesh_clear_geometry_and_metadata`
//! (`blenkernel/intern/mesh.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (`-b --factory-startup`):
//! - `attributes.new(name, type, domain)` appends the attribute at the end, with the lowest
//!   free `.NNN` name among every attribute (internal ones and `position` included:
//!   `new("position", …)` gives `position.001`); an empty name becomes `Attribute`. Values
//!   are 0 / False, white for colours, identity for quaternions and matrices. A colour
//!   attribute on POINT/CORNER becomes the active and default colour when none is set.
//!   `CURVE`, `INSTANCE` and `LAYER` raise `RuntimeError: Error: Attribute domain not
//!   supported by this geometry type`.
//! - `attributes.remove(a)` of `position`, `.edge_verts`, `.corner_vert`, `.corner_edge` raises
//!   `RuntimeError: Error: Attribute is required and cannot be removed`; removing a colour
//!   attribute keeps the active/default colour names; removing the active UV map clears it.
//! - `uv_layers.new(name="UVMap", do_init=True)`: unique name as above; `do_init` copies the
//!   active UV map, or, without one, resets each face (triangle `(0,0) (1,0) (1,1)`, quad
//!   `(0,0) (1,0) (1,1) (0,1)`, n-gon on a circle starting at `(0.5, 1)`); `do_init=False`
//!   gives zeros. The first UV map becomes active and active-render. A ninth UV map is
//!   refused (`None`, WARNING `Cannot add more than 8 UV maps`).
//! - `clear_geometry()` leaves `position`, `.edge_verts`, `.corner_vert`, `.corner_edge`
//!   (empty, in that order) and the material slots; UV maps and vertex groups are gone.
//! - `update()` derives the edges when the mesh has faces but no edges (`calc_edges=True`
//!   always recomputes them, keeping the existing ones): `.edge_verts` and `.select_edge` are
//!   re-created at the end of the attribute list.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use smol_str::SmolStr;

use super::{
    AttrData, Attribute, AttributeSet, CORNER_EDGE, CORNER_VERT, DeformVert, Domain, EDGE_VERTS,
    Mesh, POSITION, SELECT_EDGE, SELECT_POLY, SELECT_VERT,
};

/// `MAX_MTFACE`: the most UV maps a mesh can have.
pub const MAX_UV_MAPS: usize = 8;

/// Byte limit of attribute names (`MAX_CUSTOMDATA_LAYER_NAME - 1`).
const MAX_ATTRIBUTE_NAME: usize = 67;

/// Attributes that cannot be removed (`Attribute.is_required`).
const REQUIRED: [&str; 4] = [POSITION, ".edge_verts", ".corner_vert", ".corner_edge"];

/// Why `Mesh::add_attribute` refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttributeError {
    /// `Attribute domain not supported by this geometry type`
    DomainUnsupported,
    /// An unknown `data_type` identifier.
    TypeUnsupported,
    /// `Attribute is required and cannot be removed`
    Required,
    /// No attribute of that name.
    NotFound,
}

impl AttributeError {
    /// Blender's report text.
    pub fn message(&self) -> &'static str {
        match self {
            AttributeError::DomainUnsupported => {
                "Attribute domain not supported by this geometry type"
            }
            AttributeError::TypeUnsupported => "Attribute type not supported",
            AttributeError::Required => "Attribute is required and cannot be removed",
            AttributeError::NotFound => "Attribute does not exist",
        }
    }
}

/// A column of `len` CustomData default values of a data type (`CD_SET_DEFAULT`: white for
/// colours, identity for quaternions, zero otherwise: a FLOAT4X4 attribute starts as zero
/// matrices on every domain, verified on 5.2.4).
pub fn default_column(data_type: &str, len: usize) -> Option<AttrData> {
    Some(match data_type {
        "FLOAT" => AttrData::Float(Arc::new(vec![0.0; len])),
        "INT" => AttrData::Int(Arc::new(vec![0; len])),
        "INT8" => AttrData::Int8(Arc::new(vec![0; len])),
        "BOOLEAN" => AttrData::Bool(Arc::new(vec![false; len])),
        "FLOAT2" => AttrData::Float2(Arc::new(vec![[0.0; 2]; len])),
        "FLOAT_VECTOR" => AttrData::Float3(Arc::new(vec![[0.0; 3]; len])),
        "FLOAT4" => AttrData::Float4(Arc::new(vec![[0.0; 4]; len])),
        "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(vec![[1.0; 4]; len])),
        "BYTE_COLOR" => AttrData::ByteColor(Arc::new(vec![[255; 4]; len])),
        "INT16_2D" => AttrData::Int16x2(Arc::new(vec![[0; 2]; len])),
        "INT32_2D" => AttrData::Int32x2(Arc::new(vec![[0; 2]; len])),
        "QUATERNION" => AttrData::Quaternion(Arc::new(vec![[1.0, 0.0, 0.0, 0.0]; len])),
        "FLOAT4X4" => AttrData::Float4x4(Arc::new(vec![[[0.0; 4]; 4]; len])),
        "STRING" => AttrData::String(Arc::new(vec![SmolStr::default(); len])),
        _ => return None,
    })
}

/// `mesh_uv_reset_array`: the default UVs of a face with `len` corners.
fn reset_face_uvs(len: usize) -> Vec<[f32; 2]> {
    match len {
        3 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        4 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        n if n > 2 => {
            let dfac = std::f32::consts::TAU / n as f32;
            (0..n)
                .map(|i| {
                    let fac = dfac * i as f32;
                    [0.5 * libm::sinf(fac) + 0.5, 0.5 * libm::cosf(fac) + 0.5]
                })
                .collect()
        }
        n => vec![[0.0, 0.0]; n],
    }
}

impl Mesh {
    /// Element count of a domain.
    pub fn domain_size(&self, d: Domain) -> usize {
        match d {
            Domain::Point => self.verts_num(),
            Domain::Edge => self.edges_num(),
            Domain::Face => self.faces_num(),
            Domain::Corner => self.corners_num(),
        }
    }

    /// `BKE_attribute_calc_unique_name`: `requested` (empty → `Attribute`) made unique among
    /// every attribute and vertex-group name.
    pub fn unique_attribute_name(&self, requested: &str) -> SmolStr {
        let taken = |n: &str| {
            self.attributes().get(n).is_some() || self.vertex_group_names.iter().any(|g| g == n)
        };
        frap_rna::naming::unique_subname(&taken, requested, MAX_ATTRIBUTE_NAME, "Attribute")
    }

    /// `attributes.new(name, type, domain)` (`BKE_attribute_new` plus the colour defaults of
    /// `rna_AttributeGroup_new`): appends a default-valued attribute; returns its name.
    /// `domain` is Blender's identifier (`POINT`, …, `CURVE`/`INSTANCE`/`LAYER` are refused).
    pub fn add_attribute(
        &mut self,
        requested: &str,
        data_type: &str,
        domain: &str,
    ) -> Result<SmolStr, AttributeError> {
        let domain = match domain {
            "POINT" => Domain::Point,
            "EDGE" => Domain::Edge,
            "FACE" => Domain::Face,
            "CORNER" => Domain::Corner,
            _ => return Err(AttributeError::DomainUnsupported),
        };
        let data = default_column(data_type, self.domain_size(domain))
            .ok_or(AttributeError::TypeUnsupported)?;
        let name = self.unique_attribute_name(requested);
        let is_color = matches!(data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
            && matches!(domain, Domain::Point | Domain::Corner);
        self.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain,
            data,
        });
        if is_color {
            self.color_active.get_or_insert_with(|| name.clone());
            self.color_default.get_or_insert_with(|| name.clone());
        }
        Ok(name)
    }

    /// `attributes.remove(attribute)` (`BKE_attribute_remove`): the active UV maps that named
    /// it are cleared; the colour names are kept (verified).
    pub fn remove_attribute(&mut self, name: &str) -> Result<(), AttributeError> {
        if REQUIRED.contains(&name) {
            return Err(AttributeError::Required);
        }
        self.attributes_mut()
            .remove(name)
            .ok_or(AttributeError::NotFound)?;
        for active in [&mut self.uv_active, &mut self.uv_active_render] {
            if active.as_deref() == Some(name) {
                *active = None;
            }
        }
        Ok(())
    }

    /// Number of UV maps (FLOAT2 corner attributes, internal ones excluded).
    pub fn uv_map_count(&self) -> usize {
        self.attributes()
            .iter()
            .filter(|a| {
                a.domain == Domain::Corner
                    && matches!(a.data, AttrData::Float2(_))
                    && !a.name.starts_with('.')
            })
            .count()
    }

    /// `uv_layers.new(name, do_init)` (`ED_mesh_uv_add` with `active_set = false`): the new UV
    /// map's name, or `None` past [`MAX_UV_MAPS`].
    pub fn add_uv_map(&mut self, requested: &str, do_init: bool) -> Option<SmolStr> {
        let existing = self.uv_map_count();
        if existing >= MAX_UV_MAPS {
            return None;
        }
        let requested = if requested.is_empty() {
            "UVMap"
        } else {
            requested
        };
        let name = self.unique_attribute_name(requested);
        let active = self
            .uv_active
            .as_deref()
            .and_then(|n| self.attributes().get(n))
            .or_else(|| {
                self.attributes().iter().find(|a| {
                    a.domain == Domain::Corner
                        && matches!(a.data, AttrData::Float2(_))
                        && !a.name.starts_with('.')
                })
            })
            .and_then(|a| match &a.data {
                AttrData::Float2(v) => Some(v.clone()),
                _ => None,
            });
        let uvs: Vec<[f32; 2]> = match (do_init, active) {
            (true, Some(v)) => v.as_ref().clone(),
            (true, None) => {
                let mut out = Vec::with_capacity(self.corners_num());
                for f in 0..self.faces_num() {
                    out.extend(reset_face_uvs(self.face(f).len()));
                }
                out
            }
            (false, _) => vec![[0.0; 2]; self.corners_num()],
        };
        self.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uvs)),
        });
        if existing == 0 {
            self.uv_active = Some(name.clone());
            self.uv_active_render = Some(name.clone());
        }
        Some(name)
    }

    /// `Mesh.update(calc_edges, calc_edges_loose)`: derives missing edges (module docs) and
    /// drops the runtime caches.
    pub fn update(&mut self, calc_edges: bool) {
        if calc_edges || (self.faces_num() > 0 && self.edges_num() == 0) {
            self.calc_edges(calc_edges, true);
        }
        let _ = self.attributes_mut(); // clears the caches
    }

    /// `vertices.add(count)` / `edges.add` / `polygons.add` / `loops.add` (`mesh_add_verts`,
    /// `mesh_add_edges`, `mesh_add_polys`, `mesh_add_loops`, `editors/mesh/mesh_data.cc`):
    /// appends `count` elements to `domain` (nothing for `count <= 0`). Verified on 5.2.4:
    /// - every attribute of the domain grows; new vertices take the CustomData defaults
    ///   (identity quaternion, zero elsewhere, colours included), new edges, faces and corners
    ///   are zero (a QUATERNION too); new vertices sit at the origin, new edges are `(0, 0)`,
    ///   new corners use vertex 0 and edge 0;
    /// - new vertices, edges and faces are selected: `.select_vert` / `.select_edge` /
    ///   `.select_poly` is created when missing (before `position` / `.edge_verts` on an
    ///   empty mesh); corners create `.corner_vert` then `.corner_edge`;
    /// - new faces start where the last face ended and the last of them takes the corners
    ///   after it (`[(0, 3)]` + 1 → `[(0, 3), (3, 0)]`; no face and 4 corners + 2 →
    ///   `[(0, 0), (0, 4)]`); new corners belong to the last face;
    /// - vertex-group weights grow (empty), shape keys keep their length.
    pub fn add_elements(&mut self, domain: Domain, count: i64) {
        let Ok(n) = usize::try_from(count) else {
            return;
        };
        if n == 0 {
            return;
        }
        let old = self.domain_size(domain);
        let new = old + n;
        let select = match domain {
            Domain::Point => Some(SELECT_VERT),
            Domain::Edge => Some(SELECT_EDGE),
            Domain::Face => Some(SELECT_POLY),
            Domain::Corner => None,
        };
        let builtin: &[(&str, &str)] = match domain {
            Domain::Point => &[(POSITION, "FLOAT_VECTOR")],
            Domain::Edge => &[(EDGE_VERTS, "INT32_2D")],
            Domain::Face => &[],
            Domain::Corner => &[(CORNER_VERT, "INT"), (CORNER_EDGE, "INT")],
        };
        let attrs = self.attributes_mut();
        if let Some(sel) = select
            && attrs.get(sel).is_none()
        {
            attrs.insert(Attribute {
                name: sel.into(),
                domain,
                data: AttrData::Bool(Arc::new(vec![false; old])),
            });
        }
        for (name, ty) in builtin {
            if attrs.get(name).is_none()
                && let Some(data) = default_column(ty, old)
            {
                attrs.insert(Attribute {
                    name: (*name).into(),
                    domain,
                    data,
                });
            }
        }
        let keep: Vec<usize> = (0..old).collect();
        for a in attrs.iter_mut().filter(|a| a.domain == domain) {
            a.data = a.data.gather_with_default(&keep, new);
            // Only new vertices take the CustomData defaults: the other domains are
            // zero-filled, a QUATERNION too.
            if domain != Domain::Point
                && let AttrData::Quaternion(v) = &mut a.data
            {
                Arc::make_mut(v)[old..].fill([0.0; 4]);
            }
        }
        if let Some(sel) = select
            && let Some(a) = attrs.get_mut(sel)
            && let AttrData::Bool(v) = &mut a.data
        {
            Arc::make_mut(v)[old..].fill(true);
        }
        match domain {
            Domain::Point => {
                if let Some(dv) = self.deform_verts.as_mut() {
                    Arc::make_mut(dv).resize(new, DeformVert::default());
                }
            }
            Domain::Face => {
                let corners = self.corners_num() as i32;
                let offs = Arc::make_mut(&mut self.face_offsets);
                let last = *offs.last().unwrap_or(&0);
                offs.truncate(old);
                offs.extend(std::iter::repeat_n(last, n));
                offs.push(corners);
            }
            Domain::Corner => {
                // The last face takes the new corners (no face: nothing to extend).
                if self.face_offsets.len() > 1
                    && let Some(last) = Arc::make_mut(&mut self.face_offsets).last_mut()
                {
                    *last = new as i32;
                }
            }
            Domain::Edge => {}
        }
    }

    /// `BKE_mesh_transform` without shape keys: positions by `m` (column-major 4×4).
    pub fn transform(&mut self, m: &glam::Mat4) {
        for p in self.positions_mut().iter_mut() {
            *p = m.transform_point3(glam::Vec3::from_array(*p)).to_array();
        }
    }

    /// `BKE_mesh_clear_geometry_and_metadata` (module docs).
    pub fn clear_geometry(&mut self) {
        // The temporary tangent layer goes with the geometry (`CustomData_free`).
        self.tangents = None;
        let mut attrs = AttributeSet::default();
        for (name, domain, data) in [
            (POSITION, Domain::Point, AttrData::Float3(Arc::default())),
            (
                ".edge_verts",
                Domain::Edge,
                AttrData::Int32x2(Arc::default()),
            ),
            (
                ".corner_vert",
                Domain::Corner,
                AttrData::Int(Arc::default()),
            ),
            (
                ".corner_edge",
                Domain::Corner,
                AttrData::Int(Arc::default()),
            ),
        ] {
            attrs.insert(Attribute {
                name: name.into(),
                domain,
                data,
            });
        }
        *self.attributes_mut() = attrs;
        self.face_offsets = Arc::new(vec![0]);
        self.deform_verts = None;
        self.vertex_group_names.clear();
        self.uv_active = None;
        self.uv_active_render = None;
        self.color_active = None;
        self.color_default = None;
    }

    /// The first structural problem of the mesh (`BKE_mesh_validate` checks frap relies on):
    /// a corner or edge vertex out of range, a degenerate edge, a corner edge out of range,
    /// non-increasing face offsets, columns whose lengths disagree with their domain, a face
    /// of fewer than 3 corners or using a vertex twice, or a corner whose edge does not join
    /// its vertex to the next corner's (verified on 5.2.4: `polygons.add(1)`, `loops.add(1)`,
    /// a repeated face vertex and swapped `edge_index` values all make `validate()` answer
    /// `True`).
    pub fn validate_report(&self) -> Option<String> {
        let (nv, ne, nc) = (
            self.verts_num() as i64,
            self.edges_num() as i64,
            self.corners_num(),
        );
        for a in self.attributes().iter() {
            if a.data.len() != self.domain_size(a.domain) {
                return Some(format!(
                    "attribute '{}' has {} values for {} elements",
                    a.name,
                    a.data.len(),
                    self.domain_size(a.domain)
                ));
            }
        }
        if self.face_offsets().windows(2).any(|w| w[1] < w[0])
            || self.face_offsets().last().copied().unwrap_or(0) as usize != nc
        {
            return Some("face offsets are not increasing".into());
        }
        if let Some(e) = self
            .edges()
            .iter()
            .position(|e| e[0] == e[1] || e.iter().any(|v| !(0..nv).contains(&i64::from(*v))))
        {
            return Some(format!("edge {e} is invalid"));
        }
        if let Some(c) = self
            .corner_verts()
            .iter()
            .position(|v| !(0..nv).contains(&i64::from(*v)))
        {
            return Some(format!("corner {c} uses a missing vertex"));
        }
        if self.corner_edges().len() == nc
            && let Some(c) = self
                .corner_edges()
                .iter()
                .position(|e| !(0..ne).contains(&i64::from(*e)))
        {
            return Some(format!("corner {c} uses a missing edge"));
        }
        let (verts, edges, corner_edges) = (self.corner_verts(), self.edges(), self.corner_edges());
        for (f, w) in self.face_offsets().windows(2).enumerate() {
            let (a, b) = (w[0] as usize, w[1] as usize);
            if b - a < 3 {
                return Some(format!("face {f} has {} corners", b - a));
            }
            let face = &verts[a..b];
            if let Some(i) = (1..face.len()).find(|&i| face[..i].contains(&face[i])) {
                return Some(format!("face {f} uses vertex {} twice", face[i]));
            }
            if corner_edges.len() != nc {
                continue;
            }
            for c in a..b {
                let next = if c + 1 == b { a } else { c + 1 };
                let (v, w) = (verts[c], verts[next]);
                let e = edges[corner_edges[c] as usize];
                if e != [v, w] && e != [w, v] {
                    return Some(format!(
                        "corner {c} uses edge {}, which does not join vertices {v} and {w}",
                        corner_edges[c]
                    ));
                }
            }
        }
        None
    }

    /// The repairs of `BKE_mesh_validate` frap makes (M8S10-3, verified on 5.2.4), once
    /// [`Mesh::validate_report`] finds nothing it cannot repair: a vertex position with a
    /// non-finite component is zeroed (`(nan, 1, 2)` → `(0, 0, 0)`); an edge joining the same
    /// two vertices as an earlier one is removed, the corners using it use the earlier one and
    /// the edge attributes follow; a face using the same vertices as an earlier one (in any
    /// order) is removed with its corners and their attributes. `Ok(true)` when something was
    /// changed; `Err` names the first problem frap cannot repair yet.
    pub fn validate_repair(&mut self) -> Result<bool, String> {
        if let Some(problem) = self.validate_report() {
            return Err(problem);
        }
        let mut changed = false;
        if self
            .positions()
            .iter()
            .any(|p| p.iter().any(|x| !x.is_finite()))
        {
            for p in self.positions_mut() {
                if p.iter().any(|x| !x.is_finite()) {
                    *p = [0.0; 3];
                }
            }
            changed = true;
        }
        // Duplicate edges: keep the first of each vertex pair.
        let edges = self.edges().to_vec();
        let mut first: HashMap<(i32, i32), usize> = HashMap::with_capacity(edges.len());
        let mut remap = vec![0i32; edges.len()];
        let mut kept: Vec<usize> = Vec::with_capacity(edges.len());
        for (i, e) in edges.iter().enumerate() {
            let key = (e[0].min(e[1]), e[0].max(e[1]));
            match first.get(&key) {
                Some(&j) => remap[i] = remap[j],
                None => {
                    first.insert(key, i);
                    remap[i] = kept.len() as i32;
                    kept.push(i);
                }
            }
        }
        if kept.len() != edges.len() {
            let corner_edges: Vec<i32> = self
                .corner_edges()
                .iter()
                .map(|&e| remap[e as usize])
                .collect();
            let n = kept.len();
            for a in self.attributes_mut().iter_mut() {
                if a.domain == Domain::Edge {
                    a.data = a.data.gather_with_default(&kept, n);
                }
            }
            if let Some(a) = self.attributes_mut().get_mut(CORNER_EDGE) {
                a.data = AttrData::Int(Arc::new(corner_edges));
            }
            changed = true;
        }
        // Duplicate faces: keep the first of each vertex set.
        let mut seen: HashSet<Vec<i32>> = HashSet::with_capacity(self.faces_num());
        let mut kept_faces = Vec::with_capacity(self.faces_num());
        for f in 0..self.faces_num() {
            let mut key = self.corner_verts()[self.face(f)].to_vec();
            key.sort_unstable();
            if seen.insert(key) {
                kept_faces.push(f);
            }
        }
        if kept_faces.len() != self.faces_num() {
            let mut offsets = Vec::with_capacity(kept_faces.len() + 1);
            let mut kept_corners = Vec::with_capacity(self.corners_num());
            offsets.push(0);
            for &f in &kept_faces {
                kept_corners.extend(self.face(f));
                offsets.push(kept_corners.len() as i32);
            }
            let (nf, nc) = (kept_faces.len(), kept_corners.len());
            for a in self.attributes_mut().iter_mut() {
                match a.domain {
                    Domain::Face => a.data = a.data.gather_with_default(&kept_faces, nf),
                    Domain::Corner => a.data = a.data.gather_with_default(&kept_corners, nc),
                    _ => {}
                }
            }
            self.face_offsets = Arc::new(offsets);
            self.set_corner_tangents(None);
            changed = true;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn cube() -> Mesh {
        primitives::cube(2.0, true, [1.0; 3])
    }

    fn names(m: &Mesh) -> Vec<String> {
        m.attributes().iter().map(|a| a.name.to_string()).collect()
    }

    #[test]
    fn add_attribute_names_values_and_colours() {
        let mut m = cube();
        assert_eq!(m.add_attribute("foo", "FLOAT", "POINT").unwrap(), "foo");
        assert_eq!(m.add_attribute("foo", "FLOAT", "POINT").unwrap(), "foo.001");
        assert_eq!(
            m.add_attribute("position", "FLOAT", "POINT").unwrap(),
            "position.001"
        );
        assert_eq!(m.add_attribute("", "FLOAT", "POINT").unwrap(), "Attribute");
        assert_eq!(
            m.add_attribute("x", "FLOAT", "LAYER"),
            Err(AttributeError::DomainUnsupported)
        );
        assert_eq!(names(&m).last().unwrap(), "Attribute");
        let c = m.add_attribute("C", "BYTE_COLOR", "CORNER").unwrap();
        assert_eq!(m.color_active.as_deref(), Some("C"));
        assert_eq!(m.color_default.as_deref(), Some("C"));
        match &m.attributes().get(&c).unwrap().data {
            AttrData::ByteColor(v) => assert_eq!(v[0], [255; 4]),
            _ => panic!("type"),
        }
        m.add_attribute("D", "FLOAT_COLOR", "POINT").unwrap();
        assert_eq!(m.color_active.as_deref(), Some("C"));
        m.remove_attribute("C").unwrap();
        assert_eq!(m.color_active.as_deref(), Some("C"));
        assert_eq!(
            m.remove_attribute("position"),
            Err(AttributeError::Required)
        );
    }

    #[test]
    fn uv_maps_init_copy_reset_and_limit() {
        let mut m = cube();
        let before = m.uv_map_count();
        let n = m.add_uv_map("", true).unwrap();
        assert_eq!(n, "UVMap.001");
        assert_eq!(m.uv_map_count(), before + 1);
        let first = m.uv_maps()[0].1.to_vec();
        assert_eq!(m.uv_maps()[1].1, first.as_slice());
        let z = m.add_uv_map("Z", false).unwrap();
        assert!(
            m.uv_maps()
                .iter()
                .find(|(k, _)| *k == z.as_str())
                .unwrap()
                .1
                .iter()
                .all(|uv| *uv == [0.0, 0.0])
        );
        while m.uv_map_count() < MAX_UV_MAPS {
            m.add_uv_map("", true).unwrap();
        }
        assert_eq!(m.add_uv_map("", true), None);
        assert_eq!(reset_face_uvs(5)[0], [0.5, 1.0]);
        assert!((reset_face_uvs(5)[1][0] - 0.97553).abs() < 1e-4);
    }

    #[test]
    fn clear_geometry_keeps_required_layers() {
        let mut m = cube();
        m.clear_geometry();
        assert_eq!(
            names(&m),
            ["position", ".edge_verts", ".corner_vert", ".corner_edge"]
        );
        assert_eq!(m.verts_num(), 0);
        assert_eq!(m.faces_num(), 0);
        assert!(m.validate_report().is_none());
    }

    #[test]
    fn transform_and_validate() {
        let mut m = cube();
        assert!(m.validate_report().is_none());
        m.transform(&glam::Mat4::from_translation(glam::Vec3::X));
        assert!(m.positions().iter().all(|p| p[0] == 0.0 || p[0] == 2.0));
    }
}
