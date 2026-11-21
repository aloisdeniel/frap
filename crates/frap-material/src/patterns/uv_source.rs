// SPDX-License-Identifier: GPL-3.0-or-later
//! The UV map a texture reads (`texCoord`), ARCHITECTURE §16.2.
//!
//! A per-texture pattern: `LowerCx::tex_ref` reads the node in front of the UV chain with
//! [`uv_of`] and `RaiseCx::texture` adds the UV Map node with [`raise_uv_map`]; the
//! material-wide entry registered here has nothing to do on its own.
//!
//! Lowering ports the UV part of `__gather_texture_transform_and_tex_coord`
//! (`exp/material/texture_info.py`, glTF-Blender-IO 5.2.41, Apache-2.0): a UV Map node with a
//! map name (`Fixed`, [`UvSource::Named`]), an Attribute node of type GEOMETRY with a name
//! (`Attribute`, [`UvSource::Attribute`]: a UV map of that name, or a generic attribute the
//! exporter writes as an extra `TEXCOORD_n`), else the active render UV map (`Render`); the
//! exporter resolves the name to a `TEXCOORD_n` per mesh (`get_final_material`). frap adds a WARNING when the UVs come
//! from a node it does not understand (the add-on silently uses the render map).
//!
//! Raising ports the UV Map part of `texture()` (`imp/texture.py`): `TEXCOORD_n` → a UV Map
//! node naming `UVMap` / `UVMap.00n` (the importer's names), created for `n > 0` or when a
//! Mapping node or the wrapping chain needs an explicit input.

use frap_nodes::{Node, SocketAddr};
use frap_rna::{Report, Value};
use smol_str::SmolStr;

use crate::graph::{Src, unrecognised};
use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, UvSource};

/// The UV source of Image Texture `tex` given what feeds the start of its UV chain.
pub fn uv_of(tex: &Node, src: Src<'_>, diags: &mut Vec<Report>) -> UvSource {
    match src {
        Src::Const(_) | Src::Missing => UvSource::ActiveRender,
        Src::Linked(n, _) if n.bl_idname == "ShaderNodeUVMap" => match n.prop_str("uv_map") {
            "" => UvSource::ActiveRender,
            name => UvSource::Named(name.into()),
        },
        Src::Linked(n, _)
            if n.bl_idname == "ShaderNodeAttribute"
                && n.prop_str("attribute_type") == "GEOMETRY" =>
        {
            match n.prop_str("attribute_name") {
                "" => UvSource::ActiveRender,
                name => UvSource::Attribute(name.into()),
            }
        }
        Src::Linked(n, o)
            if n.bl_idname == "ShaderNodeTexCoord" && n.outputs[o].identifier == "UV" =>
        {
            UvSource::ActiveRender
        }
        Src::Linked(n, _) => {
            unrecognised(
                diags,
                format!(
                    "Image Texture \"{}\": texture coordinates from \"{}\" are not supported; using the active UV map",
                    tex.name, n.name
                ),
            );
            UvSource::ActiveRender
        }
    }
}

/// The UV Map node of the importer's `texture()` (none for `TEXCOORD_0` unless `needed`).
pub(crate) fn raise_uv_map(
    cx: &mut RaiseCx,
    uv: &UvSource,
    needed: bool,
    uv_socket: SocketAddr,
    x: f32,
    y: f32,
) {
    let uv_name: Option<SmolStr> = match uv {
        UvSource::Named(n) => Some(n.clone()),
        UvSource::Index(0) => needed.then(|| "UVMap".into()),
        UvSource::Index(i) => Some(format!("UVMap.{i:03}").into()),
        _ => needed.then(SmolStr::default),
    };
    if let Some(name) = uv_name {
        let node = cx.add_node("ShaderNodeUVMap", [x - 160.0, y - 70.0]);
        cx.set_prop(node, "uv_map", Value::Str(name));
        let o = cx.output(node, "UV");
        cx.link(uv_socket, o);
    }
}

/// Per-texture pattern: nothing material-wide to lower (see the module documentation).
fn lower(_: &LowerCx, _: &mut LoweredMaterial) {}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "uv_source",
    order: 3,
    lower,
    raise: None,
};
