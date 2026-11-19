// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.PointCloud` (§5.6; M3, WP 3.7): a number of points and their POINT-domain
//! attributes (`position`, `radius` and generic ones, in creation order), plus material slots.
//! Created by `import_scene.gltf(import_point_as_pointcloud=True)` and exported as glTF POINTS.
//!
//! Blender 5.2.4 semantics (verified): `bpy.data.pointclouds.new()` has no point and no
//! attribute; `resize(n)` creates `position` (zeros); a point cloud without `radius` reads
//! `0.01` per point (`pointcloud_radius_default`), and writing `points[i].radius` creates the
//! `radius` attribute filled with that default first (`radius_for_write`). The binding covers
//! `points` (`co`, `radius`, `index`); `materials`, `attributes`, `color_attributes` and
//! `animation_data` are data only (no RNA binding yet).

use frap_anim::AnimData;
use frap_mesh::{AttrData, Attribute, AttributeSet, Domain};
use frap_rna::*;
use smol_str::SmolStr;

/// `pointcloud_radius_default`.
pub const DEFAULT_RADIUS: f32 = 0.01;

/// Point cloud data (`bpy.data.pointclouds`).
#[derive(Clone, Debug)]
pub struct PointCloud {
    /// ID header.
    pub header: IdHeader,
    /// `totpoint`: the POINT domain size.
    points_num: usize,
    /// POINT-domain attributes (`position`, `radius`, generic ones).
    attributes: AttributeSet,
    /// `PointCloud.materials` (material slots).
    pub materials: Vec<Option<IdRef<MaterialId>>>,
    /// `PointCloud.animation_data`
    pub animation_data: Option<AnimData>,
}

impl PointCloud {
    /// An empty point cloud (`bpy.data.pointclouds.new(name)`).
    pub fn new(name: impl Into<SmolStr>) -> PointCloud {
        PointCloud {
            header: IdHeader::new(name),
            points_num: 0,
            attributes: AttributeSet::default(),
            materials: Vec::new(),
            animation_data: None,
        }
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.points_num
    }

    /// True without points.
    pub fn is_empty(&self) -> bool {
        self.points_num == 0
    }

    /// The attributes, in creation order.
    pub fn attributes(&self) -> &AttributeSet {
        &self.attributes
    }

    /// Adds or replaces a POINT attribute; its length must be the point count.
    pub fn set_attribute(&mut self, name: &str, data: AttrData) -> Result<(), String> {
        if data.len() != self.points_num {
            return Err(format!(
                "attribute '{name}' has {} values for {} points",
                data.len(),
                self.points_num
            ));
        }
        self.attributes.insert(Attribute {
            name: name.into(),
            domain: Domain::Point,
            data,
        });
        Ok(())
    }

    /// `resize(n)`: every attribute truncated or extended with its default; `position` exists
    /// afterwards.
    pub fn resize(&mut self, n: usize) {
        let mut attrs = AttributeSet::default();
        for a in self.attributes.iter() {
            let keep: Vec<usize> = (0..n.min(self.points_num)).collect();
            let data = a.data.gather_with_default(&keep, n);
            attrs.insert(Attribute {
                name: a.name.clone(),
                domain: Domain::Point,
                data,
            });
        }
        self.attributes = attrs;
        self.points_num = n;
        if self.attributes.get("position").is_none() {
            self.attributes.insert(Attribute {
                name: "position".into(),
                domain: Domain::Point,
                data: AttrData::Float3(std::sync::Arc::new(vec![[0.0; 3]; n])),
            });
        }
    }

    /// Point positions (zeros without a `position` attribute).
    pub fn positions(&self) -> Vec<[f32; 3]> {
        match self.attributes.get("position").map(|a| &a.data) {
            Some(AttrData::Float3(v)) => v.to_vec(),
            _ => vec![[0.0; 3]; self.points_num],
        }
    }

    /// Point radii (`DEFAULT_RADIUS` without a `radius` attribute).
    pub fn radii(&self) -> Vec<f32> {
        match self.attributes.get("radius").map(|a| &a.data) {
            Some(AttrData::Float(v)) => v.to_vec(),
            _ => vec![DEFAULT_RADIUS; self.points_num],
        }
    }

    /// `points[i].co = v`.
    pub fn set_position(&mut self, i: usize, v: [f32; 3]) {
        let mut p = self.positions();
        if let Some(x) = p.get_mut(i) {
            *x = v;
        }
        self.attributes.insert(Attribute {
            name: "position".into(),
            domain: Domain::Point,
            data: AttrData::Float3(std::sync::Arc::new(p)),
        });
    }

    /// `points[i].radius = v` (creates `radius` filled with the default first).
    pub fn set_radius(&mut self, i: usize, v: f32) {
        let mut r = self.radii();
        if let Some(x) = r.get_mut(i) {
            *x = v;
        }
        self.attributes.insert(Attribute {
            name: "radius".into(),
            domain: Domain::Point,
            data: AttrData::Float(std::sync::Arc::new(r)),
        });
    }
}

crate::id::id_rna! { PointCloud as frap_spec_rna::POINT_CLOUD, "POINTCLOUD" {
    points => soa(pub Point as frap_spec_rna::POINT, len = |p| p.len()) {
        co => get(|p, i| p.positions()[i]) set(|p, i, v: [f32; 3]| p.set_position(i, v))
            (tag = Vector),
        radius => get(|p, i| p.radii()[i]) set(|p, i, v: f32| p.set_radius(i, v)),
        index => get(|_p, i| i as i32),
    },
}}

impl IdType for PointCloud {
    type Kind = PointCloudId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        self.materials.iter().flatten().for_each(|m| f(m.any()));
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        for m in &mut self.materials {
            if m.map(|m| m.any()) == Some(from) {
                *m = to.and_then(IdRef::from_any);
            }
        }
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_match_rna() {
        frap_rna::testing::check_bindings::<PointCloud>();
    }

    #[test]
    fn resize_radius_default_and_radius_write() {
        let mut pc = PointCloud::new("P");
        assert!(pc.is_empty() && pc.attributes().iter().next().is_none());
        pc.resize(3);
        let names: Vec<_> = pc.attributes().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["position"]);
        assert_eq!(pc.radii(), [DEFAULT_RADIUS; 3]);
        pc.set_radius(0, 0.5);
        assert_eq!(pc.radii(), [0.5, DEFAULT_RADIUS, DEFAULT_RADIUS]);
        pc.set_position(2, [1.0, 2.0, 3.0]);
        pc.resize(4);
        assert_eq!(pc.positions()[2], [1.0, 2.0, 3.0]);
        assert_eq!(pc.positions()[3], [0.0; 3]);
        assert_eq!(pc.radii().len(), 4);
        assert!(
            pc.set_attribute("x", AttrData::Float(vec![0.0; 2].into()))
                .is_err()
        );
    }
}
