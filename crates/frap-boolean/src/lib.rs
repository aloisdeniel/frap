// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-boolean` (layer L3): the [`BooleanSolver`] trait (CONTRACT, M4 W0) and its backends:
//! `manifold-rust` for `solver='MANIFOLD'` (feature `manifold`, on by default through
//! `boolean-manifold`, ARCHITECTURE §3.6) and the same backend for `'EXACT'` (T3; Blender's
//! exact mesh arrangement is M9). `'FLOAT'` is not a backend here: it is Blender's BMesh
//! intersect, ported in `frap_bmesh::ops::intersect` (WP 8.6) and run by the BOOLEAN modifier
//! and `mesh.intersect*` themselves. Results carry the source of every face, corner and vertex
//! so the BOOLEAN modifier and `mesh.intersect*` transfer UVs, materials and attributes as
//! Blender's `mesh_boolean` does. `--stmt-timeout` cancels both backends within milliseconds
//! of the deadline (`manifold::imp::on_worker`).
//!
//! The crate sits below `frap-mesh` (L4): operands are plain arrays ([`BoolMesh`]).
//!
//! Ownership (M4): WP 4.6 owns this crate (`crates/frap-boolean/**`) after W0; it may add
//! fields to the `#[non_exhaustive]` structs and items to the modules.
#![deny(unsafe_code)]

pub mod assemble;
pub mod knife;
pub mod manifold;

/// `BooleanModifier.operation` / `mesh.intersect_boolean(operation=…)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoolOperation {
    /// `INTERSECT`
    Intersect,
    /// `UNION`
    Union,
    /// `DIFFERENCE`
    #[default]
    Difference,
}

/// `BooleanModifier.solver` / `mesh.intersect*(solver=…)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SolverKind {
    /// `FLOAT` (Blender's BMesh intersect: `frap_bmesh::ops::intersect`, not a backend of
    /// this crate).
    Float,
    /// `EXACT` (Blender's exact-arithmetic mesh_boolean; T3 through manifold-rust).
    #[default]
    Exact,
    /// `MANIFOLD` (Blender 5.x Manifold solver; T2).
    Manifold,
}

/// One operand in the common (first operand's object) space.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct BoolMesh<'a> {
    /// Vertex positions.
    pub positions: &'a [[f32; 3]],
    /// Face offsets into `corner_verts` (`faces + 1` entries).
    pub face_offsets: &'a [u32],
    /// Vertex of each corner.
    pub corner_verts: &'a [u32],
    /// Operand-to-common-space transform (column-major 4x4; identity for the first operand).
    pub transform: [[f32; 4]; 4],
    /// The face triangulation as corner indices (`Mesh::corner_tris()`, Blender's
    /// `mesh.corner_tris()` of the **transformed** mesh, face by face). Empty: the solver fans
    /// every face from its first corner (exact for triangles and convex quads).
    pub corner_tris: &'a [[u32; 3]],
}

impl<'a> BoolMesh<'a> {
    /// An operand already in the common space.
    pub fn new(
        positions: &'a [[f32; 3]],
        face_offsets: &'a [u32],
        corner_verts: &'a [u32],
    ) -> Self {
        BoolMesh {
            positions,
            face_offsets,
            corner_verts,
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            corner_tris: &[],
        }
    }

    /// The same operand with an operand-to-common-space transform (column-major 4x4,
    /// `world_to_object * ob.matrix_world` for the BOOLEAN modifier's operands).
    pub fn with_transform(mut self, transform: [[f32; 4]; 4]) -> Self {
        self.transform = transform;
        self
    }

    /// The same operand with its face triangulation (corner indices, see
    /// [`BoolMesh::corner_tris`]).
    pub fn with_corner_tris(mut self, corner_tris: &'a [[u32; 3]]) -> Self {
        self.corner_tris = corner_tris;
        self
    }

    /// Number of faces.
    pub fn faces_num(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }

    /// Whether the transform is exactly the identity (`math::is_identity`).
    pub fn is_identity(&self) -> bool {
        (0..4).all(|c| (0..4).all(|r| self.transform[c][r] == if c == r { 1.0 } else { 0.0 }))
    }

    /// The positions in the common space: `math::transform_point(transform, co)` in `f32`
    /// (`bke::mesh_transform`), or the positions themselves for the identity.
    pub fn transformed_positions(&self) -> Vec<[f32; 3]> {
        if self.is_identity() {
            return self.positions.to_vec();
        }
        let m = &self.transform;
        self.positions
            .iter()
            .map(|p| transform_point(m, *p))
            .collect()
    }
}

/// `math::transform_point(float4x4, float3)`: `mat3 * p + location`, in `f32`.
pub fn transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    let mut r = [0.0f32; 3];
    for (i, ri) in r.iter_mut().enumerate() {
        *ri = m[0][i] * p[0] + m[1][i] * p[1] + m[2][i] * p[2] + m[3][i];
    }
    r
}

/// Options of one boolean.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BoolOptions {
    /// The operation (operand 0 is the base, the others the cutters).
    pub operation: BoolOperation,
    /// The solver requested.
    pub solver: SolverKind,
    /// `use_self` (EXACT: self-intersections of the operands).
    pub use_self: bool,
    /// `use_hole_tolerant` (EXACT).
    pub hole_tolerant: bool,
    /// `double_threshold` (FLOAT).
    pub double_threshold: f32,
}

impl Default for BoolOptions {
    /// A fresh BOOLEAN modifier's settings.
    fn default() -> Self {
        BoolOptions {
            operation: BoolOperation::Difference,
            solver: SolverKind::Exact,
            use_self: false,
            hole_tolerant: false,
            double_threshold: 1e-6,
        }
    }
}

/// `(operand index, element index in that operand)`.
pub type Src = (u32, u32);

/// The result mesh and the origin of every element.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BoolResult {
    /// Vertex positions (common space).
    pub positions: Vec<[f32; 3]>,
    /// Face offsets (`faces + 1` entries).
    pub face_offsets: Vec<u32>,
    /// Vertex of each corner.
    pub corner_verts: Vec<u32>,
    /// Source face of each output face.
    pub face_src: Vec<Src>,
    /// Source vertex of each output vertex (`None`: created at an intersection).
    pub vert_src: Vec<Option<Src>>,
    /// Source corner of each output corner, or the source face whose plane it lies on
    /// (`None`: interpolate from the face).
    pub corner_src: Vec<Option<Src>>,
}

/// Why a boolean failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BoolError {
    /// The solver refused the input (non-manifold operand for MANIFOLD, …), with Blender's
    /// message where it has one.
    #[error("{0}")]
    Input(String),
    /// The solver is not available in this build.
    #[error("{0}")]
    Unavailable(String),
    /// `BooleanErrorType::NonManifold`: the operands (indices into the operand list) that are
    /// not manifold; empty when the solver could not tell which.
    #[error("non-manifold inputs")]
    NonManifold(Vec<u32>),
    /// `BooleanErrorType::ResultTooBig`.
    #[error("result too big")]
    ResultTooBig,
    /// `BooleanErrorType::UnknownError`.
    #[error("unknown error")]
    Unknown,
}

/// A boolean backend (CONTRACT, ARCHITECTURE §3.2, §7.2). Implementations are stateless.
pub trait BooleanSolver: Send + Sync {
    /// Backend name (`"manifold-rust"`).
    fn name(&self) -> &'static str;
    /// Whether `kind` is computed by this backend (else the caller reports the alias).
    fn supports(&self, kind: SolverKind) -> bool;
    /// Computes the boolean of `operands` (at least one).
    fn run(&self, operands: &[BoolMesh<'_>], opts: &BoolOptions) -> Result<BoolResult, BoolError>;
}

/// The backend `frap-eval` hands to the BOOLEAN evaluator and `mesh.intersect*` use.
pub fn default_solver() -> &'static dyn BooleanSolver {
    &manifold::ManifoldSolver
}
