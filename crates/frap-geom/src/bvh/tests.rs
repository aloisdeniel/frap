// SPDX-License-Identifier: GPL-3.0-or-later
//! `BvhTree` against brute force (every query, every tree type and k-DOP), and against the
//! orders Blender 5.2.4's `mathutils.bvhtree` produced on the same triangles
//! (`bvh/fixtures/*.txt`, written by `bvh/fixtures/gen_bvh_fixtures.py`).

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// A deterministic LCG; coordinates are multiples of 1/1024 (exact in `f32` and in Python).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// A coordinate in `[-4, 4)` on the 1/1024 grid.
    fn coord(&mut self) -> f32 {
        (self.next() % 8192) as f32 / 1024.0 - 4.0
    }

    /// A small offset in `[-0.5, 0.5)`.
    fn offset(&mut self) -> f32 {
        (self.next() % 1024) as f32 / 1024.0 - 0.5
    }
}

/// `n` small triangles scattered in a box.
fn triangles(seed: u64, n: usize) -> Vec<[V3; 3]> {
    let mut r = Lcg(seed);
    (0..n)
        .map(|_| {
            let c = [r.coord(), r.coord(), r.coord()];
            let mut v = || [c[0] + r.offset(), c[1] + r.offset(), c[2] + r.offset()];
            [v(), v(), v()]
        })
        .collect()
}

fn build(tris: &[[V3; 3]], epsilon: f32, tree_type: u8, axis: u8) -> BvhTree {
    let mut t = BvhTree::new(tris.len(), epsilon, tree_type, axis);
    for (i, tri) in tris.iter().enumerate() {
        t.insert(i, tri);
    }
    t.balance();
    t
}

/// The k-DOP of a triangle as `insert` computes it, independently of the tree.
fn kdop(tri: &[V3; 3], epsilon: f32, axes: std::ops::Range<usize>) -> Vec<(f32, f32)> {
    let eps = if f32::EPSILON > epsilon {
        f32::EPSILON
    } else {
        epsilon
    };
    axes.map(|a| {
        let mut lo = f32::MAX;
        let mut hi = -f32::MAX;
        for p in tri {
            let d = p[0] * KDOP_AXES[a][0] + p[1] * KDOP_AXES[a][1] + p[2] * KDOP_AXES[a][2];
            lo = lo.min(d);
            hi = hi.max(d);
        }
        let e = eps * KDOP_AXES_LENGTH[a];
        (lo - e, hi + e)
    })
    .collect()
}

fn axis_range(axis: u8) -> std::ops::Range<usize> {
    match axis {
        26 => 0..13,
        18 => 7..13,
        14 => 0..7,
        8 => 0..4,
        _ => 0..3,
    }
}

const TREE_TYPES: [u8; 6] = [2, 3, 4, 6, 8, 32];
const AXES: [u8; 4] = [6, 8, 14, 26];
const SIZES: [usize; 7] = [1, 2, 3, 5, 17, 64, 301];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dist_sq(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

#[test]
fn every_leaf_is_reachable_once() {
    for &tt in &TREE_TYPES {
        for &n in &SIZES {
            let tris = triangles(n as u64 * 7 + tt as u64, n);
            let t = build(&tris, 0.0, tt, 6);
            let mut seen = vec![0usize; n];
            // A huge range query reaches every leaf exactly once.
            let hits = t.range_query([0.0; 3], 1.0e6, &mut |i, _, _| seen[i] += 1);
            assert_eq!(hits, n, "tree_type {tt}, n {n}");
            assert!(seen.iter().all(|&c| c == 1), "tree_type {tt}, n {n}");
            // Branches never exceed tree_type children and bound their children.
            for slot in t.leaf_num..t.leaf_num + t.branch_num {
                let node = t.nodes[slot];
                let k = t.node_num[node as usize] as usize;
                assert!(k >= 1 && k <= tt as usize);
                for c in 0..k {
                    let ch = t.child(node, c) as usize;
                    for a in 0..3 {
                        assert!(t.bv[node as usize][2 * a] <= t.bv[ch][2 * a]);
                        assert!(t.bv[node as usize][2 * a + 1] >= t.bv[ch][2 * a + 1]);
                    }
                }
            }
        }
    }
}

#[test]
fn empty_and_unbalanced_trees_find_nothing() {
    let mut t = BvhTree::new(4, 0.0, 4, 6);
    assert!(t.is_empty());
    let none = |_: usize, _: V3, _: &mut Nearest| {};
    let mut cb = none;
    assert_eq!(t.find_nearest([0.0; 3], f32::MAX, &mut cb), None);
    t.insert(0, &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
    // Not balanced yet: no root.
    assert_eq!(t.range_query([0.0; 3], 10.0, &mut |_, _, _| {}), 0);
    t.balance();
    assert_eq!(t.range_query([0.0; 3], 10.0, &mut |_, _, _| {}), 1);
    let mut e = BvhTree::new(0, 0.0, 2, 26);
    e.balance();
    assert_eq!(e.bounding_box(), None);
    assert!(t.overlap(&e, &mut |_, _| true).is_empty());
    assert_eq!(
        e.ray_cast([0.0; 3], [1.0, 0.0, 0.0], 0.0, 10.0, &mut |_, _, _, _| {}),
        None
    );
}

#[test]
fn epsilon_is_at_least_flt_epsilon() {
    assert_eq!(BvhTree::new(1, 0.0, 2, 6).epsilon(), f32::EPSILON);
    assert_eq!(BvhTree::new(1, 0.5, 2, 6).epsilon(), 0.5);
    let mut t = BvhTree::new(1, 0.25, 2, 8);
    t.insert(0, &[[1.0, 2.0, 3.0]]);
    t.balance();
    let (lo, hi) = t.bounding_box().unwrap();
    assert_eq!(lo, [0.75, 1.75, 2.75]);
    assert_eq!(hi, [1.25, 2.25, 3.25]);
}

#[test]
fn range_query_matches_brute_force() {
    let queries = triangles(99, 40);
    for &tt in &TREE_TYPES {
        for &axis in &AXES {
            for &n in &SIZES {
                let tris = triangles(n as u64 + 1000 * tt as u64 + axis as u64, n);
                let t = build(&tris, 0.01, tt, axis);
                for (qi, q) in queries.iter().enumerate() {
                    let co = q[0];
                    let radius = 0.3 + (qi % 7) as f32 * 0.4;
                    let mut got = BTreeMap::new();
                    let hits = t.range_query(co, radius, &mut |i, c, d| {
                        assert_eq!(c, co);
                        assert!(got.insert(i, d).is_none(), "leaf {i} reported twice");
                    });
                    assert_eq!(hits, got.len());
                    let mut want = BTreeMap::new();
                    for (i, tri) in tris.iter().enumerate() {
                        let k = kdop(tri, 0.01, 0..3);
                        let p: Vec<f32> = (0..3).map(|a| co[a].max(k[a].0).min(k[a].1)).collect();
                        let d = dist_sq([p[0], p[1], p[2]], co);
                        if d < radius * radius {
                            want.insert(i, d);
                        }
                    }
                    assert_eq!(got, want, "tree_type {tt}, axis {axis}, n {n}, query {qi}");
                }
            }
        }
    }
}

#[test]
fn find_nearest_matches_brute_force() {
    let queries = triangles(7, 60);
    for &tt in &TREE_TYPES {
        for &axis in &AXES {
            for &n in &SIZES {
                let tris = triangles(n as u64 * 31 + tt as u64 + axis as u64, n);
                let t = build(&tris, 0.0, tt, axis);
                for optimal in [false, true] {
                    for q in &queries {
                        let co = q[1];
                        // Exact: the nearest vertex of the triangle.
                        let mut exact = |i: usize, c: V3, best: &mut Nearest| {
                            for p in &tris[i] {
                                let d = dist_sq(*p, c);
                                if d < best.dist_sq {
                                    *best = Nearest {
                                        index: Some(i),
                                        co: *p,
                                        no: [0.0; 3],
                                        dist_sq: d,
                                    };
                                }
                            }
                        };
                        let mut got = Nearest::within(f32::MAX);
                        t.find_nearest_ex(co, &mut got, Some(&mut exact), optimal);
                        let mut want = Nearest::within(f32::MAX);
                        for i in 0..n {
                            exact(i, co, &mut want);
                        }
                        assert_eq!(got.dist_sq, want.dist_sq, "tt {tt} axis {axis} n {n}");
                        assert_eq!(got.co, want.co);
                        // A bounded search finds nothing beyond the bound.
                        let limit = want.dist_sq * 0.5;
                        let r = t.find_nearest(co, limit, &mut exact);
                        assert_eq!(r, None);
                    }
                }
            }
        }
    }
}

#[test]
fn ray_cast_matches_brute_force() {
    let mut r = Lcg(12345);
    for &tt in &TREE_TYPES {
        for &axis in &AXES {
            for &n in &SIZES {
                let tris = triangles(n as u64 * 13 + tt as u64 * 3 + axis as u64, n);
                let t = build(&tris, 0.0, tt, axis);
                for k in 0..40 {
                    let origin = [r.coord(), r.coord(), r.coord()];
                    // Some axis-aligned rays (the FLT_MAX inverse-direction path).
                    let raw = match k % 5 {
                        0 => [1.0, 0.0, 0.0],
                        1 => [0.0, -1.0, 0.0],
                        _ => [r.coord(), r.coord(), r.coord() + 0.01],
                    };
                    let (dir, _) = crate::normals::normalize_v3(raw);
                    let max_dist = if k % 3 == 0 {
                        2.0
                    } else {
                        BVH_RAYCAST_DIST_MAX
                    };
                    let pre = IsectRayPrecalc::new(dir);
                    let mut exact = |i: usize, o: V3, _d: V3, best: &mut RayHit| {
                        let [a, b, c] = tris[i];
                        if let Some((l, _)) = isect_ray_tri_watertight_v3(o, &pre, a, b, c)
                            && l < best.dist
                        {
                            best.index = Some(i);
                            best.dist = l;
                        }
                    };
                    let got = t.ray_cast(origin, dir, 0.0, max_dist, &mut exact);
                    let mut want = RayHit::within(max_dist);
                    for i in 0..n {
                        exact(i, origin, dir, &mut want);
                    }
                    let want = want.index.map(|_| want);
                    assert_eq!(
                        got.map(|h| h.dist),
                        want.map(|h| h.dist),
                        "tt {tt} axis {axis} n {n} ray {k}"
                    );
                    // A thick ray (`ray_nearest_hit`) sees at least what the thin one sees.
                    let thick = t.ray_cast(origin, dir, 0.05, max_dist, &mut exact);
                    if let (Some(w), Some(h)) = (want, thick) {
                        assert!(h.dist <= w.dist);
                    }
                    if want.is_some() {
                        assert!(thick.is_some());
                    }
                }
            }
        }
    }
}

#[test]
fn ray_cast_without_callback_hits_bounds() {
    let tris = triangles(5, 50);
    let t = build(&tris, 0.0, 4, 6);
    let mut hit = RayHit::within(BVH_RAYCAST_DIST_MAX);
    let o = [-10.0, 0.0, 0.0];
    let d = [1.0, 0.0, 0.0];
    let got = t.ray_cast_ex(o, d, 0.0, &mut hit, None, false);
    // Brute force: the nearest leaf box entry along the ray.
    let mut best: Option<(f32, usize)> = None;
    for (i, tri) in tris.iter().enumerate() {
        let k = kdop(tri, 0.0, 0..3);
        if k[1].0 <= 0.0 && k[1].1 >= 0.0 && k[2].0 <= 0.0 && k[2].1 >= 0.0 {
            let tt = k[0].0 - o[0];
            if best.is_none_or(|(b, _)| tt < b) {
                best = Some((tt, i));
            }
        }
    }
    assert_eq!(got, best.map(|b| b.1));
    if let Some((dist, _)) = best {
        assert_eq!(hit.dist, dist);
        assert_eq!(hit.co, [o[0] + dist, 0.0, 0.0]);
    }
}

#[test]
fn ray_cast_all_sees_every_box_on_the_ray() {
    let tris = triangles(77, 200);
    let t = build(&tris, 0.0, 2, 26);
    let o = [-10.0, 0.25, -0.5];
    let (d, _) = crate::normals::normalize_v3([1.0, 0.01, 0.02]);
    let pre = IsectRayPrecalc::new(d);
    let mut all = BTreeSet::new();
    t.ray_cast_all(
        o,
        d,
        0.0,
        BVH_RAYCAST_DIST_MAX,
        &mut |i, ray, hit| {
            assert_eq!(hit.index, None);
            assert!(ray.isect_precalc.is_some());
            let [a, b, c] = tris[i];
            if isect_ray_tri_watertight_v3(o, &pre, a, b, c).is_some() {
                all.insert(i);
            }
            // Writing the hit does not prune the next ones.
            hit.dist = 0.0;
        },
        true,
    );
    let want: BTreeSet<usize> = (0..tris.len())
        .filter(|&i| {
            let [a, b, c] = tris[i];
            isect_ray_tri_watertight_v3(o, &pre, a, b, c).is_some()
        })
        .collect();
    assert_eq!(all, want);
}

fn brute_overlap(a: &[[V3; 3]], b: &[[V3; 3]], eps: f32, axis: u8) -> BTreeSet<(usize, usize)> {
    let ka: Vec<_> = a.iter().map(|t| kdop(t, eps, axis_range(axis))).collect();
    let kb: Vec<_> = b.iter().map(|t| kdop(t, eps, axis_range(axis))).collect();
    let mut out = BTreeSet::new();
    for (i, x) in ka.iter().enumerate() {
        for (j, y) in kb.iter().enumerate() {
            if x.iter().zip(y).all(|(p, q)| !(p.0 > q.1 || q.0 > p.1)) {
                out.insert((i, j));
            }
        }
    }
    out
}

#[test]
fn overlap_matches_brute_force() {
    for &tt in &TREE_TYPES {
        for &axis in &AXES {
            for &n in &SIZES {
                let a = triangles(n as u64 + 17 * tt as u64 + axis as u64, n);
                let b = triangles(n as u64 + 5, n + 3);
                let ta = build(&a, 0.0, tt, axis);
                let tb = build(&b, 0.0, 4, axis);
                let got = ta.overlap(&tb, &mut |_, _| true);
                let set: BTreeSet<_> = got.iter().copied().collect();
                assert_eq!(set.len(), got.len(), "duplicates");
                assert_eq!(
                    set,
                    brute_overlap(&a, &b, 0.0, axis),
                    "tt {tt} axis {axis} n {n}"
                );
                // The callback filters.
                let odd = ta.overlap(&tb, &mut |i, j| (i + j) % 2 == 1);
                assert!(odd.iter().all(|(i, j)| (i + j) % 2 == 1));
                assert_eq!(
                    odd.len(),
                    set.iter().filter(|(i, j)| (i + j) % 2 == 1).count()
                );
                // With itself: every pair but the diagonal, both orders.
                let me = ta.overlap(&ta, &mut |_, _| true);
                let want: BTreeSet<_> = brute_overlap(&a, &a, 0.0, axis)
                    .into_iter()
                    .filter(|(i, j)| i != j)
                    .collect();
                assert_eq!(me.iter().copied().collect::<BTreeSet<_>>(), want);
                // Self overlap: each unordered pair once.
                let selfp = ta.overlap_self(&mut |_, _| true);
                let unordered: BTreeSet<_> =
                    selfp.iter().map(|&(i, j)| (i.min(j), i.max(j))).collect();
                assert_eq!(unordered.len(), selfp.len());
                let want_u: BTreeSet<_> = want.iter().filter(|(i, j)| i < j).copied().collect();
                assert_eq!(unordered, want_u, "self tt {tt} axis {axis} n {n}");
            }
        }
    }
}

#[test]
fn overlap_max_interactions_caps_pairs_per_leaf() {
    let a = triangles(3, 120);
    let b = triangles(4, 150);
    let ta = build(&a, 0.2, 4, 26);
    let tb = build(&b, 0.2, 4, 26);
    let all: BTreeSet<_> = ta.overlap(&tb, &mut |_, _| true).into_iter().collect();
    let capped = ta.overlap_ex(&tb, None, 1, OverlapFlags::default());
    let mut per_leaf = BTreeMap::new();
    for &(i, j) in &capped {
        assert!(all.contains(&(i, j)));
        *per_leaf.entry(i).or_insert(0) += 1;
    }
    // `tree_overlap_traverse_num` restores the budget per branch of the first tree, so each
    // first-tree leaf with an overlap reports exactly one pair.
    assert!(per_leaf.values().all(|&c| c == 1));
    let leaves: BTreeSet<_> = all.iter().map(|p| p.0).collect();
    assert_eq!(per_leaf.len(), leaves.len());
}

#[test]
fn threaded_overlap_order_concatenates_reversed_stacks() {
    // More than 1024 leaves: one stack per child of the root.
    let a = triangles(21, 1500);
    let b = triangles(22, 900);
    let ta = build(&a, 0.0, 4, 6);
    let tb = build(&b, 0.0, 4, 6);
    let threaded = ta.overlap(&tb, &mut |_, _| true);
    let mut threads = BTreeSet::new();
    let _ = ta.overlap_ex(
        &tb,
        Some(&mut |_, _, th| {
            threads.insert(th);
            true
        }),
        0,
        OverlapFlags {
            use_threading: true,
            self_overlap: false,
        },
    );
    assert_eq!(threads.len(), 4);
    let single = ta.overlap_ex(&tb, None, 0, OverlapFlags::default());
    assert_eq!(
        threaded.iter().copied().collect::<BTreeSet<_>>(),
        single.iter().copied().collect::<BTreeSet<_>>()
    );
    assert_eq!(
        threaded.iter().copied().collect::<BTreeSet<_>>(),
        brute_overlap(&a, &b, 0.0, 6)
    );
}

#[test]
fn update_node_and_tree_refit() {
    let tris = triangles(8, 40);
    let mut t = build(&tris, 0.0, 4, 6);
    let moved: Vec<[V3; 3]> = tris
        .iter()
        .map(|t| t.map(|p| [p[0] + 10.0, p[1], p[2]]))
        .collect();
    for (i, tri) in moved.iter().enumerate() {
        assert!(t.update_node(i, tri, None));
    }
    assert!(!t.update_node(1000, &moved[0], None));
    t.update_tree();
    let (lo, _) = t.bounding_box().unwrap();
    assert!(lo[0] > 4.0);
    let mut got = BTreeSet::new();
    t.range_query([10.0, 0.0, 0.0], 3.0, &mut |i, _, _| {
        got.insert(i);
    });
    let mut want = BTreeSet::new();
    for (i, tri) in moved.iter().enumerate() {
        let k = kdop(tri, 0.0, 0..3);
        let co = [10.0f32, 0.0, 0.0];
        let p: Vec<f32> = (0..3).map(|a| co[a].max(k[a].0).min(k[a].1)).collect();
        if dist_sq([p[0], p[1], p[2]], co) < 9.0 {
            want.insert(i);
        }
    }
    assert_eq!(got, want);
}

// -------------------------------------------------------------------------------------------
// Blender orders (`mathutils.bvhtree.BVHTree.FromPolygons`: tree_type 4, 6-DOP).
// -------------------------------------------------------------------------------------------

/// Parses `a b` lines (comment lines start with `#`).
fn pairs(text: &str) -> Vec<(usize, usize)> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace().map(|x| x.parse::<usize>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        })
        .collect()
}

/// The triangles of `gen_bvh_fixtures.py` (same LCG, same order).
fn fixture_triangles(seed: u64, n: usize) -> Vec<[V3; 3]> {
    triangles(seed, n)
}

fn check_blender_overlap(fixture: &str, seed_a: u64, n_a: usize, seed_b: u64, n_b: usize) {
    let want = pairs(fixture);
    let a = fixture_triangles(seed_a, n_a);
    let b = fixture_triangles(seed_b, n_b);
    let ta = build(&a, 0.0, 4, 6);
    let tb = build(&b, 0.0, 4, 6);
    // Blender's callback tested a real triangle intersection: accept exactly its pairs.
    let accepted: BTreeSet<_> = want.iter().copied().collect();
    let got = ta.overlap(&tb, &mut |i, j| accepted.contains(&(i, j)));
    assert_eq!(got, want);
}

#[test]
fn overlap_order_matches_blender_single_stack() {
    check_blender_overlap(include_str!("fixtures/overlap_small.txt"), 1, 400, 2, 300);
}

#[test]
fn overlap_order_matches_blender_threaded() {
    check_blender_overlap(include_str!("fixtures/overlap_large.txt"), 3, 1600, 4, 1200);
}

#[test]
fn range_order_matches_blender() {
    // `find_nearest_range` appends in callback order; its exact test is a point–triangle
    // distance, so accept exactly the triangles Blender listed, per query.
    let text = include_str!("fixtures/range.txt");
    let tris = fixture_triangles(5, 700);
    let t = build(&tris, 0.0, 4, 6);
    let mut by_query: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (q, i) in pairs(text) {
        by_query.entry(q).or_default().push(i);
    }
    let queries = fixture_triangles(6, 30);
    for (q, query) in queries.iter().enumerate() {
        let want = by_query.remove(&q).unwrap_or_default();
        let accepted: BTreeSet<_> = want.iter().copied().collect();
        let mut got = Vec::new();
        t.range_query(query[0], 1.5, &mut |i, _, _| {
            if accepted.contains(&i) {
                got.push(i);
            }
        });
        assert_eq!(got, want, "query {q}");
    }
}
