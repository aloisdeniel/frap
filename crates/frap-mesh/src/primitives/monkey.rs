// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_monkey_add` geometry, Suzanne (ARCHITECTURE §6.1, tier T1; licence
//! decision D1 = GPL, so Blender's table is ported).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (`bmo_create_monkey_exec`
//! and its tables, in `monkey/data.rs`) and editmesh_add.cc (`add_primitive_monkey_exec`: the
//! creation matrix's 3×3 part is multiplied by `size / 2`; no scale).

mod data;

use self::data::{MONKEYF, MONKEYNF, MONKEYNV, MONKEYO, MONKEYUVS, MONKEYV};
use super::Matrix4;
use super::bm::Bm;
use super::bm::math::mul_m4_v3;
use crate::mesh::Mesh;

/// Operator properties of `mesh.primitive_monkey_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Monkey {
    /// `size`.
    pub size: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix (before the `size / 2` factor; no scale for this operator).
    pub matrix: Matrix4,
}

impl Default for Monkey {
    fn default() -> Self {
        Monkey {
            size: 2.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl Monkey {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        // Suzanne: fixed topology.
        super::estimate(Some(507), Some(1005), Some(500), Some(1968))
    }
}

/// The mesh of `mesh.primitive_monkey_add(…)`.
pub fn monkey(p: &Monkey) -> Mesh {
    // mul_mat3_m4_fl(creation_mat, size / 2)
    let dia = p.size / 2.0;
    let mut mat = p.matrix;
    for col in mat.iter_mut().take(3) {
        for x in col.iter_mut().take(3) {
            *x *= dia;
        }
    }
    let mut bm = Bm::new();
    let mut tv = vec![0u32; MONKEYNV * 2];
    for i in 0..MONKEYNV {
        let m = MONKEYV[i];
        // Rotate to face the -Y axis (double arithmetic, stored as float).
        let mut v = [
            ((m[0] as i32 + 127) as f64 / 128.0) as f32,
            (m[2] as f64 / -128.0) as f32,
            (m[1] as f64 / 128.0) as f32,
        ];
        tv[i] = bm.vert_create(v);
        v[0] = -v[0];
        if v[0].abs() < 0.001 {
            tv[MONKEYNV + i] = tv[i];
        } else {
            tv[MONKEYNV + i] = bm.vert_create(mul_m4_v3(&mat, v));
        }
        let co = bm.co(tv[i]);
        *bm.co_mut(tv[i]) = mul_m4_v3(&mat, co);
    }
    let mut uvi = 0usize;
    let mut next_uvs = |n: usize| -> Vec<[f32; 2]> {
        let out = (0..n)
            .map(|k| [MONKEYUVS[(uvi + k) * 2], MONKEYUVS[(uvi + k) * 2 + 1]])
            .collect();
        uvi += n;
        out
    };
    for (i, f) in MONKEYF.iter().enumerate().take(MONKEYNF) {
        let idx = |k: usize| (f[k] as i32 + i as i32 - MONKEYO) as usize;
        let quad = f[3] != f[2];
        let a: Vec<u32> = if quad {
            vec![tv[idx(0)], tv[idx(1)], tv[idx(2)], tv[idx(3)]]
        } else {
            vec![tv[idx(0)], tv[idx(1)], tv[idx(2)]]
        };
        let b: Vec<u32> = if quad {
            vec![
                tv[MONKEYNV + idx(2)],
                tv[MONKEYNV + idx(1)],
                tv[MONKEYNV + idx(0)],
                tv[MONKEYNV + idx(3)],
            ]
        } else {
            vec![tv[MONKEYNV + idx(2)], tv[MONKEYNV + idx(1)], tv[MONKEYNV + idx(0)]]
        };
        let fa = bm.face_create(&a);
        let fb = bm.face_create(&b);
        if p.calc_uvs {
            let ua = next_uvs(a.len());
            bm.faces.get_mut(fa).expect("live").uvs = ua;
            let ub = next_uvs(b.len());
            bm.faces.get_mut(fb).expect("live").uvs = ub;
        }
    }
    bm.to_mesh("Suzanne", p.calc_uvs)
}
