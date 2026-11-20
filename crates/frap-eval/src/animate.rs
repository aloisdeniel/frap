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
use frap_rna::{
    ActionId, EmbeddedSlot, Field, IdAny, IdCode, IdRef, Item, PropDef, Rna, RnaError,
    RnaErrorKind, RnaPtr, Root, Seg, StructDef, Value,
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
/// `scene.frame_set()`, `context.evaluated_depsgraph_get()` / `evaluated_get()`): when time is
/// dirty, writes the animation of the context scene's `frame_current` into the originals.
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
pub fn apply_pending_animation_tagged(
    host: &mut dyn Host,
    time_dirty: bool,
    tagged: &[Root],
) -> Result<AnimationApplied, RnaError> {
    if !time_dirty && tagged.is_empty() {
        return Ok(AnimationApplied::default());
    }
    let frame = host
        .main()
        .scenes
        .get(host.win().scene)
        .map_or(0, |s| s.frame_current);
    if time_dirty {
        return apply_animation_at(host, frame as f32);
    }
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
    apply_animation_filtered(host, frame as f32, &is_tagged)
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

/// Writes the animation of every animated ID at `frame` into `host`'s originals (a session
/// boundary, or a render/export clone).
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
                work.push((root, channels));
            }
        }
    }
    let mut applied = AnimationApplied::default();
    for (root, channels) in work {
        for c in channels {
            if host
                .set_path(root, &c.data_path, c.array_index, Value::Float(c.value))
                .is_ok()
            {
                applied.channels += 1;
            }
        }
    }
    Ok(applied)
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

/// The value `BKE_animsys_write_to_rna_path` writes for an animated `value` into element
/// `index` of `prop` (whose current value is `current`): booleans are `value > 1 - FLT_EPSILON`,
/// integers truncate and clamp to the hard range, floats clamp, enums take the item whose
/// integer value is `int(value)`. `None` when nothing changes (Blender skips the write when the
/// old value equals the new one) or the value cannot be written.
pub fn channel_value(prop: &PropDef, current: &Value, index: i32, value: f32) -> Option<Value> {
    use frap_rna::PropKind;
    let idx = usize::try_from(index.max(0)).ok()?;
    let new = match (&prop.kind, current) {
        (PropKind::Bool { .. }, Value::Bool(_)) => Value::Bool(value > 1.0 - f32::EPSILON),
        (PropKind::Bool { .. }, Value::BoolArray(a)) => {
            let mut a = a.clone();
            *a.get_mut(idx)? = value > 1.0 - f32::EPSILON;
            Value::BoolArray(a)
        }
        (PropKind::Int { hard, .. }, Value::Int(_)) => {
            Value::Int((value as i32 as i64).clamp(hard.0, hard.1))
        }
        (PropKind::Int { hard, .. }, Value::IntArray(a)) => {
            let mut a = a.clone();
            *a.get_mut(idx)? = (value as i32 as i64).clamp(hard.0, hard.1);
            Value::IntArray(a)
        }
        (PropKind::Float { hard, .. }, Value::Float(_)) => {
            Value::Float(value.clamp(hard.0 as f32, hard.1 as f32))
        }
        (PropKind::Float { hard, .. }, Value::Double(_)) => {
            Value::Double(f64::from(value).clamp(hard.0, hard.1))
        }
        (PropKind::Float { hard, .. }, Value::FloatArray { data, shape, tag }) => {
            let mut data = data.clone();
            *data.get_mut(idx)? = value.clamp(hard.0 as f32, hard.1 as f32);
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
            channel_value(loc, &cur, 1, 5.0),
            Some(Value::FloatArray {
                data: [1.0f32, 5.0, 3.0].into_iter().collect(),
                shape: loc.array,
                tag: frap_rna::MathTag::Vector,
            })
        );
        // Unchanged: no write.
        assert_eq!(channel_value(loc, &cur, 1, 2.0), None);
        // Out of range index: nothing to write.
        assert_eq!(channel_value(loc, &cur, 7, 2.0), None);
        // Booleans: value > 1 - FLT_EPSILON.
        let hide = ob.find_prop("hide_render").unwrap();
        assert_eq!(
            channel_value(hide, &Value::Bool(false), 0, 0.9999),
            None,
            "0.9999 stays false"
        );
        assert_eq!(
            channel_value(hide, &Value::Bool(false), 0, 1.0),
            Some(Value::Bool(true))
        );
        // Enums: the item whose value is int(value).
        let mode = ob.find_prop("rotation_mode").unwrap();
        assert_eq!(
            channel_value(mode, &Value::Enum("QUATERNION".into()), 0, 1.7),
            Some(Value::Enum("XYZ".into()))
        );
        assert_eq!(
            channel_value(mode, &Value::Enum("XYZ".into()), 0, 99.0),
            None,
            "no item with that value"
        );
        // Integers truncate toward zero and clamp to the hard range.
        let pass = ob.find_prop("pass_index").unwrap();
        assert_eq!(
            channel_value(pass, &Value::Int(0), 0, 2.9),
            Some(Value::Int(2))
        );
        assert_eq!(
            channel_value(pass, &Value::Int(0), 0, -5.0),
            None,
            "pass_index is clamped at 0, unchanged"
        );
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
