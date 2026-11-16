# Blender 5.2 RNA data model reference for frap

Status: research note, written 2026-11-16 for the frap architecture.
Scope: the `bpy.data` / `bpy.types` property model (RNA) that frap's typed property-path layer must mirror.

**How this was produced.** Nearly every name, type, default, range and enum list below was **dumped by introspection from a real Blender 5.2.4 LTS** (`~/Applications/Blender-5.2.app`, build `cd5dc428cce1`, 2026-11-10). The command was `Blender -b --factory-startup --python <script>`, reading `bl_rna.properties`, `bl_rna.functions` and live instances. Behavioural claims (naming, errors, reprs, slotted actions, socket lookup) were also checked by running code. Claims taken from memory or from reading the bundled glTF add-on source (`scripts/addons_core/io_scene_gltf2`, version 5.2.41) are marked. Anything not verified is marked **[UNVERIFIED]**.

**Default columns.** "Default (fresh datablock)" is the value read from a newly created instance (`bpy.data.X.new()` or `modifiers.new()`), **not** the RNA `default` metadata. The two differ in several places, for example `MirrorModifier.use_axis`, `Modifier.show_viewport`, `Material.blend_method` and `NodesModifier.show_group_selector`. The actual value is what Blender users see, so it is what frap must reproduce.

Legend: **RO** means read-only. `(SUBTYPE)` is the RNA subtype, which drives the mathutils wrapper and the unit display. `enum-flag` is a Python `set` of identifiers.

---

## 0. Summary of things an architect must know

1. **ID names are 255 bytes in 5.x, not 63.** `ID.name` has `maxlen=256` (255 bytes of UTF‑8 plus NUL). Truncation happens on a UTF‑8 character boundary: 200×"é" becomes 127 chars, 254 bytes. *Sub-data* names stay **63 bytes** (maxlen 64). That covers bones, vertex groups, shape keys, modifiers, nodes, UV layers, view layers and `parent_bone`. `Attribute.name` reports maxlen 0, meaning the length is dynamic. All of this was verified.
2. **Unique-name rule** (verified): the base name is split at the last `.NNN`. A collision then takes the *lowest free* number ≥ 1, zero-padded to 3 digits. Creating `"Foo"` three times gives `Foo, Foo.001, Foo.002`. Creating `"Foo.001"` while `Foo.001` exists gives `Foo.003`. A duplicate of `"Bar.5"` gives `Bar.001`. A duplicate of `"Baz.999"` gives `Baz.001`. An empty name gives the type default (`"Mesh"`, `"Subdivision"` for a SUBSURF modifier). Setting `.name` to a taken name renames the *assigned* ID to `.001`; the other ID keeps its name. `ID.rename(name, mode='NEVER'|'ALWAYS'|'SAME_ROOT')` can force the opposite and returns `RENAMED_COLLISION_FORCED` etc. Each `bpy.data` collection has its own namespace, so an Object and a Mesh can both be named `Foo`.
3. **Actions are slotted/layered (5.x) and the legacy `action.fcurves` / `action.groups` / `action.id_root` API is gone** (`hasattr` returns False). F-Curves live at `action.layers[i].strips[j].channelbags[k].fcurves`. Each channelbag belongs to an `ActionSlot` (`identifier` like `"OBCube"`, which is a 2-letter ID code plus the name). `AnimData` has `action`, `action_slot`, `action_slot_handle`, `last_slot_identifier` and `action_suitable_slots`. In practice there is one layer and one KEYFRAME strip (`ActionStrips.new(type=...)` only accepts `'KEYFRAME'`).
4. **One Action is shared across an object and its shape keys.** Verified: keying `key_blocks["Key 1"].value` on Cube put the F-Curve into **`CubeAction`** with a second slot `KEKey`, not into a new `KeyAction`. A second object that keys after being assigned the same action gets its own slot (`OBCamera`). Material node-tree keys created a separate `MaterialAction` with slot `NTShader Nodetree`. frap's animation model must be *(Action, Slot) → channelbag*, not *Action → fcurves*.
5. **`keyframe_insert` is keyword-only after `data_path`** in 5.2: `keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME') -> bool`. Calling `obj.keyframe_insert("location", 2, 20)` raises TypeError. frap's bpy front-end should accept the same signature and reject positional extras, so scripts stay Blender-compatible for the golden tests.
6. **Geometry-Nodes modifier inputs moved in 5.2.** `mod["Input_1"]` (ID-property access) now raises `TypeError: this type doesn't support IDProperties`. The new path is **`mod.properties.inputs.Input_1.value`**. Each input struct has `value`, `type` (`'VALUE'`/attribute mode), `attribute_name` and `name`. For "Smooth by Angle" the inputs are `Input_1` "Angle" (float, default 0.5236 = 30°) and `Socket_1` "Ignore Sharpness" (bool). This matters for `object.shade_auto_smooth`.
7. **Engine identifiers:** `'BLENDER_EEVEE'`, `'BLENDER_WORKBENCH'`, `'CYCLES'`. **`'BLENDER_EEVEE_NEXT'` is rejected** in 5.2 (verified error). `render.engine` is a dynamic enum, so introspection without a UI context lists only `BLENDER_EEVEE`. Do not trust `enum_items` for dynamic enums. Hard-code them in frap.
8. **`Material.use_nodes` is deprecated and ignored.** Setting it to False leaves it True, with a `DeprecationWarning: ... expected to be removed in Blender 6.0`. `materials.new()` *always* creates `Principled BSDF` (at −200,100) linked to `Material Output` (at 200,100). `worlds.new()` creates `Background` (Color 0.05 gray, Strength 1) linked to `World Output`. `Material.blend_method` is deprecated in favour of `surface_render_method` (`DITHERED`|`BLENDED`), and a fresh material reports `blend_method='HASHED'`.
9. **The glTF exporter derives alphaMode from the node graph, not from `blend_method`.** The patterns are: Alpha socket constant 1 → OPAQUE; `Math ROUND` or `1 - (x < c)` or `x > c` → MASK with that cutoff; otherwise BLEND. `doubleSided = not use_backface_culling`. The importer sets `surface_render_method` (`OPAQUE`/`MASK` → `DITHERED`, `BLEND` → `BLENDED`).
10. **Socket lookup by name returns the first *enabled* socket.** Verified on `ShaderNodeMix`: with `data_type='RGBA'`, `inputs['A']` returns identifier `A_Color`; with `'VECTOR'` it returns `A_Vector`. Sockets also have a stable `identifier`, but `inputs['A_Color']` does **not** work: name lookup only, as the verified KeyError shows. frap must store `(name, identifier, enabled)` per socket and implement the same rule. Also support integer indices: Blender's own `repr`/`path_from_id` emits `inputs[0]`, not `inputs["Base Color"]`.
11. **Python-only (non-RNA) properties** exist that agents will use: `Object.children`, `children_recursive`, `users_collection`, `users_scene`, and `Mesh.shade_smooth()`, `shade_flat()`, `from_pydata()`. These are not in `bl_rna`. frap's schema export must add them by hand. `Object.dimensions` depends on the *evaluated* bounding box: after changing `scale` without a depsgraph update, `dimensions` was stale (verified). frap should evaluate eagerly so it is always correct, and document that.
12. **Light units:** `lights.new()` gives 10 W for all types. `object.light_add` gives 10 W, except SUN which gets 1.0 (strength in W/m²). The factory-startup "Light" is 1000 W with radius 0.1. **`light_add` now gives 10 W, not the old 1000 W.** The glTF exporter (SPEC mode, the default) converts point/spot as `cd = W / (4π) × 683 × 2^exposure` and sun as `lux = W/m² × 683 × 2^exposure`. AREA lights are *not exported* (warning). `range` is written only if `use_custom_distance`. Spot `outer = spot_size/2` and `inner = outer·(1 − spot_blend)`.
13. **Mesh smoothing is attribute-based (4.1+):** the `sharp_face` (FACE, bool) and `sharp_edge` (EDGE, bool) attributes. The default cube *has* `sharp_face`, so it is flat. `shade_smooth()` removes it, and `set_sharp_from_angle(angle=)` adds `sharp_edge`. `use_auto_smooth`/`auto_smooth_angle` no longer exist. "Auto smooth" is the `Smooth by Angle` GN modifier (essentials asset, `NODES` type, `use_pin_to_last=True`).
14. **mathutils reprs to mirror for print/JSON:** `Vector((0.0, 0.0, 0.0))`, `Euler((0.0, 0.0, 0.0), 'XYZ')`, `Quaternion((1.0, 0.0, 0.0, 0.0))`, `Color((1.0, 1.0, 1.0))` (3-component COLOR only), and a multi-line `Matrix(((1.0, 0.0, 0.0, 0.0),\n        (...)))`. **4-component colors (`diffuse_color`, socket `default_value`), `rotation_axis_angle` and bool arrays are `bpy_prop_array`, not mathutils.** Their repr is the *path* (`bpy.data.materials['Material'].diffuse_color`). frap's JSON output should simply emit arrays and carry a type tag.

---

## 1. Property-path grammar and value model

### 1.1 Grammar frap should accept, following Blender's own `repr`/`path_from_id` forms

```
path       := root accessor*
root       := 'bpy.data.' collection_name          # bpy.data.objects
            | 'bpy.context.' context_member        # bpy.context.object, .scene, .view_layer ...
            | 'D.' ... | 'C.' ...                  # optional aliases (Blender console convention)
accessor   := '.' identifier                       # RNA property or Python-defined property
            | '[' string ']'                       # collection lookup by name: ['Cube'] or ["Cube"]
            | '[' int ']'                          # index: [0], negative allowed ([-1])
            | '[' int ']' on an array property     # location[1] -> scalar
call       := accessor '(' args ')'                # RNA function: new(), remove(), keyframe_insert()...
```

Verified behaviours:

| Expression | Result in Blender 5.2 |
|---|---|
| `bpy.data.objects['Nope']` | `KeyError: 'bpy_prop_collection[key]: key "Nope" not found'` |
| `bpy.data.objects.get('Nope')` | `None` |
| `bpy.data.objects.find('Cube')` | index `1` (−1 if missing) |
| `bpy.data.objects[0]`, `[-1]` | ordered by name (Camera, Cube, Light in factory scene) |
| `obj.path_resolve('location[1]')` | `7.0` (scalar) |
| `obj.path_resolve('modifiers["D"].ratio')` | `1.0` |
| `mod.path_from_id('ratio')` | `modifiers["D"].ratio` (double quotes inside a path) |
| `socket.path_from_id('default_value')` | `nodes["Principled BSDF"].inputs[0].default_value` (**index**, not name) |
| `repr(obj)` | `bpy.data.objects['Cube']` (single quotes at root, double quotes below) |
| `repr(mod)` | `bpy.data.objects['Cube'].modifiers["D"]` |

Data paths stored in F-Curves are **relative to the owning ID** and use the `path_from_id` form, for example `pose.bones["Bone.001"].rotation_quaternion`, `modifiers["Bevel"].width` and `nodes["Principled BSDF"].inputs[2].default_value`. frap must produce byte-identical strings, because the glTF exporter pattern-matches on them.

### 1.2 Assignment semantics (verified)

| Statement | Behaviour |
|---|---|
| `o.location = (1,2,3)` | whole-vector assign; any sequence of length 3 (ints are coerced) |
| `o.location.x = 5` / `o.location[1] = 7` | component assign |
| `o.location = (1,2)` | `ValueError: ... sequences of dimension 0 should contain 3 items, not 2` |
| `o.location = 5` | `TypeError` |
| `o.type = 'EMPTY'` | `AttributeError: bpy_struct: attribute "type" from "Object" is read-only` |
| `o.rotation_mode = 'FOO'` | `TypeError: ... enum "FOO" not found in ('QUATERNION', 'XYZ', ...)` (message lists valid items, which is good for LLM self-correction) |
| `mod.delimit = {'UV','SEAM'}` | enum-flag takes a set; reads back as a set |
| `o.rotation_mode = 'QUATERNION'` | **converts** the current Euler into `rotation_quaternion`, so values are preserved across modes |
| `o.dimensions = (1,1,1)` | writes `scale` from the evaluated bounding box |
| Float/int ranges | hard min/max are **clamped** silently, not rejected (standard RNA behaviour; [UNVERIFIED for every prop]) |

### 1.3 Value types to JSON (recommended frap encoding)

| RNA type/subtype | Python object | Repr example | Suggested JSON |
|---|---|---|---|
| FLOAT[3] TRANSLATION/XYZ/XYZ_LENGTH/DIRECTION | `mathutils.Vector` | `Vector((0.0, 0.0, 0.0))` | `[x,y,z]` + `"$type":"Vector"` |
| FLOAT[3] EULER | `mathutils.Euler` (has `.order`) | `Euler((0.0, 0.0, 0.0), 'XYZ')` | `[x,y,z]` (+ order) |
| FLOAT[4] QUATERNION | `mathutils.Quaternion` | `Quaternion((1.0, 0.0, 0.0, 0.0))` (**w first**) | `[w,x,y,z]` |
| FLOAT[3] COLOR | `mathutils.Color` | `Color((1.0, 1.0, 1.0))` | `[r,g,b]` |
| FLOAT[4] COLOR / COLOR_GAMMA | `bpy_prop_array` | path repr | `[r,g,b,a]` |
| FLOAT[4] AXISANGLE | `bpy_prop_array` | path repr | `[angle, x, y, z]` (**angle first**) |
| FLOAT[16] MATRIX dims 4×4 | `mathutils.Matrix` | `Matrix(((1.0, 0.0, 0.0, 0.0),\n        (0.0, ...)))` (row-major rows) | `[[row0],[row1],[row2],[row3]]` |
| FLOAT[9] MATRIX 3×3 (`Bone.matrix`) | `Matrix` 3x3 | | `[[..],[..],[..]]` |
| BOOLEAN[n] | `bpy_prop_array` | path repr | `[false,false,false]` |
| ENUM | `str` | `'XYZ'` | `"XYZ"` |
| ENUM flag | `set[str]` | `{'UV', 'SEAM'}`, empty `set()` | `["SEAM","UV"]` (sorted) |
| POINTER to ID | ID | `bpy.data.objects['Cube']` | `{"$ref":"objects","name":"Cube"}` |
| POINTER to non-ID struct | struct | path repr | nested object or path string |
| COLLECTION | `bpy_prop_collection` | path repr | list of names or refs |

`str(Vector)` is `<Vector (0.0000, 0.0000, 0.0000)>` and `str(Matrix)` is `<Matrix 4x4 (1.0000, ...)` on multiple lines. `print(...)` uses `str`, and the REPL echo uses `repr`. Recommendation: frap's `print(path)` returns JSON. A `"py_repr"` field can be added if exact Blender text is ever needed.

### 1.4 `bpy.context` members relevant to frap (headless)

Verified list contains, among others: `object`, `active_object`, `selected_objects`, `selected_editable_objects`, `visible_objects`, `selectable_objects`, `editable_objects`, `objects_in_mode`, `objects_in_mode_unique_data`, `edit_object`, `pose_object`, `mode`, `scene`, `view_layer`, `collection`, `layer_collection`, `active_bone`, `active_pose_bone`, `selected_bones`, `selected_pose_bones`, `editable_bones`, `visible_bones`, `active_action`, `active_nla_track`, `active_nla_strip`, `tool_settings`, `blend_data`, `preferences`, `evaluated_depsgraph_get()`, `temp_override()`.

`bpy.context.mode` values (string, distinct from `Object.mode`): `OBJECT`, `EDIT_MESH`, `EDIT_ARMATURE`, `POSE`, `SCULPT`, `PAINT_WEIGHT`, etc. [names partly from memory]. `Object.mode` enum: `OBJECT, EDIT, POSE, SCULPT, VERTEX_PAINT, WEIGHT_PAINT, TEXTURE_PAINT, PARTICLE_EDIT, EDIT_GPENCIL, SCULPT_GREASE_PENCIL, PAINT_GREASE_PENCIL, WEIGHT_GREASE_PENCIL, VERTEX_GREASE_PENCIL, SCULPT_CURVES` (**RO**; it changes only via `object.mode_set`).

Selection and active state are **per view layer**: `obj.select_get(view_layer=None)`, `obj.select_set(state, view_layer=None)`, `obj.hide_get()`, `obj.hide_set()`, `obj.visible_get()`, `view_layer.objects.active`, `view_layer.objects.selected`. In factory startup the active object is `Cube` and it is selected. `hide_set` (view-layer "eye", stored on Base) is separate from `Object.hide_viewport` (global "monitor" toggle) and from `hide_render`.

---

## 2. `bpy.data` collections and ID basics

### 2.1 Collection functions (verified signatures; `?` = optional)

| `bpy.data.` | Element type | `new(...)` | `remove(...)` | Other |
|---|---|---|---|---|
| `objects` | `Object` | `new(name, object_data)`: `object_data=None` gives an EMPTY; type is inferred from data | `remove(object, do_unlink?, do_id_user?, do_ui_user?)` | |
| `meshes` | `Mesh` | `new(name)` | same | `new_from_object(object, preserve_all_data_layers?, depsgraph?)` |
| `materials` | `Material` | `new(name)` (always has node tree) | same | `create_gpencil_data`, `remove_gpencil_data` |
| `images` | `Image` | `new(name, width, height, alpha?, float_buffer?, stereo3d?, is_data?, tiled?)` | same | `load(filepath, check_existing?)` |
| `textures` | `Texture` | `new(name, type)` type ∈ NONE BLEND CLOUDS DISTORTED_NOISE IMAGE MAGIC MARBLE MUSGRAVE NOISE STUCCI VORONOI WOOD | same | (only needed for DISPLACE modifier `texture`) |
| `cameras` | `Camera` | `new(name)` | same | |
| `lights` | `Light` (`PointLight`/`SunLight`/`SpotLight`/`AreaLight`) | `new(name, type)` type ∈ POINT SUN SPOT AREA | same | |
| `armatures` | `Armature` | `new(name)` | same | |
| `actions` | `Action` | `new(name)` | same | |
| `scenes` | `Scene` | `new(name)`: no world, no camera, 1 view layer | `remove(scene, do_unlink?)` | |
| `worlds` | `World` | `new(name)` (Background + World Output nodes) | same | |
| `collections` | `Collection` | `new(name)`: **not linked** until `parent.children.link(c)` | same | |
| `node_groups` | `NodeTree` | `new(name, type)` type ∈ GeometryNodeTree CompositorNodeTree ShaderNodeTree TextureNodeTree | same | |
| `shape_keys` | `Key` | none (created by `Object.shape_key_add`) | none | |

All collections also have `tag(value)`, plus `get(name, default)`, `find(name)`, `keys()`, `values()`, `items()`, `len()` and iteration. Other collections in 5.2 BlendData: `libraries, screens, window_managers, lattices, curves, metaballs, fonts, brushes, texts, speakers, sounds, particles, palettes, annotations, grease_pencils, movieclips, masks, linestyles, cache_files, paint_curves, workspaces, lightprobes, hair_curves, pointclouds, volumes`, plus `all_ids`. BlendData props: `filepath` (RO), `is_dirty`, `is_saved`, `use_autopack`, `version`, `colorspace`.

Sub-collections with new/remove (verified):

| Path | Functions |
|---|---|
| `Object.modifiers` (`ObjectModifiers`) | `new(name, type)`, `remove(modifier)`, `clear()`, `move(from_index, to_index)`, `active` |
| `Object.vertex_groups` (`VertexGroups`) | `new(name?)` (default name `"Group"`), `remove(group)`, `clear()`, `active`, `active_index` |
| `Mesh.materials` / `IDMaterials` | `append(material)`, `pop(index?)`, `clear()` |
| `Mesh.uv_layers` | `new(name?, do_init?)`, `remove(layer)`, `active`, `active_index`, `active_render`, `active_render_index` |
| `Mesh.attributes` / `color_attributes` | `new(name, type, domain)`, `remove(attr)`, `domain_size(domain)`, `active`, `active_color`, `active_color_name`, `default_color_name`, `render_color_index` |
| `Mesh.vertices/edges/loops/polygons` | `add(count)` |
| `NodeTree.nodes` | `new(type: str bl_idname)`, `remove(node)`, `clear()`, `active` |
| `NodeTree.links` | `new(input, output, verify_limits?, handle_dynamic_sockets?)` (**first arg is the *from* socket, i.e. an output**, despite the name `input`), `remove(link)`, `clear()` |
| `Collection.objects` / `.children` | `link(x)`, `unlink(x)` |
| `Armature.edit_bones` | `new(name)`, `remove(bone)`, `active` (EDIT mode only, empty otherwise) |
| `Action.slots` | `new(id_type, name)`, `remove(action_slot)`, `active` |
| `Action.layers` | `new(name)`, `remove(anim_layer)` |
| `ActionLayer.strips` | `new(type='KEYFRAME')`, `remove(anim_strip)` |
| `ActionKeyframeStrip.channelbags` | `new(slot)`, `remove(channelbag)`; strip fns `channelbag(slot, ensure=False)`, `key_insert(...)` |
| `ActionChannelbag.fcurves` | `new(data_path, index?, group_name?)`, `new_from_fcurve(source, data_path?)`, `ensure(data_path, index?, group_name?)`, `find(data_path, index?)`, `remove(fcurve)`, `clear()` |
| `FCurve.keyframe_points` | `insert(frame, value, options?={'REPLACE','NEEDED','FAST'}, keyframe_type?)`, `add(count)`, `remove(keyframe, fast?)`, `clear()`, `sort()`, `deduplicate()`, `handles_recalc()` |
| `AnimData.nla_tracks` | `new(prev?)`, `remove(track)`, `active` |
| `NlaTrack.strips` | `new(name, start:int, action)`, `remove(strip)` |

Behaviour of `NodeTree.links.new`: linking a second output into an already-linked input **replaces** the existing link. Verified: the link count stayed the same.

### 2.2 `ID` common properties (every datablock)

| Property | Type | Notes |
|---|---|---|
| `name` | str, max 255 bytes | unique per collection; see §0.1–0.2 |
| `name_full` | str RO | includes library prefix |
| `id_type` | enum RO | ACTION ARMATURE … OBJECT … WORLD |
| `users` | int RO | object-data user count; `use_fake_user=True` adds 1 (verified 1→2) |
| `use_fake_user` | bool | keeps orphan datablocks on save |
| `use_extra_user`, `is_embedded_data`, `is_evaluated`, `original`, `library`, `session_uid`, `tag`, `is_runtime_data`, `is_missing`, `is_editable`, `asset_data`, `override_library`, `preview` | | mostly irrelevant for frap |
| fns | | `rename(name, mode='NEVER')`, `copy()`, `user_clear()`, `user_remap(new_id)`, `animation_data_create()`, `animation_data_clear()`, `evaluated_get(depsgraph)`, `update_tag()` |

User counting (verified): a mesh used by one object has `users == 1`. An object created with `objects.new` but **not linked to any collection** has `users == 0` and will be purged on save. frap should model `users` for orphan purge on `wm.save_as_mainfile` and for `remove(do_unlink=True)`.

---

## 3. Object

Full table: [Appendix A › Object](#tbl-object). Key points:

* `type` (**RO**): `MESH, CURVE, SURFACE, META, FONT, CURVES, POINTCLOUD, VOLUME, GREASEPENCIL, ARMATURE, LATTICE, EMPTY, LIGHT, LIGHT_PROBE, CAMERA, SPEAKER`. It is determined by `data` at creation. frap needs MESH, EMPTY, CAMERA, LIGHT, ARMATURE.
* Transform: `location` (Vector), `rotation_mode` (`QUATERNION, XYZ, XZY, YXZ, YZX, ZXY, ZYX, AXIS_ANGLE`, default `XYZ`), `rotation_euler`, `rotation_quaternion` (w,x,y,z), `rotation_axis_angle` (angle,x,y,z; default (0,0,1,0)), `scale`, plus `delta_location`, `delta_rotation_euler`, `delta_rotation_quaternion`, `delta_scale`. Only the channel matching `rotation_mode` is used. The others keep stale values but are converted on a mode change.
* Matrices: `matrix_basis` is the local TRS from loc/rot/scale (+delta). `matrix_local` is relative to the parent and includes `matrix_parent_inverse`. `matrix_world = parent.matrix_world @ matrix_parent_inverse @ matrix_basis` for `parent_type='OBJECT'`. Writing `matrix_world`/`matrix_basis` decomposes back into loc/rot/scale. `matrix_parent_inverse` defaults to identity. `object.parent_set` (operator) sets it to the inverse parent world matrix, while assigning `obj.parent = p` in RNA leaves it as identity and makes the child "jump". frap must reproduce both behaviours.
* Parenting: `parent` (ptr Object), `parent_type` (`OBJECT, ARMATURE, LATTICE, VERTEX, VERTEX_3, BONE`), `parent_bone` (str, 63 bytes), `parent_vertices` (int[3]). `children` / `children_recursive` / `users_collection` / `users_scene` are **Python-defined** (not RNA). For glTF skinning the exporter expects a mesh parented to an armature with an `ARMATURE` modifier. `parent_type='ARMATURE'` is optional.
* Dimensions: `dimensions` (XYZ_LENGTH) is derived from the evaluated bounding box × scale and is writable. It reads stale values until the depsgraph is evaluated.
* Visibility: `hide_viewport`, `hide_render`, `hide_select`, plus the view-layer `hide_get/hide_set`. Probe and other hide flags are irrelevant.
* Display: `display_type` (`BOUNDS, WIRE, SOLID, TEXTURED`, default TEXTURED), `show_wire`, `show_all_edges`, `show_in_front`, `show_name`, `show_axis`, `show_bounds`, `display_bounds_type`, `color` (RGBA object color, used by Workbench `color_type='OBJECT'`).
* Empties: `empty_display_type` (`PLAIN_AXES, ARROWS, SINGLE_ARROW, CIRCLE, CUBE, SPHERE, CONE, IMAGE`), `empty_display_size` (1.0).
* Materials: `material_slots` (coll of `MaterialSlot{link: 'OBJECT'|'DATA' (default DATA), material, slot_index, name}`), `active_material`, `active_material_index`. Slots are created by appending to `mesh.materials` (DATA link) or via the `object.material_slot_add` operator.
* `modifiers`, `constraints`, `vertex_groups`, `pose` (armatures), `active_shape_key`, `active_shape_key_index`, `use_shape_key_edit_mode`, `animation_data` (RO ptr; create via `animation_data_create()`), `mode` (RO).
* Functions (verified): `select_get/select_set/hide_get/hide_set/visible_get`, `shape_key_add(name="Key", from_mix=True)`, `shape_key_remove(key)`, `shape_key_clear()`, `to_mesh(preserve_all_data_layers?, depsgraph?)`, `to_mesh_clear()`, `ray_cast(origin, direction, distance?, depsgraph?)` → (result, location, normal, index), `closest_point_on_mesh`, `convert_space(pose_bone?, matrix, from_space, to_space)`, `calc_matrix_camera`, `camera_fit_coords`, `find_armature()`, `is_modified(scene, settings)`, `update_from_editmode()`, `evaluated_get(depsgraph)`.

---

## 4. Mesh

Full table: [Appendix A › Mesh](#tbl-mesh).

* Element collections: `vertices` (`MeshVertex{co, normal RO, select, hide, groups RO (VertexGroupElement{group, weight}), index RO, undeformed_co RO}`), `edges` (`MeshEdge{vertices int[2], select, hide, use_seam, use_edge_sharp, is_loose RO, index}`), `loops` (`MeshLoop{vertex_index, edge_index, normal RO, tangent RO, bitangent RO, bitangent_sign RO}`), `polygons` (`MeshPolygon{vertices, loop_start, loop_total RO, material_index, select, hide, use_smooth, normal RO, center RO, area RO, flip()}`) and `loop_triangles` (RO).
* Normals: `normals_domain` RO (`POINT|FACE|CORNER`), `vertex_normals`, `polygon_normals`, `corner_normals` (RO collections of `MeshNormalValue.vector`). Custom normals are set with `normals_split_custom_set(normals)` / `normals_split_custom_set_from_vertices(normals)`.
* **Attributes are the source of truth.** Built-in names seen on the default cube: `position` (FLOAT_VECTOR/POINT), `.edge_verts` (INT32_2D/EDGE), `.corner_vert`, `.corner_edge` (INT/CORNER), `sharp_face` (BOOLEAN/FACE), `UVMap` (FLOAT2/CORNER), and the selection layers `.select_vert`, `.select_edge`, `.select_poly`, `.uv_select_vert`, `.uv_select_edge`, `.uv_select_face`. Optional: `sharp_edge` (EDGE bool), `material_index` (FACE int), `crease_edge` / `crease_vert` (FLOAT), `bevel_weight_edge` / `bevel_weight_vert` (FLOAT; the Bevel modifier's `edge_weight` / `vertex_weight` default to these names), `custom_normal` (CORNER, 4.4+ [UNVERIFIED name]), and UV-pin `.pin` sublayers. Names starting with `.` are internal (`Attribute.is_internal`).
* `Attribute{name, data_type RO, domain RO, storage_type RO (ARRAY|SINGLE — new 5.x), is_internal RO, is_required RO}`. data_type ∈ `FLOAT, INT, BOOLEAN, FLOAT_VECTOR, FLOAT_COLOR, QUATERNION, FLOAT4X4, STRING, INT8, INT16_2D, INT32_2D, FLOAT2, FLOAT4, BYTE_COLOR`. domain ∈ `POINT, EDGE, FACE, CORNER, CURVE, INSTANCE, LAYER`. The value accessors are `attr.data[i].value` / `.vector` / `.color`.
* UV: `uv_layers` → `MeshUVLoopLayer{name (63 bytes), active, active_render, data (MeshUVLoop{uv, pin_uv}), uv (Float2AttributeValue{vector}), pin}`. glTF TEXCOORD_n order = `uv_layers` order.
* Vertex colors: `color_attributes` (FLOAT_COLOR or BYTE_COLOR on POINT or CORNER), `attributes.active_color_name`, `default_color_name` (render). The legacy `vertex_colors` still exists as a compatibility view. The glTF exporter exports `COLOR_0` from the active/render color attribute (exporter option `export_vertex_color` ∈ `MATERIAL|ACTIVE|NAME|NONE`, default `MATERIAL`, meaning it is exported only when a material uses it; verified in add-on source).
* `materials` (IDMaterials), `shape_keys` (ptr Key, RO), `total_vert_sel/edge_sel/face_sel` (RO).
* RNA functions: `transform(matrix, shape_keys?)`, `flip_normals()`, `set_sharp_from_angle(angle=)`, `split_faces()`, `calc_tangents(uvmap?)`, `free_tangents()`, `calc_loop_triangles()`, `calc_smooth_groups()`, `normals_split_custom_set*`, `update()`, `clear_geometry()`, `validate()`, `validate_material_indices()`, `count_selected_items()`. **Python-defined:** `from_pydata(vertices, edges, faces)`, `shade_smooth()`, `shade_flat()`, `edge_keys`.
* Edit-mode element access (`bmesh`) is *not* RNA. In Blender, `mesh.vertices[i].select` reflects edit-mode selection only after leaving edit mode or calling `obj.update_from_editmode()`. frap keeps its BMesh-like structure authoritative and should expose RNA-equivalent reads that are always fresh. Document this as an intentional, harmless divergence.

---

## 5. Modifiers

### 5.1 Base `Modifier` properties (verified actual values on `modifiers.new`)

| Property | Type | Fresh value | Notes |
|---|---|---|---|
| `name` | str (63 bytes) | given name, uniquified per object (`Bevel`, `Bevel.001`); empty → short UI name (`"Subdivision"` for SUBSURF) | |
| `type` | enum RO | | full list below |
| `show_viewport` | bool | **True** (RNA default says False) | |
| `show_render` | bool | True | glTF exporter `export_apply` uses the viewport (or render) setting [UNVERIFIED which] |
| `show_in_editmode` | bool | True | |
| `show_on_cage` | bool | False | |
| `show_expanded` | bool | True | UI only |
| `is_active` | bool | True (last added becomes active) | |
| `use_pin_to_last` | bool | False (True for Smooth by Angle) | keeps modifier at the bottom of the stack |
| `is_override_data` | bool RO | | |
| `use_apply_on_spline` | bool | False | curves only |
| `execution_time` | float RO | | |
| `persistent_uid` | int RO | | stable id (5.x) |

`Object.modifiers.new(name, type)` type enum (mesh-relevant; GP and physics omitted in the tables): `DATA_TRANSFER, MESH_CACHE, MESH_SEQUENCE_CACHE, NORMAL_EDIT, WEIGHTED_NORMAL, UV_PROJECT, UV_WARP, VERTEX_WEIGHT_EDIT, VERTEX_WEIGHT_MIX, VERTEX_WEIGHT_PROXIMITY, ARRAY, BEVEL, BOOLEAN, BUILD, DECIMATE, EDGE_SPLIT, NODES, MASK, MIRROR, MESH_TO_VOLUME, MULTIRES, REMESH, SCREW, SKIN, SOLIDIFY, SUBSURF, TRIANGULATE, VOLUME_TO_MESH, WELD, WIREFRAME, ARMATURE, CAST, CURVE, DISPLACE, HOOK, LAPLACIANDEFORM, LATTICE, MESH_DEFORM, SHRINKWRAP, SIMPLE_DEFORM, SMOOTH, CORRECTIVE_SMOOTH, LAPLACIANSMOOTH, SURFACE_DEFORM, WARP, WAVE, VOLUME_DISPLACE, CLOTH, COLLISION, DYNAMIC_PAINT, EXPLODE, FLUID, OCEAN, PARTICLE_INSTANCE, PARTICLE_SYSTEM, SOFT_BODY, SURFACE` plus 30 `GREASE_PENCIL_*` and `LINEART`. `MESH_TO_VOLUME`, `VOLUME_DISPLACE` and `LINEART` return `None` on a mesh object, because they are type-restricted.

Default names from `bpy.ops.object.modifier_add(type=T)` (verified 5.2; these are the DNA/UI-short names, *not* the enum labels): SUBSURF `Subdivision`, MIRROR `Mirror`, ARRAY `Array`, BEVEL `Bevel`, SOLIDIFY `Solidify`, BOOLEAN `Boolean`, TRIANGULATE `Triangulate`, DECIMATE `Decimate`, WELD `Weld`, DISPLACE `Displace`, SCREW `Screw`, SKIN `Skin`, WIREFRAME `Wireframe`, SMOOTH `Smooth`, CAST `Cast`, CURVE `Curve`, **SIMPLE_DEFORM `SimpleDeform`**, LATTICE `Lattice`, ARMATURE `Armature`, **EDGE_SPLIT `EdgeSplit`**, **WEIGHTED_NORMAL `WeightedNormal`**, REMESH `Remesh`, SHRINKWRAP `Shrinkwrap`, NORMAL_EDIT `NormalEdit`, UV_PROJECT `UVProject`, UV_WARP `UVWarp`, MASK `Mask`, BUILD `Build`, LAPLACIANSMOOTH `LaplacianSmooth`, CORRECTIVE_SMOOTH `CorrectiveSmooth`, WAVE `Wave`, WARP `Warp`, HOOK `Hook`, DATA_TRANSFER `DataTransfer`; NODES `GeometryNodes` [UNVERIFIED — the operator run for NODES/MULTIRES was skipped because the batch hung in background mode]. `modifiers.new("", T)` uses the same names (verified for SUBSURF → `Subdivision`). Uniquification is per object (`Bevel`, `Bevel.001`).

Per-type property tables with every RNA property, type, fresh default and enum items: **[Appendix B](#appendix-b-modifier-property-tables)**. Tier 1 (the user's list plus glTF-relevant deformers): SUBSURF, MIRROR, ARRAY, BEVEL, SOLIDIFY, BOOLEAN, TRIANGULATE, DECIMATE, WELD, DISPLACE, SCREW, SKIN, WIREFRAME, SMOOTH, CAST, CURVE, SIMPLE_DEFORM, LATTICE, ARMATURE, EDGE_SPLIT, WEIGHTED_NORMAL, REMESH, SHRINKWRAP, NODES. Tier 2 is also included.

Notable 5.2 details:

* `BooleanModifier.solver` ∈ `FLOAT, EXACT, MANIFOLD` (default EXACT). MANIFOLD is new in 4.5/5.x. `operand_type` ∈ OBJECT/COLLECTION. `material_mode` ∈ INDEX/TRANSFER.
* `BevelModifier.edge_weight` / `vertex_weight` are **attribute-name strings** (`'bevel_weight_edge'`, `'bevel_weight_vert'`), not booleans. `affect` ∈ VERTICES/EDGES. `limit_method` default **ANGLE** (30°).
* `SubsurfModifier.use_custom_normals`, `adaptive_space` (`PIXEL|OBJECT`), `adaptive_pixel_size`, `adaptive_object_edge_length`: adaptive subdivision moved out of Cycles-experimental into the modifier in 5.x.
* `TriangulateModifier.quad_method` default `SHORTEST_DIAGONAL`, `keep_custom_normals`.
* `DisplaceModifier` needs a `Texture` datablock (`bpy.data.textures.new(name, type)`). `direction` ∈ `X, Y, Z, NORMAL, CUSTOM_NORMAL, RGB_TO_XYZ`. Procedural textures (CLOUDS etc.) need an implementation of Blender's legacy texture noise to match golden tests. Consider supporting `IMAGE` and NONE first.
* `ArrayModifier.fit_type` ∈ `FIXED_COUNT, FIT_LENGTH, FIT_CURVE`. `relative_offset_displace` (1,0,0), `use_relative_offset` True.
* `SolidifyModifier.offset` default **−1**, `thickness` 0.01, `solidify_mode` ∈ EXTRUDE/NON_MANIFOLD.
* `WeldModifier.mode` ∈ `ALL|CONNECTED`. The Python identifier for "Distance" is `merge_threshold`.

### 5.2 "Smooth by Angle" (geometry-nodes) — verified 5.2 API

* Operators: `object.shade_auto_smooth(use_auto_smooth=True, angle=0.5236)` adds the modifier, `object.shade_smooth_by_angle(angle, keep_sharp_edges)` sets `sharp_edge` attributes directly (no modifier), and `object.shade_smooth(keep_sharp_edges)` / `object.shade_flat()`. Verified property lists: `shade_auto_smooth{use_auto_smooth, angle}`, `shade_smooth_by_angle{angle, keep_sharp_edges}`, `shade_smooth{keep_sharp_edges}`.
* The resulting modifier is `name="Smooth by Angle"`, `type='NODES'`, `node_group=bpy.data.node_groups["Smooth by Angle"]` (`is_modifier=True`), with `use_pin_to_last=True` and `show_group_selector=False`. The interface is `Input_0` "Mesh" (Geometry), `Input_1` "Angle" (Float, 0.5236), `Socket_1` "Ignore Sharpness" (Bool False) and `Output_2` "Mesh".
* Read/write the angle: **`mod.properties.inputs.Input_1.value`**. `mod.properties` is a `GeometryNodesModifierInterface{inputs, outputs, panels}`. Each input struct is dynamically typed: `{name, value, type: 'VALUE'|'ATTRIBUTE'…, attribute_name, layer_name?}`. `mod["Input_1"]` **raises** in 5.2.
* frap does not need a general geometry-nodes evaluator. Implement the `NODES` modifier with a **built-in registry of known node groups** (start with "Smooth by Angle") keyed by group name, and treat other groups as unsupported (error). The essentials asset node tree also has to exist in `bpy.data.node_groups` for parity.

---

## 6. Material and shader nodes

### 6.1 Material

Full table: [Appendix A › Material](#tbl-material). Important fields for glTF and rendering:

| Property | Fresh value | Notes |
|---|---|---|
| `use_nodes` | True | **deprecated, ignored on set** (removal in 6.0) |
| `node_tree` | ShaderNodeTree (embedded, RO ptr) | contains `Principled BSDF` and `Material Output` |
| `surface_render_method` | `DITHERED` | `DITHERED|BLENDED` (EEVEE). Importer: BLEND → BLENDED |
| `blend_method` | `HASHED` | deprecated mirror of surface_render_method (`OPAQUE, CLIP, HASHED, BLEND`) |
| `alpha_threshold` | 0.5 | legacy; exporter ignores it and detects clip from nodes |
| `use_backface_culling` | False | glTF `doubleSided = not use_backface_culling` |
| `use_backface_culling_shadow`, `use_transparency_overlap` (True), `use_transparent_shadow` (True), `use_raytrace_refraction`, `thickness_mode` (`SPHERE|SLAB`), `displacement_method` (`BUMP|DISPLACEMENT|BOTH`) | | EEVEE |
| `diffuse_color` | (0.8,0.8,0.8,1) | viewport/Workbench color; glTF importer sets it to the base color when there is no texture |
| `metallic` 0, `roughness` 0.4, `specular_intensity` 0.5, `specular_color` (1,1,1) | | viewport-only |
| `pass_index` | 0 | |

### 6.2 Node tree API

* `nodes.new(type="ShaderNodeTexImage")` takes the `bl_idname` string. The default `name` is the label (`"Image Texture"`), uniquified with `.001`. `label` is `''`, `location` is (0,0) and `width` is 240 for the image node.
* `Node` common props: `name`, `label`, `type` (RO legacy enum string like `'TEX_IMAGE'`, `'BSDF_PRINCIPLED'`), `bl_idname`, `location`, `location_absolute` (new), `width`, `height`, `dimensions` RO, `parent` (frame), `hide`, `mute`, `select`, `use_custom_color`, `color`, `inputs`, `outputs`, `internal_links`.
* `NodeSocket` props: `name`, `identifier` (RO), `label`, `description`, `is_output` RO, `enabled`, `hide`, `hide_value`, `is_unavailable` RO, `is_inactive` RO, `is_multi_input` RO, `link_limit`, `type`, `bl_idname`, `node` RO, `is_linked` (Python-visible), `links` (Python-defined) and `default_value` (per socket class).
* `links.new(from_output_socket, to_input_socket)` returns `NodeLink{from_node, from_socket, to_node, to_socket, is_valid, is_muted, is_hidden}`.
* Lookups: `inputs['Name']` (first **enabled** socket with that name; KeyError if missing), `inputs[i]`, `inputs.get('Name')` (None if missing). Identifiers are not usable as keys.
* Removed in 5.x: `ShaderNodeSeparateRGB`, `ShaderNodeCombineRGB` and `ShaderNodeSeparateHSV` raise `Node type ... undefined`. Use `ShaderNodeSeparateColor` / `ShaderNodeCombineColor` with `mode`. `ShaderNodeMixRGB` still exists as "Mix (Legacy)".

### 6.3 Principled BSDF in 5.2 (verified, in socket order)

`ShaderNodeBsdfPrincipled` (`type='BSDF_PRINCIPLED'`). Properties: `distribution` ∈ `GGX|MULTI_GGX` (default **MULTI_GGX**), `subsurface_method` ∈ `BURLEY|RANDOM_WALK|RANDOM_WALK_SKIN|RANDOM_WALK_LEGACY` (default RANDOM_WALK).

| # | Input | Socket type | Default | glTF mapping (exporter) |
|---|---|---|---|---|
| 0 | `Base Color` | Color | (0.8,0.8,0.8,1) | baseColorFactor/Texture |
| 1 | `Metallic` | FloatFactor | 0.0 | metallicFactor (B channel of MR texture) |
| 2 | `Roughness` | FloatFactor | 0.5 | roughnessFactor (G channel) |
| 3 | `IOR` | Float | 1.5 | KHR_materials_ior |
| 4 | `Alpha` | FloatFactor | 1.0 | baseColor alpha + alphaMode detection |
| 5 | `Thin Wall` | Bool | False | **new in 5.x**; KHR_materials_volume interplay [UNVERIFIED exporter support] |
| 6 | `Normal` | Vector (hide_value) | — | normalTexture via Normal Map node |
| 7 | `Weight` | Float | 0.0 | *disabled* (internal) |
| 8 | `Diffuse Roughness` | FloatFactor | 0.0 | **new in 4.3+**; not exported [UNVERIFIED] |
| 9 | `Subsurface Weight` | FloatFactor | 0.0 | (KHR_materials_diffuse_transmission? no; not exported [UNVERIFIED]) |
| 10 | `Subsurface Radius` | Vector | (1, 0.2, 0.1) | — |
| 11 | `Subsurface Scale` | FloatDistance | 0.005 | — |
| 12 | `Subsurface IOR` | FloatFactor | 1.4 | *disabled* unless RANDOM_WALK_SKIN |
| 13 | `Subsurface Anisotropy` | FloatFactor | 0.0 | — |
| 14 | `Specular IOR Level` | FloatFactor | 0.5 | KHR_materials_specular (specularFactor = 2×level, clamped) [mapping detail UNVERIFIED] |
| 15 | `Specular Tint` | Color | (1,1,1,1) | KHR_materials_specular specularColorFactor |
| 16 | `Anisotropic` | FloatFactor | 0.0 | KHR_materials_anisotropy strength |
| 17 | `Anisotropic Rotation` | FloatFactor | 0.0 | anisotropy rotation (×2π) |
| 18 | `Tangent` | Vector (hide_value) | — | anisotropy texture via Tangent/Normal-map pattern |
| 19 | `Transmission Weight` | FloatFactor | 0.0 | KHR_materials_transmission |
| 20 | `Coat Weight` | FloatFactor | 0.0 | KHR_materials_clearcoat clearcoatFactor |
| 21 | `Coat Roughness` | FloatFactor | 0.03 | clearcoatRoughnessFactor |
| 22 | `Coat IOR` | Float | 1.5 | (fixed 1.5 in glTF) |
| 23 | `Coat Tint` | Color | (1,1,1,1) | — |
| 24 | `Coat Normal` | Vector (hide_value) | — | clearcoatNormalTexture |
| 25 | `Sheen Weight` | FloatFactor | 0.0 | KHR_materials_sheen |
| 26 | `Sheen Roughness` | FloatFactor | 0.5 | sheenRoughnessFactor |
| 27 | `Sheen Tint` | Color | (1,1,1,1) | sheenColorFactor |
| 28 | `Emission Color` | Color | **(1,1,1,1)** | emissiveFactor (× strength) |
| 29 | `Emission Strength` | Float | 0.0 | >1 → KHR_materials_emissive_strength |
| 30 | `Thin Film Thickness` | FloatWavelength | 0.0 | KHR_materials_iridescence thickness (nm) |
| 31 | `Thin Film IOR` | Float | 1.33 | iridescenceIor |

Output: `BSDF`. Volume (KHR_materials_volume) uses `Volume Absorption` / `Principled Volume` on the Material Output `Volume` input, plus Material Output **`Thickness`** (new socket in 5.x, Float, hide_value). Dispersion (KHR_materials_dispersion) has **no Principled socket in 5.2's list**. The exporter reads it from a custom property or a Glass/Dispersion pattern [UNVERIFIED; check `io_scene_gltf2/blender/exp/material/extensions/dispersion.py`].

Full socket tables for the other nodes (Image Texture, Normal Map with new `convention` OPENGL/DIRECTX, Mapping, Tex Coord, UV Map, Color Attribute, Attribute, Separate/Combine Color, Mix, Math, Emission, Transparent, Mix Shader, Output, Volume, Background, Environment, etc.) are in [Appendix C](#appendix-c-shader-node-socket-tables).

The glTF exporter recognises these node patterns (from add-on source, summarized; a separate research note should cover the details):
* Image Texture → (optional Mapping [for KHR_texture_transform] → UV Map/Tex Coord.UV) → Principled input.
* Metallic/Roughness: Image → Separate Color (RGB) → `.Blue` → Metallic, `.Green` → Roughness.
* Occlusion: a custom group node named `glTF Material Output` / `glTF Settings` with input `Occlusion`.
* Normal: Image (Non-Color) → Normal Map (TANGENT, `uv_map`) → Normal.
* Vertex color: Color Attribute → Mix (MULTIPLY) or direct → Base Color.
* Alpha clip: see §0.9.
* Emission: Emission Color + Strength, or a separate Emission shader via Add/Mix Shader.
* Unlit (KHR_materials_unlit): Background shader, or Mix Shader of Transparent and Emission/Background with Light Path `Is Camera Ray`.

---

## 7. Camera, Light, World

### 7.1 Camera (`bpy.types.Camera`)
Full table: [Appendix A › Camera](#tbl-camera). `type` ∈ `PERSP, ORTHO, PANO, CUSTOM` (glTF: PERSP/ORTHO only). `lens` 50 mm, `lens_unit` (`MILLIMETERS|FOV`), `angle` (derived FOV 0.6911 rad = 39.6°; writable, sets lens), `angle_x`, `angle_y`, `sensor_width` 36, `sensor_height` 24, `sensor_fit` (`AUTO|HORIZONTAL|VERTICAL`), `ortho_scale` 6.0, `clip_start` 0.1, `clip_end` 1000 (note the float32: 0.10000000149), `shift_x`, `shift_y`, `dof` (ptr CameraDOFSettings). The factory "Camera" object is at loc (7.3589, −6.9258, 4.9583), rot (1.1093, 0, 0.8149).

glTF exporter (from memory, [UNVERIFIED in source]): yfov is computed from sensor fit and render aspect. ORTHO gives `xmag = ortho_scale/2` scaled by aspect. `znear`/`zfar` come from clip.

### 7.2 Light (`Light` → `PointLight` / `SunLight` / `SpotLight` / `AreaLight`; change type via `light.type = 'SPOT'`, which re-types the struct)

Common props: `type`, `color` (Color 3), `energy` (W for point/spot/area, W/m² for sun; fresh 10.0), `use_temperature`, `temperature` (6500 K), `temperature_color` RO, `exposure` (0, ±32 stops), `normalize` (True; normalize by area), `specular_factor`, `diffuse_factor`, `transmission_factor`, `volume_factor`, `use_custom_distance` (False), `cutoff_distance` (40), `use_shadow` (True), `shadow_soft_size` (radius; fresh `lights.new` reads **0.0**, factory Light is 0.1 [check the operator default]), `node_tree`/`use_nodes` (also deprecated).
Per type: Point `use_soft_falloff` (True). Spot `spot_size` (0.7854 = 45°, range [1°,180°]), `spot_blend` (0.15), `use_square`, `use_soft_falloff`. Sun `angle` (0.00918 rad), `shadow_cascade_*`. Area `shape` (`SQUARE, RECTANGLE, DISK, ELLIPSE`), `size` 0.25, `size_y` 0.25, `spread` (π). EEVEE shadow fields: `shadow_buffer_clip_start`, `shadow_filter_radius`, `shadow_maximum_resolution`, `use_shadow_jitter`, `shadow_jitter_overblur`, `use_absolute_resolution`.

**glTF exporter conversion** (verified in `io_scene_gltf2/blender/exp/lights.py` and `light_spots.py`, add-on 5.2.41):

| Blender | KHR_lights_punctual | Formula (lighting_mode `SPEC`, default) |
|---|---|---|
| POINT energy W | point intensity (cd) | `W / (4π) × 683 × 2^exposure` (`× area` if `normalize=False`) |
| SPOT energy W | spot intensity (cd) | same as point (no cone normalization) |
| SUN strength W/m² | directional intensity (lux) | `W/m² × 683 × 2^exposure` |
| AREA | — | **not exported** (warning "Unsupported light source") |
| `color` | color | `color × temperature_color` (if `use_temperature`) × Emission node color (Cycles light nodes, ignored when engine is EEVEE) |
| `spot_size`, `spot_blend` | `outerConeAngle = spot_size/2`; `innerConeAngle = outer − outer·spot_blend` | |
| `cutoff_distance` | `range` | only if `use_custom_distance` |
| `export_import_convert_lighting_mode` | `SPEC` (×683) / `COMPAT` (no 683) / `RAW` (energy as-is, deprecated) | |

Light orientation: Blender lights point along local −Z, as in glTF. The exporter handles the Y-up axis conversion only, with no extra rotation node [from memory, UNVERIFIED].

### 7.3 World
`World{color (0.05,0.05,0.05) [factory "World": 0.050876], use_nodes (deprecated), node_tree, light_settings, mist_settings, probe_resolution, sun_threshold, sun_angle, use_sun_shadow, ...}`. Node tree: `Background{Color, Strength 1.0, Weight(disabled)}` → `World Output{Surface, Volume}`. An HDRI is `ShaderNodeTexEnvironment{image, projection: EQUIRECTANGULAR|MIRROR_BALL, interpolation}` → Background.Color. The glTF core spec has no world export. frap uses the world only for rendering (IBL).

---

## 8. Scene, render, display, color management

Tables: [Scene](#tbl-scene), [RenderSettings](#tbl-rendersettings), [ImageFormatSettings](#tbl-imageformatsettings), [View3DShading](#tbl-view3dshading), [ColorManagedViewSettings](#tbl-colormanagedviewsettings), [View3DCursor](#tbl-view3dcursor), [UnitSettings](#tbl-unitsettings).

Verified factory-startup values: `scene.name='Scene'`, `frame_start=1`, `frame_end=250`, `frame_current=1`, `frame_step=1`, `render.fps=24`, `fps_base=1.0`, `resolution_x=1920`, `resolution_y=1080`, `resolution_percentage=100`, `render.engine='BLENDER_EEVEE'`, `render.filepath='/tmp/'` (factory; RNA default `'//'`), `image_settings.file_format='PNG'`, `color_mode='RGBA'`, `color_depth='8'`, `compression=15`, `film_transparent=False`, `scene.camera=Camera`, `scene.world=World`, view layer `ViewLayer`, master collection plus child `Collection`. `bpy.data.scenes.new()` gives `world=None`, `camera=None`, engine EEVEE, AgX.

* `render.engine`: `'BLENDER_EEVEE' | 'BLENDER_WORKBENCH' | 'CYCLES'` (dynamic enum). frap should accept EEVEE and WORKBENCH and reject CYCLES with a clear "unsupported" error, or alias it to EEVEE with a warning. That is a product decision.
* `render.image_settings.file_format` ∈ `AVIF, JPEG, OPEN_EXR, PNG, WEBP, BMP, CINEON, DPX, IRIS, JPEG2000, HDR, TARGA, TARGA_RAW, TIFF, OPEN_EXR_MULTILAYER, FFMPEG`. `media_type` ∈ `IMAGE|MULTI_LAYER_IMAGE|VIDEO` is **new in 5.x**. `color_depth` ∈ `8,10,12,16,32` (strings). `quality` 90, `compression` 15.
* `render.filepath` with `//` means relative to the .blend (frap: relative to the .frap file, or to the cwd when unsaved). For animation, `#` characters are replaced by the zero-padded frame number, and the extension is appended if `use_file_extension`. `render.frame_path(frame=)` returns the resolved path.
* `scene.view_settings.view_transform` ∈ `Standard, ACES 1.3, ACES 2.0, Khronos PBR Neutral, AgX, Filmic, Filmic Log, False Color, Raw` (verified via error message). The default is **`AgX`**. `look` defaults to `'None'` (a string). `exposure` 0, `gamma` 1. `display_settings.display_device` is `'sRGB'`. frap should implement at least Standard, AgX and Khronos PBR Neutral, plus Raw.
* `scene.display.shading` (Workbench render settings): `type` (WIREFRAME/SOLID/MATERIAL/RENDERED), `light` (`STUDIO|MATCAP|FLAT`, default STUDIO), `studio_light` (dynamic enum, default `'Default'`; matcaps like `basic_1.exr` etc. [UNVERIFIED list in background]), `color_type` (`MATERIAL|OBJECT|RANDOM|VERTEX|TEXTURE|SINGLE`, default MATERIAL), `single_color` (0.8 gray), `show_xray`, `xray_alpha` 0.5, `show_shadows` False, `show_cavity` False, `cavity_type`, `show_object_outline`, `background_type` (`THEME|WORLD|VIEWPORT`), `wireframe_color_type`. `scene.display.light_direction` and `shadow_shift`.
* `scene.cursor`: `location`, `rotation_mode` (XYZ), `rotation_euler`, `rotation_quaternion`, `rotation_axis_angle`, `matrix`.
* `scene.unit_settings`: `system='METRIC'`, `scale_length=1.0`, `length_unit='METERS'`, `system_rotation='DEGREES'` (display only; RNA values are always radians), `mass_unit`, `time_unit`, `temperature_unit='KELVIN'`.
* `scene.eevee` (SceneEEVEE) includes `taa_render_samples`, `use_shadows`, `shadow_ray_count`, `shadow_step_count`, `use_raytracing`, `ray_tracing_method`, `use_fast_gi`, `volumetric_*`, `light_threshold`, `use_overscan` and more. frap can expose a subset (`taa_render_samples`, `use_shadows`) and ignore the rest with a warning.

---

## 9. Image

Full table: [Appendix A › Image](#tbl-image). `images.new(name, w, h)` gives `source='GENERATED'`, `generated_type='BLANK'`, `generated_color=(0,0,0,1)`, `colorspace_settings.name='sRGB'` (`is_data=True` gives `'Non-Color'`), `alpha_mode='STRAIGHT'` (`STRAIGHT|PREMUL|CHANNEL_PACKED|NONE`), `file_format='PNG'` (RNA default TARGA), `channels=4`, `size=(w,h)` RO, `packed_file=None`, `filepath=''`.
* `images.load(filepath, check_existing=False)` sets `source='FILE'`. `image.pack()` / `unpack(method)`. `packed_file{size, data}`. `filepath_raw` is the unexpanded path.
* `colorspace_settings.name` is a dynamic enum of OCIO names. The relevant ones are `'sRGB'`, `'Non-Color'`, `'Linear Rec.709'`, `'scene_linear'`, `'Linear Rec.2020'`, `'ACEScg'`, `'Filmic sRGB'`, etc. (65 names verified). There is also `colorspace_settings.is_data`. The glTF exporter treats any non-color-data image on non-color sockets as linear.
* glTF images: the exporter writes PNG/JPEG (or WEBP with EXT_texture_webp) according to `export_image_format` (`AUTO|JPEG|WEBP|NONE`, verified). frap's .frap container stores the original bytes (as the packed-file equivalent).
* `pixels` is a flat float RGBA array (len = w·h·channels). `Image.save(filepath?, quality?, save_copy?)` and `save_render(filepath, scene?, quality?)` exist.

---

## 10. Armature, Bone, EditBone, PoseBone

Tables: [Armature](#tbl-armature), [EditBone](#tbl-editbone), [Bone](#tbl-bone), [PoseBone](#tbl-posebone).

* Three views of the same bone. **EditBone** (`arm.data.edit_bones`) is writable only in EDIT mode; the collection is empty (len 0) otherwise, which was verified. **Bone** (`arm.data.bones`) is the rest pose and RO for geometry (`head`, `tail`, `head_local`, `tail_local`, `matrix` 3×3, `matrix_local` 4×4, `length` are all RO). **PoseBone** (`obj.pose.bones`) holds the animated local transforms.
* `bpy.ops.object.armature_add()` gives object "Armature" with data "Armature" and one bone "Bone" from (0,0,0) to (0,0,1), roll 0.
* `edit_bones.new(name)` gives head = tail = (0,0,0). **A zero-length bone is deleted on leaving edit mode**, so the tail must be set [known Blender behaviour, not re-verified]. Fresh EditBone values: `roll=0`, `use_deform=True`, `use_inherit_rotation=True`, `use_local_location=True`, `inherit_scale='FULL'`, `use_connect=False`.
* `use_connect=True` snaps `head` to `parent.tail` (verified). The EditBone fields are `head`, `tail`, `roll` (rad), `length`, `matrix` (4×4, writable), `parent`, `use_connect`, `use_deform`, `use_inherit_rotation`, `inherit_scale` (`FULL, FIX_SHEAR, ALIGNED, AVERAGE, NONE, NONE_LEGACY`), `use_local_location`, `use_relative_parent`, `select`, `select_head`, `select_tail`, `hide`, `lock`, `collections`, and `align_roll(vector)`.
* PoseBone: `location`, `rotation_mode` (**default `QUATERNION`**, unlike Object's XYZ), `rotation_quaternion`, `rotation_euler`, `rotation_axis_angle`, `scale`, `matrix_basis` (local), `matrix` (armature space, writable), `matrix_channel` RO, `head`/`tail`/`length` RO, `lock_*`, `constraints`, `custom_shape`, `select`, `hide`. Bone selection moved to PoseBone in 5.x: `Bone.select`, `select_head`, `select_tail` are **absent** (verified); `Bone.hide` still exists alongside `PoseBone.hide`.
* Bone collections (4.0+): `arm.data.collections` / `collections_all` (default "Bones"). Bone layers are gone.
* Pose keyframes: `pb.keyframe_insert("rotation_quaternion", frame=1)` creates F-Curves with `data_path='pose.bones["Bone.001"].rotation_quaternion'`, indices 0..3, **group = bone name** (verified), in the armature *object's* action.
* `Armature.pose_position` ∈ `POSE|REST`, `display_type` ∈ `OCTAHEDRAL, STICK, BBONE, ENVELOPE, WIRE`. `Armature.transform(matrix)`.
* glTF skin mapping: joints = all bones by default; only deform bones if the exporter option `export_def_bones=True` (default False, verified). inverseBindMatrices come from `bone.matrix_local` and the armature world matrix. Weights come from vertex groups whose names match bone names, with an Armature modifier present.

## 11. Vertex groups

* `obj.vertex_groups.new(name="Group")` → `VertexGroup{name (63 bytes), index RO, lock_weight}`. Default names are `Group`, `Group.001`, ….
* `vg.add(index: list[int], weight: float, type: 'REPLACE'|'ADD'|'SUBTRACT')`. Index must be a **list** (Python sequence). Verified: `add([0,1,2],0.5,'REPLACE')` then `add([0],0.25,'ADD')` gives weight(0)=0.75.
* `vg.remove(index: list[int])`, `vg.weight(index) -> float`. The latter raises `RuntimeError: Error: Vertex not in group` if the vertex is unassigned (verified).
* Per-vertex read: `mesh.vertices[i].groups` → `[VertexGroupElement{group: int, weight: float}]`.
* glTF: JOINTS_0/WEIGHTS_0 (4 influences per set; more sets if `export_all_influences`), normalized.

## 12. Shape keys

* `obj.shape_key_add(name="Key", from_mix=True) -> ShapeKey`. The first call creates the `Key` datablock (`mesh.shape_keys`, name `"Key"`, `use_relative=True`) and a reference key named as given (conventionally `"Basis"`).
* `ShapeKey{name (63 bytes), value (0..1 within slider_min/slider_max), slider_min 0, slider_max 1, relative_key (ptr ShapeKey, default Basis), vertex_group (str), mute, lock_shape, interpolation ('KEY_LINEAR' default; KEY_CARDINAL, KEY_BSPLINE, KEY_CATMULL_ROM), frame RO, data/points (ShapeKeyPoint{co})}`, plus functions `normals_vertex_get()`, `normals_polygon_get()`, `normals_split_get()`.
* `Key{key_blocks, reference_key RO, use_relative, eval_time, user RO, animation_data}`.
* `k.data[i].co = (x,y,z)` stores an absolute position (not a delta). The glTF morph target is `key.co - basis.co` (relative to `relative_key`? The exporter uses the reference key [UNVERIFIED for non-Basis relative keys]).
* Animation: `key_block.keyframe_insert("value")` gives `data_path='key_blocks["Key 1"].value'` on the **Key** ID. As shown in §0.4, it lands in the **object's action** under slot `KE<keyname>` (verified in 5.2). glTF morph weights animation needs all key blocks sampled together.

## 13. Animation: Actions (slotted), F-Curves, keyframes, NLA

### 13.1 Structure (verified 5.2)

```
Action (ID)
 ├─ slots[]: ActionSlot{identifier "OBCube" (RO? writable via name), name_display "Cube",
 │             target_id_type 'OBJECT'|'KEY'|'NODETREE'|..., handle int, active, select,
 │             users() -> [IDs], duplicate()}
 ├─ layers[]: ActionLayer{name "Layer", strips[]}
 │             └─ strips[]: ActionKeyframeStrip{type 'KEYFRAME', channelbags[],
 │                                              channelbag(slot, ensure=False), key_insert(...)}
 │                           └─ channelbags[]: ActionChannelbag{slot RO, slot_handle, fcurves[], groups[]}
 │                                              └─ fcurves[]: FCurve
 ├─ is_action_layered (True after first key), is_action_legacy, is_empty
 ├─ use_frame_range False, frame_start, frame_end, frame_range (computed (1,20)), curve_frame_range
 ├─ use_cyclic, pose_markers
 └─ fns: deselect_keys(), fcurve_ensure_for_datablock(datablock, data_path, index, group_name), flip_with_pose(object)
AnimData (on any animatable ID)
   action, action_slot, action_slot_handle, last_slot_identifier, action_suitable_slots RO,
   action_extrapolation, action_blend_type, action_influence, use_nla, use_tweak_mode, use_pin,
   nla_tracks, drivers
```

Verified behaviours:
* `obj.keyframe_insert("location", frame=1)` auto-creates Action `"<ObjectName>Action"` (`CubeAction`), slot `OBCube`, one layer `"Layer"`, one KEYFRAME strip, and a channelbag with F-Curves grouped as `"Object Transforms"`.
* Slot identifier prefix = 2-letter ID code: `OB` Object, `KE` Key, `NT` NodeTree, `MA` Material, `CA` Camera, `LA` Light, `AR` Armature, `WO` World, `ME` Mesh [the last six by convention, UNVERIFIED individually].
* Assigning an existing action to a second ID (`o2.animation_data.action = a`) leaves `action_slot=None` until a suitable slot is chosen or created. `keyframe_insert` on it adds slot `OBCamera`. `slot.users()` returns the IDs that use it.
* Manual construction: `a.slots.new(id_type='OBJECT', name="Thing")` → identifier `OBThing`. Then `a.layers.new("Layer")`, `layer.strips.new(type='KEYFRAME')`, `strip.channelbags.new(slot)`, `cb.fcurves.new("location", index=0)` and `fc.keyframe_points.insert(frame=1, value=3.0)`. A second `layers.new` **succeeds** in 5.2 (multi-layer is allowed in the API). [UNVERIFIED whether evaluation and the glTF export blend multiple layers; treat one layer as the supported case.]
* Helpers in `bpy_extras.anim_utils`: `action_get_channelbag_for_slot(action, slot)`, `action_ensure_channelbag_for_slot`, `animdata_get_channelbag_for_assigned_slot(anim_data)` and `action_get_first_suitable_slot`.

### 13.2 FCurve and Keyframe

`FCurve{data_path, array_index, group (ActionGroup; name), extrapolation ('CONSTANT'|'LINEAR'), mute, hide, lock, select, color_mode, color, auto_smoothing, is_valid, is_empty, keyframe_points, sampled_points, modifiers, driver}`, plus functions (`evaluate(frame)`, `update()`, `range()` — from memory).

Keyframe (verified fresh values from `keyframe_insert`): `co=(frame, value)`, `interpolation='BEZIER'`, `handle_left_type=handle_right_type='AUTO_CLAMPED'`, `easing='AUTO'`, `type='KEYFRAME'`, `back=1.70158`, `amplitude=0.8`, `period=4.1`, `handle_left`, `handle_right`, `co_ui`, `select_control_point`, `select_left_handle`, `select_right_handle`.
* `interpolation` ∈ `CONSTANT, LINEAR, BEZIER, SINE, QUAD, CUBIC, QUART, QUINT, EXPO, CIRC, BACK, BOUNCE, ELASTIC`.
* handle types ∈ `FREE, ALIGNED, VECTOR, AUTO, AUTO_CLAMPED`. easing ∈ `AUTO, EASE_IN, EASE_OUT, EASE_IN_OUT`. keyframe `type` ∈ `KEYFRAME, BREAKDOWN, MOVING_HOLD, EXTREME, JITTER, GENERATED`.
* `keyframe_points.add(n)` appends points at (0,0) with BEZIER.
* `keyframe_insert(..., options=)` flags: `INSERTKEY_NEEDED, INSERTKEY_VISUAL, INSERTKEY_REPLACE, INSERTKEY_AVAILABLE, INSERTKEY_CYCLE_AWARE`. `keyframe_points.insert(..., options=)` flags: `REPLACE, NEEDED, FAST`. `keyframe_delete(data_path, *, index=-1, frame=None, group="")`. `driver_add(path, index=-1, /)` (positional-only).
* Default user-preference key interpolation is BEZIER with AUTO_CLAMPED handles. The glTF exporter maps CONSTANT → STEP and LINEAR → LINEAR. BEZIER is exported as CUBICSPLINE only when all keyframes are bezier and "always sample" is off; otherwise it is baked/sampled to LINEAR (option `export_force_sampling`, default **True**, verified in the add-on source, with a sampling interpolation fallback enum `LINEAR|STEP`, default LINEAR).
* Bezier handle math (AUTO_CLAMPED recalculation) must be reimplemented exactly (`BKE_nurb_handle_calc` semantics) to match Blender's sampled values. This is a golden-test risk.

### 13.3 NLA

`AnimData.nla_tracks.new(prev=None)` gives `NlaTrack{name "NlaTrack", strips, active, is_solo, mute, lock, select}`. `track.strips.new(name, start:int, action)` gives `NlaStrip{name, type, action, action_slot, action_slot_handle, last_slot_identifier, action_suitable_slots, frame_start, frame_end, action_frame_start, action_frame_end, blend_type ('REPLACE'), extrapolation ('HOLD'), blend_in, blend_out, use_auto_blend, repeat 1.0, scale 1.0, influence, use_animated_influence, strip_time, use_reverse, use_sync_length, mute, select, active, fcurves, modifiers, strips}`. Verified: a strip of the 1–20 action starting at 1 gave frame_start 1, frame_end 20, slot auto-assigned `OBCube`.
glTF export (`export_animation_mode`: `ACTIONS|ACTIVE_ACTIONS|BROADCAST|NLA_TRACKS|SCENE`, verified in add-on 5.2.41): the default `ACTIONS` exports each action (and slot) as a glTF animation, using NLA tracks for discovery. `NLA_TRACKS` exports one animation per track name.

---

## 14. Collections and view layers

* `scene.collection` is the master "Scene Collection" (embedded, cannot be renamed or removed). It has `objects`, `children`, `all_objects`, `hide_*` and `color_tag`. `bpy.data.collections` holds the non-master ones. The factory scene has `Collection` containing Camera, Cube and Light.
* `Collection{name, objects (link/unlink), children (link/unlink), all_objects RO, hide_viewport, hide_render, hide_select, instance_offset, color_tag, exporters (5.x collection exporters), active_exporter_index}`.
* `obj.users_collection` (Python) lists the collections containing the object. `scene.objects` lists every object in the scene.
* `view_layer.layer_collection` (LayerCollection tree with `exclude`, `hide_viewport`, `holdout`, `indirect_only`). `view_layer.active_layer_collection` decides where new objects go: **the operators `primitive_*_add` link the new object into `view_layer.active_layer_collection.collection`**. That is `Collection` in the factory scene, not the master.
* glTF export: the scene hierarchy becomes nodes. Collections are not glTF nodes unless instanced (`instance_type='COLLECTION'` empties become nodes). The exporter option `use_active_collection` and others filter what is exported.

---

## 15. Recommendations for frap's RNA layer

1. **Generate the schema from Blender, don't hand-write it.** The introspection scripts used for this note (dumping `bl_rna.properties` with type/subtype/array length/enum items/min/max/default, plus *instance* values) can be committed as `tools/rna_dump.py`. Run them against `$BLENDER_BIN` to produce `schema/blender-5.2-rna.json`. Then generate Rust struct definitions or a static table (`build.rs` or a checked-in codegen) for the subset frap implements. The same JSON feeds `frap --describe`. A test then detects drift whenever Blender is bumped.
2. **Core abstraction:** `StructDef{identifier, base, props: [PropDef{id, kind: Bool|Int|Float|String|Enum{items, flag}|Pointer{fixed_type}|Collection{elem, funcs}, array: Option<[u8;2] dims>, subtype, min, max, soft_min/max, default, readonly, python_only}], funcs: [FuncDef{params, returns}]}`. Implementations provide `get(&self, prop) -> Value` / `set(&mut self, prop, Value) -> Result`. `Value` variants should mirror the JSON types in §1.3, including the `Vector`/`Euler(order)`/`Quaternion`/`Matrix`/`Color` tags.
3. **Error messages:** copy Blender's wording (`enum "X" not found in (...)`, `key "Nope" not found`, `attribute "type" from "Object" is read-only`, `sequences of dimension 0 should contain 3 items, not 2`). LLMs that know Blender can then self-correct, and golden tests can compare error class.
4. **Separate "original" and "evaluated" data**, as Blender does, for `dimensions`, `matrix_world`, modifier results and the glTF export. Evaluate lazily, but before every read of a derived property.
5. **Name handling:** implement a single `unique_name(collection, requested, maxbytes)` with UTF‑8 boundary truncation and the "lowest free .NNN" rule. Use 255 bytes for ID names and 63 bytes for sub-data. Implement `ID.rename(mode)` too.
6. **Python-defined extras** (`children`, `users_collection`, `Mesh.from_pydata/shade_smooth/shade_flat`, `bpy_extras.anim_utils` helpers) should be flagged `python_only: true` in the schema.
7. **Version pinning:** every `.frap` scene.json should record `blender_rna_version: "5.2"`. 5.x has already removed or renamed APIs (legacy action API, `BLENDER_EEVEE_NEXT`, GN modifier ID props, SeparateRGB), so expect churn in 5.3+.

---

## 16. Open questions / uncertainties

| # | Item | Status |
|---|---|---|
| U1 | `modifier_add` default name for NODES (`GeometryNodes`?) and MULTIRES | the others are verified (§5.1) |
| U2 | `Light.shadow_soft_size` reads 0.0 on `lights.new` and on `light_add`, **even with `light_add(radius=1.0)`**. The factory Light is 0.1 (stored in startup.blend); earlier versions defaulted to 0.25. | Verified value. It looks like a 5.2 quirk or a regression; mirror the observed values and re-test on 5.2.x updates |
| U3 | Remaining glTF exporter options | the key enums are verified above; the full list belongs in the exporter research note |
| U4 | Exporter handling of `Thin Wall`, `Diffuse Roughness`, dispersion | **[UNVERIFIED]** |
| U5 | Multi-layer Actions: API allows `layers.new` twice, but evaluation/export semantics unclear | **[UNVERIFIED]** |
| U6 | `studio_light` / matcap enum items in background mode (only `DEFAULT` returned with no context) | Dynamic enum; get the list from `bpy.context.preferences.studio_lights` |
| U7 | Float clamping vs. error on out-of-range sets for every property | standard RNA clamps; not exhaustively verified |
| U8 | Slot ID-code prefixes beyond OB/KE/NT | convention, not individually verified |
| U9 | Custom split normals attribute name in 5.2 (`custom_normal`) | **[UNVERIFIED]** |
| U10 | Whether `inputs['Name']` lookup preferring enabled sockets applies to all node types (verified only on ShaderNodeMix) | likely generic |

---

## Appendix A: core struct property tables (dumped from Blender 5.2.4)

Values are read from fresh instances: `bpy.data.objects.new("O", mesh)`, `materials.new`, `cameras.new`, `lights.new(type)`, `worlds.new`, the factory `Scene` (render/display/view settings), `images.new(64,64)`, `collections.new`, `armatures.new` plus an edit bone with tail (0,0,1). `4x4` means a matrix. UI-only (`open_*`, `show_*` of panels), Cycles, Line Art, Grease Pencil and light-linking properties are omitted.

<a id="tbl-object"></a>
#### `Object`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum **RO** | 'MESH' | MESH / CURVE / SURFACE / META / FONT / CURVES / POINTCLOUD / VOLUME / GREASEPENCIL / ARMATURE / LATTICE / EMPTY / LIGHT / LIGHT_PROBE / CAMERA / SPEAKER |
| `data` | ptr → ID | — |  |
| `parent` | ptr → Object | — |  |
| `parent_type` | enum | 'OBJECT' | OBJECT / ARMATURE / LATTICE / VERTEX / VERTEX_3 / BONE |
| `parent_bone` | str | '' | max 63 bytes |
| `parent_vertices` | int[3] | (0, 0, 0) |  |
| `matrix_parent_inverse` | float[4x4] (MATRIX) | matrix |  |
| `location` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `rotation_mode` | enum | 'XYZ' | QUATERNION / XYZ / XZY / YXZ / YZX / ZXY / ZYX / AXIS_ANGLE |
| `rotation_euler` | float[3] (EULER) | (0, 0, 0) |  |
| `rotation_quaternion` | float[4] (QUATERNION) | (1, 0, 0, 0) |  |
| `rotation_axis_angle` | float[4] (AXISANGLE) | (0, 0, 1, 0) |  |
| `scale` | float[3] (XYZ) | (1, 1, 1) |  |
| `delta_location` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `delta_rotation_euler` | float[3] (EULER) | (0, 0, 0) |  |
| `delta_rotation_quaternion` | float[4] (QUATERNION) | (1, 0, 0, 0) |  |
| `delta_scale` | float[3] (XYZ) | (1, 1, 1) |  |
| `dimensions` | float[3] (XYZ_LENGTH) | (0, 0, 0) |  |
| `matrix_world` | float[4x4] (MATRIX) | matrix |  |
| `matrix_local` | float[4x4] (MATRIX) | matrix |  |
| `matrix_basis` | float[4x4] (MATRIX) | matrix |  |
| `lock_location` | bool[3] | (False, False, False) |  |
| `lock_rotation` | bool[3] | (False, False, False) |  |
| `lock_rotation_w` | bool | False |  |
| `lock_rotations_4d` | bool | True |  |
| `lock_scale` | bool[3] | (False, False, False) |  |
| `hide_viewport` | bool | False |  |
| `hide_render` | bool | False |  |
| `hide_select` | bool | False |  |
| `color` | float[4] (COLOR) | (1, 1, 1, 1) |  |
| `pass_index` | int | 0 | [0, 32767] |
| `display_type` | enum | 'TEXTURED' | BOUNDS / WIRE / SOLID / TEXTURED |
| `show_wire` | bool | False |  |
| `show_all_edges` | bool | False |  |
| `show_in_front` | bool | False |  |
| `show_name` | bool | False |  |
| `show_axis` | bool | False |  |
| `show_bounds` | bool | False |  |
| `display_bounds_type` | enum | 'BOX' | BOX / SPHERE / CYLINDER / CONE / CAPSULE |
| `empty_display_type` | enum | 'PLAIN_AXES' | PLAIN_AXES / ARROWS / SINGLE_ARROW / CIRCLE / CUBE / SPHERE / CONE / IMAGE |
| `empty_display_size` | float (DISTANCE) | 1 | [0.0001, 1000] |
| `instance_type` | enum | 'NONE' | NONE / VERTS / FACES / COLLECTION |
| `instance_collection` | ptr → Collection | — |  |
| `material_slots` | coll → MaterialSlot **RO** | — |  |
| `active_material` | ptr → Material | — |  |
| `active_material_index` | int | 0 |  |
| `modifiers` | coll → Modifier **RO** | — |  |
| `constraints` | coll → Constraint **RO** | — |  |
| `vertex_groups` | coll → VertexGroup **RO** | — |  |
| `pose` | ptr → Pose **RO** | — |  |
| `active_shape_key` | ptr → ShapeKey **RO** | — |  |
| `active_shape_key_index` | int | 0 | [-32768, 32767] |
| `use_shape_key_edit_mode` | bool | False |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `mode` | enum **RO** | 'OBJECT' | OBJECT / EDIT / POSE / SCULPT / VERTEX_PAINT / WEIGHT_PAINT / TEXTURE_PAINT / PARTICLE_EDIT / EDIT_GPENCIL / SCULPT_GREASE_PENCIL / PAINT_GREASE_PENCIL / WEIGHT_GREASE_PENCIL / VERTEX_GREASE_PENCIL / SCULPT_CURVES |
| `track_axis` | enum | 'POS_Y' | POS_X / POS_Y / POS_Z / NEG_X / NEG_Y / NEG_Z |
| `up_axis` | enum | 'Z' | X / Y / Z |

<a id="tbl-mesh"></a>
#### `Mesh`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `vertices` | coll → MeshVertex **RO** | — |  |
| `edges` | coll → MeshEdge **RO** | — |  |
| `loops` | coll → MeshLoop **RO** | — |  |
| `polygons` | coll → MeshPolygon **RO** | — |  |
| `normals_domain` | enum **RO** | 'POINT' | POINT / FACE / CORNER |
| `vertex_normals` | coll → MeshNormalValue **RO** | — |  |
| `polygon_normals` | coll → MeshNormalValue **RO** | — |  |
| `corner_normals` | coll → MeshNormalValue **RO** | — |  |
| `loop_triangles` | coll → MeshLoopTriangle **RO** | — |  |
| `loop_triangle_polygons` | coll → ReadOnlyInteger **RO** | — |  |
| `uv_layers` | coll → MeshUVLoopLayer **RO** | — |  |
| `vertex_colors` | coll → MeshLoopColorLayer **RO** | — |  |
| `attributes` | coll → Attribute **RO** | — |  |
| `color_attributes` | coll → Attribute **RO** | — |  |
| `has_custom_normals` | bool **RO** | False |  |
| `shape_keys` | ptr → Key **RO** | — |  |
| `total_vert_sel` | int **RO** | 0 |  |
| `total_edge_sel` | int **RO** | 0 |  |
| `total_face_sel` | int **RO** | 0 |  |
| `is_editmode` | bool **RO** | False |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `materials` | coll → Material **RO** | — |  |

<a id="tbl-material"></a>
#### `Material`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `surface_render_method` | enum | 'DITHERED' | DITHERED / BLENDED |
| `displacement_method` | enum | 'BUMP' | BUMP / DISPLACEMENT / BOTH |
| `blend_method` | enum | 'HASHED' | OPAQUE / CLIP / HASHED / BLEND |
| `alpha_threshold` | float (FACTOR) | 0.5 | [0, 1] |
| `use_transparency_overlap` | bool | True |  |
| `show_transparent_back` | bool | True |  |
| `use_backface_culling` | bool | False |  |
| `use_backface_culling_shadow` | bool | False |  |
| `use_backface_culling_lightprobe_volume` | bool | True |  |
| `use_transparent_shadow` | bool | True |  |
| `use_raytrace_refraction` | bool | False |  |
| `use_screen_refraction` | bool | False |  |
| `use_sss_translucency` | bool | False |  |
| `refraction_depth` | float (DISTANCE) | 0 |  |
| `thickness_mode` | enum | 'SPHERE' | SPHERE / SLAB |
| `use_thickness_from_shadow` | bool | False |  |
| `volume_intersection_method` | enum | 'FAST' | FAST / ACCURATE |
| `max_vertex_displacement` | float (DISTANCE) | 0 |  |
| `pass_index` | int | 0 | [0, 32767] |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `diffuse_color` | float[4] (COLOR) | (0.8, 0.8, 0.8, 1) |  |
| `specular_color` | float[3] (COLOR) | (1, 1, 1) |  |
| `roughness` | float (FACTOR) | 0.4 | [0, 1] |
| `specular_intensity` | float (FACTOR) | 0.5 | [0, 1] |
| `metallic` | float (FACTOR) | 0 | [0, 1] |
| `is_grease_pencil` | bool **RO** | False |  |

<a id="tbl-camera"></a>
#### `Camera`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'PERSP' | PERSP / ORTHO / PANO / CUSTOM |
| `lens` | float (DISTANCE_CAMERA) | 50 |  |
| `lens_unit` | enum | 'MILLIMETERS' | MILLIMETERS / FOV |
| `angle` | float (ANGLE) | 0.691111 | [0.006405, 3.01675] |
| `angle_x` | float (ANGLE) | 0.691111 | [0.006405, 3.01675] |
| `angle_y` | float (ANGLE) | 0.47109 | [0.006405, 3.01675] |
| `sensor_width` | float (DISTANCE_CAMERA) | 36 |  |
| `sensor_height` | float (DISTANCE_CAMERA) | 24 |  |
| `sensor_fit` | enum | 'AUTO' | AUTO / HORIZONTAL / VERTICAL |
| `ortho_scale` | float | 6 |  |
| `clip_start` | float (DISTANCE) | 0.1 |  |
| `clip_end` | float (DISTANCE) | 1000 |  |
| `shift_x` | float | 0 |  |
| `shift_y` | float | 0 |  |
| `dof` | ptr → CameraDOFSettings **RO** | — |  |
| `animation_data` | ptr → AnimData **RO** | — |  |

<a id="tbl-light_point"></a>
#### `Light_POINT`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'POINT' | POINT / SUN / SPOT / AREA |
| `use_temperature` | bool | False |  |
| `color` | float[3] (COLOR) | (1, 1, 1) |  |
| `temperature` | float (COLOR_TEMPERATURE) | 6500 | [800, 20000] |
| `temperature_color` | float[3] (COLOR) **RO** | (1, 1, 1) |  |
| `specular_factor` | float (FACTOR) | 1 |  |
| `diffuse_factor` | float (FACTOR) | 1 |  |
| `transmission_factor` | float (FACTOR) | 1 |  |
| `volume_factor` | float (FACTOR) | 1 |  |
| `use_custom_distance` | bool | False |  |
| `cutoff_distance` | float (DISTANCE) | 40 |  |
| `use_shadow` | bool | True |  |
| `exposure` | float (FACTOR) | 0 | [-32, 32] |
| `normalize` | bool | True |  |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `use_soft_falloff` | bool | True |  |
| `energy` | float | 10 |  |
| `shadow_buffer_clip_start` | float (DISTANCE) | 0.05 |  |
| `shadow_soft_size` | float (DISTANCE) | 0 |  |
| `shadow_filter_radius` | float | 1 |  |
| `shadow_maximum_resolution` | float (DISTANCE) | 0.001 |  |
| `use_shadow_jitter` | bool | False |  |
| `shadow_jitter_overblur` | float (PERCENTAGE) | 10 | [0, 100] |
| `use_absolute_resolution` | bool | False |  |

<a id="tbl-light_sun"></a>
#### `Light_SUN`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'SUN' | POINT / SUN / SPOT / AREA |
| `use_temperature` | bool | False |  |
| `color` | float[3] (COLOR) | (1, 1, 1) |  |
| `temperature` | float (COLOR_TEMPERATURE) | 6500 | [800, 20000] |
| `temperature_color` | float[3] (COLOR) **RO** | (1, 1, 1) |  |
| `specular_factor` | float (FACTOR) | 1 |  |
| `diffuse_factor` | float (FACTOR) | 1 |  |
| `transmission_factor` | float (FACTOR) | 1 |  |
| `volume_factor` | float (FACTOR) | 1 |  |
| `use_custom_distance` | bool | False |  |
| `cutoff_distance` | float (DISTANCE) | 40 |  |
| `use_shadow` | bool | True |  |
| `exposure` | float (FACTOR) | 0 | [-32, 32] |
| `normalize` | bool | True |  |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `angle` | float (ANGLE) | 0.00918 | [0, 3.14159] |
| `energy` | float | 10 |  |
| `shadow_buffer_clip_start` | float (DISTANCE) | 0.05 |  |
| `shadow_soft_size` | float (DISTANCE) | 0 |  |
| `shadow_filter_radius` | float | 1 |  |
| `shadow_maximum_resolution` | float (DISTANCE) | 0.001 |  |
| `use_shadow_jitter` | bool | False |  |
| `shadow_jitter_overblur` | float (PERCENTAGE) | 10 | [0, 100] |
| `shadow_cascade_max_distance` | float (DISTANCE) | 200 |  |
| `shadow_cascade_count` | int | 4 | [1, 4] |
| `shadow_cascade_exponent` | float (FACTOR) | 0.8 | [0, 1] |
| `shadow_cascade_fade` | float (FACTOR) | 0.1 | [0, 1] |

<a id="tbl-light_spot"></a>
#### `Light_SPOT`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'SPOT' | POINT / SUN / SPOT / AREA |
| `use_temperature` | bool | False |  |
| `color` | float[3] (COLOR) | (1, 1, 1) |  |
| `temperature` | float (COLOR_TEMPERATURE) | 6500 | [800, 20000] |
| `temperature_color` | float[3] (COLOR) **RO** | (1, 1, 1) |  |
| `specular_factor` | float (FACTOR) | 1 |  |
| `diffuse_factor` | float (FACTOR) | 1 |  |
| `transmission_factor` | float (FACTOR) | 1 |  |
| `volume_factor` | float (FACTOR) | 1 |  |
| `use_custom_distance` | bool | False |  |
| `cutoff_distance` | float (DISTANCE) | 40 |  |
| `use_shadow` | bool | True |  |
| `exposure` | float (FACTOR) | 0 | [-32, 32] |
| `normalize` | bool | True |  |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `energy` | float | 10 |  |
| `shadow_buffer_clip_start` | float (DISTANCE) | 0.05 |  |
| `shadow_soft_size` | float (DISTANCE) | 0 |  |
| `shadow_filter_radius` | float | 1 |  |
| `shadow_maximum_resolution` | float (DISTANCE) | 0.001 |  |
| `use_shadow_jitter` | bool | False |  |
| `shadow_jitter_overblur` | float (PERCENTAGE) | 10 | [0, 100] |
| `use_absolute_resolution` | bool | False |  |
| `use_square` | bool | False |  |
| `spot_blend` | float | 0.15 | [0, 1] |
| `spot_size` | float (ANGLE) | 0.785398 | [0.017453, 3.14159] |
| `show_cone` | bool | False |  |
| `use_soft_falloff` | bool | True |  |

<a id="tbl-light_area"></a>
#### `Light_AREA`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'AREA' | POINT / SUN / SPOT / AREA |
| `use_temperature` | bool | False |  |
| `color` | float[3] (COLOR) | (1, 1, 1) |  |
| `temperature` | float (COLOR_TEMPERATURE) | 6500 | [800, 20000] |
| `temperature_color` | float[3] (COLOR) **RO** | (1, 1, 1) |  |
| `specular_factor` | float (FACTOR) | 1 |  |
| `diffuse_factor` | float (FACTOR) | 1 |  |
| `transmission_factor` | float (FACTOR) | 1 |  |
| `volume_factor` | float (FACTOR) | 1 |  |
| `use_custom_distance` | bool | False |  |
| `cutoff_distance` | float (DISTANCE) | 40 |  |
| `use_shadow` | bool | True |  |
| `exposure` | float (FACTOR) | 0 | [-32, 32] |
| `normalize` | bool | True |  |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `energy` | float | 10 |  |
| `shadow_buffer_clip_start` | float (DISTANCE) | 0.05 |  |
| `shadow_soft_size` | float (DISTANCE) | 0 |  |
| `shadow_filter_radius` | float | 1 |  |
| `shadow_maximum_resolution` | float (DISTANCE) | 0.001 |  |
| `use_shadow_jitter` | bool | False |  |
| `shadow_jitter_overblur` | float (PERCENTAGE) | 10 | [0, 100] |
| `use_absolute_resolution` | bool | False |  |
| `shape` | enum | 'SQUARE' | SQUARE / RECTANGLE / DISK / ELLIPSE |
| `size` | float (DISTANCE) | 0.25 |  |
| `size_y` | float (DISTANCE) | 0.25 |  |
| `spread` | float (ANGLE) | 3.14159 | [0, 3.14159] |

<a id="tbl-world"></a>
#### `World`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `animation_data` | ptr → AnimData **RO** | — |  |
| `use_eevee_finite_volume` | bool | False |  |
| `color` | float[3] (COLOR) | (0.05, 0.05, 0.05) |  |
| `light_settings` | ptr → WorldLighting **RO** | — |  |
| `mist_settings` | ptr → WorldMistSettings **RO** | — |  |
| `node_tree` | ptr → NodeTree **RO** | — |  |
| `use_nodes` | bool | True |  |
| `lightgroup` | str | '' |  |
| `probe_resolution` | enum | '1024' | 128 / 256 / 512 / 1024 / 2048 / 4096 |
| `sun_threshold` | float | 10 |  |
| `sun_angle` | float (ANGLE) | 0.00918 | [0, 3.14159] |
| `use_sun_shadow` | bool | True |  |
| `sun_shadow_maximum_resolution` | float (DISTANCE) | 0.001 |  |
| `sun_shadow_filter_radius` | float | 1 |  |
| `use_sun_shadow_jitter` | bool | False |  |
| `sun_shadow_jitter_overblur` | float (PERCENTAGE) | 10 | [0, 100] |

<a id="tbl-scene"></a>
#### `Scene`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `camera` | ptr → Object | — |  |
| `world` | ptr → World | — |  |
| `objects` | coll → Object **RO** | — |  |
| `collection` | ptr → Collection **RO** | — |  |
| `view_layers` | coll → ViewLayer **RO** | — |  |
| `frame_current` | int (TIME) | 1 | [-1048574, 1048574] |
| `frame_start` | int (TIME) | 1 | [0, 1048574] |
| `frame_end` | int (TIME) | 250 | [0, 1048574] |
| `frame_step` | int (TIME) | 1 | [0, 1048574] |
| `frame_subframe` | float (TIME) | 0 | [0, 1] |
| `frame_current_final` | float (TIME) **RO** | 1 | [-1.04857e+06, 1.04857e+06] |
| `render` | ptr → RenderSettings **RO** | — |  |
| `unit_settings` | ptr → UnitSettings **RO** | — |  |
| `cursor` | ptr → View3DCursor **RO** | — |  |
| `view_settings` | ptr → ColorManagedViewSettings **RO** | — |  |
| `display_settings` | ptr → ColorManagedDisplaySettings **RO** | — |  |
| `display` | ptr → SceneDisplay **RO** | — |  |
| `eevee` | ptr → SceneEEVEE **RO** | — |  |
| `animation_data` | ptr → AnimData **RO** | — |  |
| `use_gravity` | bool | True |  |
| `gravity` | float[3] (ACCELERATION) | (0, 0, -9.81) |  |

<a id="tbl-rendersettings"></a>
#### `RenderSettings`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `engine` | enum | 'BLENDER_EEVEE' | BLENDER_EEVEE |
| `resolution_x` | int (PIXEL) | 1920 | [4, 65536] |
| `resolution_y` | int (PIXEL) | 1080 | [4, 65536] |
| `resolution_percentage` | int (PERCENTAGE) | 100 | [1, 32767] |
| `pixel_aspect_x` | float | 1 | [1, 200] |
| `pixel_aspect_y` | float | 1 | [1, 200] |
| `fps` | int | 24 | [1, 32767] |
| `fps_base` | float | 1 | [1e-05, 1e+06] |
| `filepath` | str (FILE_PATH) | '/tmp/' | max 1023 bytes |
| `film_transparent` | bool | False |  |
| `filter_size` | float (PIXEL) | 1.5 | [0, 500] |
| `use_file_extension` | bool | True |  |
| `use_overwrite` | bool | True |  |
| `use_placeholder` | bool | False |  |
| `use_border` | bool | False |  |
| `use_crop_to_border` | bool | False |  |
| `border_min_x` | float | 0 | [0, 1] |
| `border_max_x` | float | 1 | [0, 1] |
| `border_min_y` | float | 0 | [0, 1] |
| `border_max_y` | float | 1 | [0, 1] |
| `image_settings` | ptr → ImageFormatSettings **RO** | — |  |
| `use_simplify` | bool | False |  |
| `simplify_subdivision` | int | 6 | [0, 32767] |
| `use_motion_blur` | bool | False |  |
| `use_freestyle` | bool | False |  |

<a id="tbl-imageformatsettings"></a>
#### `ImageFormatSettings`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `media_type` | enum | 'IMAGE' | IMAGE / MULTI_LAYER_IMAGE / VIDEO |
| `file_format` | enum | 'PNG' | AVIF / JPEG / OPEN_EXR / PNG / WEBP / BMP / CINEON / DPX / IRIS / JPEG2000 / HDR / TARGA / TARGA_RAW / TIFF / OPEN_EXR_MULTILAYER / FFMPEG |
| `color_mode` | enum | 'RGBA' | BW / RGB / RGBA |
| `color_depth` | enum | '8' | 8 / 10 / 12 / 16 / 32 |
| `quality` | int (PERCENTAGE) | 90 | [0, 100] |
| `compression` | int (PERCENTAGE) | 15 | [0, 100] |
| `exr_codec` | enum | 'ZIP' | NONE / ZIP / PIZ / DWAA / DWAB / HTJ2K / ZIPS / RLE / PXR24 / B44 / B44A |
| `color_management` | enum | 'FOLLOW_SCENE' | FOLLOW_SCENE / OVERRIDE |

<a id="tbl-view3dshading"></a>
#### `View3DShading`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `type` | enum | 'SOLID' | WIREFRAME / SOLID / MATERIAL / RENDERED |
| `light` | enum | 'STUDIO' | STUDIO / MATCAP / FLAT |
| `studio_light` | enum | 'Default' | DEFAULT |
| `color_type` | enum | 'MATERIAL' | MATERIAL / OBJECT / RANDOM / VERTEX / TEXTURE / SINGLE |
| `single_color` | float[3] (COLOR) | (0.8, 0.8, 0.8) | [0, 1] |
| `wireframe_color_type` | enum | '' | THEME / OBJECT / RANDOM |
| `background_type` | enum | 'THEME' | THEME / WORLD / VIEWPORT |
| `background_color` | float[3] (COLOR) | (0, 0, 0) | [0, 1] |
| `show_xray` | bool | False |  |
| `xray_alpha` | float (FACTOR) | 0.5 | [0, 1] |
| `show_xray_wireframe` | bool | False |  |
| `xray_alpha_wireframe` | float (FACTOR) | 0 | [0, 1] |
| `show_shadows` | bool | False |  |
| `shadow_intensity` | float (FACTOR) | 0.5 | [0, 1] |
| `show_cavity` | bool | False |  |
| `cavity_type` | enum | 'WORLD' | WORLD / SCREEN / BOTH |
| `cavity_ridge_factor` | float (FACTOR) | 1 | [0, 250] |
| `cavity_valley_factor` | float (FACTOR) | 1 | [0, 250] |
| `curvature_ridge_factor` | float (FACTOR) | 0 | [0, 2] |
| `curvature_valley_factor` | float (FACTOR) | 0 | [0, 2] |
| `show_object_outline` | bool | False |  |
| `object_outline_color` | float[3] (COLOR) | (0, 0, 0) | [0, 1] |
| `show_backface_culling` | bool | False |  |
| `show_specular_highlight` | bool | True |  |
| `use_dof` | bool | False |  |
| `use_world_space_lighting` | bool | False |  |
| `studiolight_rotate_z` | float (ANGLE) | 0 | [-3.14159, 3.14159] |
| `studiolight_intensity` | float (FACTOR) | 0 |  |
| `studiolight_background_alpha` | float (FACTOR) | 0 | [0, 1] |
| `studiolight_background_blur` | float (FACTOR) | 0 | [0, 1] |
| `use_scene_lights` | bool | False |  |
| `use_scene_world` | bool | False |  |
| `use_scene_lights_render` | bool | False |  |
| `use_scene_world_render` | bool | False |  |
| `render_pass` | enum | 'COMBINED' | COMBINED / EMISSION / ENVIRONMENT / AO / SHADOW / TRANSPARENT / DIFFUSE_LIGHT / DIFFUSE_COLOR / SPECULAR_LIGHT / SPECULAR_COLOR / VOLUME_LIGHT / POSITION / NORMAL / MIST / CryptoObject / CryptoAsset / CryptoMaterial / AOV |

<a id="tbl-scenedisplay"></a>
#### `SceneDisplay`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `light_direction` | float[3] (DIRECTION) | (0.57735, 0.57735, 0.57735) |  |
| `shadow_shift` | float (ANGLE) | 0.1 | [0, 1] |
| `shadow_focus` | float (FACTOR) | 0 | [0, 1] |
| `matcap_ssao_distance` | float (DISTANCE) | 0.2 | [0, 100000] |
| `matcap_ssao_attenuation` | float | 1 | [0, 100000] |
| `matcap_ssao_samples` | int | 16 | [1, 500] |
| `render_aa` | enum | '8' | OFF / FXAA / 5 / 8 / 11 / 16 / 32 |
| `viewport_aa` | enum | 'FXAA' | OFF / FXAA / 5 / 8 / 11 / 16 / 32 |
| `shading` | ptr → View3DShading **RO** | — |  |

<a id="tbl-colormanagedviewsettings"></a>
#### `ColorManagedViewSettings`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `view_transform` | enum | 'AgX' | NONE |
| `look` | enum | 'None' | NONE |
| `exposure` | float (FACTOR) | 0 | [-32, 32] |
| `gamma` | float (FACTOR) | 1 | [0, 5] |
| `use_curve_mapping` | bool | False |  |
| `use_white_balance` | bool | False |  |
| `white_balance_temperature` | float (COLOR_TEMPERATURE) | 6500 | [1800, 100000] |
| `white_balance_tint` | float (FACTOR) | 10 | [-500, 500] |

<a id="tbl-view3dcursor"></a>
#### `View3DCursor`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `location` | float[3] (XYZ_LENGTH) | (0, 0, 0) |  |
| `rotation_quaternion` | float[4] (QUATERNION) | (1, 0, 0, 0) |  |
| `rotation_axis_angle` | float[4] (AXISANGLE) | (0, 0, 1, 0) |  |
| `rotation_euler` | float[3] (EULER) | (0, 0, 0) |  |
| `rotation_mode` | enum | 'XYZ' | QUATERNION / XYZ / XZY / YXZ / YZX / ZXY / ZYX / AXIS_ANGLE |
| `matrix` | float[4x4] (MATRIX) | matrix |  |

<a id="tbl-unitsettings"></a>
#### `UnitSettings`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `system` | enum | 'METRIC' | NONE / METRIC / IMPERIAL |
| `system_rotation` | enum | 'DEGREES' | DEGREES / RADIANS |
| `scale_length` | float | 1 | [0, 1e+09] |
| `use_separate` | bool | False |  |
| `length_unit` | enum | 'METERS' | DEFAULT |
| `mass_unit` | enum | 'KILOGRAMS' | DEFAULT |
| `time_unit` | enum | 'SECONDS' | DEFAULT |
| `temperature_unit` | enum | 'KELVIN' | DEFAULT |

<a id="tbl-image"></a>
#### `Image`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `filepath` | str (FILE_PATH) | '' | max 1023 bytes |
| `filepath_raw` | str (FILE_PATH) | '' | max 1023 bytes |
| `source` | enum | 'GENERATED' | FILE / SEQUENCE / MOVIE / GENERATED / VIEWER / TILED |
| `type` | enum **RO** | 'UV_TEST' | IMAGE / MULTILAYER / UV_TEST / RENDER_RESULT / COMPOSITING |
| `file_format` | enum | 'PNG' | AVIF / JPEG / OPEN_EXR / PNG / WEBP / BMP / CINEON / DPX / IRIS / JPEG2000 / HDR / TARGA / TARGA_RAW / TIFF / OPEN_EXR_MULTILAYER / FFMPEG |
| `packed_file` | ptr → PackedFile **RO** | — |  |
| `packed_files` | coll → ImagePackedFile **RO** | — |  |
| `size` | int[2] (PIXEL) **RO** | (64, 64) |  |
| `resolution` | float[2] (XYZ) | (2834.65, 2834.65) |  |
| `channels` | int **RO** | 4 |  |
| `depth` | int **RO** | 24 |  |
| `has_data` | bool **RO** | True |  |
| `is_dirty` | bool **RO** | False |  |
| `is_float` | bool **RO** | False |  |
| `colorspace_settings` | ptr → ColorManagedInputColorspaceSettings **RO** | — |  |
| `alpha_mode` | enum | 'STRAIGHT' | STRAIGHT / PREMUL / CHANNEL_PACKED / NONE |
| `use_half_precision` | bool | True |  |
| `generated_type` | enum | 'BLANK' | BLANK / UV_GRID / COLOR_GRID |
| `generated_width` | int (PIXEL) | 64 | [1, 65536] |
| `generated_height` | int (PIXEL) | 64 | [1, 65536] |
| `generated_color` | float[4] (COLOR_GAMMA) | (0, 0, 0, 1) |  |
| `use_generated_float` | bool | False |  |
| `pixels` | float | (0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1) |  |
| `tiles` | coll → UDIMTile **RO** | — |  |

<a id="tbl-collection"></a>
#### `Collection`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `objects` | coll → Object **RO** | — |  |
| `all_objects` | coll → Object **RO** | — |  |
| `children` | coll → Collection **RO** | — |  |
| `hide_viewport` | bool | False |  |
| `hide_render` | bool | False |  |
| `hide_select` | bool | False |  |
| `instance_offset` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `color_tag` | enum | 'NONE' | NONE / COLOR_01 / COLOR_02 / COLOR_03 / COLOR_04 / COLOR_05 / COLOR_06 / COLOR_07 / COLOR_08 |
| `exporters` | coll → CollectionExport **RO** | — |  |

<a id="tbl-armature"></a>
#### `Armature`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `bones` | coll → Bone **RO** | — |  |
| `edit_bones` | coll → EditBone **RO** | — |  |
| `collections` | coll → BoneCollection | — |  |
| `collections_all` | coll → BoneCollection **RO** | — |  |
| `pose_position` | enum | 'POSE' | POSE / REST |
| `display_type` | enum | 'OCTAHEDRAL' | OCTAHEDRAL / STICK / BBONE / ENVELOPE / WIRE |
| `show_names` | bool | False |  |
| `show_axes` | bool | False |  |
| `show_bone_colors` | bool | True |  |
| `use_mirror_x` | bool | False |  |
| `animation_data` | ptr → AnimData **RO** | — |  |

<a id="tbl-editbone"></a>
#### `EditBone`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `name` | str | 'B' |  |
| `parent` | ptr → EditBone | — |  |
| `head` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `tail` | float[3] (TRANSLATION) | (0, 0, 1) |  |
| `roll` | float (ANGLE) | 0 |  |
| `length` | float (DISTANCE) | 1 |  |
| `matrix` | float[4x4] (MATRIX) | matrix |  |
| `use_connect` | bool | False |  |
| `use_deform` | bool | True |  |
| `use_inherit_rotation` | bool | True |  |
| `inherit_scale` | enum | 'FULL' | FULL / FIX_SHEAR / ALIGNED / AVERAGE / NONE / NONE_LEGACY |
| `use_local_location` | bool | True |  |
| `use_relative_parent` | bool | False |  |
| `select` | bool | False |  |
| `select_head` | bool | False |  |
| `select_tail` | bool | True |  |
| `hide` | bool | False |  |
| `lock` | bool | False |  |
| `collections` | coll → BoneCollection **RO** | — |  |

<a id="tbl-bone"></a>
#### `Bone`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `name` | str | 'B' | max 63 bytes |
| `parent` | ptr → Bone **RO** | — |  |
| `children` | coll → Bone **RO** | — |  |
| `head` | float[3] (TRANSLATION) **RO** | (0, 0, 0) |  |
| `tail` | float[3] (TRANSLATION) **RO** | (0, 0, 1) |  |
| `head_local` | float[3] (TRANSLATION) **RO** | (0, 0, 0) |  |
| `tail_local` | float[3] (TRANSLATION) **RO** | (0, 0, 1) |  |
| `length` | float (DISTANCE) **RO** | 1 |  |
| `matrix` | float[3x3] (MATRIX) **RO** | matrix |  |
| `matrix_local` | float[4x4] (MATRIX) **RO** | matrix |  |
| `use_connect` | bool **RO** | False |  |
| `use_deform` | bool | True |  |
| `use_inherit_rotation` | bool | True |  |
| `inherit_scale` | enum | 'FULL' | FULL / FIX_SHEAR / ALIGNED / AVERAGE / NONE / NONE_LEGACY |
| `use_local_location` | bool | True |  |
| `hide` | bool | False |  |
| `collections` | coll → BoneCollection **RO** | — |  |

<a id="tbl-posebone"></a>
#### `PoseBone`

| Property | Type | Default (fresh datablock) | Range / enum items |
|---|---|---|---|
| `name` | str | 'B' | max 63 bytes |
| `bone` | ptr → Bone **RO** | — |  |
| `parent` | ptr → PoseBone **RO** | — |  |
| `child` | ptr → PoseBone **RO** | — |  |
| `location` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `rotation_mode` | enum | 'QUATERNION' | QUATERNION / XYZ / XZY / YXZ / YZX / ZXY / ZYX / AXIS_ANGLE |
| `rotation_quaternion` | float[4] (QUATERNION) | (1, 0, 0, 0) |  |
| `rotation_euler` | float[3] (EULER) | (0, 0, 0) |  |
| `rotation_axis_angle` | float[4] (AXISANGLE) | (0, 0, 1, 0) |  |
| `scale` | float[3] (XYZ) | (1, 1, 1) |  |
| `matrix_basis` | float[4x4] (MATRIX) | matrix |  |
| `matrix` | float[4x4] (MATRIX) | matrix |  |
| `matrix_channel` | float[4x4] (MATRIX) **RO** | matrix |  |
| `head` | float[3] (TRANSLATION) **RO** | (0, 0, 0) |  |
| `tail` | float[3] (TRANSLATION) **RO** | (0, 0, 1) |  |
| `length` | float (DISTANCE) **RO** | 1 |  |
| `lock_location` | bool[3] | (False, False, False) |  |
| `lock_rotation` | bool[3] | (False, False, False) |  |
| `lock_rotation_w` | bool | False |  |
| `lock_rotations_4d` | bool | True |  |
| `lock_scale` | bool[3] | (False, False, False) |  |
| `constraints` | coll → Constraint **RO** | — |  |
| `custom_shape` | ptr → Object | — |  |
| `select` | bool | False |  |
| `hide` | bool | False |  |

---

## Appendix B: modifier property tables

The values are read from `obj.modifiers.new("M", T)` on the default cube in Blender 5.2.4. Tier 1 comes first: the user-listed modifiers and the glTF-relevant deformers. Tier 2 follows. UI panel toggles (`open_*`) are omitted. Base `Modifier` properties are listed in §5.1.

#### `SUBSURF` — `bpy.types.SubsurfModifier` (enum label "Subdivision Surface")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `uv_smooth` | enum | 'PRESERVE_BOUNDARIES' | NONE / PRESERVE_CORNERS / PRESERVE_CORNERS_AND_JUNCTIONS / PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE / PRESERVE_BOUNDARIES / SMOOTH_ALL |
| `quality` | int | 3 | [1, 10] |
| `boundary_smooth` | enum | 'ALL' | PRESERVE_CORNERS / ALL |
| `subdivision_type` | enum | 'CATMULL_CLARK' | CATMULL_CLARK / SIMPLE |
| `levels` | int | 1 | [0, 11] |
| `render_levels` | int | 2 | [0, 11] |
| `use_creases` | bool | True |  |
| `use_custom_normals` | bool | False |  |
| `use_limit_surface` | bool | True |  |
| `use_adaptive_subdivision` | bool | False |  |
| `adaptive_space` | enum | 'PIXEL' | PIXEL / OBJECT |
| `adaptive_pixel_size` | float (PIXEL) | 1 | [0.1, 1000] |
| `adaptive_object_edge_length` | float (DISTANCE) | 0.01 | [0.0001, 1000] |

#### `MIRROR` — `bpy.types.MirrorModifier` (enum label "Mirror")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `use_axis` | bool[3] | (True, False, False) |  |
| `use_bisect_axis` | bool[3] | (False, False, False) |  |
| `use_bisect_flip_axis` | bool[3] | (False, False, False) |  |
| `use_clip` | bool | False |  |
| `use_mirror_vertex_groups` | bool | True |  |
| `use_mirror_merge` | bool | True |  |
| `use_mirror_u` | bool | False |  |
| `use_mirror_v` | bool | False |  |
| `use_mirror_udim` | bool | False |  |
| `mirror_offset_u` | float (FACTOR) | 0 | [-1, 1] |
| `mirror_offset_v` | float (FACTOR) | 0 | [-1, 1] |
| `offset_u` | float (FACTOR) | 0 | [-10000, 10000] |
| `offset_v` | float (FACTOR) | 0 | [-10000, 10000] |
| `merge_threshold` | float (DISTANCE) | 0.001 | ≥ 0 |
| `bisect_threshold` | float (DISTANCE) | 0.001 | ≥ 0 |
| `mirror_object` | ptr → Object | None |  |

#### `ARRAY` — `bpy.types.ArrayModifier` (enum label "Array")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `fit_type` | enum | 'FIXED_COUNT' | FIXED_COUNT / FIT_LENGTH / FIT_CURVE |
| `count` | int | 2 |  |
| `fit_length` | float (DISTANCE) | 0 |  |
| `curve` | ptr → Object | None |  |
| `use_constant_offset` | bool | False |  |
| `constant_offset_displace` | float[3] (TRANSLATION) | (1, 0, 0) |  |
| `use_relative_offset` | bool | True |  |
| `relative_offset_displace` | float[3] (XYZ) | (1, 0, 0) |  |
| `use_merge_vertices` | bool | False |  |
| `use_merge_vertices_cap` | bool | False |  |
| `merge_threshold` | float (DISTANCE) | 0.01 | ≥ 0 |
| `use_object_offset` | bool | False |  |
| `offset_object` | ptr → Object | None |  |
| `start_cap` | ptr → Object | None |  |
| `end_cap` | ptr → Object | None |  |
| `offset_u` | float (FACTOR) | 0 | [-1, 1] |
| `offset_v` | float (FACTOR) | 0 | [-1, 1] |

#### `BEVEL` — `bpy.types.BevelModifier` (enum label "Bevel")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `width` | float (DISTANCE) | 0.1 | ≥ 0 |
| `width_pct` | float (PERCENTAGE) | 0.1 | ≥ 0 |
| `segments` | int | 1 | [1, 1000] |
| `affect` | enum | 'EDGES' | VERTICES / EDGES |
| `limit_method` | enum | 'ANGLE' | NONE / ANGLE / WEIGHT / VGROUP |
| `edge_weight` | str | 'bevel_weight_edge' |  |
| `vertex_weight` | str | 'bevel_weight_vert' |  |
| `angle_limit` | float (ANGLE) | 0.523599 | [0, 3.14159] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `use_clamp_overlap` | bool | True |  |
| `offset_type` | enum | 'OFFSET' | OFFSET / WIDTH / DEPTH / PERCENT / ABSOLUTE |
| `profile_type` | enum | 'SUPERELLIPSE' | SUPERELLIPSE / CUSTOM |
| `profile` | float (FACTOR) | 0.5 | [0, 1] |
| `material` | int | -1 | [-1, 32767] |
| `loop_slide` | bool | True |  |
| `mark_seam` | bool | False |  |
| `mark_sharp` | bool | False |  |
| `harden_normals` | bool | False |  |
| `face_strength_mode` | enum | 'FSTR_NONE' | FSTR_NONE / FSTR_NEW / FSTR_AFFECTED / FSTR_ALL |
| `miter_outer` | enum | 'MITER_SHARP' | MITER_SHARP / MITER_PATCH / MITER_ARC |
| `miter_inner` | enum | 'MITER_SHARP' | MITER_SHARP / MITER_ARC |
| `spread` | float (DISTANCE) | 0.1 | ≥ 0 |
| `custom_profile` | ptr → CurveProfile RO | None |  |
| `vmesh_method` | enum | 'ADJ' | ADJ / CUTOFF |

#### `SOLIDIFY` — `bpy.types.SolidifyModifier` (enum label "Solidify")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `solidify_mode` | enum | 'EXTRUDE' | EXTRUDE / NON_MANIFOLD |
| `thickness` | float (DISTANCE) | 0.01 |  |
| `thickness_clamp` | float (FACTOR) | 0 | [0, 100] |
| `use_thickness_angle_clamp` | bool | False |  |
| `thickness_vertex_group` | float (FACTOR) | 0 | [0, 1] |
| `offset` | float (FACTOR) | -1 |  |
| `edge_crease_inner` | float (FACTOR) | 0 | [0, 1] |
| `edge_crease_outer` | float (FACTOR) | 0 | [0, 1] |
| `edge_crease_rim` | float (FACTOR) | 0 | [0, 1] |
| `material_offset` | int | 0 | [-32768, 32767] |
| `material_offset_rim` | int | 0 | [-32768, 32767] |
| `vertex_group` | str | '' |  |
| `shell_vertex_group` | str | '' |  |
| `rim_vertex_group` | str | '' |  |
| `use_rim` | bool | True |  |
| `use_even_offset` | bool | False |  |
| `use_quality_normals` | bool | False |  |
| `invert_vertex_group` | bool | False |  |
| `use_flat_faces` | bool | False |  |
| `use_flip_normals` | bool | False |  |
| `use_rim_only` | bool | False |  |
| `nonmanifold_thickness_mode` | enum | 'CONSTRAINTS' | FIXED / EVEN / CONSTRAINTS |
| `nonmanifold_boundary_mode` | enum | 'NONE' | NONE / ROUND / FLAT |
| `nonmanifold_merge_threshold` | float (DISTANCE) | 0.0001 | [0, 1] |
| `bevel_convex` | float | 0 | [-1, 1] |

#### `BOOLEAN` — `bpy.types.BooleanModifier` (enum label "Boolean")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `collection` | ptr → Collection | None |  |
| `operation` | enum | 'DIFFERENCE' | INTERSECT / UNION / DIFFERENCE |
| `operand_type` | enum | 'OBJECT' | OBJECT / COLLECTION |
| `double_threshold` | float (DISTANCE) | 1e-06 | [0, 1] |
| `solver` | enum | 'EXACT' | FLOAT / EXACT / MANIFOLD |
| `use_self` | bool | False |  |
| `use_hole_tolerant` | bool | False |  |
| `material_mode` | enum | 'INDEX' | INDEX / TRANSFER |
| `debug_options` | enum-flag (set) | {} | SEPARATE / NO_DISSOLVE / NO_CONNECT_REGIONS |

#### `TRIANGULATE` — `bpy.types.TriangulateModifier` (enum label "Triangulate")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `quad_method` | enum | 'SHORTEST_DIAGONAL' | BEAUTY / FIXED / FIXED_ALTERNATE / SHORTEST_DIAGONAL / LONGEST_DIAGONAL |
| `ngon_method` | enum | 'BEAUTY' | BEAUTY / CLIP |
| `min_vertices` | int | 4 |  |
| `keep_custom_normals` | bool | False |  |

#### `DECIMATE` — `bpy.types.DecimateModifier` (enum label "Decimate")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `decimate_type` | enum | 'COLLAPSE' | COLLAPSE / UNSUBDIV / DISSOLVE |
| `ratio` | float (FACTOR) | 1 | [0, 1] |
| `iterations` | int | 0 | [0, 32767] |
| `angle_limit` | float (ANGLE) | 0.087266 | [0, 3.14159] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `use_collapse_triangulate` | bool | False |  |
| `use_symmetry` | bool | False |  |
| `symmetry_axis` | enum | 'X' | X / Y / Z |
| `vertex_group_factor` | float (FACTOR) | 1 | [0, 1000] |
| `use_dissolve_boundaries` | bool | False |  |
| `delimit` | enum-flag (set) | {} | NORMAL / MATERIAL / SEAM / SHARP / UV |
| `face_count` | int RO | 0 |  |

#### `WELD` — `bpy.types.WeldModifier` (enum label "Weld")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `mode` | enum | 'ALL' | ALL / CONNECTED |
| `merge_threshold` | float (DISTANCE) | 0.001 | ≥ 0 |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `loose_edges` | bool | False |  |

#### `DISPLACE` — `bpy.types.DisplaceModifier` (enum label "Displace")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `vertex_group` | str | '' |  |
| `mid_level` | float (FACTOR) | 0.5 |  |
| `strength` | float | 1 |  |
| `direction` | enum | 'NORMAL' | X / Y / Z / NORMAL / CUSTOM_NORMAL / RGB_TO_XYZ |
| `space` | enum | 'LOCAL' | LOCAL / GLOBAL |
| `invert_vertex_group` | bool | False |  |
| `texture` | ptr → Texture | None |  |
| `texture_coords` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `uv_layer` | str | '' |  |
| `texture_coords_object` | ptr → Object | None |  |
| `texture_coords_bone` | str | '' |  |

#### `SCREW` — `bpy.types.ScrewModifier` (enum label "Screw")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `steps` | int | 16 | [1, 10000] |
| `render_steps` | int | 16 | [1, 10000] |
| `iterations` | int | 1 | [1, 10000] |
| `axis` | enum | 'Z' | X / Y / Z |
| `angle` | float (ANGLE) | 6.28318 |  |
| `screw_offset` | float (DISTANCE) | 0 |  |
| `merge_threshold` | float (DISTANCE) | 0.01 | ≥ 0 |
| `use_normal_flip` | bool | False |  |
| `use_normal_calculate` | bool | False |  |
| `use_object_screw_offset` | bool | False |  |
| `use_merge_vertices` | bool | False |  |
| `use_smooth_shade` | bool | True |  |
| `use_stretch_u` | bool | False |  |
| `use_stretch_v` | bool | False |  |

#### `SKIN` — `bpy.types.SkinModifier` (enum label "Skin")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `branch_smoothing` | float (FACTOR) | 0 | [0, 1] |
| `use_smooth_shade` | bool | False |  |
| `use_x_symmetry` | bool | True |  |
| `use_y_symmetry` | bool | False |  |
| `use_z_symmetry` | bool | False |  |

#### `WIREFRAME` — `bpy.types.WireframeModifier` (enum label "Wireframe")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `thickness` | float (DISTANCE) | 0.02 |  |
| `thickness_vertex_group` | float (FACTOR) | 0 | [0, 1] |
| `offset` | float (FACTOR) | 0 |  |
| `use_replace` | bool | True |  |
| `use_boundary` | bool | False |  |
| `use_even_offset` | bool | True |  |
| `use_relative_offset` | bool | False |  |
| `use_crease` | bool | False |  |
| `crease_weight` | float | 1 |  |
| `material_offset` | int | 0 | [-32768, 32767] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |

#### `SMOOTH` — `bpy.types.SmoothModifier` (enum label "Smooth")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `use_x` | bool | True |  |
| `use_y` | bool | True |  |
| `use_z` | bool | True |  |
| `factor` | float (FACTOR) | 0.5 |  |
| `iterations` | int | 1 | [0, 32767] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |

#### `CAST` — `bpy.types.CastModifier` (enum label "Cast")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `cast_type` | enum | 'SPHERE' | SPHERE / CYLINDER / CUBOID |
| `object` | ptr → Object | None |  |
| `invert_vertex_group` | bool | False |  |
| `use_x` | bool | True |  |
| `use_y` | bool | True |  |
| `use_z` | bool | True |  |
| `use_radius_as_size` | bool | True |  |
| `use_transform` | bool | False |  |
| `factor` | float (FACTOR) | 0.5 |  |
| `radius` | float (DISTANCE) | 0 | ≥ 0 |
| `size` | float | 0 | ≥ 0 |
| `vertex_group` | str | '' |  |

#### `CURVE` — `bpy.types.CurveModifier` (enum label "Curve")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `deform_axis` | enum | 'POS_X' | POS_X / POS_Y / POS_Z / NEG_X / NEG_Y / NEG_Z |

#### `SIMPLE_DEFORM` — `bpy.types.SimpleDeformModifier` (enum label "Simple Deform")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `deform_method` | enum | 'TWIST' | TWIST / BEND / TAPER / STRETCH |
| `vertex_group` | str | '' |  |
| `deform_axis` | enum | 'X' | X / Y / Z |
| `origin` | ptr → Object | None |  |
| `factor` | float | 0.785398 |  |
| `angle` | float (ANGLE) | 0.785398 |  |
| `limits` | float[2] | (0, 1) | [0, 1] |
| `lock_x` | bool | False |  |
| `lock_y` | bool | False |  |
| `lock_z` | bool | False |  |
| `invert_vertex_group` | bool | False |  |

#### `LATTICE` — `bpy.types.LatticeModifier` (enum label "Lattice")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `strength` | float | 1 |  |

#### `ARMATURE` — `bpy.types.ArmatureModifier` (enum label "Armature")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `use_bone_envelopes` | bool | False |  |
| `use_vertex_groups` | bool | True |  |
| `use_deform_preserve_volume` | bool | False |  |
| `use_multi_modifier` | bool | False |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |

#### `EDGE_SPLIT` — `bpy.types.EdgeSplitModifier` (enum label "Edge Split")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `split_angle` | float (ANGLE) | 0.523599 | [0, 3.14159] |
| `use_edge_angle` | bool | True |  |
| `use_edge_sharp` | bool | True |  |

#### `WEIGHTED_NORMAL` — `bpy.types.WeightedNormalModifier` (enum label "Weighted Normal")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `weight` | int | 50 | [1, 100] |
| `mode` | enum | 'FACE_AREA' | FACE_AREA / CORNER_ANGLE / FACE_AREA_WITH_ANGLE |
| `thresh` | float | 0.01 | [0, 10] |
| `keep_sharp` | bool | False |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `use_face_influence` | bool | False |  |

#### `REMESH` — `bpy.types.RemeshModifier` (enum label "Remesh")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `mode` | enum | 'VOXEL' | BLOCKS / SMOOTH / SHARP / VOXEL |
| `scale` | float | 0.9 | [0, 0.99] |
| `threshold` | float | 1 | [0, 1] |
| `octree_depth` | int | 4 | [1, 24] |
| `sharpness` | float | 1 |  |
| `voxel_size` | float (DISTANCE) | 0.1 | ≥ 0 |
| `adaptivity` | float (DISTANCE) | 0 |  |
| `use_remove_disconnected` | bool | True |  |
| `use_smooth_shade` | bool | False |  |

#### `SHRINKWRAP` — `bpy.types.ShrinkwrapModifier` (enum label "Shrinkwrap")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `wrap_method` | enum | 'NEAREST_SURFACEPOINT' | NEAREST_SURFACEPOINT / PROJECT / NEAREST_VERTEX / TARGET_PROJECT |
| `wrap_mode` | enum | 'ON_SURFACE' | ON_SURFACE / INSIDE / OUTSIDE / OUTSIDE_SURFACE / ABOVE_SURFACE |
| `cull_face` | enum | 'OFF' | OFF / FRONT / BACK |
| `target` | ptr → Object | None |  |
| `auxiliary_target` | ptr → Object | None |  |
| `vertex_group` | str | '' |  |
| `offset` | float (DISTANCE) | 0 |  |
| `project_limit` | float (DISTANCE) | 0 | ≥ 0 |
| `use_project_x` | bool | False |  |
| `use_project_y` | bool | False |  |
| `use_project_z` | bool | False |  |
| `subsurf_levels` | int | 0 | [0, 6] |
| `use_negative_direction` | bool | False |  |
| `use_positive_direction` | bool | True |  |
| `use_invert_cull` | bool | False |  |
| `invert_vertex_group` | bool | False |  |

#### `NODES` — `bpy.types.NodesModifier` (enum label "Geometry Nodes")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `node_group` | ptr → NodeTree | None |  |
| `bake_directory` | str (DIR_PATH) | '' |  |
| `bake_target` | enum | 'PACKED' | PACKED / DISK |
| `bakes` | coll → NodesModifierBake RO | — |  |
| `panels` | coll → NodesModifierPanel RO | — |  |
| `node_warnings` | coll → NodesModifierWarning RO | — |  |
| `properties` | ptr → NodesModifierProperties RO | None |  |

#### `NORMAL_EDIT` — `bpy.types.NormalEditModifier` (enum label "Normal Edit")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `mode` | enum | 'RADIAL' | RADIAL / DIRECTIONAL |
| `offset` | float[3] (COORDINATES) | (0, 0, 0) |  |
| `mix_mode` | enum | 'COPY' | COPY / ADD / SUB / MUL |
| `mix_factor` | float (FACTOR) | 1 | [0, 1] |
| `mix_limit` | float (ANGLE) | 3.14159 | [0, 3.14159] |
| `no_polynors_fix` | bool | False |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `target` | ptr → Object | None |  |
| `use_direction_parallel` | bool | False |  |

#### `UV_PROJECT` — `bpy.types.UVProjectModifier` (enum label "UV Project")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `uv_layer` | str | '' |  |
| `projector_count` | int | 1 | [1, 10] |
| `projectors` | coll → UVProjector RO | — |  |
| `aspect_x` | float | 1 |  |
| `aspect_y` | float | 1 |  |
| `scale_x` | float | 1 | ≥ 0 |
| `scale_y` | float | 1 | ≥ 0 |

#### `UV_WARP` — `bpy.types.UVWarpModifier` (enum label "UV Warp")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `axis_u` | enum | 'X' | X / Y / Z |
| `axis_v` | enum | 'Y' | X / Y / Z |
| `center` | float[2] | (0.5, 0.5) |  |
| `offset` | float[2] | (0, 0) |  |
| `scale` | float[2] | (1, 1) |  |
| `rotation` | float (ANGLE) | 0 |  |
| `object_from` | ptr → Object | None |  |
| `bone_from` | str | '' |  |
| `object_to` | ptr → Object | None |  |
| `bone_to` | str | '' |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `uv_layer` | str | '' |  |

#### `MASK` — `bpy.types.MaskModifier` (enum label "Mask")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `mode` | enum | 'VERTEX_GROUP' | VERTEX_GROUP / ARMATURE |
| `armature` | ptr → Object | None |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `use_smooth` | bool | False |  |
| `threshold` | float (FACTOR) | 0 | [0, 1] |

#### `BUILD` — `bpy.types.BuildModifier` (enum label "Build")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `frame_start` | float (TIME) | 1 | [-1.04857e+06, 1.04857e+06] |
| `frame_duration` | float (TIME) | 100 | [1, 1.04857e+06] |
| `use_reverse` | bool | False |  |
| `use_random_order` | bool | False |  |
| `seed` | int | 0 | [1, 1048574] |

#### `MULTIRES` — `bpy.types.MultiresModifier` (enum label "Multiresolution")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `uv_smooth` | enum | 'PRESERVE_BOUNDARIES' | NONE / PRESERVE_CORNERS / PRESERVE_CORNERS_AND_JUNCTIONS / PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE / PRESERVE_BOUNDARIES / SMOOTH_ALL |
| `quality` | int | 4 | [1, 10] |
| `boundary_smooth` | enum | 'ALL' | PRESERVE_CORNERS / ALL |
| `levels` | int | 0 | [0, 255] |
| `sculpt_levels` | int | 0 | [0, 255] |
| `render_levels` | int | 0 | [0, 255] |
| `total_levels` | int RO | 0 | [0, 255] |
| `is_external` | bool RO | False |  |
| `filepath` | str (FILE_PATH) | '' |  |
| `use_creases` | bool | True |  |
| `use_custom_normals` | bool | False |  |
| `use_sculpt_base_mesh` | bool | False |  |

#### `LAPLACIANSMOOTH` — `bpy.types.LaplacianSmoothModifier` (enum label "Smooth Laplacian")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `use_x` | bool | True |  |
| `use_y` | bool | True |  |
| `use_z` | bool | True |  |
| `use_volume_preserve` | bool | True |  |
| `use_normalized` | bool | True |  |
| `lambda_factor` | float | 0.01 |  |
| `lambda_border` | float | 0.01 |  |
| `iterations` | int | 1 | [0, 32767] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |

#### `CORRECTIVE_SMOOTH` — `bpy.types.CorrectiveSmoothModifier` (enum label "Smooth Corrective")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `factor` | float (FACTOR) | 0.5 |  |
| `iterations` | int | 5 | [0, 32767] |
| `scale` | float (FACTOR) | 1 |  |
| `rest_source` | enum | 'ORCO' | ORCO / BIND |
| `smooth_type` | enum | 'SIMPLE' | SIMPLE / LENGTH_WEIGHTED |
| `invert_vertex_group` | bool | False |  |
| `vertex_group` | str | '' |  |
| `is_bind` | bool RO | False |  |
| `use_only_smooth` | bool | False |  |
| `use_pin_boundary` | bool | False |  |

#### `WAVE` — `bpy.types.WaveModifier` (enum label "Wave")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `use_x` | bool | True |  |
| `use_y` | bool | True |  |
| `use_cyclic` | bool | True |  |
| `use_normal` | bool | False |  |
| `use_normal_x` | bool | True |  |
| `use_normal_y` | bool | True |  |
| `use_normal_z` | bool | True |  |
| `time_offset` | float (TIME) | 0 | [-1.04857e+06, 1.04857e+06] |
| `lifetime` | float (TIME) | 0 | [-1.04857e+06, 1.04857e+06] |
| `damping_time` | float (TIME) | 10 | [-1.04857e+06, 1.04857e+06] |
| `falloff_radius` | float (DISTANCE) | 0 | ≥ 0 |
| `start_position_x` | float (DISTANCE) | 0 |  |
| `start_position_y` | float (DISTANCE) | 0 |  |
| `start_position_object` | ptr → Object | None |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `speed` | float | 0.25 |  |
| `height` | float (DISTANCE) | 0.5 |  |
| `width` | float (DISTANCE) | 1.5 | ≥ 0 |
| `narrowness` | float (DISTANCE) | 1.5 | ≥ 0 |
| `texture` | ptr → Texture | None |  |
| `texture_coords` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `uv_layer` | str | '' |  |
| `texture_coords_object` | ptr → Object | None |  |
| `texture_coords_bone` | str | '' |  |

#### `WARP` — `bpy.types.WarpModifier` (enum label "Warp")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object_from` | ptr → Object | None |  |
| `bone_from` | str | '' |  |
| `object_to` | ptr → Object | None |  |
| `bone_to` | str | '' |  |
| `strength` | float | 1 |  |
| `falloff_type` | enum | 'SMOOTH' | NONE / CURVE / SMOOTH / SPHERE / ROOT / INVERSE_SQUARE / SHARP / LINEAR / CONSTANT |
| `falloff_radius` | float (DISTANCE) | 1 |  |
| `falloff_curve` | ptr → CurveMapping RO | None |  |
| `use_volume_preserve` | bool | False |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `texture` | ptr → Texture | None |  |
| `texture_coords` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `uv_layer` | str | '' |  |
| `texture_coords_object` | ptr → Object | None |  |
| `texture_coords_bone` | str | '' |  |

#### `HOOK` — `bpy.types.HookModifier` (enum label "Hook")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `strength` | float (FACTOR) | 1 | [0, 1] |
| `falloff_type` | enum | 'SMOOTH' | NONE / CURVE / SMOOTH / SPHERE / ROOT / INVERSE_SQUARE / SHARP / LINEAR / CONSTANT |
| `falloff_radius` | float (DISTANCE) | 0 | ≥ 0 |
| `falloff_curve` | ptr → CurveMapping RO | None |  |
| `center` | float[3] (TRANSLATION) | (0, 0, 0) |  |
| `matrix_inverse` | float[16] (MATRIX) | identity |  |
| `object` | ptr → Object | None |  |
| `subtarget` | str | '' |  |
| `use_falloff_uniform` | bool | False |  |
| `vertex_group` | str | '' |  |
| `vertex_indices` | int[64] RO | {} |  |
| `invert_vertex_group` | bool | False |  |

#### `DATA_TRANSFER` — `bpy.types.DataTransferModifier` (enum label "Data Transfer")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `use_object_transform` | bool | True |  |
| `use_vert_data` | bool | False |  |
| `use_edge_data` | bool | False |  |
| `use_loop_data` | bool | False |  |
| `use_poly_data` | bool | False |  |
| `data_types_verts` | enum-flag (set) | {} | VGROUP_WEIGHTS / BEVEL_WEIGHT_VERT / COLOR_VERTEX |
| `data_types_edges` | enum-flag (set) | {} | SHARP_EDGE / SEAM / CREASE / BEVEL_WEIGHT_EDGE / FREESTYLE_EDGE |
| `data_types_loops` | enum-flag (set) | {} | CUSTOM_NORMAL / COLOR_CORNER / UV |
| `data_types_polys` | enum-flag (set) | {} | SMOOTH / FREESTYLE_FACE |
| `vert_mapping` | enum | 'NEAREST' | TOPOLOGY / NEAREST / EDGE_NEAREST / EDGEINTERP_NEAREST / POLY_NEAREST / POLYINTERP_NEAREST / POLYINTERP_VNORPROJ |
| `edge_mapping` | enum | 'NEAREST' | TOPOLOGY / VERT_NEAREST / NEAREST / POLY_NEAREST / EDGEINTERP_VNORPROJ |
| `loop_mapping` | enum | 'NEAREST_POLYNOR' | TOPOLOGY / NEAREST_NORMAL / NEAREST_POLYNOR / NEAREST_POLY / POLYINTERP_NEAREST / POLYINTERP_LNORPROJ |
| `poly_mapping` | enum | 'NEAREST' | TOPOLOGY / NEAREST / NORMAL / POLYINTERP_PNORPROJ |
| `use_max_distance` | bool | False |  |
| `max_distance` | float (DISTANCE) | 1 | ≥ 0 |
| `ray_radius` | float (DISTANCE) | 0 | ≥ 0 |
| `islands_precision` | float (DISTANCE) | 0 | [0, 1] |
| `layers_vgroup_select_src` | enum | 'ALL' | ACTIVE / ALL / BONE_SELECT / BONE_DEFORM |
| `layers_vcol_vert_select_src` | enum | 'ALL' | ACTIVE / ALL / BONE_SELECT / BONE_DEFORM |
| `layers_vcol_loop_select_src` | enum | 'ALL' | ACTIVE / ALL / BONE_SELECT / BONE_DEFORM |
| `layers_uv_select_src` | enum | 'ALL' | ACTIVE / ALL / BONE_SELECT / BONE_DEFORM |
| `layers_vgroup_select_dst` | enum | 'NAME' | ACTIVE / NAME / INDEX |
| `layers_vcol_vert_select_dst` | enum | 'NAME' | ACTIVE / NAME / INDEX |
| `layers_vcol_loop_select_dst` | enum | 'NAME' | ACTIVE / NAME / INDEX |
| `layers_uv_select_dst` | enum | 'NAME' | ACTIVE / NAME / INDEX |
| `mix_mode` | enum | 'REPLACE' | REPLACE / ABOVE_THRESHOLD / BELOW_THRESHOLD / MIX / ADD / SUB / MUL |
| `mix_factor` | float (FACTOR) | 1 | [0, 1] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |

#### `VERTEX_WEIGHT_EDIT` — `bpy.types.VertexWeightEditModifier` (enum label "Vertex Weight Edit")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `vertex_group` | str | '' |  |
| `falloff_type` | enum | 'LINEAR' | LINEAR / CURVE / SHARP / SMOOTH / ROOT / ICON_SPHERECURVE / RANDOM / STEP |
| `invert_falloff` | bool | False |  |
| `normalize` | bool | False |  |
| `map_curve` | ptr → CurveMapping RO | None |  |
| `use_add` | bool | False |  |
| `use_remove` | bool | False |  |
| `default_weight` | float (FACTOR) | 0 | [0, 1] |
| `add_threshold` | float | 0.01 | [-1000, 1000] |
| `remove_threshold` | float | 0.01 | [-1000, 1000] |
| `mask_constant` | float (FACTOR) | 1 |  |
| `mask_vertex_group` | str | '' |  |
| `invert_mask_vertex_group` | bool | False |  |
| `mask_texture` | ptr → Texture | None |  |
| `mask_tex_use_channel` | enum | 'INT' | INT / RED / GREEN / BLUE / HUE / SAT / VAL / ALPHA |
| `mask_tex_mapping` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `mask_tex_uv_layer` | str | '' |  |
| `mask_tex_map_object` | ptr → Object | None |  |
| `mask_tex_map_bone` | str | '' |  |

#### `VERTEX_WEIGHT_MIX` — `bpy.types.VertexWeightMixModifier` (enum label "Vertex Weight Mix")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `vertex_group_a` | str | '' |  |
| `vertex_group_b` | str | '' |  |
| `invert_vertex_group_a` | bool | False |  |
| `invert_vertex_group_b` | bool | False |  |
| `default_weight_a` | float (FACTOR) | 0 | [0, 1] |
| `default_weight_b` | float (FACTOR) | 0 | [0, 1] |
| `mix_mode` | enum | 'SET' | SET / ADD / SUB / MUL / DIV / DIF / AVG / MIN / MAX |
| `mix_set` | enum | 'AND' | ALL / A / B / OR / AND |
| `normalize` | bool | False |  |
| `mask_constant` | float (FACTOR) | 1 |  |
| `mask_vertex_group` | str | '' |  |
| `invert_mask_vertex_group` | bool | False |  |
| `mask_texture` | ptr → Texture | None |  |
| `mask_tex_use_channel` | enum | 'INT' | INT / RED / GREEN / BLUE / HUE / SAT / VAL / ALPHA |
| `mask_tex_mapping` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `mask_tex_uv_layer` | str | '' |  |
| `mask_tex_map_object` | ptr → Object | None |  |
| `mask_tex_map_bone` | str | '' |  |

#### `VERTEX_WEIGHT_PROXIMITY` — `bpy.types.VertexWeightProximityModifier` (enum label "Vertex Weight Proximity")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `vertex_group` | str | '' |  |
| `proximity_mode` | enum | 'OBJECT' | OBJECT / GEOMETRY |
| `proximity_geometry` | enum-flag (set) | {'VERTEX'} | VERTEX / EDGE / FACE |
| `target` | ptr → Object | None |  |
| `min_dist` | float (DISTANCE) | 0 | ≥ 0 |
| `max_dist` | float (DISTANCE) | 1 | ≥ 0 |
| `falloff_type` | enum | 'LINEAR' | LINEAR / CURVE / SHARP / SMOOTH / ROOT / ICON_SPHERECURVE / RANDOM / STEP |
| `invert_falloff` | bool | False |  |
| `normalize` | bool | False |  |
| `map_curve` | ptr → CurveMapping RO | None |  |
| `mask_constant` | float (FACTOR) | 1 |  |
| `mask_vertex_group` | str | '' |  |
| `invert_mask_vertex_group` | bool | False |  |
| `mask_texture` | ptr → Texture | None |  |
| `mask_tex_use_channel` | enum | 'INT' | INT / RED / GREEN / BLUE / HUE / SAT / VAL / ALPHA |
| `mask_tex_mapping` | enum | 'LOCAL' | LOCAL / GLOBAL / OBJECT / UV |
| `mask_tex_uv_layer` | str | '' |  |
| `mask_tex_map_object` | ptr → Object | None |  |
| `mask_tex_map_bone` | str | '' |  |

#### `LAPLACIANDEFORM` — `bpy.types.LaplacianDeformModifier` (enum label "Laplacian Deform")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `vertex_group` | str | '' |  |
| `iterations` | int | 1 |  |
| `is_bind` | bool RO | False |  |
| `invert_vertex_group` | bool | False |  |

#### `MESH_DEFORM` — `bpy.types.MeshDeformModifier` (enum label "Mesh Deform")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `is_bound` | bool RO | False |  |
| `invert_vertex_group` | bool | False |  |
| `vertex_group` | str | '' |  |
| `precision` | int | 5 | [2, 10] |
| `use_dynamic_bind` | bool | False |  |

#### `SURFACE_DEFORM` — `bpy.types.SurfaceDeformModifier` (enum label "Surface Deform")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `target` | ptr → Object | None |  |
| `falloff` | float | 4 | [2, 16] |
| `is_bound` | bool RO | False |  |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `use_sparse_bind` | bool | False |  |
| `strength` | float | 1 | [-100, 100] |

#### `MESH_CACHE` — `bpy.types.MeshCacheModifier` (enum label "Mesh Cache")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `cache_format` | enum | 'MDD' | MDD / PC2 |
| `interpolation` | enum | 'LINEAR' | NONE / LINEAR |
| `time_mode` | enum | 'FRAME' | FRAME / TIME / FACTOR |
| `play_mode` | enum | 'SCENE' | SCENE / CUSTOM |
| `deform_mode` | enum | 'OVERWRITE' | OVERWRITE / INTEGRATE |
| `filepath` | str (FILE_PATH) | '' |  |
| `factor` | float | 1 | [0, 1] |
| `vertex_group` | str | '' |  |
| `invert_vertex_group` | bool | False |  |
| `forward_axis` | enum | 'POS_Y' | POS_X / POS_Y / POS_Z / NEG_X / NEG_Y / NEG_Z |
| `up_axis` | enum | 'POS_Z' | POS_X / POS_Y / POS_Z / NEG_X / NEG_Y / NEG_Z |
| `flip_axis` | bool[3] | (False, False, False) |  |
| `frame_start` | float (TIME) | 0 | [-1.04857e+06, 1.04857e+06] |
| `frame_scale` | float | 1 | [0, 100] |
| `eval_frame` | float | 0 | [0, 1.04857e+06] |
| `eval_time` | float | 0 | ≥ 0 |
| `eval_factor` | float (FACTOR) | 0 | [0, 1] |

#### `MESH_SEQUENCE_CACHE` — `bpy.types.MeshSequenceCacheModifier` (enum label "Mesh Sequence Cache")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `cache_file` | ptr → CacheFile | None |  |
| `object_path` | str | '' |  |
| `read_data` | enum-flag (set) | {'ATTRIBUTES', 'COLOR', 'POLY', 'UV', 'VERT'} | VERT / POLY / UV / COLOR / ATTRIBUTES |
| `use_vertex_interpolation` | bool | False |  |
| `velocity_scale` | float | 1 | ≥ 0 |

#### `VOLUME_TO_MESH` — `bpy.types.VolumeToMeshModifier` (enum label "Volume to Mesh")

| Property | Type | Default (on `modifiers.new`) | Range / enum items |
|---|---|---|---|
| `object` | ptr → Object | None |  |
| `threshold` | float | 0.1 | ≥ 0 |
| `adaptivity` | float | 0 | [0, 1] |
| `use_smooth_shade` | bool | False |  |
| `grid_name` | str | 'density' |  |
| `resolution_mode` | enum | 'GRID' | GRID / VOXEL_AMOUNT / VOXEL_SIZE |
| `voxel_size` | float | 0.1 | ≥ 0 |
| `voxel_amount` | int | 32 |  |

---

## Appendix C: shader node socket tables

These come from `material.node_tree.nodes.new(bl_idname)` in Blender 5.2.4. "disabled" means `socket.enabled == False` in the default configuration. Name lookup skips disabled sockets (§0.10). Math `operation` items: ADD SUBTRACT MULTIPLY DIVIDE MULTIPLY_ADD POWER LOGARITHM SQRT INVERSE_SQRT ABSOLUTE EXPONENT MINIMUM MAXIMUM LESS_THAN GREATER_THAN SIGN COMPARE SMOOTH_MIN SMOOTH_MAX ROUND FLOOR CEIL TRUNC FRACT MODULO FLOORED_MODULO WRAP SNAP PINGPONG SINE COSINE TANGENT ARCSINE ARCCOSINE ARCTANGENT ARCTAN2 SINH COSH TANH RADIANS DEGREES. VectorMath `operation`: ADD SUBTRACT MULTIPLY DIVIDE MULTIPLY_ADD CROSS_PRODUCT PROJECT REFLECT REFRACT FACEFORWARD DOT_PRODUCT DISTANCE LENGTH SCALE NORMALIZE ABSOLUTE POWER SIGN MINIMUM MAXIMUM ROUND FLOOR CEIL FRACTION MODULO WRAP SNAP SINE COSINE TANGENT.

#### `ShaderNodeBsdfPrincipled` — default name `"Principled BSDF"`

Node properties: `distribution` = 'MULTI_GGX' (GGX / MULTI_GGX); `subsurface_method` = 'RANDOM_WALK' (BURLEY / RANDOM_WALK / RANDOM_WALK_SKIN / RANDOM_WALK_LEGACY)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Base Color` | `Base Color` | Color | (0.8, 0.8, 0.8, 1) |  |
| 1 | `Metallic` | `Metallic` | FloatFactor | 0 |  |
| 2 | `Roughness` | `Roughness` | FloatFactor | 0.5 |  |
| 3 | `IOR` | `IOR` | Float | 1.5 |  |
| 4 | `Alpha` | `Alpha` | FloatFactor | 1 |  |
| 5 | `Thin Wall` | `Thin Wall` | Bool | False |  |
| 6 | `Normal` | `Normal` | Vector | (0, 0, 0) | hide_value |
| 7 | `Weight` | `Weight` | Float | 0 | disabled |
| 8 | `Diffuse Roughness` | `Diffuse Roughness` | FloatFactor | 0 |  |
| 9 | `Subsurface Weight` | `Subsurface Weight` | FloatFactor | 0 |  |
| 10 | `Subsurface Radius` | `Subsurface Radius` | Vector | (1, 0.2, 0.1) |  |
| 11 | `Subsurface Scale` | `Subsurface Scale` | FloatDistance | 0.005 |  |
| 12 | `Subsurface IOR` | `Subsurface IOR` | FloatFactor | 1.4 | disabled |
| 13 | `Subsurface Anisotropy` | `Subsurface Anisotropy` | FloatFactor | 0 |  |
| 14 | `Specular IOR Level` | `Specular IOR Level` | FloatFactor | 0.5 |  |
| 15 | `Specular Tint` | `Specular Tint` | Color | (1, 1, 1, 1) |  |
| 16 | `Anisotropic` | `Anisotropic` | FloatFactor | 0 |  |
| 17 | `Anisotropic Rotation` | `Anisotropic Rotation` | FloatFactor | 0 |  |
| 18 | `Tangent` | `Tangent` | Vector | (0, 0, 0) | hide_value |
| 19 | `Transmission Weight` | `Transmission Weight` | FloatFactor | 0 |  |
| 20 | `Coat Weight` | `Coat Weight` | FloatFactor | 0 |  |
| 21 | `Coat Roughness` | `Coat Roughness` | FloatFactor | 0.03 |  |
| 22 | `Coat IOR` | `Coat IOR` | Float | 1.5 |  |
| 23 | `Coat Tint` | `Coat Tint` | Color | (1, 1, 1, 1) |  |
| 24 | `Coat Normal` | `Coat Normal` | Vector | (0, 0, 0) | hide_value |
| 25 | `Sheen Weight` | `Sheen Weight` | FloatFactor | 0 |  |
| 26 | `Sheen Roughness` | `Sheen Roughness` | FloatFactor | 0.5 |  |
| 27 | `Sheen Tint` | `Sheen Tint` | Color | (1, 1, 1, 1) |  |
| 28 | `Emission Color` | `Emission Color` | Color | (1, 1, 1, 1) |  |
| 29 | `Emission Strength` | `Emission Strength` | Float | 0 |  |
| 30 | `Thin Film Thickness` | `Thin Film Thickness` | FloatWavelength | 0 |  |
| 31 | `Thin Film IOR` | `Thin Film IOR` | Float | 1.33 |  |

Outputs: `BSDF`

#### `ShaderNodeOutputMaterial` — default name `"Material Output"`

Node properties: `is_active_output` = False; `target` = 'ALL' (ALL / EEVEE / CYCLES)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Surface` | `Surface` | Shader | — |  |
| 1 | `Volume` | `Volume` | Shader | — |  |
| 2 | `Displacement` | `Displacement` | Vector | (0, 0, 0) | hide_value |
| 3 | `Thickness` | `Thickness` | Float | 0 | hide_value |

#### `ShaderNodeTexImage` — default name `"Image Texture"`

Node properties: `image` = None; `projection` = 'FLAT' (FLAT / BOX / SPHERE / TUBE); `interpolation` = 'Linear' (Linear / Closest / Cubic / Smart); `projection_blend` = 0; `extension` = 'REPEAT' (REPEAT / EXTEND / CLIP / MIRROR)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Vector` | `Vector` | Vector | (0, 0, 0) | hide_value |

Outputs: `Color`, `Alpha`

#### `ShaderNodeNormalMap` — default name `"Normal Map"`

Node properties: `space` = 'TANGENT' (TANGENT / OBJECT / WORLD / BLENDER_OBJECT / BLENDER_WORLD); `uv_map` = ''; `convention` = 'OPENGL' (OPENGL / DIRECTX); `base` = 'DISPLACED' (ORIGINAL / DISPLACED)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Strength` | `Strength` | Float | 1 |  |
| 1 | `Color` | `Color` | Color | (0.5, 0.5, 1, 1) |  |

Outputs: `Normal`

#### `ShaderNodeBump` — default name `"Bump"`

Node properties: `invert` = False

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Strength` | `Strength` | FloatFactor | 1 |  |
| 1 | `Distance` | `Distance` | Float | 0.001 |  |
| 2 | `Filter Width` | `Filter Width` | FloatPixel | 0.1 |  |
| 3 | `Height` | `Height` | Float | 1 | hide_value |
| 4 | `Normal` | `Normal` | Vector | (0, 0, 0) | hide_value |

Outputs: `Normal`

#### `ShaderNodeMapping` — default name `"Mapping"`

Node properties: `vector_type` = 'POINT' (POINT / TEXTURE / VECTOR / NORMAL)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Vector` | `Vector` | Vector | (0, 0, 0) |  |
| 1 | `Location` | `Location` | VectorTranslation | (0, 0, 0) |  |
| 2 | `Rotation` | `Rotation` | VectorEuler | (0, 0, 0) |  |
| 3 | `Scale` | `Scale` | VectorXYZ | (1, 1, 1) |  |

Outputs: `Vector`

#### `ShaderNodeTexCoord` — default name `"Texture Coordinate"`

Node properties: `object` = None; `from_instancer` = False


Outputs: `Generated`, `Normal`, `UV`, `Object`, `Camera`, `Window`, `Reflection`

#### `ShaderNodeUVMap` — default name `"UV Map"`

Node properties: `from_instancer` = False; `uv_map` = ''


Outputs: `UV`

#### `ShaderNodeVertexColor` — default name `"Color Attribute"`

Node properties: `layer_name` = ''


Outputs: `Color`, `Alpha`

#### `ShaderNodeAttribute` — default name `"Attribute"`

Node properties: `attribute_type` = 'GEOMETRY' (GEOMETRY / OBJECT / INSTANCER / VIEW_LAYER); `attribute_name` = ''


Outputs: `Color`, `Vector`, `Factor` (id `Fac`), `Alpha`

#### `ShaderNodeSeparateColor` — default name `"Separate Color"`

Node properties: `mode` = 'RGB' (RGB / HSV / HSL)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (0.8, 0.8, 0.8, 1) |  |

Outputs: `Red`, `Green`, `Blue`

#### `ShaderNodeCombineColor` — default name `"Combine Color"`

Node properties: `mode` = 'RGB' (RGB / HSV / HSL)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Red` | `Red` | FloatFactor | 0 |  |
| 1 | `Green` | `Green` | FloatFactor | 0 |  |
| 2 | `Blue` | `Blue` | FloatFactor | 0 |  |

Outputs: `Color`

#### `ShaderNodeSeparateXYZ` — default name `"Separate XYZ"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Vector` | `Vector` | Vector | (0, 0, 0) |  |

Outputs: `X`, `Y`, `Z`

#### `ShaderNodeCombineXYZ` — default name `"Combine XYZ"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `X` | `X` | Float | 0 |  |
| 1 | `Y` | `Y` | Float | 0 |  |
| 2 | `Z` | `Z` | Float | 0 |  |

Outputs: `Vector`

#### `ShaderNodeMix` — default name `"Mix"`

Node properties: `data_type` = 'FLOAT' (FLOAT / VECTOR / RGBA / ROTATION); `factor_mode` = 'UNIFORM' (UNIFORM / NON_UNIFORM); `blend_type` = 'MIX' (MIX / DARKEN / MULTIPLY / BURN / LIGHTEN / SCREEN / DODGE / ADD / OVERLAY / SOFT_LIGHT / LINEAR_LIGHT / DIFFERENCE / EXCLUSION / SUBTRACT / DIVIDE / HUE / SATURATION / COLOR / VALUE); `clamp_factor` = True; `clamp_result` = False

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Factor` | `Factor_Float` | FloatFactor | 1 |  |
| 1 | `Factor` | `Factor_Vector` | VectorFactor | (0.5, 0.5, 0.5) | disabled |
| 2 | `A` | `A_Float` | Float | 0 |  |
| 3 | `B` | `B_Float` | Float | 0 |  |
| 4 | `A` | `A_Vector` | Vector | (0, 0, 0) | disabled |
| 5 | `B` | `B_Vector` | Vector | (0, 0, 0) | disabled |
| 6 | `A` | `A_Color` | Color | (0.5, 0.5, 0.5, 1) | disabled |
| 7 | `B` | `B_Color` | Color | (0.5, 0.5, 0.5, 1) | disabled |
| 8 | `A` | `A_Rotation` | Rotation | (0, 0, 0) | disabled |
| 9 | `B` | `B_Rotation` | Rotation | (0, 0, 0) | disabled |

Outputs: `Result` (id `Result_Float`), `Result` (id `Result_Vector`) *(disabled)*, `Result` (id `Result_Color`) *(disabled)*, `Result` (id `Result_Rotation`) *(disabled)*

#### `ShaderNodeMixRGB` — default name `"Mix (Legacy)"`

Node properties: `blend_type` = 'MIX' (MIX / DARKEN / MULTIPLY / BURN / LIGHTEN / SCREEN / DODGE / ADD / OVERLAY / SOFT_LIGHT / LINEAR_LIGHT / DIFFERENCE / EXCLUSION / SUBTRACT / DIVIDE / HUE / SATURATION / COLOR / VALUE); `use_alpha` = False; `use_clamp` = False

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Factor` | `Fac` | FloatFactor | 0.5 |  |
| 1 | `Color1` | `Color1` | Color | (0.5, 0.5, 0.5, 1) |  |
| 2 | `Color2` | `Color2` | Color | (0.5, 0.5, 0.5, 1) |  |

Outputs: `Color`

#### `ShaderNodeMath` — default name `"Math"`

Node properties: `operation` = 'ADD' (41 items); `use_clamp` = False

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Value` | `Value` | Float | 0.5 |  |
| 1 | `Value` | `Value_001` | Float | 0.5 |  |
| 2 | `Value` | `Value_002` | Float | 0.5 | disabled |

Outputs: `Value`

#### `ShaderNodeVectorMath` — default name `"Vector Math"`

Node properties: `operation` = 'ADD' (30 items)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Vector` | `Vector` | Vector | (0, 0, 0) |  |
| 1 | `Vector` | `Vector_001` | Vector | (0, 0, 0) |  |
| 2 | `Vector` | `Vector_002` | Vector | (0, 0, 0) | disabled |
| 3 | `Scale` | `Scale` | Float | 1 | disabled |

Outputs: `Vector`, `Value` *(disabled)*

#### `ShaderNodeValue` — default name `"Value"`


Outputs: `Value`

#### `ShaderNodeRGB` — default name `"Color"`


Outputs: `Color`

#### `ShaderNodeInvert` — default name `"Invert Color"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Factor` | `Fac` | FloatFactor | 1 |  |
| 1 | `Color` | `Color` | Color | (0, 0, 0, 1) |  |

Outputs: `Color`

#### `ShaderNodeEmission` — default name `"Emission"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (1, 1, 1, 1) |  |
| 1 | `Strength` | `Strength` | Float | 1 |  |
| 2 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `Emission`

#### `ShaderNodeBsdfTransparent` — default name `"Transparent BSDF"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (1, 1, 1, 1) |  |
| 1 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `BSDF`

#### `ShaderNodeBsdfDiffuse` — default name `"Diffuse BSDF"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (0.8, 0.8, 0.8, 1) |  |
| 1 | `Roughness` | `Roughness` | FloatFactor | 0 |  |
| 2 | `Normal` | `Normal` | Vector | (0, 0, 0) | hide_value |
| 3 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `BSDF`

#### `ShaderNodeMixShader` — default name `"Mix Shader"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Factor` | `Fac` | FloatFactor | 0.5 |  |
| 1 | `Shader` | `Shader` | Shader | — |  |
| 2 | `Shader` | `Shader_001` | Shader | — |  |

Outputs: `Shader`

#### `ShaderNodeAddShader` — default name `"Add Shader"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Shader` | `Shader` | Shader | — |  |
| 1 | `Shader` | `Shader_001` | Shader | — |  |

Outputs: `Shader`

#### `ShaderNodeVolumeAbsorption` — default name `"Volume Absorption"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (0.8, 0.8, 0.8, 1) |  |
| 1 | `Density` | `Density` | Float | 1 |  |
| 2 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `Volume`

#### `ShaderNodeVolumePrincipled` — default name `"Principled Volume"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (0.5, 0.5, 0.5, 1) |  |
| 1 | `Color Attribute` | `Color Attribute` | String | '' |  |
| 2 | `Density` | `Density` | Float | 1 |  |
| 3 | `Density Attribute` | `Density Attribute` | String | 'density' |  |
| 4 | `Anisotropy` | `Anisotropy` | FloatFactor | 0 |  |
| 5 | `Absorption Color` | `Absorption Color` | Color | (0, 0, 0, 1) |  |
| 6 | `Emission Strength` | `Emission Strength` | Float | 0 |  |
| 7 | `Emission Color` | `Emission Color` | Color | (1, 1, 1, 1) |  |
| 8 | `Blackbody Intensity` | `Blackbody Intensity` | FloatFactor | 0 |  |
| 9 | `Blackbody Tint` | `Blackbody Tint` | Color | (1, 1, 1, 1) |  |
| 10 | `Temperature` | `Temperature` | FloatColorTemperature | 1000 |  |
| 11 | `Temperature Attribute` | `Temperature Attribute` | String | 'temperature' |  |
| 12 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `Volume`

#### `ShaderNodeBackground` — default name `"Background"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (0.8, 0.8, 0.8, 1) |  |
| 1 | `Strength` | `Strength` | Float | 1 |  |
| 2 | `Weight` | `Weight` | Float | 0 | disabled |

Outputs: `Background`

#### `ShaderNodeTexEnvironment` — default name `"Environment Texture"`

Node properties: `image` = None; `projection` = 'EQUIRECTANGULAR' (EQUIRECTANGULAR / MIRROR_BALL); `interpolation` = 'Linear' (Linear / Closest / Cubic / Smart)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Vector` | `Vector` | Vector | (0, 0, 0) | hide_value |

Outputs: `Color`

#### `ShaderNodeOutputWorld` — default name `"World Output"`

Node properties: `is_active_output` = True; `target` = 'ALL' (ALL / EEVEE / CYCLES)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Surface` | `Surface` | Shader | — |  |
| 1 | `Volume` | `Volume` | Shader | — |  |

#### `ShaderNodeNewGeometry` — default name `"Geometry"`


Outputs: `Position`, `Normal`, `Tangent`, `True Normal`, `Incoming`, `Parametric`, `Backfacing`, `Pointiness`, `Random Per Island`

#### `ShaderNodeTangent` — default name `"Tangent"`

Node properties: `direction_type` = 'RADIAL' (RADIAL / UV_MAP); `axis` = 'Z' (X / Y / Z); `uv_map` = ''


Outputs: `Tangent`

#### `ShaderNodeLightPath` — default name `"Light Path"`


Outputs: `Is Camera Ray`, `Is Shadow Ray`, `Is Diffuse Ray`, `Is Glossy Ray`, `Is Singular Ray`, `Is Reflection Ray`, `Is Transmission Ray`, `Is Volume Scatter Ray`, `Ray Length`, `Ray Depth`, `Diffuse Depth`, `Glossy Depth`, `Transparent Depth`, `Transmission Depth`, `Portal Depth`

#### `ShaderNodeValToRGB` — default name `"Color Ramp"`

Node properties: `color_ramp` = None

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Factor` | `Fac` | FloatFactor | 0.5 |  |

Outputs: `Color`, `Alpha`

#### `ShaderNodeClamp` — default name `"Clamp"`

Node properties: `clamp_type` = 'MINMAX' (MINMAX / RANGE)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Value` | `Value` | Float | 1 |  |
| 1 | `Min` | `Min` | Float | 0 |  |
| 2 | `Max` | `Max` | Float | 1 |  |

Outputs: `Result`

#### `ShaderNodeMapRange` — default name `"Map Range"`

Node properties: `clamp` = True; `interpolation_type` = 'LINEAR' (LINEAR / STEPPED / SMOOTHSTEP / SMOOTHERSTEP); `data_type` = 'FLOAT' (FLOAT / FLOAT_VECTOR)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Value` | `Value` | Float | 1 |  |
| 1 | `From Min` | `From Min` | Float | 0 |  |
| 2 | `From Max` | `From Max` | Float | 1 |  |
| 3 | `To Min` | `To Min` | Float | 0 |  |
| 4 | `To Max` | `To Max` | Float | 1 |  |
| 5 | `Steps` | `Steps` | Float | 4 | disabled |
| 6 | `Vector` | `Vector` | Vector | (0, 0, 0) | disabled, hide_value |
| 7 | `From Min` | `From_Min_FLOAT3` | Vector | (0, 0, 0) | disabled |
| 8 | `From Max` | `From_Max_FLOAT3` | Vector | (1, 1, 1) | disabled |
| 9 | `To Min` | `To_Min_FLOAT3` | Vector | (0, 0, 0) | disabled |
| 10 | `To Max` | `To_Max_FLOAT3` | Vector | (1, 1, 1) | disabled |
| 11 | `Steps` | `Steps_FLOAT3` | Vector | (4, 4, 4) | disabled |

Outputs: `Result`, `Vector` *(disabled)*

#### `ShaderNodeHueSaturation` — default name `"Hue/Saturation/Value"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Hue` | `Hue` | Float | 0.5 |  |
| 1 | `Saturation` | `Saturation` | Float | 1 |  |
| 2 | `Value` | `Value` | Float | 1 |  |
| 3 | `Factor` | `Fac` | FloatFactor | 1 |  |
| 4 | `Color` | `Color` | Color | (0.8, 0.8, 0.8, 1) |  |

Outputs: `Color`

#### `ShaderNodeBrightContrast` — default name `"Brightness/Contrast"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (1, 1, 1, 1) |  |
| 1 | `Brightness` | `Bright` | Float | 0 |  |
| 2 | `Contrast` | `Contrast` | Float | 0 |  |

Outputs: `Color`

#### `ShaderNodeGamma` — default name `"Gamma"`

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Color` | `Color` | Color | (1, 1, 1, 1) | hide_value |
| 1 | `Gamma` | `Gamma` | Float | 1 |  |

Outputs: `Color`

#### `ShaderNodeOutputLight` — default name `"Light Output"`

Node properties: `is_active_output` = True; `target` = 'ALL' (ALL / EEVEE / CYCLES)

| # | Input name | identifier | socket type | default | notes |
|---|---|---|---|---|---|
| 0 | `Surface` | `Surface` | Shader | — |  |

#### `ShaderNodeGroup` — default name `"Group"`

Node properties: `node_tree` = None

