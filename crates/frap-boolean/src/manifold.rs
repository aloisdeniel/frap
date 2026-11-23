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
//! The input (transformed positions, `corner_tris`) is bit-identical to Blender's and the
//! assembly is a line-by-line port, but manifold-rust 0.19.2 (Manifold 3.5.4, eagerly
//! evaluated) does not return the triangles of the Manifold 3.5.2 Blender links in every case:
//! where a cutter makes a hole inside a face the bridge can end at another in-line vertex, and
//! coincident vertices Manifold 3.5.2 keeps apart can be one. Those results have Blender's
//! volume but not its vertex count (known T2 gaps, listed in the BOOLEAN modifier's tests).
//!
//! EXACT (Blender's exact-arithmetic mesh arrangement, not ported: M9) runs on the same
//! pipeline with manifold-rust's robust engine (`BooleanEngine::Robust`: exact rational
//! arithmetic and winding numbers, so closed non-manifold soup is accepted and an inside-out
//! operand is empty space, as in Blender), as a left fold; the bridges merging leaves at new
//! in-line vertices are moved off them (`assemble::reroute_bridges`) so those vertices are
//! dissolved, as Blender's exact solver has none. NaN coordinates, coordinates beyond 1e12
//! (infinite ones included) and an empty result
//! that cannot be right (a UNION, a DIFFERENCE whose base reaches outside its cutters) are
//! errors, not silent empty meshes. Open operands fail with
//! [`BoolError::NonManifold`] (Blender's exact solver accepts them). Results have Blender's
//! volume but the MANIFOLD face layout (T3). `use_self` and `use_hole_tolerant` do not change
//! the result.
//!
//! `--stmt-timeout` (WP 8.6): the manifold-rust part (imports, kernel, export) runs on a
//! worker thread when the statement has a deadline (`imp::on_worker`); the statement's thread
//! answers `TIMEOUT` at the deadline even while the worker is in a step manifold-rust cannot
//! cancel (`Manifold::from_mesh_gl*`, `trim_by_plane`, `get_mesh_gl`), and the abandoned
//! worker stops at its next cancellation point. Two 130k-triangle spheres answer within about
//! 20 ms of the deadline in a debug build (`tests/cancel.rs`).

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
pub(crate) mod imp {
    use manifold_rust::cancel::CancelToken;
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
    pub(crate) struct Prepared {
        pub(crate) positions: Vec<[f32; 3]>,
        pub(crate) tris: Vec<[u32; 3]>,
        pub(crate) face_of_tri: Vec<u32>,
    }

    pub(crate) fn prepare(m: &BoolMesh<'_>) -> Prepared {
        let positions = m.transformed_positions();
        let mut tris = Vec::new();
        let mut face_of_tri = Vec::new();
        let faces_num = m.faces_num();
        if !m.corner_tris.is_empty() {
            // `corner_tris` are face by face; recover each triangle's face from its corner.
            let mut f = 0usize;
            for t in m.corner_tris {
                frap_math::budget::check();
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
                frap_math::budget::check();
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

    /// A kernel result cancelled through its token: the deadline has passed, so the budget
    /// check unwinds with `Interrupted` (the statement answers `TIMEOUT`).
    fn check_cancelled(status: Error) {
        if status == Error::Cancelled {
            frap_math::budget::check_now();
        }
    }

    /// The part of a boolean that runs inside manifold-rust: the imports
    /// (`Manifold::from_mesh_gl*`), the kernel and the export of the result (`get_mesh_gl`),
    /// on owned data so that it can run on a worker thread.
    struct Job {
        gls: Vec<MeshGL>,
        exact: bool,
        op: BoolOperation,
        /// `is_plane` of the second operand of a two-operand DIFFERENCE.
        plane: Option<([f32; 3], f32)>,
        /// The first face of the second operand (the plane's face).
        plane_face: u32,
    }

    /// What [`compute`] returns: the result mesh, or the cancellation that [`run`] turns into
    /// the `Interrupted` unwind on the calling thread (the budget is thread-local).
    enum Computed {
        /// The result, and whether it came from the kernel (not `trim_by_plane`).
        Mesh(MeshGL, bool),
        Cancelled,
    }

    /// The most deadline-bound boolean workers running at once per process (running or
    /// abandoned after a timeout: a worker holds its slot until its work ends), as the WebP
    /// encoder's `MAX_WORKERS` (M7W0-10). An abandoned worker ends at its next cancellation
    /// point, at most one uncancellable manifold-rust step later (about 60 ms per
    /// 130k-triangle import in a release build), but an agent resending a timed-out boolean
    /// in a `serve`/`mcp` session must not pile them up.
    pub(crate) const MAX_WORKERS: usize = 4;

    /// The `TIMEOUT` reason of a boolean that never got a worker slot.
    pub(crate) const SLOTS_BUSY: &str = "the boolean waited for a worker slot: earlier booleans \
        that timed out are still finishing their current step in the background (at most 4 at \
        once per process); retry once they finish";

    static WORKERS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
    static WORKER_FREED: std::sync::Condvar = std::sync::Condvar::new();

    fn workers() -> std::sync::MutexGuard<'static, usize> {
        WORKERS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A worker slot, released when the worker's work ends (also by a panic).
    struct Slot;

    impl Drop for Slot {
        fn drop(&mut self) {
            let mut n = workers();
            *n = n.saturating_sub(1);
            WORKER_FREED.notify_all();
        }
    }

    /// Waits for a worker slot until `deadline`, then unwinds with `Interrupted`
    /// ([`SLOTS_BUSY`]).
    fn acquire_slot(deadline: std::time::Instant) -> Slot {
        let mut n = workers();
        while *n >= MAX_WORKERS {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                drop(n);
                frap_math::budget::check_now_because(SLOTS_BUSY);
                n = workers();
                continue;
            }
            n = WORKER_FREED
                .wait_timeout(n, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        *n += 1;
        Slot
    }

    /// The deadline-bound boolean workers running now (tests).
    #[cfg(test)]
    pub(crate) fn running_workers() -> usize {
        *workers()
    }

    /// Runs `work` with a cancellation token (`--stmt-timeout`, ARCHITECTURE §12). Without a
    /// deadline it runs here. With one it runs on a worker thread while this thread waits for
    /// its result until the deadline: then the token is cancelled (manifold-rust's kernels
    /// poll it) and this thread unwinds at once with `Interrupted`, so the statement answers
    /// `TIMEOUT` right at the deadline even while the worker is in a step manifold-rust cannot
    /// cancel (the import of an operand, `Manifold::from_mesh_gl*`, `trim_by_plane`, the export,
    /// `get_mesh_gl`). The abandoned worker stops at its next cancellation point (after at most
    /// that step) and its result is dropped. A worker that panics (a manifold-rust bug) is
    /// Blender's unknown error. At most [`MAX_WORKERS`] such workers run at once: a further
    /// boolean waits for a slot within its own deadline (then `TIMEOUT`, [`SLOTS_BUSY`]).
    pub(crate) fn on_worker<T: Send + 'static>(
        work: impl FnOnce(&CancelToken) -> T + Send + 'static,
    ) -> Result<T, BoolError> {
        let Some(deadline) = frap_math::budget::deadline() else {
            return Ok(work(&CancelToken::new()));
        };
        frap_math::budget::check_now();
        let slot = acquire_slot(deadline);
        let token = CancelToken::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_token = token.clone();
        let spawned = std::thread::Builder::new()
            .name("frap-boolean".into())
            .spawn(move || {
                let _slot = slot;
                let r = work(&worker_token);
                let _ = tx.send(r);
            });
        if let Err(e) = spawned {
            return Err(BoolError::Input(format!(
                "frap: cannot start the boolean worker thread: {e}"
            )));
        }
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok(r) => return Ok(r),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= deadline {
                        token.cancel();
                        frap_math::budget::check_now();
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(BoolError::Unknown);
                }
            }
        }
    }

    /// [`compute`] on [`on_worker`].
    fn run_job(job: Job) -> Result<Computed, BoolError> {
        on_worker(move |token| compute(job, token))?
    }

    /// The manifold-rust part of [`run`] (see [`Job`]).
    fn compute(job: Job, token: &CancelToken) -> Result<Computed, BoolError> {
        let mut manifolds: Vec<Manifold> = Vec::with_capacity(job.gls.len());
        for gl in &job.gls {
            if token.is_cancelled() {
                return Ok(Computed::Cancelled);
            }
            manifolds.push(if job.exact {
                Manifold::from_mesh_gl_robust(gl)
            } else {
                Manifold::from_mesh_gl(gl)
            });
        }
        if token.is_cancelled() {
            return Ok(Computed::Cancelled);
        }
        let mut gl_result: MeshGL;
        let mut from_kernel = false;
        if manifolds.iter().any(|m| m.status() != Error::NoError) {
            let plane = (manifolds.len() == 2
                && job.op == BoolOperation::Difference
                && manifolds[0].status() == Error::NoError)
                .then_some(job.plane)
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
                        *id = job.plane_face;
                    }
                }
            } else {
                // MANIFOLD: the non-manifold operands. EXACT (robust import): the operands
                // that are not closed and orientable (`Error::NotClosed`) as well.
                let bad: Vec<u32> = manifolds
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| {
                        m.status() == Error::NotManifold
                            || (job.exact && m.status() != Error::NoError)
                    })
                    .map(|(i, _)| i as u32)
                    .collect();
                if bad.is_empty() {
                    return Err(BoolError::Unknown);
                }
                return Err(BoolError::NonManifold(bad));
            }
        } else {
            let op = op_type(job.op);
            let result = if job.exact {
                // The robust engine (winding numbers, like Blender's exact solver: an
                // inside-out operand counts as empty space), as a left fold.
                let mut acc = manifolds[0].clone();
                for m in &manifolds[1..] {
                    acc = acc.boolean_with_engine_and_token(
                        m,
                        op,
                        BooleanEngine::Robust,
                        Some(token),
                    );
                    if acc.status() == Error::Cancelled {
                        return Ok(Computed::Cancelled);
                    }
                }
                acc
            } else if manifolds.len() == 1 {
                manifolds.into_iter().next().unwrap()
            } else {
                let children = manifolds
                    .into_iter()
                    .map(|m| CsgNode::leaf(m.into_impl()))
                    .collect();
                Manifold::from_impl(CsgNode::op_n(op, children).evaluate_with_token(Some(token)))
            };
            if result.status() == Error::Cancelled {
                return Ok(Computed::Cancelled);
            }
            gl_result = result.get_mesh_gl(-1);
            from_kernel = true;
            if result.status() != Error::NoError {
                return Err(result_error(result.status()));
            }
        }
        Ok(Computed::Mesh(gl_result, from_kernel))
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
        if exact {
            // `f32::max` drops NaN: look for it first (manifold-rust's robust engine panics
            // on a non-finite coordinate).
            if prepared
                .iter()
                .flat_map(|p| p.positions.iter().flatten())
                .any(|c| c.is_nan())
            {
                return Err(BoolError::Input(
                    "frap: the EXACT solver needs finite coordinates (found NaN); Blender's \
                     exact mesh arrangement is not ported (M9)"
                        .into(),
                ));
            }
            let max = prepared
                .iter()
                .flat_map(|p| p.positions.iter().flatten())
                .fold(0.0f32, |m, c| m.max(c.abs()));
            if max > EXACT_RANGE {
                return Err(BoolError::Input(format!(
                    "frap: the EXACT solver handles coordinates up to {EXACT_RANGE:e} (found \
                     {max:e}); Blender's exact mesh arrangement is not ported (M9)"
                )));
            }
        }
        let gls: Vec<MeshGL> = prepared
            .iter()
            .enumerate()
            .map(|(i, p)| {
                frap_math::budget::check_now();
                mesh_gl(p, i, offsets.face_start[i])
            })
            .collect();
        let plane = (operands.len() == 2 && opts.operation == BoolOperation::Difference)
            .then(|| is_plane(&operands[1]))
            .flatten();
        let job = Job {
            gls,
            exact,
            op: opts.operation,
            plane,
            plane_face: offsets.face_start.get(1).copied().unwrap_or(0),
        };
        let (gl_result, from_kernel) = match run_job(job)? {
            Computed::Mesh(gl, k) => (gl, k),
            Computed::Cancelled => {
                check_cancelled(Error::Cancelled);
                return Err(BoolError::Unknown);
            }
        };
        if exact
            && from_kernel
            && gl_result.tri_verts.is_empty()
            && cannot_be_empty(&prepared, opts.operation)
        {
            return Err(BoolError::Input(
                "frap: the EXACT solver returned nothing for operands whose result cannot be \
                 empty (Blender's exact mesh arrangement is not ported, M9)"
                    .into(),
            ));
        }
        Ok(to_result(gl_result, operands, &prepared, &offsets, exact))
    }

    /// The largest coordinate the EXACT stand-in (manifold-rust's robust engine) is trusted
    /// with: from 1e13 on, a single far vertex makes it lose the whole result.
    const EXACT_RANGE: f32 = 1e12;

    /// Whether a boolean of these operands is non-empty for certain: a UNION of non-empty
    /// operands, a DIFFERENCE whose base reaches outside the bounding box of the cutters.
    pub(crate) fn cannot_be_empty(prepared: &[Prepared], op: BoolOperation) -> bool {
        let bbox = |ps: &[&Prepared]| {
            let mut b = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
            for p in ps {
                for t in &p.tris {
                    for &v in t {
                        let q = p.positions[v as usize];
                        for c in 0..3 {
                            b[0][c] = b[0][c].min(q[c]);
                            b[1][c] = b[1][c].max(q[c]);
                        }
                    }
                }
            }
            b
        };
        let non_empty = |p: &Prepared| !p.tris.is_empty();
        match op {
            BoolOperation::Union => prepared.iter().any(non_empty),
            BoolOperation::Difference => {
                if !non_empty(&prepared[0]) {
                    return false;
                }
                let a = bbox(&[&prepared[0]]);
                let rest: Vec<&Prepared> = prepared[1..].iter().collect();
                let b = bbox(&rest);
                (0..3).any(|c| a[0][c] < b[0][c] || a[1][c] > b[1][c])
            }
            BoolOperation::Intersect => false,
        }
    }

    /// `meshgl_to_mesh` up to the attributes: clean, assemble, and the out-to-in maps.
    fn to_result(
        mut gl: MeshGL,
        operands: &[BoolMesh<'_>],
        prepared: &[Prepared],
        offsets: &Offsets,
        exact: bool,
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
        let a = if exact {
            // The EXACT stand-in also moves the bridges off new in-line vertices.
            let input: rustc_hash::FxHashSet<[u32; 3]> = prepared
                .iter()
                .flat_map(|p| p.positions.iter().map(|q| q.map(|c| (c + 0.0).to_bits())))
                .collect();
            let is_input =
                |v: u32| input.contains(&positions[v as usize].map(|c| (c + 0.0).to_bits()));
            assemble::assemble_with(
                positions.clone(),
                &gl.tri_verts,
                &gl.face_id,
                input_faces_num,
                Some(&is_input),
            )
        } else {
            assemble::assemble(positions, &gl.tri_verts, &gl.face_id, input_faces_num)
        };

        let mut r = BoolResult {
            positions: a.positions,
            face_offsets: Vec::with_capacity(a.faces.len() + 1),
            ..Default::default()
        };
        r.face_offsets.push(0);
        for f in &a.faces {
            frap_math::budget::check();
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
            frap_math::budget::check();
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
            frap_math::budget::check();
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
    fn empty_operand() {
        let a = cube(2.0, [0.0; 3]);
        let none: Vec<[f32; 3]> = Vec::new();
        let fo = vec![0u32];
        let cv: Vec<u32> = Vec::new();
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&none, &fo, &cv),
        ];
        for solver in [SolverKind::Manifold, SolverKind::Exact] {
            for (op, faces) in [
                (BoolOperation::Difference, 6),
                (BoolOperation::Union, 6),
                (BoolOperation::Intersect, 0),
            ] {
                let opts = BoolOptions {
                    solver,
                    operation: op,
                    ..Default::default()
                };
                let r = ManifoldSolver.run(&ops, &opts).unwrap();
                assert_eq!(r.face_offsets.len() - 1, faces, "{solver:?} {op:?}");
            }
        }
    }

    #[test]
    fn exact_refuses_coordinates_out_of_range() {
        // Blender's exact solver cuts a cube with a vertex at 1e20; manifold-rust's robust
        // engine loses the whole result from 1e13 on: an error, not a silent empty mesh.
        let mut a = cube(2.0, [0.0; 3]);
        a.0[0] = [1e20, 0.0, 0.0];
        let b = cube(1.0, [0.5; 3]);
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&b.0, &b.1, &b.2),
        ];
        let r = ManifoldSolver.run(&ops, &BoolOptions::default());
        assert!(
            matches!(r, Err(BoolError::Input(ref m)) if m.contains("1e12")),
            "{r:?}"
        );
        a.0[0] = [1e12, 0.0, 0.0];
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&b.0, &b.1, &b.2),
        ];
        let r = ManifoldSolver.run(&ops, &BoolOptions::default()).unwrap();
        assert!(r.face_offsets.len() > 1);
        // MANIFOLD is Blender's: no range check.
        a.0[0] = [1e20, 0.0, 0.0];
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&b.0, &b.1, &b.2),
        ];
        let opts = BoolOptions {
            solver: SolverKind::Manifold,
            ..Default::default()
        };
        assert!(ManifoldSolver.run(&ops, &opts).is_ok());
    }

    /// M8 stabilization round 3: NaN passed the range check (`f32::max` drops it) and
    /// panicked in manifold-rust's robust engine. NaN and infinite coordinates are clean
    /// `Input` errors for EXACT; MANIFOLD does not panic either.
    #[test]
    fn exact_refuses_non_finite_coordinates() {
        let b = cube(1.0, [0.5; 3]);
        for (bad, needle) in [
            (f32::NAN, "NaN"),
            (f32::INFINITY, "1e12"),
            (f32::NEG_INFINITY, "1e12"),
            (f32::MAX, "1e12"),
        ] {
            let mut a = cube(2.0, [0.0; 3]);
            a.0[0] = [bad, 0.0, 0.0];
            for op in [
                BoolOperation::Difference,
                BoolOperation::Union,
                BoolOperation::Intersect,
            ] {
                // Either operand.
                for swap in [false, true] {
                    let (x, y) = if swap { (&b, &a) } else { (&a, &b) };
                    let ops = [
                        BoolMesh::new(&x.0, &x.1, &x.2),
                        BoolMesh::new(&y.0, &y.1, &y.2),
                    ];
                    let opts = BoolOptions {
                        operation: op,
                        ..Default::default()
                    };
                    let r = ManifoldSolver.run(&ops, &opts);
                    assert!(
                        matches!(r, Err(BoolError::Input(ref m)) if m.contains(needle)),
                        "{bad} {op:?}: {r:?}"
                    );
                    let opts = BoolOptions {
                        solver: SolverKind::Manifold,
                        operation: op,
                        ..Default::default()
                    };
                    let _ = ManifoldSolver.run(&ops, &opts);
                }
            }
        }
    }

    #[test]
    fn results_that_cannot_be_empty() {
        let a = cube(2.0, [0.0; 3]);
        let inside = cube(4.0, [0.0; 3]);
        let poke = cube(1.0, [0.9, 0.0, 0.0]);
        let p = |m: &(Vec<[f32; 3]>, Vec<u32>, Vec<u32>)| {
            imp::prepare(&BoolMesh::new(&m.0, &m.1, &m.2))
        };
        let (pa, pin, ppoke) = (p(&a), p(&inside), p(&poke));
        assert!(imp::cannot_be_empty(
            &[&pa, &pin].map(clone),
            BoolOperation::Union
        ));
        assert!(!imp::cannot_be_empty(
            &[&pa, &pin].map(clone),
            BoolOperation::Difference
        ));
        assert!(imp::cannot_be_empty(
            &[&pa, &ppoke].map(clone),
            BoolOperation::Difference
        ));
        assert!(!imp::cannot_be_empty(
            &[&pa, &ppoke].map(clone),
            BoolOperation::Intersect
        ));
    }

    fn clone(p: &imp::Prepared) -> imp::Prepared {
        imp::Prepared {
            positions: p.positions.clone(),
            tris: p.tris.clone(),
            face_of_tri: p.face_of_tri.clone(),
        }
    }

    #[test]
    fn exact_dissolves_in_line_vertices_behind_bridges() {
        // A cutter through one face (a hole): Blender's exact solver gives V16 F12 and keeps
        // no vertex where the base's triangulation diagonal crosses the hole.
        let a = cube(2.0, [0.0; 3]);
        let b = cube(1.0, [0.9, 0.2, 0.3]);
        let ops = [
            BoolMesh::new(&a.0, &a.1, &a.2),
            BoolMesh::new(&b.0, &b.1, &b.2),
        ];
        let r = ManifoldSolver.run(&ops, &BoolOptions::default()).unwrap();
        assert_eq!((r.positions.len(), r.face_offsets.len() - 1), (16, 12));
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

    /// Timed-out workers keep running until their current uncancellable step ends, but at most
    /// `MAX_WORKERS` at once: with every slot taken, a further boolean waits for one within its
    /// own deadline and answers TIMEOUT (`SLOTS_BUSY`) without starting another worker.
    #[test]
    fn abandoned_workers_are_bounded() {
        use std::time::{Duration, Instant};
        let timed_out = |sleep: Duration| {
            std::panic::catch_unwind(|| {
                let _scope = frap_math::budget::scope(frap_math::budget::Budget {
                    timeout: Some(Duration::from_millis(50)),
                    ..Default::default()
                });
                imp::on_worker(move |_token| std::thread::sleep(sleep))
            })
            .err()
            .and_then(|p| frap_math::budget::interrupted(&*p))
        };
        for _ in 0..imp::MAX_WORKERS {
            let i = timed_out(Duration::from_secs(3)).expect("TIMEOUT");
            assert_eq!(i.reason, None, "a slot was free: its own work timed out");
        }
        assert_eq!(imp::running_workers(), imp::MAX_WORKERS);
        let waited = Instant::now();
        let i = timed_out(Duration::from_secs(10)).expect("TIMEOUT");
        assert_eq!(i.reason, Some(imp::SLOTS_BUSY));
        assert!(imp::running_workers() <= imp::MAX_WORKERS);
        assert!(
            waited.elapsed() < Duration::from_secs(1),
            "{:?}",
            waited.elapsed()
        );
    }
}
