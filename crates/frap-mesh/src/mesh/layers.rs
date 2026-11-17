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

use std::sync::Arc;

use smol_str::SmolStr;

use super::{AttrData, Attribute, AttributeSet, Domain, Mesh, POSITION};

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
/// colours, identity for quaternions and matrices, zero otherwise).
pub fn default_column(data_type: &str, len: usize) -> Option<AttrData> {
    let ident4 = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
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
        "FLOAT4X4" => AttrData::Float4x4(Arc::new(vec![ident4; len])),
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

    /// `BKE_mesh_transform` without shape keys: positions by `m` (column-major 4×4).
    pub fn transform(&mut self, m: &glam::Mat4) {
        for p in self.positions_mut().iter_mut() {
            *p = m.transform_point3(glam::Vec3::from_array(*p)).to_array();
        }
    }

    /// `BKE_mesh_clear_geometry_and_metadata` (module docs).
    pub fn clear_geometry(&mut self) {
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
    /// non-increasing face offsets, or columns whose lengths disagree with their domain.
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
        None
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
