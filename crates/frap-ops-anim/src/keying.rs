// SPDX-License-Identifier: GPL-3.0-or-later
//! Keying (WP 6.4, ARCHITECTURE §18): `bpy_struct.keyframe_insert(data_path, *, index=-1,
//! frame=None, group="", options=set(), keytype='KEYFRAME') -> bool`, `keyframe_delete` and the
//! `anim.keyframe_*` operators share this module.
//!
//! Ported from blender/source/blender/animrig/intern/keyframing.cc (`insert_keyframes`,
//! `delete_keyframe`, `clear_keyframe`, `default_channel_group_for_path`,
//! `CombinedKeyingResult::generate_reports`), animrig/intern/animdata.cc (`id_action_ensure`,
//! `find_related_ids`), animrig/intern/action.cc (`assign_action_ensure_slot_for_keying`,
//! `Action::layer_keystrip_ensure`, `StripKeyframeData::keyframe_insert`), animrig/intern/
//! fcurve.cc (`create_fcurve_for_channel`, `fcurve_delete_keyframe_at_time`) and
//! python/intern/bpy_rna_anim.cc (`pyrna_struct_anim_args_parse_ex`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Keying an ID without an action creates `<IDName>Action` (`CubeAction`; for a shape-key
//! datablock the name of the mesh it belongs to, for an embedded node tree its owner's name)
//! unless a related ID (object ↔ single-user data ↔ shape keys, material ↔ its node tree)
//! already has one: shape-key values go into the **object's** action under a `KE` slot. The slot
//! (`OB<name>`, `KE<name>`, `NTShader Nodetree`) is auto-assigned or created, one layer
//! (`Layer`) and one keyframe strip are ensured, and every array element becomes one F-curve,
//! grouped `Object Transforms` (object transforms) or by bone name (pose bones), keys BEZIER +
//! AUTO_CLAMPED with handles recalculated.
//!
//! Not modelled (deviations §2): visual keying (`INSERTKEY_VISUAL`) keys the property values,
//! which equal the visual ones without constraints (frap has none) up to float rounding;
//! cycle-aware keying (no Cycles F-modifier); the NLA remapping of values keyed into an action
//! that blends over NLA tracks with a non-REPLACE mode or an influence below 1 (keyed as is).

use frap_anim::eval::props::{PathSeg, parse_path};
use frap_anim::fcurve::keys::{InsertFlags, InsertResult, KeyframeSettings, insert_vert};
use frap_anim::{Action, ActionStrip, AnimData, FCurve, KeyframeType};
use frap_eval::animate::{Leaf, anim_data, with_path};
use frap_rna::{
    ActionId, EmbeddedSlot, IdAny, IdCode, IdRef, PropDef, PropFlags, PropKind, PyExc, ReportType,
    RnaError, RnaErrorKind, RnaPtr, Root, Subtype, Value,
};
use frap_scene::{ChangeLog, Host, HostRead, Main};
use smol_str::SmolStr;

/// What keying needs from its caller: reading the state, then writing `bpy.data`. An RNA
/// function passes its `Host` ([`HostKeying`]), an operator its `OpCtx`.
pub trait KeyingHost {
    /// Read access (the bindings and get hooks see the current originals).
    fn read(&mut self) -> &dyn HostRead;
    /// `bpy.data` and the statement's change log.
    fn write(&mut self) -> (&mut Main, &mut ChangeLog);
}

/// [`KeyingHost`] over an RNA function's `Host`.
pub struct HostKeying<'a>(pub &'a mut dyn Host);

impl KeyingHost for HostKeying<'_> {
    fn read(&mut self) -> &dyn HostRead {
        &*self.0
    }
    fn write(&mut self) -> (&mut Main, &mut ChangeLog) {
        self.0.main_mut()
    }
}

impl KeyingHost for frap_op::OpCtx<'_> {
    fn read(&mut self) -> &dyn HostRead {
        self.split_eval().0
    }
    fn write(&mut self) -> (&mut Main, &mut ChangeLog) {
        self.main_mut()
    }
}

/// `eInsertKeyFlags` (the ones keying distinguishes).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyFlags {
    /// `INSERTKEY_NEEDED`
    pub needed: bool,
    /// `INSERTKEY_MATRIX` (`INSERTKEY_VISUAL` in the API)
    pub visual: bool,
    /// `INSERTKEY_REPLACE`
    pub replace: bool,
    /// `INSERTKEY_AVAILABLE`
    pub available: bool,
    /// `INSERTKEY_CYCLE_AWARE`
    pub cycle_aware: bool,
}

impl KeyFlags {
    /// The `options` set of `keyframe_insert` (`rna_enum_keying_flag_api_items`).
    pub fn from_api(options: &[SmolStr]) -> KeyFlags {
        let has = |s: &str| options.iter().any(|o| o == s);
        KeyFlags {
            needed: has("INSERTKEY_NEEDED"),
            visual: has("INSERTKEY_VISUAL"),
            replace: has("INSERTKEY_REPLACE"),
            available: has("INSERTKEY_AVAILABLE"),
            cycle_aware: has("INSERTKEY_CYCLE_AWARE"),
        }
    }

    /// `key_insertion_may_create_fcurve`
    fn may_create_fcurve(self) -> bool {
        !(self.replace || self.available)
    }
}

/// `SingleKeyingResult`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum KeyingResult {
    /// `SUCCESS`
    Success = 0,
    /// `UNKNOWN_FAILURE`
    UnknownFailure,
    /// `CANNOT_CREATE_FCURVE`
    CannotCreateFcurve,
    /// `FCURVE_NOT_KEYFRAMEABLE`
    FcurveNotKeyframeable,
    /// `NO_KEY_NEEDED`
    NoKeyNeeded,
    /// `UNABLE_TO_INSERT_TO_NLA_STACK`
    UnableToInsertToNlaStack,
    /// `ID_NOT_EDITABLE`
    IdNotEditable,
    /// `ID_NOT_ANIMATABLE`
    IdNotAnimatable,
    /// `NO_VALID_LAYER`
    NoValidLayer,
    /// `NO_VALID_STRIP`
    NoValidStrip,
    /// `NO_VALID_SLOT`
    NoValidSlot,
    /// `CANNOT_RESOLVE_PATH`
    CannotResolvePath,
}

const RESULTS: usize = 12;

/// `CombinedKeyingResult`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CombinedKeyingResult {
    counts: [usize; RESULTS],
}

impl CombinedKeyingResult {
    /// `add`
    pub fn add(&mut self, r: KeyingResult, count: usize) {
        self.counts[r as usize] += count;
    }
    /// `merge`
    pub fn merge(&mut self, o: &CombinedKeyingResult) {
        for (a, b) in self.counts.iter_mut().zip(o.counts) {
            *a += b;
        }
    }
    /// `get_count`
    pub fn count(&self, r: KeyingResult) -> usize {
        self.counts[r as usize]
    }
    /// `has_errors`
    pub fn has_errors(&self) -> bool {
        self.counts[1..].iter().any(|c| *c > 0)
    }

    /// `generate_reports(reports, level)`: the report keying failures produce.
    pub fn report(&self, level: ReportType) -> (ReportType, String) {
        use KeyingResult as K;
        if !self.has_errors() && self.count(K::Success) == 0 {
            return (
                ReportType::Warning,
                "No keys have been inserted and no errors have been reported.".into(),
            );
        }
        let mut errors: Vec<String> = Vec::new();
        let mut push = |r: K, f: &dyn Fn(usize) -> String| {
            let n = self.count(r);
            if n > 0 {
                errors.push(f(n));
            }
        };
        push(K::UnknownFailure, &|n| {
            format!("There were {n} keying failures for unknown reasons.")
        });
        push(K::CannotCreateFcurve, &|n| {
            format!(
                "Could not create {n} F-Curve(s). This can happen when only inserting to available F-Curves."
            )
        });
        push(K::FcurveNotKeyframeable, &|n| {
            format!("{n} F-Curve(s) are not keyframeable. They might be locked or sampled.")
        });
        push(K::NoKeyNeeded, &|n| {
            format!(
                "Due to the setting 'Only Insert Needed', {n} keyframe(s) have not been inserted."
            )
        });
        push(K::UnableToInsertToNlaStack, &|n| {
            format!("Due to the NLA stack setup, {n} keyframe(s) have not been inserted.")
        });
        push(K::IdNotEditable, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because they are not editable."
            )
        });
        push(K::IdNotAnimatable, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because they cannot be animated."
            )
        });
        push(K::CannotResolvePath, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because the RNA path wasn't valid for them."
            )
        });
        push(K::NoValidLayer, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because there were no layers that could accept the keys."
            )
        });
        push(K::NoValidStrip, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because there were no strips that could accept the keys."
            )
        });
        push(K::NoValidSlot, &|n| {
            format!(
                "Inserting keys on {n} data-block(s) has been skipped because of missing action slots."
            )
        });
        match errors.len() {
            0 => (
                ReportType::Warning,
                "Encountered unhandled error during keyframing".into(),
            ),
            1 => (level, errors.remove(0)),
            _ => {
                let mut s = String::from("Inserting keyframes failed:");
                for e in errors {
                    s.push_str("\n- ");
                    s.push_str(&e);
                }
                (level, s)
            }
        }
    }
}

/// `RNAPath`: an ID-relative path and an optional array index (`None`: every element).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPath {
    /// `path`
    pub path: String,
    /// `index`
    pub index: Option<i32>,
}

impl KeyPath {
    /// Every element of `path`.
    pub fn all(path: impl Into<String>) -> KeyPath {
        KeyPath {
            path: path.into(),
            index: None,
        }
    }
}

/// `id->name` (`OBCube`): the two-letter code and the name, for report texts.
pub fn id_code_name(main: &Main, root: Root) -> String {
    match root {
        Root::Id(a) => format!(
            "{}{}",
            a.code.as_str(),
            main.header(a).map(|h| h.name.as_str()).unwrap_or_default()
        ),
        Root::Embedded { owner, slot } => format!(
            "NT{}",
            main.embedded_header(owner, slot)
                .map(|h| h.name.as_str())
                .unwrap_or_default()
        ),
        _ => String::new(),
    }
}

/// The `ActionSlot.target_id_type` of the datablock at `root`.
fn id_type(root: Root) -> &'static str {
    match root {
        Root::Id(a) => frap_anim::action::id_type_of_prefix(a.code.as_str()),
        _ => "NODETREE",
    }
}

/// The `AnimData` storage of `root`, mutably (`None`: the type has no animation data in frap;
/// the same types as `frap_eval::animate::anim_data` reads).
pub fn adt_slot_mut<'m>(
    main: &'m mut Main,
    log: &mut ChangeLog,
    root: Root,
) -> Option<&'m mut Option<AnimData>> {
    fn typed<K: frap_rna::IdKind>(a: IdAny) -> Option<IdRef<K>> {
        a.typed()
    }
    macro_rules! adt {
        ($coll:ident, $a:expr) => {
            main.$coll
                .get_mut(typed($a)?, log)
                .map(|x| &mut x.animation_data)
        };
    }
    match root {
        Root::Id(a) => match a.code {
            IdCode::OB => adt!(objects, a),
            IdCode::MA => adt!(materials, a),
            IdCode::CA => adt!(cameras, a),
            IdCode::LA => adt!(lights, a),
            IdCode::WO => adt!(worlds, a),
            IdCode::IM => adt!(images, a),
            IdCode::AR => adt!(armatures, a),
            IdCode::SC => adt!(scenes, a),
            IdCode::KE => adt!(shape_keys, a),
            IdCode::NT => adt!(node_groups, a),
            IdCode::TE => adt!(textures, a),
            IdCode::PT => adt!(pointclouds, a),
            _ => None,
        },
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => main
                .materials
                .get_mut(typed(owner)?, log)
                .map(|x| &mut x.node_tree.animation_data),
            IdCode::WO => main
                .worlds
                .get_mut(typed(owner)?, log)
                .map(|x| &mut x.node_tree.animation_data),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `root` can carry animation data (`id_can_have_animdata`).
pub fn can_have_animdata(main: &Main, root: Root) -> bool {
    match root {
        Root::Id(a) => {
            matches!(
                a.code,
                IdCode::OB
                    | IdCode::MA
                    | IdCode::CA
                    | IdCode::LA
                    | IdCode::WO
                    | IdCode::IM
                    | IdCode::AR
                    | IdCode::SC
                    | IdCode::KE
                    | IdCode::NT
                    | IdCode::TE
                    | IdCode::PT
            ) && main.header(a).is_some()
        }
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => matches!(owner.code, IdCode::MA | IdCode::WO) && main.header(owner).is_some(),
        _ => false,
    }
}

/// `find_related_ids` (the IDs whose action a new key may share, closest first).
fn related_ids(main: &Main, root: Root) -> Vec<Root> {
    let users = main.user_counts();
    let real_users = |a: IdAny| users.get(&a).copied().unwrap_or(0);
    let mut out = vec![root];
    let mut i = 0;
    let push = |out: &mut Vec<Root>, r: Root| {
        if !out.contains(&r) {
            out.push(r);
        }
    };
    while i < out.len() {
        let cur = out[i];
        i += 1;
        let id = match cur {
            Root::Embedded { owner, .. } => {
                push(&mut out, Root::Id(owner));
                continue;
            }
            Root::Id(a) => a,
            _ => continue,
        };
        match id.code {
            IdCode::OB => {
                let data = id
                    .typed()
                    .and_then(|o| main.objects.get(o))
                    .and_then(|o| o.data);
                if let Some(d) = data
                    && real_users(d) == 1
                {
                    push(&mut out, Root::Id(d));
                }
            }
            IdCode::KE => {
                let from = id
                    .typed()
                    .and_then(|k| main.shape_keys.get(k))
                    .and_then(|k| k.user);
                if let Some(m) = from {
                    push(&mut out, Root::Id(m.any()));
                }
            }
            // A material relates to its node tree only when the tree has one real user;
            // embedded trees have none, so a material's own keys never share the tree's action
            // (verified: `MaterialAction.001`). The same holds for a world's tree.
            IdCode::MA => {}
            _ => {
                // Objects using this ID as their data, when it has one user.
                if real_users(id) == 1 {
                    for (o, ob) in main.objects.iter() {
                        if ob.data == Some(id) {
                            push(&mut out, Root::Id(o.any()));
                        }
                    }
                }
                if id.code == IdCode::ME
                    && let Some(k) = id
                        .typed()
                        .and_then(|m| main.meshes.get(m))
                        .and_then(|m| m.shape_keys)
                {
                    push(&mut out, Root::Id(k.any()));
                }
            }
        }
    }
    out
}

/// `find_related_action`: the first editable action of a related ID.
fn find_related_action(main: &Main, root: Root) -> Option<IdRef<ActionId>> {
    related_ids(main, root).into_iter().find_map(|r| {
        let a = anim_data(main, r)?.action?;
        let act = main.actions.get(a)?;
        act.header.library.is_none().then_some(a)
    })
}

/// The name `id_action_ensure` gives a new action: `<name>Action` of the ID, of the owner of
/// an embedded ID, or of the mesh a shape-key datablock belongs to.
fn new_action_name(main: &Main, root: Root) -> String {
    let name = match root {
        Root::Embedded { owner, .. } => main.header(owner).map(|h| h.name.clone()),
        Root::Id(a) if a.code == IdCode::KE => {
            let k = a.typed().and_then(|k| main.shape_keys.get(k));
            match k.and_then(|k| k.user) {
                Some(m) => main.header(m.any()).map(|h| h.name.clone()),
                None => k.map(|k| k.header.name.clone()),
            }
        }
        Root::Id(a) => main.header(a).map(|h| h.name.clone()),
        _ => None,
    };
    format!("{}Action", name.unwrap_or_default())
}

/// The animated ID's name with its two-letter code, as slots name it (`OBCube`).
fn slot_id_name(main: &Main, root: Root) -> String {
    id_code_name(main, root)
}

/// `generic_assign_action` for an `AnimData` that has none (`id_action_ensure`).
fn assign_action(main: &mut Main, log: &mut ChangeLog, root: Root, a: IdRef<ActionId>) {
    let ty = id_type(root);
    let name = slot_id_name(main, root);
    let last = anim_data(main, root)
        .map(|d| d.last_slot_identifier.clone())
        .unwrap_or_default();
    let slot = main
        .actions
        .get(a)
        .and_then(|act| act.slot_for_autoassign(ty, &name, &last));
    let mut identifier = None;
    if let Some(h) = slot
        && let Some(act) = main.actions.get_mut(a, log)
    {
        act.slot_setup_for_id(h, ty);
        identifier = act.slot(h).map(|s| s.identifier.clone());
    }
    if let Some(Some(adt)) = adt_slot_mut(main, log, root) {
        adt.action = Some(a);
        if let (Some(h), Some(id)) = (slot, identifier) {
            adt.action_slot_handle = h;
            adt.last_slot_identifier = id;
        }
    }
}

/// `id_action_ensure` + `assign_action_ensure_slot_for_keying` + `layer_keystrip_ensure`:
/// the action and slot handle keys go to.
fn prepare_action(main: &mut Main, log: &mut ChangeLog, root: Root) -> (IdRef<ActionId>, i32) {
    let ty = id_type(root);
    if anim_data(main, root).and_then(|d| d.action).is_none() {
        let a = match find_related_action(main, root) {
            Some(a) => a,
            None => {
                let name = new_action_name(main, root);
                main.actions.insert(&name, Action::new(name.as_str()), log)
            }
        };
        assign_action(main, log, root, a);
    }
    let adt = anim_data(main, root).cloned().unwrap_or_default();
    let a = adt.action.expect("invariant: an action was just ensured");
    let name = slot_id_name(main, root);
    let act = main.actions.get(a).expect("invariant: the action exists");
    let mut slot = act.slot(adt.action_slot_handle).map(|s| s.handle);
    if slot.is_none() {
        slot = match act.slots.as_slice() {
            [only] if !only.has_idtype() => Some(only.handle),
            _ => None,
        };
    }
    let suitable = slot
        .and_then(|h| act.slot(h))
        .is_some_and(|s| s.is_suitable_for(ty));
    let act = main
        .actions
        .get_mut(a, log)
        .expect("invariant: the action exists");
    let handle = match slot {
        Some(h) if suitable => h,
        _ => act.slot_add_for_id(ty, &name, &adt.last_slot_identifier),
    };
    act.slot_setup_for_id(handle, ty);
    let identifier = act
        .slot(handle)
        .map(|s| s.identifier.clone())
        .unwrap_or_default();
    // `layer_keystrip_ensure`
    if act.layers.is_empty() {
        act.layers.push(frap_anim::ActionLayer::new("Layer"));
    }
    if act.layers[0].strips.is_empty() {
        act.layers[0]
            .strips
            .push(ActionStrip::Keyframe(frap_anim::KeyframeStrip::default()));
    }
    if let Some(Some(d)) = adt_slot_mut(main, log, root)
        && d.action_slot_handle != handle
    {
        d.action_slot_handle = handle;
        d.last_slot_identifier = identifier;
    }
    (a, handle)
}

/// One resolved property to key.
struct Resolved {
    /// The ID-relative path (`RNA_path_from_ID_to_property`).
    path: String,
    /// `get_rna_values`
    values: Vec<f32>,
    /// The property type's F-curve flags (`fcurve_flags_for_property_type`).
    discrete: bool,
    int: bool,
    /// The colour mode a new F-curve takes (`create_fcurve_for_channel` with the factory
    /// "XYZ to RGB" preference).
    color_mode: Option<&'static str>,
    /// `default_channel_group_for_path`
    default_group: Option<SmolStr>,
}

/// `get_rna_values`: a property's values as floats (booleans 0/1, enums their item value).
pub fn rna_values(prop: &PropDef, v: &Value) -> Vec<f32> {
    match v {
        Value::Bool(b) => vec![f32::from(u8::from(*b))],
        Value::Int(i) => vec![*i as f32],
        Value::Float(f) => vec![*f],
        Value::Double(d) => vec![*d as f32],
        Value::BoolArray(a) => a.iter().map(|b| f32::from(u8::from(*b))).collect(),
        Value::IntArray(a) => a.iter().map(|i| *i as f32).collect(),
        Value::FloatArray { data, .. } => data.to_vec(),
        Value::Enum(e) => match &prop.kind {
            PropKind::Enum { items, .. } => items
                .iter()
                .find(|i| i.identifier == e.as_str())
                .map(|i| vec![i.value as f32])
                .unwrap_or_default(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Resolves `path` from `root` (`RNA_path_resolve_property` + `get_property_values`).
fn resolve(read: &dyn HostRead, root: Root, path: &str) -> Option<Resolved> {
    let segs = parse_path(path)?;
    with_path(read, root, &segs, &mut |leaf| {
        let Leaf::Prop {
            owner,
            ptr,
            prop,
            value,
        } = leaf
        else {
            return None;
        };
        let full = full_path(ptr, prop.identifier)?;
        let (discrete, int) = match prop.kind {
            PropKind::Float { .. } => (false, false),
            PropKind::Int { .. } => (false, true),
            _ => (true, true),
        };
        let color_mode = match prop.subtype {
            Subtype::Translation
            | Subtype::Xyz
            | Subtype::Euler
            | Subtype::Color
            | Subtype::Coordinates => Some("AUTO_RGB"),
            Subtype::Quaternion => Some("AUTO_YRGB"),
            _ => None,
        };
        let srna = owner.srna();
        let default_group = if srna.is_a("PoseBone") {
            ptr.path.last().and_then(|s| match s {
                frap_rna::Seg::Key(k) => Some(k.clone()),
                _ => None,
            })
        } else if srna.is_a("Object")
            && (full.contains("location") || full.contains("rotation") || full.contains("scale"))
        {
            Some("Object Transforms".into())
        } else {
            None
        };
        Some(Resolved {
            path: full,
            values: rna_values(prop, &value),
            discrete,
            int,
            color_mode,
            default_group,
        })
    })
}

/// The ID-relative path of property `prop` of the struct at `ptr`.
fn full_path(ptr: &RnaPtr, prop: &str) -> Option<String> {
    let base = ptr.path_from_id()?;
    Some(if base.is_empty() {
        prop.to_owned()
    } else {
        format!("{base}.{prop}")
    })
}

/// `insert_keyframes(bmain, struct_pointer = the ID at root, channel_group, rna_paths, …)`.
pub fn insert_keyframes(
    h: &mut dyn KeyingHost,
    root: Root,
    group: Option<&str>,
    paths: &[KeyPath],
    frame: f32,
    keytype: KeyframeType,
    flags: KeyFlags,
) -> CombinedKeyingResult {
    let mut result = CombinedKeyingResult::default();
    // Read phase: resolve every path before the data changes.
    let resolved: Vec<Option<Resolved>> = {
        let read = h.read();
        if !can_have_animdata(read.main(), root) {
            result.add(KeyingResult::IdNotAnimatable, 1);
            return result;
        }
        paths.iter().map(|p| resolve(read, root, &p.path)).collect()
    };
    let (main, log) = h.write();
    // `BKE_animdata_ensure_id`
    let has_action = match adt_slot_mut(main, log, root) {
        Some(slot) => slot.get_or_insert_with(AnimData::default).action,
        None => {
            result.add(KeyingResult::IdNotAnimatable, 1);
            return result;
        }
    };
    if has_action.is_none() && flags.available {
        result.add(KeyingResult::CannotCreateFcurve, paths.len());
        return result;
    }
    if let Some(a) = has_action
        && main
            .actions
            .get(a)
            .is_some_and(|x| x.header.library.is_some())
    {
        result.add(KeyingResult::IdNotEditable, paths.len());
        return result;
    }
    let (a, handle) = prepare_action(main, log, root);
    let settings = KeyframeSettings {
        keyframe_type: keytype,
        ..KeyframeSettings::default()
    };
    let insert_flags = InsertFlags {
        replace: flags.replace,
        needed: flags.needed,
        fast: false,
    };
    for (p, r) in paths.iter().zip(resolved) {
        let Some(r) = r else {
            result.add(KeyingResult::CannotResolvePath, 1);
            continue;
        };
        let group = group.map(SmolStr::from).or(r.default_group.clone());
        let act = main
            .actions
            .get_mut(a, log)
            .expect("invariant: the action exists");
        for (i, value) in r.values.iter().enumerate() {
            if !matches!(p.index, None | Some(-1)) && p.index != Some(i as i32) {
                result.add(KeyingResult::UnableToInsertToNlaStack, 1);
                continue;
            }
            let res = key_one(
                act,
                handle,
                &r,
                i as i32,
                group.as_deref(),
                [frame, *value],
                &settings,
                insert_flags,
                flags,
            );
            result.add(res, 1);
        }
    }
    result
}

/// `StripKeyframeData::keyframe_insert` on the first strip of the first layer.
#[allow(clippy::too_many_arguments)]
fn key_one(
    act: &mut Action,
    handle: i32,
    r: &Resolved,
    index: i32,
    group: Option<&str>,
    position: [f32; 2],
    settings: &KeyframeSettings,
    insert_flags: InsertFlags,
    flags: KeyFlags,
) -> KeyingResult {
    let Some(layer) = act.layers.first_mut() else {
        return KeyingResult::NoValidLayer;
    };
    let Some(ActionStrip::Keyframe(strip)) = layer.strips.first_mut() else {
        return KeyingResult::NoValidStrip;
    };
    let (bag, f) = if flags.may_create_fcurve() {
        let b = strip.channelbag_ensure(handle);
        let bag = &mut strip.channelbags[b];
        let f = match bag.fcurve_find(&r.path, index) {
            Some(f) => f,
            None => {
                let f = bag.fcurve_create(&r.path, index, group);
                init_fcurve(&mut bag.fcurves[f], r);
                f
            }
        };
        (b, f)
    } else {
        let Some(b) = strip.channelbag_index(handle) else {
            return KeyingResult::CannotCreateFcurve;
        };
        match strip.channelbags[b].fcurve_find(&r.path, index) {
            Some(f) => (b, f),
            None => return KeyingResult::CannotCreateFcurve,
        }
    };
    let bag = &mut strip.channelbags[bag];
    let group_locked = bag.fcurves[f]
        .group
        .as_ref()
        .and_then(|g| bag.groups.iter().find(|x| &x.name == g))
        .is_some_and(|g| g.lock);
    let fc = &mut bag.fcurves[f];
    if fc.lock || group_locked {
        return KeyingResult::FcurveNotKeyframeable;
    }
    match insert_vert(fc, position, settings, insert_flags) {
        InsertResult::Success(_) => {
            // `update_autoflags_fcurve_direct`
            fc.discrete_values = r.discrete;
            fc.int_values = r.int;
            KeyingResult::Success
        }
        InsertResult::NoKeyNeeded => KeyingResult::NoKeyNeeded,
        InsertResult::Failure => KeyingResult::UnknownFailure,
    }
}

/// `create_fcurve_for_channel`: the property type's flags and the colour mode.
fn init_fcurve(fc: &mut FCurve, r: &Resolved) {
    fc.discrete_values = r.discrete;
    fc.int_values = r.int;
    if let Some(c) = r.color_mode {
        fc.color_mode = c.into();
    }
}

/// `BKE_report(RPT_ERROR, …)` as the Python API raises it.
pub fn report_error(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Error: {msg}\n"),
    )
}

/// The F-curves of slot `handle` of `act` in every keyframe strip (`fcurves_for_action_slot`):
/// `(layer, strip, channelbag, fcurve)` indices.
fn slot_fcurves(act: &Action, handle: i32) -> Vec<(usize, usize, usize, usize)> {
    let mut out = Vec::new();
    for (l, layer) in act.layers.iter().enumerate() {
        for (s, strip) in layer.strips.iter().enumerate() {
            let Some(k) = strip.keyframe() else { continue };
            for (b, bag) in k.channelbags.iter().enumerate() {
                if bag.slot_handle != handle {
                    continue;
                }
                for f in 0..bag.fcurves.len() {
                    out.push((l, s, b, f));
                }
            }
        }
    }
    out
}

/// `BKE_fcurve_is_protected`
fn is_protected(bag: &frap_anim::Channelbag, fc: &FCurve) -> bool {
    fc.lock
        || fc
            .group
            .as_ref()
            .and_then(|g| bag.groups.iter().find(|x| &x.name == g))
            .is_some_and(|g| g.lock)
}

/// The array length of the property at `path` from `root` (0 for scalars), `None` when the
/// path does not resolve.
fn array_length(read: &dyn HostRead, root: Root, path: &str) -> Option<usize> {
    let segs = parse_path(path)?;
    with_path(read, root, &segs, &mut |leaf| match leaf {
        Leaf::Prop { prop, value, .. } => Some(match value {
            Value::BoolArray(a) => a.len(),
            Value::IntArray(a) => a.len(),
            Value::FloatArray { data, .. } => data.len(),
            _ if matches!(prop.array, frap_rna::ArrayShape::Scalar) => 0,
            _ => prop.array_len(),
        }),
        Leaf::Struct { .. } => None,
    })
}

/// `delete_keyframe(bmain, reports, id, rna_path, cfra)`: the number of F-curves a key was
/// removed from; `Err` is the `RPT_ERROR` report.
pub fn delete_keyframe(
    h: &mut dyn KeyingHost,
    root: Root,
    path: &str,
    index: Option<i32>,
    frame: f32,
) -> Result<usize, String> {
    let (len, adt) = {
        let read = h.read();
        let main = read.main();
        let Some(adt) = anim_data(main, root).cloned() else {
            return Err("No ID block and/or AnimData to delete keyframe from".into());
        };
        let Some(len) = array_length(read, root, path) else {
            return Err(format!(
                "Could not delete keyframe, as RNA path is invalid for the given ID (ID = {}, path = {path})",
                id_code_name(main, root)
            ));
        };
        (len, adt)
    };
    let Some(a) = adt.action else {
        let (main, _) = h.write();
        return Err(format!(
            "No action to delete keyframes from for ID = {}",
            id_code_name(main, root)
        ));
    };
    let (mut i, mut max) = (index.unwrap_or(0), index.unwrap_or(0) + 1);
    if index.is_none() {
        max = len as i32;
        if max == i {
            max += 1;
        }
    }
    let (main, log) = h.write();
    let Some(act) = main.actions.get_mut(a, log) else {
        return Ok(0);
    };
    let curves = slot_fcurves(act, adt.action_slot_handle);
    let mut modified: Vec<(usize, usize, usize, SmolStr, i32)> = Vec::new();
    while i < max {
        let found = curves.iter().find(|&&(l, s, b, f)| {
            frap_anim::rna::action::bag(act, l, s, b)
                .and_then(|x| x.fcurves.get(f))
                .is_some_and(|fc| fc.data_path == path && fc.array_index == i)
        });
        if let Some(&(l, s, b, f)) = found
            && let Some(bag) = frap_anim::rna::action::bag_mut(act, l, s, b)
        {
            let protected = is_protected(bag, &bag.fcurves[f]);
            let fc = &mut bag.fcurves[f];
            if !protected && delete_key_at(fc, frame) {
                modified.push((l, s, b, fc.data_path.clone(), fc.array_index));
            }
        }
        i += 1;
    }
    // Empty F-curves are removed (`animdata_fcurve_delete`).
    for (l, s, b, p, idx) in &modified {
        if let Some(bag) = frap_anim::rna::action::bag_mut(act, *l, *s, *b)
            && let Some(f) = bag.fcurve_find(p, *idx)
            && bag.fcurves[f].is_empty()
        {
            bag.fcurve_remove(f);
        }
    }
    Ok(modified.len())
}

/// `fcurve_delete_keyframe_at_time` (protection checked by the caller): removes the key at
/// exactly `time` (`BKE_fcurve_bezt_binarysearch_index` found) and recalculates the handles.
fn delete_key_at(fc: &mut FCurve, time: f32) -> bool {
    let (i, found) = frap_anim::eval::curve::binarysearch_index(&fc.keyframe_points, time);
    if !found {
        return false;
    }
    fc.keyframe_points.remove(i);
    frap_anim::eval::recalc_handles(fc);
    true
}

/// `clear_keyframe(bmain, reports, id, rna_path)`: removes the F-curves of `path` (every
/// index, or `index`) from the slot; the number removed.
pub fn clear_keyframe(
    h: &mut dyn KeyingHost,
    root: Root,
    path: &str,
    index: Option<i32>,
) -> Result<usize, String> {
    let adt = {
        let read = h.read();
        let main = read.main();
        let Some(adt) = anim_data(main, root).cloned() else {
            return Err("No ID block and/or AnimData to delete keyframe from".into());
        };
        if array_length(read, root, path).is_none() {
            return Err(format!(
                "Could not clear keyframe, as RNA path is invalid for the given ID (ID = {}, path = {path})",
                id_code_name(main, root)
            ));
        }
        adt
    };
    let (main, log) = h.write();
    let Some(a) = adt.action else {
        return Err(format!(
            "No action to delete keyframes from for ID = {}",
            id_code_name(main, root)
        ));
    };
    if adt.action_slot_handle == 0 {
        return Ok(0);
    }
    let Some(act) = main.actions.get_mut(a, log) else {
        return Ok(0);
    };
    Ok(remove_fcurves(act, adt.action_slot_handle, |fc| {
        fc.data_path == path && index.is_none_or(|i| i == fc.array_index)
    }))
}

/// Removes every F-curve of slot `handle` matching `pred` (`action_fcurve_remove`); the count.
pub fn remove_fcurves(act: &mut Action, handle: i32, pred: impl Fn(&FCurve) -> bool) -> usize {
    let mut n = 0;
    for layer in &mut act.layers {
        for strip in &mut layer.strips {
            let Some(k) = strip.keyframe_mut() else {
                continue;
            };
            for bag in k.channelbags.iter_mut().filter(|b| b.slot_handle == handle) {
                let mut f = 0;
                while f < bag.fcurves.len() {
                    if pred(&bag.fcurves[f]) {
                        bag.fcurve_remove(f);
                        n += 1;
                    } else {
                        f += 1;
                    }
                }
            }
        }
    }
    n
}

/// The parsed arguments of `bpy_struct.keyframe_insert/delete` (`pyrna_struct_keyframe_parse`).
pub struct StructArgs {
    /// The ID the path is relative to.
    pub root: Root,
    /// `path_full`
    pub path: String,
    /// The array index (`-1`: all; 0 for a scalar).
    pub index: i32,
}

/// `pyrna_struct_anim_args_parse_ex`: validates `data_path` (and `index`) on the struct `this`
/// and returns the ID-relative path. Errors are Blender's `TypeError` / `ValueError`.
pub fn parse_struct_args(
    read: &dyn HostRead,
    this: &RnaPtr,
    prefix: &str,
    data_path: &str,
    index: i32,
) -> Result<StructArgs, RnaError> {
    let type_err = |m: String| RnaError::new(RnaErrorKind::Other, PyExc::TypeError, m);
    let value_err = |m: String| RnaError::new(RnaErrorKind::Other, PyExc::ValueError, m);
    let root = match this.root {
        Root::Id(_) | Root::Embedded { .. } => this.root,
        _ => {
            return Err(type_err(format!(
                "{prefix} this struct has no data, cannot be animated"
            )));
        }
    };
    let is_idbase = this.path.is_empty();
    let not_found = || type_err(format!("{prefix} property \"{data_path}\" not found"));
    let (prop, value, full) = if is_idbase {
        let segs = parse_path(data_path).ok_or_else(not_found)?;
        // `RNA_path_resolve_property_full`: a trailing index is reported separately.
        if let Some(PathSeg::Index(_)) = segs.last()
            && segs.len() > 1
            && read_prop(read, root, &segs[..segs.len() - 1]).is_some()
        {
            return Err(value_err(format!(
                "{prefix} path includes index, must be a separate argument"
            )));
        }
        // `RNA_path_resolve_property_full` follows ID pointers: a property of another ID
        // (`data.name` on an object) "spans ID blocks"; a path that resolves nowhere
        // (`data.lens` on a mesh object) is not found (both verified).
        let (_, prop, value) = read_prop(read, root, &segs).ok_or_else(|| {
            let spans = (1..segs.len()).any(|i| {
                frap_eval::animate::id_pointer_at(read, root, &segs[..i])
                    .is_some_and(|id| read_prop(read, Root::Id(id), &segs[i..]).is_some())
            });
            if spans {
                value_err(format!("{prefix} path spans ID blocks"))
            } else {
                unread(read, root, &segs, prefix, data_path).unwrap_or_else(not_found)
            }
        })?;
        (prop, value, data_path.to_owned())
    } else {
        let prop = this.srna.find_prop(data_path).ok_or_else(not_found)?;
        let segs = parse_path(&this.path_from_id().unwrap_or_default())
            .unwrap_or_default()
            .into_iter()
            .chain([PathSeg::Prop(prop.identifier.into())])
            .collect::<Vec<_>>();
        let (ptr, prop, value) = read_prop(read, root, &segs).ok_or_else(|| {
            unread(read, root, &segs, prefix, data_path).unwrap_or_else(not_found)
        })?;
        let full = full_path(&ptr, prop.identifier)
            .ok_or_else(|| type_err(format!("{prefix} could not make path to \"{data_path}\"")))?;
        (prop, value, full)
    };
    if !prop.flags.contains(PropFlags::ANIMATABLE) {
        return Err(type_err(format!(
            "{prefix} property \"{data_path}\" not animatable"
        )));
    }
    let len = match &value {
        Value::BoolArray(a) => Some(a.len()),
        Value::IntArray(a) => Some(a.len()),
        Value::FloatArray { data, .. } => Some(data.len()),
        _ => None,
    };
    let index = match len {
        None if index == -1 => 0,
        None => {
            return Err(type_err(format!(
                "{prefix} index {index} was given while property \"{data_path}\" is not an array"
            )));
        }
        Some(n) if index < -1 || index >= n as i32 => {
            return Err(type_err(format!(
                "{prefix} index out of range \"{data_path}\", given {index}, array length is {n}"
            )));
        }
        Some(_) => index,
    };
    Ok(StructArgs {
        root,
        path: full,
        index,
    })
}

/// Why the property at `segs` (which [`read_prop`] does not reach) cannot be keyed, when the
/// struct before it is reached and Blender has the property: a pointer or collection is "not
/// animatable" (`RNA_property_animateable`, verified on 5.2.4 for `data`, `parent`,
/// `modifiers`, `animation_data`); a value property frap does not bind yet
/// (`lock_location`, `pass_index`, …) is `RNA_NOT_IMPLEMENTED`, not "not found".
fn unread(
    read: &dyn HostRead,
    root: Root,
    segs: &[PathSeg],
    prefix: &str,
    data_path: &str,
) -> Option<RnaError> {
    let (PathSeg::Prop(name), parent) = segs.split_last()? else {
        return None;
    };
    let (srna, prop) = with_path(read, root, parent, &mut |leaf| match leaf {
        Leaf::Struct { rna, .. } => {
            let srna = rna.srna();
            srna.find_prop(name).map(|p| (srna, p))
        }
        Leaf::Prop { .. } => None,
    })?;
    Some(match prop.kind {
        PropKind::Pointer { .. } | PropKind::Collection { .. } => RnaError::new(
            RnaErrorKind::Other,
            PyExc::TypeError,
            format!("{prefix} property \"{data_path}\" not animatable"),
        ),
        _ => RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::NotImplementedError,
            format!(
                "frap: {}.{} exists in Blender 5.2 but is not bound by frap yet",
                srna.identifier, prop.identifier
            ),
        ),
    })
}

/// The property at `segs` from `root`, with its owner's pointer and value.
fn read_prop(
    read: &dyn HostRead,
    root: Root,
    segs: &[PathSeg],
) -> Option<(RnaPtr, &'static PropDef, Value)> {
    with_path(read, root, segs, &mut |leaf| match leaf {
        Leaf::Prop {
            ptr, prop, value, ..
        } => Some((ptr.clone(), prop, value)),
        Leaf::Struct { .. } => None,
    })
}

/// What the 3D Viewport's keying operators key: selected objects (Object Mode) or selected
/// pose bones (Pose Mode), with the paths and group of their transform channels.
pub mod targets {
    use frap_rna::Root;
    use frap_scene::{Context, ContextMode};

    /// One keyable struct (`PointerRNA` of the selection).
    #[derive(Clone, Debug, PartialEq)]
    pub struct Target {
        /// The owning ID.
        pub root: Root,
        /// `path_from_id()` of the struct (`""` for the object itself).
        pub base: String,
        /// Its `rotation_mode`.
        pub rotation_mode: String,
        /// `get_transform_generators_base_info`'s grouping (`Object Transforms`, the bone).
        pub group: String,
    }

    impl Target {
        /// `path_add_property(base, prop)`
        pub fn path(&self, prop: &str) -> String {
            if self.base.is_empty() {
                prop.to_owned()
            } else {
                format!("{}.{prop}", self.base)
            }
        }

        /// The rotation property its `rotation_mode` keys.
        pub fn rotation_path(&self) -> String {
            self.path(match self.rotation_mode.as_str() {
                "QUATERNION" => "rotation_quaternion",
                "AXIS_ANGLE" => "rotation_axis_angle",
                _ => "rotation_euler",
            })
        }
    }

    /// `get_selection` of `anim.keyframe_insert`: `None` for an unsupported mode.
    pub fn selection(c: &Context) -> Option<Vec<Target>> {
        match c.mode() {
            ContextMode::Object => Some(objects(c)),
            ContextMode::Pose => Some(bones(c)),
            _ => None,
        }
    }

    /// `context.selected_objects` as targets.
    pub fn objects(c: &Context) -> Vec<Target> {
        c.selected_objects()
            .into_iter()
            .filter_map(|o| {
                let ob = c.main.objects.get(o)?;
                Some(Target {
                    root: Root::Id(o.any()),
                    base: String::new(),
                    rotation_mode: ob.rotation_mode.as_str().to_owned(),
                    group: "Object Transforms".into(),
                })
            })
            .collect()
    }

    /// `context.selected_pose_bones` as targets.
    pub fn bones(c: &Context) -> Vec<Target> {
        c.selected_pose_bones()
            .into_iter()
            .filter_map(|b| {
                let ob = c.main.objects.get(b.object)?;
                let pb = ob.pose.as_ref()?.bones.iter().find(|x| x.name == b.bone)?;
                Some(Target {
                    root: Root::Id(b.object.any()),
                    base: format!("pose.bones{}", frap_rna::ptr::rna_key_repr(&b.bone)),
                    rotation_mode: pb.rotation_mode.as_str().to_owned(),
                    group: b.bone.to_string(),
                })
            })
            .collect()
    }

    /// `RKS_ITER_selected_item`'s items: the selected pose bones of a posed active object,
    /// else the selected objects.
    pub fn keying_set_items(c: &Context) -> Vec<Target> {
        let posed = c
            .active_object()
            .and_then(|o| c.main.objects.get(o))
            .is_some_and(|o| o.mode.as_str() == "POSE");
        if posed { bones(c) } else { objects(c) }
    }
}

/// The built-in keying sets of `keyingsets_builtins.py` (Blender 5.2.4) that key transforms.
pub mod keyingsets {
    use super::targets::{Target, keying_set_items};
    use frap_scene::Context;

    /// A built-in keying set.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct KeyingSetInfo {
        /// `bl_idname`
        pub idname: &'static str,
        /// `bl_label` (the keying set's `name`)
        pub label: &'static str,
        /// The channels it keys (`RKS_GEN_*`).
        pub channels: &'static [Channel],
        /// `bl_options = {'INSERTKEY_VISUAL'}`
        pub visual: bool,
        /// `RKS_POLL_selected_objects` (the delta sets): objects only.
        pub objects_only: bool,
    }

    /// One `RKS_GEN_*` generator.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum Channel {
        /// `RKS_GEN_location`
        Location,
        /// `RKS_GEN_rotation`
        Rotation,
        /// `RKS_GEN_scaling`
        Scale,
        /// `delta_location`
        DeltaLocation,
        /// `delta_rotation_*`
        DeltaRotation,
        /// `delta_scale`
        DeltaScale,
        /// `RKS_GEN_available`
        Available,
    }

    use Channel::*;

    macro_rules! ks {
        ($id:literal, $label:literal, [$($c:expr),*], $visual:expr, $obj:expr) => {
            KeyingSetInfo { idname: $id, label: $label, channels: &[$($c),*], visual: $visual, objects_only: $obj }
        };
    }

    /// The transform keying sets, in registration order.
    pub const BUILTIN: &[KeyingSetInfo] = &[
        ks!("Location", "Location", [Location], false, false),
        ks!("Rotation", "Rotation", [Rotation], false, false),
        ks!("Scaling", "Scale", [Scale], false, false),
        ks!(
            "BUILTIN_KSI_LocRot",
            "Location & Rotation",
            [Location, Rotation],
            false,
            false
        ),
        ks!(
            "BUILTIN_KSI_LocScale",
            "Location & Scale",
            [Location, Scale],
            false,
            false
        ),
        ks!(
            "LocRotScale",
            "Location, Rotation & Scale",
            [Location, Rotation, Scale],
            false,
            false
        ),
        ks!(
            "LocRotScaleCProp",
            "Location, Rotation, Scale & Custom Properties",
            [Location, Rotation, Scale],
            false,
            false
        ),
        ks!(
            "BUILTIN_KSI_RotScale",
            "Rotation & Scale",
            [Rotation, Scale],
            false,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualLoc",
            "Visual Location",
            [Location],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualRot",
            "Visual Rotation",
            [Rotation],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualScaling",
            "Visual Scale",
            [Scale],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualLocRot",
            "Visual Location & Rotation",
            [Location, Rotation],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualLocScale",
            "Visual Location & Scale",
            [Location, Scale],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualLocRotScale",
            "Visual Location, Rotation & Scale",
            [Location, Rotation, Scale],
            true,
            false
        ),
        ks!(
            "BUILTIN_KSI_VisualRotScale",
            "Visual Rotation & Scale",
            [Rotation, Scale],
            true,
            false
        ),
        ks!("Available", "Available", [Available], false, true),
        ks!(
            "BUILTIN_KSI_DeltaLocation",
            "Delta Location",
            [DeltaLocation],
            false,
            true
        ),
        ks!(
            "BUILTIN_KSI_DeltaRotation",
            "Delta Rotation",
            [DeltaRotation],
            false,
            true
        ),
        ks!(
            "BUILTIN_KSI_DeltaScale",
            "Delta Scale",
            [DeltaScale],
            false,
            true
        ),
    ];

    /// The keying sets frap knows the names of but does not implement (B-Bone shapes, whole
    /// character).
    pub const UNSUPPORTED: &[&str] = &[
        "BUILTIN_KSI_BendyBones",
        "WholeCharacter",
        "WholeCharacterSelected",
    ];

    /// The `poll` of a built-in keying set (`_keyingsets_utils.RKS_POLL_*`, Blender 5.2.4).
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum Poll {
        /// `RKS_POLL_selected_items` / `RKS_POLL_selected_bones_or_objects` (no sequencer
        /// strips in the 3D Viewport).
        Items,
        /// `RKS_POLL_selected_objects`
        Objects,
        /// `RKS_POLL_selected_bones`
        Bones,
        /// `BUILTIN_KSI_Available.poll`: `bool(context.selected_objects)`.
        Available,
        /// `WholeCharacterMixin.poll`: the active object is in Pose Mode.
        PoseMode,
    }

    // Ported from `classes` of scripts/startup/keyingsets_builtins.py (Blender 5.2.4,
    // GPL-2.0-or-later): registration order (the enum order), labels, docstrings, polls.
    /// Every built-in keying set in `ANIM_keying_sets_enum_itemf` order: idname, name,
    /// description, poll.
    const ENUM_ORDER: &[(&str, &str, &str, Poll)] = &[
        (
            "Available",
            "Available",
            "Insert a keyframe on each of the already existing F-Curves",
            Poll::Available,
        ),
        (
            "Location",
            "Location",
            "Insert a keyframe on each of the location channels",
            Poll::Items,
        ),
        (
            "Rotation",
            "Rotation",
            "Insert a keyframe on each of the rotation channels",
            Poll::Items,
        ),
        (
            "Scaling",
            "Scale",
            "Insert a keyframe on each of the scale channels",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_LocRot",
            "Location & Rotation",
            "Insert a keyframe on each of the location and rotation channels",
            Poll::Items,
        ),
        (
            "LocRotScale",
            "Location, Rotation & Scale",
            "Insert a keyframe on each of the location, rotation, and scale channels",
            Poll::Items,
        ),
        (
            "LocRotScaleCProp",
            "Location, Rotation, Scale & Custom Properties",
            "Key location/rotation/scale as well as custom properties",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_LocScale",
            "Location & Scale",
            "Insert a keyframe on each of the location and scale channels",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_RotScale",
            "Rotation & Scale",
            "Insert a keyframe on each of the rotation and scale channels",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_DeltaLocation",
            "Delta Location",
            "Insert keyframes for additional location offset",
            Poll::Objects,
        ),
        (
            "BUILTIN_KSI_DeltaRotation",
            "Delta Rotation",
            "Insert keyframes for additional rotation offset",
            Poll::Objects,
        ),
        (
            "BUILTIN_KSI_DeltaScale",
            "Delta Scale",
            "Insert keyframes for additional scale factor",
            Poll::Objects,
        ),
        (
            "BUILTIN_KSI_VisualLoc",
            "Visual Location",
            "Insert a keyframe on each of the location channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualRot",
            "Visual Rotation",
            "Insert a keyframe on each of the rotation channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualScaling",
            "Visual Scale",
            "Insert a keyframe on each of the scale channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualLocRot",
            "Visual Location & Rotation",
            "Insert a keyframe on each of the location and rotation channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualLocRotScale",
            "Visual Location, Rotation & Scale",
            "Insert a keyframe on each of the location, rotation and scale channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualLocScale",
            "Visual Location & Scale",
            "Insert a keyframe on each of the location and scale channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_VisualRotScale",
            "Visual Rotation & Scale",
            "Insert a keyframe on each of the rotation and scale channels, taking into account effects of constraints and relationships",
            Poll::Items,
        ),
        (
            "BUILTIN_KSI_BendyBones",
            "B-Bone Shape",
            "Insert a keyframe for each of the B-Bone shape properties",
            Poll::Bones,
        ),
        (
            "WholeCharacter",
            "Whole Character",
            "Insert a keyframe for all properties that are likely to get animated in a character rig (useful when blocking out a shot)",
            Poll::PoseMode,
        ),
        (
            "WholeCharacterSelected",
            "Whole Character (Selected Bones Only)",
            "Insert a keyframe for all properties that are likely to get animated in a character rig (only selected bones)",
            Poll::PoseMode,
        ),
    ];

    fn polls(c: &Context, poll: Poll) -> bool {
        let active = c.active_object().and_then(|o| c.main.objects.get(o));
        let posed = active.is_some_and(|o| o.mode.as_str() == "POSE");
        let objects = match active {
            Some(o) => o.mode.as_str() == "OBJECT",
            None => !c.selected_objects().is_empty(),
        };
        let bones =
            posed && (c.active_pose_bone().is_some() || !c.selected_pose_bones().is_empty());
        match poll {
            Poll::Items => bones || objects,
            Poll::Objects => objects,
            Poll::Bones => bones,
            Poll::Available => !c.selected_objects().is_empty(),
            Poll::PoseMode => posed,
        }
    }

    /// `ANIM_keying_sets_enum_itemf` in `c`: the built-in keying sets whose poll passes
    /// (identifier, name, description); frap scenes have no keying sets of their own, so no
    /// `__ACTIVE__` item.
    pub fn enum_items(c: &Context) -> Vec<(&'static str, &'static str, &'static str)> {
        ENUM_ORDER
            .iter()
            .filter(|(.., poll)| polls(c, *poll))
            .map(|(id, name, desc, _)| (*id, *name, *desc))
            .collect()
    }

    /// `ANIM_keyingset_get_from_idname` among the built-in sets.
    pub fn find(idname: &str) -> Option<&'static KeyingSetInfo> {
        BUILTIN.iter().find(|k| k.idname == idname)
    }

    /// One `ks.paths.add(id, path, index, group_method, group_name)`.
    #[derive(Clone, Debug, PartialEq)]
    pub struct KsPath {
        /// The target.
        pub root: frap_rna::Root,
        /// The ID-relative path.
        pub path: String,
        /// `index` (`None`: the whole array).
        pub index: Option<i32>,
        /// The group the F-curves go to.
        pub group: Option<String>,
    }

    /// `validate_keyingset` + the generators: the paths of `ks`, or `None` when its poll
    /// fails (`MODIFYKEY_INVALID_CONTEXT`).
    pub fn paths(c: &Context, ks: &KeyingSetInfo) -> Option<Vec<KsPath>> {
        let posed = c
            .active_object()
            .and_then(|o| c.main.objects.get(o))
            .is_some_and(|o| o.mode.as_str() == "POSE");
        let items: Vec<Target> = if ks.objects_only {
            // `RKS_POLL_selected_objects` / Available's own poll.
            if ks.idname != "Available"
                && let Some(o) = c.active_object().and_then(|o| c.main.objects.get(o))
                && o.mode.as_str() != "OBJECT"
            {
                return None;
            }
            let items = super::targets::objects(c);
            if items.is_empty() {
                return None;
            }
            if ks.idname == "Available" && posed {
                keying_set_items(c)
            } else {
                items
            }
        } else {
            // `RKS_POLL_selected_items`: selected bones in Pose Mode, or objects.
            let bones_ok = posed && !super::targets::bones(c).is_empty();
            let objects_ok = match c.active_object().and_then(|o| c.main.objects.get(o)) {
                Some(o) => o.mode.as_str() == "OBJECT",
                None => !c.selected_objects().is_empty(),
            };
            if !bones_ok && !objects_ok {
                return None;
            }
            keying_set_items(c)
        };
        let mut out = Vec::new();
        for t in &items {
            for ch in ks.channels {
                let mut add = |path: String, index: Option<i32>, group: Option<String>| {
                    out.push(KsPath {
                        root: t.root,
                        path,
                        index,
                        group,
                    });
                };
                let g = Some(t.group.clone());
                match ch {
                    Location => add(t.path("location"), None, g),
                    Rotation => add(t.rotation_path(), None, g),
                    Scale => add(t.path("scale"), None, g),
                    DeltaLocation => add(t.path("delta_location"), None, g),
                    DeltaRotation => add(
                        t.path(match t.rotation_mode.as_str() {
                            "QUATERNION" => "delta_rotation_quaternion",
                            "AXIS_ANGLE" => continue,
                            _ => "delta_rotation_euler",
                        }),
                        None,
                        g,
                    ),
                    DeltaScale => add(t.path("delta_scale"), None, g),
                    Available => {
                        let Some(adt) = frap_eval::animate::anim_data(c.main, t.root) else {
                            continue;
                        };
                        let Some(act) = adt.action.and_then(|a| c.main.actions.get(a)) else {
                            continue;
                        };
                        for layer in &act.layers {
                            for strip in &layer.strips {
                                let Some(k) = strip.keyframe() else { continue };
                                for bag in k
                                    .channelbags
                                    .iter()
                                    .filter(|b| b.slot_handle == adt.action_slot_handle)
                                {
                                    for fc in &bag.fcurves {
                                        if !t.base.is_empty() && !fc.data_path.contains(&t.base) {
                                            continue;
                                        }
                                        add(
                                            fc.data_path.to_string(),
                                            Some(fc.array_index),
                                            Some(ks.label.to_owned()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(out)
    }
}

/// The NLA editing the `nla.*` operators and the `NlaTracks` / `NlaStrips` data API share.
///
/// Ported from blender/source/blender/blenkernel/intern/nla.cc (`BKE_nlatrack_new`,
/// `BKE_nlatrack_insert_after`, `BKE_nlatrack_set_active`, `nlastrip_new`,
/// `nlastrip_set_initial_length`, `BKE_nlastrips_has_space`, `BKE_nlastrips_add_strip_unsafe`,
/// `BKE_nlastack_add_strip`, `BKE_nlastrip_validate_name`, `BKE_nla_action_pushdown`,
/// `BKE_nlastrip_set_active`) and animrig/intern/nla.cc (`assign_action`), Blender 5.2.4,
/// GPL-2.0-or-later. Every frap track is local (`NLATRACK_OVERRIDELIBRARY_LOCAL`).
pub mod nla {
    use frap_anim::{Action, AnimData, NlaStrip, NlaTrack};
    use frap_rna::naming::unique_subname;

    /// `sizeof(NlaTrack::name) - 1`
    const NAME_MAX: usize = 63;

    /// `BKE_nlatrack_new` + `BKE_nlatrack_insert_after(prev)`: a selected, inactive track
    /// named `NlaTrack` (made unique) after track `prev` (`None`: the head); its index.
    pub fn track_new_after(adt: &mut AnimData, prev: Option<usize>) -> usize {
        let name = unique_subname(
            &|n| adt.nla_tracks.iter().any(|t| t.name == n),
            "NlaTrack",
            NAME_MAX,
            "NlaTrack",
        );
        let at = prev.map_or(0, |p| p + 1);
        adt.nla_tracks.insert(
            at,
            NlaTrack {
                active: false,
                ..NlaTrack::new(name)
            },
        );
        at
    }

    /// `BKE_nlatrack_new_tail`
    pub fn track_new_tail(adt: &mut AnimData) -> usize {
        let last = adt.nla_tracks.len().checked_sub(1);
        track_new_after(adt, last)
    }

    /// `BKE_nlatrack_set_active`
    pub fn track_set_active(adt: &mut AnimData, t: usize) {
        for (i, track) in adt.nla_tracks.iter_mut().enumerate() {
            track.active = i == t;
        }
    }

    /// `BKE_nlastrip_set_active`
    pub fn strip_set_active(adt: &mut AnimData, t: usize, s: usize) {
        for (i, track) in adt.nla_tracks.iter_mut().enumerate() {
            for (j, strip) in track.strips.iter_mut().enumerate() {
                strip.active = i == t && j == s;
            }
        }
    }

    /// `Action::get_frame_range_of_slot`: the manual range, or the keys of the slot's
    /// F-curves (`[0, 0]` without keys).
    pub fn slot_frame_range(act: &Action, handle: i32) -> [f32; 2] {
        if act.use_frame_range {
            return act.frame_range;
        }
        let (mut min, mut max, mut found) = (999_999_999.0_f32, -999_999_999.0_f32, false);
        for layer in &act.layers {
            for strip in &layer.strips {
                let Some(k) = strip.keyframe() else { continue };
                for bag in k.channelbags.iter().filter(|b| b.slot_handle == handle) {
                    for fc in &bag.fcurves {
                        if let (Some(f), Some(l)) =
                            (fc.keyframe_points.first(), fc.keyframe_points.last())
                        {
                            min = min.min(f.co[0]);
                            max = max.max(l.co[0]);
                            found = true;
                        }
                    }
                }
            }
        }
        if found {
            [min.max(-1_048_574.0), max.min(1_048_574.0)]
        } else {
            [0.0, 0.0]
        }
    }

    /// `BKE_nlastrip_new(act, id)` (+ `_for_slot` when `slot` is given): an action-clip strip
    /// over the slot's frame range, its slot auto-assigned (`nla::assign_action`: the
    /// identifier match, then the action's only suitable slot) or the given one.
    pub fn strip_new(
        act_ref: frap_rna::IdRef<frap_rna::ActionId>,
        act: &mut Action,
        id_type: &str,
        id_name: &str,
        slot: Option<i32>,
    ) -> NlaStrip {
        let handle = slot.or_else(|| {
            act.slot_for_autoassign(id_type, id_name, "")
                .or_else(|| match act.slots.as_slice() {
                    [only] if only.is_suitable_for(id_type) => Some(only.handle),
                    _ => None,
                })
        });
        let range = slot_frame_range(act, handle.unwrap_or(0));
        let mut strip = NlaStrip::new_clip("", Some(act_ref), range[0], range);
        if let Some(h) = handle {
            act.slot_setup_for_id(h, id_type);
            if let Some(sl) = act.slot(h) {
                strip.action_slot_handle = h;
                strip.last_slot_identifier = sl.identifier.clone();
            }
        }
        // `ACT_FRAME_RANGE` disables length syncing; cyclic actions get cyclic strip time.
        strip.use_sync_length = !act.use_frame_range;
        strip.use_animated_time_cyclic = act.use_cyclic && act.use_frame_range;
        strip
    }

    /// `BKE_nlastrips_has_space`
    pub fn has_space(strips: &[NlaStrip], start: f32, end: f32) -> bool {
        if (start - end).abs() <= f32::EPSILON {
            return false;
        }
        let (start, end) = if start > end {
            (end, start)
        } else {
            (start, end)
        };
        for s in strips {
            if s.frame_start >= end {
                return true;
            }
            if s.frame_end > start || s.frame_end > end {
                return false;
            }
        }
        true
    }

    /// `BKE_nlastrips_add_strip`: inserts `strip` in time order when it fits; its index.
    pub fn add_strip(track: &mut NlaTrack, strip: NlaStrip) -> Result<usize, NlaStrip> {
        if track.lock || !has_space(&track.strips, strip.frame_start, strip.frame_end) {
            return Err(strip);
        }
        let at = track
            .strips
            .iter()
            .position(|s| s.frame_start >= strip.frame_end)
            .unwrap_or(track.strips.len());
        track.strips.insert(at, strip);
        Ok(at)
    }

    /// `BKE_nlastrip_validate_name`: a default name (the action's) when empty, made unique
    /// among every strip of the `AnimData`.
    pub fn validate_name(adt: &mut AnimData, t: usize, s: usize, action_name: Option<&str>) {
        let mut name = adt.nla_tracks[t].strips[s].name.clone();
        if name.is_empty() {
            name = action_name.unwrap_or("<No Action>").into();
        }
        let unique = unique_subname(
            &|n| {
                adt.nla_tracks.iter().enumerate().any(|(i, tr)| {
                    tr.strips
                        .iter()
                        .enumerate()
                        .any(|(j, x)| (i, j) != (t, s) && x.name == n)
                })
            },
            &name,
            NAME_MAX,
            "NlaStrip",
        );
        adt.nla_tracks[t].strips[s].name = unique;
    }

    /// `BKE_nla_action_pushdown`: the assigned action becomes a strip on the top track (a new
    /// one named after the action when it has no room), the action is un-assigned (its slot
    /// remembered), the strip takes the action's blend settings and becomes active.
    /// `false` when there is no action.
    pub fn action_pushdown(
        adt: &mut AnimData,
        act_ref: frap_rna::IdRef<frap_rna::ActionId>,
        act: &mut Action,
        id_type: &str,
        id_name: &str,
    ) -> bool {
        if adt.action != Some(act_ref) {
            return false;
        }
        let handle = adt.action_slot_handle;
        let mut strip = strip_new(act_ref, act, id_type, id_name, None);
        // `BKE_nlastrip_new_for_slot`: the AnimData's slot.
        if let Some(s) = act.slot(handle) {
            strip.action_slot_handle = handle;
            strip.last_slot_identifier = s.identifier.clone();
            let range = slot_frame_range(act, handle);
            let fresh = NlaStrip::new_clip("", Some(act_ref), range[0], range);
            strip.frame_start = fresh.frame_start;
            strip.frame_end = fresh.frame_end;
            strip.action_frame_start = fresh.action_frame_start;
            strip.action_frame_end = fresh.action_frame_end;
        }
        let action_name = act.header.name.clone();
        let n = adt.nla_tracks.len();
        let placed = match adt.nla_tracks.last_mut() {
            Some(tr) => add_strip(tr, strip).map(|s| (n - 1, s)),
            None => Err(strip),
        };
        let (t, s) = match placed {
            Ok(ts) => ts,
            Err(strip) => {
                let t = track_new_tail(adt);
                track_set_active(adt, t);
                adt.nla_tracks[t].name = action_name.clone();
                let s = add_strip(&mut adt.nla_tracks[t], strip).unwrap_or(0);
                (t, s)
            }
        };
        validate_name(adt, t, s, Some(&action_name));
        // `animrig::unassign_action`: the slot is remembered.
        if let Some(sl) = act.slot(handle) {
            adt.last_slot_identifier = sl.identifier.clone();
        }
        adt.action = None;
        adt.action_slot_handle = 0;
        let strip = &mut adt.nla_tracks[t].strips[s];
        strip.blend_type = adt.action_blend_type.clone();
        strip.influence = adt.action_influence;
        strip.extrapolation = adt.action_extrapolation.clone();
        if adt.action_influence < 1.0 {
            strip.use_animated_influence = true;
        }
        strip_set_active(adt, t, s);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_reports_follow_blender() {
        let mut r = CombinedKeyingResult::default();
        assert_eq!(
            r.report(ReportType::Warning).1,
            "No keys have been inserted and no errors have been reported."
        );
        r.add(KeyingResult::NoKeyNeeded, 3);
        assert_eq!(
            r.report(ReportType::Warning),
            (
                ReportType::Warning,
                "Due to the setting 'Only Insert Needed', 3 keyframe(s) have not been inserted."
                    .into()
            )
        );
        r.add(KeyingResult::CannotCreateFcurve, 1);
        assert_eq!(
            r.report(ReportType::Error).1,
            "Inserting keyframes failed:\n- Could not create 1 F-Curve(s). This can happen when only inserting to available F-Curves.\n- Due to the setting 'Only Insert Needed', 3 keyframe(s) have not been inserted."
        );
    }

    #[test]
    fn api_flags() {
        let f = KeyFlags::from_api(&["INSERTKEY_REPLACE".into(), "INSERTKEY_NEEDED".into()]);
        assert!(f.replace && f.needed && !f.available);
        assert!(!f.may_create_fcurve());
        assert!(KeyFlags::default().may_create_fcurve());
    }

    #[test]
    fn values_of_property_kinds() {
        let p = &frap_spec_rna::OBJECT;
        let loc = p.find_prop("location").unwrap();
        let v = Value::FloatArray {
            data: smallvec::smallvec![1.0, 2.0, 3.0],
            shape: loc.array,
            tag: frap_rna::MathTag::Vector,
        };
        assert_eq!(rna_values(loc, &v), vec![1.0, 2.0, 3.0]);
        let mode = p.find_prop("rotation_mode").unwrap();
        assert_eq!(rna_values(mode, &Value::Enum("XYZ".into())), vec![1.0]);
        let hide = p.find_prop("hide_render").unwrap();
        assert_eq!(rna_values(hide, &Value::Bool(true)), vec![1.0]);
    }
}
