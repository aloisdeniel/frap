// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.vertex_colors.remove(layer)` (`rna_Mesh_vertex_color_remove` →
//! `BKE_attribute_remove`, `makesrna/intern/rna_mesh.cc`, `blenkernel/intern/attribute.cc`),
//! Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4 (cube with A, B, C on CORNER and F on
//! POINT):
//! - removes the attribute named like the layer from the called mesh; a name the mesh does not
//!   have changes nothing (a layer of another mesh: no error);
//! - an active or default colour attribute that named it moves to the colour attribute now at
//!   its index, clamped to the last one (`color_clamp_index`; none left: unset); unlike
//!   `color_attributes.remove`, which keeps both names;
//! - the active attribute keeps its index.
//!
//! Refused in Edit Mode (`RNA_EDITMODE_WRITE`, D-06).

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{arg_ptr, internal, mesh_of};
use super::mesh_set_sharp_from_angle::keep_active_index;

/// Colour attribute names (FLOAT_COLOR / BYTE_COLOR on POINT or CORNER) in attribute order.
fn color_names(m: &Mesh) -> Vec<SmolStr> {
    m.attributes()
        .iter()
        .filter(|a| {
            matches!(a.data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
                && matches!(a.domain, Domain::Point | Domain::Corner)
        })
        .map(|a| a.name.clone())
        .collect()
}

/// `mesh.vertex_colors` layer names (BYTE_COLOR on CORNER) in attribute order.
fn loop_color_names(m: &Mesh) -> Vec<SmolStr> {
    m.attributes()
        .iter()
        .filter(|a| a.domain == Domain::Corner && matches!(a.data, AttrData::ByteColor(_)))
        .map(|a| a.name.clone())
        .collect()
}

/// `BKE_attribute_remove` of the colour layer `name` with the active/default colour fallback
/// (module docs).
pub(crate) fn remove_loop_color(m: &mut Mesh, name: &str) {
    if m.attributes().get(name).is_none() {
        return;
    }
    let before = color_names(m);
    let index = |n: &Option<SmolStr>| {
        n.as_deref()
            .and_then(|n| before.iter().position(|x| x == n))
    };
    let (active, default) = (index(&m.color_active), index(&m.color_default));
    let was_active = m.color_active.as_deref() == Some(name);
    let was_default = m.color_default.as_deref() == Some(name);
    keep_active_index(m, |m| {
        m.attributes_mut().remove(name);
    });
    let after = color_names(m);
    let clamped =
        |i: Option<usize>| i.and_then(|i| after.get(i.min(after.len().checked_sub(1)?)).cloned());
    if was_active {
        m.color_active = clamped(active);
    }
    if was_default {
        m.color_default = clamped(default);
    }
}

#[rna_func(srna = "LoopColors", func = "remove")]
fn loop_colors_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let me = mesh_of(this)?;
    let target = arg_ptr(args, "layer")
        .cloned()
        .ok_or_else(|| internal("vertex_colors.remove(): no layer pointer"))?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("vertex_colors.remove(): the mesh was removed"))?;
    // The layer's name from its pointer (`…vertex_colors["A"]` or `…vertex_colors[i]`, of the
    // layer's own mesh); Blender removes that name from the called mesh.
    let source = match target.root {
        frap_rna::Root::Id(id) => id
            .typed::<frap_rna::MeshId>()
            .and_then(|m| main.meshes.get(m)),
        _ => None,
    }
    .unwrap_or(mesh);
    let name = match target.path.as_slice() {
        [.., Seg::Index(i)] => usize::try_from(*i)
            .ok()
            .and_then(|i| loop_color_names(source).get(i).cloned()),
        [.., Seg::Key(k)] => Some(k.clone()),
        _ => None,
    }
    .ok_or_else(|| internal("vertex_colors.remove(): cannot name the layer"))?;
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("vertex_colors.remove(): the mesh was removed"))?;
    remove_loop_color(mesh, &name);
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::funcs::loop_colors_new::new_layer;

    #[test]
    fn active_and_default_move_to_the_clamped_colour_index() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        for n in ["A", "B", "C"] {
            new_layer(&mut m, n, true);
        }
        m.add_attribute("F", "FLOAT_COLOR", "POINT").expect("F");
        m.color_active = Some("C".into());
        let names = |m: &Mesh| (m.color_active.clone(), m.color_default.clone());
        remove_loop_color(&mut m, "C");
        assert_eq!(names(&m), (Some("F".into()), Some("A".into())));
        remove_loop_color(&mut m, "A");
        assert_eq!(names(&m), (Some("F".into()), Some("B".into())));
        remove_loop_color(&mut m, "Z");
        remove_loop_color(&mut m, "B");
        assert_eq!(names(&m), (Some("F".into()), Some("F".into())));
        m.attributes_mut().remove("F");
        new_layer(&mut m, "G", true);
        m.color_active = Some("G".into());
        m.color_default = Some("G".into());
        remove_loop_color(&mut m, "G");
        assert_eq!(names(&m), (None, None));
    }
}
