// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex colours (WP 5.5): a Color Attribute (or GEOMETRY Attribute) node feeding `Base
//! Color` and/or `Alpha` (`LoweredMaterial::vertex_color`), directly or multiplied with a
//! constant (`node_vertex_color`: the attribute's colour and alpha). The attribute multiplies
//! the base colour (and alpha), as glTF's `COLOR_0` multiplies `baseColorFactor`: the lowered
//! factor is the constant the attribute was multiplied with (1 when it feeds the socket
//! directly). The attribute is bound as `Inputs.color` ([`MaterialParams::color_attribute`];
//! an empty name reads the mesh's default colour attribute, `GPU_attribute_default_color`).
//! Byte colours arrive decoded to linear (`frap-render-scene` mesh extraction), as Blender's
//! draw cache uploads them.

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::MaterialParams;

/// `material.ints.z` bit: `Base Color` reads the attribute.
pub const MODE_COLOR: u32 = 1;
/// `material.ints.z` bit: `Alpha` reads the attribute's alpha.
pub const MODE_ALPHA: u32 = 2;

const WGSL: &str = r"
// Vertex colours (WP 5.5): `material.ints.z` = colour (1), alpha (2).
fn feature_vertex_color(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    let mode = material.ints.z;
    let c = (*v).color;
    if ((mode & 1u) != 0u) {
        (*s).base_color *= c.rgb;
    }
    if ((mode & 2u) != 0u) {
        (*s).alpha *= c.a;
    }
}
";

/// Whether `m` reads a colour attribute.
pub fn applies(m: &LoweredMaterial) -> bool {
    m.vertex_color.as_ref().is_some_and(|v| v.color || v.alpha)
}

/// The `material.ints.z` mode and the bound attribute of `m`'s colour attribute use: one
/// attribute is bound, so the alpha follows only when it reads the colour's attribute.
pub fn mode(m: &LoweredMaterial) -> (u32, Option<smol_str::SmolStr>) {
    let Some(vc) = &m.vertex_color else {
        return (0, None);
    };
    let mut mode = 0;
    if vc.color {
        mode |= MODE_COLOR;
    }
    if vc.alpha && (!vc.color || vc.alpha_attribute == vc.attribute) {
        mode |= MODE_ALPHA;
    }
    let attribute = if vc.color {
        vc.attribute.clone()
    } else {
        vc.alpha_attribute.clone()
    };
    (mode, attribute)
}

fn pack(m: &LoweredMaterial, _scene: &RenderScene, p: &mut MaterialParams) {
    (p.vertex_color, p.color_attribute) = mode(m);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "vertex_color",
    order: 10,
    wgsl: WGSL,
    applies,
    pack,
};
