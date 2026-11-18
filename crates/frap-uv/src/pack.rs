// SPDX-License-Identifier: GPL-3.0-or-later
//! The island packer (`GEO_uv_pack.hh`, `blender/source/blender/geometry/intern/uv_pack.cc`):
//! [`PackParams`] = `UVPackIsland_Params`, [`PackIsland`] and [`pack_islands`].
//!
//! The packer mixes strategies exactly as Blender does: every island is first pre-rotated to
//! its minimum-area bounding box (when rotation is allowed), the islands are sorted by size,
//! then the "alpaca" strip packer, the optimal square tables, `BLI_box_pack_2d` and the
//! `xatlas`-style bitmap packer (concave/convex shapes only) each propose a layout and the
//! smallest one (in the target aspect) wins; `margin_method='FRACTION'` and partial pinning run
//! a root search on the scale. Ported from blender/source/blender/geometry/intern/uv_pack.cc
//! (Blender 5.2.4) with the `BLI_*` helpers it calls ([`convexhull`], `boxpack`, `qsort`).
//!
//! T3 invariants (WP 3.3): with `scale_to_fit`, every island ends inside the target square
//! (`[0, target_extent]` for the unit square), no two packed islands overlap, and islands keep
//! their relative scale.

mod boxpack;
pub mod convexhull;
mod optimal;
mod qsort;

use crate::param::beautify::polyfill_beautify;
use crate::param::bli::{M2, V2, compare_ff_relative, isect_tri_tri_v2, mul_m2_v2};

pub use boxpack::{BoxPack, box_pack_2d};
pub(crate) use qsort::qsort_r as qsort_bsd;

/// `eUVPackIsland_MarginMethod` (`pack_islands(margin_method=…)`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum MarginMethod {
    /// `SCALED`: the margin is multiplied by the islands' size (`calc_margin_from_aabb_length_sum`).
    #[default]
    Scaled,
    /// `ADD`: the margin is added as is.
    Add,
    /// `FRACTION`: the margin is a fraction of the final layout (root search on the scale).
    Fraction,
}

/// `eUVPackIsland_RotationMethod` (`rotate_method`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RotationMethod {
    /// `ED_UVPACK_ROTATION_NONE` (`rotate=False`).
    #[default]
    None,
    /// `ANY`
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
    Concave,
    /// `CONVEX`
    Convex,
    /// `AABB` (the `UVPackIsland_Params` default; the operators pass their own).
    #[default]
    Aabb,
}

/// `eUVPackIsland_PinMethod` (`pin_method`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PinMethod {
    /// `ED_UVPACK_PIN_NONE`: pins have no impact on packing (`pin=False`).
    #[default]
    None,
    /// `ED_UVPACK_PIN_IGNORE`: islands with pinned UVs are skipped (live unwrap).
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
            rotate_method: RotationMethod::None,
            scale_to_fit: true,
            only_selected_uvs: false,
            only_selected_faces: false,
            use_seams: false,
            correct_aspect: false,
            pin_method: PinMethod::None,
            pin_unselected: false,
            merge_overlap: false,
            margin: 0.001,
            margin_method: MarginMethod::Scaled,
            udim_base_offset: [0.0, 0.0],
            target_extent: 1.0,
            target_aspect_y: 1.0,
            shape_method: ShapeMethod::Aabb,
        }
    }
}

/// `PackIsland`: one island's triangles in UV space and the transform the packer chose.
///
/// After [`pack_islands`] returned `scale`, the caller maps every UV `uv` of the island with
/// `mul_v2_m2_add_v2v2(uv, build_transformation(island_scale, angle), uv, pre_translate)`
/// where `island_scale` is `scale` when [`PackIsland::can_scale`] (else 1).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PackIsland {
    /// `aspect_y`
    pub aspect_y: f32,
    /// `pinned`
    pub pinned: bool,
    /// `pre_translate` (output)
    pub pre_translate: [f32; 2],
    /// `angle` (output, radians)
    pub angle: f32,
    /// `caller_index` (unchanged by the packer)
    pub caller_index: i32,
    /// Triangle vertices (`add_triangle` / `add_polygon`), three per triangle
    /// (`triangle_vertices_`, wound clockwise by `add_triangle`).
    pub triangles: Vec<[f32; 2]>,
    /// `pivot_`: centre of the AABB.
    pivot: V2,
    /// `half_diagonal_`
    half_diagonal: V2,
    /// `pre_rotate_`
    pre_rotate: f32,
}

impl Default for PackIsland {
    fn default() -> Self {
        PackIsland::new()
    }
}

/// Blender's `0.7071f` (a rounded-up `sqrt(0.5)`, not `FRAC_1_SQRT_2`).
#[allow(clippy::approx_constant)]
const SQRT_HALF_UP: f32 = 0.7071;

/// `DEG2RADF`.
#[inline]
fn deg2radf(deg: f32) -> f32 {
    deg * (core::f64::consts::PI / 180.0) as f32
}

/// `dist_signed_squared_to_edge`.
fn dist_signed_squared_to_edge(probe: V2, uva: V2, uvb: V2) -> f32 {
    let edge = [uvb[0] - uva[0], uvb[1] - uva[1]];
    let side = [probe[0] - uva[0], probe[1] - uva[1]];
    let edge_length_squared = edge[0] * edge[0] + edge[1] * edge[1];
    if edge_length_squared < 1e-40 {
        return side[0] * side[0] + side[1] * side[1];
    }
    let numerator = edge[0] * side[1] - edge[1] * side[0];
    let numerator_ssq = if numerator >= 0.0 {
        numerator * numerator
    } else {
        -numerator * numerator
    };
    numerator_ssq / edge_length_squared
}

/// `mul_v2_m2_add_v2v2`: `mat * (a + b)` evaluated in double precision (Blender's
/// pre-translation helper, used to apply a packing transform to a UV).
pub fn mul_v2_m2_add_v2v2(mat: &[[f32; 2]; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let x = f64::from(a[0]) + f64::from(b[0]);
    let y = f64::from(a[1]) + f64::from(b[1]);
    [
        (f64::from(mat[0][0]) * x + f64::from(mat[1][0]) * y) as f32,
        (f64::from(mat[0][1]) * x + f64::from(mat[1][1]) * y) as f32,
    ]
}

/// `rctf`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Rctf {
    xmin: f32,
    xmax: f32,
    ymin: f32,
    ymax: f32,
}

impl Rctf {
    fn new(xmin: f32, xmax: f32, ymin: f32, ymax: f32) -> Rctf {
        Rctf {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }
    fn size_x(&self) -> f32 {
        self.xmax - self.xmin
    }
    fn size_y(&self) -> f32 {
        self.ymax - self.ymin
    }
    /// `BLI_rctf_init_minmax`.
    fn minmax_init() -> Rctf {
        Rctf::new(f32::MAX, -f32::MAX, f32::MAX, -f32::MAX)
    }
    /// `BLI_rctf_do_minmax_v`.
    fn do_minmax(&mut self, p: V2) {
        if p[0] < self.xmin {
            self.xmin = p[0];
        }
        if p[0] > self.xmax {
            self.xmax = p[0];
        }
        if p[1] < self.ymin {
            self.ymin = p[1];
        }
        if p[1] > self.ymax {
            self.ymax = p[1];
        }
    }
    /// `BLI_rctf_compare`.
    fn compare(&self, b: &Rctf, limit: f32) -> bool {
        (self.xmin - b.xmin).abs() <= limit
            && (self.xmax - b.xmax).abs() <= limit
            && (self.ymin - b.ymin).abs() <= limit
            && (self.ymax - b.ymax).abs() <= limit
    }
}

/// `get_aspect_scaled_extent`.
fn aspect_scaled_extent(extent: &Rctf, params: &PackParams) -> f32 {
    (extent.size_x() / params.target_aspect_y).max(extent.size_y())
}

/// `get_aspect_scaled_area`.
fn aspect_scaled_area(extent: &Rctf, params: &PackParams) -> f32 {
    (extent.size_x() / params.target_aspect_y) * extent.size_y()
}

/// `is_larger`: true if `b` is a preferred layout over `a`.
fn is_larger(a: &Rctf, b: &Rctf, params: &PackParams) -> bool {
    let extent_a = aspect_scaled_extent(a, params);
    let extent_b = aspect_scaled_extent(b, params);
    if compare_ff_relative(extent_a, extent_b, f32::EPSILON, 64) {
        let area_a = aspect_scaled_area(a, params);
        let area_b = aspect_scaled_area(b, params);
        return area_b < area_a;
    }
    extent_b < extent_a
}

/// `UVPhi`: an island's placement.
#[derive(Copy, Clone, Debug, PartialEq)]
struct UvPhi {
    translation: V2,
    rotation: f32,
}

impl Default for UvPhi {
    fn default() -> Self {
        UvPhi {
            translation: [-1.0, -1.0],
            rotation: 0.0,
        }
    }
}

impl UvPhi {
    fn is_valid(&self) -> bool {
        self.translation[0] != -1.0
    }
}

/// `angle_match`.
fn angle_match(angle: f32, target: f32) -> f32 {
    if (angle - target).abs() < deg2radf(0.1) {
        return target;
    }
    angle
}

/// `angle_wrap`: into `[-90, 90]` degrees. `M_PI_2` and `M_PI` are doubles in Blender, so
/// the sum is evaluated in double precision (`-90°` as a float wraps to `+90°`).
fn angle_wrap(angle: f32) -> f32 {
    let a = f64::from(angle);
    let t = ((a + core::f64::consts::FRAC_PI_2) / core::f64::consts::PI) as f32;
    (a - f64::from(t.floor()) * core::f64::consts::PI) as f32
}

/// `plusminus_90_angle`.
fn plusminus_90_angle(angle: f32) -> f32 {
    let mut a = angle_wrap(angle);
    a = angle_match(a, deg2radf(-90.0));
    a = angle_match(a, deg2radf(0.0));
    a = angle_match(a, deg2radf(90.0));
    a
}

/// `can_rotate_with_method`.
fn can_rotate_with_method(
    island: &PackIsland,
    params: &PackParams,
    method: RotationMethod,
) -> bool {
    if matches!(
        method,
        RotationMethod::None | RotationMethod::AxisAlignedX | RotationMethod::AxisAlignedY
    ) {
        return false;
    }
    if !island.pinned {
        return true;
    }
    !matches!(
        params.pin_method,
        PinMethod::Locked | PinMethod::Rotation | PinMethod::RotationScale
    )
}

impl PackIsland {
    /// `PackIsland::PackIsland()`: the identity transform.
    pub fn new() -> PackIsland {
        PackIsland {
            aspect_y: 1.0,
            pinned: false,
            pre_translate: [0.0, 0.0],
            angle: 0.0,
            caller_index: -31_415_927,
            triangles: Vec::new(),
            pivot: [0.0, 0.0],
            half_diagonal: [0.0, 0.0],
            pre_rotate: 0.0,
        }
    }

    /// `PackIsland::add_triangle` (stores the triangle clockwise).
    pub fn add_triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        if dist_signed_squared_to_edge(a, b, c) < 0.0 {
            self.triangles.extend_from_slice(&[a, b, c]);
        } else {
            self.triangles.extend_from_slice(&[a, c, b]);
        }
    }

    /// `PackIsland::add_polygon`: triangulates with `BLI_polyfill_calc` + beautify.
    pub fn add_polygon(&mut self, uvs: &[[f32; 2]]) {
        let n = uvs.len();
        debug_assert!(n >= 3);
        if n < 3 {
            return;
        }
        if n == 3 {
            self.add_triangle(uvs[0], uvs[1], uvs[2]);
            return;
        }
        let mut tris = frap_geom::polyfill::polyfill_calc(uvs, 0);
        polyfill_beautify(uvs, &mut tris);
        for t in tris {
            self.add_triangle(uvs[t[0] as usize], uvs[t[1] as usize], uvs[t[2] as usize]);
        }
    }

    /// `PackIsland::build_transformation(scale, angle)`: Blender's column-major `float[2][2]`
    /// (`m[col][row]`, apply with [`mul_v2_m2_add_v2v2`]).
    pub fn build_transformation(&self, scale: f32, angle: f64) -> [[f32; 2]; 2] {
        let c = libm::cos(angle);
        let s = libm::sin(angle);
        let scale = f64::from(scale);
        let aspect_y = f64::from(self.aspect_y);
        [
            [(c * scale) as f32, (-s * scale * aspect_y) as f32],
            [(s * scale / aspect_y) as f32, (c * scale) as f32],
        ]
    }

    /// `PackIsland::build_inverse_transformation(scale, angle)`.
    pub fn build_inverse_transformation(&self, scale: f32, angle: f64) -> [[f32; 2]; 2] {
        let c = libm::cos(angle);
        let s = libm::sin(angle);
        let scale = f64::from(scale);
        let aspect_y = f64::from(self.aspect_y);
        [
            [(c / scale) as f32, (s / scale * aspect_y) as f32],
            [(-s / scale / aspect_y) as f32, (c / scale) as f32],
        ]
    }

    /// `PackIsland::can_rotate_before_pack_`.
    pub fn can_rotate_before_pack(&self, params: &PackParams) -> bool {
        let mut method = params.rotate_method;
        if matches!(
            method,
            RotationMethod::AxisAlignedX | RotationMethod::AxisAlignedY
        ) {
            method = RotationMethod::AxisAligned;
        }
        can_rotate_with_method(self, params, method)
    }

    /// `PackIsland::can_rotate_`.
    pub fn can_rotate(&self, params: &PackParams) -> bool {
        can_rotate_with_method(self, params, params.rotate_method)
    }

    /// `PackIsland::can_scale_`.
    pub fn can_scale(&self, params: &PackParams) -> bool {
        if !params.scale_to_fit {
            return false;
        }
        if !self.pinned {
            return true;
        }
        !matches!(
            params.pin_method,
            PinMethod::Locked | PinMethod::Scale | PinMethod::RotationScale
        )
    }

    /// `PackIsland::can_translate_`.
    pub fn can_translate(&self, params: &PackParams) -> bool {
        if !self.pinned {
            return true;
        }
        params.pin_method != PinMethod::Locked
    }

    /// `PackIsland::calculate_pivot_`.
    fn calculate_pivot(&mut self) {
        let mut min = [f32::MAX; 2];
        let mut max = [-f32::MAX; 2];
        for p in &self.triangles {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        self.pivot = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
        self.half_diagonal = [(max[0] - min[0]) * 0.5, (max[1] - min[1]) * 0.5];
    }

    /// `PackIsland::calculate_pre_rotation_`.
    fn calculate_pre_rotation(&mut self, params: &PackParams) {
        self.pre_rotate = 0.0;
        if params.rotate_method == RotationMethod::Cardinal {
            return;
        }
        if !self.can_rotate_before_pack(params) {
            return;
        }
        let mut coords: Vec<V2> = self
            .triangles
            .iter()
            .map(|p| [p[0] * self.aspect_y, p[1]])
            .collect();
        let mut angle = -convexhull::convexhull_aabb_fit_points_2d(&coords);
        // "Stand-up" islands.
        let m = crate::param::bli::angle_to_mat2(-angle);
        for c in &mut coords {
            *c = mul_m2_v2(&m, *c);
        }
        let mut min = [f32::MAX; 2];
        let mut max = [-f32::MAX; 2];
        for p in &coords {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let diagonal = [max[0] - min[0], max[1] - min[1]];
        match params.rotate_method {
            RotationMethod::AxisAlignedX => {
                if diagonal[0] < diagonal[1] {
                    angle += deg2radf(90.0);
                }
                self.pre_rotate = angle_wrap(angle);
            }
            RotationMethod::AxisAlignedY => {
                if diagonal[0] > diagonal[1] {
                    angle += deg2radf(90.0);
                }
                self.pre_rotate = angle_wrap(angle);
            }
            _ => {
                if diagonal[1] < diagonal[0] {
                    angle += deg2radf(90.0);
                }
                self.pre_rotate = plusminus_90_angle(angle);
            }
        }
        if self.pre_rotate == 0.0 {
            return;
        }
        let m = self.build_transformation(1.0, f64::from(self.pre_rotate));
        for p in &mut self.triangles {
            *p = mul_m2_v2(&m, *p);
        }
    }

    /// `PackIsland::finalize_geometry_`.
    fn finalize_geometry(&mut self, params: &PackParams) {
        debug_assert!(self.triangles.len() >= 3);
        self.calculate_pre_rotation(params);
        if params.shape_method == ShapeMethod::Convex {
            if self.triangles.len() <= 3 {
                self.calculate_pivot();
                return;
            }
            let hull = convexhull::convexhull_2d(&self.triangles);
            if hull.len() >= 3 {
                let convex: Vec<V2> = hull.iter().map(|&i| self.triangles[i]).collect();
                self.triangles.clear();
                self.add_polygon(&convex);
            }
        }
        self.calculate_pivot();
    }

    /// `PackIsland::place_`.
    fn place(&mut self, scale: f32, phi: UvPhi) {
        self.angle = phi.rotation + self.pre_rotate;
        let inv = self.build_inverse_transformation(scale, f64::from(phi.rotation));
        let t = mul_m2_v2(&inv, phi.translation);
        self.pre_translate = [t[0] - self.pivot[0], t[1] - self.pivot[1]];
        if self.pre_rotate != 0.0 {
            let inv = self.build_inverse_transformation(1.0, f64::from(self.pre_rotate));
            self.pre_translate = mul_m2_v2(&inv, self.pre_translate);
        }
    }

    /// `PackIsland::get_diagonal_support`.
    fn diagonal_support(&self, scale: f32, rotation: f32, margin: f32) -> V2 {
        if rotation == deg2radf(-180.0) || rotation == 0.0 || rotation == deg2radf(180.0) {
            return [
                self.half_diagonal[0] * scale + margin,
                self.half_diagonal[1] * scale + margin,
            ];
        }
        if rotation == deg2radf(-90.0) || rotation == deg2radf(90.0) || rotation == deg2radf(270.0)
        {
            return [
                (self.half_diagonal[1] / self.aspect_y) * scale + margin,
                (self.half_diagonal[0] * self.aspect_y) * scale + margin,
            ];
        }
        let m = self.build_transformation(scale, f64::from(rotation));
        let d = mul_m2_v2(&m, self.half_diagonal);
        let (sx, sy) = (d[0].abs(), d[1].abs());
        [
            sx + sy * SQRT_HALF_UP + margin,
            sx * SQRT_HALF_UP + sy + margin,
        ]
    }

    /// The matrix and pre-translation that map this island's input UVs to the packed layout
    /// (`build_transformation(island_scale, angle)` with `pre_translate`, as callers apply
    /// them).
    pub fn transform_uv(&self, scale: f32, uv: [f32; 2]) -> [f32; 2] {
        let m = self.build_transformation(scale, f64::from(self.angle));
        mul_v2_m2_add_v2v2(&m, uv, self.pre_translate)
    }
}

/// `UVAABBIsland`.
#[derive(Copy, Clone, Debug)]
struct AabbIsland {
    uv_diagonal: V2,
    index: usize,
    aspect_y: f32,
}

/// `pack_islands_alpaca_turbo`.
fn alpaca_turbo(
    exclude_index: usize,
    exclude: &Rctf,
    islands: &[AabbIsland],
    target_aspect_y: f32,
    r_phis: &mut [UvPhi],
) -> Rctf {
    let mut next_u1 = exclude.xmax;
    let mut next_v1 = exclude.ymax;
    let mut zigzag = next_u1 < next_v1 * target_aspect_y;
    let mut u0 = if zigzag { next_u1 } else { 0.0 };
    let mut v0 = if zigzag { 0.0 } else { next_v1 };
    for island in islands.iter().skip(exclude_index) {
        let dsm_u = island.uv_diagonal[0];
        let dsm_v = island.uv_diagonal[1];
        let restart = if zigzag {
            next_v1 < v0 + dsm_v
        } else {
            next_u1 < u0 + dsm_u
        };
        if restart {
            zigzag = next_u1 < next_v1 * target_aspect_y;
            u0 = if zigzag { next_u1 } else { 0.0 };
            v0 = if zigzag { 0.0 } else { next_v1 };
        }
        let phi = &mut r_phis[island.index];
        phi.rotation = 0.0;
        phi.translation = [u0 + dsm_u * 0.5, v0 + dsm_v * 0.5];
        if zigzag {
            v0 += dsm_v;
            next_u1 = next_u1.max(u0 + dsm_u);
            next_v1 = next_v1.max(v0);
        } else {
            u0 += dsm_u;
            next_v1 = next_v1.max(v0 + dsm_v);
            next_u1 = next_u1.max(u0);
        }
    }
    Rctf::new(0.0, next_u1, 0.0, next_v1)
}

/// `update_hole_rotate`.
fn update_hole_rotate(
    hole: &mut V2,
    hole_diagonal: &mut V2,
    hole_rotate: &mut bool,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) {
    let hole_area = hole_diagonal[0] * hole_diagonal[1];
    let quad_area = (u1 - u0) * (v1 - v0);
    if quad_area <= hole_area {
        return;
    }
    *hole = [u0, v0];
    *hole_diagonal = [u1 - u0, v1 - v0];
    if hole_diagonal[1] < hole_diagonal[0] {
        hole_diagonal.swap(0, 1);
        *hole_rotate = true;
    } else {
        *hole_rotate = false;
    }
}

/// `pack_islands_alpaca_rotate`.
fn alpaca_rotate(
    exclude_index: usize,
    exclude: &Rctf,
    islands: &[AabbIsland],
    target_aspect_y: f32,
    r_phis: &mut [UvPhi],
) -> Rctf {
    let mut next_u1 = exclude.xmax;
    let mut next_v1 = exclude.ymax;
    let mut zigzag = next_u1 / target_aspect_y < next_v1;

    let mut hole: V2 = [0.0, 0.0];
    let mut hole_diagonal: V2 = [0.0, 0.0];
    let mut hole_rotate = false;

    let mut u0 = if zigzag { next_u1 } else { 0.0 };
    let mut v0 = if zigzag { 0.0 } else { next_v1 };

    for island in islands.iter().skip(exclude_index) {
        let phi = &mut r_phis[island.index];
        let uvdiag_x = island.uv_diagonal[0] * island.aspect_y;
        let min_dsm = uvdiag_x.min(island.uv_diagonal[1]);
        let max_dsm = uvdiag_x.max(island.uv_diagonal[1]);

        if min_dsm < hole_diagonal[0] && max_dsm < hole_diagonal[1] {
            // Place island in the hole.
            if hole_rotate == (min_dsm == island.uv_diagonal[0]) {
                phi.rotation = deg2radf(90.0);
                phi.translation = [
                    hole[0] + island.uv_diagonal[1] * 0.5 / island.aspect_y,
                    hole[1] + island.uv_diagonal[0] * 0.5 * island.aspect_y,
                ];
            } else {
                phi.rotation = 0.0;
                phi.translation = [
                    hole[0] + island.uv_diagonal[0] * 0.5,
                    hole[1] + island.uv_diagonal[1] * 0.5,
                ];
            }
            let p = [
                hole[0],
                hole[1],
                hole[0] + (if hole_rotate { max_dsm } else { min_dsm }) / island.aspect_y,
                hole[1] + (if hole_rotate { min_dsm } else { max_dsm }),
                hole[0]
                    + (if hole_rotate {
                        hole_diagonal[1]
                    } else {
                        hole_diagonal[0]
                    }),
                hole[1]
                    + (if hole_rotate {
                        hole_diagonal[0]
                    } else {
                        hole_diagonal[1]
                    }),
            ];
            hole_diagonal[0] = 0.0;
            update_hole_rotate(
                &mut hole,
                &mut hole_diagonal,
                &mut hole_rotate,
                p[0],
                p[3],
                p[4],
                p[5],
            );
            update_hole_rotate(
                &mut hole,
                &mut hole_diagonal,
                &mut hole_rotate,
                p[2],
                p[1],
                p[4],
                p[5],
            );
            continue;
        }

        let restart = if zigzag {
            next_v1 < v0 + min_dsm
        } else {
            next_u1 < u0 + min_dsm / island.aspect_y
        };
        if restart {
            update_hole_rotate(
                &mut hole,
                &mut hole_diagonal,
                &mut hole_rotate,
                u0,
                v0,
                next_u1,
                next_v1,
            );
            zigzag = next_u1 / target_aspect_y < next_v1;
            u0 = if zigzag { next_u1 } else { 0.0 };
            v0 = if zigzag { 0.0 } else { next_v1 };
        }

        if zigzag == (min_dsm == uvdiag_x) {
            phi.rotation = deg2radf(90.0);
            phi.translation = [
                u0 + island.uv_diagonal[1] * 0.5 / island.aspect_y,
                v0 + island.uv_diagonal[0] * 0.5 * island.aspect_y,
            ];
        } else {
            phi.rotation = 0.0;
            phi.translation = [
                u0 + island.uv_diagonal[0] * 0.5,
                v0 + island.uv_diagonal[1] * 0.5,
            ];
        }

        if zigzag {
            v0 += min_dsm;
            next_u1 = next_u1.max(u0 + max_dsm / island.aspect_y);
            next_v1 = next_v1.max(v0);
        } else {
            u0 += min_dsm / island.aspect_y;
            next_v1 = next_v1.max(v0 + max_dsm);
            next_u1 = next_u1.max(u0);
        }
    }
    Rctf::new(0.0, next_u1, 0.0, next_v1)
}

/// `pack_islands_fast`.
fn pack_fast(
    exclude_index: usize,
    exclude: &Rctf,
    aabbs: &[AabbIsland],
    rotate: bool,
    target_aspect_y: f32,
    r_phis: &mut [UvPhi],
) -> Rctf {
    if rotate {
        alpaca_rotate(exclude_index, exclude, aabbs, target_aspect_y, r_phis)
    } else {
        alpaca_turbo(exclude_index, exclude, aabbs, target_aspect_y, r_phis)
    }
}

/// `pack_gobel` (Frits Göbel, 1979).
fn pack_gobel(aabbs: &[AabbIsland], scale: f32, m: i32, r_phis: &mut [UvPhi]) {
    let sqrt_half = libm::sqrtf(0.5);
    for (i, aabb) in aabbs.iter().enumerate() {
        let i = i as i64;
        let phi = &mut r_phis[aabb.index];
        phi.rotation = 0.0;
        if i == 0 {
            phi.translation = [0.5 * scale, 0.5 * scale];
            continue;
        }
        let xx = ((i - 1) % i64::from(m)) as i32;
        let yy = ((i - 1) / i64::from(m)) as i32;
        phi.translation = [(xx as f32 + 0.5) * scale, (yy as f32 + 0.5) * scale];
        if xx >= yy {
            phi.translation[0] += (1.0 + sqrt_half) * scale;
        } else {
            phi.translation[1] += sqrt_half * scale;
        }
        let mm = i64::from(m) * i64::from(m + 1) + 1;
        if i == mm {
            phi.translation[0] += (m as f32 + sqrt_half) * scale;
            phi.translation[1] -= scale;
        } else if i > mm {
            phi.rotation = deg2radf(45.0);
            let t = (((i - i64::from(m) * i64::from(m + 1)) as f32 - 1.5)
                * libm::cosf(phi.rotation)
                + 1.0)
                * scale;
            phi.translation = [t, t];
        }
    }
}

/// `pack_islands_optimal_pack_table`.
#[allow(clippy::too_many_arguments)]
fn optimal_pack_table(
    table_count: usize,
    max_extent: f32,
    optimal: &[f32],
    island_count: usize,
    large_uv: f32,
    aabbs: &[AabbIsland],
    params: &PackParams,
    r_phis: &mut [UvPhi],
    r_extent: &mut Rctf,
) -> bool {
    if table_count < island_count {
        return false;
    }
    let extent = Rctf::new(0.0, large_uv * max_extent, 0.0, large_uv * max_extent);
    if is_larger(&extent, r_extent, params) {
        return false;
    }
    *r_extent = extent;
    for i in 0..island_count {
        let phi = &mut r_phis[aabbs[i].index];
        phi.translation = [optimal[i * 3] * large_uv, optimal[i * 3 + 1] * large_uv];
        phi.rotation = optimal[i * 3 + 2];
    }
    true
}

/// `pack_islands_optimal_pack`.
fn optimal_pack(
    aabbs: &[AabbIsland],
    params: &PackParams,
    r_phis: &mut [UvPhi],
    r_extent: &mut Rctf,
) {
    if params.shape_method == ShapeMethod::Aabb {
        return;
    }
    if params.target_aspect_y != 1.0 {
        return;
    }
    if params.rotate_method != RotationMethod::Any {
        return;
    }
    let mut large_uv = 0.0f32;
    for a in aabbs {
        large_uv = large_uv.max(a.uv_diagonal[0]);
        large_uv = large_uv.max(a.uv_diagonal[1]);
    }
    let mut count = aabbs.len();
    for &(table_count, max_extent, table) in &optimal::TABLES {
        optimal_pack_table(
            table_count,
            max_extent,
            table,
            count,
            large_uv,
            aabbs,
            params,
            r_phis,
            r_extent,
        );
    }
    count = match count {
        37 => 38,
        50 | 51 => 52,
        65 | 66 => 67,
        c => c,
    };
    for a in 1..20i32 {
        let n = (a * a + a + 3) as f32 + ((a - 1) as f32 * libm::sqrtf(2.0)).floor();
        let n = n as i64;
        if count as i64 == n {
            let max_uv_gobel = large_uv * (a as f32 + 1.0 + libm::sqrtf(0.5));
            let extent = Rctf::new(0.0, max_uv_gobel, 0.0, max_uv_gobel);
            if is_larger(r_extent, &extent, params) {
                *r_extent = extent;
                pack_gobel(aabbs, large_uv, a, r_phis);
            }
            return;
        }
    }
}

/// `pack_island_box_pack_2d`.
fn pack_box_pack_2d(
    aabbs: &[AabbIsland],
    params: &PackParams,
    r_phis: &mut [UvPhi],
    r_extent: &mut Rctf,
) {
    let mut boxes: Vec<BoxPack> = aabbs
        .iter()
        .map(|a| BoxPack::new(a.uv_diagonal[0] / params.target_aspect_y, a.uv_diagonal[1]))
        .collect();
    let (mut box_max_u, box_max_v) = box_pack_2d(&mut boxes);
    box_max_u *= params.target_aspect_y;
    let extent = Rctf::new(0.0, box_max_u, 0.0, box_max_v);
    if is_larger(r_extent, &extent, params) {
        *r_extent = extent;
        for (b, a) in boxes.iter().zip(aabbs) {
            let phi = &mut r_phis[a.index];
            phi.rotation = 0.0;
            phi.translation = [(b.x + b.w * 0.5) * params.target_aspect_y, b.y + b.h * 0.5];
        }
    }
}

/// `Occupancy`: the bitmap of signed distances the `xatlas` packer traces islands into.
struct Occupancy {
    bitmap_radix: i32,
    bitmap_scale_reciprocal: f32,
    bitmap: Vec<f32>,
    witness: V2,
    witness_distance: f32,
    triangle_hint: usize,
}

const TERMINAL: f32 = 1_048_576.0;

impl Occupancy {
    fn new(initial_scale: f32) -> Occupancy {
        let radix = 800;
        let mut o = Occupancy {
            bitmap_radix: radix,
            bitmap_scale_reciprocal: 1.0,
            bitmap: vec![0.0; (radix * radix) as usize],
            witness: [-1.0, -1.0],
            witness_distance: 0.0,
            triangle_hint: 0,
        };
        o.increase_scale();
        o.bitmap_scale_reciprocal = radix as f32 / initial_scale;
        o
    }

    fn increase_scale(&mut self) {
        self.bitmap_scale_reciprocal *= 0.5;
        self.clear();
    }

    fn clear(&mut self) {
        self.bitmap.fill(TERMINAL);
        self.witness = [-1.0, -1.0];
        self.witness_distance = 0.0;
        self.triangle_hint = 0;
    }

    /// `Occupancy::trace_triangle`.
    fn trace_triangle(&mut self, uv0: V2, uv1: V2, uv2: V2, margin: f32, write: bool) -> f32 {
        let x0 = uv0[0].min(uv1[0]).min(uv2[0]);
        let y0 = uv0[1].min(uv1[1]).min(uv2[1]);
        let x1 = uv0[0].max(uv1[0]).max(uv2[0]);
        let y1 = uv0[1].max(uv1[1]).max(uv2[1]);
        let spread = if write { margin * 2.0 } else { 0.0 };
        let r = self.bitmap_scale_reciprocal;
        let ix0 = (((x0 - spread) * r).floor() as i32).max(0);
        let iy0 = (((y0 - spread) * r).floor() as i32).max(0);
        let ix1 = (((x1 + spread) * r + 2.0).floor() as i32).min(self.bitmap_radix);
        let iy1 = (((y1 + spread) * r + 2.0).floor() as i32).min(self.bitmap_radix);

        let tri = FatTriangle::new(
            [uv0[0] * r, uv0[1] * r],
            [uv1[0] * r, uv1[1] * r],
            [uv2[0] * r, uv2[1] * r],
        );

        let epsilon = SQRT_HALF_UP.max(2.0 * margin * r);

        if !write
            && ix0 as f32 <= self.witness[0]
            && self.witness[0] < ix1 as f32
            && iy0 as f32 <= self.witness[1]
            && self.witness[1] < iy1 as f32
        {
            let distance = tri.signed_distance(self.witness);
            let extent = epsilon - distance - self.witness_distance;
            let pixel_round_off = -0.1f32;
            if extent > pixel_round_off {
                return extent.max(0.0);
            }
        }
        if ix0 >= ix1 {
            return -1.0;
        }

        // Iterate in the opposite direction to the outer search (witness effectiveness).
        let radix = self.bitmap_radix as usize;
        let (ix0u, ix1u) = (ix0 as usize, ix1 as usize);
        let mut y = iy1 - 1;
        while y >= iy0 {
            let row = &mut self.bitmap[y as usize * radix..][..radix];
            let py = y as f32;
            if write {
                for (x, hotspot) in row[ix0u..ix1u].iter_mut().enumerate().rev() {
                    let distance = tri.signed_distance([(ix0u + x) as f32, py]);
                    *hotspot = distance.min(*hotspot);
                }
            } else {
                for (x, &hotspot) in row[ix0u..ix1u].iter().enumerate().rev() {
                    if hotspot > epsilon {
                        continue;
                    }
                    let probe = [(ix0u + x) as f32, py];
                    let extent = epsilon - tri.signed_distance(probe) - hotspot;
                    if extent > 0.0 {
                        self.witness = probe;
                        self.witness_distance = hotspot;
                        return extent;
                    }
                }
            }
            y -= 1;
        }
        -1.0
    }

    /// `Occupancy::trace_island`.
    fn trace_island(
        &mut self,
        island: &PackIsland,
        phi: UvPhi,
        scale: f32,
        margin: f32,
        write: bool,
    ) -> f32 {
        let support = island.diagonal_support(scale, phi.rotation, margin);
        if !write && (phi.translation[0] < support[0] || phi.translation[1] < support[1]) {
            return TERMINAL;
        }
        let m = island.build_transformation(scale, f64::from(phi.rotation));
        let pivot_t = mul_m2_v2(&m, island.pivot);
        let delta = [
            phi.translation[0] - pivot_t[0],
            phi.translation[1] - pivot_t[1],
        ];
        let vert_count = island.triangles.len();
        let mut i = 0;
        while i < vert_count {
            frap_math::budget::check();
            let j = (i + self.triangle_hint) % vert_count;
            let t = |k: usize| {
                let p = mul_m2_v2(&m, island.triangles[k]);
                [p[0] + delta[0], p[1] + delta[1]]
            };
            let extent = self.trace_triangle(t(j), t(j + 1), t(j + 2), margin, write);
            if !write && extent >= 0.0 {
                self.triangle_hint = j;
                return extent;
            }
            i += 3;
        }
        -1.0
    }
}

/// A triangle prepared for `signed_distance_fat_triangle`: the edge vectors and their squared
/// lengths are computed once per triangle instead of once per pixel (the same operations in the
/// same order, so the distances are bit-identical to Blender's).
#[derive(Copy, Clone, Debug)]
struct FatTriangle {
    v: [V2; 3],
    e: [V2; 3],
    el2: [f32; 3],
}

impl FatTriangle {
    fn new(uv0: V2, uv1: V2, uv2: V2) -> FatTriangle {
        let v = [uv0, uv1, uv2];
        let e = [
            [uv1[0] - uv0[0], uv1[1] - uv0[1]],
            [uv2[0] - uv1[0], uv2[1] - uv1[1]],
            [uv0[0] - uv2[0], uv0[1] - uv2[1]],
        ];
        let el2 = e.map(|e| e[0] * e[0] + e[1] * e[1]);
        FatTriangle { v, e, el2 }
    }

    /// `dist_signed_squared_to_edge(probe, v[k], v[k + 1])`.
    #[inline]
    fn edge_ssq(&self, k: usize, probe: V2) -> f32 {
        let side = [probe[0] - self.v[k][0], probe[1] - self.v[k][1]];
        if self.el2[k] < 1e-40 {
            return side[0] * side[0] + side[1] * side[1];
        }
        let e = self.e[k];
        let numerator = e[0] * side[1] - e[1] * side[0];
        let numerator_ssq = if numerator >= 0.0 {
            numerator * numerator
        } else {
            -numerator * numerator
        };
        numerator_ssq / self.el2[k]
    }

    /// `signed_distance_fat_triangle`: negative inside, the distance to the triangle outside.
    #[inline]
    fn signed_distance(&self, probe: V2) -> f32 {
        let mut result_ssq = self
            .edge_ssq(0, probe)
            .max(self.edge_ssq(1, probe))
            .max(self.edge_ssq(2, probe));
        if result_ssq < 0.0 {
            return -(-result_ssq).sqrt();
        }
        for a in self.v {
            let d = [probe[0] - a[0], probe[1] - a[1]];
            result_ssq = result_ssq.min(d[0] * d[0] + d[1] * d[1]);
        }
        result_ssq.sqrt()
    }
}

/// `signed_distance_fat_triangle`.
#[cfg(test)]
fn signed_distance_fat_triangle(probe: V2, uv0: V2, uv1: V2, uv2: V2) -> f32 {
    let d01 = dist_signed_squared_to_edge(probe, uv0, uv1);
    let d12 = dist_signed_squared_to_edge(probe, uv1, uv2);
    let d20 = dist_signed_squared_to_edge(probe, uv2, uv0);
    let mut result_ssq = d01.max(d12).max(d20);
    if result_ssq < 0.0 {
        return -(-result_ssq).sqrt();
    }
    let l = |a: V2| {
        let d = [probe[0] - a[0], probe[1] - a[1]];
        d[0] * d[0] + d[1] * d[1]
    };
    result_ssq = result_ssq.min(l(uv0));
    result_ssq = result_ssq.min(l(uv1));
    result_ssq = result_ssq.min(l(uv2));
    result_ssq.sqrt()
}

/// `find_best_fit_for_island`.
fn find_best_fit_for_island(
    island: &PackIsland,
    scan_line: i32,
    occupancy: &mut Occupancy,
    scale: f32,
    angle_90_multiple: i32,
    margin: f32,
    target_aspect_y: f32,
) -> UvPhi {
    let bitmap_scale = 1.0 / occupancy.bitmap_scale_reciprocal;
    let sqrt_target_aspect_y = target_aspect_y.sqrt();
    let scan_line_x = (scan_line as f32 * sqrt_target_aspect_y) as i32;
    let scan_line_y = (scan_line as f32 / sqrt_target_aspect_y) as i32;

    let mut phi = UvPhi {
        rotation: deg2radf((angle_90_multiple * 90) as f32),
        ..UvPhi::default()
    };
    let support = island.diagonal_support(scale, phi.rotation, 0.0);

    let mut t = ((2.0 * support[0] + margin) * occupancy.bitmap_scale_reciprocal).ceil() as i32;
    while t < scan_line_x {
        phi.translation = [
            t as f32 * bitmap_scale - support[0],
            scan_line_y as f32 * bitmap_scale - support[1],
        ];
        let extent = occupancy.trace_island(island, phi, scale, margin, false);
        if extent < 0.0 {
            return phi;
        }
        t += 1.max(extent as i32);
    }

    t = ((2.0 * support[1] + margin) * occupancy.bitmap_scale_reciprocal).ceil() as i32;
    while t <= scan_line_y {
        phi.translation = [
            scan_line_x as f32 * bitmap_scale - support[0],
            t as f32 * bitmap_scale - support[1],
        ];
        let extent = occupancy.trace_island(island, phi, scale, margin, false);
        if extent < 0.0 {
            return phi;
        }
        t += 1.max(extent as i32);
    }
    UvPhi::default()
}

/// `guess_initial_scale`.
fn guess_initial_scale(islands: &[PackIsland], scale: f32, margin: f32) -> f32 {
    let mut sum = 1e-40f32;
    for island in islands {
        sum += island.half_diagonal[0] * 2.0 * scale + 2.0 * margin;
        sum += island.half_diagonal[1] * 2.0 * scale + 2.0 * margin;
    }
    sum.sqrt() / 6.0
}

/// `UVMinimumEnclosingSquareFinder`.
struct SquareFinder<'a> {
    margin: f32,
    params: &'a PackParams,
    best_quad: f32,
    best_angle: f32,
    best_bounds: Rctf,
    points: Vec<V2>,
    indices: Vec<usize>,
}

impl SquareFinder<'_> {
    fn update(&mut self, angle: f64) -> f32 {
        let dir = [libm::cos(angle) as f32, libm::sin(angle) as f32];
        let mut bounds = Rctf::minmax_init();
        for &i in &self.indices {
            let p = self.points[i];
            let uv = [
                p[0] * dir[0] + p[1] * dir[1],
                -p[0] * dir[1] + p[1] * dir[0],
            ];
            bounds.do_minmax(uv);
        }
        bounds.xmin -= self.margin;
        bounds.ymin -= self.margin;
        bounds.xmax += self.margin;
        bounds.ymax += self.margin;
        let current_quad = aspect_scaled_extent(&bounds, self.params);
        if self.best_quad > current_quad {
            self.best_quad = current_quad;
            self.best_angle = angle as f32;
            self.best_bounds = bounds;
        }
        current_quad
    }

    fn update_recursive(&mut self, angle0: f32, quad0: f32, angle1: f32, quad1: f32) {
        frap_math::budget::check();
        let angle_mid = (angle0 + angle1) * 0.5;
        let quad_mid = self.update(f64::from(angle_mid));
        let angle_separation = angle1 - angle0;
        if angle_separation < deg2radf(0.002) {
            return;
        }
        let search_mode = deg2radf(10.0) < angle_separation;
        if search_mode || quad0 <= quad1 {
            self.update_recursive(angle0, quad0, angle_mid, quad_mid);
        }
        if search_mode || quad1 <= quad0 {
            self.update_recursive(angle_mid, quad_mid, angle1, quad1);
        }
    }
}

/// `rotate_inside_square`.
#[allow(clippy::too_many_arguments)]
fn rotate_inside_square(
    island_indices: &[AabbIsland],
    islands: &[PackIsland],
    params: &PackParams,
    scale: f32,
    margin: f32,
    r_phis: &mut [UvPhi],
    r_extent: &mut Rctf,
) -> bool {
    if island_indices.is_empty() {
        return false;
    }
    if params.rotate_method != RotationMethod::Any {
        return false;
    }
    if params.shape_method == ShapeMethod::Aabb && island_indices.len() > 1 {
        return false;
    }
    let mut finder = SquareFinder {
        margin,
        params,
        best_quad: aspect_scaled_extent(r_extent, params) * 0.999,
        best_angle: 0.0,
        best_bounds: Rctf::default(),
        points: Vec::new(),
        indices: Vec::new(),
    };
    let aspect_y = 1.0f32;
    for a in island_indices {
        let island = &islands[a.index];
        if island.aspect_y != aspect_y {
            return false;
        }
        let island_scale = if island.can_scale(params) { scale } else { 1.0 };
        let m = island.build_transformation(island_scale, f64::from(r_phis[a.index].rotation));
        let pivot_t = mul_m2_v2(&m, island.pivot);
        let delta = [
            r_phis[a.index].translation[0] - pivot_t[0],
            r_phis[a.index].translation[1] - pivot_t[1],
        ];
        for p in &island.triangles {
            let q = mul_m2_v2(&m, *p);
            finder.points.push([q[0] + delta[0], q[1] + delta[1]]);
        }
    }
    finder.indices = convexhull::convexhull_2d(&finder.points);

    let quad_180 = finder.update(f64::from(deg2radf(-180.0)));
    finder.update_recursive(deg2radf(-180.0), quad_180, deg2radf(180.0), quad_180);

    if finder.best_angle == 0.0 {
        return false;
    }
    for a in island_indices {
        let island = &islands[a.index];
        let m = island.build_transformation(1.0, f64::from(finder.best_angle));
        let phi = &mut r_phis[a.index];
        phi.rotation += finder.best_angle;
        phi.translation = mul_m2_v2(&m, phi.translation);
        phi.translation[0] -= finder.best_bounds.xmin;
        phi.translation[1] -= finder.best_bounds.ymin;
    }
    *r_extent = Rctf::new(
        0.0,
        finder.best_bounds.size_x(),
        0.0,
        finder.best_bounds.size_y(),
    );
    true
}

/// `pack_island_xatlas`: returns the number of islands packed (0 when the layout in `r_phis`
/// was kept).
fn pack_island_xatlas(
    island_indices: &[AabbIsland],
    islands: &[PackIsland],
    scale: f32,
    margin: f32,
    params: &PackParams,
    r_phis: &mut [UvPhi],
    r_extent: &mut Rctf,
) -> usize {
    if params.shape_method == ShapeMethod::Aabb {
        return 0;
    }
    let mut phis = vec![UvPhi::default(); r_phis.len()];
    let mut occupancy = Occupancy::new(guess_initial_scale(islands, scale, margin));
    let mut extent = Rctf::new(0.0, 0.0, 0.0, 0.0);

    let square_milestone = ((island_indices.len() as f64).sqrt() / 4.0 + 2.0) as usize;

    let mut scan_line = 0i32;
    let mut traced_islands = 0usize;
    let mut i = 0usize;
    let mut placed_can_rotate = true;
    let mut increase_scale_remaining = 32i32;

    while i < island_indices.len() {
        frap_math::budget::check();
        while traced_islands < i {
            let island_index = island_indices[traced_islands].index;
            let island = &islands[island_index];
            let island_scale = if island.can_scale(params) { scale } else { 1.0 };
            occupancy.trace_island(island, phis[island_index], island_scale, margin, true);
            traced_islands += 1;
        }

        let island = &islands[island_indices[i].index];
        let mut phi = UvPhi::default();

        if !island.can_translate(params) {
            phi.translation = [
                island.pivot[0] - params.udim_base_offset[0],
                island.pivot[1] - params.udim_base_offset[1],
            ];
            phi.rotation = 0.0;
            phis[island_indices[i].index] = phi;
            i += 1;
            placed_can_rotate = false;
            continue;
        }
        let island_scale = if island.can_scale(params) { scale } else { 1.0 };

        let mut max_90_multiple = 1;
        if island.can_rotate(params) {
            if i != 0 && i < 50 {
                max_90_multiple = 4;
            }
        } else {
            placed_can_rotate = false;
        }

        for angle_90_multiple in 0..max_90_multiple {
            phi = find_best_fit_for_island(
                island,
                scan_line,
                &mut occupancy,
                island_scale,
                angle_90_multiple,
                margin,
                params.target_aspect_y,
            );
            if phi.is_valid() {
                break;
            }
        }

        if !phi.is_valid() {
            if i < 10 {
                scan_line += 1;
            } else {
                scan_line += 2;
            }
            if (scan_line as f32)
                < occupancy.bitmap_radix as f32
                    * params
                        .target_aspect_y
                        .min(1.0 / params.target_aspect_y)
                        .sqrt()
            {
                continue;
            }
            increase_scale_remaining -= 1;
            if increase_scale_remaining < 0 {
                return 0;
            }
            scan_line = 0;
            occupancy.increase_scale();
            traced_islands = 0;
            continue;
        }

        phis[island_indices[i].index] = phi;
        i += 1;

        if i == square_milestone
            && placed_can_rotate
            && rotate_inside_square(
                &island_indices[..i],
                islands,
                params,
                scale,
                margin,
                &mut phis,
                &mut extent,
            )
        {
            scan_line = 0;
            traced_islands = 0;
            occupancy.clear();
            continue;
        }

        let top_right = island.diagonal_support(island_scale, phi.rotation, margin);
        let top_right = [
            top_right[0] + phi.translation[0],
            top_right[1] + phi.translation[1],
        ];
        extent.xmax = top_right[0].max(extent.xmax);
        extent.ymax = top_right[1].max(extent.ymax);

        if !is_larger(r_extent, &extent, params) && i >= square_milestone {
            return 0;
        }

        if i < 128 || (i & 31) == 16 {
            scan_line = 0;
        } else {
            scan_line = (scan_line - 25).max(0);
        }
    }

    if !is_larger(r_extent, &extent, params) {
        return 0;
    }
    *r_extent = extent;
    for a in &island_indices[..i] {
        r_phis[a.index] = phis[a.index];
    }
    i
}

/// `pack_islands_scale_margin`: lays out `islands` at `scale` with `margin` into `r_phis` and
/// returns the size of the covering square.
fn pack_islands_scale_margin(
    islands: &[PackIsland],
    scale: f32,
    margin: f32,
    params: &PackParams,
    r_phis: &mut [UvPhi],
) -> f32 {
    let all_can_rotate = islands.iter().all(|i| i.can_rotate(params));

    let mut aabbs: Vec<AabbIsland> = islands
        .iter()
        .enumerate()
        .map(|(i, island)| {
            let island_scale = if island.can_scale(params) { scale } else { 1.0 };
            AabbIsland {
                index: i,
                uv_diagonal: [
                    island.half_diagonal[0] * 2.0 * island_scale + 2.0 * margin,
                    island.half_diagonal[1] * 2.0 * island_scale + 2.0 * margin,
                ],
                aspect_y: island.aspect_y,
            }
        })
        .collect();

    // Sort from "biggest" to "smallest" (`std::stable_sort` with a `less` predicate).
    let less = |a: &AabbIsland, b: &AabbIsland| -> bool {
        let ta = islands[a.index].can_translate(params);
        let tb = islands[b.index].can_translate(params);
        if ta != tb {
            return tb;
        }
        if all_can_rotate {
            let (mut a_u, mut a_v) = (a.uv_diagonal[0] * a.aspect_y, a.uv_diagonal[1]);
            let (mut b_u, mut b_v) = (b.uv_diagonal[0] * b.aspect_y, b.uv_diagonal[1]);
            if a_u > a_v {
                core::mem::swap(&mut a_u, &mut a_v);
            }
            if b_u > b_v {
                core::mem::swap(&mut b_u, &mut b_v);
            }
            let diff_u = a_u - b_u;
            let mut diff_v = a_v - b_v;
            diff_v += diff_u * 0.05;
            if diff_v == 0.0 {
                return diff_u > 0.0;
            }
            diff_v > 0.0
        } else {
            b.uv_diagonal[0] * b.uv_diagonal[1] < a.uv_diagonal[0] * a.uv_diagonal[1]
        }
    };
    aabbs.sort_by(|a, b| {
        if less(a, b) {
            core::cmp::Ordering::Less
        } else if less(b, a) {
            core::cmp::Ordering::Greater
        } else {
            core::cmp::Ordering::Equal
        }
    });

    // Locked islands.
    let mut locked_bounds = Rctf::default();
    let mut locked_island_count = 0usize;
    for (i, a) in aabbs.iter().enumerate() {
        let island = &islands[a.index];
        if island.can_translate(params) {
            break;
        }
        let bl = [
            island.pivot[0] - island.half_diagonal[0],
            island.pivot[1] - island.half_diagonal[1],
        ];
        let tr = [
            island.pivot[0] + island.half_diagonal[0],
            island.pivot[1] + island.half_diagonal[1],
        ];
        if i == 0 {
            locked_bounds = Rctf::new(bl[0], tr[0], bl[1], tr[1]);
        } else {
            locked_bounds.do_minmax(bl);
            locked_bounds.do_minmax(tr);
        }
        let phi = &mut r_phis[a.index];
        phi.translation = [
            island.pivot[0] - params.udim_base_offset[0],
            island.pivot[1] - params.udim_base_offset[1],
        ];
        phi.rotation = 0.0;
        locked_island_count = i + 1;
    }

    let mut alpaca_cutoff = 1024usize;
    let alpaca_cutoff_fast = 81usize;
    if params.margin_method == MarginMethod::Fraction && margin > 0.0 {
        alpaca_cutoff = alpaca_cutoff_fast;
    }
    alpaca_cutoff = alpaca_cutoff.max(locked_island_count);

    let mut slow_len = alpaca_cutoff.min(islands.len());
    // Call the "fast" packer, which can sometimes give optimal results.
    let mut extent = pack_fast(
        locked_island_count,
        &locked_bounds,
        &aabbs[..slow_len],
        all_can_rotate,
        params.target_aspect_y,
        r_phis,
    );
    let fast_extent = extent;

    if locked_island_count == 0 {
        optimal_pack(&aabbs[..slow_len], params, r_phis, &mut extent);
    }
    if locked_island_count == 0 {
        pack_box_pack_2d(&aabbs[..slow_len], params, r_phis, &mut extent);
    }
    let max_xatlas = pack_island_xatlas(
        &aabbs[..slow_len],
        islands,
        scale,
        margin,
        params,
        r_phis,
        &mut extent,
    );
    if max_xatlas != 0 {
        slow_len = max_xatlas;
    }

    if all_can_rotate && aabbs.len() > slow_len {
        rotate_inside_square(
            &aabbs[..slow_len],
            islands,
            params,
            scale,
            margin,
            r_phis,
            &mut extent,
        );
    }

    if extent.compare(&fast_extent, 0.0) {
        slow_len = slow_len.min(locked_island_count);
        extent = locked_bounds;
    }

    let final_extent = pack_fast(
        slow_len,
        &extent,
        &aabbs,
        all_can_rotate,
        params.target_aspect_y,
        r_phis,
    );
    aspect_scaled_extent(&final_extent, params)
}

/// `pack_islands_margin_fraction`: the largest scale that packs `islands` into the target
/// square (root search), placing the islands as a side effect.
fn pack_islands_margin_fraction(
    islands: &mut [PackIsland],
    margin_fraction: f32,
    rescale_margin: bool,
    params: &PackParams,
) -> f32 {
    let mut scale_low = 0.0f32;
    let mut value_low = 0.0f32;
    let mut scale_high = 0.0f32;
    let mut value_high = 0.0f32;
    let mut phis = [
        vec![UvPhi::default(); islands.len()],
        vec![UvPhi::default(); islands.len()],
    ];
    let mut phis_low: Option<usize> = None;
    let min_scale_roundoff = 1e-5f32;
    let max_iteration = 25;
    for iteration in 0..max_iteration {
        let mut scale = 1.0f32;
        if iteration == 0 {
            // First guess: the islands as they are.
        } else if scale_low == 0.0 {
            // Search mode, shrink the layout until a scale fits.
            scale = scale_high * 0.1;
        } else if scale_high == 0.0 {
            // Search mode, grow the layout until a scale does not fit.
            scale = scale_low * 10.0;
        } else {
            // Bracket mode: modified secant method on the area.
            if scale_high < scale_low * 1.0001 {
                break;
            }
            scale = (scale_low.sqrt() * value_high - scale_high.sqrt() * value_low)
                / (value_high - value_low);
            scale *= scale;
            if iteration & 1 != 0 {
                scale = (scale * (scale_low * scale_high).sqrt()).sqrt();
            }
        }
        scale = scale.max(min_scale_roundoff);

        let target = if phis_low == Some(0) { 1 } else { 0 };
        let margin = if rescale_margin {
            margin_fraction * scale
        } else {
            margin_fraction
        };
        let max_uv = pack_islands_scale_margin(islands, scale, margin, params, &mut phis[target])
            / params.target_extent;
        let value = max_uv.sqrt() - 1.0;

        if value <= 0.0 {
            scale_low = scale;
            value_low = value;
            phis_low = Some(target);
            if value == 0.0 {
                break;
            }
        } else {
            scale_high = scale;
            value_high = value;
            if scale == min_scale_roundoff {
                scale_low = scale;
                break;
            }
            if phis_low.is_none() {
                phis_low = Some(target);
            }
        }
    }
    if let Some(low) = phis_low {
        for (i, island) in islands.iter_mut().enumerate() {
            let island_scale = if island.can_scale(params) {
                scale_low
            } else {
                1.0
            };
            island.place(island_scale, phis[low][i]);
        }
    }
    scale_low
}

/// `calc_margin_from_aabb_length_sum`.
fn calc_margin_from_aabb_length_sum(islands: &[PackIsland], params: &PackParams) -> f32 {
    let mut sum = 0.0f64;
    for island in islands {
        let w = island.half_diagonal[0] * 2.0;
        let h = island.half_diagonal[1] * 2.0;
        sum += f64::from((w * h).sqrt());
    }
    // `params.margin * aabb_length_sum * 0.1f`: the float literal widened to double.
    (f64::from(params.margin) * sum * f64::from(0.1f32)) as f32
}

/// `overlap_aabb`.
fn overlap_aabb(pa: V2, ha: V2, pb: V2, hb: V2) -> bool {
    !(pa[0] + ha[0] <= pb[0] - hb[0]
        || pa[1] + ha[1] <= pb[1] - hb[1]
        || pb[0] + hb[0] <= pa[0] - ha[0]
        || pb[1] + hb[1] <= pa[1] - ha[1])
}

/// `OverlapMerger::overlap`.
fn islands_overlap(a: &PackIsland, b: &PackIsland) -> bool {
    if a.aspect_y != b.aspect_y {
        return false;
    }
    if !overlap_aabb(a.pivot, a.half_diagonal, b.pivot, b.half_diagonal) {
        return false;
    }
    for ta in a.triangles.chunks_exact(3) {
        for tb in b.triangles.chunks_exact(3) {
            if isect_tri_tri_v2(ta[0], ta[1], ta[2], tb[0], tb[1], tb[2]) {
                return true;
            }
        }
    }
    false
}

/// `OverlapMerger::merge_islands`.
fn merge_islands(a: &PackIsland, b: &PackIsland) -> PackIsland {
    let mut r = PackIsland::new();
    r.aspect_y = (a.aspect_y * b.aspect_y).sqrt();
    r.caller_index = -1;
    r.pinned = a.pinned || b.pinned;
    for t in a
        .triangles
        .chunks_exact(3)
        .chain(b.triangles.chunks_exact(3))
    {
        r.add_triangle(t[0], t[1], t[2]);
    }
    r.calculate_pivot();
    r
}

/// Island reference of the overlap merger: an input island or a merged one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Node {
    Input(usize),
    Merged(usize),
}

/// `OverlapMerger::pack_islands_overlap`.
fn pack_islands_overlap(islands: &mut [PackIsland], params: &PackParams) -> f32 {
    let mut merged: Vec<PackIsland> = Vec::new();
    let mut sub: Vec<Node> = Vec::new();
    let mut trace: Vec<(Node, Node, Node)> = Vec::new();
    for i in 0..islands.len() {
        islands[i].calculate_pivot();
        let mut island = Node::Input(i);
        let mut j = sub.len();
        while j > 0 {
            j -= 1;
            fn get<'a>(
                n: Node,
                islands: &'a [PackIsland],
                merged: &'a [PackIsland],
            ) -> &'a PackIsland {
                match n {
                    Node::Input(k) => &islands[k],
                    Node::Merged(k) => &merged[k],
                }
            }
            let (a, b) = (get(island, islands, &merged), get(sub[j], islands, &merged));
            if islands_overlap(a, b) {
                let m = merge_islands(a, b);
                merged.push(m);
                let mn = Node::Merged(merged.len() - 1);
                trace.push((island, sub[j], mn));
                island = mn;
                sub.remove(j);
            }
        }
        sub.push(island);
    }

    let mut sub_islands: Vec<PackIsland> = sub
        .iter()
        .map(|&n| match n {
            Node::Input(k) => islands[k].clone(),
            Node::Merged(k) => merged[k].clone(),
        })
        .collect();
    let sub_params = PackParams {
        merge_overlap: false,
        ..params.clone()
    };
    let result = pack_islands(&mut sub_islands, &sub_params);

    // Write the packed state back to the nodes.
    for (n, packed) in sub.iter().zip(sub_islands) {
        let dst = match *n {
            Node::Input(k) => &mut islands[k],
            Node::Merged(k) => &mut merged[k],
        };
        dst.angle = packed.angle;
        dst.pre_translate = packed.pre_translate;
        dst.pre_rotate = packed.pre_rotate;
        dst.pinned = packed.pinned;
    }
    // Must loop backwards, or we miss sub-sub-islands.
    for &(a, b, m) in trace.iter().rev() {
        let Node::Merged(mk) = m else { unreachable!() };
        let (angle, pre_translate, pre_rotate, pinned) = {
            let mi = &merged[mk];
            (mi.angle, mi.pre_translate, mi.pre_rotate, mi.pinned)
        };
        for n in [a, b] {
            let dst = match n {
                Node::Input(k) => &mut islands[k],
                Node::Merged(k) => &mut merged[k],
            };
            dst.angle = angle;
            dst.pre_translate = pre_translate;
            dst.pre_rotate = pre_rotate;
            if pinned {
                dst.pinned = true;
            }
        }
    }
    result
}

/// `pack_islands`: places every island (writing [`PackIsland::angle`] and
/// [`PackIsland::pre_translate`]) and returns the scale to apply to the islands that
/// [`PackIsland::can_scale`] (`1.0` without `scale_to_fit`).
pub fn pack_islands(islands: &mut [PackIsland], params: &PackParams) -> f32 {
    if islands.is_empty() {
        return 1.0;
    }
    if params.merge_overlap {
        return pack_islands_overlap(islands, params);
    }
    for island in islands.iter_mut() {
        island.finalize_geometry(params);
    }

    let can_scale_count = islands.iter().filter(|i| i.can_scale(params)).count();
    let can_translate_count = islands.iter().filter(|i| i.can_translate(params)).count();
    if can_translate_count == 0 {
        return 1.0;
    }

    if params.margin_method == MarginMethod::Fraction && params.margin > 0.0 && can_scale_count > 0
    {
        return pack_islands_margin_fraction(islands, params.margin, false, params);
    }

    let margin = match params.margin_method {
        MarginMethod::Add | MarginMethod::Fraction => params.margin,
        MarginMethod::Scaled => calc_margin_from_aabb_length_sum(islands, params),
    };

    if can_scale_count > 0 && can_scale_count != islands.len() {
        return pack_islands_margin_fraction(islands, margin, true, params);
    }

    let mut phis = vec![UvPhi::default(); islands.len()];
    let scale = 1.0f32;
    let max_uv = pack_islands_scale_margin(islands, scale, margin, params, &mut phis);
    let result = if can_scale_count != 0 && max_uv > 1e-14 {
        params.target_extent / max_uv
    } else {
        1.0
    };
    for (island, phi) in islands.iter_mut().zip(phis) {
        island.place(scale, phi);
    }
    result
}

/// Applies a packing result to an island's UVs: `uv = M * (uv + pre_translate)` with
/// `M = build_transformation(island_scale, angle)` and `island_scale = scale` when the island
/// [`PackIsland::can_scale`] (as `uvedit_pack_islands_multi` and `uv_parametrizer_pack` do),
/// then adds `udim_base_offset` after the transform (`base_offset`), for islands that are not
/// locked in place.
pub fn apply_to_uvs(island: &PackIsland, scale: f32, params: &PackParams, uvs: &mut [[f32; 2]]) {
    if island.pinned && params.pin_method == PinMethod::Locked {
        return;
    }
    let island_scale = if island.can_scale(params) { scale } else { 1.0 };
    let m: M2 = island.build_transformation(island_scale, f64::from(island.angle));
    let mut pre = island.pre_translate;
    if params.udim_base_offset != [0.0, 0.0]
        && let Some(inv) = crate::param::bli::invert_m2_m2(&m)
    {
        let off = mul_m2_v2(&inv, params.udim_base_offset);
        pre = [off[0] + pre[0], off[1] + pre[1]];
    }
    for uv in uvs {
        *uv = mul_v2_m2_add_v2v2(&m, *uv, pre);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, s: f32) -> PackIsland {
        let mut p = PackIsland::new();
        p.add_polygon(&[[x, y], [x + s, y], [x + s, y + s], [x, y + s]]);
        p
    }

    #[test]
    fn transforms_invert() {
        let mut p = PackIsland::new();
        p.aspect_y = 2.0;
        let m = p.build_transformation(3.0, 0.7);
        let i = p.build_inverse_transformation(3.0, 0.7);
        let v = mul_m2_v2(&i, mul_m2_v2(&m, [0.3, -1.2]));
        assert!((v[0] - 0.3).abs() < 1e-5 && (v[1] + 1.2).abs() < 1e-5);
    }

    #[test]
    fn add_triangle_winds_clockwise() {
        let mut p = PackIsland::new();
        p.add_triangle([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]);
        let t = &p.triangles;
        let area =
            (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
        assert!(area < 0.0);
    }

    #[test]
    fn single_square_fills_unit_square() {
        let mut islands = vec![square(3.0, 4.0, 2.0)];
        let params = PackParams {
            margin: 0.0,
            ..PackParams::default()
        };
        let scale = pack_islands(&mut islands, &params);
        assert!((scale - 0.5).abs() < 1e-6);
        let mut uvs = vec![[3.0, 4.0], [5.0, 6.0]];
        apply_to_uvs(&islands[0], scale, &params, &mut uvs);
        assert!(uvs[0][0].abs() < 1e-6 && uvs[0][1].abs() < 1e-6, "{uvs:?}");
        assert!((uvs[1][0] - 1.0).abs() < 1e-6 && (uvs[1][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scale_to_fit_off_keeps_scale() {
        let mut islands = vec![square(0.0, 0.0, 0.25), square(5.0, 5.0, 0.25)];
        let params = PackParams {
            scale_to_fit: false,
            ..PackParams::default()
        };
        assert_eq!(pack_islands(&mut islands, &params), 1.0);
    }

    #[test]
    fn angle_wrap_uses_double_constants() {
        // `-90°` as a float lies below `-M_PI_2` (double): Blender wraps it to `+90°`.
        let a = angle_wrap(deg2radf(-90.0));
        assert!((a - core::f32::consts::FRAC_PI_2).abs() < 1e-6, "{a}");
        assert_eq!(angle_wrap(0.25), 0.25);
    }

    #[test]
    fn fat_triangle_matches_reference() {
        let (a, b, c) = ([0.3, 0.2], [5.7, 1.1], [2.2, 4.9]);
        let t = FatTriangle::new(a, b, c);
        for y in -3..9 {
            for x in -3..9 {
                let p = [x as f32 * 0.9, y as f32 * 0.7];
                assert_eq!(
                    t.signed_distance(p).to_bits(),
                    signed_distance_fat_triangle(p, a, b, c).to_bits()
                );
            }
        }
        // Degenerate edge.
        let t = FatTriangle::new(a, a, c);
        assert_eq!(
            t.signed_distance([1.0, 1.0]).to_bits(),
            signed_distance_fat_triangle([1.0, 1.0], a, a, c).to_bits()
        );
    }

    #[test]
    fn deg2radf_matches_blender() {
        assert_eq!(deg2radf(90.0), 90.0f32 * 0.017_453_292_f32);
    }
}
