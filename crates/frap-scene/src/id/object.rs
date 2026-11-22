// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Object` (§5.6). The field set is frozen at M1 W0 (verifier fix W0-21); WP 1.10
//! owns the bindings, hooks and behaviour. Fields are named after their RNA identifier, so
//! serde keys in `.frap` equal RNA identifiers. Enum properties are stored as their RNA
//! identifier (`SmolStr`).

use frap_anim::AnimData;
use frap_rna::*;
use glam::Mat4;
use smol_str::SmolStr;

use crate::context::ObjectMode;
use crate::id::armature::Pose;
use crate::id::modifier::Modifier;

/// `MaterialSlot.link`: where a slot's material comes from.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SlotLink {
    /// `'DATA'` (the material list of `Object.data`; Blender's default).
    #[default]
    Data,
    /// `'OBJECT'` (this object's own material).
    Object,
}

/// Per-object material-slot override (§5.6: slots are derived from `data.materials`; the
/// object stores only `link` and its own material, padded lazily up to the slot index).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotOverride {
    /// `MaterialSlot.link`
    pub link: SlotLink,
    /// The object-linked material (`ob->mat[i]`), used when `link == Object`.
    pub material: Option<IdRef<MaterialId>>,
}

/// An object.
#[derive(Clone, Debug)]
pub struct Object {
    /// ID header.
    pub header: IdHeader,
    /// `Object.data` (fixed at creation; determines the read-only `type`).
    pub data: Option<IdAny>,

    /// `Object.location`
    pub location: [f32; 3],
    /// `Object.rotation_mode` ("XYZ" … "ZYX", "QUATERNION", "AXIS_ANGLE")
    pub rotation_mode: SmolStr,
    /// `Object.rotation_euler`
    pub rotation_euler: [f32; 3],
    /// `Object.rotation_quaternion` (w, x, y, z); kept whatever the mode, as Blender does.
    pub rotation_quaternion: [f32; 4],
    /// `Object.rotation_axis_angle` (angle, x, y, z)
    pub rotation_axis_angle: [f32; 4],
    /// `Object.scale`
    pub scale: [f32; 3],
    /// `Object.delta_location`
    pub delta_location: [f32; 3],
    /// `Object.delta_rotation_euler`
    pub delta_rotation_euler: [f32; 3],
    /// `Object.delta_rotation_quaternion` (w, x, y, z)
    pub delta_rotation_quaternion: [f32; 4],
    /// `Object.delta_scale`
    pub delta_scale: [f32; 3],

    /// `Object.parent`
    pub parent: Option<IdRef<ObjectId>>,
    /// `Object.parent_type` ("OBJECT", "ARMATURE", "LATTICE", "VERTEX", "VERTEX_3", "BONE")
    pub parent_type: SmolStr,
    /// `Object.parent_bone`
    pub parent_bone: SmolStr,
    /// `Object.parent_vertices`
    pub parent_vertices: [i32; 3],
    /// `Object.parent_bone_head_tail_factor` (where along the parent bone a `BONE` child sits:
    /// 0 head, 1 tail; WP 6.5)
    pub parent_bone_head_tail_factor: f32,
    /// `Object.matrix_parent_inverse` (glam is column-major; RNA reads it row-major, §13.3)
    pub matrix_parent_inverse: Mat4,

    /// `Object.modifiers` (evaluators from M4)
    pub modifiers: Vec<Modifier>,
    /// Material-slot overrides (`MaterialSlot.link`/`.material` when `link == 'OBJECT'`);
    /// the slot count is `len(data.materials)`, never this vector's length (§5.6).
    pub material_slot_overrides: Vec<SlotOverride>,
    /// `Object.active_material_index`
    pub active_material_index: i32,
    /// `Object.vertex_groups.active_index` (the names live on the Mesh, §5.6)
    pub vertex_groups_active_index: i32,
    /// `Object.pose` (armatures, M6)
    pub pose: Option<Pose>,
    /// `Object.mode`
    pub mode: ObjectMode,

    /// `Object.hide_viewport`
    pub hide_viewport: bool,
    /// `Object.hide_render`
    pub hide_render: bool,
    /// `Object.hide_select`
    pub hide_select: bool,
    /// `Object.display_type` ("TEXTURED")
    pub display_type: SmolStr,
    /// `Object.color` (Workbench OBJECT colour, glTF has none)
    pub color: [f32; 4],
    /// `Object.empty_display_type` ("PLAIN_AXES" for `objects.new(…, None)`)
    pub empty_display_type: SmolStr,
    /// `Object.empty_display_size`
    pub empty_display_size: f32,
    /// `Object.active_shape_key_index`
    pub active_shape_key_index: i32,
    /// `Object.show_name`
    pub show_name: bool,
    /// `Object.show_axis`
    pub show_axis: bool,
    /// `Object.show_wire`
    pub show_wire: bool,
    /// `Object.show_all_edges`
    pub show_all_edges: bool,
    /// `Object.show_texture_space`
    pub show_texture_space: bool,
    /// `Object.show_in_front`
    pub show_in_front: bool,
    /// `Object.show_bounds`
    pub show_bounds: bool,
    /// `Object.show_only_shape_key`
    pub show_only_shape_key: bool,
    /// `Object.display_bounds_type` ("BOX")
    pub display_bounds_type: SmolStr,
    /// `Object.animation_data`
    pub animation_data: Option<AnimData>,
    /// `Object.pass_index` (`ob->index`, 0..32767; WP 8.3.4).
    pub pass_index: i32,
    /// `Object.instance_type` (the `OB_DUPLI` bits of `transflag`: "NONE", "VERTS", "FACES",
    /// "COLLECTION"; WP 8.3.4).
    pub instance_type: SmolStr,
    /// `Object.instance_collection` (a user-counting reference; WP 8.3.4).
    pub instance_collection: Option<IdRef<CollectionId>>,
}

impl Object {
    /// New object with identity transform and the values of `bpy.data.objects.new(name, data)`
    /// (verified on 5.2.4).
    pub fn new(name: &str, data: Option<IdAny>) -> Object {
        Object {
            header: IdHeader::new(name),
            data,
            location: [0.0; 3],
            rotation_mode: "XYZ".into(),
            rotation_euler: [0.0; 3],
            rotation_quaternion: [1.0, 0.0, 0.0, 0.0],
            rotation_axis_angle: [0.0, 0.0, 1.0, 0.0],
            scale: [1.0; 3],
            delta_location: [0.0; 3],
            delta_rotation_euler: [0.0; 3],
            delta_rotation_quaternion: [1.0, 0.0, 0.0, 0.0],
            delta_scale: [1.0; 3],
            parent: None,
            parent_type: "OBJECT".into(),
            parent_bone: SmolStr::default(),
            parent_vertices: [0; 3],
            parent_bone_head_tail_factor: 1.0,
            matrix_parent_inverse: Mat4::IDENTITY,
            modifiers: Vec::new(),
            material_slot_overrides: Vec::new(),
            active_material_index: 0,
            vertex_groups_active_index: 0,
            pose: None,
            mode: ObjectMode::Object,
            hide_viewport: false,
            hide_render: false,
            hide_select: false,
            display_type: "TEXTURED".into(),
            color: [1.0; 4],
            empty_display_type: "PLAIN_AXES".into(),
            empty_display_size: 1.0,
            active_shape_key_index: 0,
            show_name: false,
            show_axis: false,
            show_wire: false,
            show_all_edges: false,
            show_texture_space: false,
            show_in_front: false,
            show_bounds: false,
            show_only_shape_key: false,
            display_bounds_type: "BOX".into(),
            animation_data: None,
            pass_index: 0,
            instance_type: "NONE".into(),
            instance_collection: None,
        }
    }
    /// `Object.type`.
    pub fn object_type(&self) -> &'static str {
        match self.data.map(|d| d.code) {
            None => "EMPTY",
            Some(IdCode::ME) => "MESH",
            Some(IdCode::CA) => "CAMERA",
            Some(IdCode::LA) => "LIGHT",
            Some(IdCode::AR) => "ARMATURE",
            Some(IdCode::PT) => "POINTCLOUD",
            Some(_) => "EMPTY",
        }
    }
}

crate::id::id_rna! { Object as frap_spec_rna::OBJECT, "OBJECT" {
    data => hook,
    type => readonly(|o| o.object_type()),
    mode => readonly(|o| o.mode.as_str()),
    parent => hook,
    parent_type => parent_type (set = hook),
    parent_vertices => parent_vertices,
    parent_bone => parent_bone (set = hook),
    parent_bone_head_tail_factor => parent_bone_head_tail_factor,
    material_slots => hook_coll,
    active_material => hook,
    active_material_index => active_material_index (set = hook),
    location => location,
    rotation_quaternion => rotation_quaternion,
    rotation_axis_angle => rotation_axis_angle,
    rotation_euler => rotation_euler (tag = Euler(rotation_mode)),
    rotation_mode => rotation_mode (set = hook),
    scale => scale,
    delta_location => delta_location,
    delta_rotation_euler => delta_rotation_euler (tag = Euler(rotation_mode)),
    delta_rotation_quaternion => delta_rotation_quaternion,
    delta_scale => delta_scale,
    matrix_world => hook,
    // Evaluated bounds (D-04): the hooks live in frap-ops-data, which can evaluate.
    dimensions => hook,
    bound_box => hook,
    matrix_local => get(|o| o.matrix_local()) set(|o, m: Mat4| o.set_matrix_local(&m)),
    matrix_basis => get(|o| o.matrix_basis()) set(|o, m: Mat4| o.set_matrix_basis(&m)),
    matrix_parent_inverse => matrix_parent_inverse,
    modifiers => coll(modifiers, item = frap_spec_rna::MODIFIER, api = "ObjectModifiers"),
    empty_display_type => empty_display_type,
    empty_display_size => empty_display_size,
    color => color,
    hide_viewport => hide_viewport (set = hook),
    hide_select => hide_select (set = hook),
    hide_render => hide_render,
    display_type => display_type,
    show_bounds => show_bounds,
    display_bounds_type => display_bounds_type,
    show_name => show_name,
    show_axis => show_axis,
    show_texture_space => show_texture_space,
    show_wire => show_wire,
    show_all_edges => show_all_edges,
    show_in_front => show_in_front,
    show_only_shape_key => show_only_shape_key,
    pass_index => pass_index,
    instance_type => instance_type (set = hook),
    instance_collection => hook,
    is_instancer => readonly(|o| o.instance_type != "NONE"),
    active_shape_key_index => get(|o| o.active_shape_key_index.max(0)) set(|o, v: i32| o.active_shape_key_index = v),
    children => python_only hook_coll,
    children_recursive => python_only hook_coll,
    users_collection => python_only hook_coll,
    users_scene => python_only hook_coll,
}}

impl IdType for Object {
    type Kind = ObjectId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(d) = self.data {
            f(d);
        }
        if let Some(p) = self.parent {
            f(p.any());
        }
        for o in &self.material_slot_overrides {
            if let Some(m) = o.material {
                f(m.any());
            }
        }
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
        if let Some(c) = self.instance_collection {
            f(c.any());
        }
        // Modifier settings: operand objects, node groups, textures (M4 W0); pose: M6.
        for m in &self.modifiers {
            for (_, r) in m.settings.id_refs() {
                if let Some(r) = r {
                    f(r);
                }
            }
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        if self.data == Some(from) {
            self.data = to;
        }
        if self.parent.map(|p| p.any()) == Some(from) {
            self.parent = to.and_then(IdRef::from_any);
        }
        if self.instance_collection.map(|c| c.any()) == Some(from) {
            self.instance_collection = to.and_then(IdRef::from_any);
        }
        for o in &mut self.material_slot_overrides {
            if o.material.map(|m| m.any()) == Some(from) {
                o.material = to.and_then(IdRef::from_any);
            }
        }
        if let Some(ad) = self.animation_data.as_mut()
            && ad.action.map(|a| a.any()) == Some(from)
        {
            ad.action = to.and_then(IdRef::from_any);
        }
        // Modifier settings (M4 W0); pose: M6.
        for m in &mut self.modifiers {
            let hits: Vec<&'static str> = m
                .settings
                .id_refs()
                .into_iter()
                .filter(|(_, r)| *r == Some(from))
                .map(|(k, _)| k)
                .collect();
            for k in hits {
                m.settings.set_id_ref(k, to);
            }
        }
    }
}
