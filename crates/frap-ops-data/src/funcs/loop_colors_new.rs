// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.vertex_colors.new(name="Col", do_init=True)` (`rna_Mesh_vertex_color_new` →
//! `ED_mesh_color_add(mesh, name, false, do_init)`, `makesrna/intern/rna_mesh.cc`,
//! `editors/mesh/mesh_data.cc`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4:
//! - appends a BYTE_COLOR attribute on CORNER named `name` made unique (`Col`, `Col.001`, …;
//!   an empty name gives `Attribute`), white;
//! - `do_init` copies the active colour attribute into it, read as byte colours on corners
//!   (a POINT attribute gives each corner its vertex's colour; float colours are sRGB-encoded);
//! - the new layer becomes the active and the default colour only when those are unset;
//! - returns the layer (`mesh.vertex_colors[name]`, a `MeshLoopColorLayer`).
//!
//! Refused in Edit Mode (`RNA_EDITMODE_WRITE`, D-06).

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{arg_bool, internal, mesh_of};

/// `linearrgb_to_srgb` then `unit_float_to_uchar_clamp` (one channel).
fn encode_channel(c: f32) -> u8 {
    let s = if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    unit_float_to_uchar_clamp(s)
}

/// `unit_float_to_uchar_clamp`.
fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// The colour attribute `name` read as byte colours on corners
/// (`attributes.lookup<ColorGeometry4b>(name, Corner)`); `None` when it is not a colour
/// attribute on POINT or CORNER.
fn corner_bytes(m: &Mesh, name: &str) -> Option<Vec<[u8; 4]>> {
    let a = m.attributes().get(name)?;
    let per_corner = |i: usize| match a.domain {
        Domain::Corner => Some(i),
        Domain::Point => m.corner_verts().get(i).map(|&v| v as usize),
        _ => None,
    };
    (0..m.corners_num())
        .map(|c| {
            let i = per_corner(c)?;
            Some(match &a.data {
                AttrData::ByteColor(v) => v[i],
                AttrData::FloatColor(v) => {
                    let f = v[i];
                    [
                        encode_channel(f[0]),
                        encode_channel(f[1]),
                        encode_channel(f[2]),
                        unit_float_to_uchar_clamp(f[3]),
                    ]
                }
                _ => return None,
            })
        })
        .collect()
}

/// `ED_mesh_color_add` + the active/default fallback of `rna_Mesh_vertex_color_new`; the new
/// layer's name.
pub(crate) fn new_layer(m: &mut Mesh, requested: &str, do_init: bool) -> SmolStr {
    let name = m.unique_attribute_name(requested);
    let n = m.corners_num();
    let data = do_init
        .then(|| m.color_active.as_deref().and_then(|a| corner_bytes(m, a)))
        .flatten()
        .unwrap_or_else(|| vec![[255; 4]; n]);
    m.attributes_mut().insert(Attribute {
        name: name.clone(),
        domain: Domain::Corner,
        data: AttrData::ByteColor(Arc::new(data)),
    });
    m.color_active.get_or_insert_with(|| name.clone());
    m.color_default.get_or_insert_with(|| name.clone());
    name
}

#[rna_func(srna = "LoopColors", func = "new")]
fn loop_colors_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let me = mesh_of(this)?;
    let requested = match args.get("name") {
        Some(v) => v.as_str().unwrap_or_default().to_owned(),
        None => "Col".to_owned(),
    };
    let do_init = arg_bool(args, "do_init", true);
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("vertex_colors.new(): the mesh was removed"))?;
    let name = new_layer(mesh, &requested, do_init);
    let srna = &frap_spec_rna::MESH_LOOP_COLOR_LAYER;
    Ok(Value::Ptr(Box::new(
        RnaPtr::id(me.any(), &frap_spec_rna::MESH)
            .child(Seg::Prop("vertex_colors"), srna)
            .child(Seg::Key(name), srna),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_then_copies_the_active_colour() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        assert_eq!(new_layer(&mut m, "Col", true), "Col");
        assert_eq!(m.color_active.as_deref(), Some("Col"));
        let col = |m: &Mesh, n: &str| match &m.attributes().get(n).expect("layer").data {
            AttrData::ByteColor(v) => v.to_vec(),
            _ => panic!("not a byte colour"),
        };
        assert!(col(&m, "Col").iter().all(|c| *c == [255; 4]));
        m.add_attribute("P", "FLOAT_COLOR", "POINT").expect("P");
        if let Some(a) = m.attributes_mut().get_mut("P")
            && let AttrData::FloatColor(v) = &mut a.data
        {
            Arc::make_mut(v).fill([0.2, 0.3, 0.4, 1.0]);
        }
        m.color_active = Some("P".into());
        assert_eq!(new_layer(&mut m, "Col", true), "Col.001");
        assert_eq!(col(&m, "Col.001")[0], [124, 149, 170, 255]);
        assert_eq!(m.color_active.as_deref(), Some("P"));
        assert_eq!(new_layer(&mut m, "Z", false), "Z");
        assert_eq!(col(&m, "Z")[0], [255; 4]);
    }
}
