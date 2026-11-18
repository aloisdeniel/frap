// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-boolean` (layer L3): the [`BooleanSolver`] trait (CONTRACT, M4 W0) and its backends:
//! `manifold-rust` for `solver='MANIFOLD'` (feature `manifold`, on by default through
//! `boolean-manifold`, ARCHITECTURE §3.6) and, until M8 (WP 8.6), the same backend for
//! `'EXACT'` (T3) and `'FLOAT'` (aliased to EXACT with a WARNING by the caller). Results carry
//! the source of every face, corner and vertex so the BOOLEAN modifier and `mesh.intersect*`
//! transfer UVs, materials and attributes as Blender's `mesh_boolean` does.
//!
//! The crate sits below `frap-mesh` (L4): operands are plain arrays ([`BoolMesh`]).
//!
//! Ownership (M4): WP 4.6 owns this crate (`crates/frap-boolean/**`) after W0; it may add
//! fields to the `#[non_exhaustive]` structs and items to the modules.
#![deny(unsafe_code)]

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
    /// `FLOAT` (Blender's BMesh intersect; aliased to EXACT until M8, WP 8.6).
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
        }
    }
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
