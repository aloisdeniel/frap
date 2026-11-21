// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_animation_pointer` export (WP 6.9, `export/pointer/**`): material (embedded node tree,
//! `NT` slots), light and camera property animation as `pointer` channels
//! (`/materials/{i}/…`, `/extensions/KHR_lights_punctual/lights/{i}/…`, `/cameras/{i}/…`),
//! ported from io_scene_gltf2 5.2.41 (`export_pointer_animation`), Apache-2.0, The
//! glTF-Blender-IO authors.
//!
//! The add-on exports them only with `export_animations`, `export_pointer_animation` and an
//! `export_animation_mode` of `SCENE` or `NLA_TRACKS` (`__init__.py`, `gltf_export_anim_pointer`;
//! the other modes ignore the option). Then:
//!
//! - [`registry`](crate::export::pointer::registry): which glTF material, light and camera each datablock became and which of its
//!   properties the gatherers read (`export_settings['KHR_animation_pointer']`), with
//!   [`material`](crate::export::pointer::material) for the node-tree sockets of a material;
//! - [`sample`](crate::export::pointer::sample): the value of each property at a frame and the add-on's conversions;
//! - [`bake`](crate::export::pointer::bake): the `SCENE` bake (the scene's range, one animation or one per datablock) and the
//!   `NLA_TRACKS` bake (one animation per NLA track group, merged by track name), and the
//!   writing of the channels.
//!
//! [`gather`](crate::export::pointer::gather) runs after the animation exporter (WP 6.7, `animation::gather`): the pointer
//! channels of the scene's animation, or of an NLA track's, go after the object channels of
//! the animation of that name, as the add-on gathers them; the other animations follow. It
//! also converts TRS and weights channels into pointers with `export_convert_animation_pointer`
//! (`exporter.py`, `gltf_trs_w_animation_pointer`).

pub mod bake;
pub mod material;
pub mod registry;
pub mod sample;

use frap_gltf::ext::khr_animation_pointer::KhrAnimationPointer;
use frap_gltf::json::{AnimationPath, Extension};
use frap_material::patterns::ior::GLTF_IOR;
use frap_spec_ops_io::export_scene::gltf::ExportAnimationMode;

use crate::export::ExportError;
use crate::export::gather::Gather;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_pointer_animation",
    "export_convert_animation_pointer",
];

/// Whether material, light and camera animation is exported as pointers
/// (`gltf_export_anim_pointer`).
pub fn enabled(g: &Gather) -> bool {
    let p = &g.settings.props;
    p.export_animations.value
        && p.export_pointer_animation.value
        && matches!(
            p.export_animation_mode.value,
            ExportAnimationMode::Scene | ExportAnimationMode::NlaTracks
        )
}

/// `gltf_trs_w_animation_pointer`: TRS and weights channels become pointers too.
pub fn converts_trs(g: &Gather) -> bool {
    enabled(g) && g.settings.props.export_convert_animation_pointer.value
}

/// Gathers the pointer animations (after `animation::gather`) and converts TRS / weights
/// channels when asked.
pub fn gather(g: &mut Gather) -> Result<(), ExportError> {
    if !enabled(g) {
        return Ok(());
    }
    let reg = registry::build(g);
    let (fps, spec) = bake::timing(g);
    let anims = match g.settings.props.export_animation_mode.value {
        ExportAnimationMode::Scene => bake::scene(g, &reg, fps, spec),
        _ => bake::tracks(g, &reg, fps, spec),
    };
    let mut pointers = bake::write(g, anims) > 0;
    if converts_trs(g) {
        pointers |= convert_trs(g) > 0;
    }
    keep_animated_empty_extensions(g);
    if check_iridescence(g) {
        pointers = g.doc.animations.iter().any(|a| {
            a.channels
                .iter()
                .any(|c| c.target.path == AnimationPath::Pointer)
        });
    }
    if pointers {
        g.use_extension(
            KhrAnimationPointer::NAME,
            KhrAnimationPointer::REQUIRED_BY_DEFAULT,
        );
    }
    Ok(())
}

/// `export.py` `__check_iridescence` with pointer animation (WP 7.7): a material keeps
/// `KHR_materials_iridescence` only when its factor is animated or not 0 and its maximum
/// thickness is animated or not 0 (static values from the material, as the gatherer wrote them
/// before frap's static exporter dropped the extension); otherwise the extension goes and so do
/// the channels pointing into it, and an animation left without channels. Returns whether
/// channels were removed. The add-on keeps the removed channels' sampler indices (they point
/// past the shortened sampler list, an invalid file); frap renumbers them (docs/deviations.md
/// §1).
pub fn check_iridescence(g: &mut Gather) -> bool {
    const NAME: &str = "KHR_materials_iridescence";
    let main = g.main;
    let settings = g.settings;
    let mut drop: Vec<usize> = Vec::new();
    for (i, m) in g.doc.materials.iter().enumerate() {
        if !m.extensions.contains(NAME) {
            continue;
        }
        let animated = |member: &str| {
            let want = format!("/materials/{i}/extensions/{NAME}/{member}");
            g.doc.animations.iter().any(|a| {
                a.channels.iter().any(|c| {
                    c.target.path == AnimationPath::Pointer
                        && KhrAnimationPointer::of(&c.target).as_deref() == Some(want.as_str())
                })
            })
        };
        let lowered = m
            .name
            .as_deref()
            .and_then(|n| main.materials.find(n))
            .and_then(|r| main.materials.get(r))
            .map(|mat| crate::export::materials::lower::lowered_for(main, mat, settings));
        let Some(ir) = lowered.as_ref().and_then(|lm| lm.ext.iridescence.as_ref()) else {
            continue;
        };
        if (!animated("iridescenceFactor") && ir.factor.factor == 0.0)
            || (!animated("iridescenceThicknessMaximum") && ir.thickness_max.factor == 0.0)
        {
            drop.push(i);
        }
    }
    if drop.is_empty() {
        return false;
    }
    let mut removed = false;
    for &i in &drop {
        g.doc.materials[i].extensions.remove(NAME);
        if let Some(c) = g
            .state::<crate::export::ext_order::CreatedEmpty>()
            .0
            .get_mut(&(i as frap_gltf::json::Index))
        {
            c.retain(|e| *e != NAME);
        }
        let prefix = format!("/materials/{i}/extensions/{NAME}/");
        for a in &mut g.doc.animations {
            let before = a.channels.len();
            a.channels.retain(|c| {
                !(c.target.path == AnimationPath::Pointer
                    && KhrAnimationPointer::of(&c.target).is_some_and(|p| p.starts_with(&prefix)))
            });
            if a.channels.len() == before {
                continue;
            }
            removed = true;
            // Keep the samplers still used, renumbered.
            let mut keep: Vec<Option<usize>> = vec![None; a.samplers.len()];
            let mut samplers = Vec::new();
            for c in &mut a.channels {
                let s = c.sampler as usize;
                let n = match keep.get(s).copied().flatten() {
                    Some(n) => n,
                    None => {
                        samplers.push(a.samplers[s].clone());
                        keep[s] = Some(samplers.len() - 1);
                        samplers.len() - 1
                    }
                };
                c.sampler = n as frap_gltf::json::Index;
            }
            a.samplers = samplers;
        }
    }
    g.doc.animations.retain(|a| !a.channels.is_empty());
    if !g.doc.materials.iter().any(|m| m.extensions.contains(NAME)) {
        g.doc.extensions_used.retain(|e| e != NAME);
        g.doc.extensions_required.retain(|e| e != NAME);
    }
    removed
}

/// `__gather_extensions`' order of the `KHR_materials_*` extensions.
const MATERIAL_EXTENSIONS: [&str; 10] = [
    "KHR_materials_clearcoat",
    "KHR_materials_transmission",
    "KHR_materials_emissive_strength",
    "KHR_materials_volume",
    "KHR_materials_specular",
    "KHR_materials_sheen",
    "KHR_materials_anisotropy",
    "KHR_materials_iridescence",
    "KHR_materials_dispersion",
    "KHR_materials_ior",
];

/// The (possibly empty) extension objects `__gather_extensions` creates for every material
/// that is not unlit (see `ext_order`, which makes the same approximation: every such
/// material counts as a Principled BSDF with unlinked Anisotropic and IOR sockets).
const ALWAYS_CREATED: [&str; 6] = [
    "KHR_materials_clearcoat",
    "KHR_materials_transmission",
    "KHR_materials_emissive_strength",
    "KHR_materials_specular",
    "KHR_materials_anisotropy",
    "KHR_materials_ior",
];

/// `__detect_animated_extensions`: for every pointer channel through an extension, the
/// innermost `KHR_*` token of its pointer.
pub fn animated_extensions(doc: &frap_gltf::Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in &doc.animations {
        for ch in &a.channels {
            if ch.target.path != AnimationPath::Pointer {
                continue;
            }
            let Some(p) = KhrAnimationPointer::of(&ch.target) else {
                continue;
            };
            if !p.contains("/KHR") {
                continue;
            }
            if let Some(ext) = p.split('/').rfind(|t| t.starts_with("KHR_"))
                && !out.iter().any(|e| e == ext)
            {
                out.push(ext.to_owned());
            }
        }
    }
    out
}

/// `__fix_json` / `__should_include_json_value` and `__check_ior`: an empty
/// `KHR_materials_*` object the add-on created is kept, in every material (the check is
/// global), when any pointer channel animates that extension. frap's material exporter never
/// writes the empty objects, so they are added back here.
///
/// `KHR_materials_ior` is then decided again, as `__check_ior` does after `__fix_json`: the
/// static exporter dropped it when no transmission, volume or specular object survived, but
/// `export_ior` had created `{"ior": v}` (the unlinked `IOR` socket's value, `ior` left out
/// at 1.5) beside the always-created transmission and specular objects. So a material that
/// now keeps one of those three gets `KHR_materials_ior` back with its static value, or as an
/// empty object when the IOR is 1.5 and animated.
pub fn keep_animated_empty_extensions(g: &mut Gather) {
    let animated = animated_extensions(&g.doc);
    if animated.is_empty() {
        return;
    }
    let created = g
        .state::<crate::export::ext_order::CreatedEmpty>()
        .0
        .clone();
    let main = g.main;
    let materials_by_name: std::collections::HashMap<&str, &frap_scene::id::Material> = main
        .materials
        .iter()
        .map(|(_, m)| (m.header.name.as_str(), m))
        .collect();
    let mut used: Vec<&'static str> = Vec::new();
    for (i, m) in g.doc.materials.iter_mut().enumerate() {
        if m.extensions.contains("KHR_materials_unlit") {
            continue;
        }
        let extra = created.get(&(i as frap_gltf::json::Index));
        let made = |e: &str| ALWAYS_CREATED.contains(&e) || extra.is_some_and(|c| c.contains(&e));
        let mut add: Vec<&'static str> = MATERIAL_EXTENSIONS
            .into_iter()
            .filter(|e| {
                *e != IOR && !m.extensions.contains(e) && made(e) && animated.iter().any(|a| a == e)
            })
            .collect();
        let mut ior_value: Option<serde_json::Value> = None;
        let needs_ior = [
            "KHR_materials_transmission",
            "KHR_materials_volume",
            "KHR_materials_specular",
        ]
        .iter()
        .any(|e| m.extensions.contains(e) || add.contains(e));
        if needs_ior
            && !m.extensions.contains(IOR)
            && made(IOR)
            && let Some(mat) = m.name.as_deref().and_then(|n| materials_by_name.get(n))
            && let Some(v) = material::ior(main, mat)
        {
            if v != GLTF_IOR {
                ior_value = Some(serde_json::json!({ "ior": crate::export::materials::num(v) }));
            } else if animated.iter().any(|a| a == IOR) {
                ior_value = Some(serde_json::json!({}));
            }
            if ior_value.is_some() {
                add.push(IOR);
            }
        }
        if add.is_empty() {
            continue;
        }
        // Rebuild in `__gather_extensions`' order, the other extensions after.
        let mut ext = frap_gltf::json::Extensions::default();
        for name in MATERIAL_EXTENSIONS {
            if let Some(v) = m.extensions.get_raw(name) {
                ext.insert_raw(name, v.clone());
            } else if name == IOR {
                if let Some(v) = ior_value.take() {
                    ext.insert_raw(name, v);
                }
            } else if add.contains(&name) {
                ext.insert_raw(name, serde_json::json!({}));
            }
        }
        for (k, v) in &m.extensions.0 {
            if !ext.contains(k) {
                ext.insert_raw(k.clone(), v.clone());
            }
        }
        m.extensions = ext;
        for e in add {
            if !used.contains(&e) {
                used.push(e);
            }
        }
    }
    for e in used {
        g.use_extension(e, false);
    }
}

const IOR: &str = "KHR_materials_ior";

/// `__traverse_property` with `gltf_trs_w_animation_pointer`: every `translation`, `rotation`,
/// `scale` and `weights` channel targets `/nodes/{node}/{path}` through a pointer instead.
/// Returns the number of channels converted.
pub fn convert_trs(g: &mut Gather) -> usize {
    let mut n = 0;
    for anim in &mut g.doc.animations {
        for ch in &mut anim.channels {
            let t = &mut ch.target;
            let path = match t.path {
                AnimationPath::Translation
                | AnimationPath::Rotation
                | AnimationPath::Scale
                | AnimationPath::Weights => t.path.as_str().to_owned(),
                _ => continue,
            };
            let Some(node) = t.node else { continue };
            let mut target = KhrAnimationPointer::target(format!("/nodes/{node}/{path}"));
            // Other extensions of the target are kept.
            for (k, v) in &t.extensions.0 {
                if k != KhrAnimationPointer::NAME {
                    target.extensions.0.insert(k.clone(), v.clone());
                }
            }
            target.extras = t.extras.take();
            *t = target;
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::bake::{PointerAnimation, PointerChannel};
    use super::*;
    use crate::export::ExportSettings;
    use frap_gltf::json::{Animation, AnimationSampler, Channel, ChannelTarget};

    fn with_gather(f: impl FnOnce(&mut Gather)) {
        let (main, win) = frap_scene::startup::factory();
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.export_pointer_animation.value = true;
        p.export_animation_mode.value = ExportAnimationMode::Scene;
        p.export_convert_animation_pointer.value = true;
        let s = ExportSettings::from_props(&p);
        let mut g = Gather::new(&main, &ctx, &s);
        f(&mut g);
    }

    fn channel(pointer: &str) -> PointerChannel {
        PointerChannel {
            pointer: pointer.into(),
            times: vec![0.0, 1.0],
            values: vec![vec![0.0], vec![1.0]],
            blend_material: None,
        }
    }

    fn node_anim(name: &str, node: u32, path: AnimationPath) -> Animation {
        Animation {
            name: Some(name.into()),
            channels: vec![Channel {
                extensions: Default::default(),
                extras: None,
                sampler: 0,
                target: ChannelTarget {
                    extensions: Default::default(),
                    extras: None,
                    node: Some(node),
                    path,
                },
            }],
            samplers: vec![AnimationSampler {
                extensions: Default::default(),
                extras: None,
                input: 0,
                interpolation: None,
                output: 1,
            }],
            ..Animation::default()
        }
    }

    /// `merge_tracks_perform` / the SCENE animation: channels join the document's animation of
    /// the merge name (after its own channels), duplicates are skipped; others are appended.
    #[test]
    fn write_merges_by_name() {
        with_gather(|g| {
            assert!(enabled(g) && converts_trs(g));
            g.doc
                .animations
                .push(node_anim("Walk", 0, AnimationPath::Translation));
            let n = bake::write(
                g,
                vec![
                    PointerAnimation {
                        name: "Mat".into(),
                        extras: None,
                        channels: vec![channel("/materials/0/alphaCutoff")],
                        merge: Some("Walk".into()),
                    },
                    PointerAnimation {
                        name: "Light".into(),
                        extras: None,
                        channels: vec![
                            channel("/materials/0/alphaCutoff"),
                            channel("/extensions/KHR_lights_punctual/lights/0/color"),
                        ],
                        merge: Some("Walk".into()),
                    },
                    PointerAnimation {
                        name: "Camera".into(),
                        extras: None,
                        channels: vec![channel("/cameras/0/perspective/zfar")],
                        merge: None,
                    },
                ],
            );
            assert_eq!(n, 3, "the duplicated pointer is skipped");
            assert_eq!(g.doc.animations.len(), 2);
            let walk = &g.doc.animations[0];
            assert_eq!(walk.channels.len(), 3);
            assert_eq!(walk.samplers.len(), 3);
            assert_eq!(walk.channels[1].sampler, 1);
            assert_eq!(
                KhrAnimationPointer::of(&walk.channels[2].target).as_deref(),
                Some("/extensions/KHR_lights_punctual/lights/0/color")
            );
            assert_eq!(g.doc.animations[1].name.as_deref(), Some("Camera"));
            assert!(!g.reports.is_empty(), "the duplicate warns");
            // Input accessors carry the double-precision min/max.
            let input = g.doc.animations[1].samplers[0].input;
            let a = g.buf.accessor(input).unwrap();
            assert_eq!(a.min.as_deref(), Some(&[0.0][..]));
            assert_eq!(a.max.as_deref(), Some(&[1.0][..]));
        });
    }

    /// `gltf_trs_w_animation_pointer`: TRS / weights channels target `/nodes/{n}/{path}`.
    #[test]
    fn trs_channels_become_pointers() {
        with_gather(|g| {
            g.doc
                .animations
                .push(node_anim("A", 3, AnimationPath::Rotation));
            g.doc
                .animations
                .push(node_anim("B", 1, AnimationPath::Weights));
            assert_eq!(convert_trs(g), 2);
            let t = &g.doc.animations[0].channels[0].target;
            assert_eq!(t.path, AnimationPath::Pointer);
            assert_eq!(t.node, None);
            assert_eq!(
                KhrAnimationPointer::of(t).as_deref(),
                Some("/nodes/3/rotation")
            );
            assert_eq!(
                KhrAnimationPointer::of(&g.doc.animations[1].channels[0].target).as_deref(),
                Some("/nodes/1/weights")
            );
            assert_eq!(convert_trs(g), 0, "pointers are left alone");
        });
    }

    #[test]
    fn animated_extensions_and_kept_empty_objects() {
        with_gather(|g| {
            g.doc.materials.push(frap_gltf::json::Material::default());
            let mut unlit = frap_gltf::json::Material::default();
            unlit
                .extensions
                .insert_raw("KHR_materials_unlit", serde_json::json!({}));
            g.doc.materials.push(unlit);
            let _ = bake::write(
                g,
                vec![PointerAnimation {
                    name: "Scene".into(),
                    extras: None,
                    channels: vec![
                        channel("/materials/0/extensions/KHR_materials_ior/ior"),
                        channel(
                            "/materials/1/extensions/KHR_materials_emissive_strength/emissiveStrength",
                        ),
                    ],
                    merge: None,
                }],
            );
            assert_eq!(
                animated_extensions(&g.doc),
                ["KHR_materials_ior", "KHR_materials_emissive_strength"]
            );
            keep_animated_empty_extensions(g);
            let names: Vec<&str> = g.doc.materials[0].extensions.names().collect();
            // IOR needs transmission, volume or specular beside it.
            assert_eq!(names, ["KHR_materials_emissive_strength"]);
            assert!(
                !g.doc.materials[1]
                    .extensions
                    .contains("KHR_materials_emissive_strength")
            );
            assert!(
                g.doc
                    .extensions_used
                    .iter()
                    .any(|e| e == "KHR_materials_emissive_strength")
            );
        });
    }

    #[test]
    fn gate_follows_the_animation_mode() {
        let (main, win) = frap_scene::startup::factory();
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        for (mode, on) in [
            (ExportAnimationMode::Actions, false),
            (ExportAnimationMode::ActiveActions, false),
            (ExportAnimationMode::Broadcast, false),
            (ExportAnimationMode::NlaTracks, true),
            (ExportAnimationMode::Scene, true),
        ] {
            let mut p = crate::export::settings::default_props();
            p.export_pointer_animation.value = true;
            p.export_animation_mode.value = mode;
            let s = ExportSettings::from_props(&p);
            let g = Gather::new(&main, &ctx, &s);
            assert_eq!(enabled(&g), on, "{mode:?}");
            assert!(!converts_trs(&g));
        }
        let mut p = crate::export::settings::default_props();
        p.export_pointer_animation.value = true;
        p.export_animation_mode.value = ExportAnimationMode::Scene;
        p.export_animations.value = false;
        let s = ExportSettings::from_props(&p);
        assert!(!enabled(&Gather::new(&main, &ctx, &s)));
    }
}
