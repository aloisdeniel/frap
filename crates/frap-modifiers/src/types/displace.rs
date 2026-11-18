// SPDX-License-Identifier: GPL-3.0-or-later
//! DISPLACE (`DisplaceModifier`): moves every vertex by `strength · (value − mid_level)` along
//! X, Y, Z (local or `GLOBAL` axes), the vertex normal, the custom normal, or the texture's RGB
//! as an XYZ vector; the value is the texture's intensity at the vertex's texture coordinate
//! (`LOCAL`, `GLOBAL`, `OBJECT`, `UV`), or white without a texture; `vertex_group` scales it.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_displace.cc and
//! blender/source/blender/modifiers/intern/MOD_util.cc (MOD_get_texture_coords), Blender
//! 5.2.4; texture evaluation in [`tex`].
//!
//! Texture types: `NONE` and `IMAGE` (OPERATORS.md §B). A procedural texture (`CLOUDS`, …) is a
//! modifier error naming the type; frap does not evaluate them. Image textures read packed or
//! file images and generated blank ones (`UV_GRID`/`COLOR_GRID` patterns are an error);
//! `texture_coords_bone` needs evaluated poses (M6), so the object's matrix is used, as Blender
//! does when the bone does not exist.

mod tex;

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{IdAny, IdRef, ImageId, ObjectId, TextureId};
use frap_scene::generated::modifier_settings::DisplaceSettings;
use smallvec::SmallVec;

use self::tex::{ImBuf, texture_value};
use super::weld::util::{
    M4, defgroup_index, defvert_find_weight, invert_m4, m4_from_glam, mul_m4_v3,
};
use crate::prelude::*;

fn displace_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Displace(d) = s {
        v.extend(d.texture);
        v.extend(d.texture_coords_object);
    }
    v
}

fn displace_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Displace(d)
            if (d.texture.is_none() && d.direction == "RGB_TO_XYZ") || d.strength == 0.0 =>
        {
            Some("Displace: no texture for RGB to XYZ, or zero strength")
        }
        _ => None,
    }
}

#[modifier(
    DISPLACE,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE,
    depends_on = displace_deps,
    is_disabled = displace_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Displace(s) = &md.settings else {
        return Ok(mesh);
    };
    let texture = s
        .texture
        .and_then(IdRef::<TextureId>::from_any)
        .and_then(|t| cx.main().textures.get(t).cloned());
    // The texture's image buffer.
    let mut image: Option<(ImBuf, bool)> = None;
    if let Some(t) = &texture {
        match t.r#type.as_str() {
            "NONE" | "IMAGE" => {}
            other => {
                return Err(ModError::Error(format!(
                    "frap: texture type '{other}' is not evaluated (DISPLACE supports IMAGE and \
                     NONE textures)"
                )));
            }
        }
        if t.r#type == "IMAGE"
            && let Some(im) = t.fields.image.and_then(IdRef::<ImageId>::from_any)
        {
            image = image_buffer(cx, im)?;
        }
    }
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    let map_object = s
        .texture_coords_object
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|o| cx.main().objects.get(*o).is_some());
    let map_world = map_object.map(|o| m4_from_glam(cx.object_matrix_world(o)));
    displace(
        s,
        texture.as_ref().map(|t| (&t.fields, t.r#type == "IMAGE")),
        image.as_ref().map(|(b, a)| (b, *a)),
        &ob_world,
        map_world.as_ref(),
        &mut mesh,
    );
    Ok(mesh)
}

/// The `ImBuf` Blender samples for an image, with "the image has alpha" (`alpha_mode` other
/// than `'NONE'`); `None` when it has no pixels.
fn image_buffer(
    cx: &mut dyn ModEvalCx,
    im: IdRef<ImageId>,
) -> Result<Option<(ImBuf, bool)>, ModError> {
    let Some(image) = cx.main().images.get(im) else {
        return Ok(None);
    };
    let has_alpha = image.alpha_mode != "NONE";
    if image.source == "GENERATED" && image.packed_file.is_none() {
        if image.generated_type != "BLANK" {
            return Err(ModError::Error(format!(
                "frap: generated image type '{}' is not evaluated (BLANK only)",
                image.generated_type
            )));
        }
        let (w, h) = (
            image.generated_width.max(1) as usize,
            image.generated_height.max(1) as usize,
        );
        let buf = if image.use_generated_float {
            let data = image.colorspace_settings.is_data;
            ImBuf::generated_float(w, h, image.generated_color, data)
        } else {
            ImBuf::generated_byte(w, h, image.generated_color)
        };
        return Ok(Some((buf, has_alpha)));
    }
    let srgb = image.colorspace_settings.name == "sRGB";
    Ok(cx
        .image(im)
        .and_then(|d| ImBuf::from_decoded(&d, srgb))
        .map(|b| (b, has_alpha)))
}

// Ported from blender/source/blender/modifiers/intern/MOD_displace.cc (displaceModifier_do,
// displaceModifier_do_task).
/// Displaces `mesh` in place. `texture` is the texture's fields and whether it is an image
/// texture; `image` its buffer.
pub(crate) fn displace(
    s: &DisplaceSettings,
    texture: Option<(&frap_scene::generated::plain::TextureFields, bool)>,
    image: Option<(&ImBuf, bool)>,
    ob_world: &M4,
    map_world: Option<&M4>,
    mesh: &mut Mesh,
) {
    let direction = s.direction.as_str();
    let use_global_direction = s.space == "GLOBAL";
    if texture.is_none() && direction == "RGB_TO_XYZ" {
        return;
    }
    if s.strength == 0.0 {
        return;
    }
    let defgrp = defgroup_index(mesh, &s.vertex_group);
    let dvert = defgrp.and_then(|_| mesh.deform_verts().map(|d| d.to_vec()));
    if defgrp.is_some() && dvert.is_none() {
        // A vertex group without vertices.
        return;
    }
    let tex_co = texture.map(|_| texture_coords(s, mesh, ob_world, map_world));
    let vert_normals: Option<std::sync::Arc<Vec<[f32; 3]>>> = match direction {
        "NORMAL" => Some(true_vert_normals(mesh)),
        "CUSTOM_NORMAL" => Some(mesh.vert_normals()),
        _ => None,
    };
    let local_mat = *ob_world;
    // Without a texture: white.
    let delta_fixed = 1.0 - s.mid_level;
    let invert_vgroup = s.invert_vertex_group;
    let positions = mesh.positions_mut();
    for i in 0..positions.len() {
        let mut weight = 1.0f32;
        if let (Some(dv), Some(g)) = (&dvert, defgrp) {
            let w = defvert_find_weight(&dv[i], g);
            weight = if invert_vgroup { 1.0 - w } else { w };
            if weight == 0.0 {
                continue;
            }
        }
        let mut strength = s.strength;
        let mut texres = tex::TexResult::default();
        let mut delta = if let (Some((fields, is_image)), Some(co)) = (texture, &tex_co) {
            texres = texture_value(fields, is_image, image, co[i]);
            texres.tin - s.mid_level
        } else {
            delta_fixed
        };
        if dvert.is_some() {
            strength *= weight;
        }
        delta *= strength;
        delta = delta.clamp(-10000.0, 10000.0);
        let p = &mut positions[i];
        match direction {
            "X" | "Y" | "Z" => {
                let k = match direction {
                    "X" => 0,
                    "Y" => 1,
                    _ => 2,
                };
                if use_global_direction {
                    p[0] += delta * local_mat[0][k];
                    p[1] += delta * local_mat[1][k];
                    p[2] += delta * local_mat[2][k];
                } else {
                    p[k] += delta;
                }
            }
            "RGB_TO_XYZ" => {
                let mut v = [
                    texres.trgba[0] - s.mid_level,
                    texres.trgba[1] - s.mid_level,
                    texres.trgba[2] - s.mid_level,
                ];
                if use_global_direction {
                    // `mul_transposed_mat3_m4_v3`.
                    let [x, y, z] = v;
                    v = [
                        local_mat[0][0] * x + local_mat[0][1] * y + local_mat[0][2] * z,
                        local_mat[1][0] * x + local_mat[1][1] * y + local_mat[1][2] * z,
                        local_mat[2][0] * x + local_mat[2][1] * y + local_mat[2][2] * z,
                    ];
                }
                for k in 0..3 {
                    p[k] += v[k] * strength;
                }
            }
            "NORMAL" | "CUSTOM_NORMAL" => {
                if let Some(n) = &vert_normals {
                    for k in 0..3 {
                        p[k] += n[i][k] * delta;
                    }
                }
            }
            _ => {}
        }
    }
}

/// `Mesh::vert_normals_true`: the vertex normals ignoring custom normals.
fn true_vert_normals(mesh: &Mesh) -> std::sync::Arc<Vec<[f32; 3]>> {
    if mesh.attributes().get(frap_mesh::CUSTOM_NORMAL).is_none() {
        return mesh.vert_normals();
    }
    let mut m = mesh.clone();
    m.attributes_mut().remove(frap_mesh::CUSTOM_NORMAL);
    m.vert_normals()
}

// Ported from blender/source/blender/modifiers/intern/MOD_util.cc (MOD_get_texture_coords).
/// The texture coordinate of every vertex.
fn texture_coords(
    s: &DisplaceSettings,
    mesh: &Mesh,
    ob_world: &M4,
    map_world: Option<&M4>,
) -> Vec<[f32; 3]> {
    let n = mesh.verts_num();
    let mut mapping = s.texture_coords.as_str();
    let mut mapref_imat = None;
    if mapping == "OBJECT" {
        match map_world {
            Some(m) => mapref_imat = Some(invert_m4(m)),
            // No map object: default to local.
            None => mapping = "LOCAL",
        }
    }
    if mapping == "UV" {
        let uv_maps = mesh.uv_maps();
        if !uv_maps.is_empty() {
            let name = if uv_maps.iter().any(|(n, _)| *n == s.uv_layer) {
                Some(s.uv_layer.as_str())
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
            for c in 0..mesh.corners_num() {
                let v = cv[c] as usize;
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
    mesh.positions()
        .iter()
        .map(|&p| match (mapping, &mapref_imat) {
            ("GLOBAL", _) => mul_m4_v3(ob_world, p),
            // `mul_v3_m4v3(ob_world)` then `mul_m4_v3(mapref_imat)`.
            ("OBJECT", Some(imat)) => mul_m4_v3(imat, mul_m4_v3(ob_world, p)),
            _ => p,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::generated::plain::TextureFields;

    fn cube() -> Mesh {
        frap_mesh::primitives::cube(2.0, true, [1.0; 3])
    }

    #[test]
    fn without_texture_moves_along_normals() {
        let mut m = cube();
        let s = DisplaceSettings::default();
        displace(
            &s,
            None,
            None,
            &super::super::weld::util::unit_m4(),
            None,
            &mut m,
        );
        // Strength 1 · (1 − 0.5) along the corner's vertex normal (1/√3 per axis).
        let p = m.positions()[0];
        let n = 0.5 / 3f32.sqrt();
        assert!((p[0].abs() - (1.0 + n)).abs() < 1e-6, "{p:?}");
    }

    /// Image sampling against Blender 5.2.4: a 5×3 RGBA PNG (straight alpha, some pixels half
    /// transparent) displacing a 7×7 grid along Z for several texture settings
    /// (`testdata/p5x3_displace.json`, written by Blender from the same PNG).
    #[test]
    fn image_sampling_matches_blender() {
        let png = include_bytes!("displace/testdata/p5x3.png");
        let dump: serde_json::Value =
            serde_json::from_str(include_str!("displace/testdata/p5x3_displace.json")).unwrap();
        let decoded = frap_imbuf::decode(png).expect("PNG decodes");
        let ibuf = ImBuf::from_decoded(&decoded, true).expect("an image buffer");
        let grid = frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
            x_subdivisions: 6,
            y_subdivisions: 6,
            size: 2.6,
            ..Default::default()
        });
        let cases = dump["cases"].as_object().unwrap();
        for (name, c) in cases {
            let mut f = TextureFields::default();
            for (k, v) in c["tex"].as_object().unwrap() {
                match k.as_str() {
                    "repeat_x" => f.repeat_x = v.as_i64().unwrap() as i32,
                    "repeat_y" => f.repeat_y = v.as_i64().unwrap() as i32,
                    "use_mirror_x" => f.use_mirror_x = v.as_bool().unwrap(),
                    "extension" => f.extension = v.as_str().unwrap().into(),
                    "use_interpolation" => f.use_interpolation = v.as_bool().unwrap(),
                    "use_checker_even" => f.use_checker_even = v.as_bool().unwrap(),
                    "checker_distance" => f.checker_distance = v.as_f64().unwrap() as f32,
                    "use_calculate_alpha" => f.use_calculate_alpha = v.as_bool().unwrap(),
                    "filter_size" => f.filter_size = v.as_f64().unwrap() as f32,
                    "use_flip_axis" => f.use_flip_axis = v.as_bool().unwrap(),
                    "use_alpha" => f.use_alpha = v.as_bool().unwrap(),
                    "crop_min_x" => f.crop_min_x = v.as_f64().unwrap() as f32,
                    "crop_max_x" => f.crop_max_x = v.as_f64().unwrap() as f32,
                    "invert_alpha" => f.invert_alpha = v.as_bool().unwrap(),
                    "use_clamp" => f.use_clamp = v.as_bool().unwrap(),
                    "contrast" => f.contrast = v.as_f64().unwrap() as f32,
                    other => panic!("unhandled texture setting {other}"),
                }
            }
            let s = DisplaceSettings {
                direction: c
                    .get("direction")
                    .and_then(|d| d.as_str())
                    .unwrap_or("Z")
                    .into(),
                texture_coords: c["coords"].as_str().unwrap().into(),
                ..DisplaceSettings::default()
            };
            let mut m = grid.clone();
            let id = super::super::weld::util::unit_m4();
            displace(&s, Some((&f, true)), Some((&ibuf, true)), &id, None, &mut m);
            let expect = dump["positions"][name].as_array().unwrap();
            for (i, (p, e)) in m.positions().iter().zip(expect).enumerate() {
                for k in 0..3 {
                    let e = e[k].as_f64().unwrap() as f32;
                    assert!(
                        (p[k] - e).abs() < 2e-5,
                        "{name}: vertex {i}[{k}] frap {} blender {e}",
                        p[k]
                    );
                }
            }
        }
    }

    #[test]
    fn none_texture_moves_inwards_on_z() {
        let mut m = cube();
        let s = DisplaceSettings {
            direction: "Z".into(),
            strength: 2.0,
            ..DisplaceSettings::default()
        };
        let f = TextureFields::default();
        let before = m.positions()[1];
        displace(
            &s,
            Some((&f, false)),
            None,
            &super::super::weld::util::unit_m4(),
            None,
            &mut m,
        );
        // Intensity 0: 2 · (0 − 0.5).
        assert_eq!(m.positions()[1][2], before[2] - 1.0);
    }

    /// Against Blender: the inputs of `tests/golden/cases/modifiers/a/displace/no_texture.py`.
    #[test]
    fn matches_blender_fixtures() {
        use super::super::weld::util::fixture::assert_evaluated;
        let id = super::super::weld::util::unit_m4();
        let cube = super::super::weld::util::fixture::factory_cube;
        let mut m = cube();
        displace(&DisplaceSettings::default(), None, None, &id, None, &mut m);
        assert_evaluated("displace/no_texture", "Cube", &m, 1e-5);
        let mut m = cube();
        let s = DisplaceSettings {
            direction: "X".into(),
            strength: 2.5,
            mid_level: 0.2,
            ..DisplaceSettings::default()
        };
        displace(&s, None, None, &id, None, &mut m);
        assert_evaluated("displace/no_texture", "Cube.001", &m, 1e-5);
    }
}
