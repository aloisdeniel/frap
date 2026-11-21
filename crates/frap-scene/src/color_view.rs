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
//!   `Low Contrast`; `AgX - Greyscale` → `Standard` reads `None`);
//! - `view_transform` offers the views of the sibling `display_settings.display_device` only
//!   (`frap_color::views::display_views`; `rna_ColorManagedViewSettings_view_transform_itemf`):
//!   `Filmic` on `Display P3` is Blender's enum `TypeError` listing that display's views
//!   (hook `hooks/color_managed_view_settings_view_transform.rs`);
//! - changing a scene's own `display_settings.display_device` replaces a view the new display
//!   does not offer (`IMB_colormanagement_validate_settings`, hook
//!   `hooks/color_managed_display_settings_display_device.rs`). Other owners
//!   (`render.image_settings`) keep their view, as Blender does (it then reads `''`, frap the
//!   stored name: deviations §2).

use frap_color::ViewTransform;
use frap_color::looks::{looks_for, validate_for_view};
use frap_color::views::{display_views, view_base};
use frap_rna::{
    Field, PropDef, PyExc, Rna, RnaCollMutGuard, RnaError, RnaErrorKind, RnaMut, RnaMutGuard,
    RnaPtr, Root, Seg, SetOutcome, StructDef, StructRef, Value,
};
use smol_str::SmolStr;

use crate::data::{ChangeLog, Main};
use crate::generated::plain::ColorManagedViewSettings;
use crate::generated::{not_a_struct, unstored_get, unstored_set};
use crate::id::paths::internal;

/// The view family whose looks `view_transform` offers (`AgX - HDR 1000 nits` offers AgX's).
fn look_family(view_transform: &str) -> Option<ViewTransform> {
    ViewTransform::from_name(view_transform)
        .or_else(|| ViewTransform::from_name(view_base(view_transform)))
}

/// The looks `view_settings.look` accepts for `view_transform` (Blender's enum items; only
/// `None` for a view frap does not know).
pub fn look_items(view_transform: &str) -> Vec<&'static str> {
    match look_family(view_transform) {
        Some(v) => looks_for(v),
        None => vec!["None"],
    }
}

/// Blender's enum `TypeError` for `name` outside `items`.
fn enum_error(name: &str, items: &[&'static str]) -> RnaError {
    let listed = items
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    RnaError::new(
        RnaErrorKind::Enum,
        PyExc::TypeError,
        format!("bpy_struct: item.attr = val: enum \"{name}\" not found in ({listed})"),
    )
    .with_detail(
        "did_you_mean",
        frap_rna::error::suggest(name, items.iter().copied()),
    )
    .with_detail("valid", items.to_vec())
}

/// `view_settings.look = name`: Blender's enum `TypeError` when the current view transform
/// does not offer `name`.
pub fn set_look(s: &mut ColorManagedViewSettings, name: &str) -> Result<(), RnaError> {
    let items = look_items(&s.view_transform);
    if !items.contains(&name) {
        return Err(enum_error(name, &items));
    }
    s.look = SmolStr::new(name);
    Ok(())
}

/// The views `view_settings.view_transform` accepts on `display` (sRGB's for a display frap
/// cannot find).
pub fn view_items(display: Option<&str>) -> &'static [&'static str] {
    display
        .and_then(display_views)
        .or_else(|| display_views("sRGB"))
        .unwrap_or_default()
}

/// Blender's enum `TypeError` when `display` does not offer the view `name`.
pub fn check_view(display: Option<&str>, name: &str) -> Result<(), RnaError> {
    let items = view_items(display);
    if items.contains(&name) {
        Ok(())
    } else {
        Err(enum_error(name, items))
    }
}

/// `view_settings.view_transform = name` with the look remap of
/// `rna_ColorManagedViewSettings_view_transform_set`.
pub fn set_view_transform(s: &mut ColorManagedViewSettings, name: &str) {
    s.view_transform = SmolStr::new(name);
    if let Some(v) = look_family(name) {
        let look = validate_for_view(v, &s.look);
        if look != s.look {
            s.look = SmolStr::new(look);
        }
    }
}

/// `IMB_colormanagement_validate_settings` after a display change: a view `display` does not
/// offer becomes the display's view of the same family, else `Standard` (with the look remap
/// of the view setter). Returns whether the view changed.
pub fn validate_for_display(s: &mut ColorManagedViewSettings, display: &str) -> bool {
    match frap_color::views::validate_for_display(display, &s.view_transform) {
        Some(v) if v != s.view_transform => {
            set_view_transform(s, v);
            true
        }
        _ => false,
    }
}

/// Reads `f` on the struct at `path` below `cur` (`Prop` segments only).
fn read_at<R>(cur: &dyn Rna, path: &[Seg], f: &mut dyn FnMut(&dyn Rna) -> Option<R>) -> Option<R> {
    let Some((seg, rest)) = path.split_first() else {
        return f(cur);
    };
    let Seg::Prop(name) = seg else {
        return None;
    };
    let p = cur.srna().find_prop(name)?;
    match cur.get(p).ok()? {
        Field::Struct(StructRef::Borrowed(r)) | Field::Embedded(_, StructRef::Borrowed(r)) => {
            read_at(r, rest, f)
        }
        Field::Struct(StructRef::Owned(b)) | Field::Embedded(_, StructRef::Owned(b)) => {
            read_at(&*b, rest, f)
        }
        _ => None,
    }
}

/// The `display_settings.display_device` next to the `ColorManagedViewSettings` at `this`
/// (`scene.view_settings` → `scene.display_settings`, `image_settings.view_settings` →
/// `image_settings.display_settings`); `None` when the owner has no display settings.
pub fn sibling_display(main: &Main, this: &RnaPtr) -> Option<SmolStr> {
    let (_, parent) = this.path.split_last()?;
    let mut read = |parent_struct: &dyn Rna| {
        let ds = parent_struct.srna().find_prop("display_settings")?;
        let Field::Struct(sr) = parent_struct.get(ds).ok()? else {
            return None;
        };
        let r: &dyn Rna = match &sr {
            StructRef::Borrowed(r) => *r,
            StructRef::Owned(b) => &**b,
        };
        match r.get(r.srna().find_prop("display_device")?).ok()? {
            Field::Value(Value::Enum(s) | Value::Str(s)) => Some(s),
            _ => None,
        }
    };
    match this.root {
        Root::Id(a) => read_at(main.id(a)?, parent, &mut read),
        Root::Embedded { owner, slot } => {
            let mut embedded = Vec::with_capacity(parent.len() + 1);
            embedded.push(Seg::Prop(slot.prop_identifier()));
            embedded.extend(parent.iter().cloned());
            read_at(main.id(owner)?, &embedded, &mut read)
        }
        _ => None,
    }
}

/// Sets `prop` on the struct at `path` below `cur`.
fn set_at(
    cur: &mut dyn RnaMut,
    path: &[Seg],
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let Some((seg, rest)) = path.split_first() else {
        return cur.set(prop, v).map(|_| ());
    };
    let Seg::Prop(name) = seg else {
        return Err(internal("colour-management settings below a collection"));
    };
    let p = cur
        .srna()
        .find_prop(name)
        .ok_or_else(|| internal("the colour-management settings path vanished"))?;
    let mut g = cur.struct_mut(p)?;
    set_at(&mut *g, rest, prop, v)
}

/// Sets `prop` through the struct binding at `this` (an ID or an embedded ID root).
pub(crate) fn set_through(
    main: &mut Main,
    log: &mut ChangeLog,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let gone = || internal("the owner of the colour-management settings no longer exists");
    match this.root {
        Root::Id(a) => {
            let r = main.id_mut(a, log).ok_or_else(gone)?;
            set_at(r, &this.path, prop, v)
        }
        Root::Embedded { owner, slot } => {
            let o = main.id_mut(owner, log).ok_or_else(gone)?;
            let p = o
                .srna()
                .find_prop(slot.prop_identifier())
                .ok_or_else(|| internal("embedded slot without a property"))?;
            let mut g = o.struct_mut(p)?;
            set_at(&mut *g, &this.path, prop, v)
        }
        _ => Err(internal(
            "colour-management settings not rooted at a datablock",
        )),
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
