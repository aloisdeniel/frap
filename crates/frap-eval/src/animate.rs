// SPDX-License-Identifier: GPL-3.0-or-later
//! Applying animation to originals through `Host::set_path` at evaluation boundaries (§7.3,
//! §18; WP 6.4).
//!
//! Blender applies animation only when the depsgraph evaluates after a time change: after
//! `scene.frame_current = 6` an animated `location` keeps its old value until the next operator,
//! `view_layer.update()`, `scene.frame_set()` or `evaluated_depsgraph_get()` (verified). The
//! `frame_current` setter therefore only marks time dirty (`Host::mark_time_dirty`), and each
//! of those boundaries calls [`apply_pending_animation`] with that flag. Render and export call
//! [`apply_animation_at`] on a COW clone of the state (`Host::snapshot_at`), never on the
//! session's originals.
//!
//! At a session boundary only the datablocks of the window's depsgraph are animated
//! ([`depsgraph_roots`]), as Blender flushes only the active depsgraph's IDs; render and export
//! clones animate every datablock ([`apply_animation_at`], D-128).
//!
//! Each animated datablock (every ID with `AnimData`, and the embedded node trees of materials
//! and worlds, addressed through `Root::Embedded`) is evaluated with
//! `frap_anim::eval::evaluate_anim_refined` (NLA, then the assigned action slot), the NLA's
//! channel structs refined from the data ([`refined_struct`]: a modifier's or a node's real
//! type), and every channel is written through `Host::set_path` with its ID-relative path, as
//! Blender's `animsys_write_orig_anim_rna` flushes the evaluated values to the originals of the
//! active depsgraph. A channel whose path does not resolve is skipped (Blender disables such an
//! F-curve and logs it).
//!
//! The module also holds the read side the keying functions share ([`with_path`]): walking an
//! ID-relative RNA path from a datablock through the bindings and the registered get hooks.

use frap_anim::eval::props::PathSeg;
use frap_anim::eval::{ActionLookup, evaluate_anim_refined};
use frap_anim::{Action, AnimData};
use std::collections::HashSet;

use frap_rna::{
    ActionId, EmbeddedSlot, Field, IdAny, IdCode, IdRef, Item, PropDef, Rna, RnaError,
    RnaErrorKind, RnaPtr, Root, SceneId, Seg, StructDef, Value,
};
use frap_scene::{Host, HostRead, Main, find_hook};

/// What an evaluation boundary wrote.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnimationApplied {
    /// Animated channels written into originals through `Host::set_path`.
    pub channels: usize,
}

/// An evaluation boundary (operator pipeline step 4, `view_layer.update()`,
/// `scene.frame_set()`, `context.evaluated_depsgraph_get()` / `evaluated_get()`, a change of
/// `window.scene`): when time is dirty, writes the animation of the context scene's
/// `frame_current` into the originals of the datablocks the window's depsgraph evaluates
/// ([`depsgraph_roots`]; Blender flushes only the active depsgraph's IDs).
/// The caller clears its time-dirty flag after an `Ok`.
pub fn apply_pending_animation(
    host: &mut dyn Host,
    time_dirty: bool,
) -> Result<AnimationApplied, RnaError> {
    apply_pending_animation_tagged(host, time_dirty, &[])
}

/// [`apply_pending_animation`] with the datablocks an RNA edit tagged for an animation update
/// since the last boundary ([`tags_animation_on_write`], [`tags_animation_on_call`]): even when
/// time is not dirty, Blender's depsgraph re-evaluates the animation of a tagged ID (and of
/// every user of a tagged action, directly or through an NLA strip) at the current frame and
/// flushes it to the originals (`ID_RECALC_ANIMATION`, verified on 5.2.4: after
/// `keyframe_points[1].co.y = 20`, `view_layer.update()` overwrites a manual `location.x`).
///
/// The window depsgraph's invisible nodes ([`DepsgraphNodes::invisible`]) are not animated: a
/// time change or a tag reaching one is remembered (`Main::invisible_pending`), and the first
/// boundary that finds it visible again applies its animation at the current frame (M5+M6
/// stabilization round 6; see `frap_scene::window::InvisiblePending`).
pub fn apply_pending_animation_tagged(
    host: &mut dyn Host,
    time_dirty: bool,
    tagged: &[Root],
) -> Result<AnimationApplied, RnaError> {
    if !time_dirty && tagged.is_empty() && host.main().invisible_pending.0.is_empty() {
        return Ok(AnimationApplied::default());
    }
    let (scene, view_layer) = {
        let win = host.win();
        (win.scene, win.view_layer.clone())
    };
    let frame = host.main().scenes.get(scene).map_or(0, |s| s.frame_current);
    let nodes = depsgraph_nodes(host.main(), scene, &view_layer);
    let tagged: HashSet<Root> = tagged.iter().copied().collect();
    let is_tagged = |root: Root, adt: &AnimData| {
        let action_tagged =
            |a: Option<IdRef<ActionId>>| a.is_some_and(|a| tagged.contains(&Root::Id(a.any())));
        tagged.contains(&root)
            || action_tagged(adt.action)
            || adt
                .nla_tracks
                .iter()
                .flat_map(|t| &t.strips)
                .any(|s| action_tagged(s.action))
    };
    // The invisible nodes keep (or gain) a pending update; those visible again are animated.
    let (woken, pending) = {
        let main = host.main();
        let mut woken = HashSet::new();
        let mut pending = Vec::new();
        for r in main.invisible_pending.of(scene, &view_layer) {
            if nodes.visible.contains(&r) {
                woken.insert(r);
            } else if nodes.invisible.contains(&r) {
                pending.push(r);
            }
        }
        for &r in &nodes.invisible {
            if let Some(adt) = anim_data(main, r)
                && (time_dirty || is_tagged(r, adt))
                && !pending.contains(&r)
            {
                pending.push(r);
            }
        }
        (woken, pending)
    };
    host.main_mut()
        .0
        .invisible_pending
        .set(scene, &view_layer, pending);
    let graph = &nodes.visible;
    if time_dirty {
        return apply_animation_filtered(host, frame as f32, &|root, _| graph.contains(&root));
    }
    if tagged.is_empty() && woken.is_empty() {
        return Ok(AnimationApplied::default());
    }
    apply_animation_filtered(host, frame as f32, &|root, adt| {
        graph.contains(&root) && (woken.contains(&root) || is_tagged(root, adt))
    })
}

/// `scene.frame_set()`'s `BKE_scene_graph_update_for_newframe` for each view layer of
/// `scene`: writes the animation at `frame` of the datablocks of its **active** depsgraphs
/// ([`depsgraph_roots`]) into the originals: the window's, and those the window showed before
/// (`Main::shown_depsgraphs`; Blender keeps a depsgraph active once shown). A scene the window
/// never showed writes nothing. The invisible nodes of those depsgraphs get a pending update
/// (`Main::invisible_pending`).
pub fn apply_scene_animation_at(
    host: &mut dyn Host,
    scene: IdRef<SceneId>,
    frame: f32,
) -> Result<AnimationApplied, RnaError> {
    let mut graph = HashSet::new();
    let mut pending = Vec::new();
    {
        let main = host.main();
        let win = host.win();
        for vl in main.scenes.get(scene).iter().flat_map(|s| &s.view_layers) {
            let active = (scene == win.scene && vl.name == win.view_layer)
                || main.shown_depsgraphs.contains(scene, &vl.name);
            if active {
                let nodes = depsgraph_nodes(main, scene, &vl.name);
                let mut p: Vec<Root> = main
                    .invisible_pending
                    .of(scene, &vl.name)
                    .into_iter()
                    .filter(|r| nodes.invisible.contains(r))
                    .collect();
                for &r in &nodes.invisible {
                    if anim_data(main, r).is_some() && !p.contains(&r) {
                        p.push(r);
                    }
                }
                pending.push((vl.name.clone(), p));
                graph.extend(nodes.visible);
            }
        }
    }
    for (vl, p) in pending {
        host.main_mut().0.invisible_pending.set(scene, &vl, p);
    }
    if graph.is_empty() {
        return Ok(AnimationApplied::default());
    }
    apply_animation_filtered(host, frame, &|root, _| graph.contains(&root))
}

// Ported from blender/source/blender/depsgraph/intern/builder/deg_builder_nodes.cc
// (`build_view_layer`, `need_pull_base_into_graph`, `build_object`, `build_scene_camera`,
// `build_world`, `modifier_walk`; Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4 with
// `frame_set`: an animated object that is a modifier operand, a parent or the scene camera is
// animated though it is in no collection of the scene; one with `hide_viewport` (or in a
// collection with `hide_viewport`, or an excluded one) is not, while `hide_set` and a layer
// collection's `hide_viewport` keep it; an orphan object, material, camera or world is not
// animated; a scene the window does not show is not animated by another scene's `frame_set`.
/// The datablocks the viewport depsgraph of `scene`'s view layer `view_layer` (its first one
/// when there is none of that name) evaluates, so animates: the scene, its world and camera,
/// the objects of the view layer's enabled bases ([`frap_scene::id::layers::BaseVisibility`]'s
/// `enabled_viewport`), and everything they reference, transitively (object data, shape keys,
/// materials, parents, object operands of viewport-enabled modifiers, armatures, node groups,
/// images, textures), with the embedded node tree of each material and world. Actions are not
/// roots (they carry no animation of their own). The objects the depsgraph builds invisible
/// are left out ([`depsgraph_nodes`]).
pub fn depsgraph_roots(main: &Main, scene: IdRef<SceneId>, view_layer: &str) -> HashSet<Root> {
    depsgraph_nodes(main, scene, view_layer).visible
}

/// The datablocks of one viewport depsgraph ([`depsgraph_nodes`]).
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct DepsgraphNodes {
    /// The datablocks the depsgraph evaluates ([`depsgraph_roots`]).
    pub visible: HashSet<Root>,
    /// The objects it holds as invisible nodes: reached only through an object operand of a
    /// viewport-disabled modifier (`show_viewport` off), or through such an object (its
    /// parent, the operands of its own modifiers). Not animated; their animation update stays
    /// pending until they become visible (`frap_scene::window::InvisiblePending`).
    pub invisible: HashSet<Root>,
}

/// [`depsgraph_roots`] and the invisible nodes of the same depsgraph.
/// `DepsgraphNodeBuilder::modifier_walk` builds an object operand of a modifier disabled for
/// the viewport with `is_visible = false`, and `build_object` passes that flag on to the
/// objects it references; IDs of other types (object data, shape keys, materials, textures,
/// node groups) are evaluated as usual (verified on 5.2.4: a BOOLEAN operand and a MIRROR
/// `mirror_object` of disabled modifiers, and their parents, keep a manual location after
/// `frame_set`; the operand's shape key and material, and the texture of its own DISPLACE
/// modifier, are animated).
pub fn depsgraph_nodes(main: &Main, scene: IdRef<SceneId>, view_layer: &str) -> DepsgraphNodes {
    let mut out = DepsgraphNodes::default();
    let Some(sc) = main.scenes.get(scene) else {
        return out;
    };
    // (id, built visible)
    let mut queue: Vec<(IdAny, bool)> = Vec::new();
    out.visible.insert(Root::Id(scene.any()));
    queue.extend(sc.world.map(|w| (w.any(), true)));
    queue.extend(sc.camera.map(|c| (c.any(), true)));
    if let Some(vl) = sc
        .view_layers
        .iter()
        .find(|v| v.name == view_layer)
        .or_else(|| sc.view_layers.first())
    {
        let vis = frap_scene::id::layers::visibility(sc, vl, &main.collections, &main.objects);
        queue.extend(
            vl.bases
                .iter()
                .filter(|b| vis.get(&b.object).is_some_and(|v| v.enabled_viewport))
                .map(|b| (b.object.any(), true)),
        );
    }
    // The IDs traversed visibly, and invisibly (a later visible visit traverses one again).
    let mut seen_visible: HashSet<IdAny> = HashSet::new();
    let mut seen_invisible: HashSet<IdAny> = HashSet::new();
    while let Some((id, visible)) = queue.pop() {
        if matches!(id.code, IdCode::AC | IdCode::LI | IdCode::SC) || main.header(id).is_none() {
            continue;
        }
        let root = Root::Id(id);
        let first = if visible {
            seen_visible.insert(id)
        } else {
            !seen_visible.contains(&id) && seen_invisible.insert(id)
        };
        if !first {
            continue;
        }
        if visible || id.code != IdCode::OB {
            out.visible.insert(root);
            if matches!(id.code, IdCode::MA | IdCode::WO) {
                out.visible.insert(Root::Embedded {
                    owner: id,
                    slot: EmbeddedSlot::NodeTree,
                });
            }
        }
        let start = queue.len();
        main.for_each_ref_of(id, &mut |x| queue.push((x, visible)));
        if visible && let Some(ob) = id.typed().and_then(|o| main.objects.get(o)) {
            for m in ob.modifiers.iter().filter(|m| !m.show_viewport) {
                for x in m.settings.id_refs().into_iter().filter_map(|(_, x)| x) {
                    // One reference per modifier slot: the same object reached otherwise
                    // (as the parent) stays visible.
                    if x.code == IdCode::OB
                        && let Some(q) = queue[start..].iter_mut().find(|q| q.0 == x && q.1)
                    {
                        q.1 = false;
                    }
                }
            }
        }
    }
    out.invisible = seen_invisible
        .into_iter()
        .filter(|id| id.code == IdCode::OB && !seen_visible.contains(id))
        .map(Root::Id)
        .collect();
    out
}

// Ported from blender/source/blender/makesrna/intern/{rna_fcurve,rna_animation,rna_nla,
// rna_action}.cc (Blender 5.2.4, GPL-2.0-or-later): the properties whose RNA update callback
// tags the owner ID (and so the users of an action) with `ID_RECALC_ANIMATION`
// (`rna_tag_animation_update`, `rna_Keyframe_update`, `rna_FCurve_update_data/_eval`,
// `rna_FModifier_update`, `rna_AnimData_update`, `rna_AnimData_dependency_update`,
// `rna_AnimData_action_slot_update`, `rna_NlaStrip_update/_transform_update/
// _dependency_update`, `rna_Action_tag_animupdate`). Properties with no update callback, or a
// notifier-only one (selection, `lock`, `hide`, `color`, names, `data_path`, the Action's own
// frame range), do not re-apply animation (verified on 5.2.4).
/// Whether assigning `prop` of a struct of type `srna` tags the owner for an animation update.
pub fn tags_animation_on_write(srna: &'static StructDef, prop: &str) -> bool {
    if srna.is_a("Keyframe") {
        !prop.starts_with("select")
    } else if srna.is_a("FCurve") {
        matches!(prop, "extrapolation" | "mute" | "auto_smoothing")
    } else if srna.is_a("FModifier") {
        !matches!(prop, "name" | "active" | "show_expanded")
    } else if srna.is_a("FModifierEnvelopeControlPoint") {
        true
    } else if srna.is_a("AnimData") {
        prop != "use_pin"
    } else if srna.is_a("NlaStrip") {
        !matches!(prop, "name" | "active" | "select")
    } else if srna.is_a("NlaTrack") {
        matches!(prop, "is_solo" | "mute")
    } else if srna.is_a("ActionLayer") {
        matches!(prop, "influence" | "mix_mode")
    } else {
        false
    }
}

// Ported from the same files: the data-API functions that tag animation
// (`rna_FKeyframe_points_*`, `rna_FCurve_update_data_ex`, `rna_ActionStrip_key_insert`,
// `rna_Action_slots_remove`, `rna_Action_layers_remove`, `rna_ActionStrips_remove`,
// `rna_Channelbags_remove`, `rna_NlaTrack_new/remove`, `rna_NlaStrip_new/remove`,
// `rna_FModifierEnvelope_points_add/remove`). `bpy_struct.keyframe_insert/delete` (a C
// method, not an RNA function) and `fcurves.new` / `modifiers.new` tag nothing (verified).
/// Whether calling the RNA function `func` of a struct of type `srna` tags the owner for an
/// animation update.
pub fn tags_animation_on_call(srna: &'static StructDef, func: &str) -> bool {
    match srna.identifier {
        "FCurveKeyframePoints" => matches!(
            func,
            "insert" | "add" | "remove" | "clear" | "sort" | "deduplicate" | "handles_recalc"
        ),
        "ActionSlots" | "ActionLayers" | "ActionStrips" | "ActionChannelbags" => func == "remove",
        "NlaTracks" | "NlaStrips" => matches!(func, "new" | "remove"),
        "FModifierEnvelopeControlPoints" => matches!(func, "add" | "remove"),
        _ if srna.is_a("FCurve") => func == "update",
        _ if srna.is_a("ActionStrip") => func == "key_insert",
        _ => false,
    }
}

/// `bpy.data.actions` as the evaluator's action lookup.
pub struct MainActions<'a>(pub &'a Main);

impl ActionLookup for MainActions<'_> {
    fn action(&self, r: IdRef<ActionId>) -> Option<&Action> {
        self.0.actions.get(r)
    }
}

/// Every datablock that may carry animation, in `bpy.data` order; the embedded node tree of a
/// material or world follows its owner.
pub fn animated_roots(main: &Main) -> Vec<Root> {
    let mut out = Vec::new();
    for id in main.all_ids() {
        out.push(Root::Id(id));
        if matches!(id.code, IdCode::MA | IdCode::WO) {
            out.push(Root::Embedded {
                owner: id,
                slot: EmbeddedSlot::NodeTree,
            });
        }
    }
    out
}

/// The `AnimData` of `root`, if it has one (`BKE_animdata_from_id`; the types frap animates,
/// as the `ID.animation_data` hook serves them).
pub fn anim_data(main: &Main, root: Root) -> Option<&AnimData> {
    fn typed<K: frap_rna::IdKind>(a: frap_rna::IdAny) -> Option<IdRef<K>> {
        a.typed()
    }
    let adt = match root {
        Root::Id(a) => match a.code {
            IdCode::OB => &main.objects.get(typed(a)?)?.animation_data,
            IdCode::MA => &main.materials.get(typed(a)?)?.animation_data,
            IdCode::CA => &main.cameras.get(typed(a)?)?.animation_data,
            IdCode::LA => &main.lights.get(typed(a)?)?.animation_data,
            IdCode::WO => &main.worlds.get(typed(a)?)?.animation_data,
            IdCode::IM => &main.images.get(typed(a)?)?.animation_data,
            IdCode::AR => &main.armatures.get(typed(a)?)?.animation_data,
            IdCode::SC => &main.scenes.get(typed(a)?)?.animation_data,
            IdCode::KE => &main.shape_keys.get(typed(a)?)?.animation_data,
            IdCode::NT => &main.node_groups.get(typed(a)?)?.animation_data,
            IdCode::TE => &main.textures.get(typed(a)?)?.animation_data,
            IdCode::PT => &main.pointclouds.get(typed(a)?)?.animation_data,
            _ => return None,
        },
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => &main.materials.get(typed(owner)?)?.node_tree.animation_data,
            IdCode::WO => &main.worlds.get(typed(owner)?)?.node_tree.animation_data,
            _ => return None,
        },
        _ => return None,
    };
    adt.as_ref()
}

/// Writes the animation of every animated ID of `bpy.data` at `frame` into `host`'s originals:
/// render and export clones (`Host::snapshot_at`). Session boundaries use the window's
/// depsgraph instead ([`apply_pending_animation_tagged`], [`apply_scene_animation_at`]).
pub fn apply_animation_at(host: &mut dyn Host, frame: f32) -> Result<AnimationApplied, RnaError> {
    apply_animation_filtered(host, frame, &|_, _| true)
}

/// [`apply_animation_at`] for the animated datablocks `filter` accepts.
fn apply_animation_filtered(
    host: &mut dyn Host,
    frame: f32,
    filter: &dyn Fn(Root, &AnimData) -> bool,
) -> Result<AnimationApplied, RnaError> {
    let mut work = Vec::new();
    {
        let read: &dyn HostRead = host;
        let main = read.main();
        for root in animated_roots(main) {
            let Some(adt) = anim_data(main, root) else {
                continue;
            };
            if adt.action.is_none() && adt.nla_tracks.is_empty() {
                continue;
            }
            if !filter(root, adt) {
                continue;
            }
            let srna = root_struct(read, root);
            let refine = |prefix: &[PathSeg], base: &'static StructDef| {
                refined_struct(read, root, prefix).filter(|s| s.is_a(base.identifier))
            };
            let channels =
                evaluate_anim_refined(adt, &MainActions(main), frame, srna, Some(&refine));
            if !channels.is_empty() {
                let channels = blender_write_order(read, root, adt, main, channels);
                work.push((root, channels));
            }
        }
    }
    let mut applied = AnimationApplied::default();
    for (root, channels) in work {
        for c in channels {
            // A range callback that reads the data: clamp before the write, with the data as it
            // is when this channel is written (earlier channels of this frame included).
            let value = clamp_to_data_range(&*host, root, &c);
            if host
                .set_path(root, &c.data_path, c.array_index, Value::Float(value))
                .is_ok()
            {
                applied.channels += 1;
            }
        }
    }
    Ok(applied)
}

// Ported from blender/source/blender/animrig/intern/evaluation.cc (`evaluate_keyframe_data`,
// `evaluate_action`, `blend_layer_results`, `apply_evaluation_result`), Blender 5.2.4,
// GPL-2.0-or-later: a layered action (no NLA) collects its channels in an `EvaluationResult`, a
// `blender::Map<PropIdentifier, AnimatedProperty>` reserved for the channelbag's F-curve count,
// and writes them in the **map's slot order**, not in F-curve order. The order matters where one
// property's range depends on another's value: `ShapeKey.value` is clamped to the sliders in
// force when it is written (`rna_ShapeKey_value_range`, `rna_ShapeKey_value_set`), so a value
// above the previous `slider_max` survives only when `slider_max` comes first (verified on
// 5.2.4 with `value` and `slider_max` keyed together: blocks `K`, `A`, `G`, `Key` keep 2.5,
// `B`, `F`, `Smile` are cut to the old 1.0, as the map order below predicts; with a third
// channel `slider_min` the map has 8 slots and `A` is cut). The NLA writes its channels in its
// own order (`nladata_flush_channels`), which `frap_anim::eval::nla` keeps.
/// `channels` (the evaluation of `adt`'s active action at one frame) in the order Blender's
/// `apply_evaluation_result` writes them; unchanged when the NLA evaluates.
fn blender_write_order(
    host: &dyn HostRead,
    root: Root,
    adt: &AnimData,
    main: &Main,
    channels: Vec<frap_anim::eval::AnimChannel>,
) -> Vec<frap_anim::eval::AnimChannel> {
    if !adt.nla_tracks.is_empty() && adt.use_nla {
        return channels;
    }
    let Some(action) = adt.action.and_then(|a| main.actions.get(a)) else {
        return channels;
    };
    let resolves = |path: &str, index: i32| {
        let Some(segs) = parse_path(path) else {
            return false;
        };
        read_path(host, root, &segs).is_some_and(|(_, prop, _)| match prop.array {
            frap_rna::ArrayShape::Scalar => true,
            _ => usize::try_from(index).is_ok_and(|i| i < prop.array_len()),
        })
    };
    let mut result: Option<blender_map::Map> = None;
    for layer in &action.layers {
        if layer.influence <= 0.0 {
            continue;
        }
        let Some(frap_anim::ActionStrip::Keyframe(strip)) = layer.strips.first() else {
            continue;
        };
        let Some(bag) = strip
            .channelbags
            .iter()
            .find(|c| c.slot_handle == adt.action_slot_handle)
        else {
            continue;
        };
        // `evaluate_keyframe_data`: reserved for every F-curve, filled with the evaluatable
        // ones whose path resolves, in F-curve order.
        let mut layer_map = blender_map::Map::default();
        layer_map.reserve(bag.fcurves.len());
        for fc in &bag.fcurves {
            if fc.data_path.is_empty() || fc.mute || fc.is_empty() {
                continue;
            }
            if resolves(&fc.data_path, fc.array_index) {
                layer_map.add_overwrite(fc.data_path.clone(), fc.array_index);
            }
        }
        if layer_map.is_empty() {
            continue;
        }
        match result.as_mut() {
            // The first layer with results is moved in as it is.
            None => result = Some(layer_map),
            // `blend_layer_results`: new channels are stored in the layer's map order.
            Some(r) => {
                for (path, index) in layer_map.keys() {
                    r.add_overwrite(path.clone(), *index);
                }
            }
        }
    }
    let Some(map) = result else {
        return channels;
    };
    let mut rest = channels;
    let mut out = Vec::with_capacity(rest.len());
    for (path, index) in map.keys() {
        if let Some(i) = rest
            .iter()
            .position(|c| c.array_index == *index && c.data_path == *path)
        {
            out.push(rest.remove(i));
        }
    }
    // Channels the map does not hold (their path does not resolve) fail at the write anyway.
    out.extend(rest);
    out
}

/// A port of the parts of `blender::Map` (blender/source/blender/blenlib/BLI_map.hh,
/// BLI_hash.hh, BLI_hash_tables.hh, BLI_probing_strategies.hh; Blender 5.2.4,
/// GPL-2.0-or-later) that decide its iteration order for `Map<PropIdentifier,
/// AnimatedProperty>`: `hash_string` (djb2 with `+`, over signed `char`s),
/// `get_default_hash(rna_path, array_index)` (`hash ^ 19349669 * index`), the
/// `PythonProbingStrategy` (`perturb >>= 5; hash = 5 * hash + 1 + perturb`), a maximum load
/// factor of 1/2, no inline slots (the key and value take more than 100 bytes), growth by
/// `realloc_and_reinsert` (old slots reinserted in slot order) and iteration in slot order.
mod blender_map {
    use smol_str::SmolStr;

    /// `hash_string`
    fn hash_string(s: &str) -> u64 {
        let mut h: u64 = 5381;
        for &b in s.as_bytes() {
            // `char` is signed on the platforms Blender is released for (x86-64, Apple arm64).
            h = h.wrapping_mul(33).wrapping_add(b as i8 as i64 as u64);
        }
        h
    }

    /// `PropIdentifier::hash`
    pub(super) fn hash(path: &str, index: i32) -> u64 {
        hash_string(path) ^ 19_349_669u64.wrapping_mul(index as i64 as u64)
    }

    /// `power_of_2_max`
    fn power_of_2_max(n: u64) -> u64 {
        if n <= 1 { n } else { n.next_power_of_two() }
    }

    type Key = (SmolStr, i32);

    /// The slots of a map (keys only: the order is all that matters here).
    #[derive(Default)]
    pub(super) struct Map {
        slots: Vec<Option<(u64, Key)>>,
        usable: usize,
        occupied: usize,
    }

    impl Map {
        fn len(&self) -> usize {
            self.slots.iter().filter(|s| s.is_some()).count()
        }

        pub(super) fn is_empty(&self) -> bool {
            self.len() == 0
        }

        /// `reserve`
        pub(super) fn reserve(&mut self, n: usize) {
            if self.usable < n {
                self.realloc_and_reinsert(n);
            }
        }

        /// `realloc_and_reinsert` with `LoadFactor(1, 2)` and no inline buffer.
        fn realloc_and_reinsert(&mut self, min_usable: usize) {
            let min_usable = min_usable.max(self.len() * 2) as u64;
            let total = power_of_2_max(min_usable * 2).max(1);
            let old = std::mem::replace(&mut self.slots, vec![None; total as usize]);
            self.usable = (total / 2) as usize;
            self.occupied = 0;
            for (h, k) in old.into_iter().flatten() {
                self.insert_new(h, k);
            }
        }

        fn insert_new(&mut self, h: u64, k: Key) {
            let mask = self.slots.len() as u64 - 1;
            let (mut probe, mut perturb) = (h, h);
            loop {
                let i = (probe & mask) as usize;
                if self.slots[i].is_none() {
                    self.slots[i] = Some((h, k));
                    self.occupied += 1;
                    return;
                }
                perturb >>= 5;
                probe = probe.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb);
            }
        }

        /// `add_overwrite`: a key already present keeps its slot.
        pub(super) fn add_overwrite(&mut self, path: SmolStr, index: i32) {
            if self.occupied >= self.usable {
                self.realloc_and_reinsert(self.len() + 1);
            }
            let h = hash(&path, index);
            let mask = self.slots.len() as u64 - 1;
            let (mut probe, mut perturb) = (h, h);
            loop {
                let i = (probe & mask) as usize;
                match &self.slots[i] {
                    None => {
                        self.slots[i] = Some((h, (path, index)));
                        self.occupied += 1;
                        return;
                    }
                    Some((sh, (p, x))) if *sh == h && *p == path && *x == index => return,
                    Some(_) => {}
                }
                perturb >>= 5;
                probe = probe.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb);
            }
        }

        /// The keys in slot order (`items()`).
        pub(super) fn keys(&self) -> impl Iterator<Item = &Key> {
            self.slots.iter().flatten().map(|(_, k)| k)
        }
    }
}

// Ported from blender/source/blender/makesrna/intern/{rna_nodetree,rna_fcurve,rna_modifier,
// rna_constraint,rna_color}.cc (`rna_Node_width_range`, `rna_Node_height_range`,
// `rna_FModifier_blending_range`, `rna_MultiresModifier_level_range`,
// `rna_ActionConstraint_minmax_range`, `rna_CurveMapping_clip{min,max}{x,y}_range`; Blender
// 5.2.4, GPL-2.0-or-later). `BKE_animsys_write_to_rna_path` clamps an animated value to
// `RNA_property_float_range` / `_int_range`, which is the range callback's answer when the
// property has one (the static range is then ignored); these callbacks read the struct's own
// data. `MeshPolygon.material_index` has no callback in 5.2.4 (`rna_MeshPoly_material_index_range`
// is `#if 0`): it keeps its static `0..INT_MAX` range ([`dynamic_hard`]).
/// The range a range callback gives `prop` of `owner` from its data, if it has such a callback.
fn data_range(
    host: &dyn HostRead,
    owner: &dyn Rna,
    ptr: &RnaPtr,
    prop: &'static PropDef,
) -> Option<(f64, f64)> {
    let srna = owner.srna();
    let read = |name: &str| -> Option<f64> {
        let p = srna.find_prop(name)?;
        match get_field(host, owner, ptr, p)? {
            Field::Value(Value::Float(f)) => Some(f64::from(f)),
            Field::Value(Value::Double(d)) => Some(d),
            Field::Value(Value::Int(i)) => Some(i as f64),
            Field::Value(Value::Enum(e)) => match &p.kind {
                frap_rna::PropKind::Enum { items, .. } => items
                    .iter()
                    .find(|i| i.identifier == e.as_str())
                    .map(|i| f64::from(i.value)),
                _ => None,
            },
            _ => None,
        }
    };
    range_from_data(srna, prop.identifier, &read)
}

/// [`data_range`] given the owner's struct, the property and a reader of the owner's other
/// properties (numbers; enums as their integer value).
fn range_from_data(
    srna: &'static StructDef,
    id: &str,
    read: &dyn Fn(&str) -> Option<f64>,
) -> Option<(f64, f64)> {
    if srna.is_a("Node") {
        return match id {
            "width" => Some((read("bl_width_min")?, read("bl_width_max")?)),
            "height" => Some((read("bl_height_min")?, read("bl_height_max")?)),
            _ => None,
        };
    }
    if srna.is_a("FModifier") && matches!(id, "blend_in" | "blend_out") {
        // `*max = fcm->efra - fcm->sfra` (in float).
        let (start, end) = (read("frame_start")? as f32, read("frame_end")? as f32);
        return Some((0.0, f64::from(end - start)));
    }
    if srna.is_a("MultiresModifier") && matches!(id, "levels" | "sculpt_levels" | "render_levels") {
        // `max_ii(0, mmd->totlvl)`. frap cannot subdivide a Multires modifier (OUT in
        // OPERATORS.md) and does not bind `total_levels`: its modifiers have no levels (0).
        return Some((0.0, read("total_levels").unwrap_or(0.0).max(0.0)));
    }
    if srna.is_a("ActionConstraint") && matches!(id, "min" | "max") {
        return Some(action_constraint_range(read("transform_channel")? as i32));
    }
    if srna.is_a("CurveMapping") {
        return match id {
            "clip_min_x" => Some((-100.0, read("clip_max_x")?)),
            "clip_min_y" => Some((-100.0, read("clip_max_y")?)),
            "clip_max_x" => Some((read("clip_min_x")?, 100.0)),
            "clip_max_y" => Some((read("clip_min_y")?, 100.0)),
            _ => None,
        };
    }
    None
}

/// `rna_ActionConstraint_minmax_range`: `transform_channel` values 0, 1, 2 (`ROTATION_X/Y/Z`)
/// range over ±180, the others over ±1000.
fn action_constraint_range(transform_channel: i32) -> (f64, f64) {
    if matches!(transform_channel, 0..=2) {
        (-180.0, 180.0)
    } else {
        (-1000.0, 1000.0)
    }
}

/// The animated `c.value` clamped as `RNA_property_float_clamp` / `RNA_property_int_clamp` does
/// for a property whose range callback reads the data ([`data_range`]): `value < min` gives
/// `min`, else `value > max` gives `max` (so `min` wins when the range is empty). Other
/// properties keep their value (the writer applies the static or [`dynamic_hard`] range).
fn clamp_to_data_range(host: &dyn HostRead, root: Root, c: &frap_anim::eval::AnimChannel) -> f32 {
    const RANGED: &[&str] = &[
        "width",
        "height",
        "blend_in",
        "blend_out",
        "levels",
        "sculpt_levels",
        "render_levels",
        "min",
        "max",
        "clip_min_x",
        "clip_min_y",
        "clip_max_x",
        "clip_max_y",
    ];
    let last = c.data_path.rsplit('.').next().unwrap_or_default();
    if !RANGED.contains(&last) {
        return c.value;
    }
    let Some(segs) = parse_path(&c.data_path) else {
        return c.value;
    };
    let range = with_path(host, root, &segs, &mut |leaf| match leaf {
        Leaf::Prop {
            owner, ptr, prop, ..
        } => data_range(host, owner, ptr, prop),
        Leaf::Struct { .. } => None,
    });
    match range {
        Some((lo, hi)) => {
            let v = f64::from(c.value);
            if v < lo {
                lo as f32
            } else if v > hi {
                hi as f32
            } else {
                c.value
            }
        }
        None => c.value,
    }
}

/// The refined struct of the datablock at `root` (`SunLight`, `ShaderNodeTree`).
pub fn root_struct(host: &dyn HostRead, root: Root) -> Option<&'static StructDef> {
    with_path(host, root, &[], &mut |leaf| match leaf {
        Leaf::Struct { rna, .. } => Some(rna.srna()),
        Leaf::Prop { .. } => None,
    })
}

/// The refined struct the path `prefix` reaches from `root` (a collection item or a pointer),
/// as the data has it; `None` when the path does not reach a struct.
pub fn refined_struct(
    host: &dyn HostRead,
    root: Root,
    prefix: &[PathSeg],
) -> Option<&'static StructDef> {
    with_path(host, root, prefix, &mut |leaf| match leaf {
        Leaf::Struct { rna, .. } => Some(rna.srna()),
        Leaf::Prop { .. } => None,
    })
}

/// What [`with_path`] reached.
pub enum Leaf<'a> {
    /// A struct (the datablock itself for an empty path).
    Struct {
        /// The struct.
        rna: &'a dyn Rna,
        /// Its canonical pointer.
        ptr: &'a RnaPtr,
    },
    /// A value property.
    Prop {
        /// The struct owning the property.
        owner: &'a dyn Rna,
        /// The owner's canonical pointer.
        ptr: &'a RnaPtr,
        /// The property.
        prop: &'static PropDef,
        /// Its current value.
        value: Value,
    },
}

/// Walks `segs` (an ID-relative RNA path, parsed) from the datablock at `root` through the
/// bindings, collection items and get hooks, and calls `f` on what it reaches. `None` when the
/// path does not resolve (or `f` answers `None`). Paths never cross an ID pointer, as F-curve
/// paths do not.
pub fn with_path<R>(
    host: &dyn HostRead,
    root: Root,
    segs: &[PathSeg],
    f: &mut dyn FnMut(Leaf<'_>) -> Option<R>,
) -> Option<R> {
    let main = host.main();
    match root {
        Root::Id(id) => {
            let r = main.id(id)?;
            let here = RnaPtr::id(id, r.srna());
            walk(host, r, &here, segs, f)
        }
        Root::Embedded { owner, slot } => {
            let o = main.id(owner)?;
            let prop = o.srna().find_prop(slot.prop_identifier())?;
            let owner_ptr = RnaPtr::id(owner, o.srna());
            match get_field(host, o, &owner_ptr, prop)? {
                Field::Embedded(_, s) | Field::Struct(s) => {
                    let here = RnaPtr::new(root, s.srna());
                    walk(host, &*s, &here, segs, f)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// `prop` of `cur`, through the registered get hook when the binding defers to one.
fn get_field<'a>(
    host: &'a dyn HostRead,
    cur: &'a dyn Rna,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Option<Field<'a>> {
    match cur.get(prop) {
        Ok(Field::NeedsHook) => find_hook(this.srna, prop.identifier)
            .and_then(|h| h.get)
            .and_then(|g| g(host, this, prop).ok()),
        Ok(f) => Some(f),
        Err(e) if e.kind == RnaErrorKind::NoAttribute => find_hook(this.srna, prop.identifier)
            .and_then(|h| h.get)
            .and_then(|g| g(host, this, prop).ok()),
        Err(_) => None,
    }
}

fn walk<R>(
    host: &dyn HostRead,
    cur: &dyn Rna,
    here: &RnaPtr,
    rest: &[PathSeg],
    f: &mut dyn FnMut(Leaf<'_>) -> Option<R>,
) -> Option<R> {
    let Some((seg, rest)) = rest.split_first() else {
        return f(Leaf::Struct {
            rna: cur,
            ptr: here,
        });
    };
    let PathSeg::Prop(name) = seg else {
        return None;
    };
    let prop = cur.srna().find_prop(name)?;
    match get_field(host, cur, here, prop)? {
        Field::Value(v) if rest.is_empty() => f(Leaf::Prop {
            owner: cur,
            ptr: here,
            prop,
            value: v,
        }),
        Field::Struct(s) | Field::Embedded(_, s) => {
            let next = here.child(Seg::Prop(prop.identifier), s.srna());
            walk(host, &*s, &next, rest, f)
        }
        Field::Coll(c) => {
            let (key, rest) = rest.split_first()?;
            let i = match key {
                PathSeg::Index(i) => {
                    let i = usize::try_from(*i).ok()?;
                    (i < c.len()).then_some(i)?
                }
                PathSeg::Key(k) => c.find(k)?,
                PathSeg::Prop(_) => return None,
            };
            match c.item(i).ok()? {
                Item::Struct(s) => {
                    let next = here
                        .child(Seg::Prop(prop.identifier), s.srna())
                        .child(c.item_seg(i), s.srna());
                    walk(host, &*s, &next, rest, f)
                }
                Item::Id(_) => None,
            }
        }
        _ => None,
    }
}

/// The ID an ID-pointer property at the end of `segs` points to (`data` of an object), `None`
/// when `segs` does not end on such a pointer or it is empty.
pub fn id_pointer_at(host: &dyn HostRead, root: Root, segs: &[PathSeg]) -> Option<IdAny> {
    let (PathSeg::Prop(name), prefix) = segs.split_last()? else {
        return None;
    };
    with_path(host, root, prefix, &mut |leaf| {
        let Leaf::Struct { rna, ptr } = leaf else {
            return None;
        };
        let prop = rna.srna().find_prop(name)?;
        match get_field(host, rna, ptr, prop)? {
            Field::Id(id) => id,
            _ => None,
        }
    })
}

/// The current value of `data_path` (parsed) from `root`, with the property it names.
pub fn read_path(
    host: &dyn HostRead,
    root: Root,
    segs: &[PathSeg],
) -> Option<(RnaPtr, &'static PropDef, Value)> {
    with_path(host, root, segs, &mut |leaf| match leaf {
        Leaf::Prop {
            ptr, prop, value, ..
        } => Some((ptr.clone(), prop, value)),
        Leaf::Struct { .. } => None,
    })
}

pub use frap_anim::eval::props::parse_path;

/// The hard range `RNA_property_float_range` / `RNA_property_int_range` answer for `prop` of
/// the struct `owner`, which differs from the static (dumped) range for properties with a range
/// callback: `ShapeKey.value` (`rna_ShapeKey_value_range`: the block's
/// `slider_min..slider_max`, applied by its setter; the other [`frap_scene::id::Scene::UNCLAMPED`]
/// setters apply their own range too) and the `default_value` of a standard node socket
/// (`rna_NodeSocketStandard_float_range` / `_int_range`: unbounded, from 0 for unsigned
/// subtypes; frap's `coerce` applies the same rule to Python assignments). `None`: no range
/// clamp before the setter.
fn dynamic_hard(owner: &str, prop: &PropDef) -> Option<(f64, f64)> {
    use frap_rna::{PropKind, Subtype};
    if frap_scene::id::Scene::UNCLAMPED.contains(&(owner, prop.identifier)) {
        return None;
    }
    let socket = owner.starts_with("NodeSocket") && prop.identifier == "default_value";
    match &prop.kind {
        PropKind::Float { .. } if socket && prop.subtype == Subtype::Unsigned => {
            Some((0.0, f64::from(f32::MAX)))
        }
        PropKind::Float { .. } if socket => Some((-f64::from(f32::MAX), f64::from(f32::MAX))),
        PropKind::Int { .. } if socket && prop.subtype == Subtype::Unsigned => {
            Some((0.0, f64::from(i32::MAX)))
        }
        PropKind::Int { .. } if socket => Some((f64::from(i32::MIN), f64::from(i32::MAX))),
        PropKind::Float { hard, .. } => Some(*hard),
        PropKind::Int { hard, .. } => Some((hard.0 as f64, hard.1 as f64)),
        _ => Some((f64::NEG_INFINITY, f64::INFINITY)),
    }
}

/// The value `BKE_animsys_write_to_rna_path` writes for an animated `value` into element
/// `index` of `prop` of the struct `owner` (whose current value is `current`): booleans are
/// `value > 1 - FLT_EPSILON`, integers truncate and clamp to the hard range, floats clamp
/// (`RNA_property_*_clamp`: the dynamic range where the property has a range callback,
/// [`dynamic_hard`]), enums take the item whose integer value is `int(value)`. `None` when
/// nothing changes (Blender skips the write when the old value equals the new one) or the
/// value cannot be written.
pub fn channel_value(
    prop: &PropDef,
    owner: &str,
    current: &Value,
    index: i32,
    value: f32,
) -> Option<Value> {
    use frap_rna::PropKind;
    let idx = usize::try_from(index.max(0)).ok()?;
    let range = dynamic_hard(owner, prop);
    let int = |v: f32| {
        let v = v as i32 as i64;
        match range {
            Some((lo, hi)) => v.clamp(lo as i64, hi as i64),
            None => v,
        }
    };
    let float = |v: f32| match range {
        Some((lo, hi)) => v.clamp(lo as f32, hi as f32),
        None => v,
    };
    let new = match (&prop.kind, current) {
        (PropKind::Bool { .. }, Value::Bool(_)) => Value::Bool(value > 1.0 - f32::EPSILON),
        (PropKind::Bool { .. }, Value::BoolArray(a)) => {
            let mut a = a.clone();
            *a.get_mut(idx)? = value > 1.0 - f32::EPSILON;
            Value::BoolArray(a)
        }
        (PropKind::Int { .. }, Value::Int(_)) => Value::Int(int(value)),
        (PropKind::Int { .. }, Value::IntArray(a)) => {
            let mut a = a.clone();
            *a.get_mut(idx)? = int(value);
            Value::IntArray(a)
        }
        (PropKind::Float { .. }, Value::Float(_)) => Value::Float(float(value)),
        (PropKind::Float { .. }, Value::Double(_)) => Value::Double(match range {
            Some((lo, hi)) => f64::from(value).clamp(lo, hi),
            None => f64::from(value),
        }),
        (PropKind::Float { .. }, Value::FloatArray { data, shape, tag }) => {
            let mut data = data.clone();
            *data.get_mut(idx)? = float(value);
            Value::FloatArray {
                data,
                shape: *shape,
                tag: *tag,
            }
        }
        (PropKind::Enum { items, .. }, Value::Enum(_)) => {
            let v = value as i32;
            Value::Enum(items.iter().find(|i| i.value == v)?.identifier.into())
        }
        _ => return None,
    };
    (new != *current).then_some(new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_anim::eval::props::parse_path;

    #[test]
    fn roots_list_embedded_trees_after_their_owner() {
        let (main, _) = frap_scene::startup::factory();
        let roots = animated_roots(&main);
        let mat = main.materials.iter().next().map(|(r, _)| r.any()).unwrap();
        let i = roots.iter().position(|r| *r == Root::Id(mat)).unwrap();
        assert_eq!(
            roots[i + 1],
            Root::Embedded {
                owner: mat,
                slot: EmbeddedSlot::NodeTree
            }
        );
    }

    /// `BKE_animsys_write_to_rna_path`'s coercions per property type.
    #[test]
    fn channel_values_follow_the_property_type() {
        let ob = frap_anim::eval::props::root_struct("OBJECT").unwrap();
        let loc = ob.find_prop("location").unwrap();
        let cur = Value::FloatArray {
            data: [1.0f32, 2.0, 3.0].into_iter().collect(),
            shape: loc.array,
            tag: frap_rna::MathTag::Vector,
        };
        assert_eq!(
            channel_value(loc, "Object", &cur, 1, 5.0),
            Some(Value::FloatArray {
                data: [1.0f32, 5.0, 3.0].into_iter().collect(),
                shape: loc.array,
                tag: frap_rna::MathTag::Vector,
            })
        );
        // Unchanged: no write.
        assert_eq!(channel_value(loc, "Object", &cur, 1, 2.0), None);
        // Out of range index: nothing to write.
        assert_eq!(channel_value(loc, "Object", &cur, 7, 2.0), None);
        // Booleans: value > 1 - FLT_EPSILON.
        let hide = ob.find_prop("hide_render").unwrap();
        assert_eq!(
            channel_value(hide, "Object", &Value::Bool(false), 0, 0.9999),
            None,
            "0.9999 stays false"
        );
        assert_eq!(
            channel_value(hide, "Object", &Value::Bool(false), 0, 1.0),
            Some(Value::Bool(true))
        );
        // Enums: the item whose value is int(value).
        let mode = ob.find_prop("rotation_mode").unwrap();
        assert_eq!(
            channel_value(mode, "Object", &Value::Enum("QUATERNION".into()), 0, 1.7),
            Some(Value::Enum("XYZ".into()))
        );
        assert_eq!(
            channel_value(mode, "Object", &Value::Enum("XYZ".into()), 0, 99.0),
            None,
            "no item with that value"
        );
        // Integers truncate toward zero and clamp to the hard range.
        let pass = ob.find_prop("pass_index").unwrap();
        assert_eq!(
            channel_value(pass, "Object", &Value::Int(0), 0, 2.9),
            Some(Value::Int(2))
        );
        assert_eq!(
            channel_value(pass, "Object", &Value::Int(0), 0, -5.0),
            None,
            "pass_index is clamped at 0, unchanged"
        );
    }

    /// Properties with a range callback clamp to the dynamic range (`RNA_property_float_clamp`):
    /// `ShapeKey.value` is left to its setter (the block's sliders), a standard socket's
    /// `default_value` is unbounded.
    #[test]
    fn channel_values_use_dynamic_ranges() {
        let s = |id: &str| frap_spec_rna::find_struct(id).unwrap();
        let value = s("ShapeKey").find_prop("value").unwrap();
        assert_eq!(
            channel_value(value, "ShapeKey", &Value::Float(0.0), 0, -0.5),
            Some(Value::Float(-0.5))
        );
        assert_eq!(
            channel_value(value, "ShapeKey", &Value::Float(0.0), 0, 2.5),
            Some(Value::Float(2.5))
        );
        let dv = s("NodeSocketFloatFactor")
            .find_prop("default_value")
            .unwrap();
        assert_eq!(
            channel_value(dv, "NodeSocketFloatFactor", &Value::Float(0.0), 0, 2.0),
            Some(Value::Float(2.0))
        );
        // Other structs keep the static range.
        let ob = frap_anim::eval::props::root_struct("OBJECT").unwrap();
        let pass = ob.find_prop("pass_index").unwrap();
        assert_eq!(channel_value(pass, "Object", &Value::Int(0), 0, -5.0), None);
    }

    /// The update callbacks that tag animation (verified on 5.2.4 for `co`, `interpolation`,
    /// `mute`, `action_influence`, `action_slot`; `select`, `lock`, `data_path` do not).
    #[test]
    fn animation_tags_follow_blenders_update_callbacks() {
        let s = |id: &str| frap_spec_rna::find_struct(id).unwrap();
        assert!(tags_animation_on_write(s("Keyframe"), "co"));
        assert!(tags_animation_on_write(s("Keyframe"), "interpolation"));
        assert!(!tags_animation_on_write(
            s("Keyframe"),
            "select_control_point"
        ));
        assert!(tags_animation_on_write(s("FCurve"), "mute"));
        assert!(!tags_animation_on_write(s("FCurve"), "select"));
        assert!(!tags_animation_on_write(s("FCurve"), "data_path"));
        assert!(tags_animation_on_write(s("AnimData"), "action_influence"));
        assert!(tags_animation_on_write(s("AnimData"), "action_slot"));
        assert!(!tags_animation_on_write(s("AnimData"), "use_pin"));
        assert!(tags_animation_on_write(s("NlaStrip"), "frame_start"));
        assert!(!tags_animation_on_write(s("NlaTrack"), "name"));
        assert!(!tags_animation_on_write(s("Action"), "frame_start"));
        assert!(!tags_animation_on_write(s("Object"), "location"));
        assert!(tags_animation_on_call(s("FCurveKeyframePoints"), "insert"));
        assert!(tags_animation_on_call(s("FCurve"), "update"));
        assert!(tags_animation_on_call(s("NlaTracks"), "new"));
        assert!(!tags_animation_on_call(s("ActionSlots"), "new"));
        assert!(!tags_animation_on_call(s("Object"), "keyframe_insert"));
    }

    /// The `blender::Map` port gives Blender's write order (verified on 5.2.4 through the
    /// `ShapeKey.value` / `slider_max` clamp: `slider_max` first for `K`, `A`, `G`, `Key`,
    /// `value` first for `B`, `F`, `Smile`; with `slider_min` as a third channel the map has 8
    /// slots and `A` writes `value` first).
    #[test]
    fn evaluation_result_order_is_blenders_map_order() {
        let order = |name: &str, props: &[&str]| {
            let mut m = blender_map::Map::default();
            m.reserve(props.len());
            for p in props {
                m.add_overwrite(format!("key_blocks[\"{name}\"].{p}").into(), 0);
            }
            m.keys()
                .map(|(p, _)| p.rsplit('.').next().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        for (name, slider_first) in [
            ("K", true),
            ("A", true),
            ("G", true),
            ("Key", true),
            ("B", false),
            ("F", false),
            ("Smile", false),
        ] {
            let o = order(name, &["value", "slider_max"]);
            assert_eq!(o[0] == "slider_max", slider_first, "{name}: {o:?}");
        }
        assert_eq!(
            order("A", &["value", "slider_max", "slider_min"]),
            ["value", "slider_min", "slider_max"]
        );
        assert_eq!(
            order("K", &["value", "slider_max", "slider_min"]),
            ["slider_max", "value", "slider_min"]
        );
        // `hash_string` is djb2 with `+`; the array index mixes in with factor 19349669.
        assert_eq!(blender_map::hash("", 0), 5381);
        assert_eq!(blender_map::hash("a", 0), 5381 * 33 + 97);
        assert_eq!(blender_map::hash("a", 1), (5381 * 33 + 97) ^ 19_349_669);
        // Growth past the reserved size reinserts in slot order and keeps every key once.
        let mut m = blender_map::Map::default();
        m.reserve(1);
        for i in 0..9 {
            m.add_overwrite("location".into(), i % 3);
        }
        assert_eq!(m.keys().count(), 3);
    }

    /// The range callbacks that read the struct's data (`rna_Node_width_range`,
    /// `rna_FModifier_blending_range`, `rna_MultiresModifier_level_range`,
    /// `rna_CurveMapping_clip*_range`), and none for other properties.
    #[test]
    fn data_ranges_read_the_owner() {
        let s = |id: &str| frap_spec_rna::find_struct(id).unwrap();
        let vals = |pairs: &'static [(&'static str, f64)]| {
            move |name: &str| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
        };
        let node = vals(&[
            ("bl_width_min", 100.0),
            ("bl_width_max", 700.0),
            ("bl_height_min", 30.0),
            ("bl_height_max", 3.0e38),
        ]);
        assert_eq!(
            range_from_data(s("ShaderNodeBsdfPrincipled"), "width", &node),
            Some((100.0, 700.0))
        );
        assert_eq!(
            range_from_data(s("ShaderNodeBsdfPrincipled"), "height", &node),
            Some((30.0, 3.0e38))
        );
        assert_eq!(
            range_from_data(s("ShaderNodeBsdfPrincipled"), "location", &node),
            None
        );
        let fm = vals(&[("frame_start", 2.0), ("frame_end", 10.5)]);
        assert_eq!(
            range_from_data(s("FModifier"), "blend_in", &fm),
            Some((0.0, 8.5))
        );
        assert_eq!(range_from_data(s("FModifier"), "influence", &fm), None);
        let none = vals(&[]);
        assert_eq!(
            range_from_data(s("MultiresModifier"), "levels", &none),
            Some((0.0, 0.0)),
            "frap's Multires modifiers have no levels"
        );
        let mr = vals(&[("total_levels", 3.0)]);
        assert_eq!(
            range_from_data(s("MultiresModifier"), "render_levels", &mr),
            Some((0.0, 3.0))
        );
        let cm = vals(&[
            ("clip_min_x", -2.0),
            ("clip_max_x", 4.0),
            ("clip_min_y", 0.5),
            ("clip_max_y", 1.0),
        ]);
        assert_eq!(
            range_from_data(s("CurveMapping"), "clip_min_x", &cm),
            Some((-100.0, 4.0))
        );
        assert_eq!(
            range_from_data(s("CurveMapping"), "clip_max_y", &cm),
            Some((0.5, 100.0))
        );
        assert_eq!(range_from_data(s("Object"), "location", &cm), None);
    }

    /// `rna_ActionConstraint_minmax_range`: rotation channels range over ±180.
    #[test]
    fn action_constraint_ranges_follow_the_channel() {
        assert_eq!(action_constraint_range(0), (-180.0, 180.0));
        assert_eq!(action_constraint_range(2), (-180.0, 180.0));
        assert_eq!(action_constraint_range(10), (-1000.0, 1000.0));
        assert_eq!(action_constraint_range(20), (-1000.0, 1000.0));
    }

    /// `MeshPolygon.material_index` has no range callback in 5.2.4: the static `0..INT_MAX`
    /// range applies to animated values (negative keys read 0, large ones stay).
    #[test]
    fn material_index_keeps_its_static_range() {
        let s = frap_spec_rna::find_struct("MeshPolygon").unwrap();
        let p = s.find_prop("material_index").unwrap();
        assert_eq!(
            channel_value(p, "MeshPolygon", &Value::Int(1), 0, -3.0),
            Some(Value::Int(0))
        );
        assert_eq!(
            channel_value(p, "MeshPolygon", &Value::Int(0), 0, 5.7),
            Some(Value::Int(5))
        );
    }

    #[test]
    fn parse_paths_used_by_channels() {
        assert_eq!(parse_path("location").unwrap().len(), 1);
        assert_eq!(
            parse_path("pose.bones[\"Bone\"].rotation_quaternion")
                .unwrap()
                .len(),
            4
        );
    }
}
