// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap_compat='SPEC'`: the add-on quirks the strict writer drops (WP 7.4).
//!
//! - **`extensionsRequired`.** io_scene_gltf2's `Extension` defaults to `required=True`
//!   (`io/com/gltf2_io_extensions.py`), so `KHR_texture_transform` (`texture_info.py`),
//!   `KHR_lights_punctual` (`nodes.py`) and the primitive's `KHR_materials_variants`
//!   (`primitives.py`) land in `extensionsRequired`, and a viewer without them refuses the
//!   file. Their specs make them optional (a viewer ignoring them still shows the model), as
//!   are every `KHR_materials_*`, `KHR_animation_pointer` and `KHR_node_visibility`; SPEC
//!   keeps them in `extensionsUsed` only. Extensions whose data a viewer cannot do without
//!   (Draco, meshopt without a fallback, `KHR_mesh_quantization`, a WebP or KTX2 texture
//!   without a fallback image, `EXT_mesh_gpu_instancing`) stay required.
//! - **Animation times.** The add-on writes `frame / (fps × fps_base)` seconds (and
//!   tangents per those seconds, `keyframes.py`); Blender's clock is
//!   `frame × fps_base / fps`. SPEC rescales every sampler of a scene with `fps_base ≠ 1`:
//!   times × fps_base², CUBICSPLINE tangents ÷ fps_base², and drops the exporter's fps_base
//!   WARNING. `KHR_animation_pointer` samplers stay as written (the pointer bake uses
//!   `(frame − slide) / fps` when it slides, which the written times no longer tell apart)
//!   and get a WARNING instead.
//! - **Skinned mesh nodes at the scene root** ([`skinned_roots`]). The add-on keeps a skinned
//!   mesh under its armature's node (Blender's usual rig: the mesh parented to the armature),
//!   which the Khronos validator reports (`NODE_SKINNED_MESH_NON_ROOT`: the specification
//!   ignores a skinned mesh node's transform and its parents', the joints place the mesh).
//!   SPEC moves such a node to the roots of the scenes that held it; a node with children
//!   first takes its former global transform, so its children stay in place.

use std::collections::{HashMap, HashSet};

use frap_gltf::accessor::{AccessorData, Components};
use frap_gltf::json::{AnimationPath, Index, Interpolation};
use frap_rna::{IdRef, SceneId};

use super::layout::{read, write_in_place};
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::node_map::NodeMap;

/// Whether the glTF spec lets a viewer ignore extension `name` (it is then never listed in
/// `extensionsRequired` under SPEC).
pub fn optional_by_spec(name: &str) -> bool {
    name.starts_with("KHR_materials_")
        || matches!(
            name,
            "KHR_texture_transform"
                | "KHR_lights_punctual"
                | "KHR_animation_pointer"
                | "KHR_node_visibility"
        )
}

/// Drops the optional extensions from `extensionsRequired`.
pub fn extensions_required(g: &mut Gather) {
    g.doc.extensions_required.retain(|n| !optional_by_spec(n));
}

/// The scene each glTF node was gathered for.
fn node_scenes(g: &mut Gather) -> HashMap<Index, IdRef<SceneId>> {
    let map = g.state::<NodeMap>();
    let mut out = HashMap::new();
    for (&(scene, _), &n) in &map.objects {
        out.insert(n, scene);
    }
    for (&(scene, _, _), &n) in &map.bones {
        out.insert(n, scene);
    }
    out
}

/// The node a channel animates, if any (`node`, or `/nodes/<i>/…` of a pointer).
fn channel_node(c: &frap_gltf::json::Channel) -> Option<Index> {
    c.target.node.or_else(|| {
        let p = c
            .target
            .extensions
            .get_raw("KHR_animation_pointer")?
            .get("pointer")?
            .as_str()?;
        p.strip_prefix("/nodes/")?.split('/').next()?.parse().ok()
    })
}

/// Rescales the animation times of scenes with `fps_base ≠ 1` to Blender's clock.
pub fn animation_times(g: &mut Gather) -> Result<(), ExportError> {
    if g.doc.animations.is_empty() {
        return Ok(());
    }
    let scenes = node_scenes(g);
    let ctx_scene = g.ctx.win.scene;
    // Per input / output accessor: the factor its samplers want (None: leave alone).
    let mut inputs: HashMap<Index, Option<f64>> = HashMap::new();
    let mut outputs: HashMap<Index, Option<f64>> = HashMap::new();
    let mut cubic: HashSet<Index> = HashSet::new();
    let mut corrected: HashSet<IdRef<SceneId>> = HashSet::new();
    let mut kept: HashSet<IdRef<SceneId>> = HashSet::new();
    for anim in &g.doc.animations {
        let scene = anim
            .channels
            .iter()
            .find_map(|c| channel_node(c).and_then(|n| scenes.get(&n).copied()))
            .unwrap_or(ctx_scene);
        let b = g
            .main
            .scenes
            .get(scene)
            .map_or(1.0, |s| f64::from(s.render.fps_base));
        if b == 1.0 {
            continue;
        }
        for (si, s) in anim.samplers.iter().enumerate() {
            let pointer = anim
                .channels
                .iter()
                .any(|c| c.sampler as usize == si && c.target.path == AnimationPath::Pointer);
            let want = if pointer {
                kept.insert(scene);
                None
            } else {
                corrected.insert(scene);
                Some(b * b)
            };
            // An accessor shared by samplers that disagree is left alone.
            for (map, a) in [(&mut inputs, s.input), (&mut outputs, s.output)] {
                let e = map.entry(a).or_insert(want);
                if *e != want {
                    *e = None;
                }
            }
            if s.interpolation == Some(Interpolation::CubicSpline) {
                cubic.insert(s.output);
            }
        }
    }
    for (&a, &k) in &inputs {
        let Some(k) = k else { continue };
        let d = read(g, a)?;
        let t: Vec<f32> = d
            .to_f32()
            .iter()
            .map(|&t| (f64::from(t) * k) as f32)
            .collect();
        let new = AccessorData {
            components: Components::F32(t),
            ..d
        };
        write_in_place(g, a, &new, true)?;
    }
    for &a in &cubic {
        let (Some(Some(k)), Some(&input)) = (
            outputs.get(&a),
            // Any sampler's input gives the key count (equal for every sampler of `a`).
            g.doc
                .animations
                .iter()
                .flat_map(|an| &an.samplers)
                .find(|s| s.output == a)
                .map(|s| &s.input),
        ) else {
            continue;
        };
        let keys = g.buf.accessor(input).map_or(0, |acc| acc.count) as usize;
        let d = read(g, a)?;
        let n = d.type_.components();
        let elems = d.count();
        if keys == 0 || elems % (3 * keys) != 0 {
            continue;
        }
        // Per key: in-tangents, values, out-tangents, each `group` elements.
        let group = elems / (3 * keys);
        let Components::F32(mut v) = d.components.clone() else {
            // Normalized integer tangents (weights) keep their encoding range: leave them.
            continue;
        };
        for (e, chunk) in v.chunks_mut(n).enumerate() {
            if (e / group) % 3 != 1 {
                for x in chunk {
                    *x = (f64::from(*x) / k) as f32;
                }
            }
        }
        let new = AccessorData {
            components: Components::F32(v),
            ..d
        };
        write_in_place(g, a, &new, false)?;
    }
    for &scene in &corrected {
        if kept.contains(&scene) {
            continue;
        }
        if let Some(name) = g.main.scenes.get(scene).map(|s| s.header.name.to_string()) {
            let prefix = format!("glTF export: animation times of scene '{name}'");
            g.reports.retain(|r| !r.message.starts_with(&prefix));
        }
    }
    for &scene in &kept {
        if let Some(name) = g.main.scenes.get(scene).map(|s| s.header.name.to_string()) {
            g.warn(format!(
                "glTF export: frap_compat='SPEC' keeps the add-on's KHR_animation_pointer times of scene '{name}' (fps_base ≠ 1)"
            ));
        }
    }
    Ok(())
}

/// Moves every node with a skinned mesh that has a parent to the roots of the scenes that
/// reach it (see the module docs).
pub fn skinned_roots(g: &mut Gather) {
    use frap_math::glam::{DMat4, DQuat, DVec3};
    let doc = &mut g.doc;
    let n = doc.nodes.len();
    let mut parent: Vec<Option<usize>> = vec![None; n];
    for (i, node) in doc.nodes.iter().enumerate() {
        for &c in &node.children {
            if let Some(p) = parent.get_mut(c as usize) {
                *p = Some(i);
            }
        }
    }
    let local = |node: &frap_gltf::json::Node| -> DMat4 {
        if let Some(m) = node.matrix {
            return DMat4::from_cols_array(&m);
        }
        let t = node.translation.unwrap_or([0.0; 3]);
        let r = node.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let s = node.scale.unwrap_or([1.0; 3]);
        DMat4::from_scale_rotation_translation(
            DVec3::from_array(s),
            DQuat::from_array(r),
            DVec3::from_array(t),
        )
    };
    let root_of = |parent: &[Option<usize>], mut i: usize| {
        while let Some(p) = parent[i] {
            i = p;
        }
        i
    };
    for i in 0..n {
        let Some(p) = parent[i] else { continue };
        if doc.nodes[i].skin.is_none() || doc.nodes[i].mesh.is_none() {
            continue;
        }
        let root = root_of(&parent, p) as Index;
        if !doc.nodes[i].children.is_empty() {
            let mut world = local(&doc.nodes[i]);
            let mut q = Some(p);
            while let Some(j) = q {
                world = local(&doc.nodes[j]) * world;
                q = parent[j];
            }
            let node = &mut doc.nodes[i];
            if node.matrix.is_some() {
                node.matrix = Some(world.to_cols_array());
            } else {
                let (s, r, t) = world.to_scale_rotation_translation();
                node.translation = (t != DVec3::ZERO).then(|| t.to_array());
                node.rotation = (r != DQuat::IDENTITY).then(|| r.to_array());
                node.scale = (s != DVec3::ONE).then(|| s.to_array());
            }
        }
        doc.nodes[p].children.retain(|&c| c as usize != i);
        parent[i] = None;
        for sc in &mut doc.scenes {
            if sc.nodes.contains(&root) && !sc.nodes.contains(&(i as Index)) {
                sc.nodes.push(i as Index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_extensions() {
        for n in [
            "KHR_texture_transform",
            "KHR_lights_punctual",
            "KHR_materials_variants",
            "KHR_materials_clearcoat",
            "KHR_animation_pointer",
            "KHR_node_visibility",
        ] {
            assert!(optional_by_spec(n), "{n}");
        }
        for n in [
            "KHR_draco_mesh_compression",
            "EXT_meshopt_compression",
            "KHR_meshopt_compression",
            "KHR_mesh_quantization",
            "EXT_texture_webp",
            "KHR_texture_basisu",
            "EXT_mesh_gpu_instancing",
        ] {
            assert!(!optional_by_spec(n), "{n}");
        }
    }
}
