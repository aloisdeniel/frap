// SPDX-License-Identifier: GPL-3.0-or-later
//! Multiple scenes (M3, WP 3.10): `bpy.data.scenes.new(name)` (`BKE_scene_add`),
//! `scene.new(type=…)` (`scene_add`: `BKE_scene_add` for `NEW`, `BKE_scene_duplicate` for
//! `EMPTY`, `LINK_COPY`, `FULL_COPY`), and scene removal (`bpy.data.scenes.remove`,
//! `scene.delete`: the last local scene cannot be removed, a window showing the removed scene
//! switches to its neighbour first). The RNA functions
//! (`frap-ops-data/src/funcs/blend_data_scenes_*.rs`) and the operators
//! (`frap-ops-object/src/ops/scene__{new,delete}.rs`) call these.
//!
//! Ported from `blenkernel/intern/scene.cc` (`BKE_scene_add`, `BKE_scene_duplicate`,
//! `BKE_scene_can_be_removed`, `BKE_scene_find_replacement`), `blenkernel/intern/collection.cc`
//! (`collection_duplicate_recursive`), `blenkernel/intern/object.cc` (`BKE_object_duplicate`)
//! and `editors/scene/scene_edit.cc` (`scene_add`, `ED_scene_replace_active_for_deletion`),
//! Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4 (`-b --factory-startup`):
//! - a new scene has no objects, no camera and no world, one view layer `ViewLayer` whose
//!   active layer collection is the scene collection, and the factory scene's settings except
//!   the DNA defaults of [`NEW_SCENE_OVERLAY`] (`render.filepath` `//`, the Workbench
//!   `display.shading` values, `eevee.use_fast_gi`, …);
//! - every scene has one user (`id_us_ensure_real`), so `scenes.remove(s, do_unlink=False)`
//!   always refuses (`must have zero users to be removed, found 1`);
//! - `NEW` is named `Scene` (uniquified), the copies keep the source name (uniquified);
//!   `EMPTY` copies the render settings (frame range and current frame included), unit,
//!   EEVEE, colour-management, display and tool settings and the custom properties, not the
//!   cursor, camera, world or objects; `LINK_COPY` shares the collections, objects, camera and
//!   world, and copies the view layers with their selection and active object; `FULL_COPY`
//!   also copies the world, every collection and object, and the object data as the factory
//!   "Duplicate Data" preferences say (mesh, camera, light, armature, point cloud and actions;
//!   materials are shared), and relinks the copies to each other (`scene.camera`, parents,
//!   modifier objects); copies of objects in Edit Mode are in Object Mode;
//! - removing the window's scene shows the previous scene in `bpy.data.scenes`, else the next.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use frap_rna::{CollectionId, IdAny, IdCode, IdRef, IdType, ObjectId, SceneId};
use serde_json::Value as J;

use crate::id::{Collection, Scene, layers};
use crate::{ChangeLog, IdMap, Main, ObjectMode, ViewLayer, WindowState, startup};

/// `scene.new(type=…)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SceneCopy {
    /// `NEW`: a new, empty scene with default settings.
    New,
    /// `EMPTY`: a copy of the settings without objects.
    Empty,
    /// `LINK_COPY`: settings copied, collections and objects linked.
    LinkCopy,
    /// `FULL_COPY`: everything copied (objects and data duplicated).
    FullCopy,
}

impl SceneCopy {
    /// The RNA enum identifier (`NEW`, `EMPTY`, `LINK_COPY`, `FULL_COPY`).
    pub fn identifier(self) -> &'static str {
        match self {
            SceneCopy::New => "NEW",
            SceneCopy::Empty => "EMPTY",
            SceneCopy::LinkCopy => "LINK_COPY",
            SceneCopy::FullCopy => "FULL_COPY",
        }
    }
}

/// The values of a scene made by `BKE_scene_add` (DNA defaults) that differ from the factory
/// scene's (`startup.blend`), as dumped on 5.2.4 by serialising `bpy.data.scenes.new('Scene')`
/// and the factory `Scene` with `tools/blender/dump_startup.py`'s serialiser and diffing them.
/// Pointers (camera, world, collections, paint modes, curve mappings) and the view layer's
/// Freestyle settings are left out: a new scene builds its own.
pub const NEW_SCENE_OVERLAY: &str = r#"{
 "display": {"$type": "SceneDisplay", "shading": {"$type": "View3DShading",
  "background_color": [0.05000000074505806, 0.05000000074505806, 0.05000000074505806],
  "cavity_type": "SCREEN", "curvature_ridge_factor": 1.0, "curvature_valley_factor": 1.0,
  "show_xray_wireframe": true, "studiolight_intensity": 1.0, "use_scene_lights_render": true,
  "use_scene_world_render": true, "wireframe_color_type": "THEME", "xray_alpha_wireframe": 0.5}},
 "eevee": {"$type": "SceneEEVEE", "use_fast_gi": false},
 "render": {"$type": "RenderSettings",
  "bake": {"$type": "BakeSettings", "image_settings": {"$type": "ImageFormatSettings",
   "view_settings": {"$type": "ColorManagedViewSettings", "view_transform": "AgX",
    "white_balance_temperature": 6500.0, "white_balance_tint": 10.0,
    "white_balance_whitepoint": [0.9993047714233398, 1.0003489255905151, 0.9985780119895935]}}},
  "ffmpeg": {"$type": "FFmpegSettings", "constant_rate_factor": "LOSSLESS", "ffmpeg_preset": ""},
  "filepath": "//",
  "image_settings": {"$type": "ImageFormatSettings", "exr_codec": "NONE",
   "view_settings": {"$type": "ColorManagedViewSettings", "view_transform": "AgX",
    "white_balance_temperature": 6500.0, "white_balance_tint": 10.0,
    "white_balance_whitepoint": [0.9993047714233398, 1.0003489255905151, 0.9985780119895935]}},
  "simplify_child_particles_render": 0.0, "simplify_subdivision_render": 0,
  "use_stamp_memory": true},
 "sync_mode": "AUDIO_SYNC",
 "tool_settings": {"$type": "ToolSettings", "annotation_stroke_placement_view2d": "",
  "gpencil_sculpt": {"$type": "GPencilSculptSettings",
   "guide": {"$type": "GPencilSculptGuide", "spacing": 20.0}, "intersection_threshold": 0.0},
  "image_paint": {"$type": "ImagePaint", "missing_texture": false, "screen_grab_size": [0, 0],
   "tile_offset": [1.0, 1.0, 1.0], "use_symmetry_feather": true},
  "particle_edit": {"$type": "ParticleEdit", "fade_frames": 2, "use_auto_velocity": true},
  "sequencer_tool_settings": {"$type": "SequencerToolSettings",
   "pivot_point": "INDIVIDUAL_ORIGINS"}}
}"#;

/// Scene properties the generic loader leaves alone (set directly: the setters are hooks).
const SKIP: &[&str] = &["frame_current", "frame_start", "frame_end"];

/// The settings of a `BKE_scene_add` scene, without view layers (built once).
fn template() -> &'static Scene {
    static T: OnceLock<Scene> = OnceLock::new();
    T.get_or_init(|| {
        let factory = &startup::factory_dump()["ids"]["scenes"][0];
        let overlay: J =
            serde_json::from_str(NEW_SCENE_OVERLAY).expect("invariant: the overlay parses");
        let mut sc = Scene::new("", Vec::new());
        let _ = startup::load_rna(&mut sc, factory, SKIP);
        let _ = startup::load_rna(&mut sc, &overlay, SKIP);
        sc.frame_start = factory["frame_start"].as_i64().unwrap_or(1) as i32;
        sc.frame_end = factory["frame_end"].as_i64().unwrap_or(250) as i32;
        sc.frame_current = factory["frame_current"].as_i64().unwrap_or(1) as i32;
        // DNA-only UV settings (M3 W2, WP 3.8): a new scene has the `ToolSettings` DNA
        // defaults (`unwrapper` CONFORMAL, no `UVCALC_FILLHOLES`), the factory scene not
        // (verified on 5.2.4: a bare `uv.unwrap()` there is ANGLE_BASED with filled holes).
        sc.tool_settings.unwrapper = "CONFORMAL".into();
        sc.tool_settings.uvcalc_flag = 0;
        sc
    })
}

/// A detached `BKE_scene_add` scene (settings, empty master collection, one `ViewLayer`).
pub fn default_scene(main: &Main) -> Scene {
    let mut sc = template().clone();
    sc.header = frap_rna::IdHeader::new("");
    let vl = ViewLayer::new_synced("ViewLayer", &sc, &main.collections, &main.objects);
    sc.view_layers = vec![vl];
    sc
}

/// `BKE_scene_add`: a new scene with Blender's default settings, a master collection and one
/// view layer `ViewLayer` (name uniquified by `IdMap::insert`).
pub fn new_scene(main: &mut Main, name: &str, log: &mut ChangeLog) -> IdRef<SceneId> {
    let sc = default_scene(main);
    main.scenes.insert(name, sc, log)
}

/// `scene.new(type)` from `src` (the window's scene): returns the new scene (the operator
/// makes it the window's scene).
pub fn copy_scene(
    main: &mut Main,
    src: IdRef<SceneId>,
    ty: SceneCopy,
    log: &mut ChangeLog,
) -> IdRef<SceneId> {
    let Some(old) = main.scenes.get(src).cloned() else {
        return new_scene(main, "Scene", log);
    };
    match ty {
        SceneCopy::New => new_scene(main, "Scene", log),
        SceneCopy::Empty => {
            let mut sc = default_scene(main);
            sc.render = old.render.clone();
            sc.frame_start = old.frame_start;
            sc.frame_end = old.frame_end;
            sc.frame_current = old.frame_current;
            sc.frame_step = old.frame_step;
            sc.unit_settings = old.unit_settings.clone();
            sc.eevee = old.eevee.clone();
            sc.display_settings = old.display_settings.clone();
            sc.view_settings = old.view_settings.clone();
            sc.display = old.display.clone();
            sc.tool_settings = old.tool_settings.clone();
            sc.header.custom = old.header.custom.clone();
            main.scenes.insert(&old.header.name, sc, log)
        }
        SceneCopy::LinkCopy | SceneCopy::FullCopy => {
            let mut sc = old.clone();
            let name = sc.header.name.clone();
            {
                let h = &mut sc.header;
                h.use_fake_user = false;
                h.library = None;
                h.is_linked_packed = false;
            }
            let new = main.scenes.insert(&name, sc, log);
            let mut dup = Dup::default();
            dup.map.insert(src.any(), new.any());
            // `BKE_animdata_duplicate_id_action` (`USER_DUP_ACT` is a factory preference).
            dup.actions_of(main, new.any(), log);
            if ty == SceneCopy::FullCopy {
                if let Some(w) = old.world {
                    dup.copy(main, w.any(), log);
                }
                dup.master_collection(main, new, log);
            }
            dup.relink(main, log);
            layers::sync_scene(main, new, log);
            new
        }
    }
}

/// `ID.newid` bookkeeping of one `BKE_scene_duplicate` call: every copy made, in creation
/// order, so `BKE_libblock_relink_to_newid` can retarget the copies to each other.
#[derive(Default)]
struct Dup {
    /// Original → copy.
    map: HashMap<IdAny, IdAny>,
}

/// A local copy of `a` in its map (`BKE_id_copy_for_duplicate`): requested name uniquified,
/// fake user, library and packed flags cleared.
fn copy_in<T: IdType>(m: &mut IdMap<T>, a: IdAny, log: &mut ChangeLog) -> Option<IdAny> {
    let r = a.typed::<T::Kind>()?;
    let mut v = m.get(r)?.clone();
    let name = v.header().name.clone();
    let h = v.header_mut();
    h.use_fake_user = false;
    h.library = None;
    h.is_linked_packed = false;
    Some(m.insert(&name, v, log).any())
}

/// Retargets every reference of the ID `a` that `map` names.
fn relink_in<T: IdType>(
    m: &mut IdMap<T>,
    a: IdAny,
    map: &HashMap<IdAny, IdAny>,
    log: &mut ChangeLog,
) {
    let Some(r) = a.typed::<T::Kind>() else {
        return;
    };
    let Some(t) = m.get(r) else {
        return;
    };
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    t.for_each_ref(&mut |x| {
        if let Some(&n) = map.get(&x)
            && seen.insert(x)
        {
            hits.push((x, n));
        }
    });
    if a.code == IdCode::SC {
        // View-layer bases and active objects are not user references (`for_each_ref` leaves
        // them out) but are relinked too (`BKE_libblock_relink_to_newid` walks the bases).
        let mut objects: Vec<(IdAny, IdAny)> = map
            .iter()
            .filter(|(o, _)| o.code == IdCode::OB)
            .filter(|(o, _)| !seen.contains(*o))
            .map(|(&o, &n)| (o, n))
            .collect();
        objects.sort_by_key(|(o, _)| o.key);
        hits.extend(objects);
    }
    if hits.is_empty() {
        return;
    }
    if let Some(t) = m.get_mut(r, log) {
        for (from, to) in hits {
            t.remap_ref(from, Some(to));
        }
    }
}

/// The layer collections of a scene's view layers follow its collections to their copies
/// (so the active layer collection and the layer flags survive a full copy).
fn relink_layer_collections(
    main: &mut Main,
    scene: IdRef<SceneId>,
    map: &HashMap<IdAny, IdAny>,
    log: &mut ChangeLog,
) {
    fn walk(lc: &mut crate::LayerCollection, map: &HashMap<IdAny, IdAny>) {
        if let Some(c) = lc.collection
            && let Some(n) = map.get(&c.any()).and_then(|n| n.typed::<CollectionId>())
        {
            lc.collection = Some(n);
        }
        for child in &mut lc.children {
            walk(child, map);
        }
    }
    if !map.keys().any(|k| k.code == IdCode::GR) {
        return;
    }
    if let Some(s) = main.scenes.get_mut(scene, log) {
        for vl in &mut s.view_layers {
            walk(&mut vl.layer_collection, map);
        }
    }
}

/// Applies `relink_in` to the ID `a`, whatever its type.
fn relink_any(main: &mut Main, a: IdAny, map: &HashMap<IdAny, IdAny>, log: &mut ChangeLog) {
    match a.code {
        IdCode::OB => relink_in(&mut main.objects, a, map, log),
        IdCode::ME => relink_in(&mut main.meshes, a, map, log),
        // `dof.focus_object` too (`Camera::for_each_ref`; verified on 5.2.4: a full scene
        // copy's camera focuses on the copied object).
        IdCode::CA => relink_in(&mut main.cameras, a, map, log),
        IdCode::LA => relink_in(&mut main.lights, a, map, log),
        IdCode::AR => relink_in(&mut main.armatures, a, map, log),
        IdCode::PT => relink_in(&mut main.pointclouds, a, map, log),
        IdCode::WO => relink_in(&mut main.worlds, a, map, log),
        IdCode::GR => relink_in(&mut main.collections, a, map, log),
        IdCode::SC => {
            relink_in(&mut main.scenes, a, map, log);
            if let Some(s) = a.typed::<SceneId>() {
                relink_layer_collections(main, s, map, log);
            }
        }
        IdCode::KE => relink_in(&mut main.shape_keys, a, map, log),
        IdCode::AC => relink_in(&mut main.actions, a, map, log),
        IdCode::MA => relink_in(&mut main.materials, a, map, log),
        _ => {}
    }
}

/// Outgoing references of any ID a scene copy duplicates.
fn for_each_ref(main: &Main, a: IdAny, f: &mut dyn FnMut(IdAny)) {
    fn go<T: IdType>(m: &IdMap<T>, a: IdAny, f: &mut dyn FnMut(IdAny)) {
        if let Some(t) = a.typed::<T::Kind>().and_then(|r| m.get(r)) {
            t.for_each_ref(f);
        }
    }
    match a.code {
        IdCode::OB => go(&main.objects, a, f),
        IdCode::ME => go(&main.meshes, a, f),
        IdCode::CA => go(&main.cameras, a, f),
        IdCode::LA => go(&main.lights, a, f),
        IdCode::AR => go(&main.armatures, a, f),
        IdCode::PT => go(&main.pointclouds, a, f),
        IdCode::WO => go(&main.worlds, a, f),
        IdCode::GR => go(&main.collections, a, f),
        IdCode::SC => go(&main.scenes, a, f),
        IdCode::KE => go(&main.shape_keys, a, f),
        IdCode::AC => go(&main.actions, a, f),
        IdCode::MA => go(&main.materials, a, f),
        _ => {}
    }
}

/// A collection being deep-copied: the new scene's master collection (`collection_old ==
/// collection_new` in Blender) or a `bpy.data.collections` ID.
#[derive(Copy, Clone)]
enum Target {
    Master(IdRef<SceneId>),
    Coll(IdRef<CollectionId>),
}

fn target(main: &Main, t: Target) -> Option<&Collection> {
    match t {
        Target::Master(s) => main.scenes.get(s).map(|s| &s.collection),
        Target::Coll(c) => main.collections.get(c),
    }
}

fn target_mut<'m>(
    main: &'m mut Main,
    t: Target,
    log: &mut ChangeLog,
) -> Option<&'m mut Collection> {
    match t {
        Target::Master(s) => main.scenes.get_mut(s, log).map(|s| &mut s.collection),
        Target::Coll(c) => main.collections.get_mut(c, log),
    }
}

impl Dup {
    /// `BKE_id_copy_for_duplicate`: the copy of `a` (made once per call), with the actions it
    /// uses copied (`USER_DUP_ACT`) and, for a mesh, its shape keys.
    fn copy(&mut self, main: &mut Main, a: IdAny, log: &mut ChangeLog) -> IdAny {
        if let Some(&n) = self.map.get(&a) {
            return n;
        }
        let new = match a.code {
            IdCode::OB => copy_in(&mut main.objects, a, log),
            IdCode::ME => copy_in(&mut main.meshes, a, log),
            IdCode::CA => copy_in(&mut main.cameras, a, log),
            IdCode::LA => copy_in(&mut main.lights, a, log),
            IdCode::AR => copy_in(&mut main.armatures, a, log),
            IdCode::PT => copy_in(&mut main.pointclouds, a, log),
            IdCode::WO => copy_in(&mut main.worlds, a, log),
            IdCode::GR => copy_in(&mut main.collections, a, log),
            IdCode::AC => copy_in(&mut main.actions, a, log),
            IdCode::KE => copy_in(&mut main.shape_keys, a, log),
            IdCode::MA => copy_in(&mut main.materials, a, log),
            _ => None,
        };
        let Some(new) = new else {
            return a;
        };
        self.map.insert(a, new);
        if a.code != IdCode::AC {
            self.actions_of(main, new, log);
        }
        if let Some(me) = new.typed::<frap_rna::MeshId>()
            && let Some(k) = main.meshes.get(me).and_then(|m| m.shape_keys)
        {
            self.copy(main, k.any(), log);
        }
        new
    }

    /// `BKE_animdata_duplicate_id_action`: copies the actions the ID `new` uses.
    fn actions_of(&mut self, main: &mut Main, new: IdAny, log: &mut ChangeLog) {
        let mut actions = Vec::new();
        for_each_ref(main, new, &mut |x| {
            if x.code == IdCode::AC && !actions.contains(&x) {
                actions.push(x);
            }
        });
        for a in actions {
            self.copy(main, a, log);
        }
    }

    /// `BKE_object_duplicate` (sub-process of a scene copy, factory `U.dupflag |
    /// USER_DUP_OBJECT`): the object, its action and its data (every data type frap has is in
    /// the factory "Duplicate Data" set; materials are not). The copy is in Object Mode.
    fn object(&mut self, main: &mut Main, ob: IdRef<ObjectId>, log: &mut ChangeLog) {
        if self.map.contains_key(&ob.any()) {
            return;
        }
        let Some(new) = self.copy(main, ob.any(), log).typed::<ObjectId>() else {
            return;
        };
        let data = main.objects.get(new).and_then(|o| o.data);
        if let Some(d) = data
            && matches!(
                d.code,
                IdCode::ME | IdCode::CA | IdCode::LA | IdCode::AR | IdCode::PT
            )
        {
            self.copy(main, d, log);
        }
        if main
            .objects
            .get(new)
            .is_some_and(|o| o.mode != ObjectMode::Object)
            && let Some(o) = main.objects.get_mut(new, log)
        {
            o.mode = ObjectMode::Object;
        }
    }

    /// `collection_duplicate_recursive` on the new scene's master collection (never copied
    /// itself: its objects and children are).
    fn master_collection(&mut self, main: &mut Main, scene: IdRef<SceneId>, log: &mut ChangeLog) {
        self.contents(main, Target::Master(scene), log);
    }

    /// `collection_duplicate_recursive` on a child collection: its copy (made once).
    fn collection(
        &mut self,
        main: &mut Main,
        old: IdRef<CollectionId>,
        log: &mut ChangeLog,
    ) -> IdRef<CollectionId> {
        if let Some(n) = self.map.get(&old.any()).and_then(|n| n.typed()) {
            return n;
        }
        let Some(new) = self.copy(main, old.any(), log).typed::<CollectionId>() else {
            return old;
        };
        if new != old {
            self.contents(main, Target::Coll(new), log);
        }
        new
    }

    /// Duplicates the objects of `t` and swaps them in (`collection_object_add` of the copy,
    /// then `collection_object_remove` of the original), then does the same with its child
    /// collections, each copy taking its original's place.
    fn contents(&mut self, main: &mut Main, t: Target, log: &mut ChangeLog) {
        let Some(c) = target(main, t) else {
            return;
        };
        let (objects, children) = (c.objects.clone(), c.children.clone());
        for &ob in &objects {
            self.object(main, ob, log);
        }
        for &ob in &objects {
            let Some(new) = self
                .map
                .get(&ob.any())
                .and_then(|n| n.typed::<ObjectId>())
                .filter(|&n| n != ob)
            else {
                continue;
            };
            if let Some(c) = target_mut(main, t, log) {
                if !c.objects.contains(&new) {
                    c.objects.push(new);
                }
                c.objects.retain(|&o| o != ob);
            }
        }
        for child in children {
            let new = self.collection(main, child, log);
            if new == child {
                continue;
            }
            if let Some(c) = target_mut(main, t, log) {
                if !c.children.contains(&new)
                    && let Some(i) = c.children.iter().position(|&x| x == child)
                {
                    c.children.insert(i + 1, new);
                }
                c.children.retain(|&x| x != child);
            }
        }
    }

    /// `BKE_libblock_relink_to_newid`: every copy now uses the other copies instead of their
    /// originals (`scene.camera`, parents, modifier objects, object data, actions, …).
    fn relink(&self, main: &mut Main, log: &mut ChangeLog) {
        let mut news: Vec<IdAny> = self.map.values().copied().collect();
        news.sort_by_key(|a| a.key);
        for n in news {
            relink_any(main, n, &self.map, log);
        }
    }
}

/// `BKE_id_copy_ex(…, LIB_ID_COPY_DEFAULT | LIB_ID_COPY_ACTIONS)` of an object, object data,
/// material, world, collection or action: the copy (requested name uniquified, fake user and
/// library cleared), with the actions it uses copied and, for a mesh, its shape keys; `None`
/// for a type frap does not copy this way. Used by `object.make_single_user`.
pub fn duplicate_id(main: &mut Main, a: IdAny, log: &mut ChangeLog) -> Option<IdAny> {
    let mut dup = Dup::default();
    let new = dup.copy(main, a, log);
    if new == a {
        return None;
    }
    dup.relink(main, log);
    Some(new)
}

/// `BKE_libblock_relink_to_newid` on `ids`: every reference to a key of `map` becomes its
/// value (for a scene, the objects of its view-layer bases and active objects too).
pub fn relink_ids(
    main: &mut Main,
    ids: &[IdAny],
    map: &HashMap<IdAny, IdAny>,
    log: &mut ChangeLog,
) {
    for &a in ids {
        relink_any(main, a, map, log);
    }
}

/// `BKE_scene_can_be_removed`: a linked scene always, a local one while another local scene
/// remains.
pub fn can_be_removed(main: &Main, scene: IdRef<SceneId>) -> bool {
    let Some(s) = main.scenes.get(scene) else {
        return false;
    };
    s.header.library.is_some()
        || main
            .scenes
            .iter()
            .any(|(r, o)| r != scene && o.header.library.is_none())
}

/// `BKE_scene_find_replacement`: the closest previous scene in `bpy.data.scenes`, else the
/// closest next one.
pub fn find_replacement(main: &Main, scene: IdRef<SceneId>) -> Option<IdRef<SceneId>> {
    let i = main.scenes.index_of(scene)?;
    let all: Vec<IdRef<SceneId>> = main.scenes.iter().map(|(r, _)| r).collect();
    all[..i].last().copied().or_else(|| all.get(i + 1).copied())
}

/// The message of a scene that cannot be removed (`rna_Main_scenes_remove`).
pub fn last_scene_message(name: &str) -> String {
    format!("Scene '{name}' is the last local one, cannot be removed")
}

/// Removes `scene` (`BKE_scene_can_be_removed`, `ED_scene_replace_active_for_deletion`,
/// `BKE_id_delete`): `Err` with Blender's message when it is the last local scene; a window
/// showing it switches to [`find_replacement`] first. Its objects and collections stay in
/// `bpy.data` (their users decremented).
pub fn remove_scene(
    main: &mut Main,
    win: &mut WindowState,
    scene: IdRef<SceneId>,
    log: &mut ChangeLog,
) -> Result<(), String> {
    let Some(name) = main.scenes.get(scene).map(|s| s.header.name.clone()) else {
        return Err("StructRNA of type Scene has been removed".into());
    };
    let replacement = find_replacement(main, scene).filter(|_| can_be_removed(main, scene));
    let Some(replacement) = replacement else {
        return Err(last_scene_message(&name));
    };
    if win.scene == scene {
        crate::window::set_scene(main, win, replacement)?;
        log.context_changed = true;
    }
    main.remove(scene.any(), log);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::startup;

    fn names(main: &Main) -> Vec<String> {
        main.scenes
            .iter()
            .map(|(_, s)| s.header.name.to_string())
            .collect()
    }

    #[test]
    fn the_overlay_loads_without_errors() {
        let overlay: J = serde_json::from_str(NEW_SCENE_OVERLAY).unwrap();
        let mut sc = Scene::new("", Vec::new());
        assert_eq!(
            startup::load_rna(&mut sc, &overlay, SKIP),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_new_scene_has_blender_defaults_and_nothing_in_it() {
        let (mut main, _) = startup::factory();
        let mut log = ChangeLog::default();
        let s = new_scene(&mut main, "", &mut log);
        let sc = main.scenes.get(s).unwrap();
        assert_eq!(sc.header.name, "Scene.001");
        assert_eq!(sc.render.filepath, "//");
        assert_eq!(sc.render.resolution_x, 1920);
        assert_eq!(sc.display.shading.xray_alpha_wireframe, 0.5);
        assert_eq!(sc.display.shading.cavity_type, "SCREEN");
        assert!(!sc.eevee.use_fast_gi);
        assert_eq!(sc.view_settings.view_transform, "AgX");
        assert_eq!(
            (sc.frame_start, sc.frame_end, sc.frame_current),
            (1, 250, 1)
        );
        assert!(sc.camera.is_none() && sc.world.is_none());
        assert!(sc.collection.objects.is_empty() && sc.collection.children.is_empty());
        assert_eq!(sc.view_layers.len(), 1);
        assert_eq!(sc.view_layers[0].name, "ViewLayer");
        assert!(sc.view_layers[0].bases.is_empty());
        assert_eq!(names(&main), ["Scene", "Scene.001"]);
    }

    #[test]
    fn copies_follow_their_type() {
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let src = win.scene;
        if let Some(s) = main.scenes.get_mut(src, &mut log) {
            s.render.resolution_x = 640;
            s.frame_current = 7;
            s.cursor.location = [1.0, 2.0, 3.0];
        }
        let new = copy_scene(&mut main, src, SceneCopy::New, &mut log);
        let empty = copy_scene(&mut main, src, SceneCopy::Empty, &mut log);
        let link = copy_scene(&mut main, src, SceneCopy::LinkCopy, &mut log);
        let full = copy_scene(&mut main, src, SceneCopy::FullCopy, &mut log);
        assert_eq!(
            names(&main),
            ["Scene", "Scene.001", "Scene.002", "Scene.003", "Scene.004"]
        );
        let get = |r| main.scenes.get(r).unwrap();
        assert_eq!(get(new).render.resolution_x, 1920);
        assert_eq!(get(empty).render.resolution_x, 640);
        assert_eq!(get(empty).frame_current, 7);
        assert_eq!(get(empty).cursor.location, [0.0; 3]);
        assert!(get(empty).world.is_none() && get(empty).camera.is_none());
        // Linked copy: the same collection, camera and world, the same selection.
        assert_eq!(get(link).collection.children, get(src).collection.children);
        assert_eq!(get(link).camera, get(src).camera);
        assert_eq!(get(link).world, get(src).world);
        assert_eq!(
            get(link).view_layers[0].active,
            get(src).view_layers[0].active
        );
        // Full copy: everything copied and relinked, materials shared.
        let f = get(full);
        let coll = f.collection.children[0];
        assert_eq!(main.collections.name_of(coll), Some("Collection.001"));
        let cam = f.camera.unwrap();
        assert_eq!(main.objects.name_of(cam), Some("Camera.001"));
        assert_eq!(main.worlds.name_of(f.world.unwrap()), Some("World.001"));
        let active = f.view_layers[0].active.unwrap();
        assert_eq!(main.objects.name_of(active), Some("Cube.001"));
        assert!(
            f.view_layers[0]
                .bases
                .iter()
                .any(|b| b.object == active && b.selected())
        );
        let cube = main.objects.get(active).unwrap();
        let me = cube.data.unwrap().typed::<frap_rna::MeshId>().unwrap();
        assert_eq!(main.meshes.name_of(me), Some("Cube.001"));
        let mat = main.meshes.get(me).unwrap().materials[0].unwrap();
        assert_eq!(main.materials.name_of(mat), Some("Material"));
        // The original scene is untouched.
        assert_eq!(
            main.collections.name_of(get(src).collection.children[0]),
            Some("Collection")
        );
    }

    #[test]
    fn removal_keeps_one_local_scene_and_moves_the_window() {
        let (mut main, mut win) = startup::factory();
        let mut log = ChangeLog::default();
        let first = win.scene;
        assert_eq!(
            remove_scene(&mut main, &mut win, first, &mut log),
            Err("Scene 'Scene' is the last local one, cannot be removed".into())
        );
        let a = new_scene(&mut main, "A", &mut log);
        let z = new_scene(&mut main, "Z", &mut log);
        // `A` sorts first: its replacement is the next scene.
        assert_eq!(find_replacement(&main, a), Some(first));
        assert_eq!(find_replacement(&main, z), Some(first));
        assert_eq!(find_replacement(&main, first), Some(a));
        remove_scene(&mut main, &mut win, first, &mut log).unwrap();
        assert_eq!(win.scene, a);
        assert_eq!(names(&main), ["A", "Z"]);
        // The removed scene's objects stay.
        assert!(main.objects.find("Cube").is_some());
        remove_scene(&mut main, &mut win, z, &mut log).unwrap();
        assert!(remove_scene(&mut main, &mut win, a, &mut log).is_err());
    }

    #[test]
    fn a_full_copy_camera_focuses_on_the_copied_object() {
        // Verified on 5.2.4: `Camera.001.dof.focus_object` is `Cube.001`.
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let cube = main.objects.find("Cube").unwrap();
        let ca = main.cameras.find("Camera").unwrap();
        main.cameras
            .get_mut(ca, &mut log)
            .unwrap()
            .fields
            .dof
            .focus_object = Some(cube.any());
        copy_scene(&mut main, win.scene, SceneCopy::FullCopy, &mut log);
        let ca1 = main.cameras.find("Camera.001").unwrap();
        let focus = main.cameras.get(ca1).unwrap().fields.dof.focus_object;
        assert_eq!(focus, main.objects.find("Cube.001").map(|r| r.any()));
        // The original keeps its own.
        let focus = main.cameras.get(ca).unwrap().fields.dof.focus_object;
        assert_eq!(focus, Some(cube.any()));
    }

    #[test]
    fn an_object_linked_into_another_scene_keeps_its_selection() {
        // Verified on 5.2.4: the factory Cube linked into a new scene is selected there, the
        // Camera is not; `select_set` on any view layer decides for the next link.
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let s2 = new_scene(&mut main, "S2", &mut log);
        let s3 = new_scene(&mut main, "S3", &mut log);
        let cube = main.objects.find("Cube").unwrap();
        let cam = main.objects.find("Camera").unwrap();
        let link = |main: &mut Main, s, ob, log: &mut ChangeLog| {
            main.scenes
                .get_mut(s, log)
                .unwrap()
                .collection
                .objects
                .push(ob);
            layers::sync_all(main, log);
        };
        let selected = |main: &Main, s, ob| {
            main.scenes.get(s).unwrap().view_layers[0]
                .base(ob)
                .is_some_and(|b| b.selected())
        };
        link(&mut main, s2, cube, &mut log);
        link(&mut main, s2, cam, &mut log);
        assert!(selected(&main, s2, cube));
        assert!(!selected(&main, s2, cam));
        // Deselected in the window's scene, still selected in S2: the last `select_set` wins.
        layers::select_set(&mut main, &mut log, win.scene, 0, cube, false).unwrap();
        link(&mut main, s3, cube, &mut log);
        assert!(!selected(&main, s3, cube));
        layers::select_set(&mut main, &mut log, s2, 0, cam, true).unwrap();
        link(&mut main, s3, cam, &mut log);
        assert!(selected(&main, s3, cam));
    }
}
