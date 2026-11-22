// SPDX-License-Identifier: GPL-3.0-or-later
//! `Main` + window state → `scene.json` (WP 1.23). See the module docs of `scene_map` for the
//! layout.

use std::collections::BTreeSet;
use std::sync::Arc;

use frap_anim::AnimData;
use frap_mesh::{AttrData, Mesh};
use frap_nodes::{InOut, Node, NodeTree, NodeTreeType, Socket, SocketType, SocketValue};
use frap_rna::{IdAny, IdHeader, IdRef, IdType, MeshId, Value};
use frap_scene::id::{
    Camera, Collection, Image, Library, Light, Material, Modifier, Object, PointCloud, Scene,
    SlotLink, Texture, World,
};
use frap_scene::{IdMap, LayerCollection, Main, Preferences, ViewLayer, WindowState};
use serde_json::{Map, Value as J, json};

use super::MapError;
use super::json::{f32j, f32s, f64j, idprop_j, mat4j, ser};
use crate::buffers::{BufferStore, Element, ImageStore};

/// Mesh to write for a mesh ID: the edit-mode view when it is in Edit Mode (§6.3).
pub(crate) type MeshView<'a> = &'a dyn Fn(IdRef<MeshId>) -> Option<Arc<Mesh>>;

pub(crate) struct Writer<'a> {
    main: &'a Main,
    written: BTreeSet<IdAny>,
    pub(crate) buffers: BufferStore,
    pub(crate) images: ImageStore,
    mesh_view: MeshView<'a>,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(main: &'a Main, skip: &BTreeSet<IdAny>, mesh_view: MeshView<'a>) -> Self {
        Writer {
            main,
            written: main
                .all_ids()
                .into_iter()
                .filter(|a| !skip.contains(a))
                .collect(),
            buffers: BufferStore::new(),
            images: ImageStore::new(),
            mesh_view,
        }
    }

    /// `{"$id": code, "name": …, "library": …}`; `null` for `None` or an ID that is not
    /// written (an orphan referenced by a non-user pointer such as `Scene.camera`).
    pub(crate) fn idref(&self, a: Option<IdAny>) -> J {
        let Some(a) = a.filter(|a| self.written.contains(a)) else {
            return J::Null;
        };
        let Some(h) = self.main.header(a) else {
            return J::Null;
        };
        let mut m = Map::new();
        m.insert("$id".into(), json!(a.code.as_str()));
        m.insert("name".into(), json!(h.name.as_str()));
        if let Some(lib) = h.library
            && let Some(l) = self.main.libraries.get(lib)
        {
            m.insert("library".into(), json!(l.header.name.as_str()));
        }
        J::Object(m)
    }
    fn r<K: frap_rna::IdKind>(&self, r: Option<IdRef<K>>) -> J {
        self.idref(r.map(|r| r.any()))
    }

    /// Encodes an array: inline when short, else a content-addressed buffer.
    fn buf<T: Element>(&mut self, data: &[T]) -> Result<J, MapError> {
        let enc = self.buffers.encode(data);
        ser(&enc)
    }

    fn header(&self, h: &IdHeader, m: &mut Map<String, J>) {
        m.insert("use_fake_user".into(), json!(h.use_fake_user));
        if h.library.is_some() {
            m.insert("is_linked_packed".into(), json!(h.is_linked_packed));
        }
        m.insert(
            "id_properties".into(),
            J::Object(
                h.custom
                    .iter()
                    .map(|(k, v)| (k.to_string(), idprop_j(v)))
                    .collect(),
            ),
        );
    }

    /// `animation_data` (WP 6.4: every field, the NLA tracks and strips included).
    fn anim(&self, a: &Option<AnimData>) -> J {
        match a {
            None => J::Null,
            Some(a) => json!({
                "action": self.r(a.action),
                "action_slot_handle": a.action_slot_handle,
                "last_slot_identifier": a.last_slot_identifier.as_str(),
                "use_nla": a.use_nla,
                "action_influence": f32j(a.action_influence),
                "action_blend_type": a.action_blend_type.as_str(),
                "action_extrapolation": a.action_extrapolation.as_str(),
                "use_tweak_mode": a.use_tweak_mode,
                "use_pin": a.use_pin,
                "nla_tracks": J::Array(a.nla_tracks.iter().map(|t| self.nla_track(t)).collect()),
            }),
        }
    }

    fn nla_track(&self, t: &frap_anim::NlaTrack) -> J {
        json!({
            "name": t.name.as_str(),
            "mute": t.mute,
            "is_solo": t.is_solo,
            "lock": t.lock,
            "select": t.select,
            "active": t.active,
            "strips": J::Array(t.strips.iter().map(|s| json!({
                "name": s.name.as_str(),
                "type": s.r#type.as_str(),
                "action": self.r(s.action),
                "action_slot_handle": s.action_slot_handle,
                "last_slot_identifier": s.last_slot_identifier.as_str(),
                "frame_start": f32j(s.frame_start),
                "frame_end": f32j(s.frame_end),
                "action_frame_start": f32j(s.action_frame_start),
                "action_frame_end": f32j(s.action_frame_end),
                "scale": f32j(s.scale),
                "repeat": f32j(s.repeat),
                "blend_type": s.blend_type.as_str(),
                "extrapolation": s.extrapolation.as_str(),
                "blend_in": f32j(s.blend_in),
                "blend_out": f32j(s.blend_out),
                "use_auto_blend": s.use_auto_blend,
                "influence": f32j(s.influence),
                "use_animated_influence": s.use_animated_influence,
                "strip_time": f32j(s.strip_time),
                "use_animated_time": s.use_animated_time,
                "use_animated_time_cyclic": s.use_animated_time_cyclic,
                "mute": s.mute,
                "use_reverse": s.use_reverse,
                "use_sync_length": s.use_sync_length,
                "select": s.select,
                "active": s.active,
            })).collect()),
        })
    }

    /// An action (WP 6.4): slots, layers, keyframe strips, channelbags, groups and F-curves;
    /// key coordinates, handles and per-key numbers go to buffers.
    fn action(&mut self, a: &frap_anim::Action) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert(
            "slots".into(),
            J::Array(
                a.slots
                    .iter()
                    .map(|s| {
                        json!({
                            "handle": s.handle,
                            "identifier": s.identifier.as_str(),
                            "target_id_type": s.target_id_type.as_str(),
                            "select": s.select,
                            "show_expanded": s.show_expanded,
                            "active": s.active,
                        })
                    })
                    .collect(),
            ),
        );
        let mut layers = Vec::new();
        for l in &a.layers {
            let mut strips = Vec::new();
            for st in &l.strips {
                let Some(k) = st.keyframe() else { continue };
                let mut bags = Vec::new();
                for b in &k.channelbags {
                    let mut fcurves = Vec::new();
                    for fc in &b.fcurves {
                        fcurves.push(self.fcurve(fc)?);
                    }
                    bags.push(json!({
                        "slot_handle": b.slot_handle,
                        "groups": J::Array(b.groups.iter().map(|g| json!({
                            "name": g.name.as_str(),
                            "color_set": g.color_set.as_str(),
                            "show_expanded": g.show_expanded,
                            "mute": g.mute,
                            "lock": g.lock,
                            "select": g.select,
                            "show_expanded_graph": g.show_expanded_graph,
                            "use_pin": g.use_pin,
                        })).collect()),
                        "fcurves": J::Array(fcurves),
                    }));
                }
                strips.push(json!({"type": "KEYFRAME", "channelbags": J::Array(bags)}));
            }
            layers.push(json!({
                "name": l.name.as_str(),
                "influence": f32j(l.influence),
                "mix_mode": l.mix_mode.as_str(),
                "strips": J::Array(strips),
            }));
        }
        m.insert("layers".into(), J::Array(layers));
        m.insert("last_slot_handle".into(), json!(a.last_slot_handle));
        m.insert("use_frame_range".into(), json!(a.use_frame_range));
        m.insert("frame_range".into(), f32s(&a.frame_range));
        m.insert("use_cyclic".into(), json!(a.use_cyclic));
        Ok(m)
    }

    fn fcurve(&mut self, fc: &frap_anim::FCurve) -> Result<J, MapError> {
        let k = &fc.keyframe_points;
        let co: Vec<[f32; 2]> = k.iter().map(|x| x.co).collect();
        let hl: Vec<[f32; 2]> = k.iter().map(|x| x.handle_left).collect();
        let hr: Vec<[f32; 2]> = k.iter().map(|x| x.handle_right).collect();
        let params: Vec<[f32; 3]> = k.iter().map(|x| [x.back, x.amplitude, x.period]).collect();
        let sel: Vec<[bool; 3]> = k
            .iter()
            .map(|x| {
                [
                    x.select_left_handle,
                    x.select_control_point,
                    x.select_right_handle,
                ]
            })
            .collect();
        let names = |f: &dyn Fn(&frap_anim::Keyframe) -> &'static str| {
            J::Array(k.iter().map(|x| json!(f(x))).collect())
        };
        Ok(json!({
            "data_path": fc.data_path.as_str(),
            "array_index": fc.array_index,
            "group": fc.group.as_ref().map_or(J::Null, |g| json!(g.as_str())),
            "extrapolation": fc.extrapolation.identifier(),
            "auto_smoothing": fc.auto_smoothing.as_str(),
            "mute": fc.mute,
            "hide": fc.hide,
            "lock": fc.lock,
            "select": fc.select,
            "is_valid": fc.is_valid,
            "color_mode": fc.color_mode.as_str(),
            "color": f32s(&fc.color),
            "discrete_values": fc.discrete_values,
            "int_values": fc.int_values,
            "keyframe_points": {
                "co": self.buf(&co)?,
                "handle_left": self.buf(&hl)?,
                "handle_right": self.buf(&hr)?,
                "back_amplitude_period": self.buf(&params)?,
                "select_left_control_right": self.buf(&sel)?,
                "handle_left_type": names(&|x| x.handle_left_type.identifier()),
                "handle_right_type": names(&|x| x.handle_right_type.identifier()),
                "interpolation": names(&|x| x.interpolation.identifier()),
                "easing": names(&|x| x.easing.identifier()),
                "type": names(&|x| x.r#type.identifier()),
            },
        }))
    }

    /// An armature (WP 6.4): rest bones (parents by index), bone collections, display.
    fn armature(&self, a: &frap_scene::id::Armature) -> Map<String, J> {
        let mut m = Map::new();
        let opt = |i: Option<usize>| i.map_or(J::Null, |i| json!(i));
        m.insert(
            "bones".into(),
            J::Array(
                a.bones
                    .iter()
                    .map(|b| {
                        json!({
                            "name": b.name.as_str(),
                            "parent": opt(b.parent),
                            "head_local": f32s(&b.head_local),
                            "tail_local": f32s(&b.tail_local),
                            "matrix_local": J::Array(b.matrix_local.iter().map(|r| f32s(r)).collect()),
                            "roll": f32j(b.roll),
                            "use_connect": b.use_connect,
                            "use_deform": b.use_deform,
                            "use_inherit_rotation": b.use_inherit_rotation,
                            "inherit_scale": b.inherit_scale.as_str(),
                            "use_local_location": b.use_local_location,
                            "use_relative_parent": b.use_relative_parent,
                            "envelope_distance": f32j(b.envelope_distance),
                            "envelope_weight": f32j(b.envelope_weight),
                            "use_envelope_multiply": b.use_envelope_multiply,
                            "head_radius": f32j(b.head_radius),
                            "tail_radius": f32j(b.tail_radius),
                            "bbone_segments": b.bbone_segments,
                            "hide": b.hide,
                            "hide_select": b.hide_select,
                            "select": b.select,
                            "collections": J::Array(b.collections.iter().map(|c| json!(c.as_str())).collect()),
                        })
                    })
                    .collect(),
            ),
        );
        m.insert("active_bone".into(), opt(a.active_bone));
        m.insert(
            "collections".into(),
            J::Array(
                a.collections
                    .iter()
                    .map(|c| {
                        json!({
                            "name": c.name.as_str(),
                            "parent": opt(c.parent),
                            "is_visible": c.is_visible,
                            "is_solo": c.is_solo,
                        })
                    })
                    .collect(),
            ),
        );
        m.insert("active_collection".into(), opt(a.active_collection));
        m.insert("display_type".into(), json!(a.display_type.as_str()));
        m.insert("pose_position".into(), json!(a.pose_position.as_str()));
        m.insert("show_names".into(), json!(a.show_names));
        m.insert("show_axes".into(), json!(a.show_axes));
        m.insert("use_mirror_x".into(), json!(a.use_mirror_x));
        m.insert(
            "show_bone_custom_shapes".into(),
            json!(a.show_bone_custom_shapes),
        );
        m.insert("show_bone_colors".into(), json!(a.show_bone_colors));
        m.insert("axes_position".into(), f32j(a.axes_position));
        m.insert(
            "relation_line_position".into(),
            json!(a.relation_line_position.as_str()),
        );
        m.insert("animation_data".into(), self.anim(&a.animation_data));
        m
    }

    /// An object's pose (WP 6.4): the channels of every bone and the pose options.
    fn pose(&self, p: &frap_scene::id::Pose) -> J {
        json!({
            "bones": J::Array(p.bones.iter().map(|b| json!({
                "name": b.name.as_str(),
                "location": f32s(&b.location),
                "rotation_mode": b.rotation_mode.as_str(),
                "rotation_quaternion": f32s(&b.rotation_quaternion),
                "rotation_euler": f32s(&b.rotation_euler),
                "rotation_axis_angle": f32s(&b.rotation_axis_angle),
                "scale": f32s(&b.scale),
                "lock_location": b.lock_location,
                "lock_rotation": b.lock_rotation,
                "lock_rotation_w": b.lock_rotation_w,
                "lock_scale": b.lock_scale,
                "lock_rotations_4d": b.lock_rotations_4d,
                "custom_shape": self.r(b.custom_shape),
                "select": b.select,
                "hide": b.hide,
                "select_head": b.select_head,
                "select_tail": b.select_tail,
            })).collect()),
            "use_mirror_x": p.use_mirror_x,
            "use_mirror_relative": p.use_mirror_relative,
            "use_auto_ik": p.use_auto_ik,
        })
    }

    /// A shape-key datablock (WP 6.4): its key blocks with their positions in buffers.
    fn key(&mut self, k: &frap_scene::id::Key) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("user".into(), self.r(k.user));
        m.insert("use_relative".into(), json!(k.use_relative));
        m.insert("eval_time".into(), f32j(k.eval_time));
        m.insert("uidgen".into(), json!(k.uidgen));
        let mut blocks = Vec::new();
        for b in &k.key_blocks {
            blocks.push(json!({
                "name": b.name.as_str(),
                "data": self.buf(&b.data)?,
                "value": f32j(b.value),
                "slider_min": f32j(b.slider_min),
                "slider_max": f32j(b.slider_max),
                "mute": b.mute,
                "relative_key": b.relative_key,
                "vertex_group": b.vertex_group.as_str(),
                "interpolation": b.interpolation.as_str(),
                "lock_shape": b.lock_shape,
                "select": b.select,
                "pos": f32j(b.pos),
                "uid": b.uid,
            }));
        }
        m.insert("key_blocks".into(), J::Array(blocks));
        m.insert("animation_data".into(), self.anim(&k.animation_data));
        Ok(m)
    }

    /// One `bpy.data` collection: local IDs keyed by name in Blender order, linked IDs under
    /// `"$linked": {"<library name>": {…}}`.
    fn coll<T: IdType>(
        &mut self,
        map: &IdMap<T>,
        mut f: impl FnMut(&mut Self, IdRef<T::Kind>, &T) -> Result<Map<String, J>, MapError>,
    ) -> Result<J, MapError> {
        let mut out = Map::new();
        let mut linked: Map<String, J> = Map::new();
        for (r, t) in map.iter() {
            if !self.written.contains(&r.any()) {
                continue;
            }
            let mut m = f(self, r, t)?;
            self.header(t.header(), &mut m);
            let name = t.header().name.to_string();
            match t.header().library.and_then(|l| self.main.libraries.get(l)) {
                None => {
                    out.insert(name, J::Object(m));
                }
                Some(lib) => {
                    let e = linked
                        .entry(lib.header.name.to_string())
                        .or_insert_with(|| J::Object(Map::new()));
                    if let J::Object(e) = e {
                        e.insert(name, J::Object(m));
                    }
                }
            }
        }
        if !linked.is_empty() {
            out.insert(super::LINKED.into(), J::Object(linked));
        }
        Ok(J::Object(out))
    }

    /// The whole `scene.json` tree.
    pub(crate) fn scene_json(
        &mut self,
        win: &WindowState,
        prefs: &Preferences,
    ) -> Result<J, MapError> {
        let main = self.main;
        let mut root = Map::new();
        root.insert("window".into(), self.window(win)?);
        root.insert(
            "libraries".into(),
            self.coll(&main.libraries, |w, _, l| Ok(w.library(l)))?,
        );
        root.insert(
            "scenes".into(),
            self.coll(&main.scenes, |w, _, s| w.scene(s))?,
        );
        root.insert(
            "collections".into(),
            self.coll(&main.collections, |w, _, c| Ok(w.collection(c)))?,
        );
        root.insert(
            "objects".into(),
            self.coll(&main.objects, |w, _, o| w.object(o))?,
        );
        root.insert(
            "meshes".into(),
            self.coll(&main.meshes, |w, r, m| w.mesh_id(r, m))?,
        );
        root.insert(
            "materials".into(),
            self.coll(&main.materials, |w, _, m| w.material(m))?,
        );
        root.insert(
            "node_groups".into(),
            self.coll(&main.node_groups, |w, _, t| w.tree(t))?,
        );
        root.insert(
            "cameras".into(),
            self.coll(&main.cameras, |w, _, c| w.camera(c))?,
        );
        root.insert(
            "lights".into(),
            self.coll(&main.lights, |w, _, l| w.light(l))?,
        );
        root.insert(
            "worlds".into(),
            self.coll(&main.worlds, |w, _, x| w.world(x))?,
        );
        root.insert(
            "images".into(),
            self.coll(&main.images, |w, _, i| w.image(i))?,
        );
        root.insert(
            "actions".into(),
            self.coll(&main.actions, |w, _, a| w.action(a))?,
        );
        root.insert(
            "armatures".into(),
            self.coll(&main.armatures, |w, _, a| Ok(w.armature(a)))?,
        );
        root.insert(
            "shape_keys".into(),
            self.coll(&main.shape_keys, |w, _, k| w.key(k))?,
        );
        root.insert(
            "textures".into(),
            self.coll(&main.textures, |w, _, t| w.texture(t))?,
        );
        root.insert(
            "pointclouds".into(),
            self.coll(&main.pointclouds, |w, _, p| w.pointcloud(p))?,
        );
        root.insert("preferences".into(), prefs_j(prefs));
        Ok(J::Object(root))
    }

    fn window(&self, win: &WindowState) -> Result<J, MapError> {
        let scene =
            self.main.scenes.name_of(win.scene).ok_or_else(|| {
                MapError::invalid("window.scene", "the window scene does not exist")
            })?;
        Ok(json!({
            "scene": scene,
            "view_layer": win.view_layer.as_str(),
            "view3d": {
                "view_matrix": mat4j(&win.view3d.view_matrix),
                "shading": ser(&win.view3d.shading)?,
            },
        }))
    }

    fn library(&self, l: &Library) -> Map<String, J> {
        let mut m = Map::new();
        m.insert("filepath".into(), json!(l.filepath.as_str()));
        m.insert("parent".into(), self.r(l.parent));
        m.insert("is_archive".into(), json!(l.is_archive));
        m
    }

    fn scene(&mut self, s: &Scene) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("camera".into(), self.r(s.camera));
        m.insert("world".into(), self.r(s.world));
        m.insert("frame_start".into(), json!(s.frame_start));
        m.insert("frame_end".into(), json!(s.frame_end));
        m.insert("frame_current".into(), json!(s.frame_current));
        m.insert("frame_step".into(), json!(s.frame_step));
        m.insert("render".into(), ser(&s.render)?);
        m.insert("view_settings".into(), ser(&s.view_settings)?);
        m.insert("display_settings".into(), ser(&s.display_settings)?);
        m.insert("display".into(), ser(&s.display)?);
        m.insert("eevee".into(), ser(&s.eevee)?);
        m.insert("cursor".into(), ser(&s.cursor)?);
        m.insert("tool_settings".into(), ser(&s.tool_settings)?);
        m.insert("unit_settings".into(), ser(&s.unit_settings)?);
        let mut coll = self.collection(&s.collection);
        coll.insert("name".into(), json!(s.collection.header.name.as_str()));
        m.insert("collection".into(), J::Object(coll));
        m.insert(
            "view_layers".into(),
            J::Array(s.view_layers.iter().map(|v| self.view_layer(v)).collect()),
        );
        m.insert("animation_data".into(), self.anim(&s.animation_data));
        // M7 (WP 7.6): written only when present, so older readers keep reading the files
        // frap writes without them.
        if !s.timeline_markers.is_empty() {
            let markers = s
                .timeline_markers
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name.as_str(),
                        "frame": t.frame,
                        "select": t.select,
                        "camera": self.r(t.camera),
                    })
                })
                .collect();
            m.insert("timeline_markers".into(), J::Array(markers));
        }
        let gv = &s.gltf2_variants;
        if *gv != frap_scene::id::SceneGltfVariants::default() {
            let variants = gv
                .variants
                .iter()
                .map(|v| json!({"name": v.name.as_str(), "variant_idx": v.variant_idx}))
                .collect();
            m.insert(
                "gltf2_KHR_materials_variants_variants".into(),
                J::Array(variants),
            );
            m.insert("gltf2_active_variant".into(), json!(gv.active_variant));
        }
        Ok(m)
    }

    fn view_layer(&self, v: &ViewLayer) -> J {
        let bases: Vec<J> = v
            .bases
            .iter()
            .filter(|b| self.written.contains(&b.object.any()))
            .map(|b| {
                json!({
                    "object": self.r(Some(b.object)),
                    "select": b.flag.contains(frap_scene::BaseFlag::SELECTED),
                    "hide": b.flag.contains(frap_scene::BaseFlag::HIDDEN),
                })
            })
            .collect();
        json!({
            "name": v.name.as_str(),
            "bases": bases,
            "active": self.r(v.active),
            "layer_collection": self.layer_collection(&v.layer_collection),
            "active_layer_collection": v.active_layer_collection.0.iter().collect::<Vec<_>>(),
        })
    }

    fn layer_collection(&self, lc: &LayerCollection) -> J {
        let mut j = json!({
            "collection": self.r(lc.collection),
            "exclude": lc.exclude,
            "hide_viewport": lc.hide_viewport,
            "holdout": lc.holdout,
            "indirect_only": lc.indirect_only,
            "children": lc.children.iter().map(|c| self.layer_collection(c)).collect::<Vec<_>>(),
        });
        // `LAYER_COLLECTION_PREVIOUSLY_EXCLUDED` (saved in .blend files too); written only when
        // set, so files without it read as before (M5+M6 stabilization round 6).
        if lc.previously_excluded
            && let Some(m) = j.as_object_mut()
        {
            m.insert("previously_excluded".into(), J::Bool(true));
        }
        j
    }

    fn collection(&self, c: &Collection) -> Map<String, J> {
        let mut m = Map::new();
        m.insert(
            "objects".into(),
            J::Array(
                c.objects
                    .iter()
                    .filter(|o| self.written.contains(&o.any()))
                    .map(|o| self.r(Some(*o)))
                    .collect(),
            ),
        );
        m.insert(
            "children".into(),
            J::Array(
                c.children
                    .iter()
                    .filter(|o| self.written.contains(&o.any()))
                    .map(|o| self.r(Some(*o)))
                    .collect(),
            ),
        );
        m.insert("hide_select".into(), json!(c.hide_select));
        m.insert("hide_viewport".into(), json!(c.hide_viewport));
        m.insert("hide_render".into(), json!(c.hide_render));
        m.insert("color_tag".into(), json!(c.color_tag.as_str()));
        m
    }

    fn object(&mut self, o: &Object) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("type".into(), json!(o.object_type()));
        m.insert("data".into(), self.idref(o.data));
        m.insert("parent".into(), self.r(o.parent));
        m.insert("parent_type".into(), json!(o.parent_type.as_str()));
        m.insert("parent_bone".into(), json!(o.parent_bone.as_str()));
        m.insert("parent_vertices".into(), json!(o.parent_vertices));
        m.insert(
            "matrix_parent_inverse".into(),
            mat4j(&o.matrix_parent_inverse),
        );
        m.insert("location".into(), f32s(&o.location));
        m.insert("rotation_mode".into(), json!(o.rotation_mode.as_str()));
        m.insert("rotation_euler".into(), f32s(&o.rotation_euler));
        m.insert("rotation_quaternion".into(), f32s(&o.rotation_quaternion));
        m.insert("rotation_axis_angle".into(), f32s(&o.rotation_axis_angle));
        m.insert("scale".into(), f32s(&o.scale));
        m.insert("delta_location".into(), f32s(&o.delta_location));
        m.insert("delta_rotation_euler".into(), f32s(&o.delta_rotation_euler));
        m.insert(
            "delta_rotation_quaternion".into(),
            f32s(&o.delta_rotation_quaternion),
        );
        m.insert("delta_scale".into(), f32s(&o.delta_scale));
        m.insert("mode".into(), json!(o.mode.as_str()));
        m.insert(
            "modifiers".into(),
            J::Array(
                o.modifiers
                    .iter()
                    .map(|md| modifier_j(md, &|a| self.idref(a)))
                    .collect::<Result<_, _>>()?,
            ),
        );
        m.insert(
            "material_slots".into(),
            J::Array(
                o.material_slot_overrides
                    .iter()
                    .map(|s| {
                        json!({
                            "link": match s.link { SlotLink::Data => "DATA", SlotLink::Object => "OBJECT" },
                            "material": self.r(s.material),
                        })
                    })
                    .collect(),
            ),
        );
        m.insert(
            "active_material_index".into(),
            json!(o.active_material_index),
        );
        m.insert(
            "vertex_groups_active_index".into(),
            json!(o.vertex_groups_active_index),
        );
        m.insert(
            "pose".into(),
            o.pose.as_ref().map_or(J::Null, |p| self.pose(p)),
        );
        m.insert("hide_viewport".into(), json!(o.hide_viewport));
        m.insert("hide_render".into(), json!(o.hide_render));
        m.insert("hide_select".into(), json!(o.hide_select));
        m.insert("display_type".into(), json!(o.display_type.as_str()));
        m.insert("color".into(), f32s(&o.color));
        m.insert(
            "empty_display_type".into(),
            json!(o.empty_display_type.as_str()),
        );
        m.insert("empty_display_size".into(), f32j(o.empty_display_size));
        m.insert(
            "active_shape_key_index".into(),
            json!(o.active_shape_key_index),
        );
        m.insert("show_name".into(), json!(o.show_name));
        m.insert("show_axis".into(), json!(o.show_axis));
        m.insert("show_wire".into(), json!(o.show_wire));
        m.insert("show_all_edges".into(), json!(o.show_all_edges));
        m.insert("show_texture_space".into(), json!(o.show_texture_space));
        m.insert("show_in_front".into(), json!(o.show_in_front));
        m.insert("show_bounds".into(), json!(o.show_bounds));
        m.insert("show_only_shape_key".into(), json!(o.show_only_shape_key));
        m.insert(
            "display_bounds_type".into(),
            json!(o.display_bounds_type.as_str()),
        );
        m.insert("animation_data".into(), self.anim(&o.animation_data));
        Ok(m)
    }

    fn mesh_id(&mut self, r: IdRef<MeshId>, me: &Mesh) -> Result<Map<String, J>, MapError> {
        // Edit-mode meshes are written from their cached view (§6.3, §15).
        match (self.mesh_view)(r) {
            Some(v) => self.mesh(&v),
            None => self.mesh(me),
        }
    }

    /// The `attributes` array of a mesh or point cloud (`name`, `domain`, `data_type`, `data`).
    fn attributes<'x>(
        &mut self,
        owner: &str,
        it: impl Iterator<Item = &'x frap_mesh::Attribute>,
    ) -> Result<J, MapError> {
        let mut attrs = Vec::new();
        for a in it {
            let data = match &a.data {
                AttrData::Float(v) => self.buf(v.as_slice())?,
                AttrData::Int(v) => self.buf(v.as_slice())?,
                AttrData::Int8(v) => self.buf(v.as_slice())?,
                AttrData::Bool(v) => self.buf(v.as_slice())?,
                AttrData::Float2(v) => self.buf(v.as_slice())?,
                AttrData::Float3(v) => self.buf(v.as_slice())?,
                AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
                    self.buf(v.as_slice())?
                }
                AttrData::ByteColor(v) => self.buf(v.as_slice())?,
                AttrData::Int16x2(v) => self.buf(v.as_slice())?,
                AttrData::Int32x2(v) => self.buf(v.as_slice())?,
                AttrData::Float4x4(v) => {
                    let flat: Vec<[f32; 16]> = v
                        .iter()
                        .map(|m| {
                            let mut o = [0.0; 16];
                            for (i, row) in m.iter().enumerate() {
                                o[i * 4..i * 4 + 4].copy_from_slice(row);
                            }
                            o
                        })
                        .collect();
                    self.buf(&flat)?
                }
                AttrData::String(v) => J::Array(v.iter().map(|s| json!(s.as_str())).collect()),
                other => {
                    return Err(MapError::invalid(
                        &format!("{owner}.attributes[{:?}]", a.name),
                        format!("cannot save {} attributes", other.data_type()),
                    ));
                }
            };
            attrs.push(json!({
                "name": a.name.as_str(),
                "domain": a.domain.identifier(),
                "data_type": a.data.data_type(),
                "data": data,
            }));
        }
        Ok(J::Array(attrs))
    }

    /// A point cloud (WP 3.7): its point count, attributes, material slots, animation data.
    fn pointcloud(&mut self, pc: &PointCloud) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("points".into(), json!(pc.len()));
        let owner = format!("pointclouds[{:?}]", pc.header.name);
        let attrs = self.attributes(&owner, pc.attributes().iter())?;
        m.insert("attributes".into(), attrs);
        m.insert(
            "materials".into(),
            J::Array(pc.materials.iter().map(|x| self.r(*x)).collect()),
        );
        m.insert("animation_data".into(), self.anim(&pc.animation_data));
        Ok(m)
    }

    fn mesh(&mut self, me: &Mesh) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("face_offsets".into(), self.buf(me.face_offsets())?);
        let owner = format!("meshes[{:?}]", me.header.name);
        let attrs = self.attributes(&owner, me.attributes().iter())?;
        m.insert("attributes".into(), attrs);
        m.insert(
            "deform_verts".into(),
            match me.deform_verts() {
                None => J::Null,
                Some(dv) => {
                    let mut offsets = Vec::with_capacity(dv.len() + 1);
                    let mut groups: Vec<u32> = Vec::new();
                    let mut weights: Vec<f32> = Vec::new();
                    offsets.push(0i32);
                    for d in dv {
                        for (g, w) in &d.weights {
                            groups.push(*g);
                            weights.push(*w);
                        }
                        offsets.push(groups.len() as i32);
                    }
                    json!({
                        "offsets": self.buf(&offsets)?,
                        "groups": self.buf(&groups)?,
                        "weights": self.buf(&weights)?,
                    })
                }
            },
        );
        m.insert(
            "materials".into(),
            J::Array(me.materials.iter().map(|x| self.r(*x)).collect()),
        );
        m.insert(
            "vertex_group_names".into(),
            J::Array(
                me.vertex_group_names
                    .iter()
                    .map(|n| json!(n.as_str()))
                    .collect(),
            ),
        );
        m.insert(
            "vertex_group_locks".into(),
            J::Array(
                me.vertex_group_locks
                    .iter()
                    .map(|n| json!(n.as_str()))
                    .collect(),
            ),
        );
        m.insert("shape_keys".into(), self.r(me.shape_keys));
        let o = |s: &Option<smol_str::SmolStr>| s.as_ref().map_or(J::Null, |s| json!(s.as_str()));
        m.insert("uv_active".into(), o(&me.uv_active));
        m.insert("uv_active_render".into(), o(&me.uv_active_render));
        m.insert("color_active".into(), o(&me.color_active));
        m.insert("color_default".into(), o(&me.color_default));
        if let Some(a) = &me.attributes_active {
            m.insert("attributes_active".into(), json!(a.as_str()));
        }
        m.insert("use_auto_texspace".into(), json!(me.texspace.auto));
        m.insert("texspace_location".into(), f32s(&me.texspace.location));
        m.insert("texspace_size".into(), f32s(&me.texspace.size));
        // The glTF add-on's variant data (M7 W0, persisted by WP 7.6), only when present.
        let gv = &me.gltf2_variants;
        if *gv != frap_mesh::gltf_variants::MeshGltfVariants::default() {
            let data = gv
                .mesh_data
                .iter()
                .map(|p| {
                    json!({
                        "name": p.name.as_str(),
                        "material_slot_index": p.material_slot_index,
                        "material": self.r(p.material),
                        "variants": p
                            .variants
                            .iter()
                            .map(|v| {
                                json!({
                                    "name": v.name.as_str(),
                                    "variant": {
                                        "variant_idx": v.variant_idx,
                                        "name": v.variant_name.as_str(),
                                    },
                                })
                            })
                            .collect::<Vec<_>>(),
                        "active_variant_idx": p.active_variant_idx,
                    })
                })
                .collect();
            m.insert("gltf2_variant_mesh_data".into(), J::Array(data));
            let defaults = gv
                .default_materials
                .iter()
                .map(|d| {
                    json!({
                        "name": d.name.as_str(),
                        "material_slot_index": d.material_slot_index,
                        "default_material": self.r(d.default_material),
                    })
                })
                .collect();
            m.insert("gltf2_variant_default_materials".into(), J::Array(defaults));
            m.insert("gltf2_variant_pointer".into(), json!(gv.pointer.as_str()));
        }
        Ok(m)
    }

    fn material(&mut self, ma: &Material) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("node_tree".into(), J::Object(self.tree(&ma.node_tree)?));
        m.insert(
            "surface_render_method".into(),
            json!(ma.surface_render_method.as_str()),
        );
        m.insert("blend_method".into(), json!(ma.blend_method.as_str()));
        m.insert(
            "use_backface_culling".into(),
            json!(ma.use_backface_culling),
        );
        m.insert("diffuse_color".into(), f32s(&ma.diffuse_color));
        m.insert("metallic".into(), f32j(ma.metallic));
        m.insert("roughness".into(), f32j(ma.roughness));
        m.insert("alpha_threshold".into(), f32j(ma.alpha_threshold));
        m.insert("pass_index".into(), json!(ma.pass_index));
        m.insert("use_nodes".into(), json!(ma.use_nodes));
        m.insert("animation_data".into(), self.anim(&ma.animation_data));
        Ok(m)
    }

    fn world(&mut self, w: &World) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("node_tree".into(), J::Object(self.tree(&w.node_tree)?));
        m.insert("color".into(), f32s(&w.color));
        m.insert("use_nodes".into(), json!(w.use_nodes));
        m.insert("animation_data".into(), self.anim(&w.animation_data));
        Ok(m)
    }

    pub(crate) fn tree(&mut self, t: &NodeTree) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("name".into(), json!(t.header.name.as_str()));
        m.insert(
            "bl_idname".into(),
            json!(match t.ty {
                NodeTreeType::Geometry => "GeometryNodeTree",
                _ => "ShaderNodeTree",
            }),
        );
        m.insert(
            "nodes".into(),
            J::Array(
                t.nodes
                    .iter()
                    .map(|n| self.node(n))
                    .collect::<Result<_, _>>()?,
            ),
        );
        m.insert(
            "links".into(),
            J::Array(
                t.links
                    .iter()
                    .map(|l| {
                        json!({
                            "from_node": l.from_node,
                            "from_socket": l.from_socket.as_str(),
                            "to_node": l.to_node,
                            "to_socket": l.to_socket.as_str(),
                        })
                    })
                    .collect(),
            ),
        );
        m.insert(
            "interface".into(),
            J::Array(
                t.interface
                    .iter()
                    .map(|s| {
                        json!({
                            "name": s.name.as_str(),
                            "identifier": s.identifier.as_str(),
                            "in_out": match s.in_out { InOut::Input => "INPUT", InOut::Output => "OUTPUT" },
                            "socket_type": socket_type_id(s.socket_type),
                            "default_value": self.socket_value(&s.default_value),
                        })
                    })
                    .collect(),
            ),
        );
        m.insert("next_node_uid".into(), json!(t.next_node_uid));
        if t.next_interface_uid != 0 {
            m.insert("next_interface_uid".into(), json!(t.next_interface_uid));
        }
        m.insert("animation_data".into(), self.anim(&t.animation_data));
        Ok(m)
    }

    fn node(&mut self, n: &Node) -> Result<J, MapError> {
        let mut props = Map::new();
        for (k, v) in &n.props.values {
            props.insert(
                (*k).to_owned(),
                self.value(v, &format!("node {:?}.{k}", n.name))?,
            );
        }
        Ok(json!({
            "uid": n.uid,
            "name": n.name.as_str(),
            "bl_idname": n.bl_idname,
            "label": n.label,
            "location": f32s(&n.location),
            "width": f32j(n.width),
            "parent": n.parent,
            "mute": n.mute,
            "hide": n.hide,
            "inputs": n.inputs.iter().map(|s| self.socket(s)).collect::<Vec<_>>(),
            "outputs": n.outputs.iter().map(|s| self.socket(s)).collect::<Vec<_>>(),
            "props": props,
        }))
    }

    fn socket(&self, s: &Socket) -> J {
        json!({
            "identifier": s.identifier.as_str(),
            "name": s.name.as_str(),
            "type": socket_type_id(s.ty),
            "enabled": s.enabled,
            "hide_value": s.hide_value,
            "default_value": self.socket_value(&s.default_value),
        })
    }

    fn socket_value(&self, v: &SocketValue) -> J {
        match v {
            SocketValue::Float(f) => f32j(*f),
            SocketValue::Int(i) => json!(i),
            SocketValue::Bool(b) => json!(b),
            SocketValue::Vector(a) | SocketValue::Color(a) => f32s(a),
            SocketValue::Rotation(a) => f32s(a),
            SocketValue::String(s) | SocketValue::Menu(s) => json!(s.as_str()),
            SocketValue::Id(a) => self.idref(*a),
            _ => J::Null,
        }
    }

    /// An RNA value of a node property (the variant is recovered from the node type's default
    /// on load).
    pub(crate) fn value(&self, v: &Value, path: &str) -> Result<J, MapError> {
        Ok(match v {
            Value::None => J::Null,
            Value::Bool(b) => json!(b),
            Value::Int(i) => json!(i),
            Value::Float(f) => f32j(*f),
            Value::Double(f) => f64j(*f),
            Value::Str(s) | Value::Enum(s) => json!(s.as_str()),
            Value::EnumSet(s) => J::Array(s.iter().map(|x| json!(x.as_str())).collect()),
            Value::BoolArray(a) => json!(a.as_slice()),
            Value::IntArray(a) => json!(a.as_slice()),
            Value::FloatArray { data, .. } => f32s(data),
            Value::Id(a) => self.idref(*a),
            other => {
                return Err(MapError::invalid(
                    path,
                    format!("cannot save the value {other:?}"),
                ));
            }
        })
    }

    fn camera(&mut self, c: &Camera) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        let mut fields = ser(&c.fields)?;
        // `#[serde(skip)]` ID pointers of the generated struct.
        if let J::Object(f) = &mut fields {
            f.insert("custom_shader".into(), self.idref(c.fields.custom_shader));
            if let Some(J::Object(dof)) = f.get_mut("dof") {
                dof.insert("focus_object".into(), self.idref(c.fields.dof.focus_object));
            }
            m.extend(std::mem::take(f));
        }
        m.insert("animation_data".into(), self.anim(&c.animation_data));
        Ok(m)
    }

    fn texture(&mut self, t: &Texture) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("type".into(), json!(t.r#type.as_str()));
        if let J::Object(f) = ser(&t.fields)? {
            m.extend(f);
        }
        // `#[serde(skip)]` ID pointer of the generated struct.
        m.insert("image".into(), self.idref(t.fields.image));
        m.insert("animation_data".into(), self.anim(&t.animation_data));
        Ok(m)
    }

    fn light(&mut self, l: &Light) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("type".into(), json!(l.r#type.as_str()));
        if let J::Object(f) = ser(&l.fields)? {
            m.extend(f);
        }
        m.insert("animation_data".into(), self.anim(&l.animation_data));
        Ok(m)
    }

    fn image(&mut self, i: &Image) -> Result<Map<String, J>, MapError> {
        let mut m = Map::new();
        m.insert("source".into(), json!(i.source.as_str()));
        m.insert("filepath".into(), json!(i.filepath));
        m.insert(
            "packed_file".into(),
            match &i.packed_file {
                None => J::Null,
                Some(p) => {
                    let ext = ext_of_mime(&p.mime, &i.file_format);
                    let r = self.images.insert(p.bytes.clone(), ext).map_err(|e| {
                        MapError::invalid(&format!("images[{:?}]", i.header.name), e.to_string())
                    })?;
                    json!({
                        "data": ser(&r)?,
                        "mime": p.mime.as_str(),
                        "filepath": i.packed_filepath,
                    })
                }
            },
        );
        m.insert("file_format".into(), json!(i.file_format.as_str()));
        m.insert("generated_type".into(), json!(i.generated_type.as_str()));
        m.insert("generated_width".into(), json!(i.generated_width));
        m.insert("generated_height".into(), json!(i.generated_height));
        m.insert("generated_color".into(), f32s(&i.generated_color));
        m.insert("use_generated_float".into(), json!(i.use_generated_float));
        m.insert("generated_depth".into(), json!(i.generated_depth));
        m.insert("colorspace_settings".into(), ser(&i.colorspace_settings)?);
        m.insert("alpha_mode".into(), json!(i.alpha_mode.as_str()));
        m.insert("size".into(), json!(i.size));
        m.insert("animation_data".into(), self.anim(&i.animation_data));
        Ok(m)
    }
}

/// Lowercase extension of a packed image (`image/png` → `png`).
fn ext_of_mime(mime: &str, file_format: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/ktx2" => "ktx2",
        "image/x-exr" | "image/exr" => "exr",
        "image/vnd.radiance" | "image/x-hdr" => "hdr",
        "image/bmp" | "image/x-bmp" | "image/x-ms-bmp" => "bmp",
        "image/x-tga" | "image/x-targa" | "image/tga" => "tga",
        _ => match file_format {
            "JPEG" => "jpg",
            "WEBP" => "webp",
            "OPEN_EXR" | "OPEN_EXR_MULTILAYER" => "exr",
            "HDR" => "hdr",
            "BMP" => "bmp",
            _ => "png",
        },
    }
}

/// `Modifier` → `{"name", "type", show flags, "persistent_uid", <settings…>}` (§15).
/// One modifier: the base properties, the settings flattened, and the settings' ID pointers
/// (operand objects, node groups, textures; M4 W0) through `idref`.
fn modifier_j(md: &Modifier, idref: &dyn Fn(Option<IdAny>) -> J) -> Result<J, MapError> {
    let mut m = Map::new();
    m.insert("name".into(), json!(md.name.as_str()));
    m.insert("type".into(), json!(md.settings.type_identifier()));
    m.insert("show_viewport".into(), json!(md.show_viewport));
    m.insert("show_render".into(), json!(md.show_render));
    m.insert("show_in_editmode".into(), json!(md.show_in_editmode));
    m.insert("show_on_cage".into(), json!(md.show_on_cage));
    m.insert("show_expanded".into(), json!(md.show_expanded));
    m.insert("is_active".into(), json!(md.is_active));
    m.insert("use_pin_to_last".into(), json!(md.use_pin_to_last));
    m.insert("persistent_uid".into(), json!(md.persistent_uid));
    // Externally tagged enum: {"Bevel": {…}}; flatten the settings into the modifier.
    if let J::Object(tagged) = ser(&md.settings)?
        && let Some((_, J::Object(fields))) = tagged.into_iter().next()
    {
        for (k, v) in fields {
            if k != "type" {
                m.insert(k, v);
            }
        }
    }
    for (k, r) in md.settings.id_refs() {
        m.insert(k.into(), idref(r));
    }
    Ok(J::Object(m))
}

pub(crate) fn socket_type_id(t: SocketType) -> &'static str {
    match t {
        SocketType::Value => "VALUE",
        SocketType::Int => "INT",
        SocketType::Boolean => "BOOLEAN",
        SocketType::Vector => "VECTOR",
        SocketType::Rgba => "RGBA",
        SocketType::Shader => "SHADER",
        SocketType::String => "STRING",
        SocketType::Rotation => "ROTATION",
        SocketType::Matrix => "MATRIX",
        SocketType::Menu => "MENU",
        SocketType::Geometry => "GEOMETRY",
        SocketType::Object => "OBJECT",
        SocketType::Image => "IMAGE",
        SocketType::Collection => "COLLECTION",
        SocketType::Material => "MATERIAL",
        SocketType::Texture => "TEXTURE",
        _ => "CUSTOM",
    }
}

fn prefs_j(p: &Preferences) -> J {
    let mut addons = Map::new();
    for (module, values) in &p.addons {
        let mut m = Map::new();
        for (k, v) in values {
            let j = match v {
                Value::Bool(b) => json!(b),
                Value::Int(i) => json!(i),
                Value::Float(f) => f32j(*f),
                Value::Double(f) => f64j(*f),
                Value::Str(s) | Value::Enum(s) => json!(s.as_str()),
                _ => continue,
            };
            m.insert(k.to_string(), j);
        }
        addons.insert(module.to_string(), J::Object(m));
    }
    json!({ "addons": addons })
}

#[cfg(test)]
mod ext_tests {
    use super::ext_of_mime;

    #[test]
    fn packed_extensions() {
        // The MIME types `Image.pack()` records (frap-ops-data `mime_of`).
        for (mime, ext) in [
            ("image/png", "png"),
            ("image/jpeg", "jpg"),
            ("image/webp", "webp"),
            ("image/x-exr", "exr"),
            ("image/vnd.radiance", "hdr"),
            ("image/bmp", "bmp"),
            ("image/x-tga", "tga"),
        ] {
            assert_eq!(ext_of_mime(mime, "PNG"), ext, "{mime}");
        }
        assert_eq!(ext_of_mime("application/octet-stream", "OPEN_EXR"), "exr");
        assert_eq!(ext_of_mime("application/octet-stream", "BMP"), "bmp");
        assert_eq!(ext_of_mime("application/octet-stream", "TARGA"), "png");
    }
}
