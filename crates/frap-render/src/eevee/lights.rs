// SPDX-License-Identifier: GPL-3.0-or-later
//! `RLight` → GPU light records (WP 5.1): EEVEE's light units and shape parameters, ported
//! from `eevee_light.cc` (`Light::sync`, `shape_parameters_set`, `shape_radiance_get`,
//! `point_radiance_get`) so that a light of `energy` watts lights a diffuse surface as Blender
//! 5.2's EEVEE does (verified by the diffuse-plane goldens of
//! `tests/golden/cases/render_eevee/core/`).
//!
//! - colour: `energy × 2^exposure` (the builder folds the exposure) `× color`, times the
//!   blackbody colour of `temperature` with `use_temperature`, times the emitter area when
//!   `normalize` is off (`BKE_light_area`);
//! - radiance per unit colour (`power[]`): spheres and disks `1 / (4π r² · π)`, rectangles and
//!   ellipses `1 / (π · area)`, suns `(1 + 1/tan²(angle/2)) / π` (irradiance → radiance), each
//!   times `diffuse/specular/transmission_factor`; volumes are evaluated as points;
//! - point and spot lights with `use_soft_falloff` are disks facing the shaded point,
//!   otherwise spheres; spots add the cone mask (`spot_size`, `spot_blend`, object scale);
//! - local lights fade out at the influence radius `sqrt(max_power / light_threshold)`
//!   (or `cutoff_distance` with `use_custom_distance`).
//!
//! The shader evaluates each light with the linearly transformed cosines of
//! `eevee_light_lib`/`eevee_ltc_lib` (`shaders/eevee_forward.wgsl`).

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

use frap_render_scene::{RLight, RLightKind};

/// EEVEE's `eLightType` (values as in `eevee_light_shared.hh`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum LightType {
    /// `LIGHT_SUN`
    Sun = 0,
    /// `LIGHT_OMNI_SPHERE` (point light without soft falloff)
    OmniSphere = 10,
    /// `LIGHT_OMNI_DISK` (point light, soft falloff)
    OmniDisk = 11,
    /// `LIGHT_SPOT_SPHERE`
    SpotSphere = 12,
    /// `LIGHT_SPOT_DISK`
    SpotDisk = 13,
    /// `LIGHT_RECT` (square and rectangle area lights)
    Rect = 20,
    /// `LIGHT_ELLIPSE` (disk and ellipse area lights)
    Ellipse = 21,
}

impl LightType {
    /// `is_sun_light`
    pub fn is_sun(self) -> bool {
        self == LightType::Sun
    }
    /// `is_area_light`
    pub fn is_area(self) -> bool {
        matches!(self, LightType::Rect | LightType::Ellipse)
    }
    /// `is_spot_light`
    pub fn is_spot(self) -> bool {
        matches!(self, LightType::SpotSphere | LightType::SpotDisk)
    }
    /// `is_point_light` (omni and spot)
    pub fn is_point(self) -> bool {
        matches!(
            self,
            LightType::OmniSphere
                | LightType::OmniDisk
                | LightType::SpotSphere
                | LightType::SpotDisk
        )
    }
}

/// `to_light_type`
pub fn light_type(kind: &RLightKind, use_soft_falloff: bool) -> LightType {
    match kind {
        RLightKind::Sun { .. } => LightType::Sun,
        RLightKind::Spot { .. } if use_soft_falloff => LightType::SpotDisk,
        RLightKind::Spot { .. } => LightType::SpotSphere,
        RLightKind::Area { shape, .. } if matches!(shape.as_str(), "DISK" | "ELLIPSE") => {
            LightType::Ellipse
        }
        RLightKind::Area { .. } => LightType::Rect,
        _ if use_soft_falloff => LightType::OmniDisk,
        _ => LightType::OmniSphere,
    }
}

/// One light as EEVEE syncs it (`eevee::Light` after `sync`), in Blender units.
#[derive(Clone, Debug, PartialEq)]
pub struct LightRecord {
    /// EEVEE light type.
    pub kind: LightType,
    /// Object matrix with unit axes (scale removed, handedness made consistent).
    pub object_to_world: Mat4,
    /// The scale removed from the object matrix.
    pub scale: Vec3,
    /// `energy × color` (× blackbody, × area without `normalize`).
    pub color: Vec3,
    /// Radiance scale per closure: diffuse, specular, transmission, volume.
    pub power: [f32; 4],
    /// Influence radius of the surface lighting (local lights; 0 culls the light).
    pub influence_radius_max: f32,
    /// `1 / influence_radius²` for surfaces.
    pub influence_radius_invsqr_surface: f32,
    /// Sphere/disk radius (clamped to 0.001) or, for area lights, half the diagonal.
    pub shape_radius: f32,
    /// Radius the soft shadows are jittered over (0: hard shadows).
    pub shadow_radius: f32,
    /// Spot: `scale.z / scale.xy`.
    pub spot_size_inv: [f32; 2],
    /// Spot: `1 / blend`.
    pub spot_mul: f32,
    /// Spot: `-cos(size/2) · spot_mul`.
    pub spot_bias: f32,
    /// Spot: `tan(spot_size / 2)` (clamped below 90°).
    pub spot_tan: f32,
    /// Area: half extents (object scale applied, at least 0.003).
    pub area_size: [f32; 2],
    /// Sun: `tan(angle / 2)` clamped to `[0.001, 20]`.
    pub sun_shape_radius: f32,
    /// Sun: half angle the soft shadows are jittered over.
    pub sun_shadow_angle: f32,
    /// `shadow_filter_radius`
    pub filter_radius: f32,
    /// `use_shadow` (the scene's `use_shadows` is applied by the shadow planner).
    pub use_shadow: bool,
}

/// Blender's `max_ff(1e-16, light_threshold)`.
fn threshold(t: f32) -> f32 {
    t.max(1e-16)
}

/// `Light::attenuation_radius_get`
fn attenuation_radius(l: &RLight, light_threshold: f32, power: f32) -> f32 {
    if let Some(d) = l.eevee.cutoff_distance {
        return d;
    }
    (power / threshold(light_threshold)).sqrt()
}

/// `BKE_light_area`
pub fn light_area(l: &RLight) -> f32 {
    match &l.kind {
        RLightKind::Area { shape, size } => {
            let m = Mat3::from_mat4(l.matrix_world);
            let sx = size[0] * m.x_axis.length();
            let rect_or_ellipse = matches!(shape.as_str(), "RECTANGLE" | "ELLIPSE");
            let sy = if rect_or_ellipse { size[1] } else { size[0] } * m.y_axis.length();
            let mut area = sx * sy;
            if matches!(shape.as_str(), "DISK" | "ELLIPSE") {
                area *= std::f32::consts::FRAC_PI_4;
            }
            area
        }
        RLightKind::Sun { angle } => {
            let a = angle / 2.0;
            if a > 0.0 {
                std::f32::consts::PI * a.sin() * a.sin()
            } else {
                1.0
            }
        }
        _ => {
            if l.radius > 0.0 {
                4.0 * std::f32::consts::PI * l.radius * l.radius
            } else {
                4.0
            }
        }
    }
}

/// `BKE_light_color`
pub fn light_color(l: &RLight) -> Vec3 {
    let mut c = Vec3::from(l.color);
    if l.eevee.use_temperature {
        c *= Vec3::from(blackbody(l.eevee.temperature));
    }
    c
}

/// `Light::sync` (no light linking, every visibility flag on).
pub fn sync(l: &RLight, light_threshold: f32) -> LightRecord {
    let kind = light_type(&l.kind, l.eevee.use_soft_falloff);
    let mut color = l.energy * light_color(l);
    if !l.eevee.normalize {
        color *= light_area(l);
    }
    // `normalize_and_get_size` of the 3 x 3 part; consistent handedness.
    let m = l.matrix_world;
    let mut x = m.x_axis.truncate();
    let mut y = m.y_axis.truncate();
    let mut z = m.z_axis.truncate();
    let scale = Vec3::new(x.length(), y.length(), z.length());
    x = x.normalize_or_zero();
    y = y.normalize_or_zero();
    z = z.normalize_or_zero();
    if x.cross(y).dot(z) < 0.0 {
        y = -y;
    }
    let object_to_world = Mat4::from_cols(
        x.extend(0.0),
        y.extend(0.0),
        z.extend(0.0),
        m.w_axis.truncate().extend(1.0),
    );

    let mut r = LightRecord {
        kind,
        object_to_world,
        scale,
        color,
        power: [0.0; 4],
        influence_radius_max: 0.0,
        influence_radius_invsqr_surface: 0.0,
        shape_radius: 0.0,
        shadow_radius: 0.0,
        spot_size_inv: [1.0, 1.0],
        spot_mul: 0.0,
        spot_bias: 1.0,
        spot_tan: 0.0,
        area_size: [0.0, 0.0],
        sun_shape_radius: 0.0,
        sun_shadow_angle: 0.0,
        filter_radius: l.eevee.shadow_filter_radius,
        use_shadow: l.use_shadow,
    };

    // `shape_parameters_set`
    if !kind.is_sun() {
        let max_power = light_color(l).max_element() * (l.energy / 100.0).abs();
        let e = &l.eevee;
        let surface_max_power = e.diffuse_factor.max(e.specular_factor) * max_power;
        let volume_max_power = e.volume_factor * max_power;
        let rs = attenuation_radius(l, light_threshold, surface_max_power);
        let rv = attenuation_radius(l, light_threshold, volume_max_power);
        r.influence_radius_max = rs.max(rv);
        r.influence_radius_invsqr_surface = if rs != 0.0 { 1.0 / (rs * rs) } else { 0.0 };
    }
    match &l.kind {
        RLightKind::Sun { angle } => {
            let half = angle.min(179.9f32.to_radians()) / 2.0;
            r.sun_shadow_angle = if half > 0.0 { half.max(1e-8) } else { 0.0 };
            r.sun_shape_radius = half.tan().clamp(0.001, 20.0);
        }
        RLightKind::Area { shape, size } => {
            let irregular = matches!(shape.as_str(), "RECTANGLE" | "ELLIPSE");
            let mut s = [size[0], if irregular { size[1] } else { size[0] }];
            s[0] *= scale.x / 2.0;
            s[1] *= scale.y / 2.0;
            r.shadow_radius = s[0].hypot(s[1]);
            if s[0] * s[1] < 0.00001 {
                r.influence_radius_max = 0.0;
            }
            r.area_size = [s[0].max(0.003), s[1].max(0.003)];
            r.shape_radius = (r.area_size[0].hypot(r.area_size[1]) / 2.0).max(0.001);
        }
        RLightKind::Spot { size, blend } => {
            let spot_size = (size * 0.5).cos();
            let spot_blend = (1.0 - spot_size) * blend;
            r.spot_size_inv = [scale.z / scale.x.max(1e-8), scale.z / scale.y.max(1e-8)];
            r.spot_mul = 1.0 / spot_blend.max(1e-8);
            r.spot_bias = -spot_size * r.spot_mul;
            r.spot_tan = (size * 0.5).min(std::f32::consts::FRAC_PI_2 - 0.0001).tan();
            point_shape(&mut r, l.radius);
        }
        _ => point_shape(&mut r, l.radius),
    }

    let shape_power = shape_radiance(&r);
    let point_power = point_radiance(&r);
    let e = &l.eevee;
    r.power = [
        e.diffuse_factor * shape_power,
        e.specular_factor * shape_power,
        e.transmission_factor * shape_power,
        e.volume_factor * point_power,
    ];
    r
}

fn point_shape(r: &mut LightRecord, radius: f32) {
    r.shadow_radius = if radius > 0.0 { radius.max(1e-8) } else { 0.0 };
    r.shape_radius = radius.max(0.001);
}

/// `Light::shape_radiance_get`: radiance per unit of `color`.
pub fn shape_radiance(r: &LightRecord) -> f32 {
    use std::f32::consts::{FRAC_1_PI, PI};
    match r.kind {
        LightType::Rect | LightType::Ellipse => {
            let mut area = r.area_size[0] * r.area_size[1] * 4.0;
            if r.kind == LightType::Ellipse {
                area *= PI / 4.0;
            }
            FRAC_1_PI / area
        }
        LightType::Sun => {
            let inv_sin_sq = 1.0 + 1.0 / (r.sun_shape_radius * r.sun_shape_radius);
            FRAC_1_PI * inv_sin_sq
        }
        _ => {
            let area = 4.0 * PI * r.shape_radius * r.shape_radius;
            1.0 / (area * PI)
        }
    }
}

/// `Light::point_radiance_get` (volumes).
pub fn point_radiance(r: &LightRecord) -> f32 {
    use std::f32::consts::{FRAC_1_PI, FRAC_PI_2, PI};
    match r.kind {
        LightType::Rect | LightType::Ellipse => {
            let area = r.area_size[0] * r.area_size[1] * 4.0;
            let tmp = FRAC_PI_2 / (FRAC_PI_2 + area.sqrt());
            let mrp_scaling = tmp + (1.0 - tmp) * FRAC_1_PI;
            FRAC_1_PI * mrp_scaling
        }
        LightType::Sun => 1.0,
        _ => 1.0 / (4.0 * PI),
    }
}

/// Shadow kind of a GPU light record.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ShadowKind {
    /// No shadow.
    None = 0,
    /// Six cube faces around the (jittered) light position.
    Cube = 1,
    /// One perspective map along the spot axis.
    Spot = 2,
    /// Four orthographic cascades along the sun direction.
    Cascades = 3,
}

/// Where a light's shadow maps are in the shadow atlas.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShadowBinding {
    /// Kind.
    pub kind: ShadowKind,
    /// First shadow map (and matrix) of the light: see [`super::shadows::Plan`].
    pub first_layer: u32,
    /// Shadow origin of this sample (cube and spot maps).
    pub origin: Vec3,
    /// World size of one texel at unit distance (perspective maps).
    pub texel_scale: f32,
    /// Sun: far end of each cascade (view-space distance).
    pub cascade_far: [f32; 4],
    /// Sun: world size of one texel of each cascade.
    pub cascade_texel: [f32; 4],
}

impl ShadowBinding {
    /// No shadow.
    pub const NONE: ShadowBinding = ShadowBinding {
        kind: ShadowKind::None,
        first_layer: 0,
        origin: Vec3::ZERO,
        texel_scale: 0.0,
        cascade_far: [0.0; 4],
        cascade_texel: [0.0; 4],
    };
}

/// The light record the forward shader reads (`eevee_forward.wgsl` `Light`, 16-byte aligned
/// rows).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuLight {
    /// Unit object axes and position (columns of the object matrix).
    pub x_axis: [f32; 4],
    /// .
    pub y_axis: [f32; 4],
    /// .
    pub z_axis: [f32; 4],
    /// .
    pub position: [f32; 4],
    /// `color` (rgb).
    pub color: [f32; 4],
    /// Diffuse, specular, transmission, volume radiance scales.
    pub power: [f32; 4],
    /// `influence_radius_invsqr_surface`, `influence_radius_max`, `shape_radius`,
    /// `shadow_radius`.
    pub local: [f32; 4],
    /// `spot_size_inv.xy`, `spot_mul`, `spot_bias`.
    pub spot: [f32; 4],
    /// Area half size xy, sun shape radius, sun shadow angle.
    pub shape: [f32; 4],
    /// Shadow origin xyz, texel scale.
    pub shadow_origin: [f32; 4],
    /// Sun cascades: far distances.
    pub cascade_far: [f32; 4],
    /// Sun cascades: texel world sizes.
    pub cascade_texel: [f32; 4],
    /// `shadow_filter_radius`, unused ×3.
    pub shadow_params: [f32; 4],
    /// Type, shadow kind, first shadow layer, unused.
    pub ints: [u32; 4],
}

impl LightRecord {
    /// The GPU record with this sample's shadow binding.
    pub fn gpu(&self, shadow: &ShadowBinding) -> GpuLight {
        let m = &self.object_to_world;
        GpuLight {
            x_axis: m.x_axis.to_array(),
            y_axis: m.y_axis.to_array(),
            z_axis: m.z_axis.to_array(),
            position: m.w_axis.to_array(),
            color: self.color.extend(0.0).to_array(),
            power: self.power,
            local: [
                self.influence_radius_invsqr_surface,
                self.influence_radius_max,
                self.shape_radius,
                self.shadow_radius,
            ],
            spot: [
                self.spot_size_inv[0],
                self.spot_size_inv[1],
                self.spot_mul,
                self.spot_bias,
            ],
            shape: [
                self.area_size[0],
                self.area_size[1],
                self.sun_shape_radius,
                self.sun_shadow_angle,
            ],
            shadow_origin: shadow.origin.extend(shadow.texel_scale).to_array(),
            cascade_far: shadow.cascade_far,
            cascade_texel: shadow.cascade_texel,
            shadow_params: [self.filter_radius, 0.0, 0.0, 0.0],
            ints: [self.kind as u32, shadow.kind as u32, shadow.first_layer, 0],
        }
    }

    /// Whether the light can light anything (`influence_radius_max > 0` for local lights,
    /// some power).
    pub fn is_visible(&self) -> bool {
        let lit = self.color.max_element() > 0.0 && self.power[..2].iter().any(|p| *p > 0.0);
        lit && (self.kind.is_sun() || self.influence_radius_max > 0.0)
    }

    /// The light position.
    pub fn position(&self) -> Vec3 {
        self.object_to_world.w_axis.truncate()
    }

    /// The light's local +Z axis (it emits along -Z).
    pub fn z_axis(&self) -> Vec3 {
        self.object_to_world.z_axis.truncate()
    }
}

// Ported from blender/source/blender/imbuf/intern/colormanagement.cc
// (`blackbody_temperature_to_rec709`, v5.2.4): the same table as frap-scene's
// `Light::temperature_color` (verified against Blender there), which this crate cannot name
// (§19: frap-render never depends on frap-scene).
#[allow(clippy::excessive_precision)]
const BLACKBODY_R: [[f32; 3]; 7] = [
    [1.61919106e+03, -2.05010916e-03, 5.02995757e+00],
    [2.48845471e+03, -1.11330907e-03, 3.22621544e+00],
    [3.34143193e+03, -4.86551192e-04, 1.76486769e+00],
    [4.09461742e+03, -1.27446582e-04, 7.25731635e-01],
    [4.67028036e+03, 2.91258199e-05, 1.26703442e-01],
    [4.59509185e+03, 2.87495649e-05, 1.50345020e-01],
    [3.78717450e+03, 9.35907826e-06, 3.99075871e-01],
];
#[allow(clippy::excessive_precision)]
const BLACKBODY_G: [[f32; 3]; 7] = [
    [-4.88999748e+02, 6.04330754e-04, -7.55807526e-02],
    [-7.55994277e+02, 3.16730098e-04, 4.78306139e-01],
    [-1.02363977e+03, 1.20223470e-04, 9.36662319e-01],
    [-1.26571316e+03, 4.87340896e-06, 1.27054498e+00],
    [-1.42529332e+03, -4.01150431e-05, 1.43972784e+00],
    [-1.17554822e+03, -2.16378048e-05, 1.30408023e+00],
    [-5.00799571e+02, -4.59832026e-06, 1.09098763e+00],
];
#[allow(clippy::excessive_precision)]
const BLACKBODY_B: [[f32; 4]; 7] = [
    [
        5.96945309e-11,
        -4.85742887e-08,
        -9.70622247e-05,
        -4.07936148e-03,
    ],
    [
        2.40430366e-11,
        5.55021075e-08,
        -1.98503712e-04,
        2.89312858e-02,
    ],
    [
        -1.40949732e-11,
        1.89878968e-07,
        -3.56632824e-04,
        9.10767778e-02,
    ],
    [
        -3.61460868e-11,
        2.84822009e-07,
        -4.93211319e-04,
        1.56723440e-01,
    ],
    [
        -1.97075738e-11,
        1.75359352e-07,
        -2.50542825e-04,
        -2.22783266e-02,
    ],
    [
        -1.61997957e-13,
        -1.64216008e-08,
        3.86216271e-04,
        -7.38077418e-01,
    ],
    [
        6.72650283e-13,
        -2.73078809e-08,
        4.24098264e-04,
        -7.52335691e-01,
    ],
];

/// `IMB_colormanagement_blackbody_temperature_to_rgb` (Rec.709 = frap's scene linear),
/// clamped to `[0, FLT_MAX]`.
#[allow(clippy::excessive_precision)]
pub fn blackbody(t: f32) -> [f32; 3] {
    let rec709 = if t >= 12000.0 {
        [0.8262954810464208, 0.9945080501520986, 1.566307710274283]
    } else if t < 800.0 {
        [5.413294490189271, -0.20319390035873933, -0.0822535242887164]
    } else {
        let i = if t >= 6365.0 {
            6
        } else if t >= 3315.0 {
            5
        } else if t >= 1902.0 {
            4
        } else if t >= 1449.0 {
            3
        } else if t >= 1167.0 {
            2
        } else if t >= 965.0 {
            1
        } else {
            0
        };
        let (r, g, b) = (&BLACKBODY_R[i], &BLACKBODY_G[i], &BLACKBODY_B[i]);
        let t_inv = 1.0 / t;
        [
            r[0] * t_inv + r[1] * t + r[2],
            g[0] * t_inv + g[1] * t + g[2],
            ((b[0] * t + b[1]) * t + b[2]) * t + b[3],
        ]
    };
    rec709.map(|c| c.clamp(0.0, f32::MAX))
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use frap_render_scene::RLightEevee;

    use super::*;

    fn light(kind: RLightKind, energy: f32, radius: f32) -> RLight {
        RLight {
            kind,
            color: [1.0, 1.0, 1.0],
            energy,
            radius,
            use_shadow: true,
            matrix_world: Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0)),
            eevee: RLightEevee::default(),
        }
    }

    /// Irradiance at distance `d` on the light axis, from the radiance and the solid angle of
    /// the shape (what the LTC integral gives for a surface facing the light).
    #[test]
    fn point_light_irradiance_is_power_over_4pi_d2() {
        let r = sync(&light(RLightKind::Point, 100.0, 0.1), 0.01);
        assert_eq!(r.kind, LightType::OmniDisk);
        let radiance = r.color.x * r.power[0];
        // A disk of radius r facing the point at distance d: E = π L r² / (r² + d²).
        let (rad, d) = (0.1f32, 2.0f32);
        let e = PI * radiance * rad * rad / (rad * rad + d * d);
        let want = 100.0 / (4.0 * PI * (rad * rad + d * d));
        assert!((e - want).abs() / want < 1e-5, "{e} vs {want}");
    }

    #[test]
    fn sun_radiance_gives_strength_as_irradiance() {
        let r = sync(
            &light(RLightKind::Sun { angle: 0.00918043 }, 3.0, 0.0),
            0.01,
        );
        assert_eq!(r.kind, LightType::Sun);
        let radiance = r.color.x * r.power[0];
        // Disk of half angle a: E = π L sin²(a), and the shape radius is tan(a).
        let a = r.sun_shape_radius.atan();
        let e = PI * radiance * a.sin() * a.sin();
        assert!((e - 3.0).abs() < 1e-3, "{e}");
    }

    #[test]
    fn area_light_flux_is_energy() {
        let mut l = light(
            RLightKind::Area {
                shape: "SQUARE".into(),
                size: [2.0, 2.0],
            },
            50.0,
            0.0,
        );
        let r = sync(&l, 0.01);
        assert_eq!(r.kind, LightType::Rect);
        assert_eq!(r.area_size, [1.0, 1.0]);
        // Flux of a Lambertian emitter: π L A = energy.
        let flux = PI * r.color.x * r.power[0] * 4.0;
        assert!((flux - 50.0).abs() < 1e-4, "{flux}");
        l.kind = RLightKind::Area {
            shape: "DISK".into(),
            size: [2.0, 2.0],
        };
        let r = sync(&l, 0.01);
        assert_eq!(r.kind, LightType::Ellipse);
        let flux = PI * r.color.x * r.power[0] * PI;
        assert!((flux - 50.0).abs() < 1e-4, "{flux}");
        // Without `normalize` the energy is per unit area.
        l.eevee.normalize = false;
        let r = sync(&l, 0.01);
        assert!((r.color.x - 50.0 * PI).abs() < 1e-4);
    }

    #[test]
    fn spot_cone_parameters() {
        let r = sync(
            &light(
                RLightKind::Spot {
                    size: 90f32.to_radians(),
                    blend: 0.15,
                },
                100.0,
                0.25,
            ),
            0.01,
        );
        assert_eq!(r.kind, LightType::SpotDisk);
        let cos_half = (45f32).to_radians().cos();
        let blend = (1.0 - cos_half) * 0.15;
        assert!((r.spot_mul - 1.0 / blend).abs() < 1e-3);
        assert!((r.spot_bias + cos_half / blend).abs() < 1e-3);
        assert!((r.spot_tan - 1.0).abs() < 1e-5);
        assert_eq!(r.shadow_radius, 0.25);
    }

    #[test]
    fn influence_radius_follows_threshold() {
        let r = sync(&light(RLightKind::Point, 1000.0, 0.1), 0.01);
        // max_power = 1000 / 100 = 10; sqrt(10 / 0.01).
        assert!((r.influence_radius_max - 1000f32.sqrt()).abs() < 1e-3);
        let mut l = light(RLightKind::Point, 1000.0, 0.1);
        l.eevee.cutoff_distance = Some(5.0);
        assert_eq!(sync(&l, 0.01).influence_radius_max, 5.0);
        // Hard falloff: sphere.
        l.eevee.use_soft_falloff = false;
        assert_eq!(sync(&l, 0.01).kind, LightType::OmniSphere);
    }

    #[test]
    fn temperature_and_factors() {
        let mut l = light(RLightKind::Point, 10.0, 0.1);
        l.eevee.use_temperature = true;
        l.eevee.temperature = 6500.0;
        l.eevee.specular_factor = 0.5;
        let r = sync(&l, 0.01);
        let bb = blackbody(6500.0);
        assert!((r.color.x - 10.0 * bb[0]).abs() < 1e-5);
        assert!((r.power[1] - 0.5 * r.power[0]).abs() < 1e-9);
        // Blender 5.2.4 `Light.temperature_color` at 1000 K.
        let k1000 = blackbody(1000.0);
        assert!((k1000[0] - 4.601_361_3).abs() < 1e-4 && (k1000[1] - 0.039_041_907).abs() < 1e-5);
    }

    #[test]
    fn scaled_light_axes_are_unit() {
        let mut l = light(
            RLightKind::Area {
                shape: "RECTANGLE".into(),
                size: [1.0, 0.5],
            },
            10.0,
            0.0,
        );
        l.matrix_world = Mat4::from_scale(Vec3::new(2.0, 3.0, -1.0));
        let r = sync(&l, 0.01);
        assert!((r.object_to_world.x_axis.length() - 1.0).abs() < 1e-6);
        let (x, y, z) = (
            r.object_to_world.x_axis.truncate(),
            r.object_to_world.y_axis.truncate(),
            r.object_to_world.z_axis.truncate(),
        );
        assert!(x.cross(y).dot(z) > 0.0);
        assert_eq!(r.area_size, [1.0, 0.75]);
        let g = r.gpu(&ShadowBinding::NONE);
        assert_eq!(g.ints[0], LightType::Rect as u32);
        assert_eq!(std::mem::size_of::<GpuLight>(), 14 * 16);
    }
}
