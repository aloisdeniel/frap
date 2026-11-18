// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Modifier` data (ARCHITECTURE §7.1). Every Blender modifier type exists as data;
//! evaluators arrive with M4 (`frap-modifiers`), whose W0 freezes `ModifierSettings`.
//!
//! The RNA binding is hand-written because the struct is refined per type (`BevelModifier`):
//! the `Modifier` base properties are stored here, every other property is the generated
//! settings struct's (`ModifierSettings::rna_get`/`rna_set`).

use frap_rna::{
    Field, PropDef, Rna, RnaCollMutGuard, RnaError, RnaMut, RnaMutGuard, SetOutcome, StructDef,
    Value,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

pub use crate::generated::modifier_settings::ModifierSettings;

/// `NodesModifier.properties.inputs` (Blender 5.2: runtime-defined RNA, path
/// `mod.properties.inputs.Input_1.value`; `mod["Input_1"]` raises `TypeError: this type doesn't
/// support IDProperties`, ARCHITECTURE §7.1). Stored on the generated `NodesSettings` (field
/// `inputs`, M4 W0); the RNA binding is WP 4.1's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GnInputs(pub IndexMap<SmolStr, GnInput>);

/// One group input of a NODES modifier (`NodesModifierProperties.inputs.<identifier>`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnInput {
    /// `name`: the interface socket's display name ("Angle").
    pub name: SmolStr,
    /// `value`
    pub value: GnValue,
    /// `type`: how the input is driven (`'VALUE'`, `'ATTRIBUTE'`).
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
            // Unique per object (sub-data name): the `Modifier.name` hook.
            "name" => Ok(SetOutcome::NeedsHook),
            "show_viewport" => flag(&mut self.show_viewport, &v),
            "show_render" => flag(&mut self.show_render, &v),
            "show_in_editmode" => flag(&mut self.show_in_editmode, &v),
            "show_on_cage" => flag(&mut self.show_on_cage, &v),
            "show_expanded" => flag(&mut self.show_expanded, &v),
            "is_active" => flag(&mut self.is_active, &v),
            "use_pin_to_last" => flag(&mut self.use_pin_to_last, &v),
            "type" | "is_override_data" | "persistent_uid" | "execution_time" => {
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
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a struct",
        ))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

/// `Modifier.name` is set through a hook (unique per object, 63 bytes; `hooks/modifier_name.rs`).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static MODIFIER_NAME_DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "Modifier",
    prop: "name",
    coll: false,
};
