// SPDX-License-Identifier: GPL-3.0-or-later
//! UI members of `bpy.context` in frap's virtual 3D View (ARCHITECTURE §8; WP 2.3).
//!
//! - `window`: the `Window` stub (`frap-scene` binding; `scene`/`view_layer` writable).
//! - `space_data`: a `SpaceView3D` stub with `type` (`'VIEW_3D'`), `shading` (the virtual
//!   viewport's `View3DShading`, writable, saved in `.frap`) and `region_3d` (a read-only
//!   `RegionView3D` stub: `view_matrix` of the factory Layout viewport, `view_perspective`
//!   `'PERSP'`, `is_perspective`, `is_orthographic_side_view`).
//! - `preferences`: the factory preferences, read-only: `edit` (`PreferencesEdit`, every value
//!   of `--factory-startup` 5.2.4) and `addons` (the eight factory add-on modules; only
//!   `addons['io_scene_gltf2'].preferences` is bound, and writable: it is saved in `.frap`).
//!   Writing any other preference is refused (`NOT_IMPLEMENTED`): Blender would change the
//!   user preferences of the running process, which a headless session does not have.
//! - `area`, `region`, `region_data`, `screen`: `None`. frap has no screen, area or region (§8);
//!   Blender 5.2.4 returns the Layout screen's members inside the VIEW_3D override, and
//!   `bpy.data.screens['Layout']` for `screen` even outside one (verified).
//!
//! Pointers below `bpy.context` keep `Root::Context` and the member path
//! (`bpy.context.space_data.shading`): Blender's repr names the Layout screen
//! (`bpy.data.screens['Layout'].areas[3].spaces[0].shading`), which frap does not model.

use frap_rna::traits::{ToRnaValue, index_out_of_range, no_attribute};
use frap_rna::{
    ArrayShape, CollRef, EnumDefault, EnumItem, Field, FrapError, Item, MathTag, PropDef,
    PropFlags, PropKind, PyExc, Rna, RnaColl, RnaError, RnaErrorKind, RnaMut, RnaPtr, Root, Seg,
    StructDef, StructRef, Subtype, Unit, Value,
};
use frap_scene::generated::plain::View3DShading;
use frap_scene::{Host, HostRead, Preferences, VirtualView3D, WindowState};
use smol_str::SmolStr;

use super::super::Target;
use super::super::access::{not_bound, rna_err, stale};
use super::Member;
use crate::errors::NOT_IMPLEMENTED;

// ------------------------------------------------------------------ SpaceView3D stub

static SPACE_TYPE_ITEMS: [EnumItem; 1] = [EnumItem {
    identifier: "VIEW_3D",
    name: "3D Viewport",
    description: "Manipulate objects in a 3D environment",
    value: 1,
}];

static SPACE_VIEW3D_PROPS: [PropDef; 3] = [
    PropDef {
        identifier: "type",
        name: "Type",
        description: "Space data type",
        kind: PropKind::Enum {
            items: &SPACE_TYPE_ITEMS,
            default: EnumDefault::Item("VIEW_3D"),
            dynamic: None,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::READONLY,
        deprecated: None,
    },
    PropDef {
        identifier: "shading",
        name: "Shading Settings",
        description: "Settings for shading in the 3D viewport",
        kind: PropKind::Pointer {
            fixed_type: "View3DShading",
            is_id: false,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::READONLY.union(PropFlags::NEVER_NULL),
        deprecated: None,
    },
    PropDef {
        identifier: "region_3d",
        name: "3D Region",
        description: "3D region for this space. When the space is in quad view, the camera region",
        kind: PropKind::Pointer {
            fixed_type: "RegionView3D",
            is_id: false,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::READONLY,
        deprecated: None,
    },
];

/// `SpaceView3D` as frap exposes it: the stub of §8 (Blender's struct has ~80 more members,
/// which frap's virtual viewport does not model; `Space` is not in the dumped spec either).
pub static SPACE_VIEW3D: StructDef = StructDef {
    identifier: "SpaceView3D",
    base: None,
    name: "3D View Space",
    description: "3D View space data (frap: the virtual Layout viewport stub)",
    props: &SPACE_VIEW3D_PROPS,
    funcs: &[],
    name_property: None,
    is_id: false,
};

struct SpaceView3DRef<'a>(&'a VirtualView3D);

impl Rna for SpaceView3DRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &SPACE_VIEW3D
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "type" => Field::Value(Value::Enum(SmolStr::new_static("VIEW_3D"))),
            "shading" => Field::Struct(StructRef::Borrowed(&self.0.shading)),
            "region_3d" => Field::Struct(StructRef::Owned(Box::new(RegionView3DRef(self.0)))),
            other => return Err(no_attribute(&SPACE_VIEW3D, other)),
        })
    }
}

struct RegionView3DRef<'a>(&'a VirtualView3D);

impl Rna for RegionView3DRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::REGION_VIEW3_D
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(Field::Value(match prop.identifier {
            "view_matrix" => self.0.view_matrix.to_rna_value(prop, MathTag::Matrix),
            "view_perspective" => Value::Enum(SmolStr::new_static("PERSP")),
            "is_perspective" => Value::Bool(true),
            "is_orthographic_side_view" => Value::Bool(false),
            // Every other member answers `RNA_NOT_IMPLEMENTED` (the resolver's "not bound").
            other => return Err(no_attribute(self.srna(), other)),
        }))
    }
}

// ------------------------------------------------------------------ Preferences

/// The add-on modules of `--factory-startup` 5.2.4, in `preferences.addons` order (verified).
const FACTORY_ADDONS: [&str; 8] = [
    "io_anim_bvh",
    "io_curve_svg",
    "io_mesh_uv_layout",
    "io_scene_fbx",
    "io_scene_gltf2",
    "cycles",
    "pose_library",
    "bl_pkg",
];

/// The add-on whose preferences frap binds (§8, §4.1).
const GLTF: &str = "io_scene_gltf2";

struct PreferencesRef<'a>(&'a Preferences);

impl Rna for PreferencesRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::PREFERENCES
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "edit" => Field::Struct(StructRef::Owned(Box::new(PreferencesEditRef))),
            "addons" => Field::Coll(CollRef::Owned(Box::new(AddonsRef(self.0)))),
            other => return Err(no_attribute(self.srna(), other)),
        })
    }
}

/// `preferences.edit`: the factory values (`U_default`), which equal the RNA defaults except
/// `keyframe_new_handle_type` (`AUTO_CLAMPED`; every member compared on 5.2.4).
struct PreferencesEditRef;

impl Rna for PreferencesEditRef {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::PREFERENCES_EDIT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(Field::Value(match prop.identifier {
            "keyframe_new_handle_type" => Value::Enum(SmolStr::new_static("AUTO_CLAMPED")),
            _ => Value::default_for(prop),
        }))
    }
}

struct AddonsRef<'a>(&'a Preferences);

impl RnaColl for AddonsRef<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::ADDON
    }
    fn len(&self) -> usize {
        FACTORY_ADDONS.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        FACTORY_ADDONS.get(i).map(|m| SmolStr::new_static(m))
    }
    fn find(&self, key: &str) -> Option<usize> {
        FACTORY_ADDONS.iter().position(|m| *m == key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        let module = *FACTORY_ADDONS
            .get(i)
            .ok_or_else(|| index_out_of_range(i, FACTORY_ADDONS.len()))?;
        Ok(Item::Struct(StructRef::Owned(Box::new(AddonRef {
            prefs: self.0,
            module,
        }))))
    }
}

struct AddonRef<'a> {
    prefs: &'a Preferences,
    module: &'static str,
}

impl Rna for AddonRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::ADDON
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "module" => Field::Value(Value::Str(SmolStr::new_static(self.module))),
            "preferences" if self.module == GLTF => {
                Field::Struct(StructRef::Owned(Box::new(GltfPrefsRef(self.prefs))))
            }
            other => return Err(no_attribute(self.srna(), other)),
        })
    }
}

/// `addons['io_scene_gltf2'].preferences`: the stored values, else the add-on's defaults.
struct GltfPrefsRef<'a>(&'a Preferences);

impl Rna for GltfPrefsRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::GLTF_ADDON_PREFERENCES
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if prop.identifier == "bl_idname" {
            return Ok(Field::Value(Value::Str(SmolStr::new_static(GLTF))));
        }
        let stored = self.0.addons.get(GLTF).and_then(|m| m.get(prop.identifier));
        Ok(Field::Value(
            stored.cloned().unwrap_or_else(|| Value::default_for(prop)),
        ))
    }
}

// ------------------------------------------------------------------ bpy.context root

/// `bpy.context` as the root of the UI member paths (`window`, `space_data`, `preferences`).
pub struct ContextUi<'a> {
    win: &'a WindowState,
    prefs: &'a Preferences,
}

impl Rna for ContextUi<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::CONTEXT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "window" => Field::Struct(StructRef::Borrowed(self.win)),
            "space_data" => {
                Field::Struct(StructRef::Owned(Box::new(SpaceView3DRef(&self.win.view3d))))
            }
            "preferences" => Field::Struct(StructRef::Owned(Box::new(PreferencesRef(self.prefs)))),
            other => return Err(no_attribute(self.srna(), other)),
        })
    }
}

/// The root struct a `Root::Context` pointer with a path walks from.
pub fn root(host: &dyn HostRead) -> ContextUi<'_> {
    let c = host.ctx();
    ContextUi {
        win: c.win,
        prefs: c.prefs,
    }
}

/// The struct type of a UI member.
fn member_srna(identifier: &str) -> Option<&'static StructDef> {
    Some(match identifier {
        "window" => &frap_spec_rna::WINDOW,
        "space_data" => &SPACE_VIEW3D,
        "preferences" => &frap_spec_rna::PREFERENCES,
        _ => return None,
    })
}

/// Reads `bpy.context.<prop>` for the UI members.
pub(super) fn member(host: &dyn HostRead, prop: &'static PropDef) -> Result<Member, FrapError> {
    let _ = host;
    if let Some(srna) = member_srna(prop.identifier) {
        let ptr = RnaPtr::new(Root::Context, &frap_spec_rna::CONTEXT)
            .child(Seg::Prop(prop.identifier), srna);
        return Ok(Member::Target(Target::Struct(ptr)));
    }
    match prop.identifier {
        "area" | "region" | "region_data" | "screen" => Ok(Member::Target(Target::None)),
        other => Err(FrapError::new(
            PyExc::NotImplementedError,
            NOT_IMPLEMENTED,
            format!("frap: bpy.context.{other} is not implemented yet"),
        )),
    }
}

// ------------------------------------------------------------------ writes

fn read_only_preferences(ptr: &RnaPtr, prop: &PropDef) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!(
            "frap: {}.{} is a user preference; frap's preferences are the factory ones and \
             read-only, except bpy.context.preferences.addons['io_scene_gltf2'].preferences",
            ptr_repr(ptr),
            prop.identifier
        ),
    )
    .with_hint("headless sessions have no user preferences; set the operator's own option instead")
}

fn ptr_repr(ptr: &RnaPtr) -> String {
    let mut s = String::from("bpy.context");
    for seg in &ptr.path {
        match seg {
            Seg::Prop(p) => {
                s.push('.');
                s.push_str(p);
            }
            Seg::Key(k) => s.push_str(&format!("['{k}']")),
            Seg::Index(i) => s.push_str(&format!("[{i}]")),
        }
    }
    s
}

/// Writes `prop` of the struct at `ptr` (a `Root::Context` pointer with a path; set hooks such
/// as `Window.scene` and `View3DShading.studio_light` run before this).
pub fn set_prop(
    host: &mut dyn Host,
    ptr: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), FrapError> {
    let first = match ptr.path.first() {
        Some(Seg::Prop(p)) => *p,
        _ => return Err(stale("a bpy.context pointer without a member")),
    };
    match (first, &ptr.path[1..]) {
        ("space_data", [Seg::Prop("shading")]) => {
            let shading: &mut View3DShading = &mut host.win_mut().view3d.shading;
            let srna = shading.srna();
            match shading.set(prop, v) {
                Ok(_) => Ok(()),
                Err(e) if e.kind == RnaErrorKind::NoAttribute => Err(not_bound(srna, prop)),
                Err(e) => Err(rna_err(e)),
            }
        }
        ("space_data", _) => Err(FrapError::new(
            PyExc::NotImplementedError,
            NOT_IMPLEMENTED,
            format!(
                "frap: {}.{} cannot be set: frap's virtual 3D View is fixed (the factory Layout \
                 viewport)",
                ptr_repr(ptr),
                prop.identifier
            ),
        )),
        (
            "preferences",
            [
                Seg::Prop("addons"),
                Seg::Key(module),
                Seg::Prop("preferences"),
            ],
        ) if module == GLTF => {
            let prefs = host
                .prefs_mut()
                .ok_or_else(|| stale("this host has no preferences"))?;
            prefs
                .addons
                .entry(SmolStr::new_static(GLTF))
                .or_default()
                .insert(SmolStr::new(prop.identifier), v);
            Ok(())
        }
        ("preferences", _) => Err(read_only_preferences(ptr, prop)),
        _ => Err(not_bound(ptr.srna, prop)),
    }
}

#[cfg(test)]
mod tests {
    use frap_math::glam::Mat4;

    use super::*;

    #[test]
    fn stub_props_resolve() {
        for p in ["type", "shading", "region_3d"] {
            assert!(SPACE_VIEW3D.find_prop(p).is_some(), "{p}");
        }
        let rv = &frap_spec_rna::REGION_VIEW3_D;
        let v = VirtualView3D {
            view_matrix: Mat4::IDENTITY,
            shading: View3DShading::default(),
        };
        let r = RegionView3DRef(&v);
        assert!(r.get(rv.find_prop("view_matrix").unwrap()).is_ok());
        let e = r.get(rv.find_prop("view_distance").unwrap()).err().unwrap();
        assert_eq!(e.kind, RnaErrorKind::NoAttribute);
    }

    #[test]
    fn factory_edit_preferences() {
        let e = &frap_spec_rna::PREFERENCES_EDIT;
        let get = |p: &str| match PreferencesEditRef.get(e.find_prop(p).unwrap()) {
            Ok(Field::Value(v)) => v,
            _ => panic!("{p}"),
        };
        assert_eq!(
            get("keyframe_new_handle_type"),
            Value::Enum("AUTO_CLAMPED".into())
        );
        assert_eq!(
            get("keyframe_new_interpolation_type"),
            Value::Enum("BEZIER".into())
        );
        assert_eq!(get("use_duplicate_mesh"), Value::Bool(true));
        assert_eq!(get("use_duplicate_material"), Value::Bool(false));
        assert_eq!(get("undo_steps"), Value::Int(32));
    }
}
