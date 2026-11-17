// SPDX-License-Identifier: GPL-3.0-or-later
//! The few hand-written parts of the manifest: `defaults_that_surprise[]`, the "unset"
//! semantics of Blender's generic object-add properties, and the poll context legend. Every
//! operator default quoted here is checked against the generated spec by a test
//! (`tests/acceptance.rs`), so a Blender update cannot make this table lie silently.
//!
//! Each fact was verified on Blender 5.2.4 `-b --factory-startup` (WP 1.18 probes): `object.light_add(type='POINT')` creates a 10 W light (the factory
//! `Light` has 1000 W); `primitive_cube_add()` with `rotation` unset and `align='CURSOR'` takes
//! the cursor rotation, an explicit `rotation` wins. `scale` scales the mesh data while the
//! object scale stays 1, but only for the primitives in [`PRIMITIVES_APPLYING_SCALE`]; those in
//! [`PRIMITIVES_IGNORING_SCALE`] ignore it (`scale=(3, 1, 0.5)` and `scale=(0, 0, 0)` give the
//! same dimensions as unset), and `mesh.primitive_torus_add` has none.

use frap_rna::OperatorSpec;

/// What a surprising default belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SurpriseKind {
    /// An operator property default (`target` = `ns.op.prop`); checked against the spec.
    OperatorProperty,
    /// A value of data created by an operator or `bpy.data.*.new` (`target` = RNA path form).
    Data,
}

/// One `defaults_that_surprise[]` entry.
#[derive(Copy, Clone, Debug)]
pub struct SurpriseDefault {
    /// `export_scene.gltf.export_apply`, `Light.energy`.
    pub target: &'static str,
    /// Kind.
    pub kind: SurpriseKind,
    /// The default, as JSON text (`false`, `10`, `"GLB"`).
    pub default_json: &'static str,
    /// Why it surprises, and what to pass instead.
    pub why: &'static str,
}

/// Defaults agents get wrong most often (§14).
pub const DEFAULTS_THAT_SURPRISE: &[SurpriseDefault] = &[
    SurpriseDefault {
        target: "export_scene.gltf.export_apply",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "modifiers are NOT applied on export; pass export_apply=True to export the evaluated mesh",
    },
    SurpriseDefault {
        target: "export_scene.gltf.export_cameras",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "cameras are not exported unless export_cameras=True",
    },
    SurpriseDefault {
        target: "export_scene.gltf.export_lights",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "lights are not exported unless export_lights=True (KHR_lights_punctual)",
    },
    SurpriseDefault {
        target: "export_scene.gltf.use_selection",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "the whole scene is exported, not only the selection; pass use_selection=True to limit it",
    },
    SurpriseDefault {
        target: "import_scene.gltf.merge_vertices",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "imported meshes keep glTF's split vertices (seams at every UV/normal split) unless merge_vertices=True",
    },
    SurpriseDefault {
        target: "render.render.write_still",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "render.render() writes nothing to disk unless write_still=True (the image goes to bpy.data.images['Render Result']); the path is scene.render.filepath",
    },
    SurpriseDefault {
        target: "mesh.bevel.offset",
        kind: SurpriseKind::OperatorProperty,
        default_json: "0",
        why: "bevel with the default offset=0 changes nothing; always pass offset (and segments)",
    },
    SurpriseDefault {
        target: "mesh.vertices_smooth.factor",
        kind: SurpriseKind::OperatorProperty,
        default_json: "0",
        why: "vertices_smooth with the default factor=0 changes nothing; pass factor (e.g. 0.5)",
    },
    SurpriseDefault {
        target: "mesh.primitive_cube_add.scale",
        kind: SurpriseKind::OperatorProperty,
        default_json: "[0,0,0]",
        why: "the (0, 0, 0) default is a placeholder: unset means (1, 1, 1); scale is applied to the mesh data and the new object's scale stays 1. Only cube, cone, cylinder, uv_sphere and ico_sphere apply scale: plane, grid, circle, monkey and the curve.*/surface.* primitives ignore it (set object.scale or use transform.resize afterwards)",
    },
    SurpriseDefault {
        target: "export_scene.gltf.export_current_frame",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "objects are exported at their frame-0 rest transforms, not at scene.frame_current; pass export_current_frame=True to export the pose of the current frame",
    },
    SurpriseDefault {
        target: "transform.rotate.value",
        kind: SurpriseKind::OperatorProperty,
        default_json: "0",
        why: "angles are in radians everywhere (operators and RNA): value=90 is 90 rad; use radians(90) or math.pi/2",
    },
    SurpriseDefault {
        target: "object.shade_smooth.keep_sharp_edges",
        kind: SurpriseKind::OperatorProperty,
        default_json: "true",
        why: "edges marked sharp stay sharp after shade_smooth; pass keep_sharp_edges=False to smooth everything",
    },
    SurpriseDefault {
        target: "object.parent_set.keep_transform",
        kind: SurpriseKind::OperatorProperty,
        default_json: "false",
        why: "parent_set keeps the child in place through matrix_parent_inverse: its location/rotation/scale values do not change and are now relative to the parent's inverse; read matrix_world for the world transform",
    },
    SurpriseDefault {
        target: "Light.energy",
        kind: SurpriseKind::Data,
        default_json: "10",
        why: "object.light_add(type='POINT') and bpy.data.lights.new() create a 10 W light (the factory scene's 'Light' has 1000 W); a 10 W point light is nearly invisible at scene scale, set energy",
    },
    SurpriseDefault {
        target: "Scene.frame_current",
        kind: SurpriseKind::Data,
        default_json: "1",
        why: "the factory scene starts at frame 1 (frame_start=1); keyframes and evaluation use frame_current, not frame 0",
    },
];

/// The `*.primitive_*_add` operators whose Blender exec ignores the `scale` property:
/// `scale=(3, 1, 0.5)` and `scale=(0, 0, 0)` give the same dimensions as unset and the object
/// scale stays 1 (verified on 5.2.4). `mesh.primitive_cube_add_gizmo` (needs a 3D view, so it
/// cannot be probed headless) is in neither list and gets no claim.
pub const PRIMITIVES_IGNORING_SCALE: &[&str] = &[
    "mesh.primitive_plane_add",
    "mesh.primitive_grid_add",
    "mesh.primitive_circle_add",
    "mesh.primitive_monkey_add",
    "curve.primitive_bezier_curve_add",
    "curve.primitive_bezier_circle_add",
    "curve.primitive_nurbs_curve_add",
    "curve.primitive_nurbs_circle_add",
    "curve.primitive_nurbs_path_add",
    "surface.primitive_nurbs_surface_curve_add",
    "surface.primitive_nurbs_surface_circle_add",
    "surface.primitive_nurbs_surface_surface_add",
    "surface.primitive_nurbs_surface_cylinder_add",
    "surface.primitive_nurbs_surface_sphere_add",
    "surface.primitive_nurbs_surface_torus_add",
];

/// The `*.primitive_*_add` operators whose Blender exec applies the `scale` property to the new
/// mesh data (`make_prim_init` + `ED_object_new_primitive_matrix` with the scale, verified on
/// 5.2.4). The other primitives with a `scale` property ignore it ([`PRIMITIVES_IGNORING_SCALE`]).
pub const PRIMITIVES_APPLYING_SCALE: &[&str] = &[
    "mesh.primitive_cube_add",
    "mesh.primitive_cone_add",
    "mesh.primitive_cylinder_add",
    "mesh.primitive_uv_sphere_add",
    "mesh.primitive_ico_sphere_add",
];

/// True when `spec` has Blender's generic object-add properties (`ED_object_add_generic_props`:
/// `align`, `location`, `rotation`), whose unset values have special meaning.
fn has_generic_add_props(spec: &OperatorSpec) -> bool {
    ["align", "location", "rotation"]
        .iter()
        .all(|p| spec.prop(p).is_some())
}

/// What leaving `prop` unset means (Blender's `RNA_struct_property_is_set` rules,
/// `ED_object_add_generic_get_opts`), or `None` when unset just means the default.
pub fn unset_semantics(spec: &OperatorSpec, prop: &str) -> Option<&'static str> {
    if !has_generic_add_props(spec) {
        return None;
    }
    match prop {
        "location" => Some("unset: the 3D cursor location (scene.cursor.location)"),
        "rotation" => Some(
            "unset: from align (WORLD: no rotation, VIEW: the 3D view rotation, CURSOR: the 3D cursor rotation); an explicit rotation always wins",
        ),
        "scale" if PRIMITIVES_APPLYING_SCALE.contains(&spec.idname) => Some(
            "unset: (1, 1, 1); the scale is applied to the new mesh data, the object's scale stays (1, 1, 1)",
        ),
        "scale" if PRIMITIVES_IGNORING_SCALE.contains(&spec.idname) => Some(
            "ignored: this primitive does not apply scale (any value gives the same result as unset); set object.scale or use transform.resize afterwards",
        ),
        _ => None,
    }
}

/// Legend of the dumped poll matrix contexts (`tools/blender/dump_ops.py`, [OBJ §0]).
pub const POLL_CONTEXTS: &[(&str, &str)] = &[
    (
        "A",
        "Object Mode, the factory 'Cube' (a mesh) active and selected",
    ),
    ("B", "Object Mode, nothing selected and no active object"),
    ("C", "Edit Mode on a mesh"),
    ("D", "Pose Mode on an armature"),
    ("E", "Edit Mode on an armature"),
];
