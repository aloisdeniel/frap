// SPDX-License-Identifier: GPL-3.0-or-later
//! VERTEX_WEIGHT_MIX (`VertexWeightMixModifier`): mixes the weights of `vertex_group_b` (or
//! `default_weight_b`) into `vertex_group_a` (or `default_weight_a`) with `mix_mode` (`SET`,
//! `ADD`, `SUB`, `MUL`, `DIV`, `DIF`, `AVG`, `MIN`, `MAX`), on the vertices of `mix_set` (`ALL`,
//! in `A`, in `B`, in either, in both), with `invert_vertex_group_a/_b`, the influence
//! (`mask_constant`, `mask_vertex_group`) and `normalize`; affected vertices outside group A
//! are added to it.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weightvgmix.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `mix_weight` and `modify_mesh`.

use frap_bmesh::shape::deform;
use frap_rna::IdAny;
use frap_scene::generated::modifier_settings::VertexWeightMixSettings;

use super::vertex_weight_edit::weightvg::{self, Mask, ZEROFLOOR, do_mask, group_index, update_vg};
use crate::prelude::*;

/// `foreach_ID_link`: the mask texture and its mapping object.
fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::VertexWeightMix(w) = s {
        v.extend(w.mask_texture);
        v.extend(w.mask_tex_map_object);
    }
    v
}

/// `is_disabled`: no vertex group A.
fn disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::VertexWeightMix(w) if w.vertex_group_a.is_empty() => {
            Some("VertexWeightMix: no vertex group A")
        }
        _ => None,
    }
}

#[modifier(
    VERTEX_WEIGHT_MIX,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::VertexWeightMix(s) = &md.settings else {
        return Ok(mesh);
    };
    if s.mask_texture.is_some() {
        return Err(weightvg::texture_unsupported("VERTEX_WEIGHT_MIX"));
    }
    Ok(vertex_weight_mix(s, mesh))
}

/// `mix_weight`
pub(crate) fn mix_weight(weight: f32, mut weight2: f32, mix_mode: &str) -> f32 {
    match mix_mode {
        "ADD" => weight + weight2,
        "SUB" => weight - weight2,
        "MUL" => weight * weight2,
        "DIV" => {
            if weight2 < 0.0 && weight2 > -ZEROFLOOR {
                weight2 = -ZEROFLOOR;
            } else if (0.0..ZEROFLOOR).contains(&weight2) {
                weight2 = ZEROFLOOR;
            }
            weight / weight2
        }
        "DIF" => {
            if weight < weight2 {
                weight2 - weight
            } else {
                weight - weight2
            }
        }
        "AVG" => (weight + weight2) * 0.5,
        "MIN" => {
            if weight < weight2 {
                weight
            } else {
                weight2
            }
        }
        "MAX" => {
            if weight > weight2 {
                weight
            } else {
                weight2
            }
        }
        _ => weight2,
    }
}

/// `modify_mesh` of VERTEX_WEIGHT_MIX.
pub(crate) fn vertex_weight_mix(s: &VertexWeightMixSettings, mut mesh: Mesh) -> Mesh {
    let verts_num = mesh.verts_num();
    if verts_num == 0 || mesh.vertex_group_names.is_empty() {
        return mesh;
    }
    let Some(ga) = group_index(&mesh, &s.vertex_group_a) else {
        return mesh;
    };
    let gb = if s.vertex_group_b.is_empty() {
        None
    } else {
        match group_index(&mesh, &s.vertex_group_b) {
            Some(g) => Some(g),
            None => return mesh,
        }
    };
    let has_mdef = mesh.deform_verts().is_some_and(|d| !d.is_empty());
    if !has_mdef && s.mix_set != "ALL" {
        return mesh;
    }
    mesh.deform_verts_mut();
    let dvert = mesh.deform_verts().unwrap_or(&[]);
    let find = |i: usize, g: Option<u32>| -> Option<f32> {
        let g = g?;
        deform::find(&dvert[i], g).map(|k| dvert[i].weights[k].1)
    };
    // (vertex, weight in A, weight in B) of the affected vertices.
    let mut affected: Vec<(usize, Option<f32>, Option<f32>)> = Vec::new();
    for i in 0..verts_num {
        let (a, b) = (find(i, Some(ga)), find(i, gb));
        let keep = match s.mix_set.as_str() {
            "A" => a.is_some(),
            "B" => b.is_some(),
            "OR" => a.is_some() || b.is_some(),
            "AND" => a.is_some() && b.is_some(),
            _ => true,
        };
        if keep {
            affected.push((i, a, b));
        }
    }
    if affected.is_empty() {
        return mesh;
    }
    let all = s.mix_set != "A" && s.mix_set != "B" && s.mix_set != "OR" && s.mix_set != "AND";
    let indices: Vec<usize> = affected.iter().map(|x| x.0).collect();
    let mut org_w = Vec::with_capacity(affected.len());
    let mut new_w = Vec::with_capacity(affected.len());
    for &(_, a, b) in &affected {
        let wa = a.unwrap_or(s.default_weight_a);
        let wa = if s.invert_vertex_group_a {
            1.0 - wa
        } else {
            wa
        };
        let wb = b.unwrap_or(s.default_weight_b);
        let wb = if s.invert_vertex_group_b {
            1.0 - wb
        } else {
            wb
        };
        org_w.push(wa);
        new_w.push(mix_weight(wa, wb, &s.mix_mode));
    }
    let mask = Mask {
        constant: s.mask_constant,
        vertex_group: &s.mask_vertex_group,
        invert_vertex_group: s.invert_mask_vertex_group,
    };
    let idx = (!all).then_some(indices.as_slice());
    do_mask(&mesh, idx, &mut org_w, &new_w, &mask);
    update_vg(
        mesh.deform_verts_mut(),
        ga,
        idx,
        &org_w,
        true,
        -f32::MAX,
        false,
        0.0,
        s.normalize,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_mesh::DeformVert;

    #[test]
    fn mix_modes() {
        assert_eq!(mix_weight(0.2, 0.5, "SET"), 0.5);
        assert!((mix_weight(0.2, 0.5, "ADD") - 0.7).abs() < 1e-6);
        assert!((mix_weight(0.2, 0.5, "SUB") + 0.3).abs() < 1e-6);
        assert!((mix_weight(0.2, 0.5, "MUL") - 0.1).abs() < 1e-6);
        assert!((mix_weight(0.2, 0.5, "DIV") - 0.4).abs() < 1e-6);
        assert_eq!(mix_weight(0.2, 0.0, "DIV"), 0.2 / ZEROFLOOR);
        assert!((mix_weight(0.2, 0.5, "DIF") - 0.3).abs() < 1e-6);
        assert!((mix_weight(0.2, 0.5, "AVG") - 0.35).abs() < 1e-6);
        assert_eq!(mix_weight(0.2, 0.5, "MIN"), 0.2);
        assert_eq!(mix_weight(0.2, 0.5, "MAX"), 0.5);
    }

    #[test]
    fn sets_select_vertices() {
        let mut m = Mesh::new("p");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[],
            &[vec![0, 1, 2, 3]],
            false,
        );
        m.vertex_group_names.push("A".into());
        m.vertex_group_names.push("B".into());
        let dv = m.deform_verts_mut();
        dv[0] = DeformVert {
            weights: smallvec::smallvec![(0, 0.2), (1, 0.6)],
        };
        dv[1] = DeformVert {
            weights: smallvec::smallvec![(0, 0.4)],
        };
        dv[2] = DeformVert {
            weights: smallvec::smallvec![(1, 0.3)],
        };
        let wa = |m: &Mesh| -> Vec<Option<f32>> {
            m.deform_verts()
                .unwrap()
                .iter()
                .map(|d| d.weights.iter().find(|x| x.0 == 0).map(|x| x.1))
                .collect()
        };
        let mut s = VertexWeightMixSettings {
            vertex_group_a: "A".into(),
            vertex_group_b: "B".into(),
            ..Default::default()
        };
        s.mix_mode = "ADD".into();
        s.mask_constant = 1.0;
        s.mix_set = "AND".into();
        let r = vertex_weight_mix(&s, m.clone());
        assert_eq!(wa(&r), [Some(0.8), Some(0.4), None, None]);
        s.mix_set = "OR".into();
        let r = vertex_weight_mix(&s, m.clone());
        assert_eq!(wa(&r)[2], Some(0.3), "vertex 2 joins A");
        assert_eq!(wa(&r)[3], None);
        s.mix_set = "ALL".into();
        s.default_weight_b = 0.1;
        let r = vertex_weight_mix(&s, m);
        assert_eq!(wa(&r)[3], Some(0.1));
    }
}
