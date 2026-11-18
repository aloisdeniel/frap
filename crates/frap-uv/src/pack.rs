// SPDX-License-Identifier: GPL-3.0-or-later
//! The island packer (`GEO_uv_pack.hh`, `blender/source/blender/geometry/intern/uv_pack.cc`):
//! [`PackParams`] = `UVPackIsland_Params`, [`PackIsland`] and [`pack_islands`]. Stub bodies
//! until WP 3.3 (T3 invariants: no overlap, inside the unit square, margins).

/// `eUVPackIsland_MarginMethod` (`pack_islands(margin_method=…)`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum MarginMethod {
    /// `SCALED`
    #[default]
    Scaled,
    /// `ADD`
    Add,
    /// `FRACTION`
    Fraction,
}

/// `eUVPackIsland_RotationMethod` (`rotate_method`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RotationMethod {
    /// No rotation (`rotate=False`).
    None,
    /// `ANY`
    #[default]
    Any,
    /// `CARDINAL`
    Cardinal,
    /// `AXIS_ALIGNED`
    AxisAligned,
    /// `AXIS_ALIGNED_X`
    AxisAlignedX,
    /// `AXIS_ALIGNED_Y`
    AxisAlignedY,
}

/// `eUVPackIsland_ShapeMethod` (`shape_method`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ShapeMethod {
    /// `CONCAVE`
    #[default]
    Concave,
    /// `CONVEX`
    Convex,
    /// `AABB`
    Aabb,
}

/// `eUVPackIsland_PinMethod` (`pin_method`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PinMethod {
    /// `IGNORE`
    #[default]
    Ignore,
    /// `LOCKED` (pinned islands keep scale, rotation and position)
    Locked,
    /// `ROTATION`
    Rotation,
    /// `ROTATION_SCALE`
    RotationScale,
    /// `SCALE`
    Scale,
}

/// `UVPackIsland_Params`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PackParams {
    /// `rotate_method`
    pub rotate_method: RotationMethod,
    /// `scale_to_fit`
    pub scale_to_fit: bool,
    /// `only_selected_uvs`
    pub only_selected_uvs: bool,
    /// `only_selected_faces`
    pub only_selected_faces: bool,
    /// `use_seams`
    pub use_seams: bool,
    /// `correct_aspect`
    pub correct_aspect: bool,
    /// `pin_method`
    pub pin_method: PinMethod,
    /// `pin_unselected`
    pub pin_unselected: bool,
    /// `merge_overlap`
    pub merge_overlap: bool,
    /// `margin`
    pub margin: f32,
    /// `margin_method`
    pub margin_method: MarginMethod,
    /// `udim_base_offset`
    pub udim_base_offset: [f32; 2],
    /// `target_extent`
    pub target_extent: f32,
    /// `target_aspect_y`
    pub target_aspect_y: f32,
    /// `shape_method`
    pub shape_method: ShapeMethod,
}

impl Default for PackParams {
    /// `UVPackIsland_Params::UVPackIsland_Params()`.
    fn default() -> Self {
        PackParams {
            rotate_method: RotationMethod::Any,
            scale_to_fit: true,
            only_selected_uvs: false,
            only_selected_faces: false,
            use_seams: false,
            correct_aspect: false,
            pin_method: PinMethod::Ignore,
            pin_unselected: false,
            merge_overlap: false,
            margin: 0.001,
            margin_method: MarginMethod::Scaled,
            udim_base_offset: [0.0, 0.0],
            target_extent: 1.0,
            target_aspect_y: 1.0,
            shape_method: ShapeMethod::Concave,
        }
    }
}

/// `PackIsland`: one island's triangles in UV space and the transform the packer chose.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PackIsland {
    /// `aspect_y`
    pub aspect_y: f32,
    /// `pinned`
    pub pinned: bool,
    /// `pre_translate`
    pub pre_translate: [f32; 2],
    /// `angle`
    pub angle: f32,
    /// `caller_index`
    pub caller_index: i32,
    /// Triangle vertices (`add_triangle` / `add_polygon`), three per triangle.
    pub triangles: Vec<[f32; 2]>,
}

impl PackIsland {
    /// `PackIsland::add_triangle`.
    pub fn add_triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        self.triangles.extend_from_slice(&[a, b, c]);
    }

    /// `PackIsland::build_transformation(scale, angle)` (2x2, row-major).
    pub fn build_transformation(&self, scale: f32, angle: f64) -> [[f32; 2]; 2] {
        let _ = (scale, angle);
        todo!("WP 3.3: PackIsland::build_transformation")
    }
}

/// `pack_islands`: places every island and returns the scale applied (`1.0` without
/// `scale_to_fit`).
pub fn pack_islands(islands: &mut [PackIsland], params: &PackParams) -> f32 {
    let _ = (islands, params);
    todo!("WP 3.3: pack_islands")
}
