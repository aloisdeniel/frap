// SPDX-License-Identifier: GPL-3.0-or-later
//! The helpers shared by VERTEX_WEIGHT_EDIT, VERTEX_WEIGHT_MIX and VERTEX_WEIGHT_PROXIMITY:
//! the falloff mapping, the influence masking (global factor or mask vertex group) and the
//! write-back to the vertex group (normalize, clamp, add above / remove below a threshold).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weightvg_util.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `weightvg_do_map`, `weightvg_do_mask` (constant and vertex-group
//! masks), `weightvg_update_vg`; and `BLI_ghashutil_strhash_p`
//! (blenlib/intern/BLI_ghash_utils.cc) for the `RANDOM` falloff seed.
//!
//! The `CURVE` falloff evaluates the modifier's default curve (0,0)–(1,1): `map_curve` is not
//! bound (its `CurveMapping` is not dumped, OPERATORS.md §B), so the curve is always the
//! identity clipped to [0, 1]. Mask textures (`mask_texture`) are not evaluated: the modifier
//! reports a `frap:` error and passes the mesh through.

use frap_bmesh::shape::deform;
use frap_math::rng::BliRng;
use frap_mesh::{DeformVert, Mesh};

use crate::ModError;

/// `MOD_WVG_ZEROFLOOR`
pub(crate) const ZEROFLOOR: f32 = 1.0e-32;

/// `MOD_WVG_MAPPING_*` of a `falloff_type` identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Falloff {
    /// `LINEAR` (`MOD_WVG_MAPPING_NONE`)
    None,
    Curve,
    Sharp,
    Smooth,
    Root,
    /// `ICON_SPHERECURVE`
    Sphere,
    Random,
    Step,
}

impl Falloff {
    pub(crate) fn from_identifier(id: &str) -> Falloff {
        match id {
            "CURVE" => Falloff::Curve,
            "SHARP" => Falloff::Sharp,
            "SMOOTH" => Falloff::Smooth,
            "ROOT" => Falloff::Root,
            "ICON_SPHERECURVE" => Falloff::Sphere,
            "RANDOM" => Falloff::Random,
            "STEP" => Falloff::Step,
            _ => Falloff::None,
        }
    }
}

/// `BLI_ghashutil_strhash_p`: djb2 over the bytes.
pub(crate) fn strhash(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for &c in s.as_bytes() {
        h = (h << 5).wrapping_add(h).wrapping_add((c as i8) as u32);
    }
    h
}

/// The RNG of the `RANDOM` falloff: `BLI_rng_new_srandom(BLI_ghashutil_strhash(ob->id.name + 2))`.
pub(crate) fn falloff_rng(falloff: Falloff, object_name: &str) -> Option<BliRng> {
    (falloff == Falloff::Random).then(|| BliRng::new_srandom(strhash(object_name)))
}

/// `weightvg_do_map`: maps each weight through the falloff, inverted with `do_invert`.
pub(crate) fn do_map(
    new_w: &mut [f32],
    falloff: Falloff,
    do_invert: bool,
    rng: Option<&mut BliRng>,
) {
    if !do_invert && falloff == Falloff::None {
        return;
    }
    let mut rng = rng;
    for w in new_w.iter_mut() {
        let mut fac = *w;
        fac = match falloff {
            // The default `CurveMapping` (0,0)–(1,1), clipped.
            Falloff::Curve => fac.clamp(0.0, 1.0),
            Falloff::Sharp => fac * fac,
            Falloff::Smooth => 3.0 * fac * fac - 2.0 * fac * fac * fac,
            Falloff::Root => fac.sqrt(),
            Falloff::Sphere => (2.0 * fac - fac * fac).sqrt(),
            Falloff::Random => match rng.as_deref_mut() {
                Some(r) => r.get_float() * fac,
                None => fac,
            },
            Falloff::Step => {
                if fac >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Falloff::None => fac,
        };
        *w = if do_invert { 1.0 - fac } else { fac };
    }
}

/// The masking inputs of the three modifiers.
pub(crate) struct Mask<'a> {
    /// `mask_constant`
    pub constant: f32,
    /// `mask_vertex_group`
    pub vertex_group: &'a str,
    /// `invert_mask_vertex_group`
    pub invert_vertex_group: bool,
}

/// The error of a mask texture (not evaluated by frap).
pub(crate) fn texture_unsupported(ty: &str) -> ModError {
    ModError::Error(format!(
        "frap: {ty} mask_texture is not evaluated (texture masks are not supported)"
    ))
}

/// `weightvg_do_mask`: mixes `new_w` into `org_w` (indexed by `indices`, or by vertex)
/// with the global factor, scaled by the mask group's weights when it exists.
pub(crate) fn do_mask(
    mesh: &Mesh,
    indices: Option<&[usize]>,
    org_w: &mut [f32],
    new_w: &[f32],
    mask: &Mask<'_>,
) {
    let fact = mask.constant;
    if fact == 0.0 {
        return;
    }
    let ref_didx = if mask.vertex_group.is_empty() {
        None
    } else {
        mesh.vertex_group_names
            .iter()
            .position(|n| n == mask.vertex_group)
    };
    if let Some(g) = ref_didx {
        let Some(dvert) = mesh.deform_verts() else {
            return;
        };
        for i in 0..org_w.len() {
            let idx = indices.map_or(i, |x| x[i]);
            let w = deform::weight(&dvert[idx], g as u32);
            let f = (if mask.invert_vertex_group { 1.0 - w } else { w }) * fact;
            org_w[i] = (new_w[i] * f) + (org_w[i] * (1.0 - f));
        }
    } else {
        let ifact = 1.0 - fact;
        for i in 0..org_w.len() {
            org_w[i] = (new_w[i] * fact) + (org_w[i] * ifact);
        }
    }
}

/// `weightvg_update_vg`: writes `weights` (normalized to the [min, max] range with
/// `do_normalize`, clamped to [0, 1]) into group `g` of the vertices `indices` (all without);
/// a vertex in the group at or below `rem_thresh` leaves it (`do_rem`), one outside at or above
/// `add_thresh` joins it (`do_add`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_vg(
    dvert: &mut [DeformVert],
    g: u32,
    indices: Option<&[usize]>,
    weights: &[f32],
    do_add: bool,
    add_thresh: f32,
    do_rem: bool,
    rem_thresh: f32,
    do_normalize: bool,
) {
    let num = weights.len();
    if num == 0 {
        return;
    }
    let mut min_w = weights[0];
    let mut norm_fac = 1.0f32;
    if do_normalize {
        let mut max_w = weights[0];
        for &w in &weights[1..] {
            if w < min_w {
                min_w = w;
            } else if w > max_w {
                max_w = w;
            }
        }
        let range = max_w - min_w;
        if range.abs() > f32::EPSILON {
            norm_fac = 1.0 / range;
        } else {
            min_w = 0.0;
        }
    }
    for i in 0..num {
        let mut w = weights[i];
        let dv = &mut dvert[indices.map_or(i, |x| x[i])];
        let dw = deform::find(dv, g);
        if do_normalize {
            w = (w - min_w) * norm_fac;
        }
        w = w.clamp(0.0, 1.0);
        match dw {
            Some(k) => {
                if do_rem && w <= rem_thresh {
                    deform::remove_at(dv, k);
                } else {
                    dv.weights[k].1 = w;
                }
            }
            None => {
                if do_add && w >= add_thresh {
                    deform::add_notest(dv, g, w);
                }
            }
        }
    }
}

/// `BKE_id_defgroup_name_index(&mesh->id, name)`.
pub(crate) fn group_index(mesh: &Mesh, name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    mesh.vertex_group_names
        .iter()
        .position(|n| n == name)
        .map(|i| i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloffs() {
        let mut w = [0.25f32, 0.5, 1.0];
        do_map(&mut w, Falloff::Sharp, false, None);
        assert_eq!(w, [0.0625, 0.25, 1.0]);
        let mut w = [0.25f32, 0.75];
        do_map(&mut w, Falloff::Step, true, None);
        assert_eq!(w, [1.0, 0.0]);
        let mut w = [0.5f32];
        do_map(&mut w, Falloff::Smooth, false, None);
        assert_eq!(w, [0.5]);
        let mut w = [0.36f32];
        do_map(&mut w, Falloff::Root, false, None);
        assert!((w[0] - 0.6).abs() < 1e-6);
        assert_eq!(strhash("Cube"), {
            let mut h: u32 = 5381;
            for c in "Cube".bytes() {
                h = h.wrapping_mul(33).wrapping_add(u32::from(c));
            }
            h
        });
    }

    #[test]
    fn strhash_sign_extends_non_ascii_bytes() {
        // Blender reads the name as `const signed char *`: UTF-8 bytes >= 0x80 sign-extend.
        // "É" is 0xC3 0x89.
        let expected = 5381u32
            .wrapping_mul(33)
            .wrapping_add(0xFFFF_FFC3)
            .wrapping_mul(33)
            .wrapping_add(0xFFFF_FF89);
        assert_eq!(strhash("\u{c9}"), expected);
        assert_ne!(strhash("\u{c9}bauche"), strhash("Ebauche"));
    }

    #[test]
    fn update_adds_removes_and_normalizes() {
        let mut dv = vec![
            DeformVert {
                weights: smallvec::smallvec![(0, 0.5)],
            },
            DeformVert::default(),
            DeformVert {
                weights: smallvec::smallvec![(1, 0.3), (0, 0.9)],
            },
        ];
        update_vg(
            &mut dv,
            0,
            None,
            &[0.05, 0.6, 2.0],
            true,
            0.5,
            true,
            0.1,
            false,
        );
        assert!(dv[0].weights.is_empty(), "removed at or below 0.1");
        assert_eq!(dv[1].weights.as_slice(), &[(0, 0.6)]);
        assert_eq!(dv[2].weights.as_slice(), &[(1, 0.3), (0, 1.0)], "clamped");
        let mut dv = vec![DeformVert::default(); 3];
        update_vg(
            &mut dv,
            2,
            None,
            &[1.0, 2.0, 3.0],
            true,
            0.0,
            false,
            0.0,
            true,
        );
        let w: Vec<f32> = dv.iter().map(|d| d.weights[0].1).collect();
        assert_eq!(w, [0.0, 0.5, 1.0]);
    }
}
