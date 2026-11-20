// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like shadow maps (WP 5.1, `eevee/shadows.rs`): depth only, one layer of the
// `Depth32Float` shadow array per pass, reverse depth (1 at the light).

struct Layer {
    view_proj: mat4x4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> layer: Layer;
@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return layer.view_proj * (draw.model * vec4<f32>(position, 1.0));
}
