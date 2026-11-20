// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_animation_mode='SCENE'`: the scene's frame range baked as the viewport plays it
//! (actions, NLA and parents as they are), one animation per object with
//! `export_anim_scene_split_object` (the default), else one animation named after the scene.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/animation/scene_animation.py` (Apache-2.0,
//! The glTF-Blender-IO authors): `gather_scene_animations`. Material, light and camera
//! channels (`export_pointer_animation`) are `KHR_animation_pointer`'s (WP 6.9).

use std::collections::HashMap;

use indexmap::IndexMap;

use super::exp::{AKey, Exp};
use super::{GAnim, GChannel, Source};

/// `gather_scene_animations`.
pub(crate) fn gather(exp: &mut Exp<'_>) -> Vec<GAnim> {
    let (start, end) = (exp.frame_start, exp.frame_end);
    let mut animations = Vec::new();
    let mut total: Vec<GChannel> = Vec::new();
    for (_, ob, _) in exp.exported_objects() {
        let Some(o) = exp.world.ob(ob).cloned() else {
            continue;
        };
        let key = AKey::Obj(ob);
        let mut r = IndexMap::new();
        r.insert(key.clone(), (start, end));
        exp.ranges.insert(ob, r);
        if exp.s.slide_to_zero && start > 0 {
            exp.add_slide(start, ob, key.clone());
        }
        if o.object_type() != "ARMATURE" {
            total.extend(exp.object_channels(ob, &key, &None, &HashMap::new()));
            if exp.s.morph_anim && o.object_type() == "MESH" && exp.world.key(ob).is_some() {
                total.extend(exp.sk_channel(ob, &key, &None));
            }
        } else {
            total.extend(exp.armature_channels(ob, &key, &None, &HashMap::new(), None));
        }
        if exp.s.split_object && !total.is_empty() {
            animations.push(GAnim {
                name: o.header.name.to_string(),
                channels: std::mem::take(&mut total),
                sources: vec![super::Source::Object(ob)],
            });
        }
    }
    if !exp.s.split_object && !total.is_empty() {
        let name = exp
            .world
            .main
            .scenes
            .get(exp.scene)
            .map(|s| s.header.name.to_string())
            .unwrap_or_default();
        animations.push(GAnim {
            name,
            channels: total,
            sources: vec![Source::Scene(exp.scene)],
        });
    }
    animations
}
