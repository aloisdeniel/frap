# Blender 5.2 operator catalogue for frap: object, primitives, transform, snapping, animation, render, files, glTF

Status: research document (no code). Target: **Blender 5.2 LTS Python API**.

**Provenance.** Unless marked *uncertain*, *believed* or *inferred*, every name, type, default, enum item, range and `bl_options` value below was **dumped from a real Blender 5.2.4 LTS** (build hash `cd5dc428cce1`, 2026-11-10) running `Blender -b --factory-startup` on macOS, through `bpy.ops.<ns>.<op>.get_rna_type()`. Poll results come from calling `op.poll()` in five context states, with and without a VIEW_3D `context.temp_override`. Statements marked *verified* come from scripted experiments in the same build. The glTF add-on is `io_scene_gltf2` **v5.2.41**. Section 8 shows how to regenerate the data.

Related research docs, written separately, cover the edit-mode `mesh.*` operators, the `uv.*` operators, modifier properties, shader nodes and RNA data properties. This document covers only the namespaces listed in the task.

---

## 0. How to read this document

- **Priority**: `P0` is MVP and must ship first. `P1` is needed for the fixed scope (GLB features, skinning, morphs, rendering). `P2` is parity or nice to have. `OUT` is out of scope and listed only for completeness.
- **Poll A-E**: `A` is object mode with mesh `Cube` active and selected (factory scene). `B` is object mode with nothing selected and no active object. `C` is mesh edit mode. `D` is armature pose mode. `E` is armature edit mode. Each column holds one of three values. `Y` means the poll passes in plain `blender -b`. `V` means it passes only with a VIEW_3D window/area/region override. `N` means it fails.
- ***(hidden)*** marks RNA `PROP_HIDDEN`: the property is not drawn in the redo panel, but **Python can still set it**, and scripts do (`enter_editmode`, `scale`, `modifier=`, `select_all(action=...)`, `delete(use_global=...)`, `transform_apply(isolate_users=...)`). frap must accept hidden properties and include them in `--describe`, flagged as hidden.
- Angles are always **radians** (`subtype` `ANGLE`/`EULER`, unit `ROTATION`). Lengths use unit `LENGTH`, scaled by `scene.unit_settings.scale_length` only for display.
- A type such as `FLOAT[3] XYZ (LENGTH)` gives the RNA type, the array length, the subtype and the unit. `ENUM flag-set` is an enum that takes a *set* of identifiers, for example `{'VERT','EDGE'}`.
- `dynamic enum` means Blender computes the items at runtime from context (see pitfall 4).

## 1. Key findings and pitfalls for the architect

1. **`screen.frame_set` does not exist** (verified: the `screen` namespace has only `frame_jump`, `frame_offset` and `keyframe_jump`). There are three ways to change frames:
   - the *method* `bpy.context.scene.frame_set(frame, subframe=0.0)` (RNA function parameters `frame`, `subframe`; it evaluates the animation: verified, a Bezier-interpolated location at frame 5 was 0.834);
   - the *property* `scene.frame_current = n`;
   - `screen.frame_jump(end=False)` and `screen.frame_offset(delta=n)`.

   The bpy-line front-end must therefore support **method calls on RNA structs**, not only `bpy.ops.*` calls and property assignment.
2. **Blender 5.x uses slotted, layered Actions.** **`Action.fcurves` no longer exists** (verified `hasattr(action,'fcurves') == False`). F-Curves live at `action.layers[0].strips[0].channelbag(slot).fcurves`, and `obj.animation_data.action_slot` selects the slot. Inserting a key on `Cube` created action **`CubeAction`**, slot identifier **`OBCube`** (2-letter ID-type prefix plus the name), one layer, one keyframe strip, and F-Curve group **`Object Transforms`**. New keys default to interpolation `BEZIER` with handles `AUTO_CLAMPED` (taken from `preferences.edit.keyframe_new_interpolation_type` and `keyframe_new_handle_type`). frap's data model and paths must follow this layout, or agents trained on 4.x paths (`action.fcurves`) will fail. Recommendation: return a clear error that names the 5.x path.
3. **frap needs "is the property set?" semantics, not just defaults.** Blender operators call `RNA_struct_property_is_set()`:
   - A primitive's `location` defaults to the **3D cursor** when not passed. Passing `(0,0,0)` explicitly is different from omitting it.
   - `rotation` falls back to `align` when not passed.
   - `scale` has a sentinel default of `(0,0,0)`, which means unset, so 1.
   - `render.render(frame_start/frame_end)` overrides the scene range only when the values are specified.

   The typed operator registry must keep "explicitly provided" flags for each argument.
4. **Dynamic enums.** `object.mode_set.mode` (filtered by active object type), `object.modifier_add.type` (by object type), `export_scene.gltf.export_format` (`GLTF_EMBEDDED` appears only behind an add-on preference), `anim.keyframe_insert.type` (keying sets), `object.constraint_add.type`, `object.vertex_group_set_active.group`, the `vertex_group_*` `group_select_mode` and `object.collection_link.collection` change their items with context. For such properties the `--describe` schema should publish the static superset plus a context rule. An invalid value must give Blender's error text: `TypeError: Converting py args to operator properties: enum "EDIT" not found in ('OBJECT')` (verified).
5. **Errors come in three kinds in Blender**, and frap's JSON should preserve the distinction:
   - a) *argument errors* raise `TypeError`, for example `... keyword "sizee" unrecognized` and `MESH_OT_primitive_cube_add.size expected a float type, not str` (verified);
   - b) *poll failures* raise `RuntimeError: Operator bpy.ops.X.poll() <message>`, for example `Object has no vertex groups`, `This modifier operation is not allowed from Edit mode`, `failed, context is incorrect` (verified);
   - c) *exec failures* either raise `RuntimeError: Error: <report>` (for example `Cannot render, no camera`, `Cannot apply to a multi user: Object "Cube", Mesh "Cube", aborting`) or **silently return `{'CANCELLED'}` with only a WARNING report**: `join` gives `Active object is not a selected mesh` / `No mesh data to join`, and `modifier_remove`/`modifier_apply` with an unknown name return CANCELLED with no message (verified).

   Recommendation: one response shape, `{status: FINISHED|CANCELLED|ERROR, reports:[{type: INFO|WARNING|ERROR, message}], ...}`. Treat CANCELLED as a failure with no state change, and synthesize a message where Blender gives none. INFO reports also matter, for example `Applied modifier was not first, result may not be as expected` (verified).
6. **Macro operators** (`object.duplicate_move`, `object.duplicate_move_linked`, and later `mesh.extrude_region_move`, `mesh.duplicate_move`, `mesh.loopcut_slide`) take **nested property groups** named after the sub-operator id, for example `TRANSFORM_OT_translate={'value':(2,0,0)}` (verified). Both front-ends need syntax for nested dicts. A short-form proposal: `object.duplicate_move TRANSFORM_OT_translate.value=2,0,0`.
7. **3D View context.** About 20 catalogued operators (`view3d.snap_*`, `anim.keyframe_*`, `object.hide_view_*`, `transform.shear/bend/create_orientation`) fail their poll in `blender -b` unless a VIEW_3D area is overridden (`V` in the tables). Other operators change behaviour when a 3D View exists:
   - `camera_add` sets `scene.camera` only with a view (verified);
   - `align='VIEW'` uses the view rotation (verified: `(1.1088, 0, 1.1549)` with the factory *Layout* viewport, identity without one).

   **Recommendation:** frap acts as if one virtual 3D View exists. Its view matrix equals Blender's factory-startup Layout viewport, so `align='VIEW'` matches Blender. The golden-test harness must run every Blender script inside `with bpy.context.temp_override(window=w, area=<VIEW_3D area>, region=<WINDOW region>):`, which works in `-b` because the factory screen *Layout* exists. Otherwise Blender itself fails or diverges.
8. **Primitives.**
   - The object name equals the mesh name (`Cube`, `Sphere`, `Icosphere`, `Suzanne`, ...).
   - **`scale`/`size`/`radius` are baked into the vertices; object scale stays 1** (verified).
   - Faces are flat through the `sharp_face` attribute, and a `UVMap` UV layer is created.
   - In edit mode the geometry is merged into the active mesh and becomes the only selection.
   - Exact vertex order matters for position-by-index goldens.
   - **Suzanne's vertex table is GPL code** (licensing decision needed).
9. **Shading model since 4.1:** `use_auto_smooth` is gone. Smoothness is the face attribute `sharp_face` plus the edge attribute `sharp_edge`. `shade_smooth` removes `sharp_face`, `shade_flat` sets it, and `shade_smooth_by_angle` is destructive (it writes `sharp_edge`). **`shade_auto_smooth` adds a Geometry Nodes modifier "Smooth by Angle"** with node group "Smooth by Angle" from the Essentials assets (verified). frap needs a built-in stand-in for that one node group.
10. **Duplicate Data preferences**: `object.duplicate` deep-copies the mesh but **shares materials** by factory default (full `use_duplicate_*` table at the end of section 3).
11. **Orphans**: `delete` and `join` leave 0-user data-blocks in `bpy.data` (verified). Blender does not write 0-user IDs on save, so `.frap` save should drop them, which also matches what an agent sees after a reload.
12. **Default values differ by creation path.** `light_add` POINT gives **10 W**, while the startup `Light` has **1000 W** (verified). `material.new` creates Principled BSDF + Material Output but **does not assign the material to a slot when called from a script** (verified). Agents should use `bpy.data.materials.new()` + `mesh.materials.append()`, so frap must support `bpy.data.<collection>.new/remove/load` and `append`.
13. **Transforms decompose matrices.** Non-uniform scale or rotation in GLOBAL orientation on rotated objects is decomposed back into loc/rot/scale, which loses shear (verified: resize (2,1,1) on a 0.5 rad Z-rotated cube gave scale (1.8195, 1.2998, 1.0)). `transform.rotate(value=+θ)` rotates by +θ about the axis (verified). Edit-mode transform values are in **world space**.
14. **Selection order** is the view-layer base order, not click order (verified: `selected_objects` = `['Light','Cube','Plane']` regardless of the order of the `select_set` calls). The active object can be unselected.
15. **ID names may be up to 255 bytes in 5.x** (verified: a 300-char name was truncated to 255; it was 63 bytes before 5.0). Collision suffixes `.001`, `.002`, ... reuse the base name if it is free (verified).
16. **Undo is disabled in background mode** (verified poll message). It is not required for frap, but the "leave state unchanged on error" requirement means frap needs its own transactional rollback per command.
17. **glTF exporter defaults that surprise agents**:
    - `export_cameras=False`, `export_lights=False`, `export_apply=False` (modifiers not applied), `use_selection=False`, `export_yup=True`;
    - `export_vertex_color='MATERIAL'` (colour attributes are exported only when the material uses them);
    - `export_format` is forced onto the file extension;
    - **KHR_texture_basisu is not supported by Blender** (a frap-only extension; it must be namespaced);
    - Draco and meshopt come from native libraries, and there is no mature pure-Rust Draco encoder (risk).
18. **Render**: the factory engine is `BLENDER_EEVEE`, the factory view transform is **`AgX`** (frap must implement AgX to look like Blender), output defaults are `PNG RGBA 8-bit` at 1920x1080 100% with `filepath='/tmp/'`. The output-path rules (extension, `#` padding, `0001`) are in 4.17. The result is the image `Render Result`.
19. **Modes**: `Object.mode` uses `OBJECT/EDIT/POSE/...`, while `context.mode` uses `OBJECT/EDIT_MESH/EDIT_ARMATURE/POSE/...` (full lists in section 2). Entering EDIT is multi-object: every selected object of the active object's type enters it.

## 2. Context model frap must support

### 2.1 `bpy.context` members

All members below exist in Blender 5.2. The values were observed in `-b` without an area ("bg") and with a VIEW_3D override ("v3d"). frap should expose them read-only (`bpy.context.object` etc.) and allow assignment only where Blender allows it. In Blender, `context` is read-only: you write through `view_layer.objects.active` or `object.select_set()`.

| Member | Type | frap | Semantics (Blender 5.2) |
|---|---|---|---|
| `object` | Object or None | P0 | Active object in the 3D View / object context. Equals `view_layer.objects.active` in all states tested (bg and v3d). |
| `active_object` | Object or None | P0 | `view_layer.objects.active`. It **can be unselected** (verified). |
| `selected_objects` | list[Object] | P0 | Selected objects of the view layer, in **base order** (not selection order; verified). |
| `selected_editable_objects` | list[Object] | P1 | Selected and not library-linked. Most object operators iterate this list. |
| `visible_objects` | list[Object] | P1 | `visible_get()` true in the view layer. |
| `selectable_objects` | list[Object] | P2 | Visible and `hide_select == False`. |
| `editable_objects` | list[Object] | P2 | |
| `edit_object` | Object or None | P0 | Active object when in an edit mode, else None (verified). |
| `objects_in_mode` | list[Object] | P0 | Every object in the current non-object mode (multi-object editing; verified 18 meshes entered EDIT together). |
| `objects_in_mode_unique_data` | list[Object] | P1 | Same, de-duplicated by shared data. |
| `mode` | enum str | P0 | `OBJECT`, `EDIT_MESH`, `EDIT_CURVE`, `EDIT_CURVES`, `EDIT_SURFACE`, `EDIT_TEXT`, `EDIT_ARMATURE`, `EDIT_METABALL`, `EDIT_LATTICE`, `EDIT_GREASE_PENCIL`, `EDIT_POINTCLOUD`, `POSE`, `SCULPT`, `PAINT_WEIGHT`, `PAINT_VERTEX`, `PAINT_TEXTURE`, `PARTICLE`, `PAINT_GPENCIL`, `EDIT_GPENCIL`, `SCULPT_GPENCIL`, `WEIGHT_GPENCIL`, `VERTEX_GPENCIL`, `SCULPT_CURVES`, `PAINT_GREASE_PENCIL`, `SCULPT_GREASE_PENCIL`, `WEIGHT_GREASE_PENCIL`, `VERTEX_GREASE_PENCIL` (verified list). frap: `OBJECT`, `EDIT_MESH`, `EDIT_ARMATURE`, `POSE` (later `PAINT_WEIGHT`). |
| `scene` | Scene | P0 | Active scene of the window. |
| `view_layer` | ViewLayer | P0 | Active view layer (`ViewLayer` in the factory file). Holds `objects.active`, the selection (per-base flags) and `active_layer_collection`. |
| `collection` | Collection | P0 | Active collection (`view_layer.active_layer_collection.collection`). New objects are linked here (factory: `Collection`). |
| `layer_collection` | LayerCollection | P1 | Active layer collection. |
| `tool_settings` | ToolSettings | P0 | `scene.tool_settings` (see 2.3). |
| `active_bone` | EditBone/Bone or None | P1 | Active bone (armature edit or pose mode). |
| `active_pose_bone` | PoseBone or None | P1 | |
| `selected_bones`, `selected_editable_bones`, `editable_bones`, `visible_bones` | list[EditBone] | P1 | Armature edit mode. |
| `selected_pose_bones`, `selected_pose_bones_from_active_object`, `visible_pose_bones` | list[PoseBone] | P1 | Pose mode. |
| `pose_object` | Object or None | P1 | Armature in pose mode. |
| `weight_paint_object` | Object or None | P2 | If weight paint is added. |
| `engine` | str | P2 | `scene.render.engine` (`BLENDER_EEVEE` factory). |
| `blend_data` | BlendData | P0 | Same object as `bpy.data`. |
| `preferences` | Preferences | P2 | Read-only subset: `edit.use_duplicate_*`, `edit.keyframe_new_interpolation_type`, `edit.keyframe_new_handle_type`, `edit.key_insert_channels`. |
| `window`, `window_manager`, `screen`, `workspace`, `area`, `region`, `region_data`, `space_data` | UI | OUT | `area`/`region`/`space_data` are None in `-b`, and `SpaceView3D`/`RegionView3D` with an override. frap can return None, or a stub for the virtual view if needed (e.g. `space_data.shading` for Workbench). |
| `active_action`, `active_editable_fcurve`, `selected_editable_fcurves`, `selected_editable_keyframes`, `active_nla_track`, `active_nla_strip`, `selected_nla_strips` | anim editors | OUT | Only valid in animation editors. Use the data API instead. |
| `material`, `mesh`, `armature`, `camera`, `light`, `world`, `texture`, `modifier`, `bone`, `pose_bone` | Properties-editor pointers | OUT | Only in the Properties editor or with a UI context pointer. Not available to scripts in `-b` (which is why `material.new` doesn't assign). |

### 2.2 Scene-level state

| Path | Type / default (factory, verified) | Notes |
|---|---|---|
| `scene.cursor.location` | FLOAT[3] XYZ, (0,0,0) | Used by primitives, `*_add`, `origin_set(ORIGIN_CURSOR)`, pivot `CURSOR`. |
| `scene.cursor.rotation_mode` | ENUM, `XYZ` | Items: `QUATERNION`, `AXIS_ANGLE`, `XYZ`, `XZY`, `YXZ`, `YZX`, `ZXY`, `ZYX`. |
| `scene.cursor.rotation_euler` / `rotation_quaternion` / `rotation_axis_angle` | FLOAT[3] / [4] / [4] | Used by `align='CURSOR'`. |
| `scene.cursor.matrix` | FLOAT[16] (4x4) | Read/write. |
| `view_layer.objects.active` | Object (writable) | How scripts set the active object. |
| `Object.select_set(state)`, `select_get()`, `hide_set(state)`, `hide_get()`, `visible_get()` | methods | Selection and hiding are **per view layer** (Base flags). There is no `Object.select` property. |
| `scene.camera` | Object or None | Required by `render.render`. |
| `scene.frame_start / frame_end / frame_current / frame_step` | 1 / 250 / 1 / 1 | |
| `scene.render.fps / fps_base` | 24 / 1.0 | glTF time = frame / (fps / fps_base). |
| `scene.render.engine` | `BLENDER_EEVEE` | Plus `BLENDER_WORKBENCH`, `CYCLES` (dynamic enum, registered engines). |
| `scene.render.resolution_x / _y / resolution_percentage` | 1920 / 1080 / 100 | |
| `scene.render.filepath` | `/tmp/` | |
| `scene.render.image_settings.file_format / color_mode / color_depth` | `PNG` / `RGBA` / `8` | |
| `scene.render.film_transparent` | False | |
| `scene.view_settings.view_transform` | `AgX` | `display_settings.display_device` = `sRGB`. |
| `scene.unit_settings.system / scale_length / length_unit` | `METRIC` / 1.0 / `METERS` | |
| `view_layer.active_layer_collection` | LayerCollection | Changes `context.collection`. |

### 2.3 `scene.tool_settings` members used by the catalogued operators

| Property | Default (verified) | Used by |
|---|---|---|
| `mesh_select_mode` | BOOLEAN[3] `(True, False, False)` = vertex mode | Edit-mode selection semantics (vert/edge/face flush). Set by `mesh.select_mode` or by direct assignment. |
| `transform_pivot_point` | `MEDIAN_POINT` (items `BOUNDING_BOX_CENTER`, `CURSOR`, `INDIVIDUAL_ORIGINS`, `MEDIAN_POINT`, `ACTIVE_ELEMENT`) | `transform.rotate/resize/mirror`, `view3d.snap_cursor_to_selected`. |
| `use_proportional_edit`, `proportional_edit_falloff`, `proportional_size` | False / `SMOOTH` / 1.0 | Default values of the transform props. |
| `use_snap` | False | Ignored (non-interactive). |
| `vertex_group_weight` | 1.0 | `object.vertex_group_assign`. |
| `double_threshold` | 0.001 (verified, not 0.0001) | `mesh.remove_doubles` / automerge. |
| `use_mesh_automerge` | False | Edit-mode transforms. |
| `use_keyframe_insert_auto` | False | Auto-keying. frap: keep False. |
| `transform_pivot_point`, `use_transform_data_origin`, `use_transform_skip_children` | | Object-mode transform options (P2). |

### 2.4 Factory startup scene (`wm.read_homefile()` / `wm.read_factory_settings()`), verified

| Item | Value |
|---|---|
| Objects | `Camera` (CAMERA, data `Camera`, loc (7.3589, -6.9258, 4.9583), rot (1.1093, 0, 0.8149) XYZ), `Cube` (MESH, data `Cube`, 8 verts, material `Material`), `Light` (LIGHT POINT 1000 W, loc (4.0762, 1.0055, 5.9039), rot (0.6503, 0.0552, 1.8664)) |
| Selection | active = `Cube`, selected = [`Cube`], mode OBJECT |
| Collections | `Collection` (child of the scene master collection) holds all three objects. The master collection holds no objects directly. |
| Other data | materials `Material`, `Dots Stroke` (grease pencil); world `World` (nodes `World Output`, `Background`); scene `Scene`; view layer `ViewLayer`; screens `Animation`, `Compositing`, `Geometry Nodes`, `Layout`, `Modeling`, ... |
| Scene | frames 1..250 @ 24 fps, engine `BLENDER_EEVEE`, 1920x1080 100%, filepath `/tmp/`, METRIC 1.0, cursor at origin, `scene.camera` = `Camera` |

Decision for the architect: an LLM agent probably wants **`use_empty=True`** (an empty scene) as frap's initial state. The golden harness then starts both tools with `read_homefile(use_empty=True)` or with the factory scene, and must be explicit about which.


## 3. Operator summary (all catalogued operators)

Poll columns: **A** object mode, mesh `Cube` active+selected | **B** object mode, nothing selected, no active | **C** mesh edit mode | **D** armature pose mode | **E** armature edit mode. `Y` = poll passes in plain `blender -b`, `V` = passes only with a VIEW_3D area/region context override, `N` = fails (see notes: some fail only because required data, e.g. vertex groups, is absent).

**4.1 Object modes**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.mode_set` | Set Object Mode | P0 | `Y N Y Y Y` | mode, toggle |
| `object.mode_set_with_submode` | Set Object Mode with Sub-mode | P2 | `Y N Y Y Y` | mode, toggle |
| `object.editmode_toggle` | Edit Mode | P1 | `Y N Y Y Y` | - |
| `object.posemode_toggle` | Toggle Pose Mode | P1 | `Y N Y Y Y` | - |

**4.2 Object selection**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.select_all` | (De)select All | P0 | `Y Y N N N` | - |
| `object.select_by_type` | Select by Type | P1 | `Y Y N N N` | extend, type |
| `object.select_pattern` | Select Pattern | P1 | `Y Y N Y Y` | pattern, case_sensitive, extend |
| `object.select_grouped` | Select Grouped | P2 | `N N N N N` | extend, type |
| `object.select_hierarchy` | Select Hierarchy | P2 | `Y N Y Y Y` | direction, extend |
| `object.select_linked` | Select Linked | P2 | `Y Y N N N` | extend, type |
| `object.select_random` | Select Random | P2 | `Y Y N N N` | ratio, seed, action |
| `object.select_camera` | Select Camera | P2 | `Y Y Y Y Y` | extend |
| `object.select_mirror` | Select Mirror | P2 | `Y Y N N N` | extend |
| `object.select_more` | Select More | P2 | `Y Y N N N` | - |
| `object.select_less` | Select Less | P2 | `Y Y N N N` | - |
| `object.select_same_collection` | Select Same Collection | P2 | `Y Y N N N` | collection |

**4.3 Adding objects (non-mesh)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.camera_add` | Add Camera | P0 | `Y Y N N N` | location, rotation |
| `object.light_add` | Add Light | P0 | `Y Y N N N` | type, radius, align, location, rotation |
| `object.empty_add` | Add Empty | P0 | `Y Y N N N` | type, radius, align, location, rotation |
| `object.armature_add` | Add Armature | P0 | `Y Y N N N` | radius, align, location, rotation |
| `object.add` | Add Object | P2 | `Y Y N N N` | radius, type, align, location, rotation |
| `object.add_named` | Add Object | P2 | `Y Y N N N` | linked |
| `object.empty_image_add` | Add Empty Image/Drop Image to Empty | OUT | `V V V V V` | align, location, rotation, background |
| `object.collection_instance_add` | Add Collection Instance | P2 | `Y Y N N N` | name, collection, align, location, rotation |
| `object.data_instance_add` | Add Object Data Instance | OUT | `Y Y N N N` | type, align, location, rotation |
| `object.text_add` | Add Text | OUT | `Y Y N N N` | radius, align, location, rotation |

**4.4 Delete / duplicate / join**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.delete` | Delete | P0 | `Y Y N N N` | - |
| `object.duplicate` | Duplicate Objects | P0 | `Y Y N N N` | linked |
| `object.duplicate_move` | Duplicate Objects | P0 | `Y Y N N N` | OBJECT_OT_duplicate, TRANSFORM_OT_translate |
| `object.duplicate_move_linked` | Duplicate Linked | P1 | `Y Y N N N` | OBJECT_OT_duplicate, TRANSFORM_OT_translate |
| `object.duplicates_make_real` | Make Instances Real | P2 | `Y Y N N N` | use_base_parent, use_hierarchy |
| `object.join` | Join | P0 | `Y N Y Y Y` | - |
| `object.join_shapes` | Join as Shapes | P2 | `Y N N N N` | use_mirror |
| `object.convert` | Convert To | P1 | `Y Y N N N` | target, keep_original, merge_customdata, thickness, faces, offset |

**4.5 Parenting**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.parent_set` | Make Parent | P0 | `Y N Y Y Y` | type, xmirror, keep_transform |
| `object.parent_clear` | Clear Parent | P0 | `Y Y Y Y Y` | type |
| `object.parent_no_inverse_set` | Make Parent without Inverse | P1 | `Y N Y Y Y` | keep_transform |
| `object.parent_inverse_apply` | Apply Parent Inverse | P2 | `Y Y N N N` | - |
| `object.vertex_parent_set` | Make Vertex Parent | OUT | `N N Y N N` | - |

**4.6 Origin and object transforms**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.origin_set` | Set Origin | P0 | `Y Y Y Y Y` | type, center |
| `object.origin_clear` | Clear Origin | P2 | `Y Y Y Y Y` | - |
| `object.transform_apply` | Apply Object Transform | P0 | `Y Y N N N` | location, rotation, scale, properties, corrective_flip_normals |
| `object.visual_transform_apply` | Apply Visual Transform | P2 | `Y Y Y Y Y` | - |
| `object.transforms_to_deltas` | Transforms to Deltas | P2 | `Y Y Y Y Y` | mode, reset_values |
| `object.anim_transforms_to_deltas` | Animated Transforms to Deltas | OUT | `Y Y Y Y Y` | - |
| `object.location_clear` | Clear Location | P0 | `Y Y Y Y Y` | clear_delta |
| `object.rotation_clear` | Clear Rotation | P0 | `Y Y Y Y Y` | clear_delta |
| `object.scale_clear` | Clear Scale | P0 | `Y Y Y Y Y` | clear_delta |
| `object.randomize_transform` | Randomize Transform | P2 | `Y Y N N N` | random_seed, use_delta, use_loc, loc, use_rot, rot, use_scale, scale_even, scale |
| `object.align` | Align Objects | P2 | `Y Y N N N` | bb_quality, align_mode, relative_to, align_axis |

**4.7 Shading**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.shade_smooth` | Shade Smooth | P0 | `Y Y N Y N` | keep_sharp_edges |
| `object.shade_flat` | Shade Flat | P0 | `Y Y N Y N` | keep_sharp_edges |
| `object.shade_smooth_by_angle` | Shade Smooth by Angle | P0 | `Y Y N Y N` | angle, keep_sharp_edges |
| `object.shade_auto_smooth` | Shade Auto Smooth | P1 | `Y Y N Y N` | use_auto_smooth, angle |

**4.8 Modifier stack**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.modifier_add` | Add Modifier | P0 | `Y N Y Y Y` | type |
| `object.modifier_remove` | Remove Modifier | P0 | `Y N Y Y Y` | - |
| `object.modifier_apply` | Apply Modifier | P0 | `Y N N Y N` | merge_customdata |
| `object.modifier_apply_as_shapekey` | Apply Modifier as Shape Key | P2 | `Y N N Y N` | keep_modifier |
| `object.modifier_copy` | Copy Modifier | P1 | `Y N Y Y Y` | - |
| `object.modifier_copy_to_selected` | Copy Modifier to Selected | P2 | `N N N N N` | - |
| `object.modifier_move_up` | Move Up Modifier | P0 | `Y N Y Y Y` | - |
| `object.modifier_move_down` | Move Down Modifier | P0 | `Y N Y Y Y` | - |
| `object.modifier_move_to_index` | Move Active Modifier to Index | P0 | `Y N Y Y Y` | index |
| `object.modifier_set_active` | Set Active Modifier | P1 | `Y N Y Y Y` | - |
| `object.modifiers_clear` | Clear Object Modifiers | P1 | `Y N Y N N` | - |
| `object.modifiers_copy_to_selected` | Copy Modifiers to Selected Objects | P2 | `N N N N N` | - |
| `object.subdivision_set` | Subdivision Set | P1 | `Y Y Y Y Y` | level, relative |

**4.9 Material slots**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.material_slot_add` | Add Material Slot | P0 | `Y N Y N N` | - |
| `object.material_slot_remove` | Remove Material Slot | P0 | `Y N N N N` | - |
| `object.material_slot_remove_all` | Remove All Materials | P2 | `Y N N N N` | - |
| `object.material_slot_remove_unused` | Remove Unused Slots | P1 | `Y N N N N` | - |
| `object.material_slot_assign` | Assign Material Slot | P0 | `Y N Y N N` | - |
| `object.material_slot_select` | Select Material Slot | P1 | `Y Y Y Y Y` | - |
| `object.material_slot_deselect` | Deselect Material Slot | P1 | `Y Y Y Y Y` | - |
| `object.material_slot_copy` | Copy Material to Selected | P2 | `Y N Y N N` | - |
| `object.material_slot_move` | Move Material | P2 | `Y N Y N N` | direction |

**4.10 Vertex groups and weights**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.vertex_group_add` | Add Vertex Group | P0 | `Y N Y N N` | - |
| `object.vertex_group_remove` | Remove Vertex Group | P0 | `N N N N N` | all, all_unlocked |
| `object.vertex_group_assign` | Assign to Vertex Group | P0 | `N N Y N N` | - |
| `object.vertex_group_assign_new` | Assign to New Group | P1 | `N N Y N N` | - |
| `object.vertex_group_remove_from` | Remove from Vertex Group | P0 | `N N Y N N` | use_all_groups, use_all_verts |
| `object.vertex_group_select` | Select Vertex Group | P1 | `N N Y N N` | - |
| `object.vertex_group_deselect` | Deselect Vertex Group | P1 | `N N Y N N` | - |
| `object.vertex_group_set_active` | Set Active Vertex Group | P1 | `N N N N N` | group |
| `object.vertex_group_copy` | Duplicate Vertex Group | P2 | `N N N N N` | - |
| `object.vertex_group_copy_to_selected` | Copy Vertex Group to Selected | P2 | `N N N N N` | - |
| `object.vertex_group_normalize` | Normalize Vertex Group | P1 | `N N N N N` | - |
| `object.vertex_group_normalize_all` | Normalize All Vertex Groups | P0 | `N N N N N` | group_select_mode, lock_active |
| `object.vertex_group_invert` | Invert Vertex Group | P2 | `N N N N N` | group_select_mode, auto_assign, auto_remove |
| `object.vertex_group_clean` | Clean Vertex Group Weights | P1 | `N N N N N` | group_select_mode, limit, keep_single |
| `object.vertex_group_quantize` | Quantize Vertex Weights | P2 | `N N N N N` | group_select_mode, steps |
| `object.vertex_group_levels` | Vertex Group Levels | P2 | `N N N N N` | group_select_mode, offset, gain |
| `object.vertex_group_limit_total` | Limit Number of Weights per Vertex | P0 | `N N N N N` | group_select_mode, limit |
| `object.vertex_group_lock` | Change the Lock On Vertex Groups | P2 | `N N N N N` | action, mask |
| `object.vertex_group_mirror` | Mirror Vertex Group | P2 | `N N N N N` | mirror_weights, flip_group_names, all_groups, use_topology |
| `object.vertex_group_move` | Move Vertex Group | P2 | `N N N N N` | direction |
| `object.vertex_group_smooth` | Smooth Vertex Weights | P2 | `N N Y N N` | group_select_mode, factor, repeat, expand |
| `object.vertex_group_sort` | Sort Vertex Groups | P2 | `N N N N N` | sort_type |
| `object.vertex_weight_copy` | Copy Active | P2 | `N N Y N N` | - |
| `object.vertex_weight_delete` | Delete Weight | P2 | `N N Y N N` | - |
| `object.vertex_weight_normalize_active_vertex` | Normalize Active | P2 | `N N Y N N` | - |
| `object.vertex_weight_paste` | Paste Weight to Selected | P2 | `N N Y N N` | - |
| `object.vertex_weight_set_active` | Set Active Group | P2 | `N N Y N N` | - |

**4.11 Shape keys**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.shape_key_add` | Add Shape Key | P0 | `Y N N Y N` | from_mix |
| `object.shape_key_remove` | Remove Shape Key | P0 | `N N N N N` | all, apply_mix |
| `object.shape_key_clear` | Clear Shape Keys | P1 | `Y N Y Y Y` | - |
| `object.shape_key_retime` | Re-Time Shape Keys | OUT | `Y N Y Y Y` | - |
| `object.shape_key_mirror` | Mirror Shape Key | P2 | `N N N N N` | use_topology |
| `object.shape_key_move` | Move Shape Key | P1 | `N N N N N` | type |
| `object.shape_key_copy` | Duplicate Shape Key | P2 | `N N N N N` | - |
| `object.shape_key_lock` | Change the Lock On Shape Keys | P2 | `N N N N N` | action |
| `object.shape_key_make_basis` | Make Shape Key the Basis Key | P2 | `N N N N N` | - |
| `object.shape_key_apply_to_basis` | Apply to Basis Key | P2 | `N N N N N` | - |
| `object.shape_key_transfer` | Transfer Shape Key | P2 | `Y N N N N` | mode, use_clamp |

**4.12 Collections, visibility, links, constraints**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `object.hide_view_set` | Hide Objects | P1 | `V V V V V` | - |
| `object.hide_view_clear` | Show Hidden Objects | P1 | `V V V V V` | select |
| `object.hide_render_clear_all` | Clear All Restrict Render | P2 | `Y Y Y Y Y` | - |
| `object.move_to_collection` | Move to Collection | P1 | `Y Y N N N` | new_collection_name |
| `object.link_to_collection` | Link to Collection | P2 | `Y Y N N N` | new_collection_name |
| `object.collection_add` | Add to Collection | P2 | `Y Y N N N` | - |
| `object.collection_remove` | Remove Collection | P2 | `Y Y N N N` | - |
| `object.collection_link` | Link to Collection | P2 | `Y Y N N N` | collection |
| `object.collection_unlink` | Unlink Collection | P2 | `Y Y N N N` | - |
| `object.make_single_user` | Make Single User | P1 | `Y Y N N N` | type, object, obdata, material, animation, obdata_animation |
| `object.make_links_data` | Link/Transfer Data | P1 | `Y N N N N` | type |
| `object.make_local` | Make Local | OUT | `Y Y N N N` | type |
| `object.track_set` | Make Track | P2 | `Y Y N N N` | type |
| `object.track_clear` | Clear Track | P2 | `Y Y N N N` | type |
| `object.constraint_add` | Add Constraint | P2 | `Y N Y Y Y` | type |
| `collection.create` | Create New Collection | P2 | `Y Y N N N` | name |
| `collection.objects_add_active` | Add Selected to Active Object's Collection | P2 | `Y Y N N N` | collection |
| `collection.objects_remove` | Remove from Collection | P2 | `Y Y N N N` | collection |
| `collection.objects_remove_all` | Remove from All Collections | P2 | `Y Y N N N` | - |
| `collection.objects_remove_active` | Remove Selected from Active Collection | P2 | `Y Y N N N` | collection |

**4.13 Mesh primitives (`mesh.primitive_*_add`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `mesh.primitive_plane_add` | Add Plane | P0 | `Y Y Y Y Y` | size, calc_uvs, align, location, rotation |
| `mesh.primitive_cube_add` | Add Cube | P0 | `Y Y Y Y Y` | size, calc_uvs, align, location, rotation |
| `mesh.primitive_circle_add` | Add Circle | P0 | `Y Y Y Y Y` | vertices, radius, fill_type, calc_uvs, align, location, rotation |
| `mesh.primitive_uv_sphere_add` | Add UV Sphere | P0 | `Y Y Y Y Y` | segments, ring_count, radius, calc_uvs, align, location, rotation |
| `mesh.primitive_ico_sphere_add` | Add Ico Sphere | P0 | `Y Y Y Y Y` | subdivisions, radius, calc_uvs, align, location, rotation |
| `mesh.primitive_cylinder_add` | Add Cylinder | P0 | `Y Y Y Y Y` | vertices, radius, depth, end_fill_type, calc_uvs, align, location, rotation |
| `mesh.primitive_cone_add` | Add Cone | P0 | `Y Y Y Y Y` | vertices, radius1, radius2, depth, end_fill_type, calc_uvs, align, location, rotation |
| `mesh.primitive_torus_add` | Add Torus | P0 | `Y Y Y Y Y` | align, location, rotation, major_segments, minor_segments, mode, major_radius, minor_radius, abso_major_rad, abso_minor_rad, generate_uvs |
| `mesh.primitive_grid_add` | Add Grid | P0 | `Y Y Y Y Y` | x_subdivisions, y_subdivisions, size, calc_uvs, align, location, rotation |
| `mesh.primitive_monkey_add` | Add Monkey | P0 | `Y Y Y Y Y` | size, calc_uvs, align, location, rotation |

**4.14 Transform (`transform.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `transform.translate` | Move | P0 | `Y Y Y Y Y` | value, orient_type, constraint_axis, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected, use_snap_project |
| `transform.rotate` | Rotate | P0 | `Y Y Y Y Y` | value, orient_axis, orient_type, constraint_axis, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected, use_snap_project |
| `transform.resize` | Resize | P0 | `Y Y Y Y Y` | value, orient_type, constraint_axis, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected, use_snap_project |
| `transform.mirror` | Mirror | P1 | `Y Y Y Y Y` | orient_type, constraint_axis |
| `transform.transform` | Transform | P1 | `Y Y Y Y Y` | value, orient_axis, orient_type, constraint_axis, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected, use_snap_project |
| `transform.shear` | Shear | P2 | `V V V V V` | angle, orient_axis, orient_axis_ortho, orient_type, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.tosphere` | To Sphere | P1 | `Y Y Y Y Y` | value, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.bend` | Bend | P2 | `V V V V V` | value, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.push_pull` | Push/Pull | P2 | `Y Y Y Y Y` | value, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.shrink_fatten` | Shrink/Fatten | P1 | `N N Y N N` | value, use_even_offset, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.trackball` | Trackball | OUT | `Y Y Y Y Y` | value, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.edge_slide` | Edge Slide | P1 | `N N Y N N` | value, use_even, flipped, use_clamp, mirror, use_snap_project, correct_uv |
| `transform.vert_slide` | Vertex Slide | P2 | `N N Y N N` | value, use_even, flipped, use_clamp, mirror, use_snap_project, correct_uv |
| `transform.edge_crease` | Edge Crease | P1 | `N N Y N N` | value |
| `transform.vert_crease` | Vertex Crease | P2 | `N N Y N N` | value |
| `transform.edge_bevelweight` | Edge Bevel Weight | P1 | `N N Y N N` | value |
| `transform.vertex_random` | Randomize | P2 | `N N Y N Y` | offset, uniform, normal, seed |
| `transform.vertex_warp` | Warp | OUT | `N N Y N Y` | warp_angle, offset_angle, min, max |
| `transform.rotate_normal` | Rotate Normals | P2 | `N N Y N N` | value, orient_axis, orient_type, constraint_axis, mirror |
| `transform.skin_resize` | Skin Resize | OUT | `N N Y N N` | value, orient_type, constraint_axis, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected, use_snap_project |
| `transform.tilt` | Tilt | OUT | `N N Y N Y` | value, mirror, use_proportional_edit, proportional_edit_falloff, proportional_size, use_proportional_connected, use_proportional_projected |
| `transform.create_orientation` | Create Orientation | P2 | `V V V V V` | name, use_view, use, overwrite |
| `transform.select_orientation` | Select Orientation | P2 | `V V V V V` | orientation |

**4.15 3D cursor and snapping (`view3d.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `view3d.snap_cursor_to_active` | Snap Cursor to Active | P0 | `V V V V V` | - |
| `view3d.snap_cursor_to_center` | Snap Cursor to World Origin | P0 | `V V V V V` | - |
| `view3d.snap_cursor_to_grid` | Snap Cursor to Grid | P1 | `V V V V V` | - |
| `view3d.snap_cursor_to_selected` | Snap Cursor to Selected | P0 | `V V V V V` | - |
| `view3d.snap_selected_to_active` | Snap Selection to Active | P1 | `V V V V V` | - |
| `view3d.snap_selected_to_cursor` | Snap Selection to Cursor | P0 | `V V V V V` | use_offset, use_rotation |
| `view3d.snap_selected_to_grid` | Snap Selection to Grid | P2 | `V V V V V` | - |
| `view3d.camera_to_view_selected` | Camera Fit Frame to Selected | P0 | `Y Y Y Y Y` | - |
| `view3d.object_as_camera` | Set Active Object as Camera | P1 | `V V V V V` | - |

**4.16 Animation operators (`anim.*`, `screen.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `anim.keyframe_insert` | Insert Keyframe | P0 | `V V V V V` | - |
| `anim.keyframe_insert_by_name` | Insert Keyframe (by name) | P1 | `V V V V V` | - |
| `anim.keyframe_insert_menu` | Insert Keyframe Menu | OUT | `V V V V V` | - |
| `anim.keyframe_delete` | Delete Keying-Set Keyframe | P1 | `V V V V V` | - |
| `anim.keyframe_delete_by_name` | Delete Keying-Set Keyframe (by name) | P2 | `V V V V V` | - |
| `anim.keyframe_delete_v3d` | Delete Keyframe | P1 | `V V V V V` | - |
| `anim.keyframe_clear_v3d` | Remove Animation | P1 | `V V V V V` | - |
| `anim.keying_set_active_set` | Set Active Keying Set | P2 | `V V V V V` | type |
| `anim.change_frame` | Change Frame | OUT | `N N N N N` | frame, snap, seq_solo_preview, pass_through_on_strip_handles |
| `anim.start_frame_set` | Set Start Frame | OUT | `N N N N N` | - |
| `anim.end_frame_set` | Set End Frame | OUT | `N N N N N` | - |
| `anim.channels_bake` | Bake Channels | P2 | `N N N N N` | use_scene_range, range, step, remove_outside_range, interpolation_type, bake_modifiers |
| `screen.frame_jump` | Jump to Endpoint | P1 | `Y Y Y Y Y` | end |
| `screen.frame_offset` | Frame Offset | P1 | `Y Y Y Y Y` | delta |
| `screen.keyframe_jump` | Jump to Keyframe | P2 | `Y Y Y Y Y` | next |

**4.17 Render (`render.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `render.render` | Render | P0 | `Y Y Y Y Y` | animation, write_still, use_viewport, use_sequencer_scene, layer, scene, frame_start, frame_end |
| `render.opengl` | Viewport Render | P2 | `Y Y Y Y Y` | animation, render_keyed_only, sequencer, write_still, view_context |

**4.18 Files and session (`wm.*`, `scene.*`, `ed.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `wm.open_mainfile` | Open | P0 | `Y Y Y Y Y` | filepath, load_ui, use_scripts, display_file_selector |
| `wm.save_as_mainfile` | Save As | P0 | `Y Y Y Y Y` | filepath, compress, relative_remap, copy |
| `wm.save_mainfile` | Save Blender File | P0 | `Y Y Y Y Y` | filepath, compress, relative_remap |
| `wm.read_homefile` | Reload Start-Up File | P0 | `Y Y Y Y Y` | - |
| `wm.read_factory_settings` | Load Factory Settings | P1 | `Y Y Y Y Y` | - |
| `wm.revert_mainfile` | Revert | P2 | `N N N N N` | use_scripts |
| `scene.new` | New Scene | P2 | `Y Y Y Y Y` | type |
| `scene.delete` | Delete Scene | P2 | `N N N N N` | - |
| `scene.view_layer_add` | Add View Layer | OUT | `Y Y Y Y Y` | type |
| `scene.view_layer_remove` | Remove View Layer | OUT | `N N N N N` | - |
| `ed.undo` | Undo | OUT | `N N N N N` | - |
| `ed.redo` | Redo | OUT | `N N N N N` | - |
| `ed.undo_push` | Undo Push | OUT | `Y Y Y Y Y` | message |
| `ed.undo_history` | Undo History | OUT | `N N N N N` | item |

**4.19 Data-block creation (`material.*`, `image.*`, `world.*`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `material.new` | New Material | P0 | `Y N Y N N` | - |
| `material.copy` | Copy Material | P2 | `Y N Y N N` | - |
| `material.paste` | Paste Material | P2 | `Y N Y N N` | - |
| `image.new` | New Image | P0 | `Y Y Y Y Y` | name, width, height, alpha, generated_type, float, tiled |
| `image.open` | Open Image | P0 | `Y Y Y Y Y` | filepath, directory, relative_path, use_sequence_detection, use_udim_detecting |
| `image.save` | Save Image | P2 | `N N N N N` | - |
| `image.save_as` | Save As Image | P2 | `N N N N N` | save_as_render, copy, filepath, relative_path |
| `image.pack` | Pack Image | P1 | `N N N N N` | - |
| `image.unpack` | Unpack Image | P2 | `N N N N N` | method, id |
| `image.reload` | Reload Image | P2 | `Y Y Y Y Y` | - |
| `world.new` | New World | P1 | `Y Y Y Y Y` | - |

**4.20 glTF import (`import_scene.gltf`)**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `import_scene.gltf` | Import glTF 2.0 | P0 | `Y Y Y Y Y` | export_import_convert_lighting_mode, files, loglevel, import_pack_images, merge_vertices, import_shading, bone_heuristic, disable_bone_shape, bone_shape_scale_factor, guess_original_bind_pose, import_webp_texture, import_unused_materials, import_select_created_objects, import_scene_extras, ... (17 total) |

**4.21 glTF export**

| bl_idname | Label | Priority | Poll A B C D E | Props (visible) |
|---|---|---|---|---|
| `export_scene.gltf` | Export glTF 2.0 | P0 | `Y Y Y Y Y` | filepath, export_import_convert_lighting_mode, gltf_export_id, export_use_gltfpack, export_gltfpack_tc, export_gltfpack_tq, export_gltfpack_si, export_gltfpack_sa, export_gltfpack_slb, export_gltfpack_vp, export_gltfpack_vt, export_gltfpack_vn, export_gltfpack_vc, export_gltfpack_vpi, ... (108 total) |


### Factory "Duplicate Data" preferences (verified, `preferences.edit.use_duplicate_*`)

| Data | Deep-copied by `object.duplicate`? |
|---|---|
| mesh, curve, surface, text, metaball, armature, lattice, light, camera, speaker, lightprobe, grease_pencil, curves, pointcloud, **action** | yes |
| **material**, node_tree, particle, volume | **no (shared)** |


## 4. Operator details

### 4.1 Object modes

#### `object.mode_set` - Set Object Mode

*Sets the object interaction mode* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: -

`bpy.ops.object.mode_set(mode='OBJECT', toggle=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mode` | ENUM | `'OBJECT'` | `OBJECT`, `EDIT`, `POSE`, `SCULPT`, `VERTEX_PAINT`, `WEIGHT_PAINT`, `TEXTURE_PAINT`, `PARTICLE_EDIT`, `EDIT_GPENCIL`, `SCULPT_GREASE_PENCIL`, `PAINT_GREASE_PENCIL`, `WEIGHT_GREASE_PENCIL`, `VERTEX_GREASE_PENCIL`, `SCULPT_CURVES` |  |
| `toggle` | BOOLEAN | `False` |  |  |

- `mode` is a **context-dependent (dynamic) enum**: items are filtered by the active object's type. Camera/Light/Empty -> only `OBJECT`; Mesh -> `OBJECT, EDIT, SCULPT, VERTEX_PAINT, WEIGHT_PAINT, TEXTURE_PAINT` (+`PARTICLE_EDIT` with particles); Armature -> `OBJECT, EDIT, POSE`. Asking for an invalid mode raises `TypeError: Converting py args to operator properties: enum "EDIT" not found in ('OBJECT')` (verified). frap scope: `OBJECT`, `EDIT` (mesh, armature), `POSE` (armature); reject others with the same style of error.
- Poll: needs an active, editable object (fails when `view_layer.objects.active is None`).
- `EDIT` enters **multi-object edit**: every *selected* object of the same type as the active one enters edit mode (`context.objects_in_mode`); selected objects of other types stay in object mode but remain selected (verified).
- `toggle=True`: if already in `mode`, return to `OBJECT`.
- `Object.mode` uses the short names (`'EDIT'`), `context.mode` uses the long names (`'EDIT_MESH'`, `'EDIT_ARMATURE'`, `'POSE'`, `'OBJECT'`).
- Leaving edit mode writes the BMesh/edit-bones back to the datablock (`Mesh`, `Armature`). Property reads of `mesh.vertices` while in edit mode return the *stale* pre-edit data in Blender — frap may choose to always sync but should document it.
- Calling `mode_set(mode='OBJECT')` while already in object mode is a no-op returning FINISHED (believed, not verified).

#### `object.mode_set_with_submode` - Set Object Mode with Sub-mode

*Sets the object interaction mode* | priority **P2** | poll A-E: `Y N Y Y Y` | bl_options: -

`bpy.ops.object.mode_set_with_submode(mode='OBJECT', toggle=False, mesh_select_mode={})`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mode` | ENUM | `'OBJECT'` | `OBJECT`, `EDIT`, `POSE`, `SCULPT`, `VERTEX_PAINT`, `WEIGHT_PAINT`, `TEXTURE_PAINT`, `PARTICLE_EDIT`, `EDIT_GPENCIL`, `SCULPT_GREASE_PENCIL`, `PAINT_GREASE_PENCIL`, `WEIGHT_GREASE_PENCIL`, `VERTEX_GREASE_PENCIL`, `SCULPT_CURVES` |  |
| `toggle` | BOOLEAN | `False` |  |  |
| `mesh_select_mode` *(hidden)* | ENUM flag-set | `{}` | `VERT`, `EDGE`, `FACE` |  |

Same as `mode_set` plus `mesh_select_mode` (ENUM flag set of `VERT`,`EDGE`,`FACE`) that also sets `tool_settings.mesh_select_mode`.

#### `object.editmode_toggle` - Edit Mode

*Toggle object's edit mode* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Toggle between OBJECT and EDIT for the active object (and other selected objects of the same type).

#### `object.posemode_toggle` - Toggle Pose Mode

*Enable or disable posing/selecting bones* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Toggle POSE mode for the active armature.

### 4.2 Object selection

#### `object.select_all` - (De)select All

*Change selection of all visible objects in scene* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_all(action='TOGGLE')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `action` *(hidden)* | ENUM | `'TOGGLE'` | `TOGGLE`, `SELECT`, `DESELECT`, `INVERT` | Selection action to execute |

- `action`: `TOGGLE` (default) = if *any* selectable object is selected -> deselect all, else select all; `SELECT`, `DESELECT`, `INVERT`.
- Never changes the active object (an active object can be unselected — verified: after DESELECT `context.active_object` is still Cube, `selected_objects == []`).
- Poll: object mode only (fails in edit/pose; there the equivalents are `mesh.select_all`, `pose.select_all`, `armature.select_all`).
- Only affects visible + selectable objects of the current view layer.

#### `object.select_by_type` - Select by Type

*Select all visible objects that are of a type* | priority **P1** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_by_type(extend=False, type='MESH')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `extend` | BOOLEAN | `False` |  | Extend selection instead of deselecting everything first |
| `type` | ENUM | `'MESH'` | `MESH`, `CURVE`, `SURFACE`, `META`, `FONT`, `CURVES`, `POINTCLOUD`, `VOLUME`, `GREASEPENCIL`, `ARMATURE`, `LATTICE`, `EMPTY`, `LIGHT`, `LIGHT_PROBE`, `CAMERA`, `SPEAKER` |  |

Select all visible objects of `type`. `extend=False` first deselects everything (verified). Active unchanged.

#### `object.select_pattern` - Select Pattern

*Select objects matching a naming pattern* | priority **P1** | poll A-E: `Y Y N Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_pattern(pattern='*', case_sensitive=False, extend=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `pattern` | STRING | `"*"` |  | Name filter using '*', '?' and '[abc]' unix style wildcards |
| `case_sensitive` | BOOLEAN | `False` |  | Do a case sensitive compare |
| `extend` | BOOLEAN | `True` |  | Extend the existing selection |

fnmatch-style glob on object names (`*`, `?`, `[seq]`). **`extend` defaults to True** (adds to selection, verified). In edit-armature / pose mode it matches bone names instead.

#### `object.select_grouped` - Select Grouped

*Select all visible objects grouped by various properties* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_grouped(extend=False, type='CHILDREN_RECURSIVE')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `extend` | BOOLEAN | `False` |  | Extend selection instead of deselecting everything first |
| `type` | ENUM | `'CHILDREN_RECURSIVE'` | `CHILDREN_RECURSIVE`, `CHILDREN`, `PARENT`, `SIBLINGS`, `TYPE`, `COLLECTION`, `HOOK`, `PASS`, `COLOR`, `KEYINGSET`, `LIGHT_TYPE` |  |

Select objects related to the active one (`CHILDREN_RECURSIVE`, `CHILDREN`, `PARENT`, `SIBLINGS`, `TYPE`, `COLLECTION`, ...). Poll returned False in all headless states tested (needs an active *selected* object and probably more context) — uncertain.

#### `object.select_hierarchy` - Select Hierarchy

*Select object relative to the active object's position in the hierarchy* | priority **P2** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_hierarchy(direction='PARENT', extend=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `direction` | ENUM | `'PARENT'` | `PARENT`, `CHILD` | Direction to select in the hierarchy |
| `extend` | BOOLEAN | `False` |  | Extend the existing selection |

Select parent (`direction='PARENT'`) or children (`'CHILD'`) of selected objects; the newly selected become active.

#### `object.select_linked` - Select Linked

*Select all visible objects that are linked* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_linked(extend=False, type='OBDATA')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `extend` | BOOLEAN | `False` |  | Extend selection instead of deselecting everything first |
| `type` | ENUM | `'OBDATA'` | `OBDATA`, `MATERIAL`, `DUPGROUP`, `PARTICLE`, `LIBRARY`, `LIBRARY_OBDATA` |  |

Select objects sharing data with the active (`OBDATA`, `MATERIAL`, ...).

#### `object.select_random` - Select Random

*Select or deselect random visible objects* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_random(ratio=0.5, seed=0, action='SELECT')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `ratio` | FLOAT FACTOR | `0.5` | [0, 1] | Portion of items to select randomly |
| `seed` | INT | `0` | [0, INT_MAX] soft [0, 255] | Seed for the random number generator |
| `action` | ENUM | `'SELECT'` | `SELECT`, `DESELECT` | Selection action to execute |

#### `object.select_camera` - Select Camera

*Select the active camera* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_camera(extend=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `extend` | BOOLEAN | `False` |  | Extend the selection |

Select the scene camera and make it active.

#### `object.select_mirror` - Select Mirror

*Select the mirror objects of the selected object e.g. "L.sword" and "R.sword"* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_mirror(extend=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `extend` | BOOLEAN | `False` |  | Extend selection instead of deselecting everything first |

Select objects with mirrored names (`.L`/`.R`).

#### `object.select_more` - Select More

*Select connected parent/child objects* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

Select parents/children of selected (hierarchy grow).

#### `object.select_less` - Select Less

*Deselect objects at the boundaries of parent/child relationships* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.select_same_collection` - Select Same Collection

*Select object in the same collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.select_same_collection(collection='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection` | STRING | `""` |  | Name of the collection to select |

### 4.3 Adding objects (non-mesh)

#### `object.camera_add` - Add Camera

*Add a camera object to the scene* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.camera_add(enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` *(hidden)* | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

- Adds object `Camera` + camera data `Camera` (lens 50 mm, sensor 36 mm, `sensor_fit='AUTO'`, `clip_start=0.1`, `clip_end=1000`, `type='PERSP'`) at the 3D cursor; selected+active, others deselected (same placement rules as `mesh.primitive_*`).
- **Rotation**: `align` default WORLD -> rotation (0,0,0) (camera looks down -Z) when no 3D view; with a 3D View context Blender aligns it to the view (`camera_add` always uses the view rotation in the UI; verified rotation (1.1088,0,1.1549) = factory viewport). Prefer explicit `rotation=`.
- **scene.camera**: set to the new camera only when a 3D View exists and the scene has no camera (verified: background without area -> `scene.camera` stays None; with VIEW_3D override -> set). Pitfall for render: frap should decide (recommend: mirror the VIEW_3D-context behaviour, i.e. set `scene.camera` if None, and run Blender goldens under a VIEW_3D `temp_override`).

#### `object.light_add` - Add Light

*Add a light object to the scene* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.light_add(type='POINT', radius=1, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'POINT'` | `POINT`, `SUN`, `SPOT`, `AREA` |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

- Adds object + light data named after the type: `Point`, `Sun`, `Spot`, `Area` (verified). Placement like primitives (cursor, selected+active).
- Data defaults from light_add (verified 5.2, **differ from the startup-file `Light`** which has 1000 W): POINT energy 10 W, SPOT 10 W (`spot_size`=0.785398 (45 deg), `spot_blend`=0.15), AREA 10 W (`shape='SQUARE'`, `size`=1.0), SUN strength 1.0; color (1,1,1); `shadow_soft_size` read back as 0.0 (verified twice; the factory `Light` and `bpy.data.lights.new` may differ); `temperature` 6500, `exposure` 0.
- `radius` does NOT change object scale, `shadow_soft_size` or `empty_display_size` in 5.2 (verified radius=2/3) — effectively unused for lights.
- glTF mapping (KHR_lights_punctual) needs the exporter's `export_import_convert_lighting_mode` (SPEC/COMPAT/RAW) W->candela/lux conversion. AREA lights are not representable in glTF (exporter skips them).

#### `object.empty_add` - Add Empty

*Add an empty object to the scene* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.empty_add(type='PLAIN_AXES', radius=1, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'PLAIN_AXES'` | `PLAIN_AXES`, `ARROWS`, `SINGLE_ARROW`, `CIRCLE`, `CUBE`, `SPHERE`, `CONE`, `IMAGE` |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Adds `Empty` object; `type` -> `empty_display_type`, `radius` -> `empty_display_size` (verified radius=3 -> size 3, scale stays 1). Empties export as glTF nodes without mesh.

#### `object.armature_add` - Add Armature

*Add an armature object to the scene* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.armature_add(radius=1, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

- Adds object `Armature` + data `Armature` with one bone `Bone`, head (0,0,0), tail (0,0,`radius`) in armature space (verified). Placement like primitives.
- `enter_editmode=True` -> `EDIT_ARMATURE` mode (verified; the new bone was **not** selected in edit mode).

#### `object.add` - Add Object

*Add an object to the scene* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.add(radius=1, type='EMPTY', enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `type` | ENUM | `'EMPTY'` | `MESH`, `CURVE`, `SURFACE`, `META`, `FONT`, `CURVES`, `POINTCLOUD`, `VOLUME`, `GREASEPENCIL`, `ARMATURE`, `LATTICE`, `EMPTY`, `LIGHT`, `LIGHT_PROBE`, `CAMERA`, `SPEAKER` |  |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Generic add by `type` (`MESH` -> empty mesh object `Mesh`, `EMPTY`, `LIGHT`, `CAMERA`, ...). Same placement rules as primitives.

#### `object.add_named` - Add Object

*Add named object* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.add_named(linked=False, name='', session_uid=0, matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0), drop_x=0, drop_y=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `linked` | BOOLEAN | `False` |  | Duplicate object but not object data, linking to the original data |
| `name` *(hidden)* | STRING | `""` |  | Name of the data-block to use by the operator |
| `session_uid` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Session UID of the data-block to use by the operator |
| `matrix` *(hidden)* | FLOAT[16] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `drop_x` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | X-coordinate (screen space) to place the new object under |
| `drop_y` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Y-coordinate (screen space) to place the new object under |

Duplicate a named object (`name`/`session_uid`) at the cursor/drop point.

#### `object.empty_image_add` - Add Empty Image/Drop Image to Empty

*Add an empty image type to scene with data* | priority **OUT** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.object.empty_image_add(filepath='', hide_props_region=True, check_existing=False, filter_blender=False, filter_backup=False, filter_image=True, filter_movie=True, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=9, relative_path=True, show_multiview=False, use_multiview=False, display_type='DEFAULT', sort_method='' (dynamic), name='', session_uid=0, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0), background=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` *(hidden)* | STRING FILE_PATH | `""` |  | Path to file |
| `hide_props_region` *(hidden)* | BOOLEAN | `True` |  | Collapse the region displaying the operator settings |
| `check_existing` *(hidden)* | BOOLEAN | `False` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `9` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `relative_path` *(hidden)* | BOOLEAN | `True` |  | Select the file relative to the blend file |
| `show_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `use_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | `DEFAULT`, `FILE_SORT_ALPHA`, `FILE_SORT_EXTENSION`, `FILE_SORT_TIME`, `FILE_SORT_SIZE`, `ASSET_CATALOG` |  |
| `name` *(hidden)* | STRING | `""` |  | Name of the data-block to use by the operator |
| `session_uid` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Session UID of the data-block to use by the operator |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |
| `background` | BOOLEAN | `False` |  | Make the image render behind all objects |

Image empty (reference image). Needs a VIEW_3D context.

#### `object.collection_instance_add` - Add Collection Instance

*Add a collection instance* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.collection_instance_add(name='Collection', collection='' (dynamic), align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0), session_uid=0, drop_x=0, drop_y=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `name` | STRING | `"Collection"` |  | Collection name to add |
| `collection` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |
| `session_uid` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Session UID of the data-block to use by the operator |
| `drop_x` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | X-coordinate (screen space) to place the new object under |
| `drop_y` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Y-coordinate (screen space) to place the new object under |

Add a collection instance empty (`instance_type='COLLECTION'`). glTF exporter can export instances (realized).

#### `object.data_instance_add` - Add Object Data Instance

*Add an object data instance* | priority **OUT** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.data_instance_add(name='', session_uid=0, type='ACTION', align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0), drop_x=0, drop_y=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `name` *(hidden)* | STRING | `""` |  | Name of the data-block to use by the operator |
| `session_uid` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Session UID of the data-block to use by the operator |
| `type` | ENUM | `'ACTION'` | `ACTION`, `ARMATURE`, `BRUSH`, `CACHEFILE`, `CAMERA`, `COLLECTION`, `CURVE`, `CURVES`, `FONT`, `GREASEPENCIL`, `GREASEPENCIL_V3`, `IMAGE`, `KEY`, `LATTICE`, `LIBRARY`, `LIGHT`, `LIGHT_PROBE`, `LINESTYLE`, `MASK`, `MATERIAL`, `MESH`, `META`, `MOVIECLIP`, `NODETREE`, `OBJECT`, `PAINTCURVE`, `PALETTE`, `PARTICLE`, `POINTCLOUD`, `SCENE`, `SCREEN`, `SOUND`, `SPEAKER`, `TEXT`, `TEXTURE`, `VOLUME`, `WINDOWMANAGER`, `WORKSPACE`, `WORLD` |  |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |
| `drop_x` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | X-coordinate (screen space) to place the new object under |
| `drop_y` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] | Y-coordinate (screen space) to place the new object under |

#### `object.text_add` - Add Text

*Add a text object to the scene* | priority **OUT** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.text_add(radius=1, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Text objects are not glTF; could be converted to mesh via `object.convert`. Out of scope unless fonts are supported.

### 4.4 Delete / duplicate / join

#### `object.delete` - Delete

*Delete selected objects* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.delete(use_global=False, confirm=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_global` *(hidden)* | BOOLEAN | `False` |  | Remove object from all scenes |
| `confirm` *(hidden)* | BOOLEAN | `True` |  | Prompt for confirmation |

- Deletes all **selected** objects (not just the active). `use_global=True` also unlinks from all other scenes. `confirm` only matters for invoke (UI).
- The object-data (mesh, material, ...) is **not** freed; it becomes an orphan (`users == 0`) and stays in `bpy.data.meshes` until the file is saved/reloaded (Blender does not write 0-user IDs unless `use_fake_user`). Verified: deleting `Cube` leaves `bpy.data.meshes['Cube'].users == 0`. frap's `.frap` writer should mirror the "orphans are dropped on save" rule.
- Children of a deleted object are un-parented **keeping their world transform** (verified).
- If the active object is deleted, `view_layer.objects.active` becomes `None`.

#### `object.duplicate` - Duplicate Objects

*Duplicate selected objects* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.duplicate(linked=False, mode='TRANSLATION')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `linked` | BOOLEAN | `False` |  | Duplicate object but not object data, linking to the original data |
| `mode` *(hidden)* | ENUM | `'TRANSLATION'` | `INIT`, `DUMMY`, `TRANSLATION`, `ROTATION`, `RESIZE`, `SKIN_RESIZE`, `TOSPHERE`, `SHEAR`, `BEND`, `SHRINKFATTEN`, `TILT`, `TRACKBALL`, `PUSHPULL`, `CREASE`, `VERTEX_CREASE`, `MIRROR`, `BONE_SIZE`, `BONE_ENVELOPE`, `BONE_ENVELOPE_DIST`, `CURVE_SHRINKFATTEN`, `MASK_SHRINKFATTEN`, `BONE_ROLL`, `TIME_TRANSLATE`, `TIME_SLIDE`, `TIME_SCALE`, `TIME_EXTEND`, `BAKE_TIME`, `BWEIGHT`, `ALIGN`, `EDGESLIDE`, `SEQSLIDE`, `GPENCIL_OPACITY` |  |

- Duplicates all selected objects; duplicates become selected, originals deselected, active = duplicate of the previous active (verified).
- Which data is deep-copied follows **Preferences > Editing > Duplicate Data** (`preferences.edit.use_duplicate_*`). Factory defaults (verified 5.2): copied = mesh, curve, surface, text, metaball, armature, lattice, light, camera, speaker, lightprobe, grease pencil, curves, pointcloud, **action**; *shared* (linked) = **material**, node_tree, particle, volume. So a duplicated cube gets mesh `Cube.001` but keeps the same `Material`.
- `linked=True` shares the object data (Alt+D).
- The new object is located at the same transform (no offset); `duplicate_move` adds the translate.
- Names: lowest free numeric suffix (`Cube.001`, `Cube.002`, ...). Note Blender 4.x+ name collision algorithm: if the base name is free it is used (verified `Cube` reused after renaming).

#### `object.duplicate_move` - Duplicate Objects

*Duplicate the selected objects and move them* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: MACRO, REGISTER, UNDO

`bpy.ops.object.duplicate_move(OBJECT_OT_duplicate={...}, TRANSFORM_OT_translate={...})`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `OBJECT_OT_duplicate` | POINTER -> OBJECT_OT_duplicate | `-` |  | Duplicate selected objects |
| `TRANSFORM_OT_translate` | POINTER -> TRANSFORM_OT_translate | `-` |  | Move selected items |

- **Macro operator** (`bl_options` contains `MACRO`): properties are two nested pointer groups: `OBJECT_OT_duplicate={'linked': False, 'mode': 'TRANSLATION'}` and `TRANSFORM_OT_translate={'value': (x,y,z), 'orient_type': ..., ...}` (verified: `duplicate_move(TRANSFORM_OT_translate={'value':(2,0,0)})` -> new object at x=2).
- frap's front-ends must support **nested dict arguments** for macros (bpy form: Python dict literal; short form: needs a syntax such as `TRANSFORM_OT_translate.value=2,0,0`). Same for `duplicate_move_linked`, and in edit mode `mesh.extrude_region_move`, `mesh.duplicate_move`, etc.

#### `object.duplicate_move_linked` - Duplicate Linked

*Duplicate the selected objects, but not their object data, and move them* | priority **P1** | poll A-E: `Y Y N N N` | bl_options: MACRO, REGISTER, UNDO

`bpy.ops.object.duplicate_move_linked(OBJECT_OT_duplicate={...}, TRANSFORM_OT_translate={...})`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `OBJECT_OT_duplicate` | POINTER -> OBJECT_OT_duplicate | `-` |  | Duplicate selected objects |
| `TRANSFORM_OT_translate` | POINTER -> TRANSFORM_OT_translate | `-` |  | Move selected items |

Macro: `OBJECT_OT_duplicate(linked=True)` + `TRANSFORM_OT_translate`.

#### `object.duplicates_make_real` - Make Instances Real

*Make instanced objects attached to this object real* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.duplicates_make_real(use_base_parent=False, use_hierarchy=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_base_parent` | BOOLEAN | `False` |  | Parent newly created objects to the original instancer |
| `use_hierarchy` | BOOLEAN | `False` |  | Maintain parent child relationships |

Make instanced (collection/vertex/face instancing) objects real. Only relevant if instancing is supported.

#### `object.join` - Join

*Join selected objects into active object* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

No properties.

- Target = **active object**, which must also be **selected** and a mesh, else CANCELLED with warning `Active object is not a selected mesh` (verified). With no other selected mesh: CANCELLED `No mesh data to join` (verified). In Blender these are *CANCELLED + warning*, not exceptions — frap should still surface them as JSON errors.
- Other selected meshes are merged into the active object's mesh, their vertices transformed into the active object's local space; those objects are deleted; their mesh datablocks become orphans (verified `bpy.data.meshes['Sphere']` remains).
- Material slots are merged (union, by material identity; faces are re-indexed) — verified the joined object got slots `['Material', None]` (the joined sphere had no material, so an empty slot was added).
- Vertex groups merged by name; UV maps / color attributes merged by name (layers missing on some meshes are filled with defaults); shape keys: if the active has shape keys, others get basis-copied keys (uncertain details).
- Selection after: only the active object selected (verified).
- Poll: object mode, active object editable (poll was True in edit mode too but the exec checks mode; treat as object-mode op).

#### `object.join_shapes` - Join as Shapes

*Add the vertex positions of selected objects as shape keys or update existing shape keys with matching names* | priority **P2** | poll A-E: `Y N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.join_shapes(use_mirror=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_mirror` | BOOLEAN | `False` |  | Mirror the new shape key values |

Add selected objects (same vertex count) as shape keys of the active object. Useful for glTF morph targets authoring.

#### `object.convert` - Convert To

*Convert selected objects to another type* | priority **P1** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.convert(target='MESH', keep_original=False, merge_customdata=True, thickness=5, faces=True, offset=0.01)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `target` | ENUM | `'MESH'` | `CURVE`, `MESH`, `POINTCLOUD`, `CURVES`, `GREASEPENCIL` | Type of object to convert to |
| `keep_original` | BOOLEAN | `False` |  | Keep original objects instead of replacing them |
| `merge_customdata` | BOOLEAN | `True` |  | Merge UV coordinates that share a vertex to account for imprecision in some modifiers |
| `thickness` | INT | `5` | [1, 100] |  |
| `faces` | BOOLEAN | `True` |  | Export faces as filled strokes |
| `offset` | FLOAT DISTANCE (LENGTH) | `0.01` | [0, 1e12] soft [0, 100] | Offset strokes from fill |

`target='MESH'` on a mesh object **applies the whole modifier stack** (evaluated) and clears modifiers (verified: subsurf applied, modifiers []). `keep_original=True` makes a converted copy. Other targets (CURVE, CURVES, POINTCLOUD, GREASEPENCIL) OUT.

### 4.5 Parenting

#### `object.parent_set` - Make Parent

*Set the object's parenting* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.parent_set(type='OBJECT', xmirror=False, keep_transform=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'OBJECT'` | `OBJECT`, `ARMATURE`, `ARMATURE_NAME`, `ARMATURE_AUTO`, `ARMATURE_ENVELOPE`, `BONE`, `BONE_RELATIVE`, `CURVE`, `FOLLOW`, `PATH_CONST`, `LATTICE`, `VERTEX`, `VERTEX_TRI` |  |
| `xmirror` | BOOLEAN | `False` |  | Apply weights symmetrically along X axis, for Envelope/Automatic vertex groups creation |
| `keep_transform` | BOOLEAN | `False` |  | Apply transformation before parenting |

- Parent = **active** object; children = all *other selected* objects. Selection is unchanged (verified both stay selected).
- `type='OBJECT'`, `keep_transform=False` (default): sets `child.parent`, sets `child.matrix_parent_inverse = parent.matrix_world.inverted()` so the child does not move visually; **child.location/rotation/scale values are not modified** (verified: cone keeps location (0,3,0), `matrix_parent_inverse` != identity).
- `keep_transform=True`: if the child already had a parent, its current world transform is first baked into its loc/rot/scale (then same as above).
- Armature types: `ARMATURE` (adds Armature modifier only), `ARMATURE_NAME` (also creates empty vertex groups per deform bone), `ARMATURE_ENVELOPE` (envelope weights), `ARMATURE_AUTO` (automatic heat weights — requires a heat-diffusion solver; hard), `BONE` / `BONE_RELATIVE` (parent to active bone; `parent_type='BONE'`, `parent_bone`). Skinning in glTF needs `ARMATURE`/`ARMATURE_NAME`/`ARMATURE_AUTO`.
- Loops (parenting to own child) are refused with an error ("Loop in parents").
- `VERTEX`, `VERTEX_TRI`, `CURVE`, `FOLLOW`, `PATH_CONST`, `LATTICE`: OUT/P2 (not glTF).

#### `object.parent_clear` - Clear Parent

*Clear the object's parenting* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.parent_clear(type='CLEAR')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'CLEAR'` | `CLEAR`, `CLEAR_KEEP_TRANSFORM`, `CLEAR_INVERSE` |  |

- On all selected objects. `CLEAR`: parent=None, local values kept => object jumps (world = matrix_basis). `CLEAR_KEEP_TRANSFORM`: bakes world matrix into loc/rot/scale (verified). `CLEAR_INVERSE`: keeps parent but resets `matrix_parent_inverse` to identity (object may jump).

#### `object.parent_no_inverse_set` - Make Parent without Inverse

*Set the object's parenting without setting the inverse parent correction* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.parent_no_inverse_set(keep_transform=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `keep_transform` | BOOLEAN | `False` |  | Preserve the world transform throughout parenting |

Parent without inverse: `matrix_parent_inverse` = identity. With `keep_transform=False` (default) the child's **location is reset to (0,0,0)** so it sits at the parent origin (verified). `keep_transform=True` computes local values keeping the world transform. This is the closest to glTF semantics (glTF has no parent-inverse matrix: the exporter folds it into the node TRS).

#### `object.parent_inverse_apply` - Apply Parent Inverse

*Apply the object's parent inverse to its data* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

Bake `matrix_parent_inverse` into the local transform and reset it to identity.

#### `object.vertex_parent_set` - Make Vertex Parent

*Parent selected objects to the selected vertices* | priority **OUT** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

No properties.

Edit mode: parent selected objects to 1 or 3 selected vertices.

### 4.6 Origin and object transforms

#### `object.origin_set` - Set Origin

*Set the object's origin, by either moving the data, or set to center of data, or use 3D cursor* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.origin_set(type='GEOMETRY_ORIGIN', center='MEDIAN')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'GEOMETRY_ORIGIN'` | `GEOMETRY_ORIGIN`, `ORIGIN_GEOMETRY`, `ORIGIN_CURSOR`, `ORIGIN_CENTER_OF_MASS`, `ORIGIN_CENTER_OF_VOLUME` |  |
| `center` | ENUM | `'MEDIAN'` | `MEDIAN`, `BOUNDS` |  |

- Acts on all selected objects. `GEOMETRY_ORIGIN`: moves geometry so that its center (per `center`: `MEDIAN` of vertices or `BOUNDS` center) is at the object origin. `ORIGIN_GEOMETRY`: moves the origin to the geometry center, compensating mesh coords. `ORIGIN_CURSOR`: origin to `scene.cursor.location` (verified). `ORIGIN_CENTER_OF_MASS` (surface area-weighted) and `ORIGIN_CENTER_OF_VOLUME` (closed mesh).
- Mesh vertex coordinates are offset in object-local space, object `location` changes so world positions remain (rotation/scale accounted for). Children are compensated to keep their world transform (believed; verify with goldens).
- Multi-user data: each datablock processed once; linked library data refused.
- Default `type` is GEOMETRY_ORIGIN (the first enum item) — careful, in the UI menu the common one is ORIGIN_GEOMETRY.

#### `object.origin_clear` - Clear Origin

*Clear the object's origin* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Description: 'Clear the object's origin'. Verified: on an unparented object it changes neither `location` nor mesh coords. Believed to clear the translation part of `matrix_parent_inverse` for parented objects (Alt+O). Low value for frap.

#### `object.transform_apply` - Apply Object Transform

*Apply the object's transformation to its data* | priority **P0** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.transform_apply(location=True, rotation=True, scale=True, properties=True, corrective_flip_normals=True, isolate_users=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `location` | BOOLEAN | `True` |  |  |
| `rotation` | BOOLEAN | `True` |  |  |
| `scale` | BOOLEAN | `True` |  |  |
| `properties` | BOOLEAN | `True` |  | Modify properties such as curve vertex radius, font size and bone envelope |
| `corrective_flip_normals` | BOOLEAN | `True` |  | Invert normals for negative scaled objects. |
| `isolate_users` *(hidden)* | BOOLEAN | `False` |  | Create new object-data users if needed |

- Bakes object loc/rot/scale (each toggle) into the object data (mesh vertices, armature bones, lights?/cameras: scale not applicable — warning) and resets those channels to identity (verified: rotation+scale applied, vertex 0 -> (1.6152,1.5464,1.0) local).
- Children are compensated (their parent-inverse/local transforms adjusted) so they do not move.
- Multi-user data -> **RuntimeError** `Cannot apply to a multi user: Object "Cube", Mesh "Cube", aborting` (verified) unless hidden prop `isolate_users=True` (then the data is copied: `Cube.002`, verified).
- `corrective_flip_normals=True`: when the applied scale has a negative determinant, face winding is flipped so normals stay outward.
- `properties=True`: also scales curve radius / font size / bone envelope (only bone envelopes matter for frap).
- Delta transforms (`delta_location` etc.) are *not* applied (they stay).

#### `object.visual_transform_apply` - Apply Visual Transform

*Apply the object's visual transformation to its data* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Apply constraint/driver results into loc/rot/scale.

#### `object.transforms_to_deltas` - Transforms to Deltas

*Convert normal object transforms to delta transforms, any existing delta transforms will be included as well* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.transforms_to_deltas(mode='ALL', reset_values=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mode` | ENUM | `'ALL'` | `ALL`, `LOC`, `ROT`, `SCALE` | Which transforms to transfer |
| `reset_values` | BOOLEAN | `True` |  | Clear transform values after transferring to deltas |

Move loc/rot/scale into `delta_*` and reset (if `reset_values`). frap's world-matrix computation must include `delta_location/delta_rotation_euler/delta_rotation_quaternion/delta_scale` regardless.

#### `object.anim_transforms_to_deltas` - Animated Transforms to Deltas

*Convert object animation for normal transforms to delta transforms* | priority **OUT** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

No properties.

#### `object.location_clear` - Clear Location

*Clear the object's location* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.location_clear(clear_delta=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `clear_delta` | BOOLEAN | `False` |  | Clear delta location in addition to clearing the normal location transform |

Selected objects: `location=(0,0,0)`; `clear_delta=True` also zeroes `delta_location`. Poll true in all modes but acts on objects (in pose mode use `pose.loc_clear`).

#### `object.rotation_clear` - Clear Rotation

*Clear the object's rotation* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.rotation_clear(clear_delta=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `clear_delta` | BOOLEAN | `False` |  | Clear delta rotation in addition to clearing the normal rotation transform |

Resets rotation for the object's current `rotation_mode` (euler (0,0,0), quaternion (1,0,0,0), axis-angle (0,0,1,0)).

#### `object.scale_clear` - Clear Scale

*Clear the object's scale* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.scale_clear(clear_delta=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `clear_delta` | BOOLEAN | `False` |  | Clear delta scale in addition to clearing the normal scale transform |

`scale=(1,1,1)` (and `delta_scale` with `clear_delta`).

#### `object.randomize_transform` - Randomize Transform

*Randomize objects location, rotation, and scale* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.randomize_transform(random_seed=0, use_delta=False, use_loc=True, loc=(0, 0, 0), use_rot=True, rot=(0, 0, 0), use_scale=True, scale_even=False, scale=(1, 1, 1))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `random_seed` | INT | `0` | [0, 10000] | Seed value for the random generator |
| `use_delta` | BOOLEAN | `False` |  | Randomize delta transform values instead of regular transform |
| `use_loc` | BOOLEAN | `True` |  | Randomize the location values |
| `loc` | FLOAT[3] TRANSLATION (LENGTH) | `(0, 0, 0)` | [-100, 100] | Maximum distance the objects can spread over each axis |
| `use_rot` | BOOLEAN | `True` |  | Randomize the rotation values |
| `rot` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-3.14159, 3.14159] | Maximum rotation over each axis |
| `use_scale` | BOOLEAN | `True` |  | Randomize the scale values |
| `scale_even` | BOOLEAN | `False` |  | Use the same scale value for all axis |
| `scale` | FLOAT[3] | `(1, 1, 1)` | [-100, 100] | Maximum scale randomization over each axis |

#### `object.align` - Align Objects

*Align objects* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.align(bb_quality=True, align_mode='OPT_2', relative_to='OPT_4', align_axis={})`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `bb_quality` | BOOLEAN | `True` |  | Enables high quality but slow calculation of the bounding box for perfect results on complex shape meshes with rotation/scale |
| `align_mode` | ENUM | `'OPT_2'` | `OPT_1`, `OPT_2`, `OPT_3` | Side of object to use for alignment |
| `relative_to` | ENUM | `'OPT_4'` | `OPT_1`, `OPT_2`, `OPT_3`, `OPT_4` | Reference location to align to |
| `align_axis` | ENUM flag-set | `{}` | `X`, `Y`, `Z` | Align to axis |

Align objects' bounding boxes (`align_axis` is an ENUM flag set X/Y/Z).

### 4.7 Shading

#### `object.shade_smooth` - Shade Smooth

*Render and display faces smooth, using interpolated vertex normals* | priority **P0** | poll A-E: `Y Y N Y N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shade_smooth(keep_sharp_edges=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `keep_sharp_edges` | BOOLEAN | `True` |  | Don't remove sharp edges. Tagged edges will remain sharp |

- Mesh data model since 4.1: smoothness is the boolean face attribute `sharp_face` and edge attribute `sharp_edge` (there is no `use_auto_smooth` any more). `shade_smooth` **removes** `sharp_face` (all faces smooth). `keep_sharp_edges=True` keeps existing `sharp_edge`; False removes it too (verified attribute list).
- All selected objects (meshes; curves too). Primitives are created **flat** (`sharp_face` all True — verified on every primitive).
- In edit mode use `mesh.faces_shade_smooth` / `mesh.faces_shade_flat` (not this op; poll False in edit mode — verified).

#### `object.shade_flat` - Shade Flat

*Render and display faces uniform, using face normals* | priority **P0** | poll A-E: `Y Y N Y N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shade_flat(keep_sharp_edges=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `keep_sharp_edges` | BOOLEAN | `True` |  | Don't remove sharp edges, which are redundant with faces shaded smooth |

Adds/sets `sharp_face=True` on all faces (verified). `keep_sharp_edges=False` also clears `sharp_edge`.

#### `object.shade_smooth_by_angle` - Shade Smooth by Angle

*Set the sharpness of mesh edges based on the angle between the neighboring faces* | priority **P0** | poll A-E: `Y Y N Y N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shade_smooth_by_angle(angle=0.523599, keep_sharp_edges=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `angle` | FLOAT ANGLE (ROTATION) | `0.523599` | [0, 3.14159] | Maximum angle between face normals that will be considered as smooth |
| `keep_sharp_edges` | BOOLEAN | `True` |  | Only add sharp edges instead of clearing existing tags first |

- **Destructive**: sets all faces smooth and writes `sharp_edge=True` on edges whose dihedral angle > `angle` (radians, default 30 deg = 0.523599). Verified: cube -> 12 sharp edges, no `sharp_face` attribute. `keep_sharp_edges=True` keeps previously marked sharp edges.
- glTF export splits vertices along sharp edges / flat faces (normals per corner).

#### `object.shade_auto_smooth` - Shade Auto Smooth

*Add modifier to automatically set the sharpness of mesh edges based on the angle between the neighboring faces* | priority **P1** | poll A-E: `Y Y N Y N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shade_auto_smooth(use_auto_smooth=True, angle=0.523599)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_auto_smooth` | BOOLEAN | `True` |  | Add modifier to set edge sharpness automatically |
| `angle` | FLOAT ANGLE (ROTATION) | `0.523599` | [0, 3.14159] | Maximum angle between face normals that will be considered as smooth |

- **Non-destructive**: adds (or updates) a Geometry Nodes modifier named **"Smooth by Angle"** whose node group "Smooth by Angle" is appended from the Essentials asset library (verified: modifier type `NODES`, node_group `Smooth by Angle`). `use_auto_smooth=False` removes it. Inputs: `angle`, plus 'Ignore Sharpness' socket.
- frap has no Geometry Nodes: recommend a built-in pseudo node-group recognised by name (`type='NODES'`, `node_group.name == 'Smooth by Angle'`) with the two inputs exposed via `modifiers['Smooth by Angle']['Socket_1']`-style ID properties (check exact socket identifiers with Blender before implementing). Pitfall: socket identifiers are opaque (`Input_1`/`Socket_1`).

### 4.8 Modifier stack

#### `object.modifier_add` - Add Modifier

*Add a procedural operation/effect to the active object* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.modifier_add(type='SUBSURF', use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'SUBSURF'` | `GREASE_PENCIL_VERTEX_WEIGHT_PROXIMITY`, `DATA_TRANSFER`, `MESH_CACHE`, `MESH_SEQUENCE_CACHE`, `NORMAL_EDIT`, `WEIGHTED_NORMAL`, `UV_PROJECT`, `UV_WARP`, `VERTEX_WEIGHT_EDIT`, `VERTEX_WEIGHT_MIX`, `VERTEX_WEIGHT_PROXIMITY`, `GREASE_PENCIL_COLOR`, `GREASE_PENCIL_TINT`, `GREASE_PENCIL_OPACITY`, `GREASE_PENCIL_VERTEX_WEIGHT_ANGLE`, `GREASE_PENCIL_TIME`, `GREASE_PENCIL_TEXTURE`, `ARRAY`, `BEVEL`, `BOOLEAN`, `BUILD`, `DECIMATE`, `EDGE_SPLIT`, `NODES`, `MASK`, `MIRROR`, `MESH_TO_VOLUME`, `MULTIRES`, `REMESH`, `SCREW`, `SKIN`, `SOLIDIFY`, `SUBSURF`, `TRIANGULATE`, `VOLUME_TO_MESH`, `WELD`, `WIREFRAME`, `GREASE_PENCIL_ARRAY`, `GREASE_PENCIL_BUILD`, `GREASE_PENCIL_LENGTH`, `LINEART`, `GREASE_PENCIL_MIRROR`, `GREASE_PENCIL_MULTIPLY`, `GREASE_PENCIL_SIMPLIFY`, `GREASE_PENCIL_SUBDIV`, `GREASE_PENCIL_ENVELOPE`, `GREASE_PENCIL_OUTLINE`, `ARMATURE`, `CAST`, `CURVE`, `DISPLACE`, `HOOK`, `LAPLACIANDEFORM`, `LATTICE`, `MESH_DEFORM`, `SHRINKWRAP`, `SIMPLE_DEFORM`, `SMOOTH`, `CORRECTIVE_SMOOTH`, `LAPLACIANSMOOTH`, `SURFACE_DEFORM`, `WARP`, `WAVE`, `VOLUME_DISPLACE`, `GREASE_PENCIL_HOOK`, `GREASE_PENCIL_NOISE`, `GREASE_PENCIL_OFFSET`, `GREASE_PENCIL_SMOOTH`, `GREASE_PENCIL_THICKNESS`, `GREASE_PENCIL_LATTICE`, `GREASE_PENCIL_DASH`, `GREASE_PENCIL_ARMATURE`, `GREASE_PENCIL_SHRINKWRAP`, `CLOTH`, `COLLISION`, `DYNAMIC_PAINT`, `EXPLODE`, `FLUID`, `OCEAN`, `PARTICLE_INSTANCE`, `PARTICLE_SYSTEM`, `SOFT_BODY`, `SURFACE` |  |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

- Adds a modifier of `type` to the **active object** (only the active unless hidden `use_selected_objects=True`), becomes `modifiers.active`. Works in object and edit mode.
- `type` items are dynamic (filtered by object type). Full static list is shown below; frap scope: SUBSURF, MIRROR, ARRAY, BEVEL, SOLIDIFY, BOOLEAN, TRIANGULATE, DECIMATE, WELD, DISPLACE, EDGE_SPLIT, SCREW, WIREFRAME, SMOOTH, CAST, SIMPLE_DEFORM, ARMATURE, WEIGHTED_NORMAL, (NODES for Smooth-by-Angle only), plus optional REMESH/SKIN/SHRINKWRAP/LATTICE/CURVE/WAVE/UV_PROJECT/UV_WARP/MASK/BUILD/DATA_TRANSFER/NORMAL_EDIT.
- **Default names (verified 5.2)** — not always the enum label: SUBSURF→`Subdivision`, MIRROR→`Mirror`, ARRAY→`Array`, BEVEL→`Bevel`, SOLIDIFY→`Solidify`, BOOLEAN→`Boolean`, TRIANGULATE→`Triangulate`, DECIMATE→`Decimate`, WELD→`Weld`, DISPLACE→`Displace`, EDGE_SPLIT→`EdgeSplit`, SCREW→`Screw`, SKIN→`Skin`, WIREFRAME→`Wireframe`, REMESH→`Remesh`, SMOOTH→`Smooth`, CAST→`Cast`, SIMPLE_DEFORM→`SimpleDeform`, CURVE→`Curve`, LATTICE→`Lattice`, ARMATURE→`Armature`, SHRINKWRAP→`Shrinkwrap`, WAVE→`Wave`, DATA_TRANSFER→`DataTransfer`, NORMAL_EDIT→`NormalEdit`, WEIGHTED_NORMAL→`WeightedNormal`, UV_PROJECT→`UVProject`, UV_WARP→`UVWarp`, VERTEX_WEIGHT_EDIT→`VertexWeightEdit`, MASK→`Mask`, BUILD→`Build`, MULTIRES→`Multires`, NODES→`GeometryNodes`, LAPLACIANSMOOTH→`LaplacianSmooth`, CORRECTIVE_SMOOTH→`CorrectiveSmooth`, HOOK→`Hook`, SURFACE_DEFORM→`SurfaceDeform`, MESH_DEFORM→`MeshDeform`, LAPLACIANDEFORM→`LaplacianDeform`. Duplicates get `.001`.
- **Insertion position**: appended at the end, EXCEPT modifiers flagged "requires original data" (HOOK, MULTIRES, ...) which are inserted after the leading run of deform-only modifiers (verified: `Hook` inserted *before* an existing `Mirror`).
- Some types are refused for some data (e.g. adding a second `MULTIRES`, or `SKIN` on non-mesh) with an error report.

#### `object.modifier_remove` - Remove Modifier

*Remove a modifier from the active object* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_remove(modifier='', report=False, use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |
| `report` *(hidden)* | BOOLEAN | `False` |  | Create a notification after the operation |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

- Removes the modifier named by the hidden STRING prop `modifier` from the active object. Missing/empty name -> **CANCELLED, no exception** (verified). frap should return an explicit error JSON (`status: "CANCELLED"`, reason) — decision for the architect: treat CANCELLED as error.
- Works in edit mode.

#### `object.modifier_apply` - Apply Modifier

*Apply modifier and remove from the stack* | priority **P0** | poll A-E: `Y N N Y N` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_apply(modifier='', report=False, merge_customdata=True, single_user=False, all_keyframes=False, use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |
| `report` *(hidden)* | BOOLEAN | `False` |  | Create a notification after the operation |
| `merge_customdata` | BOOLEAN | `True` |  | For mesh objects, merge UV coordinates that share a vertex to account for imprecision in some modifiers |
| `single_user` *(hidden)* | BOOLEAN | `False` |  | Make the object's data single user if needed |
| `all_keyframes` *(hidden)* | BOOLEAN | `False` |  | For Grease Pencil objects, apply the modifier to all the keyframes |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

- Object mode only: in edit mode the poll fails with `This modifier operation is not allowed from Edit mode` (verified).
- Applies the modifier to the object data using **viewport** settings (e.g. Subdivision `levels`, not `render_levels`; `show_viewport=False` -> "Modifier is disabled, skipping apply"), evaluated on top of the original data (not the stack before it!). Applying a non-first modifier succeeds with Info `Applied modifier was not first, result may not be as expected` (verified, FINISHED) — the result is computed as if it were first.
- Refused on multi-user data unless hidden `single_user=True`; refused for non-deform modifiers on meshes with shape keys ("Modifier cannot be applied to a mesh with shape keys").
- `merge_customdata=True` merges UVs that became split by the modifier (precision).
- Empty `modifier` name -> CANCELLED (verified).

#### `object.modifier_apply_as_shapekey` - Apply Modifier as Shape Key

*Apply modifier as a new shape key and remove from the stack* | priority **P2** | poll A-E: `Y N N Y N` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_apply_as_shapekey(keep_modifier=False, modifier='', report=False, use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `keep_modifier` | BOOLEAN | `False` |  | Do not remove the modifier from stack |
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |
| `report` *(hidden)* | BOOLEAN | `False` |  | Create a notification after the operation |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

Deform-only modifiers: store result as a new shape key (`keep_modifier`). Useful to author morph targets.

#### `object.modifier_copy` - Copy Modifier

*Duplicate modifier at the same position in the stack* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_copy(modifier='', use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

Duplicate the named modifier, inserted right after it (`Bevel.001`).

#### `object.modifier_copy_to_selected` - Copy Modifier to Selected

*Copy the modifier from the active object to all selected objects* | priority **P2** | poll A-E: `N N N N N` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_copy_to_selected(modifier='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |

#### `object.modifier_move_up` - Move Up Modifier

*Move modifier up in the stack* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_move_up(modifier='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |

Move named modifier one slot up/down. No error at the boundary (returns FINISHED/CANCELLED, uncertain).

#### `object.modifier_move_down` - Move Down Modifier

*Move modifier down in the stack* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_move_down(modifier='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |

See `modifier_move_up`.

#### `object.modifier_move_to_index` - Move Active Modifier to Index

*Change the modifier's index in the stack so it evaluates after the set number of others* | priority **P0** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_move_to_index(modifier='', index=0, use_selected_objects=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |
| `index` | INT | `0` | [0, INT_MAX] | The index to move the modifier to |
| `use_selected_objects` *(hidden)* | BOOLEAN | `False` |  | Affect all selected objects instead of just the active object |

Move named modifier to `index` (0-based, clamped). Verified moving `Mirror` above `Hook` and `Hook` down both succeed (no ordering restriction observed).

#### `object.modifier_set_active` - Set Active Modifier

*Activate the modifier to use as the context* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.object.modifier_set_active(modifier='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `modifier` *(hidden)* | STRING | `""` |  | Name of the modifier to edit |

Set `object.modifiers.active` by name.

#### `object.modifiers_clear` - Clear Object Modifiers

*Clear all modifiers from the selected objects* | priority **P1** | poll A-E: `Y N Y N N` | bl_options: REGISTER, UNDO

No properties.

Remove all modifiers from selected objects.

#### `object.modifiers_copy_to_selected` - Copy Modifiers to Selected Objects

*Copy modifiers to other selected objects* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.subdivision_set` - Subdivision Set

*Sets a Subdivision Surface level (1 to 5)* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.subdivision_set(level=1, relative=False, ensure_modifier=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `level` | INT | `1` | [-100, 100] soft [-6, 6] |  |
| `relative` | BOOLEAN | `False` |  | Apply the subdivision surface level as an offset relative to the current level |
| `ensure_modifier` *(hidden)* | BOOLEAN | `True` |  | Create the corresponding modifier if it does not exist |

Ctrl+1..5 behaviour: on selected objects ensure a `Subdivision` modifier exists (if `ensure_modifier`, hidden) and set its `levels` to `level` (or add with `relative`). In edit mode / sculpt it may target Multires (ignore).

### 4.9 Material slots

#### `object.material_slot_add` - Add Material Slot

*Add a new material slot* | priority **P0** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

- Appends an **empty** slot to the active object and makes it active (`active_material_index` = new index; verified `['Material', None]`, index 1). Slot link is `'DATA'` by default (material stored in `mesh.materials`, shared by all users of the mesh); `slot.link='OBJECT'` stores it on the object.
- Not available in edit mode? Poll True in edit mode for add, False for remove (verified).

#### `object.material_slot_remove` - Remove Material Slot

*Remove the selected material slot* | priority **P0** | poll A-E: `Y N N N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

Remove the active slot; faces using it are reassigned to the previous index (indices above are decremented). Object mode only (poll False in edit mode, verified).

#### `object.material_slot_remove_all` - Remove All Materials

*Remove all materials* | priority **P2** | poll A-E: `Y N N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.material_slot_remove_unused` - Remove Unused Slots

*Remove unused material slots* | priority **P1** | poll A-E: `Y N N N N` | bl_options: REGISTER, UNDO

No properties.

Remove slots not used by any face. glTF exporter creates one primitive per used material.

#### `object.material_slot_assign` - Assign Material Slot

*Assign active material slot to selection* | priority **P0** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

**Edit mode**: assign the active slot index (`active_material_index`) to all selected faces (`material_index` attribute). In object mode it does nothing useful.

#### `object.material_slot_select` - Select Material Slot

*Select by active material slot* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

Edit mode: select faces using the active slot.

#### `object.material_slot_deselect` - Deselect Material Slot

*Deselect by active material slot* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

Edit mode: deselect faces using the active slot.

#### `object.material_slot_copy` - Copy Material to Selected

*Copy material to selected objects* | priority **P2** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

Copy the active object's slots to other selected objects.

#### `object.material_slot_move` - Move Material

*Move the active material up/down in the list* | priority **P2** | poll A-E: `Y N Y N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.material_slot_move(direction='UP')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `direction` | ENUM | `'UP'` | `UP`, `DOWN` | Direction to move the active material towards |

Move active slot UP/DOWN, remapping face indices.

### 4.10 Vertex groups and weights

#### `object.vertex_group_add` - Add Vertex Group

*Add a new vertex group to the active object* | priority **P0** | poll A-E: `Y N Y N N` | bl_options: REGISTER, UNDO

No properties.

Add an empty group `Group` (then `Group.001`, ...) to the active object and make it active (verified `vertex_groups.active_index`). Works in object and edit mode.

#### `object.vertex_group_remove` - Remove Vertex Group

*Delete the active or all vertex groups from the active object* | priority **P0** | poll A-E: `N N N N N` | bl_options: UNDO

`bpy.ops.object.vertex_group_remove(all=False, all_unlocked=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `all` | BOOLEAN | `False` |  | Remove all vertex groups |
| `all_unlocked` | BOOLEAN | `False` |  | Remove all unlocked vertex groups |

Remove the active group (`all=True`: all groups; `all_unlocked`: unlocked ones). Poll requires the active object to already have vertex groups (poll message `Object has no vertex groups`, verified) — that is why it shows N in every state.

#### `object.vertex_group_assign` - Assign to Vertex Group

*Assign the selected vertices to the active vertex group* | priority **P0** | poll A-E: `N N Y N N` | bl_options: UNDO

No properties.

**Edit mode**: add selected vertices to the active group with weight `scene.tool_settings.vertex_group_weight` (default 1.0, verified). Weights are replaced for vertices already in the group.

#### `object.vertex_group_assign_new` - Assign to New Group

*Assign the selected vertices to a new vertex group* | priority **P1** | poll A-E: `N N Y N N` | bl_options: UNDO

No properties.

Edit mode: create a new group and assign selected vertices.

#### `object.vertex_group_remove_from` - Remove from Vertex Group

*Remove the selected vertices from active or all vertex group(s)* | priority **P0** | poll A-E: `N N Y N N` | bl_options: UNDO

`bpy.ops.object.vertex_group_remove_from(use_all_groups=False, use_all_verts=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_all_groups` | BOOLEAN | `False` |  | Remove from all groups |
| `use_all_verts` | BOOLEAN | `False` |  | Clear the active group |

Edit mode: remove selected verts from the active group (`use_all_groups`, `use_all_verts`).

#### `object.vertex_group_select` - Select Vertex Group

*Select all the vertices assigned to the active vertex group* | priority **P1** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

No properties.

Edit mode: select vertices in the active group.

#### `object.vertex_group_deselect` - Deselect Vertex Group

*Deselect all selected vertices assigned to the active vertex group* | priority **P1** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

No properties.

Edit mode: deselect vertices in the active group.

#### `object.vertex_group_set_active` - Set Active Vertex Group

*Set the active vertex group* | priority **P1** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_set_active(group='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Vertex group to set as active |

`group` is a dynamic enum of group names (index-as-string items). Prefer the property path `object.vertex_groups.active_index = i`.

#### `object.vertex_group_copy` - Duplicate Vertex Group

*Make a copy of the active vertex group* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.vertex_group_copy_to_selected` - Copy Vertex Group to Selected

*Replace vertex groups of selected objects by vertex groups of active object* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.vertex_group_normalize` - Normalize Vertex Group

*Normalize weights of the active vertex group, so that the highest ones are now 1.0* | priority **P1** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Normalize the active group so its max weight is 1.0.

#### `object.vertex_group_normalize_all` - Normalize All Vertex Groups

*Normalize all weights of all vertex groups, so that for each vertex, the sum of all weights is 1.0* | priority **P0** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_normalize_all(group_select_mode='' (dynamic), lock_active=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `lock_active` | BOOLEAN | `True` |  | Keep the values of the active group while normalizing others |

Normalize all deform weights so each vertex sums to 1 (`lock_active`). Required for clean glTF skins (exporter also normalizes). Poll requires the active object to already have vertex groups (poll message `Object has no vertex groups`, verified) — that is why it shows N in every state.

#### `object.vertex_group_invert` - Invert Vertex Group

*Invert active vertex group's weights* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_invert(group_select_mode='' (dynamic), auto_assign=True, auto_remove=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `auto_assign` | BOOLEAN | `True` |  | Add vertices from groups that have zero weight before inverting |
| `auto_remove` | BOOLEAN | `True` |  | Remove vertices from groups that have zero weight after inverting |

#### `object.vertex_group_clean` - Clean Vertex Group Weights

*Remove vertex group assignments which are not required* | priority **P1** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_clean(group_select_mode='' (dynamic), limit=0, keep_single=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `limit` | FLOAT | `0` | [0, 1] soft [0, 0.99] | Remove vertices which weight is below or equal to this limit |
| `keep_single` | BOOLEAN | `False` |  | Keep verts assigned to at least one group when cleaning |

Remove weights <= `limit` (`keep_single`).

#### `object.vertex_group_quantize` - Quantize Vertex Weights

*Set weights to a fixed number of steps* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_quantize(group_select_mode='' (dynamic), steps=4)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `steps` | INT | `4` | [1, 1000] soft [1, 100] | Number of steps between 0 and 1 |

#### `object.vertex_group_levels` - Vertex Group Levels

*Add some offset and multiply with some gain the weights of the active vertex group* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_levels(group_select_mode='' (dynamic), offset=0, gain=1)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `offset` | FLOAT | `0` | [-1, 1] | Value to add to weights |
| `gain` | FLOAT | `1` | [0, FLT_MAX] soft [0, 10] | Value to multiply weights by |

#### `object.vertex_group_limit_total` - Limit Number of Weights per Vertex

*Limit deform weights associated with a vertex to a specified number by removing lowest weights* | priority **P0** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_limit_total(group_select_mode='' (dynamic), limit=4)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `limit` | INT | `4` | [1, 32] | Maximum number of deform weights |

Keep only the `limit` (default 4) highest weights per vertex — matches glTF's 4-influences-per-set (`export_influence_nb=4`).

#### `object.vertex_group_lock` - Change the Lock On Vertex Groups

*Change the lock state of all or some vertex groups of active object* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_lock(action='TOGGLE', mask='ALL')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `action` | ENUM | `'TOGGLE'` | `TOGGLE`, `LOCK`, `UNLOCK`, `INVERT` | Lock action to execute on vertex groups |
| `mask` | ENUM | `'ALL'` | `ALL`, `SELECTED`, `UNSELECTED`, `INVERT_UNSELECTED` | Apply the action based on vertex group selection |

#### `object.vertex_group_mirror` - Mirror Vertex Group

*Mirror vertex group, flip weights and/or names, editing only selected vertices, flipping when both sides are selected otherwise copy from unselected* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_mirror(mirror_weights=True, flip_group_names=True, all_groups=False, use_topology=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mirror_weights` | BOOLEAN | `True` |  | Mirror weights |
| `flip_group_names` | BOOLEAN | `True` |  | Flip vertex group names |
| `all_groups` | BOOLEAN | `False` |  | Mirror all vertex groups weights |
| `use_topology` | BOOLEAN | `False` |  | Use topology based mirroring (for when both sides of mesh have matching, unique topology) |

#### `object.vertex_group_move` - Move Vertex Group

*Move the active vertex group up/down in the list* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_move(direction='UP')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `direction` | ENUM | `'UP'` | `UP`, `DOWN` | Direction to move the active vertex group towards |

#### `object.vertex_group_smooth` - Smooth Vertex Weights

*Smooth weights for selected vertices* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_smooth(group_select_mode='' (dynamic), factor=0.5, repeat=1, expand=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `group_select_mode` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | Define which subset of groups shall be used |
| `factor` | FLOAT | `0.5` | [0, 1] |  |
| `repeat` | INT | `1` | [1, 10000] soft [1, 200] |  |
| `expand` | FLOAT | `0` | [-1, 1] | Expand/contract weights |

#### `object.vertex_group_sort` - Sort Vertex Groups

*Sort vertex groups* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_group_sort(sort_type='NAME')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `sort_type` | ENUM | `'NAME'` | `NAME`, `BONE_HIERARCHY` | Sort type |

`sort_type`: NAME or BONE_HIERARCHY.

#### `object.vertex_weight_copy` - Copy Active

*Copy weights from active to selected* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

No properties.

Edit-mode 'Vertex Weights' panel operators acting on the active vertex (needs BMesh select history).

#### `object.vertex_weight_delete` - Delete Weight

*Delete this weight from the vertex (disabled if vertex group is locked)* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_weight_delete(weight_group=-1)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `weight_group` *(hidden)* | INT | `-1` | [-1, INT_MAX] | Index of source weight in active vertex group |

Edit-mode 'Vertex Weights' panel operators acting on the active vertex (needs BMesh select history).

#### `object.vertex_weight_normalize_active_vertex` - Normalize Active

*Normalize active vertex's weights* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

No properties.

Edit-mode 'Vertex Weights' panel operators acting on the active vertex (needs BMesh select history).

#### `object.vertex_weight_paste` - Paste Weight to Selected

*Copy this group's weight to other selected vertices (disabled if vertex group is locked)* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_weight_paste(weight_group=-1)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `weight_group` *(hidden)* | INT | `-1` | [-1, INT_MAX] | Index of source weight in active vertex group |

Edit-mode 'Vertex Weights' panel operators acting on the active vertex (needs BMesh select history).

#### `object.vertex_weight_set_active` - Set Active Group

*Set as active vertex group* | priority **P2** | poll A-E: `N N Y N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.vertex_weight_set_active(weight_group=-1)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `weight_group` *(hidden)* | INT | `-1` | [-1, INT_MAX] | Index of source weight in active vertex group |

Edit-mode 'Vertex Weights' panel operators acting on the active vertex (needs BMesh select history).

### 4.11 Shape keys

#### `object.shape_key_add` - Add Shape Key

*Add shape key to the object* | priority **P0** | poll A-E: `Y N N Y N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_add(from_mix=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `from_mix` | BOOLEAN | `True` |  | Create the new shape key from the existing mix of keys |

- First call creates the reference key **`Basis`**; later calls create `Key 1`, `Key 2`, ... (verified). The `Key` ID is named `Key` (`mesh.shape_keys.name`, `use_relative=True`). New key becomes active (`active_shape_key_index`).
- **`from_mix` defaults to True** (new key = current evaluated mix of all keys); `from_mix=False` copies the basis.
- Object mode only (poll False in edit mode, verified). glTF: non-basis keys -> morph targets (`export_morph`), key `value` -> default weights, animated `key_blocks["Key 1"].value` -> weights animation.

#### `object.shape_key_remove` - Remove Shape Key

*Remove shape key from the object* | priority **P0** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_remove(all=False, apply_mix=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `all` | BOOLEAN | `False` |  | Remove all shape keys |
| `apply_mix` | BOOLEAN | `False` |  | Apply current mix of shape keys to the geometry before removing them |

Remove the active key (`all=True` removes all; `apply_mix=True` applies the current mix to the mesh first). Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_clear` - Clear Shape Keys

*Reset the weights of all shape keys to 0 or to the closest value respecting the limits* | priority **P1** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Set all key values to 0.

#### `object.shape_key_retime` - Re-Time Shape Keys

*Resets the timing for absolute shape keys* | priority **OUT** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Absolute shape keys only.

#### `object.shape_key_mirror` - Mirror Shape Key

*Mirror the current shape key along the local X axis* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_mirror(use_topology=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_topology` | BOOLEAN | `False` |  | Use topology based mirroring (for when both sides of mesh have matching, unique topology) |

Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_move` - Move Shape Key

*Move selected shape keys up/down in the list* | priority **P1** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_move(type='TOP')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'TOP'` | `TOP`, `UP`, `DOWN`, `BOTTOM` |  |

`type`: TOP/UP/DOWN/BOTTOM (the basis can't be moved below index 0 semantics). Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_copy` - Duplicate Shape Key

*Duplicate the active shape key* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

New in 4.x/5.x: duplicate the active shape key. Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_lock` - Change the Lock On Shape Keys

*Change the lock state of all shape keys of active object* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_lock(action='LOCK')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `action` | ENUM | `'LOCK'` | `LOCK`, `UNLOCK` | Lock action to execute on vertex groups |

#### `object.shape_key_make_basis` - Make Shape Key the Basis Key

*Make this shape key the new basis key, effectively applying it to the mesh. Note that this applies the shape key at its 100% value* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

New in 5.x: make the active key the basis. Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_apply_to_basis` - Apply to Basis Key

*Apply deformations of selected shape keys to the basis key, removing them* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

New in 5.x: apply the active key's offset to the basis. Poll requires existing shape keys (N in all tested states for that reason).

#### `object.shape_key_transfer` - Transfer Shape Key

*Copy the active shape key of another selected object to this one* | priority **P2** | poll A-E: `Y N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.shape_key_transfer(mode='OFFSET', use_clamp=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mode` | ENUM | `'OFFSET'` | `OFFSET`, `RELATIVE_FACE`, `RELATIVE_EDGE` | Relative shape positions to the new shape method |
| `use_clamp` | BOOLEAN | `False` |  | Clamp the transformation to the distance each vertex moves in the original shape |

Copy the active shape from another selected object with same vertex count.

### 4.12 Collections, visibility, links, constraints

#### `object.hide_view_set` - Hide Objects

*Temporarily hide objects from the viewport* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.object.hide_view_set(unselected=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `unselected` *(hidden)* | BOOLEAN | `False` |  | Hide unselected rather than selected objects |

Needs a VIEW_3D area in Blender (V). Hides selected objects **in the view layer** (`obj.hide_get()`, set via `obj.hide_set(True)`), deselects them. `unselected=True` hides the others. Distinct from `hide_viewport` (global) and `hide_render`. glTF `use_visible` uses `visible_get()`.

#### `object.hide_view_clear` - Show Hidden Objects

*Reveal temporarily hidden objects* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.object.hide_view_clear(select=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `select` | BOOLEAN | `True` |  | Select revealed objects |

Reveal all hidden objects in the view layer; `select=True` selects the revealed ones.

#### `object.hide_render_clear_all` - Clear All Restrict Render

*Reveal all render objects by setting the hide render flag* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

No properties.

#### `object.move_to_collection` - Move to Collection

*Move objects to a collection* | priority **P1** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.move_to_collection(collection_uid=-1, is_new=False, new_collection_name='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection_uid` *(hidden)* | INT | `-1` | [-1, INT_MAX] soft [0, INT_MAX] | Session UID of the collection to move to |
| `is_new` *(hidden)* | BOOLEAN | `False` |  | Move objects to a new collection |
| `new_collection_name` | STRING | `""` |  | Name of the newly added collection |

Move selected objects to `collection_index` (index into a flattened collection tree; hidden INT) or a new collection (`is_new`, `new_collection_name`). Index semantics are awkward for agents — frap could additionally accept a collection name (extension) but keep Blender's prop names.

#### `object.link_to_collection` - Link to Collection

*Link objects to a collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.link_to_collection(collection_uid=-1, is_new=False, new_collection_name='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection_uid` *(hidden)* | INT | `-1` | [-1, INT_MAX] soft [0, INT_MAX] | Session UID of the collection to link to |
| `is_new` *(hidden)* | BOOLEAN | `False` |  | Link objects to a new collection |
| `new_collection_name` | STRING | `""` |  | Name of the newly added collection |

Like move_to_collection but links (object in several collections).

#### `object.collection_add` - Add to Collection

*Add an object to a new collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

Add active object to a new collection (not linked to the scene).

#### `object.collection_remove` - Remove Collection

*Remove the active object from this collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.collection_link` - Link to Collection

*Add an object to an existing collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.collection_link(collection='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |

#### `object.collection_unlink` - Unlink Collection

*Unlink the collection from all objects* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

#### `object.make_single_user` - Make Single User

*Make linked data local to each object* | priority **P1** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.make_single_user(type='SELECTED_OBJECTS', object=False, obdata=False, material=False, animation=False, obdata_animation=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'SELECTED_OBJECTS'` | `SELECTED_OBJECTS`, `ALL` |  |
| `object` | BOOLEAN | `False` |  | Make single user objects |
| `obdata` | BOOLEAN | `False` |  | Make single user object data |
| `material` | BOOLEAN | `False` |  | Make materials local to each data-block |
| `animation` | BOOLEAN | `False` |  | Make object animation data local to each object |
| `obdata_animation` | BOOLEAN | `False` |  | Make object data (mesh, curve etc.) animation data local to each object |

Make data single-user (`object`, `obdata`, `material`, `animation`, `obdata_animation` toggles; `type` SELECTED_OBJECTS/ALL). Needed before `transform_apply` on linked duplicates.

#### `object.make_links_data` - Link/Transfer Data

*Transfer data from active object to selected objects* | priority **P1** | poll A-E: `Y N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.make_links_data(type='OBDATA')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'OBDATA'` | `OBDATA`, `MATERIAL`, `ANIMATION`, `GROUPS`, `DUPLICOLLECTION`, `FONTS`, `MODIFIERS`, `CONSTRAINTS`, `EFFECTS`, `LIGHT_LINKING`, `SHADOW_LINKING` |  |

Ctrl+L: copy links from active to selected: `OBDATA` (share mesh), `MATERIAL` (copy material slots), `ANIMATION` (share action), `MODIFIERS` (copy modifier stack), `GROUPS` (collections). Poll requires active object selected? (N in state B).

#### `object.make_local` - Make Local

*Make library linked data-blocks local to this file* | priority **OUT** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.make_local(type='SELECT_OBJECT')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'SELECT_OBJECT'` | `SELECT_OBJECT`, `SELECT_OBDATA`, `SELECT_OBDATA_MATERIAL`, `ALL` |  |

Library linking — not applicable.

#### `object.track_set` - Make Track

*Make the object track another object, using various methods/constraints* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.track_set(type='DAMPTRACK')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'DAMPTRACK'` | `DAMPTRACK`, `TRACKTO`, `LOCKTRACK` |  |

Add a tracking constraint (DAMPTRACK/TRACKTO/LOCKTRACK) on selected objects targeting the active. Constraints are not glTF (exporter only bakes them with `export_bake_animation`/sampling).

#### `object.track_clear` - Clear Track

*Clear tracking constraint or flag from object* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.object.track_clear(type='CLEAR')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'CLEAR'` | `CLEAR`, `CLEAR_KEEP_TRANSFORM` |  |

#### `object.constraint_add` - Add Constraint

*Add a constraint to the active object* | priority **P2** | poll A-E: `Y N Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.object.constraint_add(type='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |

`type` dynamic enum of constraint types (COPY_LOCATION, TRACK_TO, CHILD_OF, IK, ...). Mostly OUT except for baking.

#### `collection.create` - Create New Collection

*Create an object collection from selected objects* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.collection.create(name='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `name` | STRING | `""` |  | Name of the new collection |

Create a collection from selected objects; the collection is **not linked to the scene** (orphan with fake user? uncertain).

#### `collection.objects_add_active` - Add Selected to Active Object's Collection

*Add selected objects to one of the collections the active-object is part of. Optionally add to "All Collections" to ensure selected objects are included in the same collections as the active object* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.collection.objects_add_active(collection='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | The collection to add other selected objects to |

#### `collection.objects_remove` - Remove from Collection

*Remove selected objects from a collection* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.collection.objects_remove(collection='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | The collection to remove this object from |

#### `collection.objects_remove_all` - Remove from All Collections

*Remove selected objects from all collections* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

No properties.

#### `collection.objects_remove_active` - Remove Selected from Active Collection

*Remove the object from an object collection that contains the active object* | priority **P2** | poll A-E: `Y Y N N N` | bl_options: REGISTER, UNDO

`bpy.ops.collection.objects_remove_active(collection='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `collection` | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) | The collection to remove other selected objects from |

### 4.13 Mesh primitives (`mesh.primitive_*_add`)

**Common semantics of all `mesh.primitive_*_add` (verified on 5.2.4):**
- Object mode: creates a new mesh object **and** mesh datablock with the same name (`Plane`, `Cube`, `Circle`, `Sphere` (uv_sphere), `Icosphere`, `Cylinder`, `Cone`, `Torus`, `Grid`, `Suzanne` (monkey)); collisions get `.001`. Linked into the **active collection** (`context.collection` = `view_layer.active_layer_collection.collection`; factory: `Collection`). New object becomes the only selected object and the active one. Mode stays OBJECT unless `enter_editmode=True` (then EDIT_MESH with all new geometry selected).
- **Edit mode**: no new object is created; the geometry is added to the edit mesh of the active object (transformed into that object's local space), the new elements are selected and all previously selected elements are deselected (verified: cube 8 verts + uv sphere -> 490 verts, 482 selected, object count unchanged).
- `location`: **if not explicitly set, the 3D cursor location is used** (verified with cursor (1,2,3)). Explicitly passed -> used as-is. frap's operator registry therefore needs Blender's `RNA_struct_property_is_set` semantics (*"was this arg passed?"*), not just default values.
- `rotation` (radians, Euler XYZ): if not set -> from `align`: `WORLD` (0,0,0); `CURSOR` = cursor rotation (verified (0,0,0.7854)); `VIEW` = viewport rotation (with no 3D view: identity; with the factory viewport: (1.1088,0,1.1549)). If `rotation` is set it wins (align is then ignored except VIEW in UI).
- `scale` (hidden, default `(0,0,0)` = "unset" sentinel): **baked into the mesh vertices; object scale stays (1,1,1)** (verified scale=(2,2,2) -> dims 4, `object.scale==(1,1,1)`). Same for `size`/`radius`: dimensions live in the mesh.
- `calc_uvs=True` creates a UV map named **`UVMap`** (torus uses `generate_uvs`).
- Faces are created **flat** (`sharp_face` attribute = True). No material slots are created.
- Mesh element order (vertex index order) is defined by Blender's `bmo_primitive.cc` / Python torus code; goldens comparing positions *by index* require replicating it exactly. **Suzanne is a hard-coded vertex table in GPL code** — copying it into frap has licensing implications (frap would need to be GPL-compatible or ship its own data captured from a Blender export, which is still Blender-derived). Flag to the architect.
- `size`/`radius` have hard min 0.0 (values below are clamped). Oddity: `size=-5` produced dims (1,1,1) (verified, unexplained — goldens should not rely on it; frap may simply reject negatives).

#### `mesh.primitive_plane_add` - Add Plane

*Construct a filled planar mesh with 4 vertices* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_plane_add(size=2, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `size` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified defaults: 4 v / 4 e / 1 f; dims (2,2,0). Lies in XY plane, normal +Z.

#### `mesh.primitive_cube_add` - Add Cube

*Construct a cube mesh that consists of six square faces* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_cube_add(size=2, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `size` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified: 8 v / 12 e / 6 f; dims (2,2,2) for `size=2`.

#### `mesh.primitive_circle_add` - Add Circle

*Construct a circle mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_circle_add(vertices=32, radius=1, fill_type='NOTHING', calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `vertices` | INT | `32` | [3, 10000000] soft [3, 500] |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `fill_type` | ENUM | `'NOTHING'` | `NOTHING`, `NGON`, `TRIFAN` |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified: `fill_type='NOTHING'` -> 32 v / 32 e / 0 f (wire, no faces => exports as glTF lines only with `use_mesh_edges`); `NGON` -> 1 face; `TRIFAN` -> center vertex + tris.

#### `mesh.primitive_uv_sphere_add` - Add UV Sphere

*Construct a spherical mesh with quad faces, except for triangle faces at the top and bottom* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, radius=1, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `segments` | INT | `32` | [3, 100000] soft [3, 500] |  |
| `ring_count` | INT | `16` | [3, 100000] soft [3, 500] |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified defaults (segments 32, ring_count 16): 482 v / 992 e / 512 f (tris at poles).

#### `mesh.primitive_ico_sphere_add` - Add Ico Sphere

*Construct a spherical mesh that consists of equally sized triangles* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `subdivisions` | INT | `2` | [1, 10] soft [1, 8] |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified subdivisions=2: 42 v / 120 e / 80 f; dims (1.9021, 2, 2) (not axis-symmetric). Max subdivisions 10.

#### `mesh.primitive_cylinder_add` - Add Cylinder

*Construct a cylinder mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_cylinder_add(vertices=32, radius=1, depth=2, end_fill_type='NGON', calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `vertices` | INT | `32` | [3, 10000000] soft [3, 500] |  |
| `radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `depth` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `end_fill_type` | ENUM | `'NGON'` | `NOTHING`, `NGON`, `TRIFAN` |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified: 64 v / 96 e / 34 f (NGON caps). `end_fill_type`: NOTHING/NGON/TRIFAN.

#### `mesh.primitive_cone_add` - Add Cone

*Construct a conic mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_cone_add(vertices=32, radius1=1, radius2=0, depth=2, end_fill_type='NGON', calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `vertices` | INT | `32` | [3, 10000000] soft [3, 500] |  |
| `radius1` | FLOAT DISTANCE (LENGTH) | `1` | [0, 1e12] soft [0.001, 100] |  |
| `radius2` | FLOAT DISTANCE (LENGTH) | `0` | [0, 1e12] soft [0, 100] |  |
| `depth` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `end_fill_type` | ENUM | `'NGON'` | `NOTHING`, `NGON`, `TRIFAN` |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified: 33 v / 64 e / 33 f (radius2=0 -> apex vertex).

#### `mesh.primitive_torus_add` - Add Torus

*Construct a torus mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: PRESET, REGISTER, UNDO

`bpy.ops.mesh.primitive_torus_add(align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), major_segments=48, minor_segments=12, mode='MAJOR_MINOR', major_radius=1, minor_radius=0.25, abso_major_rad=1.25, abso_minor_rad=0.75, generate_uvs=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` |  |
| `location` | FLOAT[3] TRANSLATION (LENGTH) | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `major_segments` | INT | `48` | [3, 256] | Number of segments for the main ring of the torus |
| `minor_segments` | INT | `12` | [3, 256] | Number of segments for the minor ring of the torus |
| `mode` | ENUM | `'MAJOR_MINOR'` | `MAJOR_MINOR`, `EXT_INT` |  |
| `major_radius` | FLOAT DISTANCE (LENGTH) | `1` | [0, 10000] soft [0, 100] | Radius from the origin to the center of the cross sections |
| `minor_radius` | FLOAT DISTANCE (LENGTH) | `0.25` | [0, 10000] soft [0, 100] | Radius of the torus's cross section |
| `abso_major_rad` | FLOAT DISTANCE (LENGTH) | `1.25` | [0, 10000] soft [0, 100] | Total Exterior Radius of the torus |
| `abso_minor_rad` | FLOAT DISTANCE (LENGTH) | `0.75` | [0, 10000] soft [0, 100] | Total Interior Radius of the torus |
| `generate_uvs` | BOOLEAN | `True` |  | Generate a default UV map |

**Python operator** (bl_options PRESET). Verified: 576 v / 1152 e / 576 f; dims (2.5,2.5,0.5). `mode` MAJOR_MINOR vs EXT_INT (`abso_major_rad`/`abso_minor_rad`). Note: UV toggle is `generate_uvs`, not `calc_uvs`. Attribute order differs from C primitives (built via from_pydata).

#### `mesh.primitive_grid_add` - Add Grid

*Construct a subdivided plane mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `x_subdivisions` | INT | `10` | [1, 10000000] soft [1, 1000] |  |
| `y_subdivisions` | INT | `10` | [1, 10000000] soft [1, 1000] |  |
| `size` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified x/y_subdivisions=10 -> 121 v / 220 e / 100 f, i.e. *subdivisions = faces per side* (changed in 3.0; old meaning was vertices per side).

#### `mesh.primitive_monkey_add` - Add Monkey

*Construct a Suzanne mesh* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.mesh.primitive_monkey_add(size=2, calc_uvs=True, enter_editmode=False, align='WORLD', location=(0, 0, 0), rotation=(0, 0, 0), scale=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `size` | FLOAT DISTANCE (LENGTH) | `2` | [0, 1e12] soft [0.001, 100] |  |
| `calc_uvs` | BOOLEAN | `True` |  | Generate a default UV map |
| `enter_editmode` *(hidden)* | BOOLEAN | `False` |  | Enter edit mode when adding this object |
| `align` | ENUM | `'WORLD'` | `WORLD`, `VIEW`, `CURSOR` | The alignment of the new object |
| `location` | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Location for the newly added object |
| `rotation` | FLOAT[3] EULER (ROTATION) | `(0, 0, 0)` | [-1e12, 1e12] soft [-6.28318, 6.28318] | Rotation for the newly added object |
| `scale` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-1e12, 1e12] soft [-1000, 1000] | Scale for the newly added object |

Verified: 507 v / 1005 e / 500 f; dims (2.7344, 1.7031, 1.9688). See licensing note.

### 4.14 Transform (`transform.*`)

**Common semantics of `transform.*` (modal operators executed non-interactively when `value` is given):**
- Object mode: acts on all selected objects (a child whose parent is also selected is moved only through its parent). Edit mode: acts on selected elements (verts/edges/faces -> their verts) of all objects in edit mode; `value` is in **world space** and converted to each object's local space (verified: translate (0,0,1) on an object scaled 2 -> local +0.5).
- Pivot for rotate/resize/mirror = `scene.tool_settings.transform_pivot_point` (`MEDIAN_POINT` default, `BOUNDING_BOX_CENTER`, `CURSOR`, `INDIVIDUAL_ORIGINS`, `ACTIVE_ELEMENT`) unless hidden `center_override` is set. Verified multi-object resize scales positions about the median.
- `orient_type`: `GLOBAL` (default), `LOCAL`, `NORMAL`, `GIMBAL`, `VIEW`, `CURSOR`, `PARENT`, or a custom orientation name. Axis constraint via `constraint_axis` (bool[3]) in that orientation.
- **Non-uniform resize/rotate in GLOBAL orientation on a rotated object** produces a matrix that is then decomposed back into loc/rot/scale (shear is lost): verified resize (2,1,1) on a 0.5 rad Z-rotated cube gave scale (1.8195, 1.2998, 1.0). frap must replicate Blender's `mat4 -> loc/rot/size` decomposition (`mat3_normalized_to_eul` + `mat3_to_size`) to match goldens.
- `transform.rotate(value=θ, orient_axis='Z')` adds **+θ** about +Z (verified rotation_euler.z == +0.5; no sign inversion in 5.2).
- Proportional editing (`use_proportional_edit`, `proportional_edit_falloff`, `proportional_size`, `use_proportional_connected`) is edit-mode falloff — P2.
- Snapping props (`snap*`) are UI/modal; ignore (accept and no-op or reject when True).
- Edit-mode transforms of meshes with mirror options (`mesh.use_mirror_x` etc.) and `use_automerge_and_split` are P2.

#### `transform.translate` - Move

*Move selected items* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.translate(value=(0, 0, 0), orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), snap_align=False, snap_normal=(0, 0, 0), gpencil_strokes=False, cursor_transform=False, texture_space=False, remove_on_cancel=False, use_duplicated_keyframes=False, view2d_edge_pan=False, release_confirm=False, use_accurate=False, use_automerge_and_split=False, translate_origin=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT[3] TRANSLATION (LENGTH) | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `orient_type` | ENUM | `'GLOBAL'` | `GLOBAL`, `LOCAL`, `NORMAL`, `GIMBAL`, `VIEW`, `CURSOR`, `PARENT` | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | `GLOBAL`, `LOCAL`, `NORMAL`, `GIMBAL`, `VIEW`, `CURSOR`, `PARENT` |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `snap_align` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_normal` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `cursor_transform` *(hidden)* | BOOLEAN | `False` |  |  |
| `texture_space` *(hidden)* | BOOLEAN | `False` |  | Edit object data texture space |
| `remove_on_cancel` *(hidden)* | BOOLEAN | `False` |  | Remove elements on cancel |
| `use_duplicated_keyframes` *(hidden)* | BOOLEAN | `False` |  | Transform duplicated keyframes |
| `view2d_edge_pan` *(hidden)* | BOOLEAN | `False` |  | Enable edge panning in 2D view |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |
| `use_automerge_and_split` *(hidden)* | BOOLEAN | `False` |  | Forces the use of Auto Merge and Split |
| `translate_origin` *(hidden)* | BOOLEAN | `False` |  | Translate origin instead of selection |

`value` FLOAT[3] (translation). In edit mode, also used by macro ops like `mesh.extrude_region_move` (`TRANSFORM_OT_translate={...}`).

#### `transform.rotate` - Rotate

*Rotate selected items* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.rotate(value=0, orient_axis='Z', orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), gpencil_strokes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT ANGLE (ROTATION) | `0` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] |  |
| `orient_axis` | ENUM | `'Z'` | `X`, `Y`, `Z` |  |
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

`value` angle (radians) about `orient_axis` (X/Y/Z) of `orient_type`.

#### `transform.resize` - Resize

*Scale (resize) selected items* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.resize(value=(1, 1, 1), mouse_dir_constraint=(0, 0, 0), orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), gpencil_strokes=False, texture_space=False, remove_on_cancel=False, use_duplicated_keyframes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT[3] XYZ | `(1, 1, 1)` | [-FLT_MAX, FLT_MAX] |  |
| `mouse_dir_constraint` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `texture_space` *(hidden)* | BOOLEAN | `False` |  | Edit object data texture space |
| `remove_on_cancel` *(hidden)* | BOOLEAN | `False` |  | Remove elements on cancel |
| `use_duplicated_keyframes` *(hidden)* | BOOLEAN | `False` |  | Transform duplicated keyframes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

`value` FLOAT[3] scale factors about the pivot.

#### `transform.mirror` - Mirror

*Mirror selected items around one or more axes* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.mirror(orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), gpencil_strokes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Scale -1 along constrained axes about the pivot (verified: X mirror of an object -> location mirrored about median, `scale.x` negated). Requires `constraint_axis`; with none it is a no-op.

#### `transform.transform` - Transform

*Transform selected items by mode type* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.transform(mode='TRANSLATION', value=(0, 0, 0, 0), orient_axis='Z', orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), snap_align=False, snap_normal=(0, 0, 0), gpencil_strokes=False, texture_space=False, remove_on_cancel=False, use_duplicated_keyframes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False, use_automerge_and_split=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `mode` *(hidden)* | ENUM | `'TRANSLATION'` | `INIT`, `DUMMY`, `TRANSLATION`, `ROTATION`, `RESIZE`, `SKIN_RESIZE`, `TOSPHERE`, `SHEAR`, `BEND`, `SHRINKFATTEN`, `TILT`, `TRACKBALL`, `PUSHPULL`, `CREASE`, `VERTEX_CREASE`, `MIRROR`, `BONE_SIZE`, `BONE_ENVELOPE`, `BONE_ENVELOPE_DIST`, `CURVE_SHRINKFATTEN`, `MASK_SHRINKFATTEN`, `BONE_ROLL`, `TIME_TRANSLATE`, `TIME_SLIDE`, `TIME_SCALE`, `TIME_EXTEND`, `BAKE_TIME`, `BWEIGHT`, `ALIGN`, `EDGESLIDE`, `SEQSLIDE`, `GPENCIL_OPACITY` |  |
| `value` | FLOAT[4] XYZ | `(0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `orient_axis` | ENUM | `'Z'` | `X`, `Y`, `Z` |  |
| `orient_type` | ENUM | `'GLOBAL'` | `GLOBAL`, `LOCAL`, `NORMAL`, `GIMBAL`, `VIEW`, `CURSOR`, `PARENT` | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | `GLOBAL`, `LOCAL`, `NORMAL`, `GIMBAL`, `VIEW`, `CURSOR`, `PARENT` |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `snap_align` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_normal` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `texture_space` *(hidden)* | BOOLEAN | `False` |  | Edit object data texture space |
| `remove_on_cancel` *(hidden)* | BOOLEAN | `False` |  | Remove elements on cancel |
| `use_duplicated_keyframes` *(hidden)* | BOOLEAN | `False` |  | Transform duplicated keyframes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |
| `use_automerge_and_split` *(hidden)* | BOOLEAN | `False` |  | Forces the use of Auto Merge and Split |

Generic: `mode` (TRANSLATION, ROTATION, RESIZE, SHEAR, TOSPHERE, BEND, SHRINKFATTEN, TILT, TRACKBALL, PUSHPULL, CREASE, MIRROR, BONE_SIZE, BONE_ENVELOPE, EDGESLIDE, VERTSLIDE, ...), `value` FLOAT[4] interpreted per mode (verified TRANSLATION with (1,0,0,0) works).

#### `transform.shear` - Shear

*Shear selected items along the given axis* | priority **P2** | poll A-E: `V V V V V` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.shear(angle=0, orient_axis='Z', orient_axis_ortho='X', orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, gpencil_strokes=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `angle` | FLOAT ANGLE (ROTATION) | `0` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] |  |
| `orient_axis` | ENUM | `'Z'` | `X`, `Y`, `Z` |  |
| `orient_axis_ortho` | ENUM | `'X'` | `X`, `Y`, `Z` |  |
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Needs VIEW_3D in Blender (V).

#### `transform.tosphere` - To Sphere

*Move selected items outward in a spherical shape around geometric center* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.tosphere(value=0, mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, gpencil_strokes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [0, 1] |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode mostly: blend selection toward a sphere (`value` 0..1).

#### `transform.bend` - Bend

*Bend selected items between the 3D cursor and the mouse* | priority **P2** | poll A-E: `V V V V V` | bl_options: BLOCKING, DEPENDS_ON_CURSOR, REGISTER, UNDO

`bpy.ops.transform.bend(value=(0), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, gpencil_strokes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT[1] ANGLE (ROTATION) | `(0)` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Needs VIEW_3D + cursor (V).

#### `transform.push_pull` - Push/Pull

*Push/Pull selected items* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.push_pull(value=0, mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT | `0` | [-FLT_MAX, FLT_MAX] |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

#### `transform.shrink_fatten` - Shrink/Fatten

*Shrink/fatten selected vertices along normals* | priority **P1** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.shrink_fatten(value=0, use_even_offset=False, mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT DISTANCE (LENGTH) | `0` | [-FLT_MAX, FLT_MAX] |  |
| `use_even_offset` | BOOLEAN | `False` |  | Scale the offset to give more even thickness |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode: move along vertex normals (`value` distance, `use_even_offset`).

#### `transform.trackball` - Trackball

*Trackball style rotation of selected items* | priority **OUT** | poll A-E: `Y Y Y Y Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.trackball(value=(0, 0), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, gpencil_strokes=False, center_override=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT[2] ANGLE (ROTATION) | `(0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `gpencil_strokes` *(hidden)* | BOOLEAN | `False` |  | Edit selected Grease Pencil strokes |
| `center_override` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [-10000, 10000] | Force using this center value (when set) |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

#### `transform.edge_slide` - Edge Slide

*Slide an edge loop along a mesh* | priority **P1** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.edge_slide(value=0, single_side=False, use_even=False, flipped=False, use_clamp=True, mirror=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), correct_uv=True, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [-10, 10] soft [-1, 1] |  |
| `single_side` *(hidden)* | BOOLEAN | `False` |  |  |
| `use_even` | BOOLEAN | `False` |  | Make the edge loop match the shape of the adjacent edge loop |
| `flipped` | BOOLEAN | `False` |  | When Even mode is active, flips between the two adjacent edge loops |
| `use_clamp` | BOOLEAN | `True` |  | Clamp within the edge extents |
| `mirror` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `correct_uv` | BOOLEAN | `True` |  | Correct UV coordinates when transforming |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode: slide selected edge loop along adjacent edges (`value` -1..1). Used by `mesh.loopcut_slide` macro (`TRANSFORM_OT_edge_slide`).

#### `transform.vert_slide` - Vertex Slide

*Slide a vertex along a mesh* | priority **P2** | poll A-E: `N N Y N N` | bl_options: BLOCKING, DEPENDS_ON_CURSOR, REGISTER, UNDO

`bpy.ops.transform.vert_slide(value=0, use_even=False, flipped=False, use_clamp=True, direction=(0, 0, 0), mirror=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), correct_uv=True, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [-10, 10] soft [-1, 1] |  |
| `use_even` | BOOLEAN | `False` |  | Make the edge loop match the shape of the adjacent edge loop |
| `flipped` | BOOLEAN | `False` |  | When Even mode is active, flips between the two adjacent edge loops |
| `use_clamp` | BOOLEAN | `True` |  | Clamp within the edge extents |
| `direction` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] | World-space direction |
| `mirror` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `correct_uv` | BOOLEAN | `True` |  | Correct UV coordinates when transforming |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode (V: depends on cursor).

#### `transform.edge_crease` - Edge Crease

*Change the crease of edges* | priority **P1** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.edge_crease(value=0, snap=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [-1, 1] |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode: add `value` (-1..1, relative) to the `crease_edge` attribute of selected edges (Subdivision modifier creases).

#### `transform.vert_crease` - Vertex Crease

*Change the crease of vertices* | priority **P2** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.vert_crease(value=0, snap=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [-1, 1] |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

`crease_vert` attribute.

#### `transform.edge_bevelweight` - Edge Bevel Weight

*Change the bevel weight of edges* | priority **P1** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.edge_bevelweight(value=0, snap=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT FACTOR | `0` | [-1, 1] |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Edit mode: `bevel_weight_edge` attribute (Bevel modifier `limit_method='WEIGHT'`).

#### `transform.vertex_random` - Randomize

*Randomize vertices* | priority **P2** | poll A-E: `N N Y N Y` | bl_options: REGISTER, UNDO

`bpy.ops.transform.vertex_random(offset=0, uniform=0, normal=0, seed=0, wait_for_input=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `offset` | FLOAT DISTANCE (LENGTH) | `0` | [-FLT_MAX, FLT_MAX] soft [-10, 10] | Distance to offset |
| `uniform` | FLOAT FACTOR | `0` | [0, 1] | Increase for uniform offset distance |
| `normal` | FLOAT FACTOR | `0` | [0, 1] | Align offset direction to normals |
| `seed` | INT | `0` | [0, 10000] soft [0, 50] | Seed for the random number generator |
| `wait_for_input` *(hidden)* | BOOLEAN | `True` |  |  |

Edit mode: randomize vertices (`offset`, `uniform`, `normal`, `seed`). Exact RNG replication impossible without porting Blender's RNG — goldens should skip.

#### `transform.vertex_warp` - Warp

*Warp vertices around the cursor* | priority **OUT** | poll A-E: `N N Y N Y` | bl_options: REGISTER, UNDO

`bpy.ops.transform.vertex_warp(warp_angle=6.28318, offset_angle=0, min=-1, max=1, viewmat=(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0), center=(0, 0, 0))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `warp_angle` | FLOAT ANGLE (ROTATION) | `6.28318` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] | Amount to warp about the cursor |
| `offset_angle` | FLOAT ANGLE (ROTATION) | `0` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] | Angle to use as the basis for warping |
| `min` | FLOAT | `-1` | [-FLT_MAX, FLT_MAX] soft [-100, 100] |  |
| `max` | FLOAT | `1` | [-FLT_MAX, FLT_MAX] soft [-100, 100] |  |
| `viewmat` *(hidden)* | FLOAT[16] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `center` *(hidden)* | FLOAT[3] XYZ_LENGTH (LENGTH) | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |

#### `transform.rotate_normal` - Rotate Normals

*Rotate custom normal of selected items* | priority **P2** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.rotate_normal(value=0, orient_axis='Z', orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT ANGLE (ROTATION) | `0` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] |  |
| `orient_axis` | ENUM | `'Z'` | `X`, `Y`, `Z` |  |
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Custom split normals — related to `mesh.set_normals_from_faces` etc.

#### `transform.skin_resize` - Skin Resize

*Scale selected vertices' skin radii* | priority **OUT** | poll A-E: `N N Y N N` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.skin_resize(value=(1, 1, 1), orient_type='GLOBAL', orient_matrix=(0, 0, 0, 0, 0, 0, 0, 0, 0), orient_matrix_type='GLOBAL', constraint_axis=(False, False, False), mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, snap_elements={INCREMENT}, use_snap_project=False, snap_target='CLOSEST', use_snap_self=True, use_snap_edit=True, use_snap_nonedit=True, use_snap_selectable=False, snap_point=(0, 0, 0), release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT[3] XYZ | `(1, 1, 1)` | [-FLT_MAX, FLT_MAX] |  |
| `orient_type` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |
| `orient_matrix` *(hidden)* | FLOAT[9] MATRIX | `(0, 0, 0, 0, 0, 0, 0, 0, 0)` | [-FLT_MAX, FLT_MAX] soft [0, 0] |  |
| `orient_matrix_type` *(hidden)* | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) |  |
| `constraint_axis` | BOOLEAN[3] XYZ | `(False, False, False)` |  |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_elements` *(hidden)* | ENUM flag-set | `{INCREMENT}` | `INCREMENT`, `GRID`, `VERTEX`, `EDGE`, `FACE`, `VOLUME`, `EDGE_MIDPOINT`, `EDGE_PERPENDICULAR`, `FACE_MIDPOINT`, `FACE_PROJECT`, `FACE_NEAREST` |  |
| `use_snap_project` | BOOLEAN | `False` |  |  |
| `snap_target` *(hidden)* | ENUM | `'CLOSEST'` | `CLOSEST`, `CENTER`, `MEDIAN`, `ACTIVE` | Point on source that will snap to target |
| `use_snap_self` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_edit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_nonedit` *(hidden)* | BOOLEAN | `True` |  |  |
| `use_snap_selectable` *(hidden)* | BOOLEAN | `False` |  |  |
| `snap_point` *(hidden)* | FLOAT[3] XYZ | `(0, 0, 0)` | [-FLT_MAX, FLT_MAX] |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

#### `transform.tilt` - Tilt

*Tilt selected control vertices of 3D curve* | priority **OUT** | poll A-E: `N N Y N Y` | bl_options: BLOCKING, REGISTER, UNDO

`bpy.ops.transform.tilt(value=0, mirror=False, use_proportional_edit=False, proportional_edit_falloff='SMOOTH', proportional_size=1, use_proportional_connected=False, use_proportional_projected=False, snap=False, release_confirm=False, use_accurate=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `value` | FLOAT ANGLE (ROTATION) | `0` | [-FLT_MAX, FLT_MAX] soft [-6.28318, 6.28318] |  |
| `mirror` | BOOLEAN | `False` |  |  |
| `use_proportional_edit` | BOOLEAN | `False` |  |  |
| `proportional_edit_falloff` | ENUM | `'SMOOTH'` | `SMOOTH`, `SPHERE`, `ROOT`, `INVERSE_SQUARE`, `SHARP`, `LINEAR`, `CONSTANT`, `RANDOM` | Falloff type for proportional editing mode |
| `proportional_size` | FLOAT | `1` | [1e-06, 1e12] soft [0.001, 100] |  |
| `use_proportional_connected` | BOOLEAN | `False` |  |  |
| `use_proportional_projected` | BOOLEAN | `False` |  |  |
| `snap` *(hidden)* | BOOLEAN | `False` |  |  |
| `release_confirm` *(hidden)* | BOOLEAN | `False` |  | Always confirm operation when releasing button |
| `use_accurate` *(hidden)* | BOOLEAN | `False` |  | Use accurate transformation |

Curves only.

#### `transform.create_orientation` - Create Orientation

*Create transformation orientation from selection* | priority **P2** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.transform.create_orientation(name='', use_view=False, use=False, overwrite=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `name` | STRING | `""` |  | Name of the new custom orientation |
| `use_view` | BOOLEAN | `False` |  | Use the current view instead of the active object to create the new orientation |
| `use` | BOOLEAN | `False` |  | Select orientation after its creation |
| `overwrite` | BOOLEAN | `False` |  | Overwrite previously created orientation with same name |

Custom transform orientation from selection. Needs VIEW_3D (V).

#### `transform.select_orientation` - Select Orientation

*Select transformation orientation* | priority **P2** | poll A-E: `V V V V V` | bl_options: UNDO

`bpy.ops.transform.select_orientation(orientation='GLOBAL')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `orientation` | ENUM | `'GLOBAL'` | dynamic enum (items depend on context) | Transformation orientation |

Sets `scene.transform_orientation_slots[0].type`. Needs VIEW_3D (V).

### 4.15 3D cursor and snapping (`view3d.*`)

#### `view3d.snap_cursor_to_active` - Snap Cursor to Active

*Snap 3D cursor to the active item* | priority **P0** | poll A-E: `V V V V V` | bl_options: REGISTER

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Cursor -> active object origin (edit mode: active element).

#### `view3d.snap_cursor_to_center` - Snap Cursor to World Origin

*Snap 3D cursor to the world origin* | priority **P0** | poll A-E: `V V V V V` | bl_options: REGISTER

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Cursor location -> (0,0,0) **and rotation reset to identity** (verified rotation (0.3,0,0) -> (0,0,0)).

#### `view3d.snap_cursor_to_grid` - Snap Cursor to Grid

*Snap 3D cursor to the nearest grid division* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Round cursor location to the view grid (grid scale 1.0 by default, scaled by `unit_settings.scale_length`) — verified (0.4,0.6,0) -> (0,1,0).

#### `view3d.snap_cursor_to_selected` - Snap Cursor to Selected

*Snap 3D cursor to the middle of the selected item(s)* | priority **P0** | poll A-E: `V V V V V` | bl_options: REGISTER

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Cursor -> median (or bounding-box center if pivot = BOUNDING_BOX_CENTER) of selected objects' origins (object mode) or selected vertices in world space (edit mode). Verified (1,2,3).

#### `view3d.snap_selected_to_active` - Snap Selection to Active

*Snap selected item(s) to the active item* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists.

#### `view3d.snap_selected_to_cursor` - Snap Selection to Cursor

*Snap selected item(s) to the 3D cursor* | priority **P0** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.view3d.snap_selected_to_cursor(use_offset=True, use_rotation=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_offset` | BOOLEAN | `True` |  | If the selection should be snapped as a whole or by each object center |
| `use_rotation` | BOOLEAN | `False` |  | If the selection should be rotated to match the cursor |

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Move selected objects (or verts) to the cursor. `use_offset=True` keeps relative offsets (moves the selection median); `use_rotation` also copies cursor rotation (objects).

#### `view3d.snap_selected_to_grid` - Snap Selection to Grid

*Snap selected item(s) to their nearest grid division* | priority **P2** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists.

#### `view3d.camera_to_view_selected` - Camera Fit Frame to Selected

*Move the camera so selected objects are framed* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

No properties.

Poll works without a view. Moves the **scene camera** (keeps its rotation) so all selected objects fit in its frame — very useful for agents before `render.render`. Requires `scene.camera`.

#### `view3d.object_as_camera` - Set Active Object as Camera

*Set the active object as the active camera for this view or scene* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

No properties.

Poll needs a VIEW_3D area in Blender (`V`); frap is headless and should behave as if one exists. Sets the active object as `scene.camera` (frap: equivalent to `bpy.context.scene.camera = obj`).

### 4.16 Animation operators (`anim.*`, `screen.*`)

#### `anim.keyframe_insert` - Insert Keyframe

*Insert keyframes on the current frame using either the active keying set, or the user preferences if no keying set is active* | priority **P0** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_insert(type='DEFAULT')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT` | The Keying Set to use |

- Poll needs an *area* in Blender (V). Inserts keys on the **selected objects** (or selected pose bones in pose mode) at `scene.frame_current`.
- `type` = keying set (dynamic enum). In 5.x, with no `type` (DEFAULT), the channels come from **Preferences > Animation > Default Key Channels** (`preferences.edit.key_insert_channels`, factory = {LOCATION, ROTATION, SCALE, CUSTOM_PROPS}); verified: plain `anim.keyframe_insert()` keyed location, rotation_euler **and scale**. Built-in keying set ids: `Available`, `Location`, `Rotation`, `Scaling` (label "Scale"), `BUILTIN_KSI_LocRot`, `LocRotScale`, `LocRotScaleCProp`, `BUILTIN_KSI_LocScale`, `BUILTIN_KSI_RotScale`, `BUILTIN_KSI_DeltaLocation`, `BUILTIN_KSI_DeltaRotation`, `BUILTIN_KSI_DeltaScale`, `BUILTIN_KSI_VisualLoc`, `BUILTIN_KSI_VisualRot`, `BUILTIN_KSI_VisualScaling`, `BUILTIN_KSI_VisualLocRot`, `BUILTIN_KSI_VisualLocRotScale`, `BUILTIN_KSI_VisualLocScale`, `BUILTIN_KSI_VisualRotScale`, `BUILTIN_KSI_BendyBones`, `WholeCharacter`, `WholeCharacterSelected` (verified list).
- Rotation channel keyed depends on `rotation_mode` (`rotation_euler` / `rotation_quaternion` / `rotation_axis_angle`).
- Most scripts use the **method** `bpy_struct.keyframe_insert()` instead (see section 5.1).

#### `anim.keyframe_insert_by_name` - Insert Keyframe (by name)

*Alternate access to 'Insert Keyframe' for keymaps to use* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_insert_by_name(type='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` *(hidden)* | STRING | `""` |  | The Keying Set to use |

`type` = keying set bl_idname string (e.g. `'Location'`, `'LocRotScale'`, `'Scaling'`). V.

#### `anim.keyframe_insert_menu` - Insert Keyframe Menu

*Insert Keyframes for specified Keying Set, with menu of available Keying Sets if undefined* | priority **OUT** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_insert_menu(type='DEFAULT', always_prompt=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT` | The Keying Set to use |
| `always_prompt` *(hidden)* | BOOLEAN | `False` |  |  |

UI menu wrapper (`always_prompt`).

#### `anim.keyframe_delete` - Delete Keying-Set Keyframe

*Delete keyframes on the current frame for all properties in the specified Keying Set* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_delete(type='DEFAULT')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT` | The Keying Set to use |

Delete keys of the given keying set `type` at the current frame on selected objects. V.

#### `anim.keyframe_delete_by_name` - Delete Keying-Set Keyframe (by name)

*Alternate access to 'Delete Keyframe' for keymaps to use* | priority **P2** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_delete_by_name(type='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` *(hidden)* | STRING | `""` |  | The Keying Set to use |

V.

#### `anim.keyframe_delete_v3d` - Delete Keyframe

*Remove keyframes on current frame for selected objects and bones* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_delete_v3d(confirm=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `confirm` *(hidden)* | BOOLEAN | `True` |  | Prompt for confirmation |

Delete all keys at the current frame for selected objects (`confirm` hidden). V.

#### `anim.keyframe_clear_v3d` - Remove Animation

*Remove all keyframe animation for selected objects* | priority **P1** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keyframe_clear_v3d(confirm=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `confirm` *(hidden)* | BOOLEAN | `True` |  | Prompt for confirmation |

Remove **all** keyframes (and F-Curves) of selected objects. V.

#### `anim.keying_set_active_set` - Set Active Keying Set

*Set a new active keying set* | priority **P2** | poll A-E: `V V V V V` | bl_options: REGISTER, UNDO

`bpy.ops.anim.keying_set_active_set(type='DEFAULT')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'DEFAULT'` | `DEFAULT` | The Keying Set to use |

#### `anim.change_frame` - Change Frame

*Interactively change the current frame number* | priority **OUT** | poll A-E: `N N N N N` | bl_options: BLOCKING, GRAB_CURSOR_X, UNDO_GROUPED

`bpy.ops.anim.change_frame(frame=0, snap=False, seq_solo_preview=False, pass_through_on_strip_handles=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `frame` | FLOAT | `0` | [-1.04857e+06, 1.04857e+06] |  |
| `snap` | BOOLEAN | `False` |  |  |
| `seq_solo_preview` | BOOLEAN | `False` |  |  |
| `pass_through_on_strip_handles` | BOOLEAN | `False` |  | Allow another operator to operate on strip handles |

Needs an animation editor; use `scene.frame_set()` / `scene.frame_current`.

#### `anim.start_frame_set` - Set Start Frame

*Set the current frame as the preview or scene start frame* | priority **OUT** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Needs timeline; use `scene.frame_start = n`.

#### `anim.end_frame_set` - Set End Frame

*Set the current frame as the preview or scene end frame* | priority **OUT** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Needs timeline; use `scene.frame_end = n`.

#### `anim.channels_bake` - Bake Channels

*Create keyframes following the current shape of F-Curves of selected channels* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.anim.channels_bake(use_scene_range=True, range=(0, 0), step=1, remove_outside_range=False, interpolation_type='BEZIER', bake_modifiers=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_scene_range` | BOOLEAN | `True` |  | If enabled, the scene start and end frame will be used to determine the bake range |
| `range` | INT[2] | `(0, 0)` | [INT_MIN, INT_MAX] soft [0, INT_MAX] | The custom range in which to create new keys. Only used when not using the scene range |
| `step` | FLOAT | `1` | [0.01, FLT_MAX] soft [1, 16] | At which interval to add keys |
| `remove_outside_range` | BOOLEAN | `False` |  | Removes keys outside the given range, leaving only the newly baked |
| `interpolation_type` | ENUM | `'BEZIER'` | `BEZIER`, `LIN`, `CONST` | Choose the interpolation type with which new keys will be added |
| `bake_modifiers` | BOOLEAN | `True` |  | Bake Modifiers into keyframes and delete them after |

Bake F-Curves (needs anim editor context). For glTF the exporter samples anyway (`export_force_sampling`).

#### `screen.frame_jump` - Jump to Endpoint

*Jump to first/last frame in frame range* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: UNDO_GROUPED

`bpy.ops.screen.frame_jump(end=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `end` | BOOLEAN | `False` |  | Jump to the last frame of the frame range |

`end=False` -> `scene.frame_current = frame_start`; `end=True` -> `frame_end`. Re-evaluates animation. Works headless.

#### `screen.frame_offset` - Frame Offset

*Move current frame forward/backward by a given number* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: UNDO_GROUPED

`bpy.ops.screen.frame_offset(delta=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `delta` | INT | `0` | [INT_MIN, INT_MAX] |  |

`frame_current += delta`. Works headless.

#### `screen.keyframe_jump` - Jump to Keyframe

*Jump to previous/next keyframe* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: UNDO_GROUPED

`bpy.ops.screen.keyframe_jump(next=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `next` | BOOLEAN | `True` |  |  |

Jump to next/previous keyframe of selected/visible objects.

### 4.17 Render (`render.*`)

#### `render.render` - Render

*(undocumented operator)* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.render.render(animation=False, write_still=False, use_viewport=False, use_sequencer_scene=False, layer='', scene='', frame_start=0, frame_end=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `animation` | BOOLEAN | `False` |  | Render files from the animation range of this scene |
| `write_still` | BOOLEAN | `False` |  | Save the rendered image to the output path (used only when animation is disabled) |
| `use_viewport` | BOOLEAN | `False` |  | When inside a 3D viewport, use layers and camera of the viewport |
| `use_sequencer_scene` | BOOLEAN | `False` |  | Render the sequencer scene instead of the active scene |
| `layer` | STRING | `""` |  | Single render layer to re-render (used only when animation is disabled) |
| `scene` | STRING | `""` |  | Scene to render, current scene if not specified |
| `frame_start` | INT | `0` | [INT_MIN, INT_MAX] | Frame to start rendering animation at. If not specified, the scene start frame will be assumed. This should only be specified if doing an animation render |
| `frame_end` | INT | `0` | [INT_MIN, INT_MAX] | Frame to end rendering animation at. If not specified, the scene end frame will be assumed. This should only be specified if doing an animation render |

- Renders the scene from **`scene.camera`** with `scene.render.engine` (`BLENDER_EEVEE` = factory default in 5.2, `BLENDER_WORKBENCH`, `CYCLES`). Without a camera: `RuntimeError: Error: Cannot render, no camera` (verified).
- Result stored in `bpy.data.images['Render Result']` (verified it exists after render). Nothing written to disk unless `write_still=True` (stills) or `animation=True` (always writes every frame).
- Output path = `scene.render.filepath` (factory `/tmp/`, relative paths `//` relative to the saved file). Still: extension added when `render.use_file_extension` (default True): `.../shot` -> `shot.png` (verified). Animation: frame number appended zero-padded to 4 digits (`anim_` -> `anim_0001.png`, verified), or `#` runs are replaced with the zero-padded frame (`hash_###_x` -> `hash_001_x.png`, verified). A path ending with `/` gives `0001.png` in that directory.
- Frame range for `animation=True`: `scene.frame_start..frame_end` step `frame_step`; the `frame_start`/`frame_end` operator props override it when *specified* (RNA description: 'If not specified, the scene start frame will be assumed') — another `is_set` check.
- Format: `scene.render.image_settings.file_format` (factory `PNG`), `color_mode`, `color_depth`, `compression`; size `resolution_x/y` (1920x1080) * `resolution_percentage` (100); `film_transparent` (False); color management `scene.view_settings.view_transform` (factory **`AgX`**), `look`, `exposure`, `gamma`, `display_settings.display_device` (`sRGB`). The frap renderer must implement AgX (and `Standard`) view transforms to look like Blender.
- `use_viewport` (render using viewport settings), `layer`, `scene` (render another scene by name), `use_sequencer_scene`: P2.
- In background mode the op is synchronous (no INVOKE).

#### `render.opengl` - Viewport Render

*Take a snapshot of the active viewport* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.render.opengl(animation=False, render_keyed_only=False, sequencer=False, write_still=False, view_context=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `animation` | BOOLEAN | `False` |  | Render files from the animation range of this scene |
| `render_keyed_only` | BOOLEAN | `False` |  | Render only those frames where selected objects have a key in their animation data. Only used when rendering animation |
| `sequencer` | BOOLEAN | `False` |  | Render using the sequencer's OpenGL display |
| `write_still` | BOOLEAN | `False` |  | Save the rendered image to the output path (used only when animation is disabled) |
| `view_context` | BOOLEAN | `True` |  | Use the current 3D view for rendering, else use scene settings |

Viewport render (needs a 3D view for `view_context=True`). frap could map it to the Workbench engine from the scene camera.

### 4.18 Files and session (`wm.*`, `scene.*`, `ed.*`)

#### `wm.open_mainfile` - Open

*Open a Blender file* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.wm.open_mainfile(filepath='', hide_props_region=True, check_existing=False, filter_blender=True, filter_backup=False, filter_image=False, filter_movie=False, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=8, display_type='DEFAULT', sort_method='' (dynamic), load_ui=True, use_scripts=False, display_file_selector=True, state=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` | STRING FILE_PATH | `""` |  | Path to file |
| `hide_props_region` *(hidden)* | BOOLEAN | `True` |  | Collapse the region displaying the operator settings |
| `check_existing` *(hidden)* | BOOLEAN | `False` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `8` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |
| `load_ui` | BOOLEAN | `True` |  | Load user interface setup in the .blend file |
| `use_scripts` | BOOLEAN | `False` |  | Allow .blend file to execute scripts automatically, default available from system preferences |
| `display_file_selector` | BOOLEAN | `True` |  |  |
| `state` *(hidden)* | INT | `0` | [INT_MIN, INT_MAX] |  |

Replace the whole session state with the file (`load_ui` irrelevant headless; `use_scripts` irrelevant). Selection/active/mode come from the file (mode is saved per object; Blender re-enters edit mode on load if saved in edit mode). Verified `bpy.data.filepath` updated.

#### `wm.save_as_mainfile` - Save As

*Save the current file in the desired location* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.wm.save_as_mainfile(filepath='', hide_props_region=True, check_existing=True, filter_blender=True, filter_backup=False, filter_image=False, filter_movie=False, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=8, display_type='DEFAULT', sort_method='' (dynamic), compress=False, relative_remap=True, copy=False, show_save_modified_images_dialog=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` | STRING FILE_PATH | `""` |  | Path to file |
| `hide_props_region` *(hidden)* | BOOLEAN | `True` |  | Collapse the region displaying the operator settings |
| `check_existing` *(hidden)* | BOOLEAN | `True` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `8` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |
| `compress` | BOOLEAN | `False` |  | Write compressed .blend file |
| `relative_remap` | BOOLEAN | `True` |  | Remap relative paths when saving to a different directory |
| `copy` | BOOLEAN | `False` |  | Save a copy of the actual working state but does not make saved file active |
| `show_save_modified_images_dialog` *(hidden)* | BOOLEAN | `False` |  | Show a popup dialog to save modified images before saving the blend file |

- Saves to `filepath` (for frap: `.frap` container). After success `bpy.data.filepath` = the new path and `bpy.data.is_dirty == False` (verified), unless `copy=True` (save a copy; current file path unchanged).
- `compress`, `relative_remap` (remap relative paths of images when the directory changes) are meaningful for frap's packed/linked images. `check_existing` is UI-only.
- Info report: `Saved as "x.blend"` (verified).
- frap decision: should `.blend` be refused with a clear error? (Blender would write a .blend; frap cannot.)

#### `wm.save_mainfile` - Save Blender File

*Save the current Blender file* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.wm.save_mainfile(filepath='', hide_props_region=True, check_existing=True, filter_blender=True, filter_backup=False, filter_image=False, filter_movie=False, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=8, display_type='DEFAULT', sort_method='' (dynamic), compress=False, relative_remap=False, exit=False, incremental=False, show_save_modified_images_dialog=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` | STRING FILE_PATH | `""` |  | Path to file |
| `hide_props_region` *(hidden)* | BOOLEAN | `True` |  | Collapse the region displaying the operator settings |
| `check_existing` *(hidden)* | BOOLEAN | `True` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `8` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |
| `compress` | BOOLEAN | `False` |  | Write compressed .blend file |
| `relative_remap` | BOOLEAN | `False` |  | Remap relative paths when saving to a different directory |
| `exit` *(hidden)* | BOOLEAN | `False` |  | Exit Blender after saving |
| `incremental` *(hidden)* | BOOLEAN | `False` |  | Save the current Blender file with a numerically incremented name that does not overwrite any existing files |
| `show_save_modified_images_dialog` *(hidden)* | BOOLEAN | `False` |  | Show a popup dialog to save modified images before saving the blend file |

Save to the current `bpy.data.filepath` (or `filepath` if given). Error if the file was never saved and no path given. Hidden `exit`, `incremental` props.

#### `wm.read_homefile` - Reload Start-Up File

*Open the default file* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.wm.read_homefile(filepath='', load_ui=True, use_splash=False, use_factory_startup=False, use_factory_startup_app_template_only=False, app_template='Template', use_empty=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` *(hidden)* | STRING FILE_PATH | `""` |  | Path to an alternative start-up file |
| `load_ui` *(hidden)* | BOOLEAN | `True` |  | Load user interface setup from the .blend file |
| `use_splash` *(hidden)* | BOOLEAN | `False` |  |  |
| `use_factory_startup` *(hidden)* | BOOLEAN | `False` |  | Load the default ('factory startup') blend file. This is independent of the normal start-up file that the user can save |
| `use_factory_startup_app_template_only` *(hidden)* | BOOLEAN | `False` |  |  |
| `app_template` *(hidden)* | STRING | `"Template"` |  |  |
| `use_empty` *(hidden)* | BOOLEAN | `False` |  | After loading, remove everything except scenes, windows, and workspaces. This makes it possible to load the startup file with its scene configuration and window layout intact, but no objects, materials, animations, ... |

- Loads the startup file. Factory startup content (verified): objects `Camera` (loc (7.3589,-6.9258,4.9583), rot (1.1093,0,0.8149)), `Cube` (mesh `Cube` with material `Material`), `Light` (POINT, 1000 W, loc (4.0762,1.0055,5.9039)); collection `Collection`; world `World` (Background node); materials `Material` and `Dots Stroke` (grease-pencil material); scene `Scene`, view layer `ViewLayer`; frame 1..250, fps 24, engine `BLENDER_EEVEE`, 1920x1080 100%, `filepath='/tmp/'`, units METRIC scale 1.0, cursor at origin. Active=Cube, selected=[Cube], mode OBJECT.
- Hidden `use_empty=True` gives an empty scene (no objects) — recommended frap default for agents? (decision). `use_factory_startup=True` ignores user startup.blend; frap has no user startup so it is always "factory".
- `bpy.data.filepath` becomes '' (unsaved).

#### `wm.read_factory_settings` - Load Factory Settings

*Load factory default startup file and preferences. To make changes permanent, use "Save Startup File" and "Save Preferences"* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: -

`bpy.ops.wm.read_factory_settings(use_factory_startup_app_template_only=False, app_template='Template', use_empty=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_factory_startup_app_template_only` *(hidden)* | BOOLEAN | `False` |  |  |
| `app_template` *(hidden)* | STRING | `"Template"` |  |  |
| `use_empty` *(hidden)* | BOOLEAN | `False` |  | After loading, remove everything except scenes, windows, and workspaces. This makes it possible to load the startup file with its scene configuration and window layout intact, but no objects, materials, animations, ... |

Like read_homefile with factory startup (and factory prefs). `use_empty`.

#### `wm.revert_mainfile` - Revert

*Reload the saved file* | priority **P2** | poll A-E: `N N N N N` | bl_options: -

`bpy.ops.wm.revert_mainfile(use_scripts=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `use_scripts` | BOOLEAN | `False` |  | Allow .blend file to execute scripts automatically, default available from system preferences |

Reload the saved file; poll requires a saved file.

#### `scene.new` - New Scene

*Add new scene by type* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.scene.new(type='NEW')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'NEW'` | `NEW`, `EMPTY`, `LINK_COPY`, `FULL_COPY` |  |

`type`: NEW (empty scene with defaults), EMPTY (copy settings only?), LINK_COPY, FULL_COPY. Becomes the window's active scene. Multi-scene matters for glTF (`scenes[]`, `use_active_scene`).

#### `scene.delete` - Delete Scene

*Delete active scene* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Delete active scene (needs window context; N headless).

#### `scene.view_layer_add` - Add View Layer

*Add a view layer* | priority **OUT** | poll A-E: `Y Y Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

`bpy.ops.scene.view_layer_add(type='NEW')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `type` | ENUM | `'NEW'` | `NEW`, `COPY`, `EMPTY` |  |

#### `scene.view_layer_remove` - Remove View Layer

*Remove the selected view layer* | priority **OUT** | poll A-E: `N N N N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

#### `ed.undo` - Undo

*Undo previous action* | priority **OUT** | poll A-E: `N N N N N` | bl_options: -

No properties.

Not required. Note: in background mode undo is disabled: poll message `Undo disabled at startup in background-mode (call ed.undo_push() to explicitly initialize the undo-system)` (verified).

#### `ed.redo` - Redo

*Redo previous action* | priority **OUT** | poll A-E: `N N N N N` | bl_options: -

No properties.

#### `ed.undo_push` - Undo Push

*Add an undo state (internal use only)* | priority **OUT** | poll A-E: `Y Y Y Y Y` | bl_options: INTERNAL

`bpy.ops.ed.undo_push(message='Add an undo step *function may be moved*')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `message` | STRING | `"Add an undo step *function may be moved*"` |  |  |

#### `ed.undo_history` - Undo History

*Undo or redo specific action in history* | priority **OUT** | poll A-E: `N N N N N` | bl_options: -

`bpy.ops.ed.undo_history(item=0)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `item` | INT | `0` | [0, INT_MAX] |  |

### 4.19 Data-block creation (`material.*`, `image.*`, `world.*`)

#### `material.new` - New Material

*Add a new material* | priority **P0** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

- Creates a new material `Material` / `Material.001` with a node tree **Principled BSDF + Material Output** (verified). In 5.x `Material.use_nodes` is deprecated (always node-based; to be removed in 6.0).
- In the UI it is invoked from a `template_ID` with a context pointer and assigns the new material to the active slot. **From a script (no context pointer) it does NOT assign it to any slot** (verified slot unchanged). Agents should prefer `bpy.data.materials.new(name)` + `obj.data.materials.append(mat)` / `obj.material_slots[i].material = mat`; frap must support those `bpy.data` / collection methods. Note `bpy.data.materials.new()` also creates the Principled node tree in 5.2 (verified).

#### `material.copy` - Copy Material

*Copy the material settings and nodes* | priority **P2** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER

No properties.

Copy material settings to an internal clipboard (needs context material).

#### `material.paste` - Paste Material

*Paste the material settings and nodes* | priority **P2** | poll A-E: `Y N Y N N` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

#### `image.new` - New Image

*Create a new image* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: UNDO

`bpy.ops.image.new(name='Untitled', width=1024, height=1024, color=(0, 0, 0, 1), alpha=True, generated_type='BLANK', float=False, use_stereo_3d=False, tiled=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `name` | STRING | `"Untitled"` |  | Image data-block name |
| `width` | INT PIXEL | `1024` | [1, INT_MAX] soft [1, 16384] | Image width |
| `height` | INT PIXEL | `1024` | [1, INT_MAX] soft [1, 16384] | Image height |
| `color` *(hidden)* | FLOAT[4] COLOR_GAMMA | `(0, 0, 0, 1)` | [0, FLT_MAX] soft [0, 1] | Default fill color |
| `alpha` | BOOLEAN | `True` |  | Create an image with an alpha channel |
| `generated_type` | ENUM | `'BLANK'` | `BLANK`, `UV_GRID`, `COLOR_GRID` | Fill the image with a grid for UV map testing |
| `float` | BOOLEAN | `False` |  | Create image with 32-bit floating-point bit depth |
| `use_stereo_3d` *(hidden)* | BOOLEAN | `False` |  | Create an image with left and right views |
| `tiled` | BOOLEAN | `False` |  | Create a tiled image |

Verified defaults: name `Untitled`, 1024x1024, `generated_type='BLANK'`, color (0,0,0,1), `float=False`, `alpha=True`, `source='GENERATED'`, colorspace `sRGB`, `alpha_mode='STRAIGHT'`. `generated_type`: BLANK, UV_GRID, COLOR_GRID. `tiled` -> UDIM (OUT). Script alternative: `bpy.data.images.new(name, width, height, alpha=False, float_buffer=False, ...)`.

#### `image.open` - Open Image

*Open image* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.image.open(allow_path_tokens=True, filepath='', directory='', files=[...], hide_props_region=True, check_existing=False, filter_blender=False, filter_backup=False, filter_image=True, filter_movie=True, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=9, relative_path=True, show_multiview=False, use_multiview=False, display_type='DEFAULT', sort_method='' (dynamic), use_sequence_detection=True, use_udim_detecting=True)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `allow_path_tokens` *(hidden)* | BOOLEAN | `True` |  | Allow the path to contain substitution tokens |
| `filepath` | STRING FILE_PATH | `""` |  | Path to file |
| `directory` | STRING DIR_PATH | `""` |  | Directory of the file |
| `files` *(hidden)* | COLLECTION -> OperatorFileListElement | `-` |  |  |
| `hide_props_region` *(hidden)* | BOOLEAN | `True` |  | Collapse the region displaying the operator settings |
| `check_existing` *(hidden)* | BOOLEAN | `False` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `9` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `relative_path` | BOOLEAN | `True` |  | Select the file relative to the blend file |
| `show_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `use_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |
| `use_sequence_detection` | BOOLEAN | `True` |  | Automatically detect animated sequences in selected images (based on file names) |
| `use_udim_detecting` | BOOLEAN | `True` |  | Detect selected UDIM files and load all matching tiles |

Load image file(s) (`filepath` or `directory`+`files`). `relative_path=True` stores `//`-relative path when the file is saved. Script alternative: `bpy.data.images.load(filepath, check_existing=False)`. frap: original image bytes are stored in the `.frap` container (pack semantics).

#### `image.save` - Save Image

*Save the image with current name and settings* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Needs Image Editor context (N headless). Script: `image.save()` / `image.save(filepath=..., quality=...)` methods.

#### `image.save_as` - Save As Image

*Save the image with another name and/or settings* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.image.save_as(save_as_render=False, copy=False, allow_path_tokens=True, filepath='', check_existing=True, filter_blender=False, filter_backup=False, filter_image=True, filter_movie=True, filter_python=False, filter_font=False, filter_sound=False, filter_text=False, filter_archive=False, filter_btx=False, filter_alembic=False, filter_usd=False, filter_obj=False, filter_volume=False, filter_folder=True, filter_blenlib=False, filemode=9, relative_path=True, show_multiview=False, use_multiview=False, display_type='DEFAULT', sort_method='' (dynamic))`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `save_as_render` | BOOLEAN | `False` |  | Save image with render color management. For display image formats like PNG, apply view and display transform. For intermediate image formats like OpenEXR, use the default render output color space |
| `copy` | BOOLEAN | `False` |  | Create a new image file without modifying the current image in Blender |
| `allow_path_tokens` *(hidden)* | BOOLEAN | `True` |  | Allow the path to contain substitution tokens |
| `filepath` | STRING FILE_PATH | `""` |  | Path to file |
| `check_existing` *(hidden)* | BOOLEAN | `True` |  | Check and warn on overwriting existing files |
| `filter_blender` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_backup` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_image` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_movie` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_python` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_font` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_sound` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_text` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_archive` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_btx` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_alembic` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_usd` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_obj` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_volume` *(hidden)* | BOOLEAN | `False` |  |  |
| `filter_folder` *(hidden)* | BOOLEAN | `True` |  |  |
| `filter_blenlib` *(hidden)* | BOOLEAN | `False` |  |  |
| `filemode` *(hidden)* | INT | `9` | [1, 9] | The setting for the file browser mode to load a .blend file, a library or a special file |
| `relative_path` | BOOLEAN | `True` |  | Select the file relative to the blend file |
| `show_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `use_multiview` *(hidden)* | BOOLEAN | `False` |  |  |
| `display_type` *(hidden)* | ENUM | `'DEFAULT'` | `DEFAULT`, `LIST_VERTICAL`, `LIST_HORIZONTAL`, `THUMBNAIL` |  |
| `sort_method` *(hidden)* | ENUM | `'' (dynamic)` | dynamic enum (items depend on context) |  |

Needs Image Editor context. Script: `image.save_render(filepath, scene=...)` for render results.

#### `image.pack` - Pack Image

*Pack an image as embedded data into the .blend file* | priority **P1** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

No properties.

Needs image context (N headless); script method `image.pack()` works. In frap all images are effectively packed in the `.frap` zip.

#### `image.unpack` - Unpack Image

*Save an image packed in the .blend file to disk* | priority **P2** | poll A-E: `N N N N N` | bl_options: REGISTER, UNDO

`bpy.ops.image.unpack(method='USE_LOCAL', id='')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `method` | ENUM | `'USE_LOCAL'` | `REMOVE`, `USE_LOCAL`, `WRITE_LOCAL`, `USE_ORIGINAL`, `WRITE_ORIGINAL` | How to unpack |
| `id` | STRING | `""` |  | Image data-block name to unpack |

Script method `image.unpack(method='USE_LOCAL')`.

#### `image.reload` - Reload Image

*Reload current image from disk* | priority **P2** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER

No properties.

#### `world.new` - New World

*Create a new world Data-Block* | priority **P1** | poll A-E: `Y Y Y Y Y` | bl_options: INTERNAL, REGISTER, UNDO

No properties.

New world `World.001` with Background node. Script alternative `bpy.data.worlds.new()`. World background color/HDRI drives frap's IBL (`world.node_tree.nodes['Background'].inputs['Color'/'Strength']`, `Environment Texture`).

### 4.20 glTF import (`import_scene.gltf`)

#### `import_scene.gltf` - Import glTF 2.0

*Load a glTF 2.0 file* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: REGISTER, UNDO

`bpy.ops.import_scene.gltf(filepath='', export_import_convert_lighting_mode='SPEC', filter_glob='*.glb;*.gltf', directory='', files=[...], loglevel=0, import_pack_images=True, merge_vertices=False, import_shading='NORMALS', bone_heuristic='BLENDER', disable_bone_shape=False, bone_shape_scale_factor=1, guess_original_bind_pose=True, import_webp_texture=False, import_unused_materials=False, import_select_created_objects=True, import_scene_extras=True, import_scene_as_collection=True, import_merge_material_slots=True, import_point_as_pointcloud=False)`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` *(hidden)* | STRING FILE_PATH | `""` |  | Filepath used for importing the file |
| `export_import_convert_lighting_mode` | ENUM | `'SPEC'` | `SPEC`, `COMPAT`, `RAW` | Optional backwards compatibility for non-standard render engines. Applies to lights |
| `filter_glob` *(hidden)* | STRING | `"*.glb;*.gltf"` |  |  |
| `directory` *(hidden)* | STRING DIR_PATH | `""` |  |  |
| `files` | COLLECTION -> OperatorFileListElement | `-` |  |  |
| `loglevel` | INT | `0` | [INT_MIN, INT_MAX] | Log Level |
| `import_pack_images` | BOOLEAN | `True` |  | Pack all images into .blend file |
| `merge_vertices` | BOOLEAN | `False` |  | The glTF format requires discontinuous normals, UVs, and other vertex attributes to be stored as separate vertices, as required for rendering on typical graphics hardware. This option attempts to combine co-located vertices where possible. Currently cannot combine verts with different normals |
| `import_shading` | ENUM | `'NORMALS'` | `NORMALS`, `FLAT`, `SMOOTH` | How normals are computed during import |
| `bone_heuristic` | ENUM | `'BLENDER'` | `BLENDER`, `TEMPERANCE`, `FORTUNE` | Heuristic for placing bones. Tries to make bones pretty |
| `disable_bone_shape` | BOOLEAN | `False` |  | Do not create bone shapes |
| `bone_shape_scale_factor` | FLOAT | `1` | [-FLT_MAX, FLT_MAX] | Scale factor for bone shapes |
| `guess_original_bind_pose` | BOOLEAN | `True` |  | Try to guess the original bind pose for skinned meshes from the inverse bind matrices. When off, use default/rest pose as bind pose |
| `import_webp_texture` | BOOLEAN | `False` |  | If a texture exists in WebP format, loads the WebP texture instead of the fallback PNG/JPEG one |
| `import_unused_materials` | BOOLEAN | `False` |  | Import materials & Images not assigned to any mesh |
| `import_select_created_objects` | BOOLEAN | `True` |  | Select created objects at the end of the import |
| `import_scene_extras` | BOOLEAN | `True` |  | Import scene extras as custom properties. Existing custom properties will be overwritten |
| `import_scene_as_collection` | BOOLEAN | `True` |  | Import the scene as a collection |
| `import_merge_material_slots` | BOOLEAN | `True` |  | Merge material slots when possible |
| `import_point_as_pointcloud` | BOOLEAN | `False` |  | Import mesh with only POINTS primitives as Point Cloud objects |

- Imports .glb/.gltf (`filepath`, or `directory`+`files` for multiple). Created objects are linked into the **active collection** (verified `Collection`) — `import_scene_as_collection` (new) may create a collection per glTF scene when there are several scenes (uncertain; verify with a multi-scene file).
- Imported objects become selected and the last/root one active; previously selected objects are deselected (verified: only `Cube.001` selected+active) — `import_select_created_objects=True`.
- Name collisions get `.001`. Images are packed into the .blend by default (`import_pack_images=True`).
- `import_shading`: NORMALS (use glTF normals as custom split normals; faces smooth), FLAT, SMOOTH. `merge_vertices` merges split vertices (glTF is per-vertex; Blender is per-corner).
- `bone_heuristic` BLENDER/TEMPERANCE/FORTUNE: how bone tails/orientations are guessed from glTF joints (glTF has no bone length) — important for round-trip fidelity.
- `export_import_convert_lighting_mode` shared with export (SPEC = physically based W<->cd/lux conversion).
- Supports extensions (from the addon source): KHR_materials_{emissive_strength, transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion, unlit, variants}, KHR_lights_punctual, KHR_texture_transform, KHR_draco_mesh_compression, EXT_texture_webp (`import_webp_texture`), EXT_mesh_gpu_instancing, KHR_animation_pointer, KHR_mesh_quantization (implicit), meshopt decode (verify).

### 4.21 glTF export (`export_scene.gltf`) - CRITICAL

#### `export_scene.gltf` - Export glTF 2.0

*Export scene as glTF 2.0 file* | priority **P0** | poll A-E: `Y Y Y Y Y` | bl_options: PRESET

`bpy.ops.export_scene.gltf(filepath='', check_existing=True, export_import_convert_lighting_mode='SPEC', gltf_export_id='', export_use_gltfpack=False, export_gltfpack_tc=True, export_gltfpack_tq=8, export_gltfpack_si=1, export_gltfpack_sa=False, export_gltfpack_slb=False, export_gltfpack_vp=14, export_gltfpack_vt=12, export_gltfpack_vn=8, export_gltfpack_vc=8, export_gltfpack_vpi='Integer', export_gltfpack_noq=True, export_gltfpack_kn=False, export_format='GLB', ui_tab='GENERAL', export_copyright='', export_image_format='AUTO', export_image_add_webp=False, export_image_webp_fallback=False, export_texture_dir='', export_jpeg_quality=75, export_image_quality=75, export_keep_originals=False, export_texcoords=True, export_normals=True, export_gn_mesh=False, export_meshopt_compression_enable=False, export_meshopt_extension='EXT_meshopt_compression', export_draco_mesh_compression_enable=False, export_draco_mesh_compression_level=6, export_draco_position_quantization=14, export_draco_normal_quantization=10, export_draco_texcoord_quantization=12, export_draco_color_quantization=10, export_draco_generic_quantization=12, export_tangents=False, export_materials='EXPORT', export_unused_images=False, export_unused_textures=False, export_vertex_color='MATERIAL', export_vertex_color_name='Color', export_all_vertex_colors=True, export_active_vertex_color_when_no_material=True, export_attributes=False, use_mesh_edges=False, use_mesh_vertices=False, export_cameras=False, use_selection=False, use_visible=False, use_renderable=False, use_active_collection_with_nested=True, use_active_collection=False, use_active_scene=False, collection='', at_collection_center=False, export_extras=False, export_yup=True, export_apply=False, export_shared_accessors=False, export_animations=True, export_frame_range=False, export_frame_step=1, export_force_sampling=True, export_sampling_interpolation_fallback='LINEAR', export_pointer_animation=False, export_animation_mode='ACTIONS', export_nla_strips_merged_animation_name='Animation', export_def_bones=False, export_hierarchy_flatten_bones=False, export_hierarchy_flatten_objs=False, export_armature_object_remove=False, export_leaf_bone=False, export_optimize_animation_size=True, export_optimize_animation_keep_anim_armature=True, export_optimize_animation_keep_anim_object=False, export_optimize_disable_viewport=False, export_negative_frame='SLIDE', export_anim_slide_to_zero=False, export_bake_animation=False, export_merge_animation='ACTION', export_anim_single_armature=True, export_reset_pose_bones=True, export_current_frame=False, export_rest_position_armature=True, export_anim_scene_split_object=True, export_skins=True, export_influence_nb=4, export_all_influences=False, export_morph=True, export_morph_normal=True, export_morph_tangent=False, export_morph_animation=True, export_morph_reset_sk_data=True, export_lights=False, export_try_sparse_sk=True, export_try_omit_sparse_sk=False, export_gpu_instances=False, export_action_filter=False, export_convert_animation_pointer=False, export_nla_strips=True, export_original_specular=False, will_save_settings=False, export_hierarchy_full_collections=False, export_extra_animations=False, export_loglevel=-1, filter_glob='*.glb')`

| Property | Type | Default | Items / range | Description |
|---|---|---|---|---|
| `filepath` | STRING FILE_PATH | `""` |  | Filepath used for exporting the file |
| `check_existing` *(hidden)* | BOOLEAN | `True` |  | Check and warn on overwriting existing files |
| `export_import_convert_lighting_mode` | ENUM | `'SPEC'` | `SPEC`, `COMPAT`, `RAW` | Optional backwards compatibility for non-standard render engines. Applies to lights |
| `gltf_export_id` | STRING | `""` |  | Identifier of caller (in case of add-on calling this exporter). Can be useful in case of Extension added by other add-ons |
| `export_use_gltfpack` | BOOLEAN | `False` |  | Use gltfpack to simplify the mesh and/or compress its textures |
| `export_gltfpack_tc` | BOOLEAN | `True` |  | Convert all textures to KTX2 with BasisU supercompression |
| `export_gltfpack_tq` | INT | `8` | [1, 10] | Texture encoding quality |
| `export_gltfpack_si` | FLOAT | `1` | [0, 1] | Simplify meshes targeting triangle count ratio |
| `export_gltfpack_sa` | BOOLEAN | `False` |  | Aggressively simplify to the target ratio disregarding quality |
| `export_gltfpack_slb` | BOOLEAN | `False` |  | Lock border vertices during simplification to avoid gaps on connected meshes |
| `export_gltfpack_vp` | INT | `14` | [1, 16] | Use N-bit quantization for positions |
| `export_gltfpack_vt` | INT | `12` | [1, 16] | Use N-bit quantization for texture coordinates |
| `export_gltfpack_vn` | INT | `8` | [1, 16] | Use N-bit quantization for normals and tangents |
| `export_gltfpack_vc` | INT | `8` | [1, 16] | Use N-bit quantization for colors |
| `export_gltfpack_vpi` | ENUM | `'Integer'` | `Integer`, `Normalized`, `Floating-point` | Type to use for vertex position attributes |
| `export_gltfpack_noq` | BOOLEAN | `True` |  | Disable quantization; produces much larger glTF files with no extensions |
| `export_gltfpack_kn` | BOOLEAN | `False` |  | Restrict some optimization to keep named nodes and meshes attached to named nodes so that named nodes can be transformed externally |
| `export_format` | ENUM | `'GLB'` | `GLB`, `GLTF_SEPARATE`, `GLTF_EMBEDDED` | Output format. Binary is most efficient, but JSON may be easier to edit later [dynamic enum; items taken from io_scene_gltf2 get_format_items(); default = item 0 = GLB] |
| `ui_tab` | ENUM | `'GENERAL'` | `GENERAL`, `MESHES`, `OBJECTS`, `ANIMATION` | Export setting categories |
| `export_copyright` | STRING | `""` |  | Legal rights and conditions for the model |
| `export_image_format` | ENUM | `'AUTO'` | `AUTO`, `JPEG`, `WEBP`, `NONE` | Output format for images. PNG is lossless and generally preferred, but JPEG might be preferable for web applications due to the smaller file size. Alternatively they can be omitted if they are not needed |
| `export_image_add_webp` | BOOLEAN | `False` |  | Creates WebP textures for every texture. For already WebP textures, nothing happens |
| `export_image_webp_fallback` | BOOLEAN | `False` |  | For all WebP textures, create a PNG fallback texture |
| `export_texture_dir` | STRING | `""` |  | Folder to place texture files in. Relative to the .gltf file |
| `export_jpeg_quality` | INT | `75` | [0, 100] | Quality of JPEG export |
| `export_image_quality` | INT | `75` | [0, 100] | Quality of image export |
| `export_keep_originals` | BOOLEAN | `False` |  | Keep original textures files if possible. WARNING: if you use more than one texture, where pbr standard requires only one, only one texture will be used. This can lead to unexpected results |
| `export_texcoords` | BOOLEAN | `True` |  | Export UVs (texture coordinates) with meshes |
| `export_normals` | BOOLEAN | `True` |  | Export vertex normals with meshes |
| `export_gn_mesh` | BOOLEAN | `False` |  | Export Geometry nodes instance meshes |
| `export_meshopt_compression_enable` | BOOLEAN | `False` |  | Compress mesh using Meshopt |
| `export_meshopt_extension` | ENUM | `'EXT_meshopt_compression'` | `EXT_meshopt_compression`, `KHR_meshopt_compression` | Extension to use for meshopt compression |
| `export_draco_mesh_compression_enable` | BOOLEAN | `False` |  | Compress mesh using Draco |
| `export_draco_mesh_compression_level` | INT | `6` | [0, 10] | Compression level (0 = most speed, 6 = most compression, higher values currently not supported) |
| `export_draco_position_quantization` | INT | `14` | [0, 30] | Quantization bits for position values (0 = no quantization) |
| `export_draco_normal_quantization` | INT | `10` | [0, 30] | Quantization bits for normal values (0 = no quantization) |
| `export_draco_texcoord_quantization` | INT | `12` | [0, 30] | Quantization bits for texture coordinate values (0 = no quantization) |
| `export_draco_color_quantization` | INT | `10` | [0, 30] | Quantization bits for color values (0 = no quantization) |
| `export_draco_generic_quantization` | INT | `12` | [0, 30] | Quantization bits for generic values like weights or joints (0 = no quantization) |
| `export_tangents` | BOOLEAN | `False` |  | Export vertex tangents with meshes |
| `export_materials` | ENUM | `'EXPORT'` | `EXPORT`, `PLACEHOLDER`, `VIEWPORT`, `NONE` | Export materials |
| `export_unused_images` | BOOLEAN | `False` |  | Export images not assigned to any material |
| `export_unused_textures` | BOOLEAN | `False` |  | Export image texture nodes not assigned to any material. This feature is not standard and needs an external extension to be included in the glTF file |
| `export_vertex_color` | ENUM | `'MATERIAL'` | `MATERIAL`, `ACTIVE`, `NAME`, `NONE` | How to export vertex color |
| `export_vertex_color_name` | STRING | `"Color"` |  | Name of vertex color to export |
| `export_all_vertex_colors` | BOOLEAN | `True` |  | Export all vertex colors, even if not used by any material. If no Vertex Color is used in the mesh materials, a fake COLOR_0 will be created, in order to keep material unchanged |
| `export_active_vertex_color_when_no_material` | BOOLEAN | `True` |  | When there is no material on object, export active vertex color |
| `export_attributes` | BOOLEAN | `False` |  | Export Attributes (when starting with underscore) |
| `use_mesh_edges` | BOOLEAN | `False` |  | Export loose edges as lines, using the material from the first material slot |
| `use_mesh_vertices` | BOOLEAN | `False` |  | Export loose points as glTF points, using the material from the first material slot |
| `export_cameras` | BOOLEAN | `False` |  | Export cameras |
| `use_selection` | BOOLEAN | `False` |  | Export selected objects only |
| `use_visible` | BOOLEAN | `False` |  | Export visible objects only |
| `use_renderable` | BOOLEAN | `False` |  | Export renderable objects only |
| `use_active_collection_with_nested` | BOOLEAN | `True` |  | Include active collection and nested collections |
| `use_active_collection` | BOOLEAN | `False` |  | Export objects in the active collection only |
| `use_active_scene` | BOOLEAN | `False` |  | Export active scene only |
| `collection` | STRING | `""` |  | Export only objects from this collection (and its children) |
| `at_collection_center` | BOOLEAN | `False` |  | Export at Collection center of mass of root objects of the collection |
| `export_extras` | BOOLEAN | `False` |  | Export custom properties as glTF extras |
| `export_yup` | BOOLEAN | `True` |  | Export using glTF convention, +Y up |
| `export_apply` | BOOLEAN | `False` |  | Apply modifiers (excluding Armatures) to mesh objects -WARNING: prevents exporting shape keys |
| `export_shared_accessors` | BOOLEAN | `False` |  | Export Primitives using shared accessors for attributes |
| `export_animations` | BOOLEAN | `True` |  | Exports active actions and NLA tracks as glTF animations |
| `export_frame_range` | BOOLEAN | `False` |  | Clips animations to selected playback range |
| `export_frame_step` | INT | `1` | [1, 120] | How often to evaluate animated values (in frames) |
| `export_force_sampling` | BOOLEAN | `True` |  | Apply sampling to all animations |
| `export_sampling_interpolation_fallback` | ENUM | `'LINEAR'` | `LINEAR`, `STEP` | Interpolation fallback for sampled animations, when the property is not keyed |
| `export_pointer_animation` | BOOLEAN | `False` |  | Export material, Light & Camera animation as Animation Pointer. Available only for baked animation mode 'NLA Tracks' and 'Scene' |
| `export_animation_mode` | ENUM | `'ACTIONS'` | `ACTIONS`, `ACTIVE_ACTIONS`, `BROADCAST`, `NLA_TRACKS`, `SCENE` | Export Animation mode |
| `export_nla_strips_merged_animation_name` | STRING | `"Animation"` |  | Name of single glTF animation to be exported |
| `export_def_bones` | BOOLEAN | `False` |  | Export Deformation bones only |
| `export_hierarchy_flatten_bones` | BOOLEAN | `False` |  | Flatten Bone Hierarchy. Useful in case of non decomposable transformation matrix |
| `export_hierarchy_flatten_objs` | BOOLEAN | `False` |  | Flatten Object Hierarchy. Useful in case of non decomposable transformation matrix |
| `export_armature_object_remove` | BOOLEAN | `False` |  | Remove Armature object if possible. If Armature has multiple root bones, object will not be removed |
| `export_leaf_bone` | BOOLEAN | `False` |  | Append a final bone to the end of each chain to specify last bone length (use this when you intend to edit the armature from exported data) |
| `export_optimize_animation_size` | BOOLEAN | `True` |  | Reduce exported file size by removing duplicate keyframes |
| `export_optimize_animation_keep_anim_armature` | BOOLEAN | `True` |  | If all keyframes are identical in a rig, force keeping the minimal animation. When off, all possible channels for the bones will be exported, even if empty (minimal animation, 2 keyframes) |
| `export_optimize_animation_keep_anim_object` | BOOLEAN | `False` |  | If all keyframes are identical for object transformations, force keeping the minimal animation |
| `export_optimize_disable_viewport` | BOOLEAN | `False` |  | When exporting animations, disable viewport for other objects, for performance |
| `export_negative_frame` | ENUM | `'SLIDE'` | `SLIDE`, `CROP` | Negative Frames are slid or cropped |
| `export_anim_slide_to_zero` | BOOLEAN | `False` |  | Set all glTF animation starting at 0.0s. Can be useful for looping animations |
| `export_bake_animation` | BOOLEAN | `False` |  | Force exporting animation on every object. Can be useful when using constraints or driver. Also useful when exporting only selection |
| `export_merge_animation` | ENUM | `'ACTION'` | `NLA_TRACK`, `ACTION`, `NONE` | Merge Animations |
| `export_anim_single_armature` | BOOLEAN | `True` |  | Export all actions, bound to a single armature. WARNING: Option does not support exports including multiple armatures |
| `export_reset_pose_bones` | BOOLEAN | `True` |  | Reset pose bones between each action exported. This is needed when some bones are not keyed on some animations |
| `export_current_frame` | BOOLEAN | `False` |  | Export the scene in the current animation frame. When off, frame 0 is used as rest transformations for objects |
| `export_rest_position_armature` | BOOLEAN | `True` |  | Export armatures using rest position as joints' rest pose. When off, current frame pose is used as rest pose |
| `export_anim_scene_split_object` | BOOLEAN | `True` |  | Export Scene as seen in Viewport, But split animation by Object |
| `export_skins` | BOOLEAN | `True` |  | Export skinning (armature) data |
| `export_influence_nb` | INT | `4` | [1, INT_MAX] | Choose how many Bone influences to export |
| `export_all_influences` | BOOLEAN | `False` |  | Allow export of all joint vertex influences. Models may appear incorrectly in many viewers |
| `export_morph` | BOOLEAN | `True` |  | Export shape keys (morph targets) |
| `export_morph_normal` | BOOLEAN | `True` |  | Export vertex normals with shape keys (morph targets) |
| `export_morph_tangent` | BOOLEAN | `False` |  | Export vertex tangents with shape keys (morph targets) |
| `export_morph_animation` | BOOLEAN | `True` |  | Export shape keys animations (morph targets) |
| `export_morph_reset_sk_data` | BOOLEAN | `True` |  | Reset shape keys between each action exported. This is needed when some SK channels are not keyed on some animations |
| `export_lights` | BOOLEAN | `False` |  | Export directional, point, and spot lights. Uses "KHR_lights_punctual" glTF extension |
| `export_try_sparse_sk` | BOOLEAN | `True` |  | Try using Sparse Accessor if it saves space |
| `export_try_omit_sparse_sk` | BOOLEAN | `False` |  | Omitting Sparse Accessor if data is empty |
| `export_gpu_instances` | BOOLEAN | `False` |  | Export using EXT_mesh_gpu_instancing. Limited to children of a given Empty. Multiple materials might be omitted |
| `export_action_filter` | BOOLEAN | `False` |  | Filter Actions to be exported |
| `export_convert_animation_pointer` | BOOLEAN | `False` |  | Export TRS and weights as Animation Pointer. Using KHR_animation_pointer extension |
| `export_nla_strips` | BOOLEAN | `True` |  | When on, multiple actions become part of the same glTF animation if they're pushed onto NLA tracks with the same name. When off, all the currently assigned actions become one glTF animation |
| `export_original_specular` | BOOLEAN | `False` |  | Export original glTF PBR Specular, instead of Blender Principled Shader Specular |
| `will_save_settings` | BOOLEAN | `False` |  | Store glTF export settings in the Blender project |
| `export_hierarchy_full_collections` | BOOLEAN | `False` |  | Export full hierarchy, including intermediate collections |
| `export_extra_animations` | BOOLEAN | `False` |  | Export additional animations. This feature is not standard and needs an external extension to be included in the glTF file |
| `export_loglevel` | INT | `-1` | [INT_MIN, INT_MAX] | Log Level |
| `filter_glob` *(hidden)* | STRING | `"*.glb"` |  |  |

- Python operator (`io_scene_gltf2` add-on v5.2.41, bl_options `PRESET`; `UNDO` not set). All 110 properties listed below (verified via RNA dump); descriptions are Blender's own.
- `filepath` extension is **forced** to match `export_format`: `GLB` -> `.glb`, `GLTF_SEPARATE`/`GLTF_EMBEDDED` -> `.gltf` (`ensure_filepath_matches_export_format`: appended if no/other extension; verified `b` -> `b.gltf` + `b.bin`).
- **`export_format` is a dynamic enum**: `GLB` (default, item 0), `GLTF_SEPARATE`; `GLTF_EMBEDDED` only when the add-on preference `allow_embedded_format` is enabled. frap should accept all three.
- Defaults to note: `use_selection=False` (exports whole scene), `export_apply=False` (**modifiers are NOT applied by default**), `export_yup=True` (+Y up conversion), `export_cameras=False` and `export_lights=False` by default (!), `export_animation_mode='ACTIONS'`, `export_force_sampling=True`, `export_materials='EXPORT'`, `export_image_format='AUTO'`, `export_vertex_color='MATERIAL'` (colors exported only if used by the material node tree), `export_influence_nb=4`, `export_morph=True`, `export_skins=True`, `export_extras=False`, Draco/meshopt off.
- Draco and meshopt use native libs shipped with Blender (`libbf_intern_draco_bridge`, `libbf_intern_meshopt_bridge`); frap needs Rust equivalents (draco: no mature pure-Rust encoder — pitfall; meshopt: `meshopt` crate). `export_meshopt_extension` offers `EXT_meshopt_compression` and the newer `KHR_meshopt_compression`.
- `export_use_gltfpack` + `export_gltfpack_*` shell out to an external `gltfpack` binary (Preferences path) — frap may mark P2/OUT.
- Extensions the exporter can emit (from source scan): KHR_materials_{emissive_strength, transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion, unlit, variants}, KHR_lights_punctual, KHR_texture_transform, KHR_draco_mesh_compression, EXT_meshopt_compression / KHR_meshopt_compression, KHR_mesh_quantization (via gltfpack only), EXT_texture_webp, EXT_mesh_gpu_instancing, KHR_animation_pointer. **KHR_texture_basisu is NOT supported by Blender's exporter** -> any frap support is a frap extension (namespace it).
- UI-only/internal: `ui_tab`, `will_save_settings`, `gltf_export_id`, `filter_glob`, `check_existing`, `export_loglevel`.


## 5. Non-operator API that scripts use instead of (or with) these operators

Agents trained on Blender scripts use these calls at least as often as the operators. The bpy-line front-end should support them as typed **RNA functions** in the same registry. A separate doc covers the RNA data properties in depth; the list below covers only what these operators interact with.

### 5.1 Frames and animation

| Call | Signature (Blender 5.2, verified) | Notes |
|---|---|---|
| `Scene.frame_set` | `scene.frame_set(frame, subframe=0.0)` | Sets `frame_current` and **evaluates animation and drivers** (verified). This replaces the non-existent `screen.frame_set`. |
| `bpy_struct.keyframe_insert` | `keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME') -> bool` | `options` subset of `INSERTKEY_NEEDED`, `INSERTKEY_VISUAL`, `INSERTKEY_REPLACE`, `INSERTKEY_AVAILABLE`, `INSERTKEY_CYCLE_AWARE`. `keytype` one of `KEYFRAME`, `BREAKDOWN`, `MOVING_HOLD`, `EXTREME`, `JITTER`, `GENERATED`. `index=-1` keys all array elements. `frame=None` uses the current frame. Works on any animatable struct: `obj.keyframe_insert("location")`, `obj.pose.bones["Bone"].keyframe_insert("rotation_quaternion")` (path is resolved relative to the ID: `pose.bones["Bone"].rotation_quaternion`), `mesh.shape_keys.key_blocks["Key 1"].keyframe_insert("value")` (keys the `Key` ID), `mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].keyframe_insert("default_value")` (keys the node tree, and is exported through KHR_animation_pointer). |
| `bpy_struct.keyframe_delete` | `keyframe_delete(data_path, *, index=-1, frame=None, group="") -> bool` | Verified it returns True. |
| `bpy_struct.driver_add/remove` | | OUT. |
| Action layout (5.x) | `action.layers[i].strips[j].channelbag(slot, ensure=False).fcurves`; `action.slots`; `obj.animation_data.action`, `.action_slot`, `.action_slot_handle` | Verified: `CubeAction`, slot `OBCube`, group `Object Transforms`. `FCurve.keyframe_points.insert(frame, value, options=set(), keytype='KEYFRAME')`, `keyframe_points.add(count)`, `KeyFrame.co`, `.interpolation` (`CONSTANT`/`LINEAR`/`BEZIER`/easing types), `.handle_left/right(_type)`. glTF supports only STEP, LINEAR and CUBICSPLINE: the exporter maps CONSTANT to STEP and LINEAR to LINEAR, and **samples** BEZIER and the rest when `export_force_sampling=True` (the default). |
| NLA | `obj.animation_data.nla_tracks.new()`, `track.strips.new(name, start, action)`, `track.name`, `strip.action_frame_start/end`, `track.mute` | `export_animation_mode='NLA_TRACKS'` exports one glTF animation per track name. `'ACTIONS'` (the default) exports one per action (and slot), and `'SCENE'` gives one merged animation. |

### 5.2 Data-block creation and linking (`bpy.data`)

| Call | Notes |
|---|---|
| `bpy.data.objects.new(name, object_data)` | Not linked to any scene. Requires `collection.objects.link(obj)`. |
| `bpy.data.meshes.new(name)`, `mesh.from_pydata(vertices, edges, faces)` | Low-level mesh creation, which agents use a lot. |
| `bpy.data.materials.new(name)` | Creates the Principled BSDF + Material Output node tree in 5.2 (verified). |
| `bpy.data.images.new(name, width, height, alpha=False, float_buffer=False, ...)`, `bpy.data.images.load(filepath, check_existing=False)`, `image.pack()`, `image.save(...)`, `image.save_render(filepath, scene=None)` | The image operators that need an Image Editor (`save`, `save_as`, `pack`) all have script equivalents. |
| `bpy.data.lights.new(name, type)`, `bpy.data.cameras.new(name)`, `bpy.data.armatures.new(name)`, `bpy.data.actions.new(name)`, `bpy.data.collections.new(name)`, `bpy.data.worlds.new(name)` | |
| `bpy.data.<coll>.remove(id, do_unlink=True)` | Deletes immediately (no orphan stage, unlike `object.delete`). |
| `collection.objects.link(obj)` / `.unlink(obj)`, `collection.children.link(coll)`, `scene.collection` (master collection) | |
| `mesh.materials.append(mat)` / `.pop(index=-1)` / `.clear()`, `obj.material_slots[i].material = mat`, `slot.link = 'OBJECT'/'DATA'` | |
| `obj.modifiers.new(name, type)`, `obj.modifiers.remove(mod)`, `obj.modifiers.move(from, to)` | Script equivalents of `modifier_add`/`remove`/`move_to_index`. Note that `new()` takes an explicit name and does not apply the "requires original data" insertion rule (*believed*). |
| `obj.vertex_groups.new(name="Group")`, `vg.add(index_list, weight, 'REPLACE'/'ADD'/'SUBTRACT')`, `vg.remove(index_list)`, `vg.weight(index)` | Works in object mode, unlike `vertex_group_assign`. This is the main path for skin weights. |
| `obj.shape_key_add(name="Key", from_mix=True)`, `obj.shape_key_remove(key)`, `obj.shape_key_clear()` | |
| `obj.evaluated_get(depsgraph)`, `bpy.context.evaluated_depsgraph_get()`, `obj.to_mesh()`, `obj.matrix_world` (read; write = set world transform) | Agents read evaluated (modifier-applied) data this way. |

## 6. Remaining `object.*` operators (not detailed above)

Blender 5.2 has **250** `object.*` operators. 129 are detailed in section 4. The other 121 are listed here with Blender's own description and a verdict.

| Operator | Description | Verdict |
|---|---|---|
| `object.add_modifier_menu` | (undocumented operator) | OUT |
| `object.assign_property_defaults` | Assign the current values of custom properties as their defaults, for use as part of the rest pose state in NLA track mixing | OUT |
| `object.bake` | Bake image textures of selected objects | OUT |
| `object.bake_image` | Bake image textures of selected objects | OUT |
| `object.camera_custom_update` | Update custom camera with new parameters from the shader | OUT |
| `object.clear_override_library` | Delete the selected local overrides and relink their usages to the linked data-blocks if possible, else reset them and mark them as non editable | OUT |
| `object.collection_external_asset_drop` | Add the dragged collection to the scene | OUT |
| `object.collection_objects_select` | Select all objects in collection | P2 |
| `object.constraint_add_with_targets` | Add a constraint to the active object, with target (where applicable) set to the selected objects/bones | P2 |
| `object.constraints_clear` | Clear all constraints from the selected objects | P2 |
| `object.constraints_copy` | Copy constraints to other selected objects | P2 |
| `object.copy_global_transform` | Copies the matrix of the currently active object or pose bone to the clipboard. Uses world-space matrices | P2 |
| `object.copy_relative_transform` | Copies the matrix of the currently active object or pose bone to the clipboard. Uses matrices relative to a specific object or the active scene camera | P2 |
| `object.correctivesmooth_bind` | Bind base pose in Corrective Smooth modifier | OUT |
| `object.curves_empty_hair_add` | Add an empty curve object to the scene with the selected mesh as surface | OUT |
| `object.curves_random_add` | Add a curves object with random curves to the scene | OUT |
| `object.data_transfer` | Transfer data layer(s) (weights, edge sharp, etc.) from active to selected meshes | P2 (copy UVs/normals/weights between meshes) |
| `object.datalayout_transfer` | Transfer layout of data layer(s) from active to selected meshes | P2 |
| `object.delete_fix_to_camera_keys` | Delete all keys that were generated by the 'Fix to Scene Camera' operator | OUT |
| `object.drop_geometry_nodes` | (undocumented operator) | OUT |
| `object.drop_named_material` | (undocumented operator) | OUT (drag & drop) |
| `object.effector_add` | Add an empty object with a physics effector to the scene | OUT |
| `object.explode_refresh` | Refresh data in the Explode modifier | OUT |
| `object.fix_to_camera` | Generate new keys to fix the selected object/bone to the camera on unkeyed frames | OUT |
| `object.forcefield_toggle` | Toggle object's force field | OUT |
| `object.geometry_node_bake_delete_single` | Delete baked data of a single bake node or simulation | OUT |
| `object.geometry_node_bake_pack_single` | Pack baked data from disk into the .blend file | OUT |
| `object.geometry_node_bake_single` | Bake a single bake node or simulation | OUT |
| `object.geometry_node_bake_unpack_single` | Unpack baked data from the .blend file to disk | OUT |
| `object.geometry_node_tree_copy_assign` | Duplicate the active geometry node group and assign it to the active modifier | OUT |
| `object.geometry_nodes_input_attribute_toggle` | Switch between an attribute and a single value to define the data for every element | OUT |
| `object.geometry_nodes_move_to_nodes` | Move inputs and outputs from in the modifier to a new node group | OUT |
| `object.grease_pencil_add` | Add a Grease Pencil object to the scene | OUT |
| `object.grease_pencil_dash_modifier_segment_add` | Add a segment to the dash modifier | OUT |
| `object.grease_pencil_dash_modifier_segment_move` | Move the active dash segment up or down | OUT |
| `object.grease_pencil_dash_modifier_segment_remove` | Remove the active segment from the dash modifier | OUT |
| `object.grease_pencil_time_modifier_segment_add` | Add a segment to the time modifier | OUT |
| `object.grease_pencil_time_modifier_segment_move` | Move the active time segment up or down | OUT |
| `object.grease_pencil_time_modifier_segment_remove` | Remove the active segment from the time modifier | OUT |
| `object.hide_collection` | Show only objects in collection (Shift to extend) | P2 |
| `object.hook_add_newob` | Hook selected vertices to a newly created object | OUT |
| `object.hook_add_selob` | Hook selected vertices to the first selected object | OUT |
| `object.hook_assign` | Assign the selected vertices to a hook | OUT |
| `object.hook_recenter` | Set hook center to cursor position | OUT |
| `object.hook_remove` | Remove a hook from the active object | OUT |
| `object.hook_reset` | Recalculate and clear offset transformation | OUT |
| `object.hook_select` | Select affected vertices on mesh | OUT |
| `object.instance_offset_from_cursor` | Set offset used for collection instances based on cursor position | P2 |
| `object.instance_offset_from_object` | Set offset used for collection instances based on the active object position | P2 |
| `object.instance_offset_to_cursor` | Set cursor position to the offset used for collection instances | P2 |
| `object.isolate_type_render` | Hide unselected render objects of same type as active by setting the hide render flag | OUT |
| `object.join_uvs` | Transfer UV Maps from active to selected objects (needs matching geometry) | P2 |
| `object.laplaciandeform_bind` | Bind mesh to system in laplacian deform modifier | OUT |
| `object.lattice_add_to_selected` | Add a lattice and use it to deform selected objects | OUT |
| `object.light_linking_blocker_collection_new` | Create new light linking collection used by the active emitter | OUT |
| `object.light_linking_blockers_link` | Light link selected blockers to the active emitter object | OUT |
| `object.light_linking_blockers_select` | Select all objects which block light from this emitter | OUT |
| `object.light_linking_receiver_collection_new` | Create new light linking collection used by the active emitter | OUT |
| `object.light_linking_receivers_link` | Light link selected receivers to the active emitter object | OUT |
| `object.light_linking_receivers_select` | Select all objects which receive light from this emitter | OUT |
| `object.light_linking_unlink_from_collection` | Remove this object or collection from the light linking collection | OUT |
| `object.lightprobe_add` | Add a light probe object | OUT |
| `object.lightprobe_cache_bake` | Bake irradiance volume light cache | OUT |
| `object.lightprobe_cache_free` | Delete cached indirect lighting | OUT |
| `object.lineart_bake_strokes` | Bake Line Art for current Grease Pencil object | OUT |
| `object.lineart_clear` | Clear all strokes in current Grease Pencil object | OUT |
| `object.make_dupli_face` | Convert objects into instanced faces | OUT |
| `object.make_links_scene` | Link selection to another scene | P2 |
| `object.make_override_library` | Create a local override of the selected linked objects, and their hierarchy of dependencies | OUT |
| `object.meshdeform_bind` | Bind mesh to cage in mesh deform modifier | OUT |
| `object.metaball_add` | Add a metaball object to the scene | OUT |
| `object.modifier_add_node_group` | Add a procedural operation/effect to the active object | OUT (geometry nodes) |
| `object.modifier_convert` | Convert particles to a mesh object | P2 |
| `object.multires_base_apply` | Modify the base mesh to conform to the displaced mesh | OUT |
| `object.multires_external_pack` | Pack displacements from an external file | OUT |
| `object.multires_external_save` | Save displacements to an external file | OUT |
| `object.multires_higher_levels_delete` | Deletes the higher resolution mesh, potential loss of detail | OUT |
| `object.multires_rebuild_subdiv` | Rebuilds all possible subdivisions levels to generate a lower resolution base mesh | OUT |
| `object.multires_reshape` | Copy vertex coordinates from other object | OUT |
| `object.multires_subdivide` | Add a new level of subdivision | OUT |
| `object.multires_unsubdivide` | Rebuild a lower subdivision level of the current base mesh | OUT |
| `object.ocean_bake` | Bake an image sequence of ocean data | OUT |
| `object.particle_system_add` | Add a particle system | OUT |
| `object.particle_system_remove` | Remove the selected particle system | OUT |
| `object.paste_transform` | Pastes the matrix from the clipboard to the currently active pose bone or object. Uses world-space matrices | P2 |
| `object.paths_calculate` | Generate motion paths for the selected objects | OUT |
| `object.paths_clear` | (undocumented operator) | OUT |
| `object.paths_update` | Recalculate motion paths for selected objects | OUT |
| `object.paths_update_visible` | Recalculate all visible motion paths for objects and poses | OUT |
| `object.pointcloud_random_add` | Add a point cloud object to the scene | OUT |
| `object.quadriflow_remesh` | Create a new quad based mesh using the surface data of the current mesh. All data layers will be lost | OUT |
| `object.quick_explode` | Make selected objects explode | OUT |
| `object.quick_fur` | Add a fur setup to the selected objects | OUT |
| `object.quick_liquid` | Make selected objects liquid | OUT |
| `object.quick_smoke` | Use selected objects as smoke emitters | OUT |
| `object.reset_override_library` | Reset the selected local overrides to their linked references values | OUT |
| `object.shaderfx_add` | Add a visual effect to the active object | OUT |
| `object.shaderfx_copy` | Duplicate effect at the same position in the stack | OUT |
| `object.shaderfx_move_down` | Move effect down in the stack | OUT |
| `object.shaderfx_move_to_index` | Change the effect's position in the list so it evaluates after the set number of others | OUT |
| `object.shaderfx_move_up` | Move effect up in the stack | OUT |
| `object.shaderfx_remove` | Remove a effect from the active Grease Pencil object | OUT |
| `object.simulation_nodes_cache_bake` | Bake simulations in geometry nodes modifiers | OUT |
| `object.simulation_nodes_cache_calculate_to_frame` | Calculate simulations in geometry nodes modifiers from the start to current frame | OUT |
| `object.simulation_nodes_cache_delete` | Delete cached/baked simulations in geometry nodes modifiers | OUT |
| `object.skin_armature_create` | Create an armature that parallels the skin layout | OUT |
| `object.skin_loose_mark_clear` | Mark/clear selected vertices as loose | OUT |
| `object.skin_radii_equalize` | Make skin radii of selected vertices equal on each axis | OUT |
| `object.skin_root_mark` | Mark selected vertices as roots | OUT |
| `object.speaker_add` | Add a speaker object to the scene | OUT |
| `object.surfacedeform_bind` | Bind mesh to target in surface deform modifier | OUT |
| `object.transfer_mode` | Switches the active object and assigns the same mode to a new one under the mouse cursor, leaving the active mode in the current one | OUT |
| `object.transform_axis_target` | Interactively point cameras and lights to the surface under the pointer (Ctrl to translate) | OUT (interactive) |
| `object.transform_to_mouse` | Snap selected item(s) to the mouse location | OUT (interactive) |
| `object.unlink_data` | (undocumented operator) | P2 |
| `object.update_shapes` | Update existing shape keys with the vertex positions of selected objects with matching names | P2 |
| `object.visual_geometry_to_objects` | Convert geometry and instances into editable objects and collections | P2 |
| `object.volume_add` | Add a volume object to the scene | OUT |
| `object.volume_import` | Import OpenVDB volume file | OUT |
| `object.voxel_remesh` | Calculates a new manifold mesh based on the volume of the current mesh. All data layers will be lost | P2 |
| `object.voxel_size_edit` | Modify the mesh voxel size interactively used in the voxel remesher | OUT |

## 7. Uncertainties and items to verify before implementing

| # | Item | Status |
|---|---|---|
| 1 | `object.join`: how shape keys and differing UV/colour layers are merged | Not verified. Add a golden test. |
| 2 | `object.origin_set`: whether children keep their world transform | Believed (Blender calls `ignore_parent_tx`). Not verified. |
| 3 | `object.origin_clear` exact effect on parented objects | Not verified. Verified: no effect on unparented objects. |
| 4 | `object.select_grouped` poll was False in every state tested | Context requirement unknown. P2 anyway. |
| 5 | `light_add` gives `shadow_soft_size == 0.0` and the `radius` prop has no visible effect | Verified twice but surprising. Compare `bpy.data.lights.new()` defaults. |
| 6 | `mesh.primitive_cube_add(size=-5)` gives dims (1,1,1) | Verified, unexplained. frap may reject negative sizes. |
| 7 | `import_scene.gltf(import_scene_as_collection=True)`: when a collection is created (multi-scene files?) | Single-scene import went into the active `Collection` (verified). |
| 8 | `export_scene.gltf` `export_format='GLTF_EMBEDDED'` | Only listed when the add-on preference `allow_embedded_format` is on. frap should accept it anyway. |
| 9 | `shade_auto_smooth` GN socket identifiers ("Angle", "Ignore Sharpness") | Dump `modifiers['Smooth by Angle'].node_group.interface.items_tree` before implementing the stand-in. |
| 10 | `object.modifiers.new()` insertion position for HOOK/MULTIRES | Believed to append. Verify. |
| 11 | `anim.keyframe_insert` with keying-set types when objects are in pose mode (keys selected bones) | Not verified here. Covered by the animation research. |
| 12 | The exact `scene.camera` assignment rule of `camera_add` with a view | Verified: set when None. Not verified when a camera already exists (believed: unchanged). |
| 13 | `object.material_slot_assign` in object mode | Believed to be a no-op for meshes. |
| 14 | Vertex ordering of every primitive, for golden position-by-index comparison | Dump with a Blender script and store as fixtures, or compare topology-invariantly. |
| 15 | `render.render(frame_start=..., frame_end=...)` override semantics | Inferred from the RNA description. Verify. |

## 8. Reproducing this data

```sh
B=~/Applications/Blender-5.2.app/Contents/MacOS/Blender
# 1) RNA dump of operators: for each op -> rna=bpy.ops.<ns>.<op>.get_rna_type();
#    for p in rna.properties: identifier, type, subtype, unit, is_hidden, is_skip_save,
#    default / default_array / array_length, hard_min/max, soft_min/max,
#    enum_items (identifier, name), is_enum_flag, fixed_type; plus op.bl_options
$B -b --factory-startup --python dump_ops.py -- ops.json
# 2) poll matrix: op.poll() in states A..E, with and without
#    bpy.context.temp_override(window=wm.windows[0], area=<VIEW_3D area of 'Layout'>, region=<WINDOW region>)
# 3) behavioural checks: small scripts printing selection/active/mode/names/counts after each op
```

Recommendation: commit these dump scripts under `tools/blender-introspect/`, so the `--describe` schema can be diffed against Blender at every Blender LTS upgrade. Operator RNA is the source of truth for frap's registry, and the dump is cheap: about 1 s of Blender runtime.
