// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 2.1 acceptance: `BMesh::from_mesh` → `to_mesh` is the identity (T1) on every primitive
//! frap generates (`frap_mesh::primitives`, each verified against Blender at T1): same vertex,
//! edge, face and corner order, positions, and every attribute with the same values. The
//! attribute **order** is identical too, except for the torus: it is a Python operator
//! (`from_pydata`), and Blender's own edit-mode round trip reorders its attributes into the
//! `BM_mesh_bm_to_me` order (checked against Blender in `core_vs_blender.rs`,
//! `roundtrip/torus`). The edit-mode conversion (`edit_mesh_make`) leaves the primitives'
//! all-selected state unchanged as well.

use frap_bmesh::{BMesh, FromMeshParams, SelectMode, ToMeshParams};
use frap_mesh::primitives::circle::{Circle, circle};
use frap_mesh::primitives::cone::{Cone, FillType, cone};
use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::grid::{Grid, grid};
use frap_mesh::primitives::ico_sphere::{IcoSphere, ico_sphere};
use frap_mesh::primitives::monkey::{Monkey, monkey};
use frap_mesh::primitives::plane::{Plane, plane};
use frap_mesh::primitives::scale_matrix;
use frap_mesh::primitives::torus::{Torus, torus};
use frap_mesh::primitives::uv_sphere::{UvSphere, uv_sphere};
use frap_mesh::{AttrData, Mesh};

fn primitives() -> Vec<(&'static str, Mesh)> {
    vec![
        ("plane", plane(&Plane::default())),
        (
            "plane_nouv",
            plane(&Plane {
                size: 3.5,
                calc_uvs: false,
                ..Plane::default()
            }),
        ),
        ("cube", cube_mesh(&Cube::default())),
        (
            "cube_scaled",
            cube_mesh(&Cube {
                size: 1.0,
                matrix: scale_matrix([1.0, 2.0, 3.0]),
                ..Cube::default()
            }),
        ),
        (
            "cube_nouv",
            cube_mesh(&Cube {
                size: 3.0,
                calc_uvs: false,
                ..Cube::default()
            }),
        ),
        ("circle", circle(&Circle::default())),
        (
            "circle_ngon",
            circle(&Circle {
                vertices: 5,
                radius: 2.0,
                fill_type: FillType::Ngon,
                ..Circle::default()
            }),
        ),
        (
            "circle_trifan",
            circle(&Circle {
                vertices: 7,
                fill_type: FillType::Trifan,
                ..Circle::default()
            }),
        ),
        ("cone", cone(&Cone::default())),
        (
            "cone_trifan",
            cone(&Cone {
                vertices: 6,
                radius2: 0.5,
                end_fill_type: FillType::Trifan,
                ..Cone::default()
            }),
        ),
        (
            "cone_open",
            cone(&Cone {
                vertices: 8,
                radius1: 0.0,
                radius2: 1.0,
                end_fill_type: FillType::Nothing,
                ..Cone::default()
            }),
        ),
        ("cylinder", cylinder(&Cylinder::default())),
        (
            "cylinder_trifan",
            cylinder(&Cylinder {
                vertices: 6,
                depth: 3.0,
                end_fill_type: FillType::Trifan,
                ..Cylinder::default()
            }),
        ),
        (
            "cylinder_open_nouv",
            cylinder(&Cylinder {
                vertices: 3,
                end_fill_type: FillType::Nothing,
                calc_uvs: false,
                ..Cylinder::default()
            }),
        ),
        ("grid", grid(&Grid::default())),
        (
            "grid_3x5",
            grid(&Grid {
                x_subdivisions: 3,
                y_subdivisions: 5,
                size: 4.0,
                ..Grid::default()
            }),
        ),
        (
            "grid_1x1_nouv",
            grid(&Grid {
                x_subdivisions: 1,
                y_subdivisions: 1,
                calc_uvs: false,
                ..Grid::default()
            }),
        ),
        ("uv_sphere", uv_sphere(&UvSphere::default())),
        (
            "uv_sphere_small",
            uv_sphere(&UvSphere {
                segments: 5,
                ring_count: 3,
                radius: 0.5,
                ..UvSphere::default()
            }),
        ),
        ("ico_sphere", ico_sphere(&IcoSphere::default())),
        (
            "ico_sphere_1",
            ico_sphere(&IcoSphere {
                subdivisions: 1,
                radius: 2.0,
                ..IcoSphere::default()
            }),
        ),
        (
            "ico_sphere_4_nouv",
            ico_sphere(&IcoSphere {
                subdivisions: 4,
                calc_uvs: false,
                ..IcoSphere::default()
            }),
        ),
        ("monkey", monkey(&Monkey::default())),
        (
            "monkey_nouv",
            monkey(&Monkey {
                size: 4.0,
                calc_uvs: false,
                ..Monkey::default()
            }),
        ),
        ("torus", torus(&Torus::default())),
        (
            "torus_small",
            torus(&Torus {
                major_segments: 12,
                minor_segments: 6,
                generate_uvs: false,
                ..Torus::default()
            }),
        ),
    ]
}

fn bits(d: &AttrData) -> Vec<u64> {
    match d {
        AttrData::Bool(v) => v.iter().map(|&b| b as u64).collect(),
        AttrData::Float(v) => v.iter().map(|x| x.to_bits() as u64).collect(),
        AttrData::Int(v) => v.iter().map(|&x| x as u32 as u64).collect(),
        AttrData::Int8(v) => v.iter().map(|&x| x as u8 as u64).collect(),
        AttrData::Float2(v) => v.iter().flatten().map(|x| x.to_bits() as u64).collect(),
        AttrData::Float3(v) => v.iter().flatten().map(|x| x.to_bits() as u64).collect(),
        AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            v.iter().flatten().map(|x| x.to_bits() as u64).collect()
        }
        AttrData::ByteColor(v) => v.iter().flatten().map(|&x| x as u64).collect(),
        AttrData::Int16x2(v) => v.iter().flatten().map(|&x| x as u16 as u64).collect(),
        AttrData::Int32x2(v) => v.iter().flatten().map(|&x| x as u32 as u64).collect(),
        other => panic!("unexpected column {}", other.data_type()),
    }
}

/// Every difference: topology, then each attribute by name (domain, type, values bit-exact),
/// then the order when `check_order`.
fn diff(a: &Mesh, b: &Mesh, check_order: bool) -> Vec<String> {
    let mut out = Vec::new();
    if a.face_offsets() != b.face_offsets() {
        out.push("face offsets differ".into());
    }
    if a.edges() != b.edges()
        || a.corner_verts() != b.corner_verts()
        || a.corner_edges() != b.corner_edges()
    {
        out.push("topology differs".into());
    }
    // Without `check_order` (the torus) the UV selection attributes `bm_to_me` adds next to a
    // UV map are ignored too: Blender adds them the same way (`roundtrip/torus` golden).
    let names = |m: &Mesh| {
        m.attributes()
            .iter()
            .map(|x| x.name.to_string())
            .filter(|n| check_order || !n.starts_with(".uv_select_"))
            .collect::<Vec<_>>()
    };
    let (na, nb) = (names(a), names(b));
    let mut sa = na.clone();
    let mut sb = nb.clone();
    sa.sort();
    sb.sort();
    if sa != sb {
        out.push(format!("attribute sets differ: {na:?} vs {nb:?}"));
    } else if check_order && na != nb {
        out.push(format!("attribute order differs: {na:?} vs {nb:?}"));
    }
    for x in a.attributes().iter() {
        let Some(y) = b.attributes().get(&x.name) else {
            continue;
        };
        if x.domain != y.domain
            || x.data.data_type() != y.data.data_type()
            || bits(&x.data) != bits(&y.data)
        {
            out.push(format!("attribute {} differs", x.name));
        }
    }
    if a.uv_active != b.uv_active || a.uv_active_render != b.uv_active_render {
        out.push(format!(
            "active UV maps differ: {:?}/{:?} vs {:?}/{:?}",
            a.uv_active, a.uv_active_render, b.uv_active, b.uv_active_render
        ));
    }
    out
}

#[test]
fn from_mesh_to_mesh_is_identity_on_every_primitive() {
    let mut failures = Vec::new();
    for (name, me) in primitives() {
        let bm = BMesh::from_mesh(&me, &FromMeshParams::default());
        assert!(bm.validate().is_empty(), "{name}: {:?}", bm.validate());
        assert_eq!(
            (
                bm.verts_num(),
                bm.edges_num(),
                bm.loops_num(),
                bm.faces_num()
            ),
            (
                me.verts_num(),
                me.edges_num(),
                me.corners_num(),
                me.faces_num()
            ),
            "{name}: element counts"
        );
        let out = bm.to_mesh(&me, &ToMeshParams::default());
        let check_order = !name.starts_with("torus");
        for d in diff(&out, &me, check_order) {
            failures.push(format!("{name}: {d}"));
        }
        // Entering Edit Mode (with the selection flush) changes nothing either.
        for mode in [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE] {
            let out = BMesh::edit_mesh_make(&me, mode).to_mesh(&me, &ToMeshParams::default());
            for d in diff(&out, &me, check_order) {
                failures.push(format!("{name} (edit, {mode:?}): {d}"));
            }
        }
        // A second round trip is stable for every primitive (the torus included).
        let again = BMesh::from_mesh(&out, &FromMeshParams::default())
            .to_mesh(&out, &ToMeshParams::default());
        for d in diff(&again, &out, true) {
            failures.push(format!("{name} (second trip): {d}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
