// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF morph-target import (WP 6.8): shape keys from `targets` (POSITION deltas, sparse
//! accessors included), named from `extras.targetNames`, the first primitive's accessor name
//! or `target_<i>`; the node's (else the mesh's) default `weights` as key values; a node whose
//! weights are animated gets a Blender mesh of its own (Blender's shape keys live on the
//! mesh).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors)
//! `blender/imp/blender_gltf.py` (`pre_compute`: `shapekey_names`, `weight_animation`,
//! `dispatch_animation_pointer`), `blender/com/gltf2_blender_utils.py` (`find_unused_name`),
//! `blender/imp/node.py` (`set_morph_weights`) and `blender/imp/mesh.py` (the shape-key
//! branch of `do_primitives`, in `import/mesh.rs`).
//!
//! The shape-key F-curves of a weights animation are made by the animation importer
//! (`import/animation/**`, WP 6.7) from [`shapekey_names`].

use std::collections::HashSet;

use frap_gltf::Document;
use frap_gltf::json::AnimationPath;
use frap_rna::{IdRef, MeshId};
use frap_scene::{ChangeLog, Main};

/// Whether morph targets are imported (WP 6.8 sets it).
pub(crate) const IMPLEMENTED: bool = true;

/// `find_unused_name(haystack, desired)`: `desired` cut to 63 UTF-8 bytes, `.001`, `.002`, …
/// appended until it is not in `haystack`.
pub(crate) fn find_unused_name(haystack: &HashSet<String>, desired: &str) -> String {
    let mut stem: String = desired.chars().take(63).collect();
    let mut suffix = String::new();
    let mut cntr = 1;
    loop {
        let name = format!("{stem}{suffix}");
        if name.len() > 63 {
            stem.pop();
            continue;
        }
        if !haystack.contains(&name) {
            return name;
        }
        suffix = format!(".{cntr:03}");
        cntr += 1;
    }
}

/// `str()` of a JSON value as Python prints the decoded object.
fn py_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(true) => "True".into(),
        serde_json::Value::Bool(false) => "False".into(),
        serde_json::Value::Null => "None".into(),
        v => v.to_string(),
    }
}

/// `pymesh.shapekey_names`: per morph target of the first primitive that has targets, the
/// shape key it becomes (`None` when no primitive morphs POSITION for it).
pub(crate) fn shapekey_names(doc: &Document, mesh_idx: usize) -> Vec<Option<String>> {
    let Some(mesh) = doc.meshes.get(mesh_idx) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut used: HashSet<String> = HashSet::from(["Basis".to_owned()]);
    let target_names = mesh
        .extras
        .as_ref()
        .and_then(|e| serde_json::from_str::<serde_json::Value>(e.0.get()).ok())
        .and_then(|v| v.get("targetNames").cloned());
    let Some(prim) = mesh.primitives.iter().find(|p| !p.targets.is_empty()) else {
        return names;
    };
    for sk in 0..prim.targets.len() {
        let morphs_position = mesh.primitives.iter().any(|p| {
            p.targets
                .get(sk)
                .is_some_and(|t| t.contains_key("POSITION"))
        });
        if !morphs_position {
            names.push(None);
            continue;
        }
        let mut name = target_names
            .as_ref()
            .and_then(|t| t.get(sk))
            .map(py_str)
            .filter(|s| !s.is_empty());
        if name.is_none() {
            name = mesh
                .primitives
                .first()
                .and_then(|p| p.targets.get(sk))
                .and_then(|t| t.get("POSITION"))
                .and_then(|&a| doc.accessors.get(a as usize))
                .and_then(|a| a.name.clone());
        }
        let name = find_unused_name(&used, &name.unwrap_or_else(|| format!("target_{sk}")));
        used.insert(name.clone());
        names.push(Some(name));
    }
    names
}

/// `node.weight_animation`: the glTF nodes whose morph weights an animation drives (a
/// `weights` channel, or a `KHR_animation_pointer` to `/nodes/<i>/weights` or to the weights
/// of the node's mesh).
pub(crate) fn weight_animated_nodes(doc: &Document) -> HashSet<usize> {
    let mut out = HashSet::new();
    let pointer_used = doc
        .extensions_used
        .iter()
        .any(|e| e == "KHR_animation_pointer");
    let mut animated_meshes: HashSet<usize> = HashSet::new();
    for a in &doc.animations {
        for c in &a.channels {
            match c.target.node {
                Some(n) => {
                    if c.target.path == AnimationPath::Weights {
                        out.insert(n as usize);
                    }
                }
                None => {
                    if c.target.path != AnimationPath::Pointer {
                        continue;
                    }
                    let Some(p) = c
                        .target
                        .extensions
                        .0
                        .get("KHR_animation_pointer")
                        .and_then(|e| e.get("pointer"))
                        .and_then(|p| p.as_str())
                    else {
                        continue;
                    };
                    let tab: Vec<&str> = p.split('/').collect();
                    if tab.len() >= 4 && tab[3] == "weights" {
                        match (tab[1], tab[2].parse::<usize>()) {
                            ("nodes", Ok(i)) => {
                                out.insert(i);
                            }
                            ("meshes", Ok(i)) => {
                                animated_meshes.insert(i);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if pointer_used {
        for (i, n) in doc.nodes.iter().enumerate() {
            if n.mesh
                .is_some_and(|m| animated_meshes.contains(&(m as usize)))
            {
                out.insert(i);
            }
        }
    }
    out
}

/// `set_morph_weights`: the node's (else the mesh's) weights as the values of the mesh's
/// shape keys, widening a key's slider range to its weight.
pub(crate) fn set_morph_weights(
    doc: &Document,
    main: &mut Main,
    log: &mut ChangeLog,
    node_idx: usize,
    mesh_idx: usize,
    me: IdRef<MeshId>,
) {
    let node = &doc.nodes[node_idx];
    let pymesh = &doc.meshes[mesh_idx];
    let weights: &[f64] = if !node.weights.is_empty() {
        &node.weights
    } else {
        &pymesh.weights
    };
    let names = shapekey_names(doc, mesh_idx);
    let Some(k) = main.meshes.get(me).and_then(|m| m.shape_keys) else {
        return;
    };
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return;
    };
    for (i, &w) in weights.iter().enumerate() {
        let Some(Some(name)) = names.get(i) else {
            continue;
        };
        let Some(kb) = key.key_blocks.iter_mut().find(|b| b.name == *name) else {
            continue;
        };
        let w = w as f32;
        if w < kb.slider_min {
            kb.set_slider_min(w);
        }
        if w > kb.slider_max {
            kb.set_slider_max(w);
        }
        kb.set_value(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_names_get_suffixes_and_fit_63_bytes() {
        let mut used: HashSet<String> = HashSet::from(["Basis".to_owned(), "A".to_owned()]);
        assert_eq!(find_unused_name(&used, "B"), "B");
        assert_eq!(find_unused_name(&used, "A"), "A.001");
        used.insert("A.001".into());
        assert_eq!(find_unused_name(&used, "A"), "A.002");
        assert_eq!(find_unused_name(&used, "Basis"), "Basis.001");
        let long = "é".repeat(40);
        let n = find_unused_name(&used, &long);
        assert!(n.len() <= 63 && n.chars().all(|c| c == 'é'));
    }

    #[test]
    fn python_str_of_target_names() {
        assert_eq!(py_str(&serde_json::json!("x")), "x");
        assert_eq!(py_str(&serde_json::json!(3)), "3");
        assert_eq!(py_str(&serde_json::json!(true)), "True");
    }
}
