// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.reverse_faces` (Blender 5.2.4 `bmo_reverse_faces_exec`, `operators/bmo_utils.cc`),
//! implemented by WP 2.9 (Normals, marks, attributes).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.reverse_faces(bmesh, faces=[], flip_multires=False)`.
//!
//! **Create/kill order**: nothing is created or killed. Each input face, in slot order, is
//! flipped with `BM_face_normal_flip_ex` (`bmesh_kernel_loop_reverse`): the loop cycle is
//! reversed around `l_first`, which stays the face's first corner ([MESH §4.9]: `[7,3,1,5]`
//! becomes `[7,5,1,3]`), every loop keeps its vertex and its custom data (UVs, colours stay on
//! their corner), the loops move to the edge of their new successor, and the normal is
//! negated. A face listed twice is flipped twice, as Blender does. `flip_multires` only
//! concerns multires displacement (`CD_MDISPS`), which frap does not store.
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC` (a top-level call updates face and
//! vertex normals afterwards; the selection is left as it is).

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `reverse_faces` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReverseFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `flip_multires` (no effect: frap stores no multires displacement).
    pub flip_multires: bool,
}

/// Output slots of `reverse_faces` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ReverseFacesOut {
    /// Always empty (`bmesh.ops.reverse_faces` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_reverse_faces_def` (`UNTAN_MULTIRES` has no frap equivalent).
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

/// Runs `reverse_faces` on `bm`.
pub fn reverse_faces(bm: &mut BMesh, i: &ReverseFacesIn) -> Result<ReverseFacesOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "reverse_faces: dead face {dead:?} in faces"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        for &f in &i.faces {
            frap_math::budget::check();
            bm.face_normal_flip(f);
        }
    });
    Ok(ReverseFacesOut::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CreateFlag;

    /// A quad `0 1 2 3` in the XY plane and a triangle sharing edge 1-2.
    fn two_faces() -> (BMesh, FaceId, FaceId) {
        let mut bm = BMesh::new();
        let co = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.5, 0.0],
        ];
        let v: Vec<_> = co
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        let q = bm
            .face_create_verts(&[v[0], v[1], v[2], v[3]], None, CreateFlag::empty(), true)
            .expect("quad");
        let t = bm
            .face_create_verts(&[v[1], v[4], v[2]], None, CreateFlag::empty(), true)
            .expect("triangle");
        bm.normals_update();
        (bm, q, t)
    }

    #[test]
    fn keeps_first_loop_and_reverses_order() {
        let (mut bm, q, _) = two_faces();
        let before = bm.face_verts(q);
        let l_first = bm.face(q).l_first;
        reverse_faces(
            &mut bm,
            &ReverseFacesIn {
                faces: vec![q],
                ..Default::default()
            },
        )
        .expect("reverse");
        let after = bm.face_verts(q);
        assert_eq!(bm.face(q).l_first, l_first);
        assert_eq!(after[0], before[0]);
        assert_eq!(&after[1..], &[before[3], before[2], before[1]]);
        assert_eq!(bm.face(q).no, [0.0, 0.0, -1.0]);
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
    }

    #[test]
    fn twice_is_identity() {
        let (mut bm, q, t) = two_faces();
        let before = (bm.face_verts(q), bm.face_verts(t));
        let input = ReverseFacesIn {
            faces: vec![q, t, q, t],
            ..Default::default()
        };
        reverse_faces(&mut bm, &input).expect("reverse");
        assert_eq!((bm.face_verts(q), bm.face_verts(t)), before);
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
    }

    #[test]
    fn dead_face_is_invalid() {
        let (mut bm, q, _) = two_faces();
        bm.face_kill(q);
        let r = reverse_faces(
            &mut bm,
            &ReverseFacesIn {
                faces: vec![q],
                ..Default::default()
            },
        );
        assert!(matches!(r, Err(BmError::Invalid(_))));
    }
}
