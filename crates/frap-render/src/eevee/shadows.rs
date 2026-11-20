// SPDX-License-Identifier: GPL-3.0-or-later
//! Shadow maps (WP 5.1): cube maps (point and area lights, and spots wider than 150°), one 2D
//! map (spot lights), four cascades (sun), all layers of one `Depth32Float` 2D array, reverse
//! depth (1 at the light, 0 far away). The scene's `eevee.use_shadows` and each light's
//! `use_shadow` gate them.
//!
//! Soft shadows: EEVEE 5.2 traces shadow-map rays over the light shape
//! (`shadow_ray_count`/`shadow_step_count`) or, with `use_shadow_jitter`, moves the shadow
//! origin over the light shape every sample (`shadow_setup`). frap always does the latter: each
//! TAA sample renders the maps from a point of the light shape chosen by the sample's
//! `SAMPLING_SHADOW_I/J/K` dimensions ([`super::taa::Sample::shadow`]) — a ball of
//! `shadow_soft_size` for point and spot lights, the rectangle/ellipse of area lights, the cone
//! of `angle` for suns — so the accumulated film converges to the penumbra the light shape
//! casts. Lookups are PCF-filtered over `shadow_filter_radius` texels.

use glam::{Mat4, Vec2, Vec3, Vec3Swizzles};

use frap_render_scene::RCamera;

use super::lights::{LightRecord, LightType, ShadowBinding, ShadowKind};
use super::taa::{Sample, sample_ball, sample_disk, sample_uniform_cone};

/// Depth format of the shadow maps.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Shadow-map size at `shadow_resolution_scale = 1`.
pub const BASE_SIZE: u32 = 1024;
/// Upper bound of the layers of one render (later lights render unshadowed).
pub const MAX_LAYERS: u32 = 48;
/// Spots up to this cone angle (radians) use one 2D map; wider ones a cube.
pub const SPOT_2D_MAX_ANGLE: f32 = 150.0 * std::f32::consts::PI / 180.0;
/// Number of sun cascades.
pub const CASCADES: usize = 4;
/// Blend factor between the logarithmic and the uniform cascade split.
const CASCADE_LAMBDA: f32 = 0.75;

/// An axis-aligned world bounding box.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl Aabb {
    /// The empty box.
    pub const EMPTY: Aabb = Aabb {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };
    /// Grows the box by `p`.
    pub fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    /// Whether no point was added.
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    /// The 8 corners.
    pub fn corners(&self) -> [Vec3; 8] {
        std::array::from_fn(|i| {
            Vec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            )
        })
    }
}

/// Shadow-map size of a render (`eevee.shadow_resolution_scale` scales it).
pub fn map_size(resolution_scale: f32, max_dim: u32) -> u32 {
    let s = (BASE_SIZE as f32 * resolution_scale.clamp(0.0, 1.0)).round() as u32;
    s.clamp(128, BASE_SIZE.min(max_dim))
}

/// The shadow kind of a light.
pub fn kind_of(r: &LightRecord) -> ShadowKind {
    match r.kind {
        LightType::Sun => ShadowKind::Cascades,
        LightType::SpotSphere | LightType::SpotDisk
            if 2.0 * r.spot_tan.atan() * spot_scale(r) <= SPOT_2D_MAX_ANGLE =>
        {
            ShadowKind::Spot
        }
        _ => ShadowKind::Cube,
    }
}

/// Widening of the spot cone by a non-uniform object scale.
fn spot_scale(r: &LightRecord) -> f32 {
    (1.0 / r.spot_size_inv[0].max(1e-8))
        .max(1.0 / r.spot_size_inv[1].max(1e-8))
        .max(1.0)
}

/// Number of layers of a shadow kind.
pub fn layer_count(kind: ShadowKind) -> u32 {
    match kind {
        ShadowKind::None => 0,
        ShadowKind::Cube => 6,
        ShadowKind::Spot => 1,
        ShadowKind::Cascades => CASCADES as u32,
    }
}

/// Which lights cast shadows, and their layers.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Per light: kind and first layer (`ShadowKind::None` for unshadowed lights).
    pub lights: Vec<(ShadowKind, u32)>,
    /// Total layers (at least 1: the atlas always exists).
    pub layers: u32,
}

/// Plans the shadow layers of `lights` (`use_shadows` is `eevee.use_shadows`).
pub fn plan(lights: &[LightRecord], use_shadows: bool, max_layers: u32) -> Plan {
    let max_layers = max_layers.min(MAX_LAYERS);
    let mut next = 0u32;
    let lights = lights
        .iter()
        .map(|r| {
            if !use_shadows || !r.use_shadow || !r.is_visible() {
                return (ShadowKind::None, 0);
            }
            let k = kind_of(r);
            let n = layer_count(k);
            if next + n > max_layers {
                return (ShadowKind::None, 0);
            }
            let first = next;
            next += n;
            (k, first)
        })
        .collect();
    Plan {
        lights,
        layers: next.max(1),
    }
}

/// World directions and up vectors of the cube faces: +X, -X, +Y, -Y, +Z, -Z (the shader picks
/// the face from the major axis of the world-space vector from the shadow origin).
pub const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];

/// The cube face of a world direction (index into [`CUBE_FACES`]).
pub fn cube_face(d: Vec3) -> usize {
    let a = d.abs();
    if a.x >= a.y && a.x >= a.z {
        if d.x >= 0.0 { 0 } else { 1 }
    } else if a.y >= a.z {
        if d.y >= 0.0 { 2 } else { 3 }
    } else if d.z >= 0.0 {
        4
    } else {
        5
    }
}

/// Right-handed view matrix at `eye` looking along `dir`.
pub fn look_to(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    let f = dir.normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(
        glam::Vec4::new(s.x, u.x, -f.x, 0.0),
        glam::Vec4::new(s.y, u.y, -f.y, 0.0),
        glam::Vec4::new(s.z, u.z, -f.z, 0.0),
        glam::Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed perspective projection, square aspect, clip depth `0` at `z_near` and `1` at
/// `z_far` (wgpu depth range).
fn perspective_depth(fov: f32, z_near: f32, z_far: f32) -> Mat4 {
    let f = 1.0 / (0.5 * fov).tan();
    let r = z_far / (z_near - z_far);
    Mat4::from_cols(
        glam::Vec4::new(f, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, r, -1.0),
        glam::Vec4::new(0.0, 0.0, r * z_near, 0.0),
    )
}

/// Reverse-depth perspective projection (1 at `near`, 0 at `far`).
fn perspective(fov: f32, near: f32, far: f32) -> Mat4 {
    perspective_depth(fov, far, near)
}

/// Right-handed orthographic projection, clip depth `0` at distance `z0` and `1` at `z1`.
pub fn orthographic(l: f32, r: f32, b: f32, t: f32, z0: f32, z1: f32) -> Mat4 {
    let rw = 1.0 / (r - l);
    let rh = 1.0 / (t - b);
    let rd = 1.0 / (z0 - z1);
    Mat4::from_cols(
        glam::Vec4::new(2.0 * rw, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 2.0 * rh, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, rd, 0.0),
        glam::Vec4::new(-(l + r) * rw, -(t + b) * rh, rd * z0, 1.0),
    )
}

/// Near and far clip of a local light's maps (`clip_near = influence / 4000`).
fn local_clip(r: &LightRecord, scene: &Aabb, origin: Vec3) -> (f32, f32) {
    let mut far = r.influence_radius_max;
    if !scene.is_empty() {
        let reach = scene
            .corners()
            .iter()
            .map(|c| c.distance(origin))
            .fold(0.0f32, f32::max);
        far = far.min(reach * 1.01 + 0.01);
    }
    let near = (r.influence_radius_max / 4000.0).max(far * 1e-5).max(1e-4);
    (near, far.max(near * 2.0))
}

/// The jittered shadow origin of a local light for `sample` (`shadow_setup`).
pub fn local_origin(r: &LightRecord, sample: &Sample) -> Vec3 {
    let rand = Vec3::from(sample.shadow);
    let local = if r.kind.is_area() {
        let unit = if r.kind == LightType::Rect {
            rand.truncate() * 2.0 - Vec2::ONE
        } else {
            sample_disk(rand.truncate())
        };
        (unit * Vec2::from(r.area_size)).extend(0.0)
    } else if r.shadow_radius == 0.0 {
        Vec3::ZERO
    } else {
        sample_ball(rand) * r.shape_radius
    };
    r.object_to_world.transform_point3(local)
}

/// The jittered direction towards a sun for `sample` (`shadow_setup`: the cone of the sun's
/// shape radius, reversed so the first sample points straight at the sun).
pub fn sun_direction(r: &LightRecord, sample: &Sample) -> Vec3 {
    let z = r.z_axis();
    if r.sun_shadow_angle == 0.0 {
        return z;
    }
    let rand = Vec2::ONE - Vec2::new(sample.shadow[0], sample.shadow[1]);
    let d = sample_uniform_cone(rand, r.sun_shape_radius.atan().cos());
    r.object_to_world.transform_vector3(d).normalize()
}

/// View-space depth splits of the cascades over `[near, far]` (practical split scheme).
pub fn cascade_splits(near: f32, far: f32) -> [f32; CASCADES + 1] {
    let mut s = [0.0; CASCADES + 1];
    for (i, v) in s.iter_mut().enumerate() {
        let t = i as f32 / CASCADES as f32;
        let log = near * (far / near).powf(t);
        let uni = near + (far - near) * t;
        *v = CASCADE_LAMBDA * log + (1.0 - CASCADE_LAMBDA) * uni;
    }
    s[0] = near;
    s[CASCADES] = far;
    s
}

/// The 4 world corners of the camera frustum at view depth `d` (distance along the view axis).
fn frustum_slice(camera: &RCamera, d: f32) -> [Vec3; 4] {
    let inv = (camera.projection * camera.view).inverse();
    let [cs, ce] = camera.clip;
    std::array::from_fn(|i| {
        let x = if i & 1 == 0 { -1.0 } else { 1.0 };
        let y = if i & 2 == 0 { -1.0 } else { 1.0 };
        let n = inv.project_point3(Vec3::new(x, y, 0.0));
        let f = inv.project_point3(Vec3::new(x, y, 1.0));
        let t = if ce > cs { (d - cs) / (ce - cs) } else { 0.0 };
        n + (f - n) * t
    })
}

/// The view-space depth range the cascades cover: the camera clip range cut to the scene.
pub fn cascade_range(camera: &RCamera, scene: &Aabb) -> (f32, f32) {
    let [cs, ce] = camera.clip;
    if scene.is_empty() {
        return (cs, ce);
    }
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for c in scene.corners() {
        let d = -(camera.view.transform_point3(c)).z;
        lo = lo.min(d);
        hi = hi.max(d);
    }
    let near = lo.max(cs).min(ce);
    let far = hi.min(ce).max(near + 1e-3);
    (near, far)
}

/// The shadow maps of one sample: each light's binding and the world → clip matrix of every
/// layer.
pub fn sample_maps(
    plan: &Plan,
    lights: &[LightRecord],
    sample: &Sample,
    camera: &RCamera,
    scene: &Aabb,
    size: u32,
) -> (Vec<ShadowBinding>, Vec<Mat4>) {
    let mut matrices = vec![Mat4::IDENTITY; plan.layers as usize];
    let bindings = lights
        .iter()
        .zip(&plan.lights)
        .map(|(r, &(kind, first))| {
            let mut b = ShadowBinding {
                kind,
                first_layer: first,
                ..ShadowBinding::NONE
            };
            match kind {
                ShadowKind::None => {}
                ShadowKind::Cube => {
                    let origin = local_origin(r, sample);
                    let (near, far) = local_clip(r, scene, origin);
                    let proj = perspective(std::f32::consts::FRAC_PI_2, near, far);
                    for (f, (dir, up)) in CUBE_FACES.iter().enumerate() {
                        let view = look_to(origin, *dir, *up);
                        matrices[first as usize + f] = proj * view;
                    }
                    b.origin = origin;
                    b.texel_scale = 2.0 / size as f32;
                }
                ShadowKind::Spot => {
                    let origin = local_origin(r, sample);
                    let (near, far) = local_clip(r, scene, origin);
                    // Cone (with the non-uniform scale) plus a margin for the PCF kernel.
                    let half = (r.spot_tan * spot_scale(r)).atan() * 1.04 + 0.01;
                    let fov = (2.0 * half).min(SPOT_2D_MAX_ANGLE + 0.2);
                    let view = look_to(origin, -r.z_axis(), r.object_to_world.y_axis.truncate());
                    matrices[first as usize] = perspective(fov, near, far) * view;
                    b.origin = origin;
                    b.texel_scale = 2.0 * (fov / 2.0).tan() / size as f32;
                }
                ShadowKind::Cascades => {
                    let to_light = sun_direction(r, sample);
                    let up = if to_light.z.abs() < 0.999 {
                        Vec3::Z
                    } else {
                        Vec3::Y
                    };
                    let view = look_to(Vec3::ZERO, -to_light, up);
                    let (near, far) = cascade_range(camera, scene);
                    let splits = cascade_splits(near, far);
                    // Depth range: every caster of the scene, in light space.
                    let (mut zmin, mut zmax) = (f32::INFINITY, f32::NEG_INFINITY);
                    for c in scene.corners() {
                        let z = view.transform_point3(c).z;
                        zmin = zmin.min(z);
                        zmax = zmax.max(z);
                    }
                    for c in 0..CASCADES {
                        let mut pts = frustum_slice(camera, splits[c]).to_vec();
                        pts.extend(frustum_slice(camera, splits[c + 1]));
                        let (mut lo, mut hi) =
                            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
                        for p in &pts {
                            let q = view.transform_point3(*p);
                            lo = lo.min(q.xy());
                            hi = hi.max(q.xy());
                            zmin = zmin.min(q.z);
                            zmax = zmax.max(q.z);
                        }
                        // Square texels, a border for the PCF kernel.
                        let ext = (hi - lo).max_element() * 0.5 * 1.04 + 1e-3;
                        let mid = (lo + hi) * 0.5;
                        let margin = (zmax - zmin) * 0.01 + 0.01;
                        let proj = orthographic(
                            mid.x - ext,
                            mid.x + ext,
                            mid.y - ext,
                            mid.y + ext,
                            -zmin + margin,
                            -zmax - margin,
                        );
                        matrices[first as usize + c] = proj * view;
                        b.cascade_far[c] = splits[c + 1];
                        b.cascade_texel[c] = 2.0 * ext / size as f32;
                    }
                    b.origin = to_light;
                }
            }
            b
        })
        .collect();
    (bindings, matrices)
}

/// Whether the maps of `plan` change between samples (some shadowed light has a shape to
/// jitter over); otherwise they are rendered once.
pub fn jittered(plan: &Plan, lights: &[LightRecord]) -> bool {
    lights.iter().zip(&plan.lights).any(|(r, (k, _))| {
        *k != ShadowKind::None
            && match r.kind {
                LightType::Sun => r.sun_shadow_angle > 0.0,
                _ => r.shadow_radius > 0.0 || r.kind.is_area(),
            }
    })
}

#[cfg(test)]
mod tests {
    use frap_render_scene::{RLight, RLightEevee, RLightKind};
    use glam::{Vec4, Vec4Swizzles};

    use super::*;
    use crate::eevee::lights::sync;

    fn rec(kind: RLightKind, m: Mat4, radius: f32) -> LightRecord {
        sync(
            &RLight {
                kind,
                color: [1.0; 3],
                energy: 100.0,
                radius,
                use_shadow: true,
                matrix_world: m,
                eevee: RLightEevee::default(),
            },
            0.01,
        )
    }

    fn camera() -> RCamera {
        let eye = Vec3::new(0.0, -8.0, 4.0);
        let view = look_to(eye, -eye, Vec3::Z);
        let mut projection = perspective_depth(0.69, 0.1, 100.0);
        projection.x_axis.x /= 16.0 / 9.0;
        RCamera {
            view,
            projection,
            is_perspective: true,
            clip: [0.1, 100.0],
            pixel_aspect: 1.0,
        }
    }

    fn scene() -> Aabb {
        Aabb {
            min: Vec3::new(-4.0, -4.0, 0.0),
            max: Vec3::new(4.0, 4.0, 2.0),
        }
    }

    /// Projects a world point through a layer: `(uv in [0, 1]², depth)`.
    fn project(m: &Mat4, p: Vec3) -> (Vec2, f32) {
        let c = *m * p.extend(1.0);
        let n = c.xyz() / c.w;
        (Vec2::new(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5), n.z)
    }

    #[test]
    fn plan_layers_and_limits() {
        let point = rec(RLightKind::Point, Mat4::IDENTITY, 0.1);
        let sun = rec(RLightKind::Sun { angle: 0.1 }, Mat4::IDENTITY, 0.0);
        let spot = rec(
            RLightKind::Spot {
                size: 1.0,
                blend: 0.1,
            },
            Mat4::IDENTITY,
            0.1,
        );
        let p = plan(&[point.clone(), sun.clone(), spot.clone()], true, 256);
        assert_eq!(
            p.lights,
            vec![
                (ShadowKind::Cube, 0),
                (ShadowKind::Cascades, 6),
                (ShadowKind::Spot, 10)
            ]
        );
        assert_eq!(p.layers, 11);
        // Scene shadows off: no layer, the atlas keeps one.
        let p = plan(std::slice::from_ref(&point), false, 256);
        assert_eq!(p.lights, vec![(ShadowKind::None, 0)]);
        assert_eq!(p.layers, 1);
        // Too many layers: later lights are unshadowed.
        let many = vec![point; 10];
        let p = plan(&many, true, 256);
        assert_eq!(p.layers, 48);
        assert_eq!(p.lights[8], (ShadowKind::None, 0));
        // A wide spot uses a cube.
        let wide = rec(
            RLightKind::Spot {
                size: 3.0,
                blend: 0.1,
            },
            Mat4::IDENTITY,
            0.1,
        );
        assert_eq!(kind_of(&wide), ShadowKind::Cube);
    }

    #[test]
    fn cube_faces_cover_directions() {
        let r = rec(
            RLightKind::Point,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)),
            0.0,
        );
        let p = plan(std::slice::from_ref(&r), true, 256);
        let (b, m) = sample_maps(&p, &[r], &Sample::new(0), &camera(), &scene(), 512);
        assert_eq!(b[0].origin, Vec3::new(0.0, 0.0, 3.0));
        // A point below the light lands in the -Z face, inside it, in front of the far plane.
        for target in [
            Vec3::new(0.3, -0.2, 0.0),
            Vec3::new(3.0, 0.5, 2.5),
            Vec3::new(-2.0, -3.5, 1.0),
        ] {
            let d = target - b[0].origin;
            let f = cube_face(d);
            let (uv, z) = project(&m[f], target);
            assert!(
                uv.cmpge(Vec2::ZERO).all() && uv.cmple(Vec2::ONE).all(),
                "{target} {uv}"
            );
            assert!(z > 0.0 && z < 1.0, "{z}");
        }
        // Reverse depth: nearer points have a larger depth.
        let (_, near) = project(&m[5], Vec3::new(0.0, 0.0, 2.0));
        let (_, far) = project(&m[5], Vec3::new(0.0, 0.0, 0.0));
        assert!(near > far);
    }

    #[test]
    fn spot_map_contains_the_cone() {
        let m_light = Mat4::from_translation(Vec3::new(1.0, 0.0, 4.0));
        let r = rec(
            RLightKind::Spot {
                size: 60f32.to_radians(),
                blend: 0.15,
            },
            m_light,
            0.0,
        );
        let p = plan(std::slice::from_ref(&r), true, 256);
        let (b, m) = sample_maps(&p, &[r], &Sample::new(0), &camera(), &scene(), 512);
        assert_eq!(b[0].kind, ShadowKind::Spot);
        // The cone edge on the ground (30° off the axis, 4 below) stays inside the map.
        let edge = Vec3::new(1.0 + 4.0 * 30f32.to_radians().tan(), 0.0, 0.0);
        let (uv, _) = project(&m[0], edge);
        assert!(uv.x > 0.95 && uv.x < 1.0, "{uv}");
        let (centre, _) = project(&m[0], Vec3::new(1.0, 0.0, 0.0));
        assert!((centre - Vec2::splat(0.5)).length() < 1e-4);
    }

    #[test]
    fn sun_cascades_cover_the_view() {
        let rot = Mat4::from_rotation_x(0.6);
        let r = rec(RLightKind::Sun { angle: 0.0 }, rot, 0.0);
        let p = plan(std::slice::from_ref(&r), true, 256);
        let cam = camera();
        let (b, m) = sample_maps(
            &p,
            std::slice::from_ref(&r),
            &Sample::new(0),
            &cam,
            &scene(),
            1024,
        );
        assert_eq!(b[0].kind, ShadowKind::Cascades);
        assert_eq!(b[0].origin, r.z_axis());
        // Cascades grow with distance, the last one ends at the far side of the scene.
        let far = b[0].cascade_far;
        assert!(far[0] < far[1] && far[1] < far[2] && far[2] < far[3]);
        assert!(b[0].cascade_texel[0] < b[0].cascade_texel[3]);
        // Every visible scene point is inside the cascade its view depth selects.
        for p in [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.9, 3.9, 0.0),
            Vec3::new(-3.0, -3.0, 1.5),
            Vec3::new(1.0, 2.0, 2.0),
        ] {
            let d = -(cam.view.transform_point3(p)).z;
            let c = far.iter().position(|f| d <= *f).unwrap_or(3);
            let (uv, z) = project(&m[c], p);
            assert!(
                uv.cmpge(Vec2::ZERO).all() && uv.cmple(Vec2::ONE).all(),
                "{p}: {uv}"
            );
            assert!((0.0..=1.0).contains(&z), "{p}: {z}");
        }
    }

    #[test]
    fn jitter_moves_origin_on_the_shape() {
        let m_light = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0));
        let point = rec(RLightKind::Point, m_light, 0.5);
        let area = rec(
            RLightKind::Area {
                shape: "RECTANGLE".into(),
                size: [2.0, 1.0],
            },
            m_light,
            0.0,
        );
        let hard = rec(RLightKind::Point, m_light, 0.0);
        let mut moved = false;
        for i in 0..16 {
            let s = Sample::new(i);
            let o = local_origin(&point, &s);
            assert!(o.distance(m_light.w_axis.xyz()) <= 0.5 + 1e-5);
            moved |= o.distance(m_light.w_axis.xyz()) > 0.05;
            let a = local_origin(&area, &s) - m_light.w_axis.xyz();
            assert!(a.x.abs() <= 1.0 + 1e-5 && a.y.abs() <= 0.5 + 1e-5 && a.z == 0.0);
            assert_eq!(local_origin(&hard, &s), m_light.w_axis.xyz());
        }
        assert!(moved);
        let p = plan(std::slice::from_ref(&hard), true, 256);
        assert!(!jittered(&p, &[hard]));
        let sun = rec(RLightKind::Sun { angle: 0.2 }, Mat4::IDENTITY, 0.0);
        let d = sun_direction(&sun, &Sample::new(3));
        assert!(d.dot(Vec3::Z) >= (sun.sun_shape_radius.atan()).cos() - 1e-5);
        let _ = Vec4::ZERO;
    }

    #[test]
    fn map_size_scales() {
        assert_eq!(map_size(1.0, 16384), 1024);
        assert_eq!(map_size(0.5, 16384), 512);
        assert_eq!(map_size(0.0, 16384), 128);
        assert_eq!(map_size(1.0, 512), 512);
        let s = cascade_splits(0.1, 100.0);
        assert_eq!(s[0], 0.1);
        assert_eq!(s[4], 100.0);
        assert!(s.windows(2).all(|w| w[0] < w[1]));
    }
}
