// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Object` (§5.6). The field set is frozen at M1 W0 (verifier fix W0-21); WP 1.10
//! owns the bindings, hooks and behaviour. Fields are named after their RNA identifier, so
//! serde keys in `.frap` equal RNA identifiers. Enum properties are stored as their RNA
//! identifier (`SmolStr`).

use frap_anim::AnimData;
use frap_rna::*;
use frap_spec_rna::{OBJECT, id, object};
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

impl Rna for Object {
    fn srna(&self) -> &'static StructDef {
        &OBJECT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if std::ptr::eq(prop, id::NAME) {
            return Ok(Field::Value(Value::Str(self.header.name.clone())));
        }
        if std::ptr::eq(prop, object::LOCATION) {
            return Ok(Field::Value(Value::FloatArray {
                data: self.location.iter().copied().collect(),
                shape: prop.array,
                tag: MathTag::Vector,
            }));
        }
        Err(super::no_attr(&OBJECT, prop))
    }
}

impl RnaMut for Object {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        if std::ptr::eq(prop, id::NAME) {
            return Ok(SetOutcome::NeedsHook);
        }
        if std::ptr::eq(prop, object::LOCATION) {
            self.location = v.as_vec3().ok_or_else(|| {
                RnaError::new(
                    RnaErrorKind::Type,
                    PyExc::TypeError,
                    "invariant: location expects float[3]",
                )
            })?;
            return Ok(SetOutcome::Done);
        }
        Err(super::no_attr(&OBJECT, prop))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(super::no_attr(&OBJECT, prop))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(super::no_attr(&OBJECT, prop))
    }
}

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
        // WP 1.10 / M4 / M6: modifier settings (objects, node groups, textures), pose.
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        if self.data == Some(from) {
            self.data = to;
        }
        if self.parent.map(|p| p.any()) == Some(from) {
            self.parent = to.and_then(IdRef::from_any);
        }
        for o in &mut self.material_slot_overrides {
            if o.material.map(|m| m.any()) == Some(from) {
                o.material = to.and_then(IdRef::from_any);
            }
        }
        // WP 1.10: animation_data.action, modifier settings.
    }
}
