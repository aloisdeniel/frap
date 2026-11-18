// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.pointmerge` (Blender 5.2.4 `bmo_pointmerge_exec`, `operators/bmo_removedoubles.cc`),
//! implemented by WP 2.7 (Delete, dissolve, merge), with its companions
//! `bmesh.ops.pointmerge_facedata` and `bmesh.ops.average_vert_facedata` (used by
//! `mesh.merge(uvs=True)`).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.pointmerge(bmesh, verts=[], merge_co=Vector(), vert_target=None)`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/
//! bmo_removedoubles.cc): without `vert_target` the first vertex of `verts` becomes the target
//! and moves to `merge_co` (and the vertex data of each cluster is averaged); every other
//! vertex of `verts` maps to the target and `weld_verts` runs as a nested operator (see its
//! notes: edges and faces are rebuilt in place, the merged vertices are killed last). The
//! target keeps its slot, so it keeps the lowest index of the merged set unless `vert_target`
//! names another one.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE` (`pointmerge`); none
//! for the two face-data operators.

use crate::ops::dissolve_verts::mods::{self, MinMax};
use crate::ops::weld_verts::{self, TargetMap};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `pointmerge` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointmergeIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `merge_co`.
    pub merge_co: [f32; 3],
    /// `vert_target`.
    pub vert_target: Option<VertId>,
}

/// Output slots of `pointmerge` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PointmergeOut {
    /// Always empty (`bmesh.ops.pointmerge` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_pointmerge_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

fn check_alive(bm: &BMesh, op: &str, verts: &[VertId], target: Option<VertId>) -> Result<(), BmError> {
    if let Some(v) = verts
        .iter()
        .chain(target.iter())
        .find(|v| !bm.verts.contains(v.0))
    {
        return Err(BmError::Invalid(format!("{op}: dead vertex {v:?}")));
    }
    Ok(())
}

/// Runs `pointmerge` on `bm`.
pub fn pointmerge(bm: &mut BMesh, i: &PointmergeIn) -> Result<PointmergeOut, BmError> {
    check_alive(bm, "pointmerge", &i.verts, i.vert_target)?;
    bm.bmo_exec(TYPE_FLAG, |bm| {
        let mut map = TargetMap::default();
        let is_explicit_snap = i.vert_target.is_some();
        let mut vert_target = i.vert_target;
        for &v in &i.verts {
            match vert_target {
                None => {
                    vert_target = Some(v);
                    bm.vert_mut(v).co = i.merge_co;
                }
                Some(t) if t != v => {
                    map.insert(v, t);
                }
                Some(_) => {}
            }
        }
        bm.bmo_exec(weld_verts::TYPE_FLAG, |bm| {
            weld_verts::weld_exec(bm, &map, false, !is_explicit_snap);
        });
    });
    Ok(PointmergeOut::default())
}

/// Input slots of `pointmerge_facedata`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointmergeFacedataIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `vert_target`.
    pub vert_target: Option<VertId>,
}

/// `bmesh.ops.pointmerge_facedata` (`bmo_pointmerge_facedata_exec`): the corners of
/// `vert_target` average their UV/colour data into its first corner, which is then copied
/// (every loop layer) to every other corner of `verts`.
pub fn pointmerge_facedata(bm: &mut BMesh, i: &PointmergeFacedataIn) -> Result<(), BmError> {
    let Some(target) = i.vert_target else {
        return Err(BmError::Invalid(
            "pointmerge_facedata: vert_target is required".into(),
        ));
    };
    check_alive(bm, "pointmerge_facedata", &i.verts, Some(target))?;
    bm.bmo_exec(OpTypeFlag::empty(), |bm| {
        let tot = bm.vert_face_count(target);
        if tot == 0 {
            return;
        }
        let fac = 1.0 / tot as f32;
        let layers = mods::math_layers(bm);
        let mut l_first = None;
        for l in bm.vert_loops(target) {
            let first = *l_first.get_or_insert(l);
            for &layer in &layers {
                let mut e2 = mods::math_get(bm, layer, l);
                e2.multiply(fac);
                mods::math_set(bm, layer, l, e2);
                if l != first {
                    let mut e1 = mods::math_get(bm, layer, first);
                    e1.add(&e2);
                    mods::math_set(bm, layer, first, e1);
                }
            }
        }
        let Some(first) = l_first else {
            return;
        };
        for &v in &i.verts {
            for l in bm.vert_loops(v) {
                if l != first {
                    bm.ldata.copy_elem(first.0, l.0);
                }
            }
        }
    });
    Ok(())
}

/// `bmesh.ops.average_vert_facedata` (`bmo_average_vert_facedata_exec`): every corner of
/// `verts` gets, per UV/colour layer, the centre of the bounding box of their values.
pub fn average_vert_facedata(bm: &mut BMesh, verts: &[VertId]) -> Result<(), BmError> {
    check_alive(bm, "average_vert_facedata", verts, None)?;
    bm.bmo_exec(OpTypeFlag::empty(), |bm| {
        let loops: Vec<_> = verts.iter().flat_map(|&v| bm.vert_loops(v)).collect();
        for layer in mods::math_layers(bm) {
            let Some(&l0) = loops.first() else {
                return;
            };
            let mut mm = MinMax::new(&mods::math_get(bm, layer, l0));
            for &l in &loops {
                mm.add(&mods::math_get(bm, layer, l));
            }
            let c = mm.center();
            for &l in &loops {
                mods::math_set(bm, layer, l, c);
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    fn quad() -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        bm.face_create_verts(&v, None, CreateFlag::empty(), true);
        (bm, v)
    }

    #[test]
    fn merge_at_point_keeps_the_first_vertex() {
        let (mut bm, v) = quad();
        let i = PointmergeIn {
            verts: vec![v[0], v[1]],
            merge_co: [0.5, 0.0, 0.0],
            vert_target: None,
        };
        pointmerge(&mut bm, &i).unwrap();
        assert_eq!(bm.verts_num(), 3);
        assert_eq!(bm.vert(v[0]).co, [0.5, 0.0, 0.0]);
        assert_eq!(bm.faces_num(), 1);
        assert_eq!(bm.face(crate::FaceId(0)).len, 3);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn explicit_target_keeps_its_position() {
        let (mut bm, v) = quad();
        let i = PointmergeIn {
            verts: vec![v[0], v[1], v[2]],
            merge_co: [9.0, 9.0, 9.0],
            vert_target: Some(v[2]),
        };
        pointmerge(&mut bm, &i).unwrap();
        assert_eq!(bm.verts_num(), 2);
        assert_eq!(bm.vert(v[2]).co, [1.0, 1.0, 0.0]);
        assert_eq!(bm.faces_num(), 0);
        assert!(bm.validate().is_empty());
    }
}
