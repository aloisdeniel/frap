// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the bone heat port (the Blender goldens are
//! `tests/golden/cases/armature/heat/*.py`).

use super::*;

/// A closed box of `nx × ny × nz` quads split into triangles, `[-1, 1]² × [0, h]`.
fn column(n: usize, h: f32) -> (Vec<V3>, Vec<[usize; 3]>) {
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    // Rings of 4·n vertices around Z, `n` + 1 rings, plus two caps of one vertex each.
    let ring = 4 * n;
    let levels = 2 * n;
    for l in 0..=levels {
        let z = h * l as f32 / levels as f32;
        for k in 0..ring {
            let t = k as f32 / n as f32;
            let (x, y) = match k / n {
                0 => (-1.0 + 2.0 * t, -1.0),
                1 => (1.0, -1.0 + 2.0 * (t - 1.0)),
                2 => (1.0 - 2.0 * (t - 2.0), 1.0),
                _ => (-1.0, 1.0 - 2.0 * (t - 3.0)),
            };
            verts.push([x, y, z]);
        }
    }
    for l in 0..levels {
        for k in 0..ring {
            let a = l * ring + k;
            let b = l * ring + (k + 1) % ring;
            let c = a + ring;
            let d = b + ring;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    let bottom = verts.len();
    verts.push([0.0, 0.0, 0.0]);
    let top = verts.len();
    verts.push([0.0, 0.0, h]);
    for k in 0..ring {
        let a = k;
        let b = (k + 1) % ring;
        tris.push([bottom, b, a]);
        let a = levels * ring + k;
        let b = levels * ring + (k + 1) % ring;
        tris.push([top, a, b]);
    }
    (verts, tris)
}

fn weights(d: &[DeformVert], g: u32) -> Vec<f32> {
    d.iter().map(|d| dv::weight(d, g)).collect()
}

fn run(
    verts: &[V3],
    tris: &[[usize; 3]],
    bones: &[(V3, V3)],
    groups: &[Option<u32>],
    flips: &[Option<u32>],
    mirror: Option<&[Option<usize>]>,
) -> (Vec<DeformVert>, Option<&'static str>) {
    let roots: Vec<V3> = bones.iter().map(|b| b.0).collect();
    let tips: Vec<V3> = bones.iter().map(|b| b.1).collect();
    let selected = vec![true; bones.len()];
    let inp = HeatInput {
        verts,
        tris,
        roots: &roots,
        tips: &tips,
        groups,
        flips,
        selected: &selected,
        mirror,
    };
    let mut d = vec![DeformVert::default(); verts.len()];
    let e = heat_bone_weighting(&mut d, &inp);
    (d, e)
}

#[test]
fn limit_weight_ramps_between_the_thresholds() {
    assert_eq!(heat_limit_weight(0.02), 0.0);
    assert_eq!(heat_limit_weight(0.025), 0.0);
    assert!((heat_limit_weight(0.0375) - 0.025).abs() < 1e-7);
    assert_eq!(heat_limit_weight(0.05), 0.05);
    assert_eq!(heat_limit_weight(0.7), 0.7);
    assert!(heat_limit_weight(f32::NAN).is_nan());
}

#[test]
fn one_bone_weights_everything_fully() {
    let (verts, tris) = column(3, 4.0);
    let (d, e) = run(
        &verts,
        &tris,
        &[([0.0, 0.0, 0.5], [0.0, 0.0, 3.5])],
        &[Some(0)],
        &[None],
        None,
    );
    assert_eq!(e, None);
    for w in weights(&d, 0) {
        assert!((w - 1.0).abs() < 1e-5, "{w}");
    }
}

#[test]
fn a_chain_partitions_unity() {
    let (verts, tris) = column(3, 6.0);
    let bones = [
        ([0.0, 0.0, 0.2], [0.0, 0.0, 2.0]),
        ([0.0, 0.0, 2.0], [0.0, 0.0, 4.0]),
        ([0.0, 0.0, 4.0], [0.0, 0.0, 5.8]),
    ];
    let (d, e) = run(
        &verts,
        &tris,
        &bones,
        &[Some(0), Some(1), Some(2)],
        &[None; 3],
        None,
    );
    assert_eq!(e, None);
    let (w0, w1, w2) = (weights(&d, 0), weights(&d, 1), weights(&d, 2));
    for (i, v) in verts.iter().enumerate() {
        // Heat weights sum to one before the low-weight cut.
        let s = w0[i] + w1[i] + w2[i];
        assert!(s > 0.9 && s < 1.0 + 1e-4, "vertex {i} sums to {s}");
        if v[2] < 1.0 {
            assert!(w0[i] > 0.9 && w2[i] == 0.0, "vertex {i}");
        }
        if v[2] > 5.0 {
            assert!(w2[i] > 0.9 && w0[i] == 0.0, "vertex {i}");
        }
    }
}

#[test]
fn locked_groups_and_missing_groups_are_left_alone() {
    let (verts, tris) = column(2, 4.0);
    let bones = [
        ([0.0, 0.0, 0.2], [0.0, 0.0, 2.0]),
        ([0.0, 0.0, 2.0], [0.0, 0.0, 3.8]),
    ];
    let mut d0 = vec![DeformVert::default(); verts.len()];
    dv::vert_add(&mut d0[0], 7, 0.25, dv::AssignMode::Replace);
    let roots: Vec<V3> = bones.iter().map(|b| b.0).collect();
    let tips: Vec<V3> = bones.iter().map(|b| b.1).collect();
    let inp = HeatInput {
        verts: &verts,
        tris: &tris,
        roots: &roots,
        tips: &tips,
        groups: &[None, Some(1)],
        flips: &[None, None],
        selected: &[true, true],
        mirror: None,
    };
    assert_eq!(heat_bone_weighting(&mut d0, &inp), None);
    assert_eq!(dv::weight(&d0[0], 7), 0.25);
    assert!(d0.iter().all(|d| dv::find(d, 0).is_none()));
    assert!(d0.iter().any(|d| dv::weight(d, 1) > 0.5));
}

#[test]
fn mirror_writes_the_flipped_group() {
    // A plane strip symmetric in X, one bone on the +X side; its weights go mirrored into
    // group 1.
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    let nx = 8;
    for j in 0..3 {
        for i in 0..=nx {
            verts.push([-2.0 + 4.0 * i as f32 / nx as f32, 0.0, j as f32 * 0.5]);
        }
    }
    for j in 0..2 {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            tris.push([a, a + 1, a + nx + 2]);
            tris.push([a, a + nx + 2, a + nx + 1]);
        }
    }
    let mirror: Vec<Option<usize>> = (0..verts.len())
        .map(|k| {
            let (j, i) = (k / (nx + 1), k % (nx + 1));
            Some(j * (nx + 1) + (nx - i))
        })
        .collect();
    let bones = [
        ([0.5, 0.2, 0.5], [2.0, 0.2, 0.5]),
        ([-0.5, 0.2, 0.5], [-2.0, 0.2, 0.5]),
    ];
    let (d, e) = run(
        &verts,
        &tris,
        &bones,
        &[Some(0), Some(2)],
        &[Some(1), None],
        Some(&mirror),
    );
    assert_eq!(e, None);
    let (w0, w1) = (weights(&d, 0), weights(&d, 1));
    for k in 0..verts.len() {
        let m = mirror[k].unwrap();
        assert_eq!(w0[k], w1[m], "vertex {k}");
    }
    assert!(w0.iter().any(|&w| w > 0.9));
}

#[test]
fn vertices_without_triangles_see_every_bone() {
    let verts = [[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]];
    let bones = [
        ([0.0, 1.0, 0.0], [0.0, 2.0, 0.0]),
        ([5.0, 1.0, 0.0], [5.0, 2.0, 0.0]),
    ];
    let (d, e) = run(
        &verts,
        &[],
        &bones,
        &[Some(0), Some(1)],
        &[None, None],
        None,
    );
    assert_eq!(e, None);
    // No Laplacian: each vertex takes its nearest bone fully (`H p / H`).
    assert!((dv::weight(&d[0], 0) - 1.0).abs() < 1e-6);
    assert_eq!(dv::weight(&d[0], 1), 0.0);
    assert!((dv::weight(&d[1], 1) - 1.0).abs() < 1e-6);
}

#[test]
fn occluding_faces_block_the_visibility_rays() {
    // A small quad below a big quad facing it: the bone above sees the big quad's vertices,
    // the small quad's rays hit the big quad's front.
    let verts = vec![
        [-5.0, -5.0, 1.0],
        [5.0, -5.0, 1.0],
        [5.0, 5.0, 1.0],
        [-5.0, 5.0, 1.0],
        [-0.5, -0.5, 0.0],
        [0.5, -0.5, 0.0],
        [0.5, 0.5, 0.0],
        [-0.5, 0.5, 0.0],
    ];
    // The big quad faces down (-Z), toward the small one.
    let big = [[0, 2, 1], [0, 3, 2]];
    let small = [[4, 5, 6], [4, 6, 7]];
    let tris: Vec<[usize; 3]> = big.iter().chain(small.iter()).copied().collect();
    let bones = [([0.0, 0.0, 3.0], [0.0, 0.0, 4.0])];
    let roots = [bones[0].0];
    let tips = [bones[0].1];
    let inp = HeatInput {
        verts: &verts,
        tris: &tris,
        roots: &roots,
        tips: &tips,
        groups: &[Some(0)],
        flips: &[None],
        selected: &[true],
        mirror: None,
    };
    let sys = HeatSystem::new(&inp);
    for v in 0..4 {
        assert!(sys.source_visible(v, 0), "big quad vertex {v}");
    }
    for v in 4..8 {
        assert!(!sys.source_visible(v, 0), "small quad vertex {v}");
        assert_eq!(sys.h[v], 0.0);
    }
}

#[test]
fn nan_coordinates_fail_with_blenders_report() {
    let verts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, f32::NAN, 0.0]];
    let tris = [[0, 1, 2]];
    let bones = [
        ([0.0, 0.0, 3.0], [0.0, 0.0, 4.0]),
        ([2.0, 0.0, 3.0], [2.0, 0.0, 4.0]),
    ];
    let (d, e) = run(
        &verts,
        &tris,
        &bones,
        &[Some(0), Some(1)],
        &[None, None],
        None,
    );
    assert_eq!(e, Some(HEAT_FAILED));
    assert!(d.iter().all(|d| d.weights.is_empty()));
}
