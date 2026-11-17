// SPDX-License-Identifier: GPL-3.0-or-later
//! Cache invariant (ARCHITECTURE §6.1, decision log #16): whatever sequence of writes goes
//! through `Mesh`'s `&mut` API, the cached normals, loop triangles and bounds always equal a
//! fresh computation (a clone starts with empty caches, so `clone()` gives the fresh values).

use std::sync::Arc;

use frap_mesh::primitives::grid::{Grid, grid};
use frap_mesh::split::corner_split;
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Write {
    MovePositions(u32, [f32; 3]),
    SetPositionsAttr(u32, f32),
    SharpFaces(Vec<bool>),
    SharpEdges(Vec<bool>),
    ShadeFlat,
    ShadeSmooth,
    CustomNormals(i16, i16),
    DropCustom,
    Retopo,
    DeformVerts,
    CalcEdges,
}

fn write_strategy() -> impl Strategy<Value = Write> {
    let f = -2.0f32..2.0;
    prop_oneof![
        (any::<u32>(), [f.clone(), f.clone(), f.clone()])
            .prop_map(|(i, d)| Write::MovePositions(i, d)),
        (any::<u32>(), f.clone()).prop_map(|(i, z)| Write::SetPositionsAttr(i, z)),
        prop::collection::vec(any::<bool>(), 9).prop_map(Write::SharpFaces),
        prop::collection::vec(any::<bool>(), 24).prop_map(Write::SharpEdges),
        Just(Write::ShadeFlat),
        Just(Write::ShadeSmooth),
        (-32767i16..32767, -32767i16..32767).prop_map(|(a, b)| Write::CustomNormals(a, b)),
        Just(Write::DropCustom),
        Just(Write::Retopo),
        Just(Write::DeformVerts),
        Just(Write::CalcEdges),
    ]
}

fn apply(m: &mut Mesh, w: &Write) {
    let nv = m.verts_num() as u32;
    match w {
        Write::MovePositions(i, d) => {
            let p = &mut m.positions_mut()[(*i % nv) as usize];
            *p = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        }
        Write::SetPositionsAttr(i, z) => {
            if let Some(Attribute {
                data: AttrData::Float3(v),
                ..
            }) = m.attributes_mut().get_mut("position")
            {
                Arc::make_mut(v)[(*i % nv) as usize][2] = *z;
            }
        }
        Write::SharpFaces(v) => {
            let n = m.faces_num();
            m.set_bool_attribute(
                frap_mesh::SHARP_FACE,
                Domain::Face,
                v.iter().copied().cycle().take(n).collect(),
            );
        }
        Write::SharpEdges(v) => {
            let n = m.edges_num();
            m.set_bool_attribute(
                frap_mesh::SHARP_EDGE,
                Domain::Edge,
                v.iter().copied().cycle().take(n).collect(),
            );
        }
        Write::ShadeFlat => m.shade_flat(),
        Write::ShadeSmooth => m.shade_smooth(),
        Write::CustomNormals(a, b) => {
            let n = m.corners_num();
            m.attributes_mut().insert(Attribute {
                name: frap_mesh::CUSTOM_NORMAL.into(),
                domain: Domain::Corner,
                data: AttrData::Int16x2(Arc::new(vec![[*a, *b]; n])),
            });
        }
        Write::DropCustom => {
            m.attributes_mut().remove(frap_mesh::CUSTOM_NORMAL);
        }
        Write::Retopo => {
            // Re-triangulate the first quad: same vertices, new topology through set_topology.
            let mut offsets = m.face_offsets().to_vec();
            let mut cv = m.corner_verts().to_vec();
            if offsets.len() > 1 && offsets[1] - offsets[0] == 4 {
                let q = [cv[0], cv[1], cv[2], cv[3]];
                cv.splice(0..4, [q[0], q[1], q[2], q[0], q[2], q[3]]);
                offsets.insert(1, 3);
                for o in offsets.iter_mut().skip(2) {
                    *o += 2;
                }
                let mut edges = m.edges().to_vec();
                edges.push([q[0], q[2]]);
                m.attributes_mut().remove(frap_mesh::SHARP_EDGE);
                m.attributes_mut().remove(frap_mesh::SHARP_FACE);
                m.attributes_mut().remove(frap_mesh::CUSTOM_NORMAL);
                m.attributes_mut().remove(".select_poly");
                m.attributes_mut().remove(".select_edge");
                m.attributes_mut().remove("UVMap");
                m.attributes_mut().remove(".uv_select_vert");
                m.attributes_mut().remove(".uv_select_edge");
                m.attributes_mut().remove(".uv_select_face");
                m.set_topology(offsets, cv, edges);
            }
        }
        Write::DeformVerts => {
            let dv = m.deform_verts_mut();
            if let Some(d) = dv.first_mut() {
                d.weights.push((0, 0.5));
            }
        }
        Write::CalcEdges => m.calc_edges(true, false),
    }
}

/// Face, vertex and corner normals, loop triangles, bounds.
type Derived = (
    Vec<[f32; 3]>,
    Vec<[f32; 3]>,
    Vec<[f32; 3]>,
    Vec<[u32; 3]>,
    Option<[[f32; 3]; 2]>,
);

fn read_all(m: &Mesh) -> Derived {
    (
        m.face_normals().to_vec(),
        m.vert_normals().to_vec(),
        m.corner_normals().to_vec(),
        m.looptris().to_vec(),
        m.bounds(),
    )
}

fn bits(v: &Derived) -> Vec<u32> {
    let mut out = Vec::new();
    for s in [&v.0, &v.1, &v.2] {
        out.extend(s.iter().flatten().map(|f| f.to_bits()));
    }
    out.extend(v.3.iter().flatten());
    if let Some(b) = v.4 {
        out.extend(b.iter().flatten().map(|f| f.to_bits()));
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn cached_values_always_equal_fresh(writes in prop::collection::vec((write_strategy(), any::<bool>()), 1..12)) {
        let mut m = grid(&Grid { x_subdivisions: 3, y_subdivisions: 3, ..Default::default() });
        let _ = read_all(&m);
        for (w, read_between) in &writes {
            apply(&mut m, w);
            if *read_between {
                let cached = read_all(&m);
                let fresh = read_all(&m.clone());
                prop_assert_eq!(bits(&cached), bits(&fresh), "after {:?}", w);
            }
        }
        let cached = read_all(&m);
        let fresh = read_all(&m.clone());
        prop_assert_eq!(bits(&cached), bits(&fresh));
    }
}

#[test]
fn clone_recomputes_after_its_own_write() {
    let a = grid(&Grid::default());
    let before = a.face_normals();
    let mut b = a.clone();
    b.positions_mut()[0][2] += 1.0;
    assert_eq!(a.face_normals(), before, "the original keeps its caches");
    assert_ne!(b.face_normals()[0], before[0], "the clone recomputes");
}

#[test]
fn default_cube_corner_split_is_24() {
    let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
    let s = corner_split(&cube);
    assert_eq!(s.len(), 24);
    assert_eq!(s.corner_vert.len(), 24);
    // Without UVs the flat normals still split every corner of the cube.
    let s = corner_split(&frap_mesh::primitives::cube(2.0, false, [1.0; 3]));
    assert_eq!(s.len(), 24);
    // Smooth without UVs: one split vertex per mesh vertex.
    let mut smooth = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
    smooth.shade_smooth();
    assert_eq!(corner_split(&smooth).len(), 8);
    // Smooth with the cube's UV seams: Blender's glTF export gives 14 vertices.
    let mut smooth_uv = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
    smooth_uv.shade_smooth();
    assert_eq!(corner_split(&smooth_uv).len(), 14);
}
