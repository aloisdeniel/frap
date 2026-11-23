// SPDX-License-Identifier: GPL-3.0-or-later
//! VERTEX_WEIGHT_EDIT (`VertexWeightEditModifier`): maps the weights of `vertex_group`
//! (`default_weight` for the vertices outside it) through `falloff_type` (inverted with
//! `invert_falloff`), mixes the result in with the influence (`mask_constant`, scaled by
//! `mask_vertex_group`), and writes it back: `normalize` stretches it to [0, 1], `use_add` adds
//! the vertices at or above `add_threshold`, `use_remove` removes those at or below
//! `remove_threshold`.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weightvgedit.cc (Blender 5.2.4,
//! GPL-2.0-or-later); the shared helpers are in [`weightvg`].

pub(crate) mod weightvg;

use frap_rna::IdAny;
use frap_scene::generated::modifier_settings::VertexWeightEditSettings;

use self::weightvg::{Falloff, Mask, do_map, do_mask, falloff_rng, group_index, update_vg};
use crate::prelude::*;

/// `foreach_ID_link`: the mask texture and its mapping object.
fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::VertexWeightEdit(w) = s {
        v.extend(w.mask_texture);
        v.extend(w.mask_tex_map_object);
    }
    v
}

/// `is_disabled`: no vertex group.
fn disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::VertexWeightEdit(w) if w.vertex_group.is_empty() => {
            Some("VertexWeightEdit: no vertex group")
        }
        _ => None,
    }
}

#[modifier(
    VERTEX_WEIGHT_EDIT,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::VertexWeightEdit(s) = &md.settings else {
        return Ok(mesh);
    };
    if s.mask_texture.is_some() {
        return Err(weightvg::texture_unsupported("VERTEX_WEIGHT_EDIT"));
    }
    let name = cx.object().header.name.to_string();
    Ok(vertex_weight_edit(s, mesh, &name))
}

/// `modify_mesh` of VERTEX_WEIGHT_EDIT on the mesh of object `object_name`.
pub(crate) fn vertex_weight_edit(s: &VertexWeightEditSettings, mut mesh: Mesh, object_name: &str) -> Mesh {
    let verts_num = mesh.verts_num();
    if verts_num == 0 || mesh.vertex_group_names.is_empty() {
        return mesh;
    }
    let Some(g) = group_index(&mesh, &s.vertex_group) else {
        return mesh;
    };
    let has_mdef = mesh.deform_verts().is_some_and(|d| !d.is_empty());
    if !has_mdef && !s.use_add {
        return mesh;
    }
    mesh.deform_verts_mut();
    let dvert = mesh.deform_verts().unwrap_or(&[]);
    let mut org_w: Vec<f32> = dvert
        .iter()
        .map(|dv| {
            dv.weights
                .iter()
                .find(|x| x.0 == g)
                .map_or(s.default_weight, |x| x.1)
        })
        .collect();
    let mut new_w = org_w.clone();
    let falloff = Falloff::from_identifier(&s.falloff_type);
    if s.invert_falloff || falloff != Falloff::None {
        let mut rng = falloff_rng(falloff, object_name);
        do_map(&mut new_w, falloff, s.invert_falloff, rng.as_mut());
    }
    let mask = Mask {
        constant: s.mask_constant,
        vertex_group: &s.mask_vertex_group,
        invert_vertex_group: s.invert_mask_vertex_group,
    };
    do_mask(&mesh, None, &mut org_w, &new_w, &mask);
    update_vg(
        mesh.deform_verts_mut(),
        g,
        None,
        &org_w,
        s.use_add,
        s.add_threshold,
        s.use_remove,
        s.remove_threshold,
        s.normalize,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_mesh::DeformVert;

    fn plane_with_group() -> Mesh {
        let mut m = Mesh::new("p");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2, 3]],
            false,
        );
        m.vertex_group_names.push("G".into());
        let dv = m.deform_verts_mut();
        dv[0] = DeformVert {
            weights: smallvec::smallvec![(0, 0.2)],
        };
        dv[1] = DeformVert {
            weights: smallvec::smallvec![(0, 0.8)],
        };
        m
    }

    fn weights(m: &Mesh) -> Vec<Option<f32>> {
        m.deform_verts()
            .unwrap()
            .iter()
            .map(|d| d.weights.iter().find(|x| x.0 == 0).map(|x| x.1))
            .collect()
    }

    #[test]
    fn add_remove_and_falloff() {
        let mut s = VertexWeightEditSettings {
            vertex_group: "G".into(),
            ..Default::default()
        };
        s.falloff_type = "SHARP".into();
        s.default_weight = 0.9;
        s.use_add = true;
        s.add_threshold = 0.5;
        s.use_remove = true;
        s.remove_threshold = 0.1;
        let m = vertex_weight_edit(&s, plane_with_group(), "Plane");
        let w = weights(&m);
        assert_eq!(w[0], None, "0.2² = 0.04 is removed");
        assert!((w[1].unwrap() - 0.64).abs() < 1e-6);
        assert!((w[2].unwrap() - 0.81).abs() < 1e-6, "0.9² ≥ 0.5 is added");
    }

    #[test]
    fn influence_and_missing_group() {
        let mut s = VertexWeightEditSettings {
            vertex_group: "G".into(),
            ..Default::default()
        };
        s.invert_falloff = true;
        s.mask_constant = 0.5;
        let m = vertex_weight_edit(&s, plane_with_group(), "Plane");
        let w = weights(&m);
        // 0.2 → inverted 0.8, half influence → 0.5.
        assert!((w[0].unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(w[2], None);
        s.vertex_group = "Nope".into();
        let m = vertex_weight_edit(&s, plane_with_group(), "Plane");
        assert_eq!(weights(&m)[0], Some(0.2));
    }
}
