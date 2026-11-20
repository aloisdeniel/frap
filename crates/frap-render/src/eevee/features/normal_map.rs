// SPDX-License-Identifier: GPL-3.0-or-later
//! Tangent-space normal maps (WP 5.5): an Image Texture (Non-Color) → Normal Map node →
//! Principled `Normal` (`LoweredMaterial::normal`, `Strength` as its scale). The mesh carries
//! the MikkTSpace tangents of the map's UV map (`frap-render-scene` builds them for every
//! object whose material has a normal map).
//!
//! Ported from blender/source/blender/gpu/shaders/material/gpu_shader_material_normal_map.glsl
//! (Blender 5.2.4, GPL-2.0-or-later): `color_to_normal_new_shading` then `node_normal_map`
//! (tangent negated on back faces, bitangent `w · cross(N, T)` negated for negatively scaled
//! objects — done by the vertex stage, which flips `w` —, strength applied in tangent space:
//! `xy · strength`, `z` mixed towards 1 by `saturate(strength)`), and `input_normal_original`
//! (the interpolated normal, facing the viewer). A zero tangent (`(0, 0, 0, 1)`, corners
//! MikkTSpace gave none) leaves the normal unchanged.
//!
//! `normal_map_perturb` is shared with the coat normal of `clearcoat.rs` (every registered
//! feature's WGSL is part of the shader).

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{MaterialParams, tex};

const WGSL: &str = r"
// `node_normal_map` (tangent space) of a normal-map texel `rgb` (WP 5.5).
fn normal_map_perturb(rgb: vec3<f32>, strength: f32, v: ptr<function, Inputs>,
                      N_in: vec3<f32>) -> vec3<f32> {
    var t = (*v).tangent;
    if (dot(t.xyz, t.xyz) == 0.0) {
        return N_in;
    }
    if (!(*v).front) {
        t = -t;
    }
    let T = normalize(t.xyz);
    let B = t.w * cross(N_in, T);
    var n = rgb * 2.0 - vec3<f32>(1.0);
    n = vec3<f32>(n.xy * strength, mix(1.0, n.z, saturate(strength)));
    return normalize(n.x * T + n.y * B + n.z * N_in);
}

// Normal map (WP 5.5): the shading normal from `material.tex[0].z`, strength `coat_tint.w`.
fn feature_normal_map(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    let t = material.tex[0].z;
    if (t < 0) {
        return;
    }
    (*s).N = normal_map_perturb(tex_sample(t, v).rgb, material.coat_tint.w, v, (*v).N);
}
";

/// Whether `m` has a normal map.
pub fn applies(m: &LoweredMaterial) -> bool {
    m.normal.is_some()
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    let Some(n) = &m.normal else { return };
    p.normal_scale = n.scale;
    p.set_texture(tex::NORMAL, &n.texture, scene);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "normal_map",
    order: 20,
    wgsl: WGSL,
    applies,
    pack,
};
