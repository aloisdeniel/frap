// SPDX-License-Identifier: GPL-3.0-or-later
//! `manifold-rust` backend (feature `manifold`, WP 4.6): `solver='MANIFOLD'` at T2, `'EXACT'`
//! at T3 until M8.
//!
//! MANIFOLD is a port of Blender's `mesh_boolean_manifold`
//! (blender/source/blender/geometry/intern/mesh_boolean_manifold.cc, Blender 5.2.4,
//! GPL-2.0-or-later) over manifold-rust, the pure-Rust port of the Manifold library Blender
//! links: every operand becomes a `MeshGL` of its (transformed) corner triangles with the
//! concatenated input face index as `faceID` (`get_manifold`), a non-manifold operand fails
//! with its index (`BooleanErrorType::NonManifold`) unless it is the plane of a two-operand
//! difference (`is_plane` + `mesh_trim_manifold`), the operands are combined with
//! `Manifold::BatchBoolean` (manifold-rust's CSG tree, the same reduction), and the result is
//! cleaned and reassembled into polygons (`crate::assemble`). The vertex and corner sources are
//! `OutToInMaps::ensure_vertex_map` / `ensure_corner_map`: an output vertex maps to the first
//! vertex of its face's input face with exactly the same position.
//!
//! EXACT (Blender's exact-arithmetic mesh arrangement, not ported: WP 8.6) runs on the same
//! pipeline with manifold-rust's robust engine (`BooleanEngine::Robust`: exact rational
//! arithmetic and winding numbers, so closed non-manifold soup is accepted and an inside-out
//! operand is empty space, as in Blender), as a left fold. Open operands fail with
//! [`BoolError::NonManifold`] (Blender's exact solver accepts them). Results have Blender's
//! volume but the MANIFOLD face layout (T3). `use_self` and `use_hole_tolerant` do not change
//! the result.

use crate::{BoolError, BoolOptions, BooleanSolver, SolverKind};
#[cfg(feature = "manifold")]
use crate::{BoolMesh, BoolOperation, BoolResult, Src};

/// The `manifold-rust` solver.
#[derive(Copy, Clone, Debug, Default)]
pub struct ManifoldSolver;

impl BooleanSolver for ManifoldSolver {
    fn name(&self) -> &'static str {
        "manifold-rust"
    }

    fn supports(&self, kind: SolverKind) -> bool {
        matches!(kind, SolverKind::Manifold | SolverKind::Exact)
    }

    #[cfg(not(feature = "manifold"))]
    fn run(
        &self,
        operands: &[crate::BoolMesh<'_>],
        opts: &BoolOptions,
    ) -> Result<crate::BoolResult, BoolError> {
        let _ = (operands, opts);
        Err(BoolError::Unavailable(
            "frap: built without the boolean-manifold feature".into(),
        ))
    }

    #[cfg(feature = "manifold")]
    fn run(&self, operands: &[BoolMesh<'_>], opts: &BoolOptions) -> Result<BoolResult, BoolError> {
        imp::run(operands, opts)
    }
}

#[cfg(feature = "manifold")]
mod imp {
    use manifold_rust::csg_tree::CsgNode;
    use manifold_rust::linalg::Vec3;
    use manifold_rust::manifold::Manifold;
    use manifold_rust::types::{BooleanEngine, Error, MeshGL, OpType};

    use super::*;
    use crate::assemble;

    /// `MeshOffsets` of the faces (the only element the solver indexes globally).
    struct Offsets {
        face_start: Vec<u32>,
    }

    impl Offsets {
        fn new(operands: &[BoolMesh<'_>]) -> Self {
            let mut o = Offsets {
                face_start: vec![0],
            };
            for m in operands {
                o.face_start
                    .push(o.face_start.last().unwrap() + m.faces_num() as u32);
            }
            o
        }
        /// `mesh_id_for_face`.
        fn mesh_for_face(&self, f: u32) -> usize {
            self.face_start[1..].partition_point(|&s| s <= f)
        }
    }

    /// One operand's transformed positions and triangles (`get_manifold`'s `MeshGL`).
    struct Prepared {
        positions: Vec<[f32; 3]>,
        tris: Vec<[u32; 3]>,
        face_of_tri: Vec<u32>,
    }

    fn prepare(m: &BoolMesh<'_>) -> Prepared {
        let positions = m.transformed_positions();
        let mut tris = Vec::new();
        let mut face_of_tri = Vec::new();
        let faces_num = m.faces_num();
        if !m.corner_tris.is_empty() {
            // `corner_tris` are face by face; recover each triangle's face from its corner.
            let mut f = 0usize;
            for t in m.corner_tris {
                while f + 1 < faces_num && t[0] >= m.face_offsets[f + 1] {
                    f += 1;
                }
                tris.push([
                    m.corner_verts[t[0] as usize],
                    m.corner_verts[t[1] as usize],
                    m.corner_verts[t[2] as usize],
                ]);
                face_of_tri.push(f as u32);
            }
        } else {
            for f in 0..faces_num {
                let (s, e) = (m.face_offsets[f] as usize, m.face_offsets[f + 1] as usize);
                for c in s + 1..e.saturating_sub(1) {
                    tris.push([m.corner_verts[s], m.corner_verts[c], m.corner_verts[c + 1]]);
                    face_of_tri.push(f as u32);
                }
            }
        }
        Prepared {
            positions,
            tris,
            face_of_tri,
        }
    }

    /// `get_manifold`: the operand as a `MeshGL` (positions, triangles, `faceID` = the
    /// concatenated face index, one run with `runOriginalID` = the operand index).
    fn mesh_gl(p: &Prepared, mesh_index: usize, face_start: u32) -> MeshGL {
        let mut gl = MeshGL {
            num_prop: 3,
            ..Default::default()
        };
        gl.vert_properties = p.positions.iter().flatten().copied().collect();
        gl.tri_verts = p.tris.iter().flatten().copied().collect();
        gl.face_id = p.face_of_tri.iter().map(|f| f + face_start).collect();
        gl.run_index = vec![0, 3 * p.tris.len() as u32];
        gl.run_original_id = vec![mesh_index as u32];
        gl
    }

    /// `is_plane`: a single quad whose two triangles have (relatively) equal normals; returns
    /// the normal and the plane's offset from the origin.
    fn is_plane(m: &BoolMesh<'_>) -> Option<([f32; 3], f32)> {
        if m.faces_num() != 1 || m.positions.len() != 4 {
            return None;
        }
        let corners = &m.corner_verts[m.face_offsets[0] as usize..m.face_offsets[1] as usize];
        if corners.len() != 4 {
            return None;
        }
        let v: Vec<[f32; 3]> = corners
            .iter()
            .map(|&c| crate::transform_point(&m.transform, m.positions[c as usize]))
            .collect();
        let n1 = normal_tri(v[0], v[1], v[2]);
        let n2 = normal_tri(v[0], v[2], v[3]);
        let almost_equal = (0..3).all(|i| (n1[i] - n2[i]).abs() <= 1e-5 * n1[i].abs());
        almost_equal.then(|| (n1, n1[0] * v[0][0] + n1[1] * v[0][1] + n1[2] * v[0][2]))
    }

    /// `math::normal_tri` (`normalize(cross_tri(v1, v2, v3))`).
    fn normal_tri(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 3] {
        let a = [v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];
        let b = [v2[0] - v3[0], v2[1] - v3[1], v2[2] - v3[2]];
        let c = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let len_sq = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
        if len_sq > 1.0e-35 {
            let l = len_sq.sqrt();
            [c[0] / l, c[1] / l, c[2] / l]
        } else {
            [0.0; 3]
        }
    }

    fn op_type(op: BoolOperation) -> OpType {
        match op {
            BoolOperation::Intersect => OpType::Intersect,
            BoolOperation::Union => OpType::Add,
            BoolOperation::Difference => OpType::Subtract,
        }
    }

    /// The status of a boolean result as Blender maps it.
    fn result_error(status: Error) -> BoolError {
        if status == Error::ResultTooLarge {
            BoolError::ResultTooBig
        } else {
            BoolError::Unknown
        }
    }

    pub(super) fn run(
        operands: &[BoolMesh<'_>],
        opts: &BoolOptions,
    ) -> Result<BoolResult, BoolError> {
        if operands.is_empty() {
            return Ok(BoolResult::default());
        }
        let offsets = Offsets::new(operands);
        let prepared: Vec<Prepared> = operands.iter().map(prepare).collect();
        let exact = opts.solver != SolverKind::Manifold;
        let manifolds: Vec<Manifold> = prepared
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let gl = mesh_gl(p, i, offsets.face_start[i]);
                if exact {
                    Manifold::from_mesh_gl_robust(&gl)
                } else {
                    Manifold::from_mesh_gl(&gl)
                }
            })
            .collect();

        let mut gl_result: MeshGL;
        if manifolds.iter().any(|m| m.status() != Error::NoError) {
            let plane = (operands.len() == 2
                && opts.operation == BoolOperation::Difference
                && manifolds[0].status() == Error::NoError)
                .then(|| is_plane(&operands[1]))
                .flatten();
            if let Some((normal, offset)) = plane {
                // `mesh_trim_manifold`.
                let trimmed = manifolds[0].trim_by_plane(
                    Vec3::new(
                        f64::from(normal[0]),
                        f64::from(normal[1]),
                        f64::from(normal[2]),
                    ),
                    f64::from(offset),
                );
                gl_result = trimmed.get_mesh_gl(-1);
                match trimmed.status() {
                    Error::NoError => {}
                    Error::NotManifold => return Err(BoolError::NonManifold(Vec::new())),
                    s => return Err(result_error(s)),
                }
                // The plane's triangles carry a fresh original ID and meaningless face ids:
                // give them the plane's face.
                if !gl_result.vert_properties.is_empty() && gl_result.run_index.len() == 3 {
                    let start = gl_result.run_index[1] as usize / 3;
                    let end = gl_result.run_index[2] as usize / 3;
                    for id in &mut gl_result.face_id[start..end] {
                        *id = offsets.face_start[1];
                    }
                }
            } else {
                // MANIFOLD: the non-manifold operands. EXACT (robust import): the operands
                // that are not closed and orientable (`Error::NotClosed`) as well.
                let bad: Vec<u32> = manifolds
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| {
                        m.status() == Error::NotManifold || (exact && m.status() != Error::NoError)
                    })
                    .map(|(i, _)| i as u32)
                    .collect();
                if bad.is_empty() {
                    return Err(BoolError::Unknown);
                }
                return Err(BoolError::NonManifold(bad));
            }
        } else {
            let op = op_type(opts.operation);
            let result = if exact {
                // The robust engine (winding numbers, like Blender's exact solver: an
                // inside-out operand counts as empty space), as a left fold.
                let mut acc = manifolds[0].clone();
                for m in &manifolds[1..] {
                    acc = acc.boolean_with_engine(m, op, BooleanEngine::Robust);
                }
                acc
            } else if manifolds.len() == 1 {
                manifolds.into_iter().next().unwrap()
            } else {
                let children = manifolds
                    .into_iter()
                    .map(|m| CsgNode::leaf(m.into_impl()))
                    .collect();
                Manifold::from_impl(CsgNode::op_n(op, children).evaluate())
            };
            gl_result = result.get_mesh_gl(-1);
            if result.status() != Error::NoError {
                return Err(result_error(result.status()));
            }
        }
        Ok(to_result(gl_result, operands, &prepared, &offsets))
    }

    /// `meshgl_to_mesh` up to the attributes: clean, assemble, and the out-to-in maps.
    fn to_result(
        mut gl: MeshGL,
        operands: &[BoolMesh<'_>],
        prepared: &[Prepared],
        offsets: &Offsets,
    ) -> BoolResult {
        let np = gl.num_prop as usize;
        if gl.vert_properties.is_empty() || gl.tri_verts.is_empty() || np < 3 {
            return BoolResult {
                face_offsets: vec![0],
                ..Default::default()
            };
        }
        let positions: Vec<[f32; 3]> = gl
            .vert_properties
            .chunks_exact(np)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        assemble::clean_tris(positions.len(), &mut gl.tri_verts, &mut gl.face_id);
        let input_faces_num = *offsets.face_start.last().unwrap() as usize;
        let a = assemble::assemble(positions, &gl.tri_verts, &gl.face_id, input_faces_num);

        let mut r = BoolResult {
            positions: a.positions,
            face_offsets: Vec::with_capacity(a.faces.len() + 1),
            ..Default::default()
        };
        r.face_offsets.push(0);
        for f in &a.faces {
            r.corner_verts.extend_from_slice(&f.verts);
            r.face_offsets.push(r.corner_verts.len() as u32);
        }
        // `ensure_face_map`.
        let local = |fid: u32| -> (usize, u32) {
            let m = offsets.mesh_for_face(fid);
            (m, fid - offsets.face_start[m])
        };
        r.face_src = a
            .faces
            .iter()
            .map(|f| {
                let (m, lf) = local(f.face_id);
                (m as u32, lf)
            })
            .collect();
        // `ensure_vertex_map`: the first vertex of the input face at exactly the same
        // position, faces and corners in order.
        let mut vert_map: Vec<Option<Src>> = vec![None; r.positions.len()];
        for f in &a.faces {
            let (m, lf) = local(f.face_id);
            let op = &operands[m];
            let (s, e) = (
                op.face_offsets[lf as usize] as usize,
                op.face_offsets[lf as usize + 1] as usize,
            );
            let in_verts = &op.corner_verts[s..e];
            for &out_v in &f.verts {
                if vert_map[out_v as usize].is_some() {
                    continue;
                }
                let p = r.positions[out_v as usize];
                if let Some(&in_v) = in_verts
                    .iter()
                    .find(|&&v| prepared[m].positions[v as usize] == p)
                {
                    vert_map[out_v as usize] = Some((m as u32, in_v));
                }
            }
        }
        // `ensure_corner_map`.
        r.corner_src = Vec::with_capacity(r.corner_verts.len());
        for f in &a.faces {
            let (m, lf) = local(f.face_id);
            let op = &operands[m];
            let s = op.face_offsets[lf as usize] as usize;
            let e = op.face_offsets[lf as usize + 1] as usize;
            for &out_v in &f.verts {
                let src = vert_map[out_v as usize].and_then(|(vm, in_v)| {
                    (vm as usize == m)
                        .then(|| op.corner_verts[s..e].iter().position(|&v| v == in_v))
                        .flatten()
                        .map(|i| (m as u32, (s + i) as u32))
                });
                r.corner_src.push(src);
            }
        }
        r.vert_src = vert_map;
        r
    }
}

#[cfg(all(test, feature = "manifold"))]
mod tests {
    use super::*;
    use crate::{BoolMesh, BoolOperation};

    /// Blender's default cube (`size`, centre), corner order of `primitive_cube_add`.
    fn cube(size: f32, c: [f32; 3]) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
        let h = size / 2.0;
        let mut p = Vec::new();
        for x in [-h, h] {
            for y in [-h, h] {
                for z in [-h, h] {
                    p.push([c[0] + x, c[1] + y, c[2] + z]);
                }
            }
        }
        let faces = [
            [0, 1, 3, 2],
            [2, 3, 7, 6],
            [6, 7, 5, 4],
            [4, 5, 1, 0],
            [2, 6, 4, 0],
            [7, 3, 1, 5],
        ];
        let cv: Vec<u32> = faces.iter().flatten().copied().collect();
        (p, vec![0, 4, 8, 12, 16, 20, 24], cv)
    }

    #[test]
    fn cube_minus_cube_corner() {
        let a = cube(2.0, [0.0; 3]);
        let b = cube(1.0, [0.5; 3]);
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&b.0, &b.1, &b.2),
        ];
        let opts = BoolOptions {
            solver: SolverKind::Manifold,
            ..Default::default()
        };
        let r = ManifoldSolver.run(&ops, &opts).unwrap();
        // Blender 5.2.4: V14 F9 (three faces of the base cut into L shapes, three new faces).
        assert_eq!(r.positions.len(), 14);
        assert_eq!(r.face_offsets.len() - 1, 9);
        assert_eq!(r.face_src.iter().filter(|s| s.0 == 1).count(), 3);
        assert_eq!(r.vert_src.iter().filter(|s| s.is_none()).count(), 0);
        let _ = BoolOperation::Union;
    }

    #[test]
    fn open_operand_is_non_manifold() {
        let a = cube(2.0, [0.0; 3]);
        let p = vec![
            [-2.0, -2.0, 0.0],
            [2.0, -2.0, 0.0],
            [2.0, 2.0, 0.0],
            [-2.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let fo = vec![0, 3, 6];
        let cv = vec![0, 1, 4, 4, 2, 3];
        let ops = [BoolMesh::new(&a.0, &a.1, &a.2), BoolMesh::new(&p, &fo, &cv)];
        let opts = BoolOptions {
            solver: SolverKind::Manifold,
            ..Default::default()
        };
        assert_eq!(
            ManifoldSolver.run(&ops, &opts),
            Err(BoolError::NonManifold(vec![1]))
        );
    }
}
