// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.lightmap_pack(PREF_CONTEXT='SEL_FACES', PREF_PACK_IN_ONE=True,
//! PREF_NEW_UVLAYER=False, PREF_BOX_DIV=12, PREF_MARGIN_DIV=0.1)`, tier T1
//! (`tests/golden/cases/uv/lightmap/*.py`). Port of the Python operator
//! `scripts/startup/bl_operators/uvcalc_lightmap.py` (`LightMapPack.execute`, `unwrap`,
//! `lightmap_uvpack`, `prettyface`), Blender 5.2.4, GPL-2.0-or-later, with its number types:
//! Python floats (`f64`) for lengths, areas and layout, `mathutils` (`f32` storage, `f64` dot
//! products) for vectors, and the `mathutils.kdtree` / `geometry.box_pack_2d` /
//! `geometry.box_fit_2d` calls it makes (`kdtree.rs`, `frap_uv::pack`).
//!
//! Poll: the active object is a mesh. Every face (in Edit Mode with `SEL_FACES`: every
//! selected face) gets its own rectangle of the active UV map: quads keep their corners,
//! triangles are paired by similar shape (a kd-tree over their sorted edge lengths) into
//! rectangles, n-gons are fitted into their bounding box. Rectangle sizes are quantized to
//! powers of two of the face sizes, grouped into larger squares (`PREF_BOX_DIV`), packed
//! (`box_pack_2d`) and shrunk by the margin (`PREF_MARGIN_DIV` percent of the layout). The
//! meshes come from the objects in Edit Mode (the operator leaves Edit Mode and re-enters it,
//! like the Python one) or else the selected objects; `PREF_PACK_IN_ONE` lays them out
//! together. A mesh without a UV map gets one; `PREF_NEW_UVLAYER` adds a copy of the active
//! UV map first, which does not become active (so the active map is still the one written, as
//! in Blender). No mesh with faces: `No mesh object`.
//!
//! Blender's own quirks are kept: the triangle pairing stops at the first triangle left
//! without a partner (the remaining triangles keep their UVs), and a single triangle's
//! rectangle uses its first two edge lengths. Several meshes are laid out in the order of the
//! objects (Blender iterates a Python `set`, whose order depends on memory addresses), so with
//! `PREF_PACK_IN_ONE` and two or more meshes the layout differs from Blender's (reproducible
//! there for one build and scene, but not derivable from the data):
//! `tests/golden/cases/uv/lightmap/pack_in_one.py` checks it at T3.

mod kdtree;
mod qsort;

use std::sync::Arc;

use frap_mesh::{AttrData, Domain};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, Input, MeshId, ObjectId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::lightmap_pack::PrefContext;
use frap_uv::param::bli;
use smol_str::SmolStr;

use super::unwrap::uvedit;

type V3 = [f32; 3];
type Uv = [f32; 2];

/// `LightMapPack.poll`: `ob and ob.type == 'MESH'` (the active object).
fn lightmap_poll(c: &Context) -> Result<(), PollFail> {
    match poll::active(c) {
        Some((_, ob)) if ob.object_type() == "MESH" => Ok(()),
        _ => Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh the active object, e.g. bpy.ops.mesh.primitive_cube_add()",
        )),
    }
}

/// The poll of `uv.lightmap_pack`.
const LIGHTMAP_POLL: Poll = Poll {
    requires: &[PollReq::ActiveType("MESH")],
    check: lightmap_poll,
};

/// One mesh's data while packing (Object Mode).
struct MeshData {
    me: IdRef<MeshId>,
    positions: Vec<V3>,
    offsets: Vec<usize>,
    corner_verts: Vec<usize>,
    normals: Arc<Vec<[f32; 3]>>,
    uv_name: SmolStr,
    uvs: Vec<Uv>,
}

impl MeshData {
    fn corners(&self, f: usize) -> core::ops::Range<usize> {
        self.offsets[f]..self.offsets[f + 1]
    }
    fn cos(&self, f: usize) -> Vec<V3> {
        self.corners(f)
            .map(|c| self.positions[self.corner_verts[c]])
            .collect()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct FaceRef {
    mesh: usize,
    face: usize,
}

/// `trylens(f)`: `(f, lens, lens_order)`.
#[derive(Clone, Debug)]
struct Tri {
    f: FaceRef,
    lens: [f64; 3],
    ord: [usize; 3],
}

/// What a `prettyface` holds in `uv`.
#[derive(Clone, Debug)]
enum Kind {
    /// A quad or n-gon: its corners (`self.uv`, rotated by `spin` for quads).
    Face(FaceRef, Vec<usize>),
    /// Two triangles (`(tri1, tri2)`) or one (`(tri1, None)`).
    Tris(usize, Option<usize>),
    /// A group of boxes (`uv = None`).
    Group(Vec<usize>),
}

#[derive(Clone, Debug)]
struct Pf {
    kind: Kind,
    width: f64,
    height: f64,
    xoff: f64,
    yoff: f64,
    has_parent: bool,
    rot: bool,
}

/// `Vector.length` of `a - b` (`f32` difference, `sqrt` of the `f64` dot product summed from
/// the last component).
fn len_sub(a: V3, b: V3) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let mut s = 0.0f64;
    for k in (0..3).rev() {
        s += f64::from(d[k] * d[k]);
    }
    s.sqrt()
}

/// `Vector.angle(other, fallback)`: `f64` dot products, `safe_acosf`.
fn vec_angle(a: V3, b: V3, fallback: f64) -> f64 {
    let (mut dot, mut ds, mut dot_o) = (0.0f64, 0.0f64, 0.0f64);
    for k in 0..3 {
        ds += f64::from(a[k]) * f64::from(a[k]);
        dot_o += f64::from(b[k]) * f64::from(b[k]);
        dot += f64::from(a[k]) * f64::from(b[k]);
    }
    if ds == 0.0 || dot_o == 0.0 {
        return fallback;
    }
    // `safe_acosf(float(dot / (…)))`. frap's `acosf` is correctly rounded; the reference
    // build's (Apple libm) is not always (`acosf(0.76604444f)` is 0.69813174, one ulp above),
    // which can reorder the two base angles of an isosceles triangle (a deviation).
    let x = (dot / (ds.sqrt() * dot_o.sqrt())) as f32;
    let r = libm::acosf(x.clamp(-1.0, 1.0));
    f64::from(r)
}

/// `rotation_between_vecs_to_quat(q, normalize(no), (0, 0, 1))`
/// (`Vector.rotation_difference`).
fn rotation_to_z(no: V3) -> [f32; 4] {
    let (a, _) = bli::normalize3(no);
    let b = [0.0f32, 0.0, 1.0];
    let (axis, len) = bli::normalize3(bli::cross3(a, b));
    if len > f32::EPSILON {
        let angle = angle_normalized_v3v3(a, b);
        let phi = 0.5 * angle;
        let (si, co) = (libm::sinf(phi), libm::cosf(phi));
        [co, axis[0] * si, axis[1] * si, axis[2] * si]
    } else if bli::dot3(a, b) > 0.0 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        // `ortho_v3_v3(axis, a)`, then a half turn about it.
        let ortho = ortho_v3_v3(a);
        let (n, l) = bli::normalize3(ortho);
        if l == 0.0 {
            return [1.0, 0.0, 0.0, 0.0];
        }
        let phi = 0.5 * core::f32::consts::PI;
        let (si, co) = (libm::sinf(phi), libm::cosf(phi));
        [co, n[0] * si, n[1] * si, n[2] * si]
    }
}

/// `angle_normalized_v3v3`.
fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if bli::dot3(v1, v2) >= 0.0 {
        return 2.0 * bli::safe_asinf(bli::len_v3v3(v1, v2) / 2.0);
    }
    let v2n = [-v2[0], -v2[1], -v2[2]];
    core::f32::consts::PI - 2.0 * bli::safe_asinf(bli::len_v3v3(v1, v2n) / 2.0)
}

/// `ortho_v3_v3`.
fn ortho_v3_v3(v: V3) -> V3 {
    let axis = {
        let (x, y, z) = (v[0].abs(), v[1].abs(), v[2].abs());
        if x >= y && x >= z {
            0
        } else if y >= z {
            1
        } else {
            2
        }
    };
    match axis {
        0 => [-v[1] - v[2], v[0], v[0]],
        1 => [v[1], -v[0] - v[2], v[1]],
        _ => [v[2], v[2], -v[0] - v[1]],
    }
}

/// `mul_qt_v3`.
fn mul_qt_v3(q: [f32; 4], v: V3) -> V3 {
    let mut r = v;
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    r[2] = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    r[0] = t1;
    r[1] = t2;
    let t1 = t0 * -q[1] + r[0] * q[0] - r[1] * q[3] + r[2] * q[2];
    let t2 = t0 * -q[2] + r[1] * q[0] - r[2] * q[1] + r[0] * q[3];
    r[2] = t0 * -q[3] + r[2] * q[0] - r[0] * q[2] + r[1] * q[1];
    r[0] = t1;
    r[1] = t2;
    r
}

/// `angle_wrap_rad` (`mod_inline` on floats).
fn angle_wrap_rad(angle: f32) -> f32 {
    let pi = core::f32::consts::PI;
    let a = angle + pi;
    let b = pi * 2.0;
    (a - (b * libm::floorf(a / b))) - pi
}

/// `prettyface(face)` for a quad or an n-gon.
fn pf_face(meshes: &mut [MeshData], f: FaceRef) -> Pf {
    let md = &mut meshes[f.mesh];
    let corners: Vec<usize> = md.corners(f.face).collect();
    let cos = md.cos(f.face);
    let (width, height) = if corners.len() == 4 {
        (
            (len_sub(cos[0], cos[1]) + len_sub(cos[2], cos[3])) / 2.0,
            (len_sub(cos[1], cos[2]) + len_sub(cos[0], cos[3])) / 2.0,
        )
    } else {
        // An n-gon: projected along its normal and fitted, UVs normalized now.
        let no = md.normals[f.face];
        let r = rotation_to_z(no);
        let cos_2d: Vec<[f32; 2]> = cos
            .iter()
            .map(|&co| {
                let p = mul_qt_v3(r, co);
                [p[0], p[1]]
            })
            .collect();
        let angle = f64::from(frap_uv::pack::convexhull::convexhull_aabb_fit_points_2d(
            &cos_2d,
        ));
        // `Matrix.Rotation(angle, 2) @ co`.
        let a = angle_wrap_rad(angle as f32);
        let (c, s) = (libm::cosf(a), libm::sinf(a));
        let cos_2d: Vec<[f32; 2]> = cos_2d
            .iter()
            .map(|p| {
                [
                    (f64::from(c * p[0]) + f64::from(-s * p[1])) as f32,
                    (f64::from(s * p[0]) + f64::from(c * p[1])) as f32,
                ]
            })
            .collect();
        let xs: Vec<f64> = cos_2d.iter().map(|p| f64::from(p[0])).collect();
        let ys: Vec<f64> = cos_2d.iter().map(|p| f64::from(p[1])).collect();
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let (xmin, ymin, xmax, ymax) = (min(&xs), min(&ys), max(&xs), max(&ys));
        let xspan = xmax - xmin;
        let yspan = ymax - ymin;
        for (i, &c) in corners.iter().enumerate() {
            md.uvs[c] = if xspan < 0.0000001 || yspan < 0.0000001 {
                [0.0, 0.0]
            } else {
                [
                    ((xs[i] - xmin) / xspan) as f32,
                    ((ys[i] - ymin) / yspan) as f32,
                ]
            };
        }
        (xspan, yspan)
    };
    Pf {
        kind: Kind::Face(f, corners),
        width,
        height,
        xoff: 0.0,
        yoff: 0.0,
        has_parent: false,
        rot: false,
    }
}

/// `prettyface(list)`: two boxes side by side or four in a square.
fn pf_group(pfs: &mut [Pf], data: Vec<usize>) -> Pf {
    let (width, height) = if data.len() == 2 {
        pfs[data[1]].xoff = pfs[data[0]].width;
        (pfs[data[0]].width * 2.0, pfs[data[0]].height)
    } else {
        let d = pfs[data[0]].width;
        pfs[data[1]].xoff += d;
        pfs[data[2]].yoff += d;
        pfs[data[3]].xoff += d;
        pfs[data[3]].yoff += d;
        (d * 2.0, d * 2.0)
    };
    for &c in &data {
        pfs[c].has_parent = true;
    }
    Pf {
        kind: Kind::Group(data),
        width,
        height,
        xoff: 0.0,
        yoff: 0.0,
        has_parent: false,
        rot: false,
    }
}

/// `prettyface.spin`.
fn spin(pfs: &mut [Pf], i: usize) {
    let mut stack = vec![i];
    while let Some(i) = stack.pop() {
        let p = &mut pfs[i];
        if let Kind::Face(_, uv) = &mut p.kind
            && uv.len() == 4
        {
            uv.rotate_left(1);
        }
        core::mem::swap(&mut p.width, &mut p.height);
        core::mem::swap(&mut p.xoff, &mut p.yoff);
        p.rot = !p.rot;
        if let Kind::Group(children) = &p.kind {
            stack.extend(children.iter().copied());
        }
    }
}

/// The scaled box of `prettyface.place`.
#[derive(Copy, Clone, Debug)]
struct Placed {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    margin_w: f64,
    margin_h: f64,
}

/// `set_uv` of a triangle of a pair: corners matched by angle size.
fn set_tri_uv(md: &mut MeshData, face: usize, rot: bool, p: [(f64, f64); 3]) {
    let cos = md.cos(face);
    let (v1, v2, v3) = (cos[0], cos[1], cos[2]);
    let pi = core::f64::consts::PI;
    let sub = |a: V3, b: V3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let a1 = vec_angle(sub(v2, v1), sub(v3, v1), pi);
    let a2 = vec_angle(sub(v1, v2), sub(v3, v2), pi);
    let a3 = pi - (a1 + a2);
    let mut angles = [(a1, 0usize), (a2, 1), (a3, 2)];
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    let ii = [angles[0].1, angles[1].1, angles[2].1];
    let corners: Vec<usize> = md.corners(face).collect();
    let mut set = |k: usize, v: (f64, f64)| md.uvs[corners[k]] = [v.0 as f32, v.1 as f32];
    if rot {
        set(ii[2], p[0]);
        set(ii[1], p[1]);
        set(ii[0], p[2]);
    } else {
        set(ii[2], p[0]);
        set(ii[0], p[1]);
        set(ii[1], p[2]);
    }
}

/// `prettyface.place`.
#[allow(clippy::too_many_arguments)]
fn place(
    pfs: &[Pf],
    tris: &[Tri],
    meshes: &mut [MeshData],
    i: usize,
    xoff: f64,
    yoff: f64,
    xfac: f64,
    yfac: f64,
    margin_w: f64,
    margin_h: f64,
) {
    let p = &pfs[i];
    let xoff = xoff + p.xoff;
    let yoff = yoff + p.yoff;
    if let Kind::Group(children) = &p.kind {
        for &c in children {
            place(
                pfs, tris, meshes, c, xoff, yoff, xfac, yfac, margin_w, margin_h,
            );
        }
        return;
    }
    let b = Placed {
        x1: xoff / xfac + margin_w,
        x2: (xoff + p.width) / xfac - margin_w,
        y1: yoff / yfac + margin_h,
        y2: (yoff + p.height) / yfac - margin_h,
        margin_w,
        margin_h,
    };
    match &p.kind {
        Kind::Tris(t1, t2) => {
            let t = &tris[*t1];
            set_tri_uv(
                &mut meshes[t.f.mesh],
                t.f.face,
                p.rot,
                [
                    (b.x1, b.y1),
                    (b.x1, b.y2 - b.margin_h),
                    (b.x2 - b.margin_w, b.y1),
                ],
            );
            if let Some(t2) = t2 {
                let t = &tris[*t2];
                set_tri_uv(
                    &mut meshes[t.f.mesh],
                    t.f.face,
                    p.rot,
                    [
                        (b.x2, b.y2),
                        (b.x2, b.y1 + b.margin_h),
                        (b.x1 + b.margin_w, b.y2),
                    ],
                );
            }
        }
        Kind::Face(f, uv) => {
            let md = &mut meshes[f.mesh];
            let s = |x: f64, y: f64| [x as f32, y as f32];
            if uv.len() == 4 {
                md.uvs[uv[1]] = s(b.x1, b.y1);
                md.uvs[uv[2]] = s(b.x1, b.y2);
                md.uvs[uv[3]] = s(b.x2, b.y2);
                md.uvs[uv[0]] = s(b.x2, b.y1);
            } else {
                let xspan = b.x2 - b.x1;
                let yspan = b.y2 - b.y1;
                for &c in uv {
                    let [x, y] = md.uvs[c];
                    md.uvs[c] = s(b.x1 + f64::from(x) * xspan, b.y1 + f64::from(y) * yspan);
                }
            }
        }
        Kind::Group(_) => {}
    }
}

/// `trylens(f)`.
fn trylens(md: &MeshData, f: FaceRef) -> Tri {
    let cos = md.cos(f.face);
    let lens = [
        len_sub(cos[0], cos[1]),
        len_sub(cos[1], cos[2]),
        len_sub(cos[2], cos[0]),
    ];
    // `lens.index(min(lens))`: the first occurrence.
    let index_of = |v: f64| lens.iter().position(|&x| x == v).unwrap_or(0);
    let min = lens.iter().copied().fold(f64::INFINITY, f64::min);
    let max = lens.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lmin = index_of(min);
    let lmax = index_of(max);
    let lmid = (0..3).find(|&i| i != lmin && i != lmax).unwrap_or(0);
    Tri {
        f,
        lens,
        ord: [lmin, lmid, lmax],
    }
}

/// `lightmap_uvpack` for one face group.
fn pack_group(meshes: &mut [MeshData], face_sel: &[FaceRef], box_div: i32, margin_div: i64) {
    let mut pfs: Vec<Pf> = Vec::new();
    for &f in face_sel {
        if meshes[f.mesh].corners(f.face).len() >= 4 {
            pfs.push(pf_face(meshes, f));
        }
    }
    let mut tris: Vec<Tri> = Vec::new();
    if pfs.len() != face_sel.len() {
        tris = face_sel
            .iter()
            .filter(|f| meshes[f.mesh].corners(f.face).len() == 3)
            .map(|&f| trylens(&meshes[f.mesh], f))
            .collect();
        let sorted = |t: &Tri| [t.lens[t.ord[0]], t.lens[t.ord[1]], t.lens[t.ord[2]]];
        let points: Vec<V3> = tris.iter().map(|t| sorted(t).map(|x| x as f32)).collect();
        let kd = kdtree::KdTree::new(&points);
        let mut added = vec![false; tris.len()];
        let threshold = 0.00001f64;
        let pf_tris = |t1: usize, t2: Option<usize>, tris: &[Tri]| -> Pf {
            let a = &tris[t1];
            let (width, height) = match t2 {
                Some(t2) => {
                    let b = &tris[t2];
                    (
                        (a.lens[a.ord[0]] + b.lens[b.ord[0]]) / 2.0,
                        (a.lens[a.ord[1]] + b.lens[b.ord[1]]) / 2.0,
                    )
                }
                None => (a.lens[0], a.lens[1]),
            };
            Pf {
                kind: Kind::Tris(t1, t2),
                width,
                height,
                xoff: 0.0,
                yoff: 0.0,
                has_parent: false,
                rot: false,
            }
        };
        for i in 0..tris.len() {
            frap_math::budget::check();
            if added[i] {
                continue;
            }
            added[i] = true;
            let sorted_l = sorted(&tris[i]).map(|x| x as f32);
            let found = kd.find_filtered(sorted_l, |idx| !added[idx as usize]);
            // `if not nearest or nearest < 0`: index 0 counts as "not found" too.
            let Some((nearest, dist)) = found.filter(|&(n, _)| n > 0) else {
                pfs.push(pf_tris(i, None, &tris));
                break;
            };
            let nearest = nearest as usize;
            pfs.push(pf_tris(i, Some(nearest), &tris));
            added[nearest] = true;
            if f64::from(dist) < threshold {
                let cluster: Vec<usize> = kd
                    .range(sorted_l, threshold as f32)
                    .into_iter()
                    .map(|i| i as usize)
                    .filter(|&i| !added[i])
                    .collect();
                if cluster.len() > 1 {
                    let mut ci = 0;
                    while ci < cluster.len() - (cluster.len() % 2) {
                        pfs.push(pf_tris(cluster[ci], Some(cluster[ci + 1]), &tris));
                        added[cluster[ci]] = true;
                        added[cluster[ci + 1]] = true;
                        ci += 2;
                    }
                }
            }
        }
    }

    // Min, max and total areas (`MeshPolygon.area`).
    let mut max_area = 0.0f64;
    let mut min_area = 100000000.0f64;
    let mut tot_area = 0.0f64;
    for &f in face_sel {
        let a = f64::from(face_area(&meshes[f.mesh].cos(f.face)));
        if a > max_area {
            max_area = a;
        }
        if a < min_area {
            min_area = a;
        }
        tot_area += a;
    }
    let max_len = max_area.sqrt();
    let min_len = min_area.sqrt();
    let side_len = tot_area.sqrt();

    // Widths, as powers of two.
    let mut curr_len = max_len;
    let mut lengths = Vec::new();
    while curr_len > min_len {
        lengths.push(curr_len);
        curr_len /= 2.0;
        if curr_len / 4.0 < side_len / margin_div as f64 {
            break;
        }
    }
    if lengths.is_empty() {
        lengths.push(curr_len);
    }
    let mut lengths_to_ints: Vec<(f64, i64)> = Vec::new();
    let mut l_int = 1i64;
    for &l in lengths.iter().rev() {
        match lengths_to_ints.iter_mut().find(|(k, _)| *k == l) {
            Some(e) => e.1 = l_int,
            None => lengths_to_ints.push((l, l_int)),
        }
        l_int *= 2;
    }
    lengths_to_ints.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });

    // Quantize.
    for i in 0..pfs.len() {
        let (w, h) = (pfs[i].width, pfs[i].height);
        let (mut bestw, mut besth) = (1000000000.0f64, 1000000000.0f64);
        let (mut new_w, mut new_h) = (0.0f64, 0.0f64);
        for &(l, li) in &lengths_to_ints {
            let d = (l - w).abs();
            if d < bestw {
                bestw = d;
                new_w = li as f64;
            }
            let d = (l - h).abs();
            if d < besth {
                besth = d;
                new_h = li as f64;
            }
        }
        pfs[i].width = new_w;
        pfs[i].height = new_h;
        if new_w > new_h {
            spin(&mut pfs, i);
        }
    }

    // Consolidate boxes (dicts keep insertion order).
    let mut even: Vec<(i64, Vec<usize>)> = Vec::new();
    let mut odd: Vec<((i64, i64), Vec<usize>)> = Vec::new();
    fn entry<K: PartialEq + Copy>(m: &mut Vec<(K, Vec<usize>)>, k: K) -> &mut Vec<usize> {
        let i = match m.iter().position(|(x, _)| *x == k) {
            Some(i) => i,
            None => {
                m.push((k, Vec::new()));
                m.len() - 1
            }
        };
        &mut m[i].1
    }
    for (i, p) in pfs.iter().enumerate() {
        let (w, h) = (p.width as i64, p.height as i64);
        if w == h {
            entry(&mut even, w).push(i);
        } else {
            entry(&mut odd, (w, h)).push(i);
        }
    }
    let float_to_int_factor = lengths_to_ints[0].0;
    let (max_int_dimension, mut ok) = if float_to_int_factor > 0.0 {
        (
            ((side_len / float_to_int_factor) / f64::from(box_div)) as i64,
            true,
        )
    } else {
        (0, false)
    };
    while ok {
        frap_math::budget::check();
        ok = false;
        // Tall boxes in groups of 2.
        let keys: Vec<(i64, i64)> = odd.iter().map(|(k, _)| *k).collect();
        for d in keys {
            if d.1 >= max_int_dimension {
                continue;
            }
            loop {
                let boxes = entry(&mut odd, d);
                if boxes.len() < 2 {
                    break;
                }
                ok = true;
                let a = boxes.pop().expect("len checked");
                let b = boxes.pop().expect("len checked");
                let parent = pf_group(&mut pfs, vec![a, b]);
                let (w, h) = (parent.width as i64, parent.height as i64);
                pfs.push(parent);
                let pi = pfs.len() - 1;
                if w == h {
                    entry(&mut even, w).push(pi);
                } else {
                    entry(&mut odd, (w, h)).push(pi);
                }
            }
        }
        // Even boxes in groups of 4.
        let keys: Vec<i64> = even.iter().map(|(k, _)| *k).collect();
        for d in keys {
            if d >= max_int_dimension {
                continue;
            }
            {
                let boxes = entry(&mut even, d);
                let mut sorted: Vec<(usize, usize)> = boxes
                    .iter()
                    .map(|&b| {
                        let n = match &pfs[b].kind {
                            Kind::Group(c) => c.len(),
                            _ => 0,
                        };
                        (n, b)
                    })
                    .collect();
                // `list.sort(key=…)` is stable.
                sorted.sort_by_key(|&(n, _)| n);
                *boxes = sorted.into_iter().map(|(_, b)| b).collect();
            }
            loop {
                let boxes = entry(&mut even, d);
                if boxes.len() < 4 {
                    break;
                }
                ok = true;
                let data: Vec<usize> = (0..4).map(|_| boxes.pop().expect("len")).collect();
                let parent = pf_group(&mut pfs, data);
                let w = parent.width as i64;
                pfs.push(parent);
                let pi = pfs.len() - 1;
                entry(&mut even, w).push(pi);
            }
        }
    }
    let top: Vec<usize> = (0..pfs.len()).filter(|&i| !pfs[i].has_parent).collect();

    // Spin every second rectangle (from the end).
    let mut d = 0;
    for &i in top.iter().rev() {
        if pfs[i].width != pfs[i].height {
            d += 1;
            if d % 2 == 1 {
                spin(&mut pfs, i);
            }
        }
    }

    // `mathutils.geometry.box_pack_2d` (boxes sorted by area first, `qsort`).
    let mut boxes: Vec<(usize, frap_uv::pack::BoxPack)> = top
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            (
                k,
                frap_uv::pack::BoxPack::new(pfs[i].width as f32, pfs[i].height as f32),
            )
        })
        .collect();
    qsort::qsort(&mut boxes, &mut |a, b| {
        let (a1, a2) = (a.1.w * a.1.h, b.1.w * b.1.h);
        if a1 < a2 {
            core::cmp::Ordering::Greater
        } else if a1 > a2 {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Equal
        }
    });
    let mut packed: Vec<frap_uv::pack::BoxPack> = boxes.iter().map(|b| b.1).collect();
    let (pw, ph) = frap_uv::pack::box_pack_2d(&mut packed);
    let mut xy = vec![(0.0f64, 0.0f64); top.len()];
    for (b, p) in boxes.iter().zip(&packed) {
        xy[b.0] = (f64::from(p.x), f64::from(p.y));
    }
    let (pw, ph) = (f64::from(pw), f64::from(ph));
    let margin_w = (pw / margin_div as f64) / pw;
    let margin_h = (ph / margin_div as f64) / ph;
    for (k, &i) in top.iter().enumerate() {
        place(
            &pfs, &tris, meshes, i, xy[k].0, xy[k].1, pw, ph, margin_w, margin_h,
        );
    }
}

/// `MeshPolygon.area` (`face_area_calc`): `area_tri_v3`, else Newell from the last corner.
fn face_area(cos: &[V3]) -> f32 {
    if cos.len() == 3 {
        return bli::area_tri_v3(cos[0], cos[1], cos[2]);
    }
    let mut n = [0.0f32; 3];
    let mut prev = cos[cos.len() - 1];
    for &c in cos {
        bli::add_newell_cross(&mut n, prev, c);
        prev = c;
    }
    bli::len3(n) * 0.5
}

/// `bpy.ops.object.mode_set(mode=…)` as the Python operator calls it.
fn mode_set(cx: &mut OpCtx, mode: &str) -> Result<(), OpError> {
    let def = frap_op::lookup("object.mode_set").map_err(|e| OpError::Runtime(e.message))?;
    let (args, _, _) = frap_op::convert_args(
        def.spec,
        Vec::new(),
        vec![("mode".into(), Input::Str(mode.into()))],
    )
    .map_err(|e| OpError::Runtime(e.message))?;
    cx.run_op("object.mode_set", args).map(|_| ())
}

#[operator(poll = LIGHTMAP_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::LightmapPack) -> OpResult {
    let is_editmode = poll::active(&cx.ctx()).is_some_and(|(_, o)| o.mode == ObjectMode::Edit);
    let sel_only = is_editmode && p.PREF_CONTEXT.value == PrefContext::SelFaces;
    // `int(1.0 / (PREF_MARGIN_DIV / 100.0))`.
    let margin_div = (1.0 / (f64::from(p.PREF_MARGIN_DIV.value) / 100.0)) as i64;

    let objects: Vec<IdRef<ObjectId>> = if is_editmode {
        let obs: Vec<_> = uvedit::edit_objects(cx)
            .into_iter()
            .map(|(o, _)| o)
            .collect();
        helpers::mode_set_object(cx)?;
        obs
    } else {
        let c = cx.ctx();
        c.view_layer()
            .bases
            .iter()
            .filter(|b| b.selected())
            .map(|b| b.object)
            .collect()
    };
    let mut mesh_ids: Vec<IdRef<MeshId>> = Vec::new();
    for ob in objects {
        let main = cx.main();
        let Some(o) = main.objects.get(ob) else {
            continue;
        };
        let Some(me) = o.data.and_then(|d| d.typed::<MeshId>()) else {
            continue;
        };
        let Some(m) = main.meshes.get(me) else {
            continue;
        };
        if m.faces_num() == 0 || m.header.library.is_some() || mesh_ids.contains(&me) {
            continue;
        }
        mesh_ids.push(me);
    }
    if mesh_ids.is_empty() {
        return Err(OpError::Runtime("No mesh object".into()));
    }

    // Faces, new UV maps.
    let mut groups: Vec<Vec<FaceRef>> = if p.PREF_PACK_IN_ONE.value {
        vec![Vec::new()]
    } else {
        Vec::new()
    };
    let mut meshes = Vec::new();
    for (mi, &me) in mesh_ids.iter().enumerate() {
        let m = cx.mesh_mut(me)?;
        let sel = m
            .bool_attribute(".select_poly", Domain::Face)
            .map(<[bool]>::to_vec);
        let faces: Vec<FaceRef> = (0..m.faces_num())
            .filter(|&f| !sel_only || sel.as_ref().is_some_and(|s| s[f]))
            .map(|face| FaceRef { mesh: mi, face })
            .collect();
        if p.PREF_PACK_IN_ONE.value {
            groups[0].extend(faces);
        } else {
            groups.push(faces);
        }
        if p.PREF_NEW_UVLAYER.value {
            m.add_uv_map("", true);
        }
        if m.uv_map_count() == 0 {
            m.add_uv_map("", true);
        }
        let uv_name = m
            .uv_active
            .clone()
            .filter(|n| m.attributes().get(n).is_some())
            .or_else(|| m.uv_maps().first().map(|(n, _)| SmolStr::from(*n)))
            .unwrap_or_default();
        let uvs = match m.attributes().get(&uv_name).map(|a| &a.data) {
            Some(AttrData::Float2(v)) => v.as_ref().clone(),
            _ => vec![[0.0; 2]; m.corners_num()],
        };
        meshes.push(MeshData {
            me,
            positions: m.positions().to_vec(),
            offsets: m.face_offsets().iter().map(|&o| o as usize).collect(),
            corner_verts: m.corner_verts().iter().map(|&v| v as usize).collect(),
            normals: m.face_normals(),
            uv_name,
            uvs,
        });
    }
    for g in &groups {
        if g.is_empty() {
            continue;
        }
        pack_group(&mut meshes, g, p.PREF_BOX_DIV.value, margin_div);
    }
    for md in meshes {
        let m = cx.mesh_mut(md.me)?;
        if let Some(a) = m.attributes_mut().get_mut(&md.uv_name) {
            a.data = AttrData::Float2(Arc::new(md.uvs));
        }
    }
    if is_editmode {
        mode_set(cx, "EDIT")?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_angles_are_python_numbers() {
        // Face 0 of `primitive_cylinder_add(vertices=9, end_fill_type='TRIFAN')`
        // (`get_tri_angles`). Blender 5.2.4 on macOS gives a1 = 0.6981317400932312: Apple's
        // `acosf(0.76604444f)` is one ulp above the correctly rounded 0.6981316804885864
        // (a deviation); a2 is equal on both.
        let v1 = [0.0f32, 0.0, -1.0];
        let v2 = [0.0f32, 1.0, -1.0];
        let v3 = [0.642_787_64_f32, 0.766_044_44, -1.0];
        let sub = |a: V3, b: V3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let pi = core::f64::consts::PI;
        let a1 = vec_angle(sub(v2, v1), sub(v3, v1), pi);
        let a2 = vec_angle(sub(v1, v2), sub(v3, v2), pi);
        assert_eq!(a1, 0.698_131_680_488_586_4);
        assert_eq!(a2, 1.221_730_470_657_348_6);
    }

    #[test]
    fn python_number_helpers() {
        assert_eq!(len_sub([3.0, 0.0, 0.0], [0.0, 4.0, 0.0]), 5.0);
        let a = vec_angle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0);
        assert!((a - core::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(vec_angle([0.0; 3], [1.0, 0.0, 0.0], 7.0), 7.0);
        // `int(1.0 / (0.1f / 100.0))` is 999 (the f32 0.1 is slightly above 0.1).
        assert_eq!((1.0 / (f64::from(0.1f32) / 100.0)) as i64, 999);
        let q = rotation_to_z([0.0, 0.0, 1.0]);
        assert_eq!(q, [1.0, 0.0, 0.0, 0.0]);
        let r = mul_qt_v3(rotation_to_z([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
        assert!((r[2] - 1.0).abs() < 1e-6);
        assert!(
            (face_area(&[[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]]) - 4.0).abs()
                < 1e-6
        );
    }
}
