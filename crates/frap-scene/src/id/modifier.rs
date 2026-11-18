// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Modifier` data (ARCHITECTURE §7.1). Every Blender modifier type exists as data;
//! evaluators arrive with M4 (`frap-modifiers`), whose W0 freezes `ModifierSettings`.
//!
//! The RNA binding is hand-written because the struct is refined per type (`BevelModifier`):
//! the `Modifier` base properties are stored here, every other property is the generated
//! settings struct's (`ModifierSettings::rna_get`/`rna_set`).
//!
//! Hooks (WP 4.1, `hooks/modifier_*.rs`): `name` (unique per object), `is_active` (clears the
//! other modifiers' flag), `use_pin_to_last` (moves the modifier, `rna_Modifier_use_pin_to_last_set`)
//! and `NodesModifier.node_group` (rebuilds `properties.inputs`, `MOD_nodes_update_interface`).
//!
//! `NodesModifier.properties` (Blender 5.2: runtime-defined RNA, verified on 5.2.4):
//! `None` without a node group; otherwise a `GeometryNodesModifierInterface` whose `inputs`
//! (`GeometryNodesInterfaceInputs`) has one struct per group input, named and typed after the
//! input's identifier: `mod.properties.inputs.Input_1.value`. Lookup by display name
//! (`inputs.Angle`) is an `AttributeError`, `inputs` is neither iterable nor sized, and
//! `mod["Input_1"]` raises `TypeError: this type doesn't support IDProperties`. Runtime RNA
//! needs `'static` definitions, so frap defines those of the group inputs it can create: the
//! inputs of the built-in "Smooth by Angle" (`Input_0` Mesh, `Input_1` Angle, `Socket_1`
//! Ignore Sharpness); an input with another identifier is stored and saved, but not reachable
//! through RNA.

use frap_rna::{
    ArrayShape, Defaults, EnumDefault, EnumItem, Field, FloatStorage, PropDef, PropFlags, PropKind,
    Rna, RnaCollMutGuard, RnaError, RnaMut, RnaMutGuard, SetOutcome, StructDef, StructRef, Subtype,
    Unit, Value,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

pub use crate::generated::modifier_settings::ModifierSettings;
use crate::generated::modifier_settings::NodesSettings;

/// `NodesModifier.properties.inputs` (Blender 5.2: runtime-defined RNA, path
/// `mod.properties.inputs.Input_1.value`; `mod["Input_1"]` raises `TypeError: this type doesn't
/// support IDProperties`, ARCHITECTURE §7.1). Stored on the generated `NodesSettings` (field
/// `inputs`, M4 W0), bound to RNA by this file (module docs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GnInputs(pub IndexMap<SmolStr, GnInput>);

/// One group input of a NODES modifier (`NodesModifierProperties.inputs.<identifier>`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnInput {
    /// `name` (the input struct's own name: `''` when created, as in Blender 5.2.4).
    pub name: SmolStr,
    /// `value`
    pub value: GnValue,
    /// `type`: how the input is driven (`'VALUE'`, `'ATTRIBUTE'`; `'FALLBACK'` for geometry).
    pub r#type: SmolStr,
    /// `attribute_name` (with `type == 'ATTRIBUTE'`).
    pub attribute_name: SmolStr,
}

/// The value of a group input, typed by the interface socket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GnValue {
    /// `NodeSocketFloat` (incl. angle/factor subtypes)
    Float(f32),
    /// `NodeSocketInt`
    Int(i32),
    /// `NodeSocketBool`
    Bool(bool),
    /// `NodeSocketVector`
    Vector([f32; 3]),
    /// `NodeSocketColor`
    Color([f32; 4]),
    /// `NodeSocketString`
    String(String),
    /// `NodeSocketMenu` (item index)
    Menu(i32),
    /// A socket without a value (`NodeSocketGeometry`; WP 4.1).
    None,
}

impl GnInput {
    /// A fresh input as `MOD_nodes_update_interface` creates it: the socket's default value,
    /// driven by its value (`'FALLBACK'` for a geometry socket), no attribute.
    pub fn new(value: GnValue) -> GnInput {
        let r#type = if value == GnValue::None {
            "FALLBACK"
        } else {
            "VALUE"
        };
        GnInput {
            name: SmolStr::default(),
            value,
            r#type: SmolStr::new_static(r#type),
            attribute_name: SmolStr::default(),
        }
    }
}

/// One entry of `Object.modifiers`.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    /// `name` (≤63 bytes, unique per object)
    pub name: SmolStr,
    /// `show_viewport`
    pub show_viewport: bool,
    /// `show_render`
    pub show_render: bool,
    /// `show_in_editmode`
    pub show_in_editmode: bool,
    /// `show_on_cage`
    pub show_on_cage: bool,
    /// `show_expanded`
    pub show_expanded: bool,
    /// `is_active`
    pub is_active: bool,
    /// `use_pin_to_last`
    pub use_pin_to_last: bool,
    /// `persistent_uid`
    pub persistent_uid: u32,
    /// Type and settings (GENERATED: `Subsurf(SubsurfSettings) | Bevel(..) | Nodes(..) | …`).
    pub settings: ModifierSettings,
}

impl Rna for Modifier {
    /// The refined struct (`BevelModifier`; `Modifier` for untyped settings).
    fn srna(&self) -> &'static StructDef {
        self.settings.srna()
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let b = |v: bool| Ok(Field::Value(Value::Bool(v)));
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(self.name.clone()))),
            "type" => Ok(Field::Value(Value::Enum(
                self.settings.type_identifier().into(),
            ))),
            "show_viewport" => b(self.show_viewport),
            "show_render" => b(self.show_render),
            "show_in_editmode" => b(self.show_in_editmode),
            "show_on_cage" => b(self.show_on_cage),
            "show_expanded" => b(self.show_expanded),
            "is_active" => b(self.is_active),
            "use_pin_to_last" => b(self.use_pin_to_last),
            "is_override_data" => b(false),
            "persistent_uid" => Ok(Field::Value(Value::Int(i64::from(
                self.persistent_uid as i32,
            )))),
            // Blender reports the last evaluation time; frap does not measure it.
            "execution_time" => Ok(Field::Value(Value::Float(0.0))),
            "properties" => match &self.settings {
                ModifierSettings::Nodes(s) if s.node_group.is_some() => {
                    Ok(Field::Struct(StructRef::Owned(Box::new(GnInterface { s }))))
                }
                ModifierSettings::Nodes(_) => Ok(Field::Value(Value::None)),
                _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
            },
            _ => self
                .settings
                .rna_get(prop)
                .ok_or_else(|| frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

impl RnaMut for Modifier {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let flag = |f: &mut bool, v: &Value| {
            if let Some(x) = v.as_bool() {
                *f = x;
            }
            Ok(SetOutcome::Done)
        };
        match prop.identifier {
            // Unique per object (sub-data name), the active flag is exclusive and pinning moves
            // the modifier: hooks (`hooks/modifier_{name,is_active,use_pin_to_last}.rs`).
            "name" | "is_active" | "use_pin_to_last" => Ok(SetOutcome::NeedsHook),
            // Rebuilds `properties.inputs` from the group (`hooks/modifier_node_group.rs`).
            "node_group" if matches!(self.settings, ModifierSettings::Nodes(_)) => {
                Ok(SetOutcome::NeedsHook)
            }
            "show_viewport" => flag(&mut self.show_viewport, &v),
            "show_render" => flag(&mut self.show_render, &v),
            "show_in_editmode" => flag(&mut self.show_in_editmode, &v),
            "show_on_cage" => flag(&mut self.show_on_cage, &v),
            "show_expanded" => flag(&mut self.show_expanded, &v),
            "type" | "is_override_data" | "persistent_uid" | "execution_time" | "properties" => {
                Err(frap_rna::traits::read_only(self.srna(), prop.identifier))
            }
            _ => {
                let srna = self.srna();
                self.settings
                    .rna_set(prop, v)
                    .unwrap_or_else(|| Err(frap_rna::traits::no_attribute(srna, prop.identifier)))
            }
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        match (prop.identifier, &mut self.settings) {
            ("properties", ModifierSettings::Nodes(s)) if s.node_group.is_some() => {
                Ok(Box::new(GnInterfaceMut { s }))
            }
            _ => Err(frap_rna::traits::wrong_access(
                srna,
                prop.identifier,
                "a struct",
            )),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

// ------------------------------------------------------- NodesModifier.properties (runtime)

const fn ptr(identifier: &'static str, fixed_type: &'static str) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Pointer {
            fixed_type,
            is_id: false,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::READONLY.union(PropFlags::NEVER_NULL),
        deprecated: None,
    }
}

const fn string(identifier: &'static str, name: &'static str) -> PropDef {
    PropDef {
        identifier,
        name,
        description: "",
        kind: PropKind::String {
            default: "",
            max_bytes: 0,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

const fn input_type(items: &'static [EnumItem], default: &'static str) -> PropDef {
    PropDef {
        identifier: "type",
        name: "Type",
        description: "",
        kind: PropKind::Enum {
            items,
            default: EnumDefault::Item(default),
            dynamic: None,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

const fn item(identifier: &'static str, name: &'static str, value: i32) -> EnumItem {
    EnumItem {
        identifier,
        name,
        description: "",
        value,
    }
}

static TYPE_FALLBACK: [EnumItem; 1] = [item("FALLBACK", "Fallback", 0)];
static TYPE_FIELD: [EnumItem; 2] = [item("VALUE", "Value", 0), item("ATTRIBUTE", "Attribute", 1)];
static TYPE_SINGLE: [EnumItem; 3] = [
    item("VALUE", "Value", 0),
    item("ATTRIBUTE", "Attribute", 1),
    item("LAYER", "Layer", 2),
];

static INTERFACE_PROPS: [PropDef; 2] = [
    ptr("inputs", "GeometryNodesInterfaceInputs"),
    ptr("outputs", "GeometryNodesInterfaceOutputs"),
];
/// `NodesModifier.properties` (`GeometryNodesModifierInterface`, runtime-defined in 5.2).
pub static GEOMETRY_NODES_MODIFIER_INTERFACE: StructDef = StructDef {
    identifier: "GeometryNodesModifierInterface",
    base: None,
    name: "GeometryNodesModifierInterface",
    description: "",
    props: &INTERFACE_PROPS,
    funcs: &[],
    name_property: None,
    is_id: false,
};

static INPUTS_PROPS: [PropDef; 4] = [
    string("name", "Name"),
    ptr("Input_0", "Input_0"),
    ptr("Input_1", "Input_1"),
    ptr("Socket_1", "Socket_1"),
];
/// `properties.inputs` (`GeometryNodesInterfaceInputs`).
pub static GEOMETRY_NODES_INTERFACE_INPUTS: StructDef = StructDef {
    identifier: "GeometryNodesInterfaceInputs",
    base: None,
    name: "GeometryNodesInterfaceInputs",
    description: "",
    props: &INPUTS_PROPS,
    funcs: &[],
    name_property: None,
    is_id: false,
};

static OUTPUTS_PROPS: [PropDef; 1] = [string("name", "Name")];
/// `properties.outputs` (`GeometryNodesInterfaceOutputs`).
pub static GEOMETRY_NODES_INTERFACE_OUTPUTS: StructDef = StructDef {
    identifier: "GeometryNodesInterfaceOutputs",
    base: None,
    name: "GeometryNodesInterfaceOutputs",
    description: "",
    props: &OUTPUTS_PROPS,
    funcs: &[],
    name_property: None,
    is_id: false,
};

static INPUT_0_PROPS: [PropDef; 2] = [
    string("name", "Name"),
    input_type(&TYPE_FALLBACK, "FALLBACK"),
];
static INPUT_1_PROPS: [PropDef; 4] = [
    string("name", "Name"),
    PropDef {
        identifier: "value",
        name: "Angle",
        description: "Maximum face angle for smooth edges",
        kind: PropKind::Float {
            default: Defaults::Scalar(0.523_598_790_168_762_2),
            hard: (-3.402_823_466_385_288_6e38, 3.402_823_466_385_288_6e38),
            soft: (0.0, std::f64::consts::PI),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::Angle,
        unit: Unit::Rotation,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
        deprecated: None,
    },
    input_type(&TYPE_FIELD, "VALUE"),
    string("attribute_name", "Attribute Name"),
];
static SOCKET_1_PROPS: [PropDef; 5] = [
    string("name", "Name"),
    PropDef {
        identifier: "value",
        name: "Ignore Sharpness",
        description: "",
        kind: PropKind::Bool {
            default: Defaults::Scalar(false),
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
        deprecated: None,
    },
    input_type(&TYPE_SINGLE, "VALUE"),
    string("attribute_name", "Attribute Name"),
    string("layer_name", "Layer Name"),
];

const fn input_struct(identifier: &'static str, props: &'static [PropDef]) -> StructDef {
    StructDef {
        identifier,
        base: None,
        name: identifier,
        description: "",
        props,
        funcs: &[],
        name_property: None,
        is_id: false,
    }
}

/// `properties.inputs.Input_0` (the "Smooth by Angle" geometry input).
pub static GN_INPUT_0: StructDef = input_struct("Input_0", &INPUT_0_PROPS);
/// `properties.inputs.Input_1` ("Smooth by Angle": Angle).
pub static GN_INPUT_1: StructDef = input_struct("Input_1", &INPUT_1_PROPS);
/// `properties.inputs.Socket_1` ("Smooth by Angle": Ignore Sharpness).
pub static GN_SOCKET_1: StructDef = input_struct("Socket_1", &SOCKET_1_PROPS);

/// The RNA struct of the group input `identifier` (module docs).
pub fn gn_input_srna(identifier: &str) -> Option<&'static StructDef> {
    match identifier {
        "Input_0" => Some(&GN_INPUT_0),
        "Input_1" => Some(&GN_INPUT_1),
        "Socket_1" => Some(&GN_SOCKET_1),
        _ => None,
    }
}

fn no_attr(srna: &'static StructDef, prop: &str) -> RnaError {
    frap_rna::traits::no_attribute(srna, prop)
}

struct GnInterface<'a> {
    s: &'a NodesSettings,
}

fn interface_get<'a>(s: &'a NodesSettings, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    match prop.identifier {
        "inputs" => Ok(Field::Struct(StructRef::Owned(Box::new(Inputs {
            inputs: &s.inputs,
        })))),
        "outputs" => Ok(Field::Struct(StructRef::Owned(Box::new(Outputs)))),
        id => Err(no_attr(&GEOMETRY_NODES_MODIFIER_INTERFACE, id)),
    }
}

impl Rna for GnInterface<'_> {
    fn srna(&self) -> &'static StructDef {
        &GEOMETRY_NODES_MODIFIER_INTERFACE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        interface_get(self.s, prop)
    }
}

struct GnInterfaceMut<'a> {
    s: &'a mut NodesSettings,
}

impl Rna for GnInterfaceMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &GEOMETRY_NODES_MODIFIER_INTERFACE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        interface_get(self.s, prop)
    }
}

impl RnaMut for GnInterfaceMut<'_> {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        Err(frap_rna::traits::read_only(self.srna(), prop.identifier))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        match prop.identifier {
            "inputs" => Ok(Box::new(InputsMut {
                inputs: &mut self.s.inputs,
            })),
            "outputs" => Ok(Box::new(Outputs)),
            id => Err(no_attr(self.srna(), id)),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

struct Outputs;

impl Rna for Outputs {
    fn srna(&self) -> &'static StructDef {
        &GEOMETRY_NODES_INTERFACE_OUTPUTS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(SmolStr::default()))),
            id => Err(no_attr(self.srna(), id)),
        }
    }
}

impl RnaMut for Outputs {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        Err(frap_rna::traits::read_only(self.srna(), prop.identifier))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(no_attr(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(no_attr(self.srna(), prop.identifier))
    }
}

struct Inputs<'a> {
    inputs: &'a GnInputs,
}

fn inputs_get<'a>(inputs: &'a GnInputs, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    if prop.identifier == "name" {
        return Ok(Field::Value(Value::Str(SmolStr::default())));
    }
    match (
        inputs.0.get(prop.identifier),
        gn_input_srna(prop.identifier),
    ) {
        (Some(input), Some(srna)) => Ok(Field::Struct(StructRef::Owned(Box::new(Input {
            input,
            srna,
        })))),
        _ => Err(no_attr(&GEOMETRY_NODES_INTERFACE_INPUTS, prop.identifier)),
    }
}

impl Rna for Inputs<'_> {
    fn srna(&self) -> &'static StructDef {
        &GEOMETRY_NODES_INTERFACE_INPUTS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        inputs_get(self.inputs, prop)
    }
}

struct InputsMut<'a> {
    inputs: &'a mut GnInputs,
}

impl Rna for InputsMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &GEOMETRY_NODES_INTERFACE_INPUTS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        inputs_get(self.inputs, prop)
    }
}

impl RnaMut for InputsMut<'_> {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            // The group's own name is not stored (`''`, as created).
            "name" => Ok(SetOutcome::Done),
            id => Err(frap_rna::traits::read_only(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        match (
            self.inputs.0.get_mut(prop.identifier),
            gn_input_srna(prop.identifier),
        ) {
            (Some(input), Some(srna)) => Ok(Box::new(InputMut { input, srna })),
            _ => Err(no_attr(srna, prop.identifier)),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

fn input_get<'a>(
    input: &'a GnInput,
    srna: &'static StructDef,
    prop: &'static PropDef,
) -> Result<Field<'a>, RnaError> {
    let s = |v: &SmolStr| Ok(Field::Value(Value::Str(v.clone())));
    match prop.identifier {
        "name" => s(&input.name),
        "type" => Ok(Field::Value(Value::Enum(input.r#type.clone()))),
        "attribute_name" if srna.find_prop("attribute_name").is_some() => s(&input.attribute_name),
        "layer_name" if srna.find_prop("layer_name").is_some() => s(&SmolStr::default()),
        "value" => match &input.value {
            GnValue::Float(f) => Ok(Field::Value(Value::Float(*f))),
            GnValue::Int(i) => Ok(Field::Value(Value::Int(i64::from(*i)))),
            GnValue::Bool(b) => Ok(Field::Value(Value::Bool(*b))),
            _ => Err(no_attr(srna, prop.identifier)),
        },
        id => Err(no_attr(srna, id)),
    }
}

struct Input<'a> {
    input: &'a GnInput,
    srna: &'static StructDef,
}

impl Rna for Input<'_> {
    fn srna(&self) -> &'static StructDef {
        self.srna
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        input_get(self.input, self.srna, prop)
    }
}

struct InputMut<'a> {
    input: &'a mut GnInput,
    srna: &'static StructDef,
}

impl Rna for InputMut<'_> {
    fn srna(&self) -> &'static StructDef {
        self.srna
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        input_get(self.input, self.srna, prop)
    }
}

impl RnaMut for InputMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let text = |v: &Value| SmolStr::from(v.as_str().unwrap_or_default());
        match prop.identifier {
            "name" => self.input.name = text(&v),
            "type" => self.input.r#type = text(&v),
            "attribute_name" => self.input.attribute_name = text(&v),
            // Not stored: `''` (layer inputs are a grease-pencil feature).
            "layer_name" => {}
            "value" => {
                self.input.value = match (&self.input.value, &v) {
                    (GnValue::Float(_), Value::Float(f)) => GnValue::Float(*f),
                    (GnValue::Float(_), Value::Double(f)) => GnValue::Float(*f as f32),
                    (GnValue::Bool(_), Value::Bool(b)) => GnValue::Bool(*b),
                    (GnValue::Int(_), Value::Int(i)) => GnValue::Int(*i as i32),
                    _ => return Err(no_attr(self.srna, prop.identifier)),
                }
            }
            id => return Err(no_attr(self.srna, id)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna,
            prop.identifier,
            "a struct",
        ))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna,
            prop.identifier,
            "a collection",
        ))
    }
}

// ------------------------------------------------------------------------- hook bindings

macro_rules! hook_decl {
    ($name:ident, $srna:literal, $prop:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
            srna: $srna,
            prop: $prop,
            coll: false,
        };
    };
}

// `Modifier.name` (`hooks/modifier_name.rs`): unique per object, 63 bytes.
hook_decl!(MODIFIER_NAME_DECL, "Modifier", "name");
// `Modifier.is_active` (`hooks/modifier_is_active.rs`).
hook_decl!(MODIFIER_IS_ACTIVE_DECL, "Modifier", "is_active");
// `Modifier.use_pin_to_last` (`hooks/modifier_use_pin_to_last.rs`).
hook_decl!(MODIFIER_PIN_DECL, "Modifier", "use_pin_to_last");
// `NodesModifier.node_group` (`hooks/modifier_node_group.rs`).
hook_decl!(
    NODES_MODIFIER_NODE_GROUP_DECL,
    "NodesModifier",
    "node_group"
);

// ------------------------------------------------------------------- stack helpers (hooks)

impl Modifier {
    /// Types whose `ModifierTypeInfo::type` is `OnlyDeform` (Blender 5.2.4 `MOD_*.cc`;
    /// checked against `frap_modifiers::registry::meta` by its tests). The pin setter needs
    /// them here, below `frap-modifiers`.
    pub const DEFORM_ONLY_TYPES: &'static [&'static str] = DEFORM_ONLY_TYPES;
    /// Types flagged `eModifierTypeFlag_RequiresOriginalData` (checked like
    /// [`Modifier::DEFORM_ONLY_TYPES`]).
    pub const REQUIRES_ORIGINAL_DATA_TYPES: &'static [&'static str] = REQUIRES_ORIGINAL_DATA_TYPES;

    /// The index of the modifier a pointer path `modifiers[<key or index>]…` names.
    pub fn index_in(mods: &[Modifier], path: &[frap_rna::Seg]) -> Option<usize> {
        modifier_index(mods, path)
    }

    /// The canonical pointer `bpy.data.objects['X'].modifiers["Name"]` of this modifier of
    /// object `ob`.
    pub fn ptr(&self, ob: frap_rna::IdAny) -> frap_rna::RnaPtr {
        modifier_ptr(ob, self)
    }

    /// `rna_Modifier_use_pin_to_last_set` on modifier `i` of `mods` (see
    /// [`set_pin_to_last`]); returns its new index.
    pub fn set_pin_to_last(mods: &mut Vec<Modifier>, i: usize, value: bool) -> usize {
        set_pin_to_last(mods, i, value)
    }
}

/// The index of the modifier a pointer path `modifiers[<key or index>]…` names.
pub(crate) fn modifier_index(mods: &[Modifier], path: &[frap_rna::Seg]) -> Option<usize> {
    use frap_rna::Seg;
    match path {
        [Seg::Prop("modifiers"), Seg::Key(k), ..] => mods.iter().position(|m| m.name == *k),
        [Seg::Prop("modifiers"), Seg::Index(i), ..] => {
            usize::try_from(*i).ok().filter(|i| *i < mods.len())
        }
        _ => None,
    }
}

/// The canonical pointer `bpy.data.objects['X'].modifiers["Name"]`.
pub(crate) fn modifier_ptr(ob: frap_rna::IdAny, m: &Modifier) -> frap_rna::RnaPtr {
    frap_rna::RnaPtr::id(ob, &frap_spec_rna::OBJECT)
        .child(
            frap_rna::Seg::Prop("modifiers"),
            &frap_spec_rna::OBJECT_MODIFIERS,
        )
        .child(frap_rna::Seg::Key(m.name.clone()), m.srna())
}

/// Types whose `ModifierTypeInfo::type` is `OnlyDeform` (Blender 5.2.4 `MOD_*.cc`; checked
/// against `frap_modifiers::registry::meta` by its tests). The pin setter needs them here,
/// below `frap-modifiers`.
const DEFORM_ONLY_TYPES: &[&str] = &[
    "ARMATURE",
    "CAST",
    "CLOTH",
    "COLLISION",
    "CORRECTIVE_SMOOTH",
    "CURVE",
    "DISPLACE",
    "GREASE_PENCIL_ARMATURE",
    "GREASE_PENCIL_HOOK",
    "GREASE_PENCIL_LATTICE",
    "GREASE_PENCIL_NOISE",
    "GREASE_PENCIL_OFFSET",
    "GREASE_PENCIL_SHRINKWRAP",
    "GREASE_PENCIL_SMOOTH",
    "GREASE_PENCIL_THICKNESS",
    "HOOK",
    "LAPLACIANDEFORM",
    "LAPLACIANSMOOTH",
    "LATTICE",
    "MESH_CACHE",
    "MESH_DEFORM",
    "PARTICLE_SYSTEM",
    "SHRINKWRAP",
    "SIMPLE_DEFORM",
    "SMOOTH",
    "SOFT_BODY",
    "SURFACE",
    "SURFACE_DEFORM",
    "WARP",
    "WAVE",
];

/// Types flagged `eModifierTypeFlag_RequiresOriginalData` (checked like
/// [`DEFORM_ONLY_TYPES`]).
const REQUIRES_ORIGINAL_DATA_TYPES: &[&str] = &["MULTIRES", "SOFT_BODY"];

/// `rna_Modifier_use_pin_to_last_set`: sets the flag on modifier `i`, then moves it to the end
/// of the unpinned run (pinning) or to the start of the pinned run (unpinning), one step at a
/// time with the checks of `object_modifier_check_move_after/before`, stopping before a refused
/// step (`modifier_move_to_index(…, allow_partial = true)`). Returns its new index.
pub(crate) fn set_pin_to_last(mods: &mut Vec<Modifier>, i: usize, value: bool) -> usize {
    let deform_only = |m: &Modifier| DEFORM_ONLY_TYPES.contains(&m.settings.type_identifier());
    let requires_orig =
        |m: &Modifier| REQUIRES_ORIGINAL_DATA_TYPES.contains(&m.settings.type_identifier());
    mods[i].use_pin_to_last = value;
    let mut to = i;
    if value {
        while to + 1 < mods.len() && !mods[to + 1].use_pin_to_last {
            if requires_orig(&mods[i]) && !deform_only(&mods[to + 1]) {
                break;
            }
            to += 1;
        }
    } else {
        while to > 0 && mods[to - 1].use_pin_to_last {
            if !deform_only(&mods[i]) && requires_orig(&mods[to - 1]) {
                break;
            }
            to -= 1;
        }
    }
    let md = mods.remove(i);
    mods.insert(to, md);
    to
}

#[cfg(test)]
mod pin_tests {
    use super::*;
    use crate::generated::modifier_settings::ModifierType;

    fn md(name: &str, ty: &str) -> Modifier {
        Modifier {
            name: name.into(),
            show_viewport: true,
            show_render: true,
            show_in_editmode: false,
            show_on_cage: false,
            show_expanded: true,
            is_active: false,
            use_pin_to_last: false,
            persistent_uid: 1,
            settings: ModifierSettings::new(ModifierType::from_identifier(ty).unwrap()),
        }
    }

    #[test]
    fn pinning_moves_like_blender() {
        // 5.2.4: pin Cloth (first) → last; pin Subdivision (now first) → before Cloth.
        let mut mods = vec![
            md("Cloth", "CLOTH"),
            md("Subdivision", "SUBSURF"),
            md("ParticleSystem", "PARTICLE_SYSTEM"),
            md("Bevel", "BEVEL"),
            md("Subdivision.001", "BEVEL"),
        ];
        assert_eq!(set_pin_to_last(&mut mods, 0, true), 4);
        assert_eq!(set_pin_to_last(&mut mods, 0, true), 3);
        let names: Vec<_> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "ParticleSystem",
                "Bevel",
                "Subdivision.001",
                "Subdivision",
                "Cloth"
            ]
        );
        // Unpinning an unpinned modifier changes nothing; unpinning Cloth moves it to the
        // start of the pinned run.
        assert_eq!(set_pin_to_last(&mut mods, 1, false), 1);
        assert_eq!(set_pin_to_last(&mut mods, 4, false), 3);
        assert_eq!(mods[3].name, "Cloth");
        let path = [frap_rna::Seg::Prop("modifiers"), frap_rna::Seg::Index(3)];
        assert_eq!(modifier_index(&mods, &path), Some(3));
    }
}
