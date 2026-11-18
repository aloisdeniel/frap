// SPDX-License-Identifier: GPL-3.0-or-later
//! What the last primitive built on this thread leaves for an Edit Mode merge (M2, WP 2.2).
//! Blender builds a primitive added in Edit Mode **into the edit BMesh**, so `frap-op`'s edit
//! merge needs, besides the mesh the generator returns:
//! - whether the builder's `calc_uvs` hands out UVs by face iteration order (cube, grid), which
//!   the edit BMesh's freed face slots can change;
//! - for cones and cylinders, the topology parameters of `bmo_create_cone_exec`: their builder
//!   kills elements (cap centres, collapsed rings, dissolved fans), so the merge runs the
//!   builder itself in the edit BMesh (allocations and kills then follow the edit mesh's
//!   pools, as in Blender) and takes the positions and UVs from the mesh;
//! - for circles, the topology parameters of `bmo_create_circle_exec`: its NGON fill creates a
//!   centre vertex first and kills it when the fan is dissolved, a freed slot the next
//!   allocation in the edit BMesh takes.

use std::cell::RefCell;

/// The record of one build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Build {
    /// The builder's `calc_uvs` hands out UVs by face **iteration** order rather than from the
    /// geometry (`BM_mesh_calc_uvs_cube`, `BM_mesh_calc_uvs_grid`): the k-th new face in pool
    /// order gets the k-th face's UVs, wherever the faces land.
    pub uvs_follow_face_order: bool,
    /// `(verts, edges, faces)` of the mesh built, to match the record to its mesh.
    pub counts: (usize, usize, usize),
    /// Set by the cone and cylinder generators.
    pub cone: Option<ConeTopology>,
    /// Set by the circle generator.
    pub circle: Option<CircleTopology>,
}

/// The topology parameters of `bmesh.ops.create_circle` (positions come from the mesh).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CircleTopology {
    /// `segments`.
    pub segments: u32,
    /// `cap_ends` (fill type other than `NOTHING`).
    pub cap_ends: bool,
    /// `cap_tris` (fill type `TRIFAN`); otherwise the fan is dissolved into an n-gon.
    pub cap_tris: bool,
}

/// The topology parameters of `bmesh.ops.create_cone` (positions come from the mesh).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConeTopology {
    /// `segments`.
    pub segments: u32,
    /// `radius1 == 0`: the bottom ring collapses.
    pub radius1_zero: bool,
    /// `radius2 == 0`: the top ring collapses.
    pub radius2_zero: bool,
    /// `cap_ends` (fill type other than `NOTHING`).
    pub cap_ends: bool,
    /// `cap_tris` (fill type `TRIFAN`); otherwise the caps are dissolved into n-gons.
    pub cap_tris: bool,
}

thread_local! {
    static LAST: RefCell<Option<Build>> = const { RefCell::new(None) };
}

/// Records the build of the mesh being converted (`Bm::to_mesh`).
pub(crate) fn store(b: Build) {
    LAST.with(|l| *l.borrow_mut() = Some(b));
}

/// Takes the record of the last primitive built on this thread (`None` once taken).
pub fn take_last() -> Option<Build> {
    LAST.with(|l| l.borrow_mut().take())
}
