// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.uv_layers.remove(layer)` (`rna_Mesh_uv_layers_remove` → `BKE_attribute_remove`,
//! `makesrna/intern/rna_mesh.cc`, `blenkernel/intern/attribute.cc`), Blender 5.2.4,
//! GPL-2.0-or-later: a name that is not an attribute of the mesh raises `RuntimeError: Error:
//! UV map 'X' not found`; otherwise the attribute and its `.pn.` pin layer are removed, and the
//! active and render UV maps that named it move to the UV map now at their clamped index (none
//! when no UV map is left). The active attribute keeps its index. Refused in Edit Mode
//! (`RNA_EDITMODE_WRITE`, D-06).

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{arg_ptr, internal, mesh_of, report_error};
use super::mesh_set_sharp_from_angle::keep_active_index;

/// UV map names (FLOAT2 on CORNER, not internal) in attribute order.
fn uv_names(m: &Mesh) -> Vec<SmolStr> {
    m.attributes()
        .iter()
        .filter(|a| {
            a.domain == Domain::Corner
                && matches!(a.data, AttrData::Float2(_))
                && !a.name.starts_with('.')
        })
        .map(|a| a.name.clone())
        .collect()
}

/// `uv_name_from_index(uv_clamp_index(index))`.
fn clamped(names: &[SmolStr], index: Option<usize>) -> Option<SmolStr> {
    let i = index?.min(names.len().checked_sub(1)?);
    names.get(i).cloned()
}

/// `BKE_attribute_remove` of a UV map `name` (module docs).
pub(crate) fn remove_uv_map(m: &mut Mesh, name: &str) {
    let before = uv_names(m);
    let active_index = m
        .uv_active
        .as_deref()
        .and_then(|n| before.iter().position(|x| x == n));
    let render_index = m
        .uv_active_render
        .as_deref()
        .and_then(|n| before.iter().position(|x| x == n));
    let was_active = m.uv_active.as_deref() == Some(name);
    let was_render = m.uv_active_render.as_deref() == Some(name);
    let is_uv = before.iter().any(|n| n == name);
    keep_active_index(m, |m| {
        let attrs = m.attributes_mut();
        attrs.remove(name);
        if is_uv {
            attrs.remove(&format!(".pn.{name}"));
        }
    });
    let after = uv_names(m);
    if was_active {
        m.uv_active = clamped(&after, active_index);
    }
    if was_render {
        m.uv_active_render = clamped(&after, render_index);
    }
}

#[rna_func(srna = "UVLoopLayers", func = "remove")]
fn u_v_loop_layers_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let me = mesh_of(this)?;
    let target = arg_ptr(args, "layer")
        .cloned()
        .ok_or_else(|| internal("uv_layers.remove(): no layer pointer"))?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("uv_layers.remove(): the mesh was removed"))?;
    let name = match target.path.as_slice() {
        [.., Seg::Index(i)] => usize::try_from(*i)
            .ok()
            .and_then(|i| uv_names(mesh).get(i).cloned()),
        [.., Seg::Key(k)] => Some(k.clone()),
        _ => None,
    }
    .ok_or_else(|| internal("uv_layers.remove(): cannot name the layer"))?;
    if mesh.attributes().get(&name).is_none() {
        return Err(report_error(format!("UV map '{name}' not found")));
    }
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("uv_layers.remove(): the mesh was removed"))?;
    remove_uv_map(mesh, &name);
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_and_render_move_to_the_clamped_index() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        m.add_uv_map("UVMap", true);
        m.add_uv_map("UVMap", true);
        m.uv_active = Some("UVMap.002".into());
        remove_uv_map(&mut m, "UVMap.002");
        assert_eq!(m.uv_active.as_deref(), Some("UVMap.001"));
        assert_eq!(m.uv_active_render.as_deref(), Some("UVMap"));
        remove_uv_map(&mut m, "UVMap");
        assert_eq!(m.uv_active_render.as_deref(), Some("UVMap.001"));
        remove_uv_map(&mut m, "UVMap.001");
        assert_eq!((m.uv_active.clone(), m.uv_active_render.clone()), (None, None));
    }
}
