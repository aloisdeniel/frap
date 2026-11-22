// SPDX-License-Identifier: GPL-3.0-or-later
//! WARP (`WarpModifier`): moves the vertices near `object_from` (or its bone `bone_from`) the
//! way `object_to` is placed relative to it, weighted by a falloff of the distance to
//! `object_from` (`falloff_type`, `falloff_radius`), `strength`, `vertex_group` and the
//! texture's intensity; `use_volume_preserve` blends the matrices (location, rotation and
//! scale separately) instead of the positions.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_warp.cc (Blender 5.2.4,
//! GPL-2.0-or-later). `falloff_curve` is not bound by frap (deviations §2), so the `CURVE`
//! falloff evaluates the default curve every new modifier has
//! (`BKE_curvemapping_add(1, 0, 0, 1, 1)`) through Blender's table ([`curve`]), as in Blender
//! until the curve is edited.

mod curve;
pub(crate) mod shared;

use frap_scene::generated::modifier_settings::WarpSettings;
use smallvec::SmallVec;

use self::shared::{
    Mapping, ModTexture, blend_m4_m4m4, interp_v3, invert_m4_in_place, inverted, obj_pchan_world,
    texture_coords, vgroup, vgroup_weight,
};
use super::weld::util::{M4, len_sq, m4_from_glam, mul_m4_m4m4, mul_m4_v3, unit_m4};
use crate::prelude::*;
use frap_rna::IdAny;

fn warp_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Warp(w) = s {
        v.extend(w.object_from);
        v.extend(w.object_to);
        v.extend(w.texture);
        v.extend(w.texture_coords_object);
    }
    v
}

fn warp_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Warp(w) if w.object_from.is_none() || w.object_to.is_none() => {
            Some("Warp: needs both 'from' and 'to' objects")
        }
        _ => None,
    }
}

#[modifier(
    WARP,
    kind = OnlyDeform,
    flags = ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE,
    depends_on = warp_deps,
    is_disabled = warp_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Warp(s) = &md.settings else {
        return Ok(mesh);
    };
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    let (Some(from), Some(to)) = (
        obj_pchan_world(cx, s.object_from, &s.bone_from),
        obj_pchan_world(cx, s.object_to, &s.bone_to),
    ) else {
        return Ok(mesh);
    };
    let texture = ModTexture::load(cx, s.texture, "WARP")?;
    let tex_co = match &texture {
        Some(_) => {
            let m = Mapping {
                texture_coords: &s.texture_coords,
                uv_layer: &s.uv_layer,
                object: s.texture_coords_object,
                bone: &s.texture_coords_bone,
            };
            let cos = mesh.positions().to_vec();
            Some(texture_coords(cx, &m, &mesh, &cos))
        }
        None => None,
    };
    let obinv = inverted(&ob_world);
    warp(
        s,
        &mul_m4_m4m4(&obinv, &from),
        &mul_m4_m4m4(&obinv, &to),
        texture.as_ref().zip(tex_co.as_deref()),
        &mut mesh,
    );
    Ok(mesh)
}

// Ported from blender/source/blender/modifiers/intern/MOD_warp.cc (warpModifier_do).
/// Warps `mesh` in place. `mat_from`/`mat_to` are the two objects (or bones) in the space of
/// the deformed object.
pub(crate) fn warp(
    s: &WarpSettings,
    mat_from: &M4,
    mat_to: &M4,
    texture: Option<(&ModTexture, &[[f32; 3]])>,
    mesh: &mut Mesh,
) {
    let falloff_radius_sq = s.falloff_radius * s.falloff_radius;
    let mut strength = s.strength;
    let (defgrp, dvert) = vgroup(mesh, &s.vertex_group);
    let dvert = dvert.map(<[_]>::to_vec);
    let defgrp = defgrp.filter(|_| dvert.is_some());
    let invert_vgroup = s.invert_vertex_group;

    let tmat = inverted(mat_from);
    let mut mat_final = mul_m4_m4m4(&tmat, mat_to);
    let mat_from_inv = inverted(mat_from);
    let mat_unit = unit_m4();

    if strength < 0.0 {
        strength = -strength;
        // An inverted location is not useful: use the negative.
        let loc = mat_final[3];
        invert_m4_in_place(&mut mat_final);
        mat_final[3][0] = -loc[0];
        mat_final[3][1] = -loc[1];
        mat_final[3][2] = -loc[2];
    }
    let mut weight = strength;
    let falloff = s.falloff_type.as_str();

    let positions = mesh.positions_mut();
    for (i, co) in positions.iter_mut().enumerate() {
        let mut fac = 1.0f32;
        if falloff != "NONE" {
            fac = len_sq(*co, [mat_from[3][0], mat_from[3][1], mat_from[3][2]]);
            if fac >= falloff_radius_sq {
                continue;
            }
            fac = (s.falloff_radius - fac.sqrt()) / s.falloff_radius;
            if fac == 0.0 {
                continue;
            }
        }
        if let (Some(g), Some(dv)) = (defgrp, &dvert) {
            weight = vgroup_weight(dv, i, g, invert_vgroup) * strength;
            if weight <= 0.0 {
                continue;
            }
        }
        // Closely match PROP_SMOOTH and similar.
        fac = match falloff {
            "NONE" | "CONSTANT" => 1.0,
            "CURVE" => curve::evaluate_default(fac),
            "LINEAR" => fac,
            "SHARP" => fac * fac,
            "SMOOTH" => 3.0 * fac * fac - 2.0 * fac * fac * fac,
            "ROOT" => fac.sqrt(),
            "SPHERE" => (2.0 * fac - fac * fac).sqrt(),
            "INVERSE_SQUARE" => fac * (2.0 - fac),
            _ => fac,
        };
        fac *= weight;
        if let Some((tex, tex_co)) = texture {
            fac *= tex.value(tex_co[i]).tin;
        }
        if fac != 0.0 {
            // Into the 'from' object's space.
            *co = mul_m4_v3(&mat_from_inv, *co);
            if fac == 1.0 {
                *co = mul_m4_v3(&mat_final, *co);
            } else if s.use_volume_preserve {
                // Interpolate the matrix for nicer locations.
                let t = blend_m4_m4m4(&mat_unit, &mat_final, fac);
                *co = mul_m4_v3(&t, *co);
            } else {
                let tvec = mul_m4_v3(&mat_final, *co);
                *co = interp_v3(*co, tvec, fac);
            }
            // Out of the 'from' object's space.
            *co = mul_m4_v3(mat_from, *co);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(x: f32, y: f32, z: f32) -> M4 {
        let mut m = unit_m4();
        m[3] = [x, y, z, 1.0];
        m
    }

    #[test]
    fn full_strength_without_falloff_moves_by_the_offset() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let before = m.positions().to_vec();
        let s = WarpSettings {
            falloff_type: "NONE".into(),
            ..WarpSettings::default()
        };
        warp(
            &s,
            &translate(0.0, 0.0, 0.0),
            &translate(0.5, 0.0, 0.0),
            None,
            &mut m,
        );
        for (a, b) in m.positions().iter().zip(&before) {
            assert!((a[0] - b[0] - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn outside_the_radius_nothing_moves() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let before = m.positions().to_vec();
        let s = WarpSettings {
            falloff_radius: 0.5,
            ..WarpSettings::default()
        };
        warp(
            &s,
            &translate(0.0, 0.0, 0.0),
            &translate(0.5, 0.0, 0.0),
            None,
            &mut m,
        );
        assert_eq!(m.positions(), &before[..]);
    }
}
