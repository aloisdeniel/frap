// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_torus_add` geometry (ARCHITECTURE §6.1, tier T1).
//!
//! Ported from blender/scripts/startup/bl_operators/add_mesh_torus.py (`add_torus`, `add_uvs`,
//! `AddTorus.execute`): Python doubles for the profile and UVs, `mathutils` single-precision
//! `Matrix.Rotation(angle, 3, 'Z') @ Vector(…)` (float products summed in double), and the RNA
//! calls' attribute order (`vertices.add`, `loops.add`, `polygons.add`, `shade_flat`,
//! `uv_layers.new`, `Mesh.update()` → `mesh_calc_edges`). The operator's location/rotation only
//! place the object; the mesh never sees a creation matrix.

use std::f64::consts::PI;
use std::sync::Arc;

use super::UV_MAP_NAME;
use super::bm::math::angle_wrap_rad;
use crate::mesh::{
    AttrData, Attribute, AttributeSet, Domain, Mesh, SELECT_POLY, SELECT_VERT, SHARP_FACE, bool_attr,
};

/// `mode` of the torus operator.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TorusMode {
    /// `'MAJOR_MINOR'`: `major_radius` / `minor_radius`.
    #[default]
    MajorMinor,
    /// `'EXT_INT'`: `abso_major_rad` (exterior) / `abso_minor_rad` (interior).
    ExtInt,
}

/// Operator properties of `mesh.primitive_torus_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Torus {
    /// `major_segments`.
    pub major_segments: i32,
    /// `minor_segments`.
    pub minor_segments: i32,
    /// `mode`.
    pub mode: TorusMode,
    /// `major_radius`.
    pub major_radius: f32,
    /// `minor_radius`.
    pub minor_radius: f32,
    /// `abso_major_rad`.
    pub abso_major_rad: f32,
    /// `abso_minor_rad`.
    pub abso_minor_rad: f32,
    /// `generate_uvs`.
    pub generate_uvs: bool,
}

impl Default for Torus {
    fn default() -> Self {
        Torus {
            major_segments: 48,
            minor_segments: 12,
            mode: TorusMode::MajorMinor,
            major_radius: 1.0,
            minor_radius: 0.25,
            abso_major_rad: 1.25,
            abso_minor_rad: 0.75,
            generate_uvs: true,
        }
    }
}

impl Torus {
    /// The radii `execute` uses: in EXT_INT mode it writes `major_radius`/`minor_radius`
    /// (float properties, so the values are rounded to f32).
    pub fn radii(&self) -> (f32, f32) {
        match self.mode {
            TorusMode::MajorMinor => (self.major_radius, self.minor_radius),
            TorusMode::ExtInt => {
                let extra = (self.abso_major_rad as f64 - self.abso_minor_rad as f64) * 0.5;
                ((self.abso_minor_rad as f64 + extra) as f32, extra as f32)
            }
        }
    }
}

/// The mesh of `mesh.primitive_torus_add(…)`.
pub fn torus(p: &Torus) -> Mesh {
    let (major_rad, minor_rad) = p.radii();
    let (major_rad, minor_rad) = (major_rad as f64, minor_rad as f64);
    let major_seg = p.major_segments.max(0) as usize;
    let minor_seg = p.minor_segments.max(0) as usize;
    let pi_2 = PI * 2.0;
    let tot_verts = major_seg * minor_seg;
    let mut verts: Vec<[f32; 3]> = Vec::with_capacity(tot_verts);
    let mut faces: Vec<i32> = Vec::with_capacity(tot_verts * 4);
    let mut i1 = 0usize;
    for major_index in 0..major_seg {
        // Matrix.Rotation(angle, 3, 'Z'): angle_wrap_rad on the float angle, cosf/sinf.
        let angle = angle_wrap_rad(((major_index as f64 / major_seg as f64) * pi_2) as f32);
        let (c, s) = (libm::cosf(angle), libm::sinf(angle));
        for minor_index in 0..minor_seg {
            let a = pi_2 * minor_index as f64 / minor_seg as f64;
            let x = (major_rad + libm::cos(a) * minor_rad) as f32;
            let z = (libm::sin(a) * minor_rad) as f32;
            // column_vector_multiplication: float products accumulated in double.
            let row = |m0: f32, m1: f32, m2: f32| -> f32 {
                let mut d = 0.0f64;
                d += (m0 * x) as f64;
                d += (m1 * 0.0) as f64;
                d += (m2 * z) as f64;
                d as f32
            };
            verts.push([row(c, -s, 0.0), row(s, c, 0.0), row(0.0, 0.0, 1.0)]);
            let (i2, i3, i4);
            if minor_index + 1 == minor_seg {
                i2 = major_index * minor_seg;
                i3 = i1 + minor_seg;
                i4 = i2 + minor_seg;
            } else {
                i2 = i1 + 1;
                i3 = i1 + minor_seg;
                i4 = i3 + 1;
            }
            let wrap = |i: usize| if i >= tot_verts { i - tot_verts } else { i };
            faces.extend([i1, wrap(i3), wrap(i4), wrap(i2)].map(|i| i as i32));
            i1 += 1;
        }
    }
    let nf = faces.len() / 4;
    let mut attrs = AttributeSet::default();
    if !verts.is_empty() {
        attrs.insert(bool_attr(SELECT_VERT, Domain::Point, vec![true; verts.len()]));
        attrs.insert(Attribute {
            name: "position".into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(verts)),
        });
    }
    if !faces.is_empty() {
        attrs.insert(Attribute {
            name: ".corner_vert".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(faces.clone())),
        });
        attrs.insert(Attribute {
            name: ".corner_edge".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(vec![0; faces.len()])),
        });
        attrs.insert(bool_attr(SELECT_POLY, Domain::Face, vec![true; nf]));
    }
    attrs.insert(bool_attr(SHARP_FACE, Domain::Face, vec![true; nf]));
    let generate_uvs = p.generate_uvs;
    if generate_uvs {
        attrs.insert(Attribute {
            name: UV_MAP_NAME.into(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uvs(minor_seg, major_seg))),
        });
    }
    let offsets: Vec<i32> = (0..=nf as i32).map(|f| f * 4).collect();
    let mut mesh = Mesh::from_parts("Torus", offsets, attrs);
    if generate_uvs {
        mesh.uv_active = Some(UV_MAP_NAME.into());
        mesh.uv_active_render = Some(UV_MAP_NAME.into());
    }
    if nf > 0 {
        mesh.calc_edges(false, true);
    }
    mesh
}

/// `add_uvs(mesh, minor_seg, major_seg)`: per quad, corners 0, 1, 3, 2 at
/// (u_prev, v_prev), (u_next, v_prev), (u_prev, v_next), (u_next, v_next), with wrap-around.
fn uvs(minor_seg: usize, major_seg: usize) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0f32; 2]; minor_seg * major_seg * 4];
    if minor_seg == 0 || major_seg == 0 {
        return out;
    }
    let u_step = 1.0 / major_seg as f64;
    let v_step = 1.0 / minor_seg as f64;
    let u_init = 0.5 + 0.5 % u_step;
    let v_init = 0.5 + 0.5 % v_step;
    let u_wrap = 1.0 - (u_step / 2.0);
    let v_wrap = 1.0 - (v_step / 2.0);
    let mut vertex_index = 0usize;
    let mut u_prev = u_init;
    let mut u_next = u_prev + u_step;
    for _ in 0..major_seg {
        let mut v_prev = v_init;
        let mut v_next = v_prev + v_step;
        for _ in 0..minor_seg {
            let l = vertex_index * 4;
            out[l] = [u_prev as f32, v_prev as f32];
            out[l + 1] = [u_next as f32, v_prev as f32];
            out[l + 3] = [u_prev as f32, v_next as f32];
            out[l + 2] = [u_next as f32, v_next as f32];
            v_prev = if v_next > v_wrap { v_next - 1.0 } else { v_next };
            v_next = v_prev + v_step;
            vertex_index += 1;
        }
        u_prev = if u_next > u_wrap { u_next - 1.0 } else { u_next };
        u_next = u_prev + u_step;
    }
    out
}
