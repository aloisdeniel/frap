// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the deform and UV modifiers of WP 8.3.12 (WARP, WAVE, UV_WARP,
//! UV_PROJECT, SHRINKWRAP, CORRECTIVE_SMOOTH): `MOD_get_vgroup`, `MOD_get_texture_coords` for
//! any `MappingInfoModifierData`, the object/bone matrices (`matrix_from_obj_pchan`), the
//! texture of a modifier (`MOD_init_texture` + `BKE_texture_get_value` through DISPLACE's port)
//! and the `BLI_math` pieces the C code calls (`blend_m4_m4m4`, `interp_v3_v3v3`, …).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_util.cc (`MOD_get_vgroup`,
//! `MOD_get_texture_coords`), blender/source/blender/modifiers/intern/MOD_warp.cc and
//! MOD_uvwarp.cc (`matrix_from_obj_pchan`), blender/source/blender/blenlib/intern/
//! math_matrix_c.cc (`blend_m4_m4m4`) and math_rotation_c.cc (`interp_qt_qtqt`,
//! `interp_dot_slerp`), Blender 5.2.4, GPL-2.0-or-later.

use frap_mesh::{AttrData, DeformVert, Domain, Mesh};
use frap_rna::{IdAny, IdRef, ImageId, ObjectId, TextureId};
use frap_scene::generated::plain::TextureFields;
use glam::Vec3;

use super::super::displace::image_buffer;
use super::super::displace::tex::{ImBuf, TexResult, texture_value};
use super::super::weld::util::{
    M4, defgroup_index, defvert_find_weight, m4_from_glam, m4_to_glam, mul_m4_m4m4, mul_m4_v3,
};
use crate::{ModError, ModEvalCx};

// ------------------------------------------------------------------------------ vertex groups

/// `MOD_get_vgroup` on a mesh: the group index of `name` and the mesh's deform verts. A known
/// group on a mesh without deform verts gives `(Some(index), None)` (Blender's `dvert ==
/// nullptr` with a valid `defgrp_index`).
pub(crate) fn vgroup<'a>(mesh: &'a Mesh, name: &str) -> (Option<u32>, Option<&'a [DeformVert]>) {
    match defgroup_index(mesh, name) {
        Some(g) => (Some(g), mesh.deform_verts().filter(|d| !d.is_empty())),
        None => (None, None),
    }
}

/// `BKE_defvert_find_weight` of vertex `i`, inverted when asked (`1 - w`).
pub(crate) fn vgroup_weight(dvert: &[DeformVert], i: usize, group: u32, invert: bool) -> f32 {
    let w = dvert.get(i).map_or(0.0, |d| defvert_find_weight(d, group));
    if invert { 1.0 - w } else { w }
}

/// `BKE_defvert_array_find_weight_safe`: 1 without a group, 0 (1 inverted) for a known group
/// on a mesh without deform verts, else the (inverted) weight.
pub(crate) fn weight_safe(
    dvert: Option<&[DeformVert]>,
    i: usize,
    group: Option<u32>,
    invert: bool,
) -> f32 {
    let Some(g) = group else {
        return 1.0;
    };
    let Some(dv) = dvert else {
        return if invert { 1.0 } else { 0.0 };
    };
    vgroup_weight(dv, i, g, invert)
}

// ------------------------------------------------------------------------------- matrices

/// The evaluated world matrix of an existing object (`None` for a missing pointer).
pub(crate) fn object_world(cx: &mut dyn ModEvalCx, ob: Option<IdAny>) -> Option<M4> {
    let r = ob.and_then(IdRef::<ObjectId>::from_any)?;
    cx.main().objects.get(r)?;
    Some(m4_from_glam(cx.object_matrix_world(r)))
}

/// `pchan->pose_mat` of bone `bone` of armature object `ob` (`BKE_pose_channel_find_name`),
/// `None` when `ob` is not an armature or has no such bone.
pub(crate) fn pose_mat(cx: &mut dyn ModEvalCx, ob: IdRef<ObjectId>, bone: &str) -> Option<M4> {
    if bone.is_empty() {
        return None;
    }
    let is_armature = cx
        .main()
        .objects
        .get(ob)
        .is_some_and(|o| o.object_type() == "ARMATURE");
    if !is_armature {
        return None;
    }
    let pose = cx.pose(ob).ok()?;
    pose.bones
        .iter()
        .find(|(n, _)| n == bone)
        .map(|(_, m)| m4_from_glam(*m))
}

/// `object_to_world · pose_mat` of the bone, or `object_to_world` without the bone
/// (`matrix_from_obj_pchan` of MOD_uvwarp.cc). `None` for a missing object.
pub(crate) fn obj_pchan_world(cx: &mut dyn ModEvalCx, ob: Option<IdAny>, bone: &str) -> Option<M4> {
    let r = ob.and_then(IdRef::<ObjectId>::from_any)?;
    let world = object_world(cx, ob)?;
    Some(match pose_mat(cx, r, bone) {
        Some(p) => mul_m4_m4m4(&world, &p),
        None => world,
    })
}

/// `invert_m4` in place: the inverse, or the matrix unchanged when it is singular.
pub(crate) fn invert_m4_in_place(m: &mut M4) {
    if let Some(r) = eig_invert_m4(m) {
        *m = r;
    }
}

/// `invert_m4_m4` (`EIG_invert_m4_m4`; a singular matrix gives the zero matrix).
pub(crate) fn inverted(m: &M4) -> M4 {
    eig_invert_m4(m).unwrap_or([[0.0; 4]; 4])
}

/// `vrecpeq_f32` (Arm `FRECPE`, `FPRecipEstimate` of the Arm ARM): an 8-bit reciprocal
/// estimate. Zeros, infinities, NaNs and values whose reciprocal is subnormal fall back to the
/// exact reciprocal (never reached by the inverses frap takes).
fn frecpe(x: f32) -> f32 {
    let b = x.to_bits();
    let sign = b >> 31;
    let exp = (b >> 23) & 0xff;
    let frac = b & 0x7f_ffff;
    if exp == 0 || exp >= 253 {
        return 1.0 / x;
    }
    // `RecipEstimate`: a 9-bit significand in [256, 511].
    let a = (256 | (frac >> 15)) * 2 + 1;
    let r = ((1u32 << 19) / a).div_ceil(2);
    let rexp = 253 - exp;
    f32::from_bits((sign << 31) | (rexp << 23) | ((r & 0xff) << 15))
}

/// The NEON reciprocal of Eigen's `pdiv`: `vrecpeq_f32`, then two `vrecpsq_f32` steps (`FRECPS`
/// computes `2 - a·b` fused).
fn recip_neon(d: f32) -> f32 {
    let mut e = frecpe(d);
    e *= (-d).mul_add(e, 2.0);
    e *= (-d).mul_add(e, 2.0);
    e
}

// Ported from intern/eigen/intern/matrix.cc (EIG_invert_m4_m4, Blender 5.2.4) and Eigen 3.4.0
// Eigen/src/LU/arch/InverseSize4.h (`compute_inverse_size4<Architecture::Target, float>`, MPL-2.0,
// the packet path Blender's arm64 build takes for `Matrix4f`), lane by lane.
/// `EIG_invert_m4_m4`: Eigen's `computeInverseWithCheck(R, invertible, 0)`; `None` when the
/// determinant is zero.
pub(crate) fn eig_invert_m4(m: &M4) -> Option<M4> {
    type P = [f32; 4];
    fn mul(a: P, b: P) -> P {
        [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
    }
    fn add(a: P, b: P) -> P {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
    }
    fn subp(a: P, b: P) -> P {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
    }
    /// `vec4f_swizzle2(a, b, p, q, r, s)`
    fn sw(a: P, b: P, p: usize, q: usize, r: usize, s: usize) -> P {
        [a[p], a[q], b[r], b[s]]
    }
    /// `vec4f_movelh(a, b)`
    fn movelh(a: P, b: P) -> P {
        [a[0], a[1], b[0], b[1]]
    }
    /// `vec4f_movehl(a, b)`
    fn movehl(a: P, b: P) -> P {
        [b[2], b[3], a[2], a[3]]
    }
    fn dup(a: P, i: usize) -> P {
        [a[i]; 4]
    }
    // `determinant()` decides invertibility (`abs(det) > 0`).
    if m4_to_glam(m).determinant() == 0.0 {
        return None;
    }
    let (l1, l2, l3, l4) = (m[0], m[1], m[2], m[3]);
    // Column-major storage on both sides: the 2x2 blocks.
    let a = movelh(l1, l2);
    let b = movehl(l2, l1);
    let c = movelh(l3, l4);
    let d = movehl(l4, l3);
    // AB = A# * B, DC = D# * C.
    let ab = subp(
        mul(sw(a, a, 3, 3, 0, 0), b),
        mul(sw(a, a, 1, 1, 2, 2), sw(b, b, 2, 3, 0, 1)),
    );
    let dc = subp(
        mul(sw(d, d, 3, 3, 0, 0), c),
        mul(sw(d, d, 1, 1, 2, 2), sw(c, c, 2, 3, 0, 1)),
    );
    let det2 = |x: P| {
        let t = mul(sw(x, x, 3, 3, 1, 1), x);
        subp(t, movehl(t, t))
    };
    let (da, db, dc2, dd) = (det2(a), det2(b), det2(c), det2(d));
    let mut dt = mul(sw(dc, dc, 0, 2, 1, 3), ab);
    dt = add(dt, movehl(dt, dt));
    dt = add(dt, sw(dt, dt, 1, 0, 0, 0));
    let d1 = mul(da, dd);
    let d2 = mul(db, dc2);
    let det = dup(subp(add(d1, d2), dt), 0);
    // `pdiv(pset1(1), det)`: Blender's arm64 Eigen divides through the NEON reciprocal
    // estimate and two Newton-Raphson steps (verified bit for bit against 5.2.4's
    // `invert_m4_m4` on scaled and rotated matrices).
    let rd = [
        recip_neon(det[0]),
        recip_neon(det[1]),
        recip_neon(det[2]),
        recip_neon(det[3]),
    ];
    // iD = D*|A| - C*A#*B
    let mut id = mul(sw(c, c, 0, 0, 2, 2), movelh(ab, ab));
    id = add(id, mul(sw(c, c, 1, 1, 3, 3), movehl(ab, ab)));
    id = subp(mul(d, dup(da, 0)), id);
    // iA = A*|D| - B*D#*C
    let mut ia = mul(sw(b, b, 0, 0, 2, 2), movelh(dc, dc));
    ia = add(ia, mul(sw(b, b, 1, 1, 3, 3), movehl(dc, dc)));
    ia = subp(mul(a, dup(dd, 0)), ia);
    // iB = C*|B| - D*B#*A
    let mut ib = mul(d, sw(ab, ab, 3, 0, 3, 0));
    ib = subp(ib, mul(sw(d, d, 1, 0, 3, 2), sw(ab, ab, 2, 1, 2, 1)));
    ib = subp(mul(c, dup(db, 0)), ib);
    // iC = B*|C| - A*C#*D
    let mut ic = mul(a, sw(dc, dc, 3, 0, 3, 0));
    ic = subp(ic, mul(sw(a, a, 1, 0, 3, 2), sw(dc, dc, 2, 1, 2, 1)));
    ic = subp(mul(b, dup(dc2, 0)), ic);
    // `pxor(rd, PNNP)`
    let rd = [rd[0], -rd[1], -rd[2], rd[3]];
    let (ia, ib, ic, id) = (mul(ia, rd), mul(ib, rd), mul(ic, rd), mul(id, rd));
    Some([
        sw(ia, ib, 3, 1, 3, 1),
        sw(ia, ib, 2, 0, 2, 0),
        sw(ic, id, 3, 1, 3, 1),
        sw(ic, id, 2, 0, 2, 0),
    ])
}

/// `interp_v3_v3v3`: `(1 - t)·a + t·b`.
pub(crate) fn interp_v3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `interp_dot_slerp`
fn interp_dot_slerp(t: f32, cosom: f32) -> [f32; 2] {
    let eps = 1e-4f32;
    if cosom.abs() < 1.0 - eps {
        let omega = libm::acosf(cosom);
        let sinom = libm::sinf(omega);
        [
            libm::sinf((1.0 - t) * omega) / sinom,
            libm::sinf(t * omega) / sinom,
        ]
    } else {
        [1.0 - t, t]
    }
}

/// `interp_qt_qtqt` (w-first quaternions).
fn interp_qt_qtqt(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut cosom = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let quat = if cosom < 0.0 {
        cosom = -cosom;
        [-a[0], -a[1], -a[2], -a[3]]
    } else {
        a
    };
    let w = interp_dot_slerp(t, cosom);
    [
        w[0] * quat[0] + w[1] * b[0],
        w[0] * quat[1] + w[1] * b[1],
        w[0] * quat[2] + w[1] * b[2],
        w[0] * quat[3] + w[1] * b[3],
    ]
}

/// `blend_m4_m4m4(out, dst, src, srcweight)`: location, rotation (slerp) and size blended
/// separately.
pub(crate) fn blend_m4_m4m4(dst: &M4, src: &M4, srcweight: f32) -> M4 {
    use frap_math::matrix::{loc_quat_size_to_mat4, mat4_to_loc_rot_size};
    use frap_math::quat::mat3_normalized_to_quat_fast;
    let (dloc, drot, dsize) = mat4_to_loc_rot_size(&m4_to_glam(dst));
    let (sloc, srot, ssize) = mat4_to_loc_rot_size(&m4_to_glam(src));
    let dquat = mat3_normalized_to_quat_fast(&drot);
    let squat = mat3_normalized_to_quat_fast(&srot);
    let floc = interp_v3(dloc.to_array(), sloc.to_array(), srcweight);
    let fquat = interp_qt_qtqt(dquat, squat, srcweight);
    let fsize = interp_v3(dsize.to_array(), ssize.to_array(), srcweight);
    m4_from_glam(loc_quat_size_to_mat4(
        Vec3::from_array(floc),
        fquat,
        Vec3::from_array(fsize),
    ))
}

// ------------------------------------------------------------------------------- textures

/// What `MOD_get_texture_coords` reads from a `MappingInfoModifierData`.
pub(crate) struct Mapping<'a> {
    /// `texture_coords` (`LOCAL`, `GLOBAL`, `OBJECT`, `UV`).
    pub texture_coords: &'a str,
    /// `uv_layer`.
    pub uv_layer: &'a str,
    /// `texture_coords_object`.
    pub object: Option<IdAny>,
    /// `texture_coords_bone`.
    pub bone: &'a str,
}

/// A modifier's texture, ready to sample (`MOD_init_texture`).
pub(crate) struct ModTexture {
    fields: TextureFields,
    is_image: bool,
    image: Option<(ImBuf, bool)>,
}

impl ModTexture {
    /// The texture `tex` of a modifier, `None` without one. Procedural types are a frap
    /// limitation (as for DISPLACE: `NONE` and `IMAGE` textures are evaluated).
    pub(crate) fn load(
        cx: &mut dyn ModEvalCx,
        tex: Option<IdAny>,
        modifier: &str,
    ) -> Result<Option<ModTexture>, ModError> {
        let Some(t) = tex
            .and_then(IdRef::<TextureId>::from_any)
            .and_then(|t| cx.main().textures.get(t).cloned())
        else {
            return Ok(None);
        };
        let is_image = match t.r#type.as_str() {
            "NONE" => false,
            "IMAGE" => true,
            other => {
                return Err(ModError::Error(format!(
                    "frap: texture type '{other}' is not evaluated ({modifier} supports IMAGE \
                     and NONE textures)"
                )));
            }
        };
        let mut image = None;
        if is_image && let Some(im) = t.fields.image.and_then(IdRef::<ImageId>::from_any) {
            image = image_buffer(cx, im)?;
        }
        Ok(Some(ModTexture {
            fields: t.fields.clone(),
            is_image,
            image,
        }))
    }

    /// `BKE_texture_get_value(tex, co, &texres, false)`.
    pub(crate) fn value(&self, co: [f32; 3]) -> TexResult {
        texture_value(
            &self.fields,
            self.is_image,
            self.image.as_ref().map(|(b, a)| (b, *a)),
            co,
        )
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_util.cc (MOD_get_texture_coords).
/// The texture coordinate of every vertex; `cos` are the deformed positions (`vertexCos`).
pub(crate) fn texture_coords(
    cx: &mut dyn ModEvalCx,
    m: &Mapping<'_>,
    mesh: &Mesh,
    cos: &[[f32; 3]],
) -> Vec<[f32; 3]> {
    let n = mesh.verts_num();
    let mut mapping = m.texture_coords;
    let mut mapref_imat = None;
    if mapping == "OBJECT" {
        match m.object.and_then(IdRef::<ObjectId>::from_any) {
            Some(r) if cx.main().objects.get(r).is_some() => {
                let world = m4_from_glam(cx.object_matrix_world(r));
                let mat = match pose_mat(cx, r, m.bone) {
                    Some(p) => mul_m4_m4m4(&world, &p),
                    None => world,
                };
                mapref_imat = Some(inverted(&mat));
            }
            // No map object: default to local.
            _ => mapping = "LOCAL",
        }
    }
    if mapping == "UV" {
        let uv_maps = mesh.uv_maps();
        if !uv_maps.is_empty() {
            let name = if uv_maps.iter().any(|(n, _)| *n == m.uv_layer) {
                Some(m.uv_layer)
            } else {
                mesh.uv_active.as_deref()
            };
            let uvs: Option<&[[f32; 2]]> =
                name.and_then(
                    |nm| match mesh.attributes().get(nm).map(|a| (&a.domain, &a.data)) {
                        Some((Domain::Corner, AttrData::Float2(v))) => Some(v.as_slice()),
                        _ => None,
                    },
                );
            let mut out = vec![[0.0f32; 3]; n];
            let mut done = vec![false; n];
            let cv = mesh.corner_verts();
            for (c, &v) in cv.iter().enumerate() {
                let v = v as usize;
                if !done[v] {
                    let uv = uvs.map_or([0.0, 0.0], |u| u[c]);
                    // Remap the UVs from [0, 1] to [-1, 1].
                    out[v][0] = uv[0] * 2.0 - 1.0;
                    out[v][1] = uv[1] * 2.0 - 1.0;
                    done[v] = true;
                }
            }
            return out;
        }
        // No UVs: default to local.
        mapping = "LOCAL";
    }
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    cos.iter()
        .map(|&p| match (mapping, &mapref_imat) {
            ("GLOBAL", _) => mul_m4_v3(&ob_world, p),
            ("OBJECT", Some(imat)) => mul_m4_v3(imat, mul_m4_v3(&ob_world, p)),
            _ => p,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::weld::util::unit_m4;

    #[test]
    fn blend_halfway_between_translations_and_rotations() {
        let a = unit_m4();
        let mut b = unit_m4();
        b[3] = [2.0, 0.0, 0.0, 1.0];
        // 90° about Z.
        b[0] = [0.0, 1.0, 0.0, 0.0];
        b[1] = [-1.0, 0.0, 0.0, 0.0];
        let m = blend_m4_m4m4(&a, &b, 0.5);
        assert!((m[3][0] - 1.0).abs() < 1e-6);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (m[0][0] - h).abs() < 1e-6 && (m[0][1] - h).abs() < 1e-6,
            "{m:?}"
        );
    }

    #[test]
    fn eigen_inverse_inverts() {
        let mut m = unit_m4();
        m[0] = [0.9, 0.3, -0.2, 0.0];
        m[1] = [-0.25, 1.1, 0.4, 0.0];
        m[2] = [0.1, -0.35, 0.8, 0.0];
        m[3] = [1.5, -2.0, 0.5, 1.0];
        let inv = eig_invert_m4(&m).unwrap();
        let p = mul_m4_m4m4(&m, &inv);
        for (i, c) in p.iter().enumerate() {
            for (j, v) in c.iter().enumerate() {
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((v - e).abs() < 1e-6, "{p:?}");
            }
        }
        assert!(eig_invert_m4(&[[0.0; 4]; 4]).is_none());
    }

    #[test]
    fn weight_safe_follows_blender() {
        assert_eq!(weight_safe(None, 0, None, false), 1.0);
        assert_eq!(weight_safe(None, 0, Some(0), false), 0.0);
        assert_eq!(weight_safe(None, 0, Some(0), true), 1.0);
    }
}
