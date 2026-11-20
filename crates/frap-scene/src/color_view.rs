// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.ColorManagedViewSettings` (`scene.view_settings`,
//! `render.image_settings.view_settings`, …): the `Rna`/`RnaMut` impls of the generated plain
//! struct, hand-written for the two properties whose behaviour depends on each other
//! (M5+M6 stabilization; `rna_impl: false` in `xtask/src/codegen/gen_plain.rs`).
//!
//! Ported from blender/source/blender/makesrna/intern/rna_color.cc
//! (`rna_ColorManagedViewSettings_look_itemf`, `_look_set`, `_view_transform_set`) and
//! blender/source/blender/imbuf/intern/colormanagement.cc
//! (`IMB_colormanagement_look_items_add`, `IMB_colormanagement_look_validate_for_view`):
//!
//! - `look` offers the looks of the current `view_transform` only
//!   (`frap_color::looks::looks_for`); the RNA enum is their union
//!   (`spec/overlay/dynamic_enums.toml`), so any other look of the config is Blender's
//!   `TypeError: bpy_struct: item.attr = val: enum "High Contrast" not found in ('None', …)`
//!   listing the current items (verified on 5.2.4 for every view transform);
//! - assigning `view_transform` keeps a compatible look, else switches to the look of the same
//!   UI name the new view offers, else `None` (`AgX - Low Contrast` → `Standard` reads
//!   `Low Contrast`; `AgX - Greyscale` → `Standard` reads `None`).

use frap_color::ViewTransform;
use frap_color::looks::{looks_for, validate_for_view};
use frap_rna::{
    Field, PropDef, PyExc, Rna, RnaCollMutGuard, RnaError, RnaErrorKind, RnaMut, RnaMutGuard,
    SetOutcome, StructDef, Value,
};
use smol_str::SmolStr;

use crate::generated::plain::ColorManagedViewSettings;
use crate::generated::{not_a_struct, unstored_get, unstored_set};

/// The looks `view_settings.look` accepts for `view_transform` (Blender's enum items; only
/// `None` for a view frap does not know).
pub fn look_items(view_transform: &str) -> Vec<&'static str> {
    match ViewTransform::from_name(view_transform) {
        Some(v) => looks_for(v),
        None => vec!["None"],
    }
}

/// `view_settings.look = name`: Blender's enum `TypeError` when the current view transform
/// does not offer `name`.
pub fn set_look(s: &mut ColorManagedViewSettings, name: &str) -> Result<(), RnaError> {
    let items = look_items(&s.view_transform);
    if !items.contains(&name) {
        let listed = items
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(RnaError::new(
            RnaErrorKind::Enum,
            PyExc::TypeError,
            format!("bpy_struct: item.attr = val: enum \"{name}\" not found in ({listed})"),
        )
        .with_detail(
            "did_you_mean",
            frap_rna::error::suggest(name, items.iter().copied()),
        )
        .with_detail("valid", items));
    }
    s.look = SmolStr::new(name);
    Ok(())
}

/// `view_settings.view_transform = name` with the look remap of
/// `rna_ColorManagedViewSettings_view_transform_set`.
pub fn set_view_transform(s: &mut ColorManagedViewSettings, name: &str) {
    s.view_transform = SmolStr::new(name);
    if let Some(v) = ViewTransform::from_name(name) {
        let look = validate_for_view(v, &s.look);
        if look != s.look {
            s.look = SmolStr::new(look);
        }
    }
}

impl Rna for ColorManagedViewSettings {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::COLOR_MANAGED_VIEW_SETTINGS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match self.rna_get(prop) {
            Some(f) => Ok(f),
            None => unstored_get(self.srna(), prop),
        }
    }
}

impl RnaMut for ColorManagedViewSettings {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match (prop.identifier, &v) {
            ("look", Value::Enum(name)) => {
                set_look(self, name)?;
                return Ok(SetOutcome::Done);
            }
            ("view_transform", Value::Enum(name)) => {
                set_view_transform(self, name);
                return Ok(SetOutcome::Done);
            }
            _ => {}
        }
        let srna = self.srna();
        match self.rna_set(prop, v) {
            Some(r) => r,
            None => unstored_set(srna, prop),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        self.rna_struct_mut(prop)
            .ok_or_else(|| not_a_struct(srna, prop))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(not_a_struct(self.srna(), prop))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_follows_the_view_transform() {
        let mut s = ColorManagedViewSettings::default();
        assert!(set_look(&mut s, "High Contrast").is_err());
        set_look(&mut s, "AgX - Low Contrast").unwrap();
        set_view_transform(&mut s, "Standard");
        assert_eq!(s.look, "Low Contrast");
        set_view_transform(&mut s, "False Color");
        assert_eq!(s.look, "False Color - Low Contrast");
        set_look(&mut s, "False Color - Greyscale").unwrap();
        set_view_transform(&mut s, "Filmic");
        assert_eq!(s.look, "None");
        let e = set_look(&mut s, "AgX - Punchy").unwrap_err();
        assert!(
            e.message
                .contains("not found in ('None', 'Very High Contrast'"),
            "{}",
            e.message
        );
        set_look(&mut s, "High Contrast").unwrap();
    }
}
