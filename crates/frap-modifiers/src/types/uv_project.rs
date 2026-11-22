// SPDX-License-Identifier: GPL-3.0-or-later
//! UV_PROJECT (`UVProjectModifier`): writes a UV map by projecting the world-space vertices
//! through up to ten projector objects (`projectors[i].object`, the first `projector_count`):
//! a camera projects with its view (perspective, orthographic, or panoramic through
//! `ProjCameraInfo`), any other object with its inverse world matrix; with several projectors
//! each face takes the one whose Z axis it faces most. `aspect_x/y` and `scale_x/y` adjust
//! camera projectors.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_uvproject.cc,
//! blender/source/blender/blenkernel/intern/uvproject.cc (`BKE_uvproject_camera_info`,
//! `BKE_uvproject_camera_info_scale`, `BKE_uvproject_from_camera`) and
//! blender/source/blender/blenkernel/intern/camera.cc (`BKE_camera_params_from_object`,
//! `BKE_camera_params_compute_viewplane`, `BKE_camera_params_compute_matrix`, with
//! `perspective_m4`/`orthographic_m4` of blenlib/intern/math_geom.cc), Blender 5.2.4,
//! GPL-2.0-or-later.

use frap_geom::normals::face_normal_calc;
use frap_mesh::{AttrData, Attribute, Domain};
use frap_rna::{CameraId, IdAny, IdRef, ObjectId};
use frap_scene::generated::modifier_settings::UVProjectSettings;
use frap_scene::generated::plain::CameraFields;
use smallvec::SmallVec;
use std::sync::Arc;

use super::warp::shared::{eig_invert_m4, inverted};
use super::weld::util::{M4, m4_from_glam, mul_m4_m4m4, mul_m4_v3, unit_m4};
use crate::prelude::*;

fn uv_project_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::UvProject(u) = s {
        v.extend(u.projectors.iter().flatten().copied());
    }
    v
}

/// One projector: its object's world matrix and, for a camera, its data.
pub(crate) struct ProjectorInput {
    /// The projector's world matrix.
    pub world: M4,
    /// The camera data when the projector is a camera object.
    pub camera: Option<CameraFields>,
}

#[modifier(
    UV_PROJECT,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = uv_project_deps,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::UvProject(s) = &md.settings else {
        return Ok(mesh);
    };
    let count = s.projector_count.clamp(1, 10) as usize;
    let mut projectors = Vec::new();
    for p in s.projectors.iter().take(count) {
        let Some(r) = p.and_then(IdRef::<ObjectId>::from_any) else {
            continue;
        };
        let Some(ob) = cx.main().objects.get(r) else {
            continue;
        };
        let camera = (ob.object_type() == "CAMERA")
            .then(|| {
                ob.data
                    .and_then(IdRef::<CameraId>::from_any)
                    .and_then(|c| cx.main().cameras.get(c))
                    .map(|c| c.fields.clone())
            })
            .flatten();
        let world = m4_from_glam(cx.object_matrix_world(r));
        projectors.push(ProjectorInput { world, camera });
    }
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    uv_project(s, &projectors, &ob_world, &mut mesh);
    Ok(mesh)
}

/// `ProjCameraInfo`
struct ProjCameraInfo {
    camangle: f32,
    camsize: f32,
    xasp: f32,
    yasp: f32,
    shiftx: f32,
    shifty: f32,
    caminv: M4,
    do_persp: bool,
    do_pano: bool,
}

/// `BKE_uvproject_camera_info(ob, nullptr, winx, winy)`; `None` for a singular matrix.
fn camera_info(cam: &CameraFields, world: &M4, winx: f32, winy: f32) -> Option<ProjCameraInfo> {
    let do_pano = cam.r#type == "PANO";
    let do_persp = matches!(cam.r#type.as_str(), "PERSP" | "CUSTOM");
    // `focallength_to_fov(lens, sensor_x) / 2`
    let camangle = 2.0 * libm::atanf((cam.sensor_width / 2.0) / cam.lens) / 2.0;
    let camsize = if do_persp {
        libm::tanf(camangle)
    } else {
        cam.ortho_scale
    };
    // Scaled cameras: `normalize_m4`, then `invert_m4`.
    let mut caminv = *world;
    for c in caminv.iter_mut().take(3) {
        let (n, l) = frap_math::matrix::normalize_v3([c[0], c[1], c[2]]);
        c[0] = n[0];
        c[1] = n[1];
        c[2] = n[2];
        if l != 0.0 {
            c[3] /= l;
        }
    }
    let caminv = eig_invert_m4(&caminv)?;
    let (xasp, yasp) = if winx > winy {
        (1.0, winx / winy)
    } else {
        (winy / winx, 1.0)
    };
    Some(ProjCameraInfo {
        camangle,
        camsize,
        xasp,
        yasp,
        // 0.5 moves the UVs into the centre.
        shiftx: 0.5 - cam.shift_x * xasp,
        shifty: 0.5 - cam.shift_y * yasp,
        caminv,
        do_persp,
        do_pano,
    })
}

/// `mul_m4_v4`
fn mul_m4_v4(m: &M4, v: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = v;
    [
        x * m[0][0] + y * m[1][0] + z * m[2][0] + m[3][0] * w,
        x * m[0][1] + y * m[1][1] + z * m[2][1] + m[3][1] * w,
        x * m[0][2] + y * m[1][2] + z * m[2][2] + m[3][2] * w,
        x * m[0][3] + y * m[1][3] + z * m[2][3] + m[3][3] * w,
    ]
}

/// `BKE_uvproject_from_camera`
fn uv_from_camera(source: [f32; 3], uci: &ProjCameraInfo) -> [f32; 2] {
    let mut pv4 = mul_m4_v4(&uci.caminv, [source[0], source[1], source[2], 1.0]);
    let mut target = [0.0f32; 2];
    if uci.do_pano {
        // The angle around the camera.
        let angle = libm::atan2f(pv4[0], -pv4[2]) / (std::f32::consts::PI * 2.0);
        if !uci.do_persp {
            target[0] = angle;
            target[1] = pv4[1] / uci.camsize;
        } else {
            let len = (pv4[0] * pv4[0] + pv4[2] * pv4[2]).sqrt();
            target[0] = angle * (std::f32::consts::PI / uci.camangle);
            target[1] = pv4[1] / (len * (uci.camsize * 2.0));
        }
    } else {
        if pv4[2] == 0.0 {
            // No division by zero.
            pv4[2] = 0.00001;
        }
        if !uci.do_persp {
            target[0] = pv4[0] / uci.camsize;
            target[1] = pv4[1] / uci.camsize;
        } else {
            target[0] = (-pv4[0] * ((1.0 / uci.camsize) / pv4[2])) / 2.0;
            target[1] = (-pv4[1] * ((1.0 / uci.camsize) / pv4[2])) / 2.0;
        }
    }
    target[0] *= uci.xasp;
    target[1] *= uci.yasp;
    // Camera shift + 0.5.
    target[0] += uci.shiftx;
    target[1] += uci.shifty;
    target
}

/// `BKE_camera_params_*` for a camera projector: the window matrix of a 1×1 window with
/// aspect `aspx`/`aspy`, the view plane scaled by `scax`/`scay`.
fn camera_winmat(cam: &CameraFields, aspx: f32, aspy: f32, scax: f32, scay: f32) -> M4 {
    // `BKE_camera_params_init` + `BKE_camera_params_from_object`.
    let is_ortho = cam.r#type == "ORTHO";
    let lens = cam.lens.max(1e-9);
    let ortho_scale = cam.ortho_scale.max(1e-9);
    let clip_start = cam.clip_start;
    let clip_end = cam.clip_end.max(cam.clip_start + 1e-3);
    let zoom = 1.0f32;
    // `BKE_camera_params_compute_viewplane(params, 1, 1, aspx, aspy)`.
    let (winx, winy) = (1.0f32, 1.0f32);
    let ycor = aspy / aspx;
    let mut pixsize = if is_ortho {
        ortho_scale
    } else {
        let sensor_size = if cam.sensor_fit == "VERTICAL" {
            cam.sensor_height
        } else {
            cam.sensor_width
        };
        (sensor_size * clip_start) / lens
    };
    let fit = match cam.sensor_fit.as_str() {
        "AUTO" => {
            if aspx * winx >= aspy * winy {
                "HORIZONTAL"
            } else {
                "VERTICAL"
            }
        }
        f => f,
    };
    let viewfac = if fit == "HORIZONTAL" {
        winx
    } else {
        ycor * winy
    };
    pixsize /= viewfac;
    pixsize *= zoom;
    let dx = cam.shift_x * viewfac;
    let dy = cam.shift_y * viewfac;
    let mut vp = [
        (-0.5 * winx + dx) * pixsize,
        (0.5 * winx + dx) * pixsize,
        (-0.5 * ycor * winy + dy) * pixsize,
        (0.5 * ycor * winy + dy) * pixsize,
    ];
    // Scale the view plane.
    vp[0] *= scax;
    vp[1] *= scax;
    vp[2] *= scay;
    vp[3] *= scay;
    let [left, right, bottom, top] = vp;
    // `BKE_camera_params_compute_matrix`.
    let mut m = [[0.0f32; 4]; 4];
    let xd = right - left;
    let yd = top - bottom;
    let zd = clip_end - clip_start;
    if xd == 0.0 || yd == 0.0 || zd == 0.0 {
        return m;
    }
    if is_ortho {
        // `orthographic_m4`
        m = unit_m4();
        m[0][0] = 2.0 / xd;
        m[3][0] = -(right + left) / xd;
        m[1][1] = 2.0 / yd;
        m[3][1] = -(top + bottom) / yd;
        m[2][2] = -2.0 / zd;
        m[3][2] = -(clip_end + clip_start) / zd;
    } else {
        // `perspective_m4`
        m[0][0] = clip_start * 2.0 / xd;
        m[1][1] = clip_start * 2.0 / yd;
        m[2][0] = (right + left) / xd;
        m[2][1] = (top + bottom) / yd;
        m[2][2] = -(clip_end + clip_start) / zd;
        m[2][3] = -1.0;
        m[3][2] = (-2.0 * clip_start * clip_end) / zd;
    }
    m
}

/// A projector ready to project (`Projector`).
struct Projector {
    projmat: M4,
    normal: [f32; 3],
    uci: Option<ProjCameraInfo>,
}

/// `mul_project_m4_v3_zfac`
fn zfac(m: &M4, co: [f32; 3]) -> f32 {
    m[0][3] * co[0] + m[1][3] * co[1] + m[2][3] * co[2] + m[3][3]
}

/// The UV map the modifier writes (`get_uv_attribute`): `uv_layer`, or the active UV map
/// (`Float2` without one), created when missing; a name taken by another kind of attribute
/// gets a unique name.
fn uv_attribute(mesh: &mut Mesh, md_name: &str) -> smol_str::SmolStr {
    let requested: smol_str::SmolStr = if md_name.is_empty() {
        match mesh.uv_active.clone() {
            Some(n) if mesh.attributes().get(&n).is_some() => n,
            _ => "Float2".into(),
        }
    } else {
        md_name.into()
    };
    let n = mesh.corners_num();
    match mesh
        .attributes()
        .get(&requested)
        .map(|a| (a.domain, &a.data))
    {
        Some((Domain::Corner, AttrData::Float2(_))) => return requested,
        Some(_) => {}
        None => {
            mesh.attributes_mut().insert(Attribute {
                name: requested.clone(),
                domain: Domain::Corner,
                data: AttrData::Float2(Arc::new(vec![[0.0; 2]; n])),
            });
            return requested;
        }
    }
    let name = mesh.unique_attribute_name(&requested);
    mesh.attributes_mut().insert(Attribute {
        name: name.clone(),
        domain: Domain::Corner,
        data: AttrData::Float2(Arc::new(vec![[0.0; 2]; n])),
    });
    name
}

// Ported from blender/source/blender/modifiers/intern/MOD_uvproject.cc (uvprojectModifier_do).
/// Projects the UV map of `mesh` (object world matrix `ob_world`) through `projectors` (the
/// set ones among the first `projector_count`, in slot order).
pub(crate) fn uv_project(
    s: &UVProjectSettings,
    projectors: &[ProjectorInput],
    ob_world: &M4,
    mesh: &mut Mesh,
) {
    let nz = |v: f32| if v != 0.0 { v } else { 1.0 };
    let (aspx, aspy, scax, scay) = (nz(s.aspect_x), nz(s.aspect_y), nz(s.scale_x), nz(s.scale_y));
    if projectors.is_empty() {
        return;
    }
    let uvname = uv_attribute(mesh, &s.uv_layer);
    // A projection matrix and a world-space normal per projector.
    let projs: Vec<Projector> = projectors
        .iter()
        .map(|p| {
            let inv = inverted(&p.world);
            let mut uci = None;
            let tmpmat = match &p.camera {
                Some(cam) if cam.r#type == "PANO" => {
                    uci = camera_info(cam, &p.world, aspx, aspy).map(|mut u| {
                        // `BKE_uvproject_camera_info_scale`
                        u.xasp *= scax;
                        u.yasp *= scay;
                        u
                    });
                    inv
                }
                Some(cam) => mul_m4_m4m4(&camera_winmat(cam, aspx, aspy, scax, scay), &inv),
                None => inv,
            };
            let mut offsetmat = unit_m4();
            for c in offsetmat.iter_mut().take(3) {
                for v in c.iter_mut().take(3) {
                    *v *= 0.5;
                }
            }
            offsetmat[3][0] = 0.5;
            offsetmat[3][1] = 0.5;
            offsetmat[3][2] = 0.5;
            let projmat = mul_m4_m4m4(&offsetmat, &tmpmat);
            // `mul_mat3_m4_v3(object_to_world, (0, 0, 1))`
            let w = &p.world;
            let normal = [
                0.0 * w[0][0] + 0.0 * w[1][0] + w[2][0],
                0.0 * w[0][1] + 0.0 * w[1][1] + w[2][1],
                0.0 * w[0][2] + 0.0 * w[1][2] + w[2][2],
            ];
            Projector {
                projmat,
                normal,
                uci,
            }
        })
        .collect();

    let positions = mesh.positions().to_vec();
    let corner_verts = mesh.corner_verts().to_vec();
    // World-space coordinates.
    let mut coords: Vec<[f32; 3]> = positions.iter().map(|&p| mul_m4_v3(ob_world, p)).collect();
    let single = projs.len() == 1;
    // One projector: project the coordinates.
    if single && projs[0].uci.is_none() {
        let m = &projs[0].projmat;
        for c in &mut coords {
            let w = zfac(m, *c).abs();
            let r = mul_m4_v3(m, *c);
            *c = [r[0] / w, r[1] / w, r[2] / w];
        }
    }
    let faces: Vec<std::ops::Range<usize>> = (0..mesh.faces_num()).map(|f| mesh.face(f)).collect();
    let Some(AttrData::Float2(uvs)) = mesh.attributes_mut().get_mut(&uvname).map(|a| &mut a.data)
    else {
        return;
    };
    let uvs = Arc::make_mut(uvs);
    for face in faces {
        let best = if single {
            &projs[0]
        } else {
            // The projector the face points at most directly.
            let no = face_normal_calc(&positions, &corner_verts[face.clone()]);
            let mut best = &projs[0];
            let mut best_dot = dot(projs[0].normal, no);
            for p in &projs[1..] {
                let d = dot(p.normal, no);
                if d > best_dot {
                    best_dot = d;
                    best = p;
                }
            }
            best
        };
        for c in face {
            let v = corner_verts[c] as usize;
            uvs[c] = if let Some(uci) = &best.uci {
                uv_from_camera(coords[v], uci)
            } else if single {
                [coords[v][0], coords[v][1]]
            } else {
                // `mul_v2_project_m4_v3`
                let m = &best.projmat;
                let w = zfac(m, coords[v]).abs();
                let co = coords[v];
                let x = m[0][0] * co[0] + m[1][0] * co[1] + m[2][0] * co[2] + m[3][0];
                let y = m[0][1] * co[0] + m[1][1] * co[1] + m[2][1] * co[2] + m[3][1];
                [x / w, y / w]
            };
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_projector_maps_local_xy() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        let s = UVProjectSettings::default();
        let p = ProjectorInput {
            world: unit_m4(),
            camera: None,
        };
        uv_project(&s, &[p], &unit_m4(), &mut m);
        let (name, uvs) = m.uv_maps()[0];
        assert_eq!(name, "Float2");
        for (c, &v) in m.corner_verts().iter().enumerate() {
            let p = m.positions()[v as usize];
            assert!((uvs[c][0] - (0.5 * p[0] + 0.5)).abs() < 1e-6);
            assert!((uvs[c][1] - (0.5 * p[1] + 0.5)).abs() < 1e-6);
        }
    }

    #[test]
    fn no_projector_leaves_the_mesh_alone() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        uv_project(&UVProjectSettings::default(), &[], &unit_m4(), &mut m);
        assert!(m.uv_maps().is_empty());
    }
}
