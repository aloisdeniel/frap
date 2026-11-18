// SPDX-License-Identifier: GPL-3.0-or-later
//! `View3DShading.studio_light` (`scene.display.shading`, `bpy.context.space_data.shading`):
//! one enum over three DNA names, routed by `type` and `light` (WP 2.3).
//!
//! Ported from blender/source/blender/makesrna/intern/rna_space.cc
//! (`rna_View3DShading_studio_light_get_storage`, `_get`, `_set`, `_itemf`) and
//! blender/source/blender/blenkernel/intern/studiolight.cc (`BKE_studiolight_find`,
//! `BKE_studiolight_find_default`):
//!
//! - `type == 'SOLID'` and `light == 'MATCAP'`: the MatCap (DNA `matcap`), items = the matcaps;
//! - `type == 'SOLID'` otherwise: the studio rig (DNA `studio_light`), items = `Default` and
//!   the `.sl` rigs;
//! - `type` `MATERIAL`/`RENDERED`: the HDRI (DNA `lookdev_light`), items = the world HDRIs;
//!   `WIREFRAME`: the same storage with no items (reads `''`, every assignment fails).
//!
//! A read gives the stored name when it is a light of the routed type, else the type's default
//! (`Default`, `basic_bright.exr`, `forest.exr`). An assignment outside the routed list is
//! Blender's `TypeError: bpy_struct: item.attr = val: enum "rim.sl" not found in (…)` (verified
//! on 5.2.4 in each mode, also for the per-mode reads after switching `light`).

use frap_rna::{
    Field, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg, Value, rna_hook,
};

use smol_str::SmolStr;

use crate::generated::plain::View3DShading;
use crate::id::paths::scene_of;
use crate::{Host, HostRead};

/// Studio rigs (`STUDIOLIGHT_TYPE_STUDIO`, the internal `Default` first), shipped with 5.2.4.
pub const STUDIO: [&str; 6] = [
    "Default",
    "basic.sl",
    "outdoor.sl",
    "paint.sl",
    "rim.sl",
    "studio.sl",
];

/// World HDRIs (`STUDIOLIGHT_TYPE_WORLD`), shipped with 5.2.4.
pub const WORLD: [&str; 8] = [
    "city.exr",
    "courtyard.exr",
    "forest.exr",
    "interior.exr",
    "night.exr",
    "studio.exr",
    "sunrise.exr",
    "sunset.exr",
];

/// MatCaps (`STUDIOLIGHT_TYPE_MATCAP`), shipped with 5.2.4.
pub const MATCAP: [&str; 27] = [
    "basic_bright.exr",
    "basic_dark.exr",
    "basic_grey.exr",
    "basic_side.exr",
    "brush_thumbnail_preview.exr",
    "ceramic_dark.exr",
    "ceramic_lightbulb.exr",
    "check_gradient.exr",
    "check_normal+y.exr",
    "check_reflection_horizontal.exr",
    "check_reflection_vertical.exr",
    "check_rim_dark.exr",
    "check_rim_light.exr",
    "clay_brown.exr",
    "clay_green.exr",
    "clay_studio.exr",
    "clay_warm.exr",
    "fullmetal.exr",
    "hard_surface_grey.exr",
    "hard_surface_red.exr",
    "metal_bronze.exr",
    "metal_carpaint.exr",
    "pearl.exr",
    "red_wax.exr",
    "resin.exr",
    "toon_dark.exr",
    "toon_light.exr",
];

/// Which DNA name `studio_light` stands for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    /// DNA `studio_light`.
    Studio,
    /// DNA `matcap`.
    Matcap,
    /// DNA `lookdev_light`.
    Lookdev,
}

/// The routed slot and the enum items of the current `type`/`light`
/// (`rna_View3DShading_studio_light_get_storage` and `_itemf`).
pub fn route(s: &View3DShading) -> (Slot, &'static [&'static str]) {
    match s.r#type.as_str() {
        "SOLID" if s.light == "MATCAP" => (Slot::Matcap, &MATCAP),
        "SOLID" => (Slot::Studio, &STUDIO),
        "MATERIAL" | "RENDERED" => (Slot::Lookdev, &WORLD),
        _ => (Slot::Lookdev, &[]),
    }
}

/// `BKE_studiolight_find(name, type)`: the stored name when it is a light of that type, else
/// the type's default (`BKE_studiolight_find_default`).
pub fn find(slot: Slot, name: &str) -> &'static str {
    let (list, default): (&[&'static str], &'static str) = match slot {
        Slot::Studio => (&STUDIO, "Default"),
        Slot::Matcap => (&MATCAP, "basic_bright.exr"),
        Slot::Lookdev => (&WORLD, "forest.exr"),
    };
    list.iter().copied().find(|n| *n == name).unwrap_or(default)
}

/// The DNA name of `slot`.
pub fn stored(s: &View3DShading, slot: Slot) -> &SmolStr {
    match slot {
        Slot::Studio => &s.studio_light,
        Slot::Matcap => &s.matcap,
        Slot::Lookdev => &s.lookdev_light,
    }
}

/// What `shading.studio_light` reads (`''` when the current type lists no light).
pub fn get_value(s: &View3DShading) -> SmolStr {
    let (slot, items) = route(s);
    if items.is_empty() {
        return SmolStr::default();
    }
    SmolStr::new_static(find(slot, stored(s, slot)))
}

/// `shading.studio_light = name`: stores `name` in the routed DNA slot, or Blender's enum
/// `TypeError` when the current type does not list it.
pub fn set_value(s: &mut View3DShading, name: &str) -> Result<(), RnaError> {
    let (slot, items) = route(s);
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
        .with_detail("valid", items.to_vec()));
    }
    let name = SmolStr::new(name);
    match slot {
        Slot::Studio => s.studio_light = name,
        Slot::Matcap => s.matcap = name,
        Slot::Lookdev => s.lookdev_light = name,
    }
    Ok(())
}

/// Where the shading struct lives: `scene.display.shading` or the virtual 3D View's.
enum Owner {
    Scene(frap_rna::IdRef<frap_rna::SceneId>),
    Viewport,
}

fn owner(this: &RnaPtr) -> Result<Owner, RnaError> {
    let is = |a: &'static str, b: &'static str| {
        this.path.len() == 2 && this.path[0] == Seg::Prop(a) && this.path[1] == Seg::Prop(b)
    };
    match this.root {
        Root::Id(_) if is("display", "shading") => Ok(Owner::Scene(scene_of(this)?)),
        Root::Context if is("space_data", "shading") => Ok(Owner::Viewport),
        _ => Err(RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::RuntimeError,
            "frap internal: View3DShading outside scene.display and space_data",
        )),
    }
}

fn gone() -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::RuntimeError,
        "frap internal: the scene of a View3DShading no longer exists",
    )
}

/// `View3DShading` is a generated plain binding (`hooked` in `xtask/src/codegen/gen_plain.rs`,
/// not `rna_struct!`), so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "View3DShading",
    prop: "studio_light",
    coll: false,
};

#[rna_hook(srna = "View3DShading", prop = "studio_light", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let s = match owner(this)? {
            Owner::Scene(scene) => &h.main().scenes.get(scene).ok_or_else(gone)?.display.shading,
            Owner::Viewport => &h.win().view3d.shading,
        };
        Ok(Field::Value(Value::Enum(get_value(s))))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Value::Enum(name) = v else {
            return Ok(());
        };
        match owner(this)? {
            Owner::Scene(scene) => {
                // Validate first: a refused assignment leaves the scene unmodified.
                let mut probe = h.main().scenes.get(scene).ok_or_else(gone)?.display.shading.clone();
                set_value(&mut probe, &name)?;
                let (main, log) = h.main_mut();
                let s = main.scenes.get_mut(scene, log).ok_or_else(gone)?;
                set_value(&mut s.display.shading, &name)
            }
            Owner::Viewport => {
                let mut probe = h.win().view3d.shading.clone();
                set_value(&mut probe, &name)?;
                set_value(&mut h.win_mut().view3d.shading, &name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_follows_blender() {
        // The 5.2.4 sequence of the WP 2.3 verifier.
        let mut s = View3DShading::default();
        assert_eq!(get_value(&s), "Default");
        assert!(set_value(&mut s, "clay_brown.exr").is_err());
        s.light = "MATCAP".into();
        assert_eq!(get_value(&s), "basic_bright.exr");
        let e = set_value(&mut s, "rim.sl").unwrap_err();
        assert!(e.message.contains("enum \"rim.sl\" not found in ('basic_bright.exr'"), "{}", e.message);
        set_value(&mut s, "clay_brown.exr").unwrap();
        s.light = "STUDIO".into();
        assert_eq!(get_value(&s), "Default");
        set_value(&mut s, "rim.sl").unwrap();
        s.light = "FLAT".into();
        assert_eq!(get_value(&s), "rim.sl");
        s.light = "MATCAP".into();
        assert_eq!(get_value(&s), "clay_brown.exr");
        s.r#type = "MATERIAL".into();
        assert_eq!(get_value(&s), "forest.exr");
        set_value(&mut s, "city.exr").unwrap();
        s.r#type = "WIREFRAME".into();
        assert_eq!(get_value(&s), "");
        let e = set_value(&mut s, "Default").unwrap_err();
        assert!(e.message.ends_with("not found in ()"), "{}", e.message);
        s.r#type = "SOLID".into();
        assert_eq!(get_value(&s), "clay_brown.exr");
        assert_eq!((s.studio_light.as_str(), s.lookdev_light.as_str()), ("rim.sl", "city.exr"));
    }

    #[test]
    fn lists_cover_the_spec_items() {
        let prop = frap_spec_rna::find_struct("View3DShading")
            .unwrap()
            .find_prop("studio_light")
            .unwrap();
        let frap_rna::PropKind::Enum { items, .. } = prop.kind else {
            panic!("studio_light is an enum")
        };
        let mut ours: Vec<&str> = STUDIO.iter().chain(&WORLD).chain(&MATCAP).copied().collect();
        let mut spec: Vec<&str> = items.iter().map(|i| i.identifier).collect();
        ours.sort_unstable();
        spec.sort_unstable();
        assert_eq!(ours, spec);
    }
}
