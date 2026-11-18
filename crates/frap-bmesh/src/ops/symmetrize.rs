// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.symmetrize` (Blender 5.2.4 `bmo_symmetrize_exec`, `operators/bmo_symmetrize.cc`),
//! implemented by WP 2.10. Composes `bisect_plane` (WP 2.8), `duplicate` (WP 2.1),
//! `reverse_faces` (WP 2.9) and `weld_verts` (WP 2.7), as Blender does.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.symmetrize(bmesh, input=[], direction='-X', dist=0.0, use_shapekey=False)`.
//!
//! **Create/kill order.** (1) `bisect_plane` cuts `input` with the plane through the origin
//! normal to the axis (`+1` for `-X`/`-Y`/`-Z`, `-1` for `X`/`Y`/`Z`), clearing the outer
//! side and snapping vertices within `dist` onto the plane. (2) `duplicate` copies the
//! bisect's `geom.out`. (3) The copy is mirrored (`scale` by −1 on the axis, shape keys too
//! with `use_shapekey`) and (4) its faces reversed. (5) Every copy of a vertex on the cut is
//! welded onto its original (`weld_verts`). The output is everything flagged from the bisect
//! and the duplicate that survived the weld. Monkey `+X` → V507 F500 ([MESH §4.14]).
//!
//! The weld sources are the vertices of `geom_cut.out` of `bisect_plane`, exactly as Blender
//! iterates them (not the output vertices lying on the plane: loose vertices on the plane
//! are not cut, and split vertices need not be exactly on it).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use super::triangulate::math;
use crate::ops::bisect_plane::{BisectPlaneIn, bisect_plane};
use crate::ops::duplicate::{DuplicateIn, duplicate};
use crate::ops::reverse_faces::{ReverseFacesIn, reverse_faces};
use crate::ops::weld_verts::{WeldVertsIn, weld_verts};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, FaceId, VertId};

/// Input slots of `symmetrize` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SymmetrizeIn {
    /// `input`.
    pub input: Vec<BMElem>,
    /// `direction` (enum identifier: `-X`, `-Y`, `-Z`, `X`, `Y`, `Z`).
    pub direction: &'static str,
    /// `dist`.
    pub dist: f32,
    /// `use_shapekey`.
    pub use_shapekey: bool,
}

impl Default for SymmetrizeIn {
    fn default() -> Self {
        SymmetrizeIn {
            input: Vec::new(),
            direction: "-X",
            dist: 0.0,
            use_shapekey: false,
        }
    }
}

/// Output slots of `symmetrize`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SymmetrizeOut {
    /// `geom.out`: the kept half and its mirrored copy, vertices then edges then faces.
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_symmetrize_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const ELE_OUT: u16 = 1;

/// The `direction` enum value (`bmo_enum_axis_neg_xyz_and_xyz`).
pub fn direction_value(s: &str) -> Option<usize> {
    Some(match s {
        "-X" => 0,
        "-Y" => 1,
        "-Z" => 2,
        "X" => 3,
        "Y" => 4,
        "Z" => 5,
        _ => return None,
    })
}

/// Runs `symmetrize` on `bm`.
pub fn symmetrize(bm: &mut BMesh, i: &SymmetrizeIn) -> Result<SymmetrizeOut, BmError> {
    let direction = direction_value(i.direction).ok_or_else(|| {
        BmError::Invalid(format!(
            "symmetrize: unknown direction '{}' (expected -X, -Y, -Z, X, Y or Z)",
            i.direction
        ))
    })?;
    if let Some(dead) = i.input.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "symmetrize: dead element {dead:?} in input"
        )));
    }
    let axis = direction % 3;
    let mut plane_no = [0.0f32; 3];
    plane_no[axis] = if direction > 2 { -1.0 } else { 1.0 };
    let mut scale = [1.0f32; 3];
    scale[axis] *= -1.0;

    bm.bmo_exec(TYPE_FLAG, |bm| {
        // Cut in half.
        let bisect = bisect_plane(
            bm,
            &BisectPlaneIn {
                geom: i.input.clone(),
                plane_no,
                dist: i.dist,
                clear_outer: true,
                use_snap_center: true,
                ..BisectPlaneIn::default()
            },
        )?;
        let bisect_geom: Vec<BMElem> = bisect
            .geom
            .iter()
            .copied()
            .filter(|&g| bm.elem_alive(g))
            .collect();
        // `geom_cut.out` of the bisect (its vertices): the weld sources. Not the same set as
        // the output vertices with `co[axis] == 0`: a loose vertex on the plane is not cut
        // (`use_tag` rebuilds the vertex tags from the input edges), and a vertex made by an
        // edge split is interpolated, flagged cut without being snapped.
        let cut: Vec<VertId> = bisect
            .geom_cut
            .iter()
            .filter_map(|g| match g {
                BMElem::Vert(v) if bm.elem_alive(*g) => Some(*v),
                _ => None,
            })
            .collect();
        // Duplicate.
        let dupe = duplicate(
            bm,
            &DuplicateIn {
                geom: bisect_geom.clone(),
                ..DuplicateIn::default()
            },
        )?;
        // Flag for output (some will be merged).
        for &g in bisect_geom.iter().chain(dupe.geom.iter()) {
            bm.oflag_enable(g, ELE_OUT);
        }
        let mut smat = [[0.0f32; 3]; 3];
        smat[0][0] = scale[0];
        smat[1][1] = scale[1];
        smat[2][2] = scale[2];
        let dupe_verts: Vec<VertId> = dupe
            .geom
            .iter()
            .filter_map(|g| match g {
                BMElem::Vert(v) => Some(*v),
                _ => None,
            })
            .collect();
        math::bmo_transform(
            bm,
            &dupe_verts,
            &math::m4_from_m3(&smat),
            &crate::IDENTITY,
            i.use_shapekey,
        );
        let dupe_faces: Vec<FaceId> = dupe
            .geom
            .iter()
            .filter_map(|g| match g {
                BMElem::Face(f) => Some(*f),
                _ => None,
            })
            .collect();
        reverse_faces(
            bm,
            &ReverseFacesIn {
                faces: dupe_faces,
                flip_multires: false,
            },
        )?;
        // Weld the copies of the cut vertices onto the originals.
        let targetmap: Vec<(VertId, VertId)> = cut
            .iter()
            .filter_map(|v| {
                dupe.vert_map
                    .iter()
                    .find(|(s, _)| s == v)
                    .map(|&(_, d)| (d, *v))
            })
            .collect();
        weld_verts(
            bm,
            &WeldVertsIn {
                targetmap,
                ..WeldVertsIn::default()
            },
        )?;
        // Create the output.
        let mut out = SymmetrizeOut::default();
        for v in bm.vert_ids() {
            if bm.oflag_test(v, ELE_OUT) {
                out.geom.push(BMElem::Vert(v));
            }
        }
        for e in bm.edge_ids() {
            if bm.oflag_test(e, ELE_OUT) {
                out.geom.push(BMElem::Edge(e));
            }
        }
        for f in bm.face_ids() {
            if bm.oflag_test(f, ELE_OUT) {
                out.geom.push(BMElem::Face(f));
            }
        }
        Ok(out)
    })
}
