# Blender 5.2 edit-mode mesh and UV operators: research catalogue for frap

Status: research, no code yet. Target: **Blender 5.2 LTS Python API**.
Verified against a local **Blender 5.2.4 LTS** (build `cd5dc428cce1`, 2026-11-10) running `blender -b --factory-startup`. Operator names, property names, types, defaults, ranges and enum items in this document come from RNA introspection, not from memory (Appendix A has the full dump). The behaviour notes (what gets selected, element counts, vertex and face order) come from experiments run in that build (Appendix B has the raw output).

Legend used throughout:

| mark | meaning |
|---|---|
| ✅ | verified by running Blender 5.2.4 headless |
| 📖 | from knowledge of the Blender C/C++ source (`editmesh_*.cc`, `bmo_*.cc`). Likely correct, but not executed here |
| ⚠️ | uncertain or surprising. Re-check before relying on it |
| 🚫H | does **not** work in `blender -b` (poll fails or crashes). frap must define its own headless semantics, and golden tests must use an equivalent sequence |

Priority column: **P0** = needed for an MVP modelling loop, **P1** = standard modelling, **P2** = nice to have or niche.

---

## 0. Findings and pitfalls the architect must know

1. **`mesh.loop_multi_select` no longer exists in 5.2.** ✅ It was split into `mesh.select_edge_loop_multi(delimit_edge_loop={...})`, `mesh.select_edge_ring_multi(delimit_edge_ring={...})` and `mesh.select_boundary_loop_multi(extend, delimit_edge_loop)`. `mesh.normals_hide`, `mesh.set_edge_flow`, `mesh.edge_slide` and `mesh.vert_slide` do not exist either. Edge and vertex slide are `transform.edge_slide` and `transform.vert_slide`.
2. **New in 5.x (LoopTools-like ops in core)** ✅: `mesh.circularize`, `mesh.flatten`, `mesh.space_edge_loops_evenly`, `mesh.reorder_vertices_spatial`, `mesh.select_by_pole_count`, `mesh.select_next_item`/`select_prev_item`. Matching bmesh ops: `bmesh.ops.circularize`, `flatten`, `space_edge_loops_evenly`.
3. **`uv.unwrap` default `method` is `'CONFORMAL'`** ✅ (it used to be ANGLE_BASED). The enum is `ANGLE_BASED | CONFORMAL | MINIMUM_STRETCH` (SLIM). New props: `use_subsurf_data`, `use_original_bounds`, `margin_method`, `no_flip`, `iterations`, `use_weights`, `weight_group='uv_importance'`, `weight_factor`. `fill_holes` defaults to **False**.
4. **UV sync select is ON by default in 5.x** ✅ (`tool_settings.use_uv_select_sync == True` in factory startup). Meshes also carry the internal UV selection attributes `.uv_select_vert`, `.uv_select_edge` (CORNER) and `.uv_select_face` (FACE).
5. **Geometry Nodes modifier inputs are no longer ID properties** ✅. In 5.2, `modifier["Input_1"]` raises `TypeError: this type doesn't support IDProperties`. The new path is `modifier.properties.inputs.Input_1.value`, whose RNA path is `modifiers["Smooth by Angle"].properties.inputs.Input_1`. `inputs['Input_1']` also works. Lookup by display name (`inputs['Angle']`) does **not**. This affects the "Smooth by Angle" path (see §2.6) and any LLM-facing docs that copy 4.x tutorials.
6. **Auto smooth is gone** ✅. `object.shade_auto_smooth(use_auto_smooth=True, angle=0.5236)` adds a **NODES modifier named "Smooth by Angle"** that uses a linked-and-packed essentials asset node group (`geometry_nodes_essentials.blend`, `is_linked_packed=True`). Its inputs are `Input_0` (Mesh geometry), `Input_1` (Angle, float, default 30°) and `Socket_1` (Ignore Sharpness, bool). `object.shade_smooth_by_angle(angle, keep_sharp_edges)` instead writes `sharp_edge`/`sharp_face` destructively. frap should implement "Smooth by Angle" as a built-in modifier that mimics this node group, keyed by the same identifiers.
7. **Element order depends on BMesh's memory pool (BLI_mempool).** ✅ Deleted slots go onto a LIFO free list and are reused by the next allocations. In the test, deleting v2 then v4 and then adding three verts put them at slots 4, 2 and then the end. `to_mesh` writes elements in pool iteration order. Many ops delete and recreate faces, so the original face's index is usually taken by some *other* new face. Extrude is an example: the side face lands in slot 5 and the cap in slot 6. **To get index-exact parity with Blender, frap's BMesh arena needs a free list with the same LIFO reuse, and each op has to replicate Blender's create and delete order.** Recommendation: golden tests compare **order-independent canonical forms** by default (sorted position multiset, face cycles as rotation-normalised position tuples, UVs per corner keyed by face and vertex position). Strict index-order tests should run only on ops that are confirmed to match (primitives, subdivide, extrude, inset, split, poke, triangulate).
8. **Interactive-only ops** 🚫H: `mesh.loopcut` and `loopcut_slide` **crash Blender in background** (SIGSEGV in `loopcut_init`, because the invoke path needs a region; with a `VIEW_3D` `temp_override` the exec runs fine, see the correction in 4.5). `uv.project_from_view` fails poll. `mesh.rip`/`rip_move` fail poll. `mesh.spin` and `mesh.screw` need an explicit `axis` (the default `(0,0,0)` gives "Invalid/unset axis", because invoke normally fills it from the view). `uv.follow_active_quads` needs an active face. `mesh.loop_select`, `edgering_select`, `select_linked_pick` and `shortest_path_pick` are pick ops that take hidden `edge_index`/`vert_index`/`face_index`/`object_index`. **frap should make these work from their index props**, which is more than Blender offers headless. The golden-test harness emulates `loopcut` with `bmesh.ops.subdivide_edges(edges=ring, cuts=n, use_grid_fill=True, use_single_edge=True, use_only_quads=True, quad_corner_type='PATH', smooth_falloff=…)` plus `select_edge_ring_multi` ✅ (Appendix B.7).
9. **HIDDEN does not mean irrelevant.** ✅ `mesh.select_all.action`, `mesh.delete.type`, `mesh.select_mode.type/use_extend/use_expand/action` and `uv.select_mode.type` are all `PROP_HIDDEN` and `SKIP_SAVE`. frap's `--describe` schema must still expose them. Use `SKIP_SAVE` as the cue that a property is not remembered between calls. frap is stateless per call anyway, but the flag matters for REDO semantics.
10. **Macro ops** ✅ such as `extrude_region_move` have exactly two pointer props named after the sub-operator idnames (`MESH_OT_extrude_region`, `TRANSFORM_OT_translate`). Python syntax: `bpy.ops.mesh.extrude_region_move(MESH_OT_extrude_region={"use_normal_flip":False}, TRANSFORM_OT_translate={"value":(0,0,1)})`. With `exec`, translate uses `orient_type='GLOBAL'` unless told otherwise ✅. The "moves along the normal" behaviour comes only from invoke in the UI. The frap short form should flatten this, for example `mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1`.
11. **Attribute names (5.2)** ✅: `position`, `sharp_face`, `sharp_edge`, `uv_seam` (renamed from `.uv_seam` / `use_seam`), `crease_edge`, `bevel_weight_edge`, `custom_normal` (INT16_2D, CORNER), `material_index`, UV maps as `FLOAT2` CORNER attributes (`UVMap`, `UVMap.001`, …). Internal attributes start with `.` (`.edge_verts`, `.corner_vert`, `.corner_edge`, `.select_vert`, `.select_edge`, `.select_poly`, `.uv_select_*`, `.hide_*`). The `Mesh.corner_verts`/`corner_edges` Python properties **are not exposed** in 5.2. Use `mesh.loops[i].vertex_index`/`edge_index` or the attributes `.corner_vert`/`.corner_edge`.
12. **`mesh.set_sharpness_by_angle` adds a `custom_normal` attribute** ⚠️✅ (observed, and it happened even on a flat-shaded cube). The likely reason is that it preserves existing custom normals. Golden tests should ignore the presence of a zeroed `custom_normal`, or frap should reproduce this.
13. **`select_random` is deterministic for a given `seed`** ✅ (identical across runs). For multi-object edit, Blender mixes in a per-object hash 📖. frap must use Blender's RNG (`BLI_rng`, a drand48-style LCG) to match. Otherwise, mark it non-golden.
14. **Default select mode is VERT** ✅, and `tool_settings` carries over across scene ops. `mesh.select_mode(type=…)` converts selection (see §3.3).

---

## 1. Blender 5.2 mesh data model (what frap's core must mirror)

### 1.1 `bpy.types.Mesh` RNA surface (5.2.4) ✅

Geometry: `vertices`, `edges`, `polygons`, `loops`, `loop_triangles`, `loop_triangle_polygons`, `attributes`, `color_attributes`, `uv_layers`, `vertex_colors` (legacy API, creates a BYTE_COLOR CORNER attribute named `Col`), `skin_vertices`, `shape_keys`, `materials`, `vertex_normals`, `polygon_normals`, `corner_normals`, `normals_domain`, `has_custom_normals`, `total_vert_sel`, `total_edge_sel`, `total_face_sel`, `is_editmode`, `use_auto_texspace`, `texspace_location`, `texspace_size`, `texture_mesh`, `use_mirror_x/y/z`, `use_mirror_topology`, `use_mirror_vertex_groups`, `radial_symmetry`, `remesh_*`, `uv_layer_clone(_index)`, `uv_layer_stencil(_index)`.

Functions: `transform(matrix)`, `flip_normals()`, `set_sharp_from_angle(angle)`, `split_faces()`, `calc_tangents(uvmap)`, `free_tangents()`, `calc_loop_triangles()`, `calc_smooth_groups()`, `normals_split_custom_set(normals)`, `normals_split_custom_set_from_vertices(normals)`, `update()`, `validate()`, `validate_material_indices()`, `count_selected_items()`, `clear_geometry()`, `unit_test_compare()`.

**Not present (removed earlier):** `use_auto_smooth`, `auto_smooth_angle`, `calc_normals_split()`, `create_normals_split()`, `free_normals_split()`, `MeshPolygon.use_smooth` remains but is a proxy for `!sharp_face`, `MeshEdge.use_seam` is a proxy for `uv_seam`.

Element structs ✅:

| struct | props |
|---|---|
| `MeshVertex` | `co`, `normal` (read-only), `select`, `hide`, `groups`, `index`, `undeformed_co` |
| `MeshEdge` | `vertices[2]`, `select`, `hide`, `use_seam`, `use_edge_sharp`, `is_loose`, `index` |
| `MeshPolygon` | `vertices`, `loop_start`, `loop_total`, `material_index`, `select`, `hide`, `use_smooth`, `normal`, `center`, `area`, `index` |
| `MeshLoop` (= face corner) | `vertex_index`, `edge_index`, `index`, `normal`, `tangent`, `bitangent_sign`, `bitangent` |
| `MeshUVLoopLayer` | `name`, `active`, `active_render`, `active_clone`, `data[].uv`, `uv[]` (vector access), `pin[]` |

Bevel weight and crease **are no longer struct props**. They exist only as attributes.

### 1.2 Storage = attributes on 4 domains ✅

The Mesh is a struct of arrays. Topology is stored as attributes too:

| name | type | domain | required | notes |
|---|---|---|---|---|
| `position` | FLOAT_VECTOR | POINT | yes | vertex coordinates |
| `.edge_verts` | INT32_2D | EDGE | yes | edge → 2 vertex indices |
| `.corner_vert` | INT | CORNER | yes | corner → vertex |
| `.corner_edge` | INT | CORNER | yes | corner → edge (edge from this corner's vert to next corner's vert) |
| (face offsets) | int[] | — | yes | `polygons[i].loop_start` / `loop_total`. Not an attribute, it is a separate offsets array (`face_offset_indices`) |
| `sharp_face` | BOOLEAN | FACE | no | **flat-shaded face = True**. Absent means all smooth. `object.shade_smooth` *removes* the attribute ✅ |
| `sharp_edge` | BOOLEAN | EDGE | no | split normals along edge (`mark_sharp`) |
| `uv_seam` | BOOLEAN | EDGE | no | `mark_seam` ✅ (5.x name) |
| `crease_edge` | FLOAT | EDGE | no | `transform.edge_crease` ✅ |
| `crease_vert` | FLOAT | POINT | no | `transform.vert_crease` 📖 |
| `bevel_weight_edge` | FLOAT | EDGE | no | `transform.edge_bevelweight` ✅ |
| `bevel_weight_vert` | FLOAT | POINT | no | 📖 |
| `material_index` | INT | FACE | no | absent means all 0 |
| `custom_normal` | INT16_2D | CORNER | no | **5.x custom normals**: encoded in a per-corner *lnor space* as 2 int16 (angle factors). Present ⇒ `has_custom_normals` and `normals_domain=='CORNER'` ✅ |
| UV maps (`UVMap`, …) | FLOAT2 | CORNER | no | `uv_layers` is a view over these. Pin is `.pn.<uvname>` BOOLEAN CORNER 📖 |
| color attributes | FLOAT_COLOR / BYTE_COLOR | POINT or CORNER | no | `geometry.color_attribute_add` default: name `Attribute`, FLOAT_COLOR, POINT ✅. `attributes.active_color_name`, `default_color_name`, `render_color_index` ✅ |
| `.select_vert/.select_edge/.select_poly` | BOOLEAN | P/E/F | no | selection, persisted in object mode |
| `.hide_vert/.hide_edge/.hide_poly` | BOOLEAN | P/E/F | no | 📖 |
| `.uv_select_vert/.uv_select_edge` | BOOLEAN | CORNER | no | 5.x UV selection ✅ |
| `.uv_select_face` | BOOLEAN | FACE | no | ✅ |
| vertex groups | — | — | — | stored in `MDeformVert` (not an attribute), names on `Object.vertex_groups` (5.x moved names to the Mesh: `mesh.vertex_group_names` 📖⚠️, check) |

`Mesh.attributes.new(name, type, domain)` ✅ types: `FLOAT, INT, BOOLEAN, FLOAT_VECTOR, FLOAT_COLOR, QUATERNION, FLOAT4X4, STRING, INT8, INT16_2D, INT32_2D, FLOAT2, FLOAT4, BYTE_COLOR`. Domains: `POINT, EDGE, FACE, CORNER, CURVE, INSTANCE, LAYER`. `attributes.remove(attr)`, `attributes.domain_size(domain)`, `attributes.active`/`active_index`/`active_color`/`active_color_index`/`render_color_index`/`default_color_name`/`active_color_name`.

`uv_layers.new(name='UVMap', do_init=True)` ✅ copies the active UV map, or the default unit layout when `do_init`. `mesh.uv_texture_add` ✅ creates `UVMap.001` (copy of active). Setting `uv_layers.active` sets the active UV. `active_render` is the one exported by glTF as `TEXCOORD_0` 📖.

### 1.3 Normals model (4.1+) ✅

* `normals_domain` ∈ `POINT | FACE | CORNER`. Blender derives it as follows: `FACE` if all faces are sharp (`sharp_face` all True), `POINT` if nothing is sharp and there are no custom normals, `CORNER` otherwise. Fresh cube: `FACE` ✅. With `custom_normal`: `CORNER` ✅.
* Corner normals: smooth fan around each vertex, split by sharp edges (`sharp_edge`), by sharp faces (`sharp_face` boundary), and by **non-manifold/boundary** edges. Face-corner fans are angle-weighted (corner angle) 📖.
* **Auto smooth** (before 4.1: `use_auto_smooth` + angle) was replaced by:
  * `object.shade_auto_smooth(use_auto_smooth=True, angle=30°)` → adds or updates a **"Smooth by Angle" GN modifier** ✅. `use_auto_smooth=False` removes it. The node group sets `sharp_edge = (edge_angle > Angle) || (!IgnoreSharpness && is_edge_sharp)` and makes faces smooth 📖✅ (nodes: Edge Angle, Is Edge Smooth, Is Shade Smooth, Compare, Boolean Math ×3, Set Shade Smooth ×2).
  * `object.shade_smooth_by_angle(angle=30°, keep_sharp_edges=True)` ✅ is destructive and writes `sharp_edge`. It also removes `sharp_face` (all smooth) ✅.
  * `mesh.set_sharpness_by_angle(angle=30°, extend=False)` ✅ is the edit-mode version. It writes `sharp_edge` on selected edges and adds `custom_normal` ⚠️.
  * `Mesh.set_sharp_from_angle(angle)` ✅ is the Python API version.
* glTF export: the exporter evaluates the mesh (modifiers applied), splits vertices where corner normals differ, and writes `NORMAL`. frap's GLB writer must split by (position, corner normal, every UV, every corner color) 📖.

### 1.4 Reference: `mesh.primitive_cube_add()` (size=2) data ✅

The startup "Cube" in the factory scene has a **different** vertex order from `primitive_cube_add`. Always build test fixtures with the operator.

```
verts  0(-1,-1,-1) 1(-1,-1,1) 2(-1,1,-1) 3(-1,1,1) 4(1,-1,-1) 5(1,-1,1) 6(1,1,-1) 7(1,1,1)
edges  [2,0][0,1][1,3][3,2][6,2][3,7][7,6][4,6][7,5][5,4][0,4][5,1]
faces  0:[0,1,3,2](-X) 1:[2,3,7,6](+Y) 2:[6,7,5,4](+X) 3:[4,5,1,0](-Y) 4:[2,6,4,0](-Z) 5:[7,3,1,5](+Z)
UVMap (corner order): f0 (.375,0)(.625,0)(.625,.25)(.375,.25) | f1 (.375,.25)(.625,.25)(.625,.5)(.375,.5)
       f2 (.375,.5)(.625,.5)(.625,.75)(.375,.75) | f3 (.375,.75)(.625,.75)(.625,1)(.375,1)
       f4 (.125,.5)(.375,.5)(.375,.75)(.125,.75) | f5 (.625,.5)(.875,.5)(.875,.75)(.625,.75)
attrs: position, UVMap (FLOAT2 CORNER), sharp_face (all True → flat). All elements selected after add.
```

---

## 2. BMesh concepts frap needs (edit-mode representation)

### 2.1 Elements ✅ (from `bmesh.types` introspection)

| type | key fields | topology links |
|---|---|---|
| `BMVert` | `co`, `normal`, `select`, `hide`, `tag`, `index` | `link_edges`, `link_faces`, `link_loops`. Disk cycle: the circular list of edges around a vertex |
| `BMEdge` | `verts[2]`, `seam`, `smooth` (= !sharp), `select`, `hide`, `index` | `link_faces`, `link_loops`. Radial cycle: the circular list of loops (face corners) using this edge |
| `BMFace` | `normal`, `material_index`, `smooth`, `select`, `hide`, `index`, `uv_select` (5.x) | `verts`, `edges`, `loops` (circular list, `loops[0]` = `l_first`) |
| `BMLoop` | `vert`, `edge` (the edge from `vert` to `link_loop_next.vert`), `face`, `index`, `uv_select_vert`/`uv_select_edge` (5.x) | `link_loop_next/prev` (face cycle), `link_loop_radial_next/prev` (around the edge) |

Custom data lives in per-domain layers: `bm.verts.layers.{float,int,bool,float_vector,float_color,color,string,deform,shape,…}`, `bm.loops.layers.uv`, and so on. The 5.x `layers` accessors are `bool, color, float, float_color, float_vector, int, string, uv` ✅.

### 2.2 Selection model

* Each element has a SELECT flag. **Flushing** (`bm.select_flush_mode()` / `EDBM_selectmode_flush`):
  * VERT mode: an edge is selected iff both verts are selected. A face is selected iff all its verts are selected.
  * EDGE mode: a face is selected iff all its edges are selected. Verts follow edges.
  * FACE mode: only faces are authoritative. Edges and verts of selected faces are selected.
* `select_set(True)` on an edge or face also selects its verts. **Deselect does not flush down** (deselecting a face leaves the verts selected). Blender runs `select_flush_mode` after ops.
* **`select_history`** (`BMEditSelection` list): an ordered list of (element, type). The **last** entry is the "active" element (active vert/edge/face). `bm.faces.active` is separate (the active face used by UV and material ops). Ops with `use_select_history=True` (extrude*, duplicate) remap history to the new elements ✅. Most destructive ops (inset, bevel, subdivide, delete, poke, split, solidify, spin) **clear** it ✅. `mesh.merge` leaves the merged vert in history ✅.
* `merge(type='FIRST'|'LAST')` needs history to start or end with a vert. The enum items are filtered dynamically 📖.

### 2.3 Index validity

`BMElem.index` is only valid after `index_update()`. Ops leave indices dirty. `bmesh.update_edit_mesh` / `to_mesh` / `EDBM_update` fix them. frap should treat "index" as "position in iteration order", computed lazily ✅ (stale indices were observed before `index_update()`).

### 2.4 Mempool ordering (critical for index parity) ✅

* Allocation reuses the most recently freed slot first (LIFO). With no freed slots, it appends.
* Iteration = pool chunk order, skipping free slots. `to_mesh()` packs in iteration order.
* Consequence (verified, extrude top face of cube): faces 0–4 unchanged. The original top face is killed. The new side faces and the cap are allocated as `[side(7,3,9,11) → slot 5], cap → 6, side → 7, side → 8, side → 9`. The first new face takes the freed slot. Verts and edges show the same pattern.

### 2.5 Edit-mode ⇄ object-mode conversion

* `object.mode_set(mode='EDIT')` builds a BMesh from the Mesh (`BM_mesh_bm_from_me`): elements keep their indices, selection comes from `.select_*`, hide from `.hide_*`, attributes become CustomData layers, and UV selection comes from `.uv_select_*`.
* `mode_set(mode='OBJECT')` writes back (`BM_mesh_bm_to_me`) in iteration order. `sharp_face`/`sharp_edge`/`uv_seam` are written only if some element is True (attributes with all-False values are removed) ✅ (observed: `shade_smooth` removes `sharp_face`, and clearing all seams removes `uv_seam`).
* Shape keys: the BMesh holds a `shape` layer per key. On write-back, offsets are propagated to other keys when the basis is edited 📖.

### 2.6 Modifier paths frap must mirror (5.2) ✅

`ob.modifiers.new(name, type)`. For NODES: `mod.node_group`, `mod.properties.inputs.<identifier>.value` (5.2 change, see §0.5). "Smooth by Angle" identifiers: `Input_1` = Angle, `Socket_1` = Ignore Sharpness.

---

## 3. Edit-mode context

### 3.1 Tool settings (`scene.tool_settings`) ✅

| prop | default (factory) | notes |
|---|---|---|
| `mesh_select_mode` | `(True, False, False)` | bool[3] vert/edge/face. Multiple can be on at once |
| `use_mesh_automerge` | False | auto-merge after transform (uses `double_threshold`) |
| `use_mesh_automerge_and_split` | False | |
| `double_threshold` | 0.001 📖 | automerge distance |
| `use_uv_select_sync` | **True** (5.x) | |
| `uv_select_mode` | `'VERTEX'` | VERTEX/EDGE/FACE/ISLAND 📖 |
| `uv_sticky_select_mode` | `'SHARED_LOCATION'` 📖 | |
| `use_uv_select_island` | False ✅ | |
| `use_proportional_edit`, `proportional_edit_falloff`, `proportional_size` | False, SMOOTH, 1.0 | used by transform when the op prop is unset |
| `use_snap` etc. | off | frap: ignore snapping (P2) |

### 3.2 `object.mode_set(mode, toggle)` ✅

Enum: `OBJECT, EDIT, POSE, SCULPT, VERTEX_PAINT, WEIGHT_PAINT, TEXTURE_PAINT, PARTICLE_EDIT, EDIT_GPENCIL, SCULPT_GREASE_PENCIL, PAINT_GREASE_PENCIL, WEIGHT_GREASE_PENCIL, VERTEX_GREASE_PENCIL, SCULPT_CURVES`. frap needs `OBJECT`, `EDIT` and `POSE` (armatures). With several selected mesh objects, EDIT enters **multi-object edit** for all selected objects of the same type 📖. Most mesh ops then loop over every object in edit mode (`BKE_view_layer_array_from_objects_in_edit_mode_unique_data`). frap should support this, because LLM scripts will select several objects.

### 3.3 `mesh.select_mode(use_extend=False, use_expand=False, type='VERT', action='TOGGLE')` ✅

Selection conversion when switching modes (verified):

* face → edge: the face's edges and verts stay selected. Face → vert: same.
* vert → face: only faces whose verts are **all** selected stay selected. Two verts → FACE mode leaves **nothing selected** ✅.
* edge → face: faces whose edges are all selected.
* `use_expand=True` (Ctrl in UI) when going up: vert→edge selects edges touching any selected vert 📖. Going down: face→vert keeps. ⚠️ Check the exact semantics if needed.
* `use_extend=True`: adds a mode (`(True,True,False)`) ✅.

---

## 4. Operator catalogue

The full property tables are in **Appendix A**. This section covers semantics. "Sel after" means the selection state after the op ✅ as observed in VERT mode on the primitive cube, unless noted.

### 4.1 Selection ops

| bl_idname | key props (defaults) | semantics | headless | prio |
|---|---|---|---|---|
| `mesh.select_all` | `action='TOGGLE'` (TOGGLE/SELECT/DESELECT/INVERT) | TOGGLE: if anything is selected, deselect all, else select all. INVERT in VERT mode inverts verts then flushes ✅. Does not touch history except to clear on deselect 📖 | ✅ | P0 |
| `mesh.select_mode` | see §3.3 | | ✅ | P0 |
| `mesh.select_random` | `ratio=0.5, seed=0, action='SELECT'` | per element of the current select mode, RNG(seed) then flush. Deterministic ✅ | ✅ | P2 |
| `mesh.select_linked` | `delimit={'SEAM'}` (NORMAL/MATERIAL/SEAM/SHARP/UV) | flood fill from the selection across edges, not crossing delimiters. **Default delimit is SEAM** ✅ | ✅ | P0 |
| `mesh.select_linked_pick` | `deselect, delimit, index` (hidden) | pick version. frap: allow `index` | 🚫H (index usable 📖) | P2 |
| `mesh.select_more` / `select_less` | `use_face_step=True` | grow/shrink by one ring. `use_face_step=True`: grow to all verts of faces touching the selection, not only edge neighbours ✅ (grid 5×5 subdivisions, boundary vert v12 → {6,7,12,13,18,19}, the 2 adjacent faces. `select_less` brings it back to {12}) ✅ | ✅ | P0 |
| `mesh.select_non_manifold` | `extend=True, use_wire, use_boundary, use_multi_face, use_non_contiguous, use_verts` (all True) | **Only in VERT or EDGE mode** (fails in face mode 📖). Grid → all boundary verts and edges ✅ | ✅ | P1 |
| `mesh.select_loose` | `extend=False` | verts/edges with no faces (by select mode) | ✅ | P1 |
| `mesh.select_face_by_sides` | `number=4, type='EQUAL'` (LESS/EQUAL/GREATER/NOTEQUAL), `extend=True` | | ✅ | P1 |
| `mesh.select_nth` | `skip=1, nth=1, offset=0` | checker deselect walking from the **active** element. ⚠️ observed: with no active element it still ran and deselected every other vert. Semantics without an active element are unclear, so do not golden-test | ✅ | P2 |
| `mesh.select_edge_loop_multi` | `delimit_edge_loop={'OUTER_CORNERS','NGONS'}` | for each selected edge, select its edge loop (replaces `loop_multi_select(ring=False)`) | ✅ | P0 |
| `mesh.select_edge_ring_multi` | `delimit_edge_ring={'NGONS'}` | for each selected edge, select its ring. Cube edge0 → edges {0,2,7,8} ✅ | ✅ | P0 |
| `mesh.select_boundary_loop_multi` | `extend=False, delimit_edge_loop` | ⚠️ new. Probably selects boundary loops of selected regions | ✅ | P2 |
| `mesh.loop_select` / `mesh.edgering_select` | `delimit_*`, hidden `extend/deselect/toggle/object_index/edge_index/vert_index/face_index` | pick ops. frap: implement with `edge_index` | 🚫H | P1 |
| `mesh.edges_select_sharp` | `sharpness=30°` | select edges whose face angle > sharpness. Cube → all 12 ✅ | ✅ | P1 |
| `mesh.faces_select_linked_flat` | `sharpness=1°` | | ✅ | P2 |
| `mesh.select_similar` | `type` (22 items), `compare`, `threshold` | | ✅ | P2 |
| `mesh.select_interior_faces` | — | | ✅ | P2 |
| `mesh.select_mirror` | `axis={'X'}, extend=False` | | ✅ | P2 |
| `mesh.select_axis` | `orientation='LOCAL', sign='POS', axis='X', threshold=1e-4` | relative to active vert | ✅ | P2 |
| `mesh.select_by_attribute` | — | selects by active BOOLEAN attribute (5.x) | ✅ | P1 (LLM-friendly) |
| `mesh.select_ungrouped` | `extend` | verts without groups | ✅ | P2 |
| `mesh.select_by_pole_count` | ⚠️ (see App. A) | new in 5.x | ✅ | P2 |
| `mesh.loop_to_region` / `region_to_loop` | `select_bigger=False` / — | | ✅ | P2 |
| `mesh.shortest_path_select` | `edge_mode='SELECT', use_face_step, use_topology_distance, use_fill, skip, nth, offset` | between the two last history elements | ✅ | P2 |
| `mesh.hide` / `mesh.reveal` | `unselected=False` / `select=True` | hidden elements are skipped by every op | ✅ | P1 |

**frap extension (namespaced):** selection by index lists and by predicates (`frap.select_elements(type='FACE', indices=[...])`, `frap.select_by_normal(direction=(0,0,1), threshold)`). LLM agents can't click, and Blender itself relies on bmesh scripting for this. Keep them clearly outside `mesh.*`.

### 4.2 Extrude family

All extrude ops are built on bmesh ops: `extrude_face_region(geom, edges_exclude, use_keep_orig, use_normal_flip, use_normal_from_adjacent, use_dissolve_ortho_edges, use_select_history, skip_input_flip)`, `extrude_discrete_faces`, `extrude_edge_only`, `extrude_vert_indiv` ✅ (signatures in App. C).

| bl_idname | composition / props | semantics | sel after (✅) | prio |
|---|---|---|---|---|
| `mesh.extrude_region` | `use_normal_flip=False, use_dissolve_ortho_edges=False, mirror(hidden)` | duplicates the selected region (faces + wire edges + loose verts) and connects its boundary with side quads. Original faces are **removed** (`use_keep_orig=False`). The new cap faces copy the attributes of the originals. Mirror-modifier clipping: edges on the mirror plane go in `edges_exclude` 📖 | new cap verts/edges/faces. History remapped to the cap face | P0 |
| `mesh.extrude_region_move` | macro: `MESH_OT_extrude_region`, `TRANSFORM_OT_translate` | as above, then translate by `value` (GLOBAL by default in exec) | ✅ cube top +Z1: V12 E20 F10, sel F[6] (cap) | **P0** |
| `mesh.extrude_region_shrink_fatten` | `MESH_OT_extrude_region` + `TRANSFORM_OT_shrink_fatten{value, use_even_offset}` | move along vertex normals by `value` ✅ (top at z=1.5 for 0.5) | cap | P0 |
| `mesh.extrude_context` / `extrude_context_move` | `use_normal_flip, use_dissolve_ortho_edges` + translate | context: if faces are selected, region. Else if edges are selected, `extrude_edges_indiv`-like. Else verts 📖. Two adjacent faces → same result as region ✅ | | P0 |
| `mesh.extrude_faces_indiv` / `extrude_faces_move` | + `TRANSFORM_OT_shrink_fatten` | each face extruded separately (`extrude_discrete_faces`). Two adjacent faces → V16 E28 F14 ✅ | new caps (history cleared) ✅ | P1 |
| `mesh.extrude_edges_indiv` / `extrude_edges_move` | `use_normal_flip` + translate | each selected edge → new edge + quad. Edge mode stays ✅ | new edge (history = new edge) ✅ | P1 |
| `mesh.extrude_verts_indiv` / `extrude_vertices_move` | translate | each vert → new vert + wire edge | new vert ✅ | P1 |
| `mesh.extrude_repeat` | `steps=10, offset=(0,0,0), scale_offset=1.0` | repeats region extrude + translate `steps` times. **5.x: offset is a vector (no more view-based `direction`)** ✅. Cube top, steps=3 → V20 F18 | last cap ✅ | P2 |
| `mesh.extrude_manifold` | macro: `MESH_OT_extrude_region` + translate | region extrude with `use_dissolve_ortho_edges=True` 📖 and automerge-and-split. Pushing the cube top **inward** just moves the face (V8) ✅ | moved face | P2 |

**Algorithm notes (region extrude, verified ordering):**
* New verts are created in **original vert iteration order**. Cube top [7,3,1,5] → new 8=dup(1), 9=dup(3), 10=dup(5), 11=dup(7) ✅.
* New edges: cap edges first (in the original face edge order: [8,9],[9,11],[11,10],[10,8]), then the 4 "vertical" edges [3,9],[11,7],[10,5],[1,8] ✅.
* Faces: the cap keeps the loop start of the original face (`[11,9,8,10]` ↔ `[7,3,1,5]`). Side faces are wound so normals point outwards. The original face slot is reused by the first side face (§2.4).
* A single vert extruded through `extrude_region_move` gives a wire edge ✅ (same as `extrude_vertices_move`).
* `use_normal_flip`: flips the side faces (used when extruding against the normal).
* `use_dissolve_ortho_edges`: dissolves side edges that end up in-plane (for coplanar extrudes).

### 4.3 `mesh.inset` ✅

Props: `use_boundary=True, use_even_offset=True, use_relative_offset=False, use_edge_rail=False, thickness=0.0, depth=0.0, use_outset=False, use_select_inset=False, use_individual=False, use_interpolate=True, release_confirm(hidden)`.

* Implemented by `bmesh.ops.inset_region` (or `inset_individual` when `use_individual`).
* **Topology (verified, cube top, thickness 0.2):** the **original face is kept and becomes the inner face**. Its **original verts move inward** (1,3,5,7 → ±0.8). **New verts (8–11) are created at the old corner positions** and the old side faces are re-linked to them. 4 new ring quads are appended (faces 6–9) and 4 new edges connect inner and outer. V12 E20 F10.
* `depth` moves the inner face along the normal (0.3 → z=1.3 ✅). `use_outset` insets outward. `use_boundary` also insets open boundary edges of the region. `use_even_offset` scales the offset by the corner angle (shell factor) to keep a constant width. `use_relative_offset` scales by edge length. `use_edge_rail` slides along existing edges.
* **Sel after:** inner faces (the originals) ✅. With `use_select_inset=True`, the new ring faces instead ✅. History is cleared ✅.
* Region inset of 2 adjacent faces: V14 E24 F12 (shared edge not inset) ✅. Individual: V16 E28 F14 ✅.
* `use_interpolate`: interpolates UVs and other corner data into the new ring faces.

### 4.4 `mesh.bevel` ✅

Props (5.2): `offset_type='OFFSET'` (OFFSET/WIDTH/DEPTH/PERCENT/ABSOLUTE), `offset=0.0` (0..1e6, DISTANCE), `profile_type='SUPERELLIPSE'` (SUPERELLIPSE/CUSTOM), `offset_pct=0` (0..100), `segments=1` (1..1000), `profile=0.5` (0..1, 0.5 = circular arc. 1.0 = convex "square". 0.25 concave), `affect='EDGES'` (VERTICES/EDGES), `clamp_overlap=False`, `loop_slide=True`, `mark_seam=False`, `mark_sharp=False`, `material=-1` (−1 = inherit), `harden_normals=False`, `face_strength_mode='NONE'` (NONE/NEW/AFFECTED/ALL), `miter_outer='SHARP'` (SHARP/PATCH/ARC), `miter_inner='SHARP'` (SHARP/ARC), `spread=0.1`, `vmesh_method='ADJ'` (ADJ/CUTOFF), `release_confirm` (hidden).

* **Default `offset=0` is a no-op**. LLMs must pass `offset`. frap's describe output should flag this.
* `affect='VERTICES'` (was `vertex_only=True` before 2.90).
* Custom profile (`profile_type='CUSTOM'`) uses `scene.tool_settings.custom_bevel_profile_preset` (a CurveProfile) 📖. P2.
* Algorithm: `bmesh.ops.bevel` → `BM_mesh_bevel` (`bmesh_bevel.cc`, about 8k lines). It builds BoundVerts per beveled vertex, computes the offset meets, builds per-edge profile arcs (superellipse with exponent r from `profile` via `PRO_*` mapping), and fills vertex meshes (VMesh: ADJ = Catmull-Clark-like adjacent pattern, CUTOFF = cut-off face). Exact vertex placement for `segments>1` at 3+-valence corners is the hardest part to replicate. Budget real effort, or accept positional tolerance in golden tests.
* **Topology (verified):**
  * Edge 0 of the cube ([2,0]), offset 0.2, seg 1 → V10 E15 F7. Original v0 and v2 are **removed**. Survivors keep their relative order (compacted). The new verts are appended. The new chamfer face is selected ✅.
  * seg 3 → V14 E21 F9, sel 3 faces.
  * all edges seg1 → V24 E48 F26. seg2 → V56 E108 F54 ✅ (golden counts).
  * vertex bevel v0 → V10 E15 F7, new triangle face selected ✅.
* **Sel after:** the new bevel faces (and their verts/edges). History cleared ✅. The select mode is not changed.
* `harden_normals` writes `custom_normal` 📖. `mark_seam/mark_sharp` propagate seams/sharps onto the bevel boundary.

### 4.5 Loop cut, edge rings, subdivide

#### `mesh.loopcut` / `mesh.loopcut_slide` 🚫H (crash in `-b`)
> **Correction (M2 W2, WP 2.6):** the crash happens without a 3D Viewport region. With a `VIEW_3D` `temp_override` (what the golden runner sets for every statement, D-10), Blender 5.2.4 `-b` runs `mesh.loopcut(object_index=…, edge_index=…)` exec without crashing, so frap's goldens call the operator directly; the B.7 recipe is no longer used.

* `loopcut`: `number_cuts=1` (SKIP_SAVE), `smoothness=0` (SKIP_SAVE, −1000..1000), `falloff='INVERSE_SQUARE'`, hidden `object_index=-1, edge_index=-1, mesh_select_mode_init`.
* `loopcut_slide`: macro `MESH_OT_loopcut` + `TRANSFORM_OT_edge_slide{value (−1..1), single_side, use_even, flipped, use_clamp, mirror, correct_uv, …}`.
* Semantics 📖 (`editmesh_loopcut.cc: ringsel_finish`): find the **edge ring** through `edge_index` (BMW_EDGERING walker. Stops at ngons and non-quads), then run `subdivide_edges(edges=ring, cuts=number_cuts, smooth=smoothness, smooth_falloff=falloff, use_smooth_even=True, quad_corner_type=PATH, use_single_edge=True, use_grid_fill=True, use_only_quads=True)` with selection type "loopcut", so **only the new loop edges end up selected**. Then edge_slide by `value`. With `number_cuts>1`, edge slide is skipped 📖⚠️.
* **frap**: implement `mesh.loopcut(edge_index=…)` and `loopcut_slide(...)` headless. The golden harness uses the bmesh emulation in B.7 ✅ (cube, ring of edge 0, cuts=2 → V16 E28 F14, new loops selected).
* LLM-friendlier alternative to document: `select_edge_ring_multi` + `subdivide` (same topology for one cut, but selects everything ✅).

#### `mesh.subdivide` ✅
Props: `number_cuts=1` (1..100, SKIP_SAVE), `smoothness=0`, `ngon=True`, `quadcorner='STRAIGHT_CUT'` (INNERVERT/PATH/STRAIGHT_CUT/FAN), `fractal=0`, `fractal_along_normal=0`, `seed=0`.
* bmesh `subdivide_edges` (`bmo_subdivide.cc`) runs over selected edges with face patterns (quad with 1/2/3/4 cut edges, tri, ngon "all edges cut" → inner grid).
* **Vertex ordering (verified):** original verts unchanged. New edge-cut verts are appended **edge by edge in edge iteration order**, and inside each edge **from `edge.verts[0]` to `edge.verts[1]`**. Then the face-interior verts (grid centres) are appended, one per fully-cut face. Example, whole cube cuts=1: 8–19 = edge midpoints in edge order. 20–25 = face centres in **reversed face order**: 20=(0,0,1) f5, 21=(0,0,−1) f4, 22=(0,−1,0) f3, 23=(1,0,0) f2, 24=(0,1,0) f1, 25=(−1,0,0) f0 ✅. ⚠️ The reversal probably comes from how faces are gathered or popped in `bmo_subdivide.cc`. Golden-test it rather than assume.
* **Neighbours of a partially subdivided region:** with `ngon=True` the neighbour quads become ngons (5-gons) ✅. With `ngon=False` they are triangulated (quad + cut pattern → tris) ✅. With `quadcorner`, the pattern applies to quads with 2 adjacent cut edges.
* **Sel after:** the subdivided region (original selection + new inner geometry). Neighbours keep their selection state. History cleared ✅.
* `smoothness` moves new verts toward a sphere-like interpolation using vertex normals. `fractal` adds random offset with `seed`.

#### `mesh.subdivide_edgering` ✅
`number_cuts=10, interpolation='PATH'` (LINEAR/PATH/SURFACE), `smoothness=1.0, profile_shape_factor=0, profile_shape='SMOOTH'`. Needs ≥2 selected edge loops. Bridges between them with rings. Sel after: original edges + new loop edges (ring + edgering on cube → E sel [0,2,7,8,12–15]) ✅. P2.

#### `mesh.unsubdivide` ✅
`iterations=2`. Reverses grid subdivision (cube subdivided → back to V8 F6). P2.

#### `mesh.offset_edge_loops` / `offset_edge_loops_slide` 📖
`use_cap_endpoint=False`. Edge-loop "double" cut either side of the selected loop. Macro + edge_slide. P2.

### 4.6 Merge / remove doubles

#### `mesh.merge` ✅
`type='CENTER'` (CENTER/CURSOR/COLLAPSE/FIRST/LAST, dynamic: FIRST/LAST only offered when history has a vert at that end), `uvs=False` (UV correction).
Positions on the cube with sel {v0,v1,v3} (verified):
* CENTER = mean of the selected verts = (−1, −⅓, ⅓)
* CURSOR = `scene.cursor.location` (in object space)
* COLLAPSE = per connected island, `bmesh.ops.collapse` over the selected edges. The centre is the **mean over edge endpoints** (weighted by edges): (−1, −0.5, 0.5) ✅. Several islands collapse separately.
* FIRST/LAST = position of the first/last history vert.
* Result: survivors renumber compactly. The **merged vert takes the lowest index of the merged set** for CENTER/CURSOR/COLLAPSE/FIRST (index 0) ✅, and the LAST vert's own slot for LAST (idx 1 here, since v0 was removed) ✅. Sel after: the merged vert, which is also in history ✅.

#### `mesh.remove_doubles` ("Merge by Distance") ✅
`threshold=1e-4` (0..10), `use_centroid=True` (merge to the centroid of the cluster instead of the first vert), `use_unselected=False` (also merge into unselected verts), `use_sharp_edge_from_normals=False` (mark sharp edges where custom normals differ). Report "Removed N vertice(s)" (Info). frap: return `{"removed": N}` in its JSON. Algorithm: `find_doubles` (KD-tree sorted along axis) + `weld_verts` 📖. Cluster targets are chosen greedily: no transitive chaining beyond the target's radius ⚠️ (huge threshold on a cube left 2 verts, not 1 ✅).

### 4.7 Delete / dissolve

| bl_idname | props | semantics (✅ cube, top face selected) | prio |
|---|---|---|---|
| `mesh.delete` | `type='VERT'` (VERT/EDGE/FACE/EDGE_FACE/ONLY_FACE), HIDDEN+SKIP_SAVE | VERT: remove selected verts and everything using them (V4 E4 F1). EDGE: remove selected edges + faces using them, and verts left unused (V8 E8 F1, as observed. ⚠️ the verts stayed because they are still used by other edges). FACE: remove faces, then edges/verts no longer used by unselected geometry (top → V8 E12 F5, since all edges are shared). EDGE_FACE: edges + faces, keep verts. ONLY_FACE: faces only. Sel after: nothing ✅ | P0 |
| `mesh.delete_loose` | `use_verts=True, use_edges=True, use_faces=False` | | P2 |
| `mesh.delete_edgeloop` | `use_face_split=True` | dissolve an edge loop and fix the end verts | P2 |
| `mesh.dissolve_verts` | `use_face_split=False, use_boundary_tear=False` | v0 → V7 E9 F4, merged into one hexagon ✅ | P1 |
| `mesh.dissolve_edges` | `use_verts=True, angle_threshold=π, use_face_split=False, use_preserve_quads=True` | e0 → V6 E9 F5 (two-edge verts then dissolved, producing tris) ✅ | P1 |
| `mesh.dissolve_faces` | `use_verts=False` | merges adjacent selected faces into one ngon. Single face = no-op ✅. Two adjacent → one hexagon, F5 ✅ | P1 |
| `mesh.dissolve_mode` | `use_verts, angle_threshold, use_preserve_quads, use_face_split, use_boundary_tear` | dispatches to verts/edges/faces by select mode (the X → Dissolve key) | P1 |
| `mesh.dissolve_limited` | `angle_limit=5°` (0..180°), `use_dissolve_boundaries=False`, `delimit={'NORMAL'}` | planar decimation. Subdivided cube → back to V8 E12 F6, with a **different face order** ✅ | P1 |
| `mesh.dissolve_degenerate` | `threshold=1e-4` | | P2 |
| `mesh.edge_collapse` | — | collapse each selected edge (island) to its midpoint. e0 → V7 ✅ | P2 |

### 4.8 Bridge / fill / connect

| bl_idname | props | semantics | prio |
|---|---|---|---|
| `mesh.bridge_edge_loops` | `type='SINGLE'` (SINGLE/CLOSED/PAIRS), `use_merge=False, merge_factor=0.5, twist_offset=0, number_cuts=0, interpolation='PATH', smoothness=1.0, profile_shape_factor=0, profile_shape='SMOOTH'` | bridges 2+ selected edge loops (`bmesh.ops.bridge_loops` + optional `subdivide_edgering`). If whole faces are selected, they are deleted first (bridge between faces) 📖. Two squares → 4 side faces ✅. cuts=2 → V16 F12 ✅. Sel after: everything bridged | P1 |
| `mesh.fill` | `use_beauty=True` | triangle fill (`triangle_fill`, scanfill + beautify) of closed selected edge loops. 8-gon circle → 6 tris ✅. Hole in a cube → 2 tris ✅ | P1 |
| `mesh.fill_grid` | `span=1, offset=0, use_interp_simple=False` | grid fill of one closed loop (or two opposite rails). Circle(8) span 2 → 4 quads, 1 centre vert ✅. Fails with "Connecting edge loops overlap" when the selection isn't a simple loop ✅ | P1 |
| `mesh.fill_holes` | `sides=4` (0 = any) | fill boundary holes up to N sides | P2 |
| `mesh.beautify_fill` | `angle_limit=π` | rotate triangle edges for better shape | P2 |
| `mesh.edge_face_add` ("F") | — | contextual create: 2 verts → edge. 3+ verts/edges forming a loop → ngon. Edge net → faces. Cube hole → 1 quad, Circle(8) → 1 ngon ✅. Sel after: new face (+ history of input edges) ✅ | **P0** |
| `mesh.vert_connect` / `vert_connect_path` / `vert_connect_concave` / `vert_connect_nonplanar` | — / — / — / `angle_limit` | J (connect verts by splitting faces) | P1 (vert_connect_path) |
| `mesh.face_split_by_edges` | — | weld loose edges into faces | P2 |
| `mesh.edge_rotate` | `use_ccw=False` | | P2 |
| `mesh.bisect` | `plane_co, plane_no, use_fill, clear_inner, clear_outer, threshold` (+ hidden xstart/ystart…) | the plane must be given explicitly headless | P2 |
| `mesh.convex_hull` | see App. A | | P2 |

### 4.9 Normals and shading

| bl_idname | props | semantics | prio |
|---|---|---|---|
| `mesh.flip_normals` | `only_clnors=False` | reverses winding of the selected faces. **Loop order changes from [7,3,1,5] to [7,5,1,3]**, keeping the first vert ✅ (BM_face_normal_flip reverses the loop cycle around `l_first`). Custom normals are flipped too | P0 |
| `mesh.normals_make_consistent` | `inside=False` | `recalc_face_normals`: per connected island, finds the face farthest from the centre (or similar) to orient outward, then flood-fills the winding ✅ (restores [7,3,1,5]) | P0 |
| `mesh.set_normals_from_faces` | `keep_sharp=False` | sets `custom_normal` from face normals ✅ | P1 |
| `mesh.faces_shade_smooth` / `faces_shade_flat` | — | writes `sharp_face` for selected faces. All smooth → the attribute is removed ✅ | P0 |
| `mesh.set_sharpness_by_angle` | `angle=30°, extend=False` | writes `sharp_edge` for selected edges (extend: only add sharp). Adds `custom_normal` ⚠️ | P1 |
| `mesh.mark_sharp` | `clear=False, use_verts=False` | `sharp_edge` on selected edges (use_verts: edges of selected verts) ✅ | P0 |
| `mesh.customdata_custom_splitnormals_add` / `_clear` | — | add or remove `custom_normal` (zeros = "use auto normal") ✅ | P1 |
| `mesh.normals_tools` | `mode` (COPY/PASTE/ADD/MULTIPLY/RESET), `absolute` | custom normal editing | P2 |
| `mesh.smooth_normals`, `average_normals`, `point_normals`, `merge_normals`, `split_normals`, `mod_weighted_strength` | see App. A | custom normal tools | P2 |
| `object.shade_smooth(keep_sharp_edges=True)` / `shade_flat(keep_sharp_edges=True)` | | object mode, whole mesh. `shade_flat` writes `sharp_face`=True and, if `keep_sharp_edges=False`, clears `sharp_edge` 📖 | P0 |
| `object.shade_smooth_by_angle(angle=30°, keep_sharp_edges=True)` | | destructive. Writes `sharp_edge`, removes `sharp_face` ✅ | P0 |
| `object.shade_auto_smooth(use_auto_smooth=True, angle=30°)` | | adds the "Smooth by Angle" GN modifier ✅ | P0 |

### 4.10 Marking and attribute layers

| bl_idname | props | attribute written ✅ | prio |
|---|---|---|---|
| `mesh.mark_seam` | `clear=False` | `uv_seam` (EDGE, BOOLEAN). Removed when all are cleared | P0 |
| `uv.mark_seam` | `clear=False` | same, from UV selection (in sync mode = mesh selection) ✅ | P1 |
| `transform.edge_crease` | `value` (−1..1, additive, clamped 0..1) | `crease_edge` | P1 (SubSurf) |
| `transform.edge_bevelweight` | `value` | `bevel_weight_edge` | P1 (Bevel modifier, limit_method WEIGHT) |
| `transform.vert_crease` 📖 | `value` | `crease_vert` | P2 |
| `mesh.mark_freestyle_edge` / `_face` | `clear` | `freestyle_edge` / `freestyle_face` 📖 | skip |
| `mesh.attribute_set` | `value_float, value_float_vector_2d/3d/4d, value_int, value_int_vector_2d, value_color, value_bool` | sets the **active attribute** on selected elements of its domain | P1 (great for LLMs, e.g. vertex colours) |
| `mesh.uv_texture_add` / `uv_texture_remove` | — | adds `UVMap.001` (copy of active) and makes it active ✅ / removes the active one | P0 |
| `geometry.attribute_add` | `name, domain='POINT', data_type='FLOAT'` (+ hidden value props 📖) | | P1 |
| `geometry.color_attribute_add` | `name='', domain='POINT'` (POINT/CORNER), `data_type='FLOAT_COLOR'` (FLOAT_COLOR/BYTE_COLOR), `color=(0,0,0,1)` | empty name → "Attribute". Becomes active + default ✅ | P0 (glTF COLOR_0) |
| `geometry.color_attribute_remove`, `color_attribute_render_set`, `attribute_remove`, `attribute_convert` 📖 | | | P1 |
| `mesh.colors_rotate/reverse`, `mesh.uvs_rotate/reverse` | `use_ccw` / — | rotate per-face corner data | P2 |
| `mesh.sort_elements` | `type, elements, reverse, seed` | reorders elements (affects glTF index order) | P2 |
| `mesh.reorder_vertices_spatial` | — | new in 5.x: spatial reorder for cache locality ⚠️ | P2 |

### 4.11 Split / separate / duplicate / rip

| bl_idname | props | semantics ✅ | prio |
|---|---|---|---|
| `mesh.split` | — | detaches the selected faces (new verts/edges for them). The face itself is re-linked, keeping its slot (top stays f5 with new verts 8–11). Sel after: the split geometry ✅ | P1 |
| `mesh.edge_split` | `type='EDGE'` (EDGE/VERT) | splits along selected edges (rips seams). On a closed cube, a single edge split gives no new verts, because the verts are still fan-connected through other faces ✅. Splitting a full loop separates the mesh | P1 |
| `mesh.separate` | `type='SELECTED'` (SELECTED/MATERIAL/LOOSE) | moves the selection into a **new object** `<name>.001` sharing materials. The original object stays active and in edit mode. The new object is selected but **not** in edit mode 📖⚠️ (observed: both selected, active = original) ✅ | P0 |
| `mesh.duplicate` | `mode` (hidden) | duplicate the selection (no transform) | P1 |
| `mesh.duplicate_move` | macro: `MESH_OT_duplicate` + `TRANSFORM_OT_translate` | new geometry appended and selected. Originals deselected ✅ | P0 |
| `mesh.rip` / `rip_move` / `rip_edge` / `rip_edge_move` | proportional props, `use_fill` | **poll fails headless** (needs region for the mouse direction) 🚫H. frap: P2. If implemented, take an explicit direction | P2 |

### 4.12 Spin / screw / solidify / wireframe

| bl_idname | props | semantics | prio |
|---|---|---|---|
| `mesh.spin` | `steps=12` (0..1e6), `dupli=False`, `angle=90°`, `use_auto_merge=True`, `use_normal_flip=False`, `center=(0,0,0)`, `axis=(0,0,0)` | lathe: extrudes and rotates the selection `steps` times around `axis` through `center` (object-space conversion of the given world values 📖). **axis is required headless** ("Invalid/unset axis") ✅. Plane spun 4×90° → V20 F18. Sel after: the last ring ✅. `use_auto_merge` merges the first and last ring when the angle is 360° | P1 |
| `mesh.screw` | `steps=9, turns=1, center, axis` | needs an **open** chain of connected verts (a closed loop errors) ✅. 3-vert chain, 8 steps → V32 E48 F16 ✅. The axis offset comes from the chain's end-point delta (helix) 📖 | P2 |
| `mesh.solidify` | `thickness=0.01` | edit-mode solidify (`bmesh.ops.solidify`). Cube all → V16 F12 (inner shell, flipped) and the new shell is selected ✅. (Different from the SOLIDIFY modifier) | P2 |
| `mesh.wireframe` | `use_boundary=True, use_even_offset=True, use_relative_offset=False, use_replace=True, thickness=0.01, offset=0.01, use_crease=False, crease_weight=0.01` | cube → V40 E96 F48 ✅ | P2 |

### 4.13 Triangulate / quads / poke / decimate / boolean

| bl_idname | props | semantics ✅ | prio |
|---|---|---|---|
| `mesh.quads_convert_to_tris` | `quad_method='BEAUTY'` (BEAUTY/FIXED/FIXED_ALTERNATE/SHORTEST_DIAGONAL/LONGEST_DIAGONAL), `ngon_method='BEAUTY'` (BEAUTY/CLIP) | each quad → 2 tris. **The first tri keeps the face slot, the second is appended after all originals** (cube: f0 [1,2,0] … f6 [1,3,2]) ✅. BEAUTY on a square quad picks the diagonal by area/angle, and ties are broken deterministically (cube f0 [0,1,3,2] → [1,2,0] + [1,3,2], i.e. diagonal 1–2 = loop corners 1 and 3, see App. B.4). FIXED = split v0–v2. FIXED_ALTERNATE = v1–v3 | P1 |
| `mesh.tris_convert_to_quads` | `face_threshold=40°, shape_threshold=40°, topology_influence=0, uvs, vcols, seam, sharp, materials` (compare flags, default False), `deselect_joined=False` | `join_triangles`. Rejoined cube → 6 quads, but **winding start and face order differ** from the original ✅ | P1 |
| `mesh.poke` | `offset=0, use_relative_offset=False, center_mode='MEDIAN_WEIGHTED'` (MEDIAN_WEIGHTED/MEDIAN/BOUNDS) | fan-triangulate around a new centre vert. Cube top → centre v8, tris [7,3,8],[3,1,8],[1,5,8],[5,7,8]. The first tri reuses the face slot ✅ | P2 |
| `mesh.flip_quad_tessellation` | — | | P2 |
| `mesh.decimate` | `ratio=1.0, use_vertex_group, vertex_group_factor, invert_vertex_group, use_symmetry, symmetry_axis='Y'` | edge-collapse decimation (quadric) | P2 (the DECIMATE modifier is more important) |
| `mesh.intersect` / `intersect_boolean` | `mode/separate_mode/threshold/solver='EXACT'` (FLOAT/EXACT. **5.x renamed FAST → FLOAT** ✅), `operation='DIFFERENCE', use_swap, use_self` | selected vs unselected geometry. EXACT = mesh-arrangements solver (hard). Share the implementation with the BOOLEAN modifier | P2 |
| `mesh.knife_project` | `cut_through=False` | **view-dependent**. Skip 🚫H | skip |

### 4.14 Deform / symmetry

| bl_idname | props | semantics | prio |
|---|---|---|---|
| `mesh.vertices_smooth` | `factor=0.0` (**default 0 = no-op**), `repeat=1, xaxis=yaxis=zaxis=True` | Laplacian-style average of neighbour verts via **edges** (`smooth_vert`). Cube factor 0.5 → ±0.6667 ✅ (v' = v·(1−f) + f·mean(neighbours via edges)). Mirror-modifier clipping respected 📖 | P1 |
| `mesh.vertices_smooth_laplacian` | `repeat, lambda_factor=1.0, lambda_border=5e-5, use_x/y/z, preserve_volume=True` | | P2 |
| `mesh.symmetrize` | `direction='NEGATIVE_X'` (±X/±Y/±Z), `threshold=1e-4` | bisects at the plane, mirrors one side, and welds. Monkey +X → V507 F500 ✅ | P1 |
| `mesh.symmetry_snap` | `direction, threshold=0.05, factor=0.5, use_center=True, use_topology=False` | | P2 |
| `mesh.face_make_planar` | `factor=1, repeat=1` | | P2 |
| `mesh.circularize` / `mesh.flatten` / `mesh.space_edge_loops_evenly` | new 5.x (LoopTools in core) | see App. A | P2 |
| `mesh.blend_from_shape` / `shape_propagate_to_all` | shape-key tools | | P2 (shape keys → morph targets) |
| `transform.translate/rotate/resize/shrink_fatten/edge_slide/vert_slide` | App. A | in edit mode they act on selected verts. Pivot = `tool_settings.transform_pivot_point` (default MEDIAN_POINT) 📖. `orient_type='NORMAL'` uses the selection normal frame. Proportional editing via `use_proportional_edit` | P0 (translate/rotate/resize), P1 (shrink_fatten, edge_slide) |

### 4.15 UV operators

Shared: they act on the **active UV map** (`mesh.uv_layers.active`), on **selected faces** (mesh selection in edit mode. With sync select on (5.x default) the UV selection follows the mesh selection). Most of them finish with a pack step (`margin`, `margin_method`). Selection is unchanged. If the mesh has no UV map, one is created ✅ (📖 `ED_uvedit_ensure`).

| bl_idname | key props (defaults) ✅ | semantics / notes | headless | prio |
|---|---|---|---|---|
| `uv.unwrap` | `method='CONFORMAL'` (ANGLE_BASED/CONFORMAL/MINIMUM_STRETCH), `fill_holes=False, correct_aspect=True, use_subsurf_data=False, use_original_bounds=False, margin_method='SCALED'` (SCALED/ADD/FRACTION), `margin=0.001, no_flip=False, iterations=10, use_weights=False, weight_group='uv_importance', weight_factor=1.0` | seam-based island split → per-island parametrisation: ABF++ (ANGLE_BASED), LSCM (CONFORMAL), SLIM (MINIMUM_STRETCH, `iterations`, `no_flip`, weights) → pack into 0..1 with margin. **Cube with the standard 7 seams: ANGLE_BASED and CONFORMAL give identical output, a cross layout of 0.25 squares** ✅. **Without seams on a closed cube: warning "Unwrap failed to solve 1 of 1 island(s)", and the old UVs are just repacked** ✅. Pinned UVs (`.pn.*`) are constraints. P0, but a big task: LSCM is needed first, ABF++ next, SLIM is P2 | ✅ | **P0** |
| `uv.smart_project` | `angle_limit=66°` (1.15192 rad), `margin_method='SCALED'`, `rotate_method='AXIS_ALIGNED_Y'` (AXIS_ALIGNED/AXIS_ALIGNED_X/AXIS_ALIGNED_Y), `island_margin=0.0, area_weight=0.0, correct_aspect=True, scale_to_bounds=False` | groups faces by normal clusters (angle_limit), projects each group orthographically along its normal, rotates per `rotate_method`, then packs (`pack_islands` with rotation). Cube → 6 islands in a 3×2 grid of ⅓ squares ✅ (exact layout in B.5). Implementation: C++ `uvedit_unwrap_ops.cc: smart_project_exec` 📖 | ✅ | **P0** |
| `uv.cube_project` | `cube_size=1.0, correct_aspect=True, clip_to_bounds=False, scale_to_bounds=False` | per face, project along the dominant axis of its normal. uv = (co_axis1, co_axis2)·(1/cube_size)·0.5 + 0.5 offset relative to the object origin 📖. Cube → each face 0..1 ✅. Origin: `cube_size` and the object bounds centre ⚠️, golden-test it | ✅ | P1 |
| `uv.cylinder_project` | `direction='VIEW_ON_EQUATOR'` (VIEW_ON_EQUATOR/VIEW_ON_POLES/ALIGN_TO_OBJECT), `align='POLAR_ZX'` (POLAR_ZX/POLAR_ZY), `pole='PINCH'` (PINCH/FAN), `seam=False, radius=1.0, correct_aspect, clip_to_bounds, scale_to_bounds` | headless there's no view, so VIEW_* falls back to an identity view matrix 📖⚠️. Cube result u ∈ [0.125, 1.125] (the seam wrap creates u>1) ✅. frap: default `direction='ALIGN_TO_OBJECT'` semantics when no view, **but check equality with Blender's headless output (B.5)** | ✅ | P1 |
| `uv.sphere_project` | same minus radius | cube → v ∈ [0.3041, 0.6959] ✅ | ✅ | P1 |
| `uv.project_from_view` | `orthographic, camera_bounds, correct_aspect, clip_to_bounds, scale_to_bounds` | needs a 3D view (poll fails) 🚫H. frap: could project from the scene camera (`camera_bounds=True`) as a documented extension, or skip | 🚫H | skip/P2 |
| `uv.reset` | — | each face gets the default unit layout: quads → (0,0)(1,0)(1,1)(0,1) per corner from `l_first` ✅. Ngons: points on a circle 📖 | ✅ | P1 |
| `uv.follow_active_quads` | `mode='LENGTH_AVERAGE'` (EVEN/LENGTH/LENGTH_AVERAGE) | needs an active quad face ("No active face" otherwise) ✅ | ✅ (with active face) | P2 |
| `uv.lightmap_pack` | `PREF_CONTEXT='SEL_FACES'` (SEL_FACES/ALL_FACES), `PREF_PACK_IN_ONE=True, PREF_NEW_UVLAYER=False, PREF_BOX_DIV=12, PREF_MARGIN_DIV=0.1` | Python add-on op (`bl_operators/uvcalc_lightmap.py`). Upper-case prop names are real | ✅ | P2 |
| `uv.pack_islands` | `udim_source='CLOSEST_UDIM'` (CLOSEST_UDIM/ACTIVE_UDIM/ORIGINAL_AABB/CUSTOM_REGION), `rotate=True, rotate_method='ANY'` (ANY/CARDINAL/AXIS_ALIGNED/AXIS_ALIGNED_X/AXIS_ALIGNED_Y), `scale=True, merge_overlap=False, margin_method='SCALED', margin=0.001, pin=False, pin_method='LOCKED'` (SCALE/ROTATION/ROTATION_SCALE/LOCKED), `shape_method='CONCAVE'` (CONCAVE/CONVEX/AABB) | acts on **UV-selected** islands. Packer: `uv_pack.cc` (xatlas-like + alpaca/box packers 📖). Exact layout parity is unlikely. Golden tests should check invariants: all UVs in [0,1], no overlaps, area ratios preserved ✅ (B.5 smart+pack) | ✅ | P0 (invariants) |
| `uv.average_islands_scale` | `scale_uv=False, shear=False` | rescales islands so UV area ∝ 3D area | ✅ | P1 |
| `uv.minimize_stretch` | `fill_holes=True, blend=0, iterations=0` | modal. `iterations=0` = interactive. Use `iterations>0` headless ⚠️ | ✅? | P2 |
| `uv.seams_from_islands` | `mark_seams=True, mark_sharp=False` | sets `uv_seam` on edges whose corners differ across faces. Smart-projected cube → all 12 edges ✅ | ✅ | P1 |
| `uv.pin` | `clear=False, invert=False` | `.pn.<uvmap>` corner bool | ✅ | P2 |
| `uv.select_all/select_mode/select_linked/select_more/less/select_split/select_overlap/select_similar/select_pinned/select_by_winding/hide/reveal` | App. A | UV-space selection. Only meaningful when sync is off. P2 | ✅ | P2 |
| `uv.weld/remove_doubles/stitch/align/align_rotation/arrange_islands/randomize_uv_transform/copy_mirrored_faces/snap_selected/rip` | App. A | | ✅ mostly | P2 |

**Correction (M3+M4 W1, verified by WP 3.3 against 5.2.4):** a bare `uv.unwrap()` without `method`/`fill_holes` takes them from the scene tool settings (`toolsettings->unwrapper` and the `UVCALC_FILLHOLES` flag; 5.2 factory: `ANGLE_BASED` and `fill_holes=True`), not the RNA defaults `CONFORMAL`/`False` listed above; `uv.smart_project` writes `island_margin` into `tool_settings.uvcalc_margin`. WP 3.8 replicates both.

**UV algorithm notes:**
* Islands are connected components of faces over edges that are **not seams** *and* whose two corners share UVs (for repacking). For unwrap, only seams and mesh boundaries split islands.
* Unwrap pipeline (`uvedit_unwrap_ops.cc` → `GEO_uv_parametrizer_*`): build a ParamHandle (faces triangulated internally, seams as constraints, pins), solve (ABF++ then LSCM, LSCM alone, or SLIM), `param_average`, then `param_pack` (margin + rotation off for unwrap by default 📖). frap needs a sparse least-squares solver (LSCM) as P0. For ABF++, use port references (OpenNL usage in Blender).
* `correct_aspect` uses the aspect of the active image texture in the material's node tree (Image Texture node) 📖. Without an image, aspect = 1.

### 4.16 Not implementing (interactive-only or out of scope)

`mesh.knife_tool`, `knife_project`, `polybuild_*`, `dupli_extrude_cursor`, `primitive_cube_add_gizmo`, `uv.select/select_box/select_circle/select_lasso/select_loop/select_edge_ring` (pick ops with mouse `location`), `uv.cursor_set`, `uv.custom_region_set`, `uv.export_layout` (P2 maybe), `uv.copy/paste` (clipboard), `mesh.mark_freestyle_*`, `customdata_mask_clear`, `customdata_skin_*`, `customdata_face_sets_clear` (sculpt), `mesh.shortest_path_pick` (pick, has an index-based equivalent).

---

## 5. Recommendations for frap's architecture

1. **Core mesh = SoA attribute store** mirroring §1.2 (names, types, domains, `.`-prefixed internals). The RNA-like property paths (`mesh.attributes['UVMap'].data[3].vector`, `mesh.polygons[2].use_smooth`, `mesh.edges[0].use_seam`, `mesh.uv_layers.active.data[i].uv`) are views over it. `use_smooth`/`use_seam`/`use_edge_sharp` are proxies that create or remove the attributes lazily, as in Blender.
2. **Edit mode = BMesh-like half-edge structure** (disk + radial cycles, as in Blender: one loop per face corner, radial lists for non-manifold). Store elements in **slot arenas with a LIFO free list** (§2.4) so index order can match Blender. Keep an `ensure_index()` lazy step.
3. **Operator layer** = typed registry generated from a single spec table (bl_idname, props with type/default/range/enum/flags HIDDEN/SKIP_SAVE, poll requirement {OBJECT, EDIT_MESH, needs_active_face, needs_selection}). Appendix A can be machine-converted into that spec, and the dump script (Appendix D) can regenerate it from any Blender version. **Generate the Rust spec from the Blender dump JSON** rather than by hand.
4. **Macros**: a `Macro { steps: [(sub_idname, props)] }` form. Python front-end: `NAME_OT_sub={...}` kwargs. Short form: `TRANSFORM_OT_translate.value=0,0,1`.
5. **Headless deviations**: for 🚫H ops frap *does more* than Blender (`loopcut` with `edge_index`, `spin` with a required axis). Every deviation is documented in `--describe` as `frap_note`, and golden tests use the equivalent bmesh sequence (Appendix B.7).
6. **JSON response per op** should include `{"result": "FINISHED"|"CANCELLED", "reports": [{type:"INFO",message:"Removed 6 vertices"}], "mesh": {"verts":…, "edges":…, "faces":…, "selected": {...}}}`. LLMs benefit from the counts after each edit-mode op.
7. **Golden-test strategy** (from §0.7): canonical comparison by default. Exact-order comparison only for the ops verified as stable here: primitives, subdivide (uniform), extrude_region (+_move), extrude_edges/verts, inset (single), split, poke, quads_convert_to_tris (FIXED), flip_normals, merge, delete.
8. **P0 implementation order:** select_all/select_mode/select_linked/more/less, delete, extrude_region_move, inset, subdivide, loopcut (headless), bevel (seg 1, then N), merge/remove_doubles, edge_face_add, flip/normals_make_consistent, mark_seam/sharp, faces_shade_*, duplicate_move, separate, uv.smart_project, uv.unwrap(CONFORMAL), uv.pack_islands, uv_texture_add, color_attribute_add, transform.*.

---

## 6. Uncertainties / to re-verify

* ⚠️ Face-centre vert order in uniform `subdivide` (observed reversed-face-order pattern: top, bottom, −Y, +X, +Y, −X). Derive it from `bmo_subdivide.cc` (it iterates faces through a gathered array, possibly in reverse) before claiming index parity.
* ⚠️ `set_sharpness_by_angle` creating `custom_normal`: observed but not explained.
* ⚠️ `select_nth` behaviour without an active element.
* ⚠️ `cylinder_project`/`sphere_project` headless "view" fallback: frap must copy whatever Blender does in `-b` (identity view → equator along X/Z). Verify with a non-cube mesh.
* ⚠️ `remove_doubles` clustering details (greedy, non-transitive).
* ⚠️ `mesh.separate`: which object ends up active, and the edit-mode state of the new object (observed: original active, both selected).
* ⚠️ Vertex-group name storage (`Mesh.vertex_group_names` vs `Object.vertex_groups`) in 5.2. Not checked here (belongs to the skinning research).
* ⚠️ `mesh.select_boundary_loop_multi`, `select_by_pole_count`, `reorder_vertices_spatial`: new ops, semantics only inferred from names.
* ⚠️ Bevel `segments>1` corner VMesh positions: expect float drift. Use tolerance 1e-4.
* ⚠️ `uv.unwrap` LSCM pin choice (which two verts get pinned per island affects the orientation and scale before packing). Blender picks the two verts farthest apart along the bounding-box diagonal 📖. Needs verification for exact parity.

## Appendix A — Full operator property tables (machine-dumped from Blender 5.2.4)

Generated by introspecting `bpy.ops.<mod>.<op>.get_rna_type().properties` in `blender -b --factory-startup`. `HIDDEN` = `PROP_HIDDEN` (not shown in UI, but **still settable from Python and often essential** — e.g. `mesh.select_all.action`, `mesh.delete.type`). `SKIP_SAVE` = not remembered between invocations (frap must reset to default each call). Float subtype/unit in brackets. Ranges are hard limits. For `transform.*`, purely interactive hidden props (snap*, gpencil_strokes, release_confirm, orient_matrix…) are omitted. Enums with empty item lists in the dump (`orient_type` on rotate/resize) are dynamic (itemf) — items are GLOBAL, LOCAL, NORMAL, GIMBAL, VIEW, CURSOR, PARENT (+ custom orientations).


### A.1 Selection (mesh)

`bpy.ops.mesh.select_all` — **(De)select All** — (De)select all vertices, edges or faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `action` | ENUM | 'TOGGLE' | TOGGLE, SELECT, DESELECT, INVERT | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.select_mode` — **Select Mode** — Change selection mode  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_extend` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `use_expand` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `type` | ENUM | 'VERT' | VERT, EDGE, FACE | HIDDEN SKIP_SAVE |
| `action` | ENUM | 'TOGGLE' | DISABLE, ENABLE, TOGGLE | HIDDEN |


`bpy.ops.mesh.select_random` — **Select Random** — Randomly select vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `ratio` | FLOAT[FACTOR] | 0.5 | 0 .. 1 |  |
| `seed` | INT | 0 | 0 .. ∞ |  |
| `action` | ENUM | 'SELECT' | SELECT, DESELECT |  |


`bpy.ops.mesh.select_linked` — **Select Linked All** — Select all vertices connected to the current selection  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit` | ENUM | {SEAM} | FLAG: NORMAL, MATERIAL, SEAM, SHARP, UV | SKIP_SAVE |


`bpy.ops.mesh.select_linked_pick` — **Select Linked** — (De)select all vertices linked to the edge under the mouse cursor  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `deselect` | BOOLEAN | False |  |  |
| `delimit` | ENUM | {SEAM} | FLAG: NORMAL, MATERIAL, SEAM, SHARP, UV | SKIP_SAVE |
| `object_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.select_more` — **Select More** — Select more vertices, edges or faces connected to initial selection  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_face_step` | BOOLEAN | True |  |  |


`bpy.ops.mesh.select_less` — **Select Less** — Deselect vertices, edges or faces at the boundary of each selection region  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_face_step` | BOOLEAN | True |  |  |


`bpy.ops.mesh.select_non_manifold` — **Select Non-Manifold** — Select all non-manifold vertices or edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `extend` | BOOLEAN | True |  |  |
| `use_wire` | BOOLEAN | True |  |  |
| `use_boundary` | BOOLEAN | True |  |  |
| `use_multi_face` | BOOLEAN | True |  |  |
| `use_non_contiguous` | BOOLEAN | True |  |  |
| `use_verts` | BOOLEAN | True |  |  |


`bpy.ops.mesh.select_loose` — **Select Loose Geometry** — Select loose geometry based on the selection mode  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.mesh.select_face_by_sides` — **Select Faces by Sides** — Select vertices or faces by the number of face sides  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `number` | INT | 4 | 3 .. ∞ |  |
| `type` | ENUM | 'EQUAL' | LESS, EQUAL, GREATER, NOTEQUAL |  |
| `extend` | BOOLEAN | True |  |  |


`bpy.ops.mesh.select_nth` — **Checker Deselect** — Deselect every Nth element starting from the active vertex, edge or face  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `skip` | INT | 1 | 1 .. ∞ |  |
| `nth` | INT | 1 | 1 .. ∞ |  |
| `offset` | INT | 0 | -∞ .. ∞ |  |


`bpy.ops.mesh.select_edge_loop_multi` — **Multi Select Edge Loops** — Select loops of connected edges from each selected edge  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit_edge_loop` | ENUM | {OUTER_CORNERS, NGONS} | FLAG: SEAM, SHARP, NGONS, INNER_CORNERS, OUTER_CORNERS |  |


`bpy.ops.mesh.select_edge_ring_multi` — **Multi Select Edge Rings** — Select rings of connected edges from each selected edge  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit_edge_ring` | ENUM | {NGONS} | FLAG: SEAM, SHARP, MATERIAL, NGONS |  |


`bpy.ops.mesh.select_boundary_loop_multi` — **Multi Select Boundary Loops** — Select entire boundary loop of each selected boundary edge  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `extend` | BOOLEAN | True |  |  |
| `delimit_edge_loop` | ENUM | {} | FLAG: SEAM, SHARP, NGONS, INNER_CORNERS, OUTER_CORNERS |  |


`bpy.ops.mesh.loop_select` — **Loop Select** — Select a loop of connected edges  
flags: UNDO, DEPENDS_ON_CURSOR, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit_edge_loop` | ENUM | {OUTER_CORNERS, NGONS} | FLAG: SEAM, SHARP, NGONS, INNER_CORNERS, OUTER_CORNERS |  |
| `delimit_face_loop` | ENUM | {} | FLAG: SEAM, SHARP, MATERIAL |  |
| `extend` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `deselect` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `toggle` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `object_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `edge_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `vert_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `face_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.edgering_select` — **Edge Ring Select** — Select an edge ring  
flags: UNDO, DEPENDS_ON_CURSOR, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit_edge_ring` | ENUM | {NGONS} | FLAG: SEAM, SHARP, MATERIAL, NGONS |  |
| `extend` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `deselect` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `toggle` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `object_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `edge_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `vert_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |
| `face_index` | INT | -1 | -1 .. ∞ | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.edges_select_sharp` — **Select Sharp Edges** — Select all sharp enough edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `sharpness` | FLOAT[ANGLE,ROTATION] | 0.523599 | 0.000174533 .. 3.14159 |  |


`bpy.ops.mesh.faces_select_linked_flat` — **Select Linked Flat Faces** — Select linked faces by angle  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `sharpness` | FLOAT[ANGLE,ROTATION] | 0.0174533 | 0.000174533 .. 3.14159 |  |


`bpy.ops.mesh.select_similar` — **Select Similar** — Select similar vertices, edges or faces by property types  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'VERT_NORMAL' | VERT_NORMAL, VERT_FACES, VERT_GROUPS, VERT_EDGES, VERT_CREASE, EDGE_LENGTH, EDGE_DIR, EDGE_FACES, EDGE_FACE_ANGLE, EDGE_CREASE, EDGE_BEVEL, EDGE_SEAM, EDGE_SHARP, EDGE_FREESTYLE, FACE_MATERIAL, FACE_AREA, FACE_SIDES, FACE_PERIMETER, FACE_NORMAL, FACE_COPLANAR, FACE_SMOOTH, FACE_FREESTYLE |  |
| `compare` | ENUM | 'EQUAL' | EQUAL, GREATER, LESS |  |
| `threshold` | FLOAT[NONE] | 0 | 0 .. 100000 |  |


`bpy.ops.mesh.select_similar_region` — **Select Similar Regions** — Select similar face regions to the current selection  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.select_interior_faces` — **Select Interior Faces** — Select faces where all edges have more than 2 face users  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.select_mirror` — **Select Mirror** — Select mesh items at mirrored locations  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `axis` | ENUM | {X} | FLAG: X, Y, Z |  |
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.mesh.select_axis` — **Select Axis** — Select all data in the mesh on a single axis  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `orientation` | ENUM | 'LOCAL' | GLOBAL, LOCAL, NORMAL, GIMBAL, VIEW, CURSOR, PARENT |  |
| `sign` | ENUM | 'POS' | POS, NEG, ALIGN |  |
| `axis` | ENUM | 'X' | X, Y, Z |  |
| `threshold` | FLOAT[NONE] | 0.0001 | 1e-06 .. 50 |  |


`bpy.ops.mesh.select_by_attribute` — **Select by Attribute** — Select elements based on the active boolean attribute  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.select_by_pole_count` — **Select By Pole Count** — Select vertices at poles by the number of connected edges. In edge and face mode the geometry connected to the vertices is selected  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `pole_count` | INT | 4 | 0 .. ∞ |  |
| `type` | ENUM | 'NOTEQUAL' | LESS, EQUAL, GREATER, NOTEQUAL |  |
| `extend` | BOOLEAN | False |  |  |
| `exclude_nonmanifold` | BOOLEAN | True |  |  |


`bpy.ops.mesh.select_ungrouped` — **Select Ungrouped** — Select vertices without a group  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.mesh.loop_to_region` — **Select Loop Inner-Region** — Select region of faces inside of a selected loop of edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `select_bigger` | BOOLEAN | False |  |  |


`bpy.ops.mesh.region_to_loop` — **Select Boundary of Selected** — Select boundary edges around the selected faces  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.shortest_path_select` — **Select Shortest Path** — Select shortest path between two vertices/edges/faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `edge_mode` | ENUM | 'SELECT' | SELECT, SEAM, SHARP, CREASE, BEVEL, FREESTYLE |  |
| `use_face_step` | BOOLEAN | False |  |  |
| `use_topology_distance` | BOOLEAN | False |  |  |
| `use_fill` | BOOLEAN | False |  |  |
| `skip` | INT | 0 | 0 .. ∞ |  |
| `nth` | INT | 1 | 1 .. ∞ |  |
| `offset` | INT | 0 | -∞ .. ∞ |  |


`bpy.ops.mesh.select_next_item` — **Select Next Element** — Select the next element (using selection order)  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.select_prev_item` — **Select Previous Element** — Select the previous element (using selection order)  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.hide` — **Hide Selected** — Hide (un)selected vertices, edges or faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `unselected` | BOOLEAN | False |  |  |


`bpy.ops.mesh.reveal` — **Reveal Hidden** — Reveal all hidden vertices, edges and faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `select` | BOOLEAN | True |  |  |



### A.2 Extrude

`bpy.ops.mesh.extrude_region_move` — **Extrude Region and Move** — Extrude region and move result  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_region` | POINTER |  | MESH_OT_extrude_region |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.extrude_region_shrink_fatten` — **Extrude Region and Shrink/Fatten** — Extrude region together along local normals  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_region` | POINTER |  | MESH_OT_extrude_region |  |
| `TRANSFORM_OT_shrink_fatten` | POINTER |  | TRANSFORM_OT_shrink_fatten |  |


`bpy.ops.mesh.extrude_context_move` — **Extrude Region and Move** — Extrude region together along the average normal  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_context` | POINTER |  | MESH_OT_extrude_context |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.extrude_faces_move` — **Extrude Individual Faces and Move** — Extrude each individual face separately along local normals  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_faces_indiv` | POINTER |  | MESH_OT_extrude_faces_indiv |  |
| `TRANSFORM_OT_shrink_fatten` | POINTER |  | TRANSFORM_OT_shrink_fatten |  |


`bpy.ops.mesh.extrude_edges_move` — **Extrude Only Edges and Move** — Extrude edges and move result  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_edges_indiv` | POINTER |  | MESH_OT_extrude_edges_indiv |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.extrude_vertices_move` — **Extrude Only Vertices and Move** — Extrude vertices and move result  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_verts_indiv` | POINTER |  | MESH_OT_extrude_verts_indiv |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.extrude_region` — **Extrude Region** — Extrude region of faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_normal_flip` | BOOLEAN | False |  |  |
| `use_dissolve_ortho_edges` | BOOLEAN | False |  |  |
| `mirror` | BOOLEAN | False |  | HIDDEN |


`bpy.ops.mesh.extrude_context` — **Extrude Context** — Extrude selection  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_normal_flip` | BOOLEAN | False |  |  |
| `use_dissolve_ortho_edges` | BOOLEAN | False |  |  |
| `mirror` | BOOLEAN | False |  | HIDDEN |


`bpy.ops.mesh.extrude_faces_indiv` — **Extrude Individual Faces** — Extrude individual faces only  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mirror` | BOOLEAN | False |  | HIDDEN |


`bpy.ops.mesh.extrude_edges_indiv` — **Extrude Only Edges** — Extrude individual edges only  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_normal_flip` | BOOLEAN | False |  |  |
| `mirror` | BOOLEAN | False |  | HIDDEN |


`bpy.ops.mesh.extrude_verts_indiv` — **Extrude Only Vertices** — Extrude individual vertices only  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mirror` | BOOLEAN | False |  | HIDDEN |


`bpy.ops.mesh.extrude_repeat` — **Extrude Repeat** — Extrude selected vertices, edges or faces repeatedly  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `steps` | INT | 10 | 0 .. 1000000 |  |
| `offset` | FLOAT[XYZ_LENGTH,LENGTH]×3 | (0, 0, 0) | -100000 .. 100000 | SKIP_SAVE |
| `scale_offset` | FLOAT[NONE] | 1 | 0 .. 1e+12 |  |


`bpy.ops.mesh.extrude_manifold` — **Extrude Manifold** — Extrude, dissolves edges whose faces form a flat surface and intersect new edges  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_extrude_region` | POINTER |  | MESH_OT_extrude_region |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |



### A.3 Inset / Bevel / Loop cut / Subdivide

`bpy.ops.mesh.inset` — **Inset Faces** — Inset new faces into selected faces  
flags: UNDO, GRAB_CURSOR, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_boundary` | BOOLEAN | True |  |  |
| `use_even_offset` | BOOLEAN | True |  |  |
| `use_relative_offset` | BOOLEAN | False |  |  |
| `use_edge_rail` | BOOLEAN | False |  |  |
| `thickness` | FLOAT[DISTANCE,LENGTH] | 0 | 0 .. 1e+12 |  |
| `depth` | FLOAT[DISTANCE,LENGTH] | 0 | -1e+12 .. 1e+12 |  |
| `use_outset` | BOOLEAN | False |  |  |
| `use_select_inset` | BOOLEAN | False |  |  |
| `use_individual` | BOOLEAN | False |  |  |
| `use_interpolate` | BOOLEAN | True |  |  |
| `release_confirm` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.bevel` — **Bevel** — Cut into selected items at an angle to create bevel or chamfer  
flags: UNDO, GRAB_CURSOR, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `offset_type` | ENUM | 'OFFSET' | OFFSET, WIDTH, DEPTH, PERCENT, ABSOLUTE |  |
| `offset` | FLOAT[DISTANCE,LENGTH] | 0 | 0 .. 1e+06 |  |
| `profile_type` | ENUM | 'SUPERELLIPSE' | SUPERELLIPSE, CUSTOM |  |
| `offset_pct` | FLOAT[PERCENTAGE] | 0 | 0 .. 100 |  |
| `segments` | INT | 1 | 1 .. 1000 |  |
| `profile` | FLOAT[NONE] | 0.5 | 0 .. 1 |  |
| `affect` | ENUM | 'EDGES' | VERTICES, EDGES |  |
| `clamp_overlap` | BOOLEAN | False |  |  |
| `loop_slide` | BOOLEAN | True |  |  |
| `mark_seam` | BOOLEAN | False |  |  |
| `mark_sharp` | BOOLEAN | False |  |  |
| `material` | INT | -1 | -1 .. ∞ |  |
| `harden_normals` | BOOLEAN | False |  |  |
| `face_strength_mode` | ENUM | 'NONE' | NONE, NEW, AFFECTED, ALL |  |
| `miter_outer` | ENUM | 'SHARP' | SHARP, PATCH, ARC |  |
| `miter_inner` | ENUM | 'SHARP' | SHARP, ARC |  |
| `spread` | FLOAT[NONE] | 0.1 | 0 .. 1e+06 |  |
| `vmesh_method` | ENUM | 'ADJ' | ADJ, CUTOFF |  |
| `release_confirm` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.loopcut_slide` — **Loop Cut and Slide** — Cut mesh loop and slide it  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_loopcut` | POINTER |  | MESH_OT_loopcut |  |
| `TRANSFORM_OT_edge_slide` | POINTER |  | TRANSFORM_OT_edge_slide |  |


`bpy.ops.mesh.loopcut` — **Loop Cut** — Add a new loop between existing loops  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `number_cuts` | INT | 1 | 1 .. 1000000 | SKIP_SAVE |
| `smoothness` | FLOAT[NONE] | 0 | -1000 .. 1000 | SKIP_SAVE |
| `falloff` | ENUM | 'INVERSE_SQUARE' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR |  |
| `object_index` | INT | -1 | -1 .. ∞ | HIDDEN |
| `edge_index` | INT | -1 | -1 .. ∞ | HIDDEN |
| `mesh_select_mode_init` | BOOLEAN×3 | (False, False, False) |  | HIDDEN |


`bpy.ops.mesh.offset_edge_loops` — **Offset Edge Loop** — Create offset edge loop from the current selection  
flags: UNDO, INTERNAL, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_cap_endpoint` | BOOLEAN | False |  |  |


`bpy.ops.mesh.offset_edge_loops_slide` — **Offset Edge Slide** — Offset edge loop slide  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_offset_edge_loops` | POINTER |  | MESH_OT_offset_edge_loops |  |
| `TRANSFORM_OT_edge_slide` | POINTER |  | TRANSFORM_OT_edge_slide |  |


`bpy.ops.mesh.subdivide` — **Subdivide** — Subdivide selected edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `number_cuts` | INT | 1 | 1 .. 100 | SKIP_SAVE |
| `smoothness` | FLOAT[NONE] | 0 | 0 .. 1000 |  |
| `ngon` | BOOLEAN | True |  |  |
| `quadcorner` | ENUM | 'STRAIGHT_CUT' | INNERVERT, PATH, STRAIGHT_CUT, FAN |  |
| `fractal` | FLOAT[NONE] | 0 | 0 .. 1e+06 |  |
| `fractal_along_normal` | FLOAT[NONE] | 0 | 0 .. 1 |  |
| `seed` | INT | 0 | 0 .. ∞ |  |


`bpy.ops.mesh.subdivide_edgering` — **Subdivide Edge-Ring** — Subdivide perpendicular edges to the selected edge-ring  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `number_cuts` | INT | 10 | 0 .. 1000 | SKIP_SAVE |
| `interpolation` | ENUM | 'PATH' | LINEAR, PATH, SURFACE |  |
| `smoothness` | FLOAT[NONE] | 1 | 0 .. 1000 |  |
| `profile_shape_factor` | FLOAT[NONE] | 0 | -1000 .. 1000 |  |
| `profile_shape` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR |  |


`bpy.ops.mesh.unsubdivide` — **Un-Subdivide** — Un-subdivide selected edges and faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `iterations` | INT | 2 | 1 .. 1000 |  |



### A.4 Merge / Delete / Dissolve

`bpy.ops.mesh.merge` — **Merge** — Merge selected vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'CENTER' | CENTER, CURSOR, COLLAPSE, FIRST, LAST |  |
| `uvs` | BOOLEAN | False |  |  |


`bpy.ops.mesh.remove_doubles` — **Merge by Distance** — Merge vertices based on their proximity  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `threshold` | FLOAT[DISTANCE,LENGTH] | 0.0001 | 1e-06 .. 50 |  |
| `use_centroid` | BOOLEAN | True |  |  |
| `use_unselected` | BOOLEAN | False |  |  |
| `use_sharp_edge_from_normals` | BOOLEAN | False |  |  |


`bpy.ops.mesh.delete` — **Delete** — Delete selected vertices, edges or faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'VERT' | VERT, EDGE, FACE, EDGE_FACE, ONLY_FACE | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.delete_loose` — **Delete Loose** — Delete loose vertices, edges or faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_verts` | BOOLEAN | True |  |  |
| `use_edges` | BOOLEAN | True |  |  |
| `use_faces` | BOOLEAN | False |  |  |


`bpy.ops.mesh.delete_edgeloop` — **Delete Edge Loop** — Delete an edge loop by merging the faces on each side  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_face_split` | BOOLEAN | True |  |  |


`bpy.ops.mesh.dissolve_verts` — **Dissolve Vertices** — Dissolve vertices, merge edges and faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_face_split` | BOOLEAN | False |  |  |
| `use_boundary_tear` | BOOLEAN | False |  |  |


`bpy.ops.mesh.dissolve_edges` — **Dissolve Edges** — Dissolve edges, merging faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_verts` | BOOLEAN | True |  |  |
| `angle_threshold` | FLOAT[ANGLE,ROTATION] | 3.14159 | 0 .. 3.14159 |  |
| `use_face_split` | BOOLEAN | False |  |  |
| `use_preserve_quads` | BOOLEAN | True |  |  |


`bpy.ops.mesh.dissolve_faces` — **Dissolve Faces** — Dissolve faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_verts` | BOOLEAN | False |  |  |


`bpy.ops.mesh.dissolve_mode` — **Dissolve Selection** — Dissolve geometry based on the selection mode  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_verts` | BOOLEAN | False |  | SKIP_SAVE |
| `angle_threshold` | FLOAT[ANGLE,ROTATION] | 3.14159 | 0 .. 3.14159 | SKIP_SAVE |
| `use_preserve_quads` | BOOLEAN | True |  | SKIP_SAVE |
| `use_face_split` | BOOLEAN | False |  |  |
| `use_boundary_tear` | BOOLEAN | False |  |  |


`bpy.ops.mesh.dissolve_limited` — **Limited Dissolve** — Dissolve selected edges and vertices, limited by the angle of surrounding geometry  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle_limit` | FLOAT[ANGLE,ROTATION] | 0.0872665 | 0 .. 3.14159 |  |
| `use_dissolve_boundaries` | BOOLEAN | False |  |  |
| `delimit` | ENUM | {NORMAL} | FLAG: NORMAL, MATERIAL, SEAM, SHARP, UV |  |


`bpy.ops.mesh.dissolve_degenerate` — **Degenerate Dissolve** — Dissolve zero area faces and zero length edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `threshold` | FLOAT[DISTANCE,LENGTH] | 0.0001 | 1e-06 .. 50 |  |


`bpy.ops.mesh.edge_collapse` — **Collapse Edges & Faces** — Collapse isolated edge and face regions, merging data such as UVs and color attributes. This can collapse edge-rings as well as regions of connected faces into vertices  
flags: UNDO, REGISTER

_(no properties)_



### A.5 Bridge / Fill / Connect

`bpy.ops.mesh.bridge_edge_loops` — **Bridge Edge Loops** — Create a bridge of faces between two or more selected edge loops  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'SINGLE' | SINGLE, CLOSED, PAIRS |  |
| `use_merge` | BOOLEAN | False |  |  |
| `merge_factor` | FLOAT[NONE] | 0.5 | 0 .. 1 |  |
| `twist_offset` | INT | 0 | -1000 .. 1000 |  |
| `number_cuts` | INT | 0 | 0 .. 1000 | SKIP_SAVE |
| `interpolation` | ENUM | 'PATH' | LINEAR, PATH, SURFACE |  |
| `smoothness` | FLOAT[NONE] | 1 | 0 .. 1000 |  |
| `profile_shape_factor` | FLOAT[NONE] | 0 | -1000 .. 1000 |  |
| `profile_shape` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR |  |


`bpy.ops.mesh.fill` — **Fill** — Fill a selected edge loop with faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_beauty` | BOOLEAN | True |  |  |


`bpy.ops.mesh.fill_grid` — **Grid Fill** — Fill grid from two loops  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `span` | INT | 1 | 1 .. 1000 | SKIP_SAVE |
| `offset` | INT | 0 | -1000 .. 1000 | SKIP_SAVE |
| `use_interp_simple` | BOOLEAN | False |  |  |


`bpy.ops.mesh.fill_holes` — **Fill Holes** — Fill in holes (boundary edge loops)  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `sides` | INT | 4 | 0 .. 1000 |  |


`bpy.ops.mesh.beautify_fill` — **Beautify Faces** — Rearrange some faces to try to get less degenerated geometry  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle_limit` | FLOAT[ANGLE,ROTATION] | 3.14159 | 0 .. 3.14159 |  |


`bpy.ops.mesh.edge_face_add` — **Make Edge/Face** — Add an edge or face to selected  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.vert_connect` — **Vertex Connect** — Connect selected vertices of faces, splitting the face  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.vert_connect_path` — **Vertex Connect Path** — Connect vertices by their selection order, creating edges, splitting faces  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.vert_connect_concave` — **Split Concave Faces** — Split concave faces by connecting vertices to make them convex  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.vert_connect_nonplanar` — **Split Non-Planar Faces** — Split non-planar faces that exceed the angle threshold  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle_limit` | FLOAT[ANGLE,ROTATION] | 0.0872665 | 0 .. 3.14159 |  |


`bpy.ops.mesh.face_split_by_edges` — **Weld Edges into Faces** — Weld loose edges into faces (splitting them into new faces)  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.edge_rotate` — **Rotate Selected Edge** — Rotate selected edge or adjoining faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_ccw` | BOOLEAN | False |  |  |


`bpy.ops.mesh.convex_hull` — **Convex Hull** — Enclose selected vertices in a convex polyhedron  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delete_unused` | BOOLEAN | True |  |  |
| `use_existing_faces` | BOOLEAN | True |  |  |
| `make_holes` | BOOLEAN | False |  |  |
| `join_triangles` | BOOLEAN | True |  |  |
| `face_threshold` | FLOAT[ANGLE,ROTATION] | 0.698132 | 0 .. 3.14159 |  |
| `shape_threshold` | FLOAT[ANGLE,ROTATION] | 0.698132 | 0 .. 3.14159 |  |
| `topology_influence` | FLOAT[FACTOR] | 0 | 0 .. 2 |  |
| `uvs` | BOOLEAN | False |  |  |
| `vcols` | BOOLEAN | False |  |  |
| `seam` | BOOLEAN | False |  |  |
| `sharp` | BOOLEAN | False |  |  |
| `materials` | BOOLEAN | False |  |  |
| `deselect_joined` | BOOLEAN | False |  |  |


`bpy.ops.mesh.bisect` — **Bisect** — Cut geometry along a plane (click-drag to define plane)  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `plane_co` | FLOAT[XYZ_LENGTH,LENGTH]×3 | (0, 0, 0) | -1e+12 .. 1e+12 | SKIP_SAVE |
| `plane_no` | FLOAT[XYZ]×3 | (0, 0, 0) | -1 .. 1 | SKIP_SAVE |
| `use_fill` | BOOLEAN | False |  |  |
| `clear_inner` | BOOLEAN | False |  |  |
| `clear_outer` | BOOLEAN | False |  |  |
| `threshold` | FLOAT[NONE] | 0.0001 | 0 .. 10 |  |
| `xstart` | INT | 0 | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `xend` | INT | 0 | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `ystart` | INT | 0 | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `yend` | INT | 0 | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `flip` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `cursor` | INT | 5 | 0 .. ∞ | HIDDEN |



### A.6 Normals / Shading / Custom normals

`bpy.ops.mesh.flip_normals` — **Flip Normals** — Flip the direction of selected faces' normals (and of their vertices)  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `only_clnors` | BOOLEAN | False |  |  |


`bpy.ops.mesh.normals_make_consistent` — **Recalculate Normals** — Make face and vertex normals point either outside or inside the mesh  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `inside` | BOOLEAN | False |  |  |


`bpy.ops.mesh.set_normals_from_faces` — **Set Normals from Faces** — Set the custom normals from the selected faces ones  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `keep_sharp` | BOOLEAN | False |  |  |


`bpy.ops.mesh.normals_tools` — **Normals Vector Tools** — Custom normals tools using Normal Vector of UI  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | ENUM | 'COPY' | COPY, PASTE, ADD, MULTIPLY, RESET | HIDDEN |
| `absolute` | BOOLEAN | False |  |  |


`bpy.ops.mesh.smooth_normals` — **Smooth Normals Vectors** — Smooth custom normals based on adjacent vertex normals  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[NONE] | 0.5 | 0 .. 1 |  |


`bpy.ops.mesh.average_normals` — **Average Normals** — Average custom normals of selected vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `average_type` | ENUM | 'CUSTOM_NORMAL' | CUSTOM_NORMAL, FACE_AREA, CORNER_ANGLE |  |
| `weight` | INT | 50 | 1 .. 100 |  |
| `threshold` | FLOAT[NONE] | 0.01 | 0 .. 10 |  |


`bpy.ops.mesh.point_normals` — **Point Normals to Target** — Point selected custom normals to specified Target  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | ENUM | 'COORDINATES' | COORDINATES, MOUSE | HIDDEN |
| `invert` | BOOLEAN | False |  |  |
| `align` | BOOLEAN | False |  |  |
| `target_location` | FLOAT[XYZ_LENGTH,LENGTH]×3 | (0, 0, 0) | -∞ .. ∞ |  |
| `spherize` | BOOLEAN | False |  |  |
| `spherize_strength` | FLOAT[NONE] | 0.1 | 0 .. 1 |  |


`bpy.ops.mesh.merge_normals` — **Merge Normals** — Merge custom normals of selected vertices  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.split_normals` — **Split Normals** — Split custom normals of selected vertices  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.mod_weighted_strength` — **Face Normals Strength** — Set/Get strength of face (used in Weighted Normal modifier)  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `set` | BOOLEAN | False |  |  |
| `face_strength` | ENUM | 'MEDIUM' | WEAK, MEDIUM, STRONG |  |


`bpy.ops.mesh.faces_shade_smooth` — **Shade Smooth** — Display faces smooth (using vertex normals)  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.faces_shade_flat` — **Shade Flat** — Display faces flat  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.set_sharpness_by_angle` — **Set Sharpness by Angle** — Set edge sharpness based on the angle between neighboring faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle` | FLOAT[ANGLE,ROTATION] | 0.523599 | 0.000174533 .. 3.14159 |  |
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.mesh.customdata_custom_splitnormals_add` — **Add Custom Normals Data** — Add a custom normals layer, if none exists yet  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.customdata_custom_splitnormals_clear` — **Clear Custom Normals Data** — Remove the custom normals layer, if it exists  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.object.shade_smooth` — **Shade Smooth** — Render and display faces smooth, using interpolated vertex normals  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `keep_sharp_edges` | BOOLEAN | True |  |  |


`bpy.ops.object.shade_flat` — **Shade Flat** — Render and display faces uniform, using face normals  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `keep_sharp_edges` | BOOLEAN | True |  |  |


`bpy.ops.object.shade_smooth_by_angle` — **Shade Smooth by Angle** — Set the sharpness of mesh edges based on the angle between the neighboring faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle` | FLOAT[ANGLE,ROTATION] | 0.523599 | 0 .. 3.14159 |  |
| `keep_sharp_edges` | BOOLEAN | True |  |  |


`bpy.ops.object.shade_auto_smooth` — **Shade Auto Smooth** — Add modifier to automatically set the sharpness of mesh edges based on the angle between the neighboring faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_auto_smooth` | BOOLEAN | True |  | SKIP_SAVE |
| `angle` | FLOAT[ANGLE,ROTATION] | 0.523599 | 0 .. 3.14159 |  |



### A.7 Marking / attributes / layers

`bpy.ops.mesh.mark_seam` — **Mark Seam** — (Un)mark selected edges as a seam  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `clear` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.mark_sharp` — **Mark Sharp** — (Un)mark selected edges as sharp  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `clear` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `use_verts` | BOOLEAN | False |  | SKIP_SAVE |


`bpy.ops.mesh.mark_freestyle_edge` — **Mark Freestyle Edge** — (Un)mark selected edges as Freestyle feature edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `clear` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |


`bpy.ops.transform.edge_crease` — **Edge Crease** — Change the crease of edges  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[FACTOR] | 0 | -1 .. 1 |  |


`bpy.ops.transform.edge_bevelweight` — **Edge Bevel Weight** — Change the bevel weight of edges  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[FACTOR] | 0 | -1 .. 1 |  |


`bpy.ops.mesh.attribute_set` — **Set Attribute** — Set values of the active attribute for selected elements  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value_float` | FLOAT[NONE] | 0 | -∞ .. ∞ |  |
| `value_float_vector_2d` | FLOAT[NONE]×2 | (0, 0) | -∞ .. ∞ |  |
| `value_float_vector_3d` | FLOAT[NONE]×3 | (0, 0, 0) | -∞ .. ∞ |  |
| `value_float_vector_4d` | FLOAT[NONE]×4 | (0, 0, 0, 0) | -∞ .. ∞ |  |
| `value_int` | INT | 0 | -∞ .. ∞ |  |
| `value_int_vector_2d` | INT×2 | (0, 0) | -∞ .. ∞ |  |
| `value_color` | FLOAT[COLOR]×4 | (1, 1, 1, 1) | -∞ .. ∞ |  |
| `value_bool` | BOOLEAN | False |  |  |


`bpy.ops.mesh.uv_texture_add` — **Add UV Map** — Add UV map  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.uv_texture_remove` — **Remove UV Map** — Remove UV map  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.geometry.attribute_add` — **Add Attribute** — Add attribute to geometry  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `name` | STRING |  |  | SKIP_SAVE |
| `domain` | ENUM | 'POINT' | POINT, EDGE, FACE, CORNER, CURVE, INSTANCE, LAYER | SKIP_SAVE |
| `data_type` | ENUM | 'FLOAT' | FLOAT, INT, BOOLEAN, FLOAT_VECTOR, FLOAT_COLOR, QUATERNION, FLOAT4X4, STRING, INT8, INT16_2D, INT32_2D, FLOAT2, FLOAT4, BYTE_COLOR | SKIP_SAVE |


`bpy.ops.geometry.color_attribute_add` — **Add Color Attribute** — Add color attribute to geometry  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `name` | STRING |  |  | SKIP_SAVE |
| `domain` | ENUM | 'POINT' | POINT, CORNER |  |
| `data_type` | ENUM | 'FLOAT_COLOR' | FLOAT_COLOR, BYTE_COLOR |  |
| `color` | FLOAT[COLOR]×4 | (0, 0, 0, 1) | 0 .. ∞ |  |


`bpy.ops.mesh.customdata_mask_clear` — **Clear Sculpt Mask Data** — Clear vertex sculpt masking data from the mesh  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.customdata_skin_add` — **Add Skin Data** — Add a vertex skin layer  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.customdata_skin_clear` — **Clear Skin Data** — Clear vertex skin layer  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.colors_rotate` — **Rotate Colors** — Rotate face corner color attribute inside faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_ccw` | BOOLEAN | False |  |  |


`bpy.ops.mesh.colors_reverse` — **Reverse Colors** — Flip direction of face corner color attribute inside faces  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.uvs_rotate` — **Rotate UVs** — Rotate UV coordinates inside faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_ccw` | BOOLEAN | False |  |  |


`bpy.ops.mesh.uvs_reverse` — **Reverse UVs** — Flip direction of UV coordinates inside faces  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.sort_elements` — **Sort Mesh Elements** — The order of selected vertices/edges/faces is modified, based on a given method  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'VIEW_ZAXIS' | VIEW_ZAXIS, VIEW_XAXIS, CURSOR_DISTANCE, MATERIAL, SELECTED, RANDOMIZE, REVERSE |  |
| `elements` | ENUM | {VERT} | FLAG: VERT, EDGE, FACE |  |
| `reverse` | BOOLEAN | False |  |  |
| `seed` | INT | 0 | 0 .. ∞ |  |


`bpy.ops.mesh.reorder_vertices_spatial` — **Reorder Mesh Spatially** — Reorder mesh faces and vertices based on their spatial position for better BVH building and sculpting performance.  
flags: UNDO, REGISTER

_(no properties)_



### A.8 Split / Separate / Duplicate / Rip

`bpy.ops.mesh.split` — **Split** — Split off selected geometry from connected unselected geometry  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.edge_split` — **Edge Split** — Split selected edges so that each neighbor face gets its own copy  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'EDGE' | EDGE, VERT |  |


`bpy.ops.mesh.separate` — **Separate** — Separate selected geometry into a new mesh  
flags: UNDO

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'SELECTED' | SELECTED, MATERIAL, LOOSE |  |


`bpy.ops.mesh.duplicate` — **Duplicate** — Duplicate selected vertices, edges or faces  
flags: 

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | INT | 1 | 0 .. ∞ | HIDDEN |


`bpy.ops.mesh.duplicate_move` — **Add Duplicate** — Duplicate mesh and move  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_duplicate` | POINTER |  | MESH_OT_duplicate |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.rip` — **Rip** — Disconnect vertices or edges from connected geometry  
flags: UNDO, DEPENDS_ON_CURSOR, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mirror` | BOOLEAN | False |  | HIDDEN |
| `use_proportional_edit` | BOOLEAN | False |  |  |
| `proportional_edit_falloff` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR, CONSTANT, RANDOM |  |
| `proportional_size` | FLOAT[NONE] | 1 | 1e-06 .. 1e+12 |  |
| `use_proportional_connected` | BOOLEAN | False |  |  |
| `use_proportional_projected` | BOOLEAN | False |  |  |
| `release_confirm` | BOOLEAN | False |  | HIDDEN |
| `use_accurate` | BOOLEAN | False |  | HIDDEN |
| `use_fill` | BOOLEAN | False |  |  |


`bpy.ops.mesh.rip_move` — **Rip** — Rip polygons and move the result  
flags: UNDO, MACRO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `MESH_OT_rip` | POINTER |  | MESH_OT_rip |  |
| `TRANSFORM_OT_translate` | POINTER |  | TRANSFORM_OT_translate |  |


`bpy.ops.mesh.rip_edge` — **Extend Vertices** — Extend vertices along the edge closest to the cursor  
flags: UNDO, REGISTER, DEPENDS_ON_CURSOR

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `location` | FLOAT[XYZ]×3 | (0, 0, 0) | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `direction` | FLOAT[XYZ]×3 | (0, 0, 0) | -∞ .. ∞ | HIDDEN SKIP_SAVE |



### A.9 Spin / Screw / Solidify / Wireframe

`bpy.ops.mesh.spin` — **Spin** — Extrude selected vertices in a circle around the cursor in indicated viewport  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `steps` | INT | 12 | 0 .. 1000000 |  |
| `dupli` | BOOLEAN | False |  | SKIP_SAVE |
| `angle` | FLOAT[ANGLE,ROTATION] | 1.5708 | -1e+12 .. 1e+12 |  |
| `use_auto_merge` | BOOLEAN | True |  |  |
| `use_normal_flip` | BOOLEAN | False |  |  |
| `center` | FLOAT[XYZ_LENGTH,LENGTH]×3 | (0, 0, 0) | -1e+12 .. 1e+12 |  |
| `axis` | FLOAT[XYZ]×3 | (0, 0, 0) | -1 .. 1 |  |


`bpy.ops.mesh.screw` — **Screw** — Extrude selected vertices in screw-shaped rotation around the cursor in indicated viewport  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `steps` | INT | 9 | 1 .. 100000 |  |
| `turns` | INT | 1 | 1 .. 100000 |  |
| `center` | FLOAT[XYZ_LENGTH,LENGTH]×3 | (0, 0, 0) | -1e+12 .. 1e+12 |  |
| `axis` | FLOAT[XYZ]×3 | (0, 0, 0) | -1 .. 1 |  |


`bpy.ops.mesh.solidify` — **Solidify** — Create a solid skin by extruding, compensating for sharp angles  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `thickness` | FLOAT[DISTANCE,LENGTH] | 0.01 | -10000 .. 10000 |  |


`bpy.ops.mesh.wireframe` — **Wireframe** — Create a solid wireframe from faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_boundary` | BOOLEAN | True |  |  |
| `use_even_offset` | BOOLEAN | True |  |  |
| `use_relative_offset` | BOOLEAN | False |  |  |
| `use_replace` | BOOLEAN | True |  |  |
| `thickness` | FLOAT[DISTANCE,LENGTH] | 0.01 | 0 .. 10000 |  |
| `offset` | FLOAT[DISTANCE,LENGTH] | 0.01 | 0 .. 10000 |  |
| `use_crease` | BOOLEAN | False |  |  |
| `crease_weight` | FLOAT[NONE] | 0.01 | 0 .. 1000 |  |



### A.10 Triangulate / Quads / Poke / Decimate / Boolean

`bpy.ops.mesh.quads_convert_to_tris` — **Triangulate Faces** — Triangulate selected faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `quad_method` | ENUM | 'BEAUTY' | BEAUTY, FIXED, FIXED_ALTERNATE, SHORTEST_DIAGONAL, LONGEST_DIAGONAL |  |
| `ngon_method` | ENUM | 'BEAUTY' | BEAUTY, CLIP |  |


`bpy.ops.mesh.tris_convert_to_quads` — **Triangles to Quads** — Merge triangles into four sided polygons where possible  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `face_threshold` | FLOAT[ANGLE,ROTATION] | 0.698132 | 0 .. 3.14159 |  |
| `shape_threshold` | FLOAT[ANGLE,ROTATION] | 0.698132 | 0 .. 3.14159 |  |
| `topology_influence` | FLOAT[FACTOR] | 0 | 0 .. 2 |  |
| `uvs` | BOOLEAN | False |  |  |
| `vcols` | BOOLEAN | False |  |  |
| `seam` | BOOLEAN | False |  |  |
| `sharp` | BOOLEAN | False |  |  |
| `materials` | BOOLEAN | False |  |  |
| `deselect_joined` | BOOLEAN | False |  |  |


`bpy.ops.mesh.poke` — **Poke Faces** — Split a face into a fan  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `offset` | FLOAT[DISTANCE,LENGTH] | 0 | -1000 .. 1000 |  |
| `use_relative_offset` | BOOLEAN | False |  |  |
| `center_mode` | ENUM | 'MEDIAN_WEIGHTED' | MEDIAN_WEIGHTED, MEDIAN, BOUNDS |  |


`bpy.ops.mesh.flip_quad_tessellation` — **Flip Quad Tessellation** — Flips the tessellation of selected quads  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.mesh.decimate` — **Decimate Geometry** — Simplify geometry by collapsing edges  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `ratio` | FLOAT[NONE] | 1 | 0 .. 1 |  |
| `use_vertex_group` | BOOLEAN | False |  |  |
| `vertex_group_factor` | FLOAT[NONE] | 1 | 0 .. 1000 |  |
| `invert_vertex_group` | BOOLEAN | False |  |  |
| `use_symmetry` | BOOLEAN | False |  |  |
| `symmetry_axis` | ENUM | 'Y' | X, Y, Z |  |


`bpy.ops.mesh.intersect` — **Intersect (Knife)** — Cut an intersection into faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | ENUM | 'SELECT_UNSELECT' | SELECT, SELECT_UNSELECT |  |
| `separate_mode` | ENUM | 'CUT' | ALL, CUT, NONE |  |
| `threshold` | FLOAT[DISTANCE,LENGTH] | 1e-06 | 0 .. 0.01 |  |
| `solver` | ENUM | 'EXACT' | FLOAT, EXACT |  |


`bpy.ops.mesh.intersect_boolean` — **Intersect (Boolean)** — Cut solid geometry from selected to unselected  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `operation` | ENUM | 'DIFFERENCE' | INTERSECT, UNION, DIFFERENCE |  |
| `use_swap` | BOOLEAN | False |  |  |
| `use_self` | BOOLEAN | False |  |  |
| `threshold` | FLOAT[DISTANCE,LENGTH] | 1e-06 | 0 .. 0.01 |  |
| `solver` | ENUM | 'EXACT' | FLOAT, EXACT |  |


`bpy.ops.mesh.knife_project` — **Knife Project** — Use other objects outlines and boundaries to project knife cuts  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `cut_through` | BOOLEAN | False |  |  |



### A.11 Deform / Symmetry / LoopTools-style

`bpy.ops.mesh.vertices_smooth` — **Smooth Vertices** — Flatten angles of selected vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[FACTOR] | 0 | -10 .. 10 |  |
| `repeat` | INT | 1 | 1 .. 1000 |  |
| `xaxis` | BOOLEAN | True |  |  |
| `yaxis` | BOOLEAN | True |  |  |
| `zaxis` | BOOLEAN | True |  |  |
| `wait_for_input` | BOOLEAN | True |  | HIDDEN SKIP_SAVE |


`bpy.ops.mesh.vertices_smooth_laplacian` — **Laplacian Smooth Vertices** — Laplacian smooth of selected vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `repeat` | INT | 1 | 1 .. 1000 |  |
| `lambda_factor` | FLOAT[NONE] | 1 | 1e-07 .. 1000 |  |
| `lambda_border` | FLOAT[NONE] | 5e-05 | 1e-07 .. 1000 |  |
| `use_x` | BOOLEAN | True |  |  |
| `use_y` | BOOLEAN | True |  |  |
| `use_z` | BOOLEAN | True |  |  |
| `preserve_volume` | BOOLEAN | True |  |  |


`bpy.ops.mesh.symmetrize` — **Symmetrize** — Enforce symmetry (both form and topological) across an axis  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `direction` | ENUM | 'NEGATIVE_X' | NEGATIVE_X, POSITIVE_X, NEGATIVE_Y, POSITIVE_Y, NEGATIVE_Z, POSITIVE_Z |  |
| `threshold` | FLOAT[DISTANCE,LENGTH] | 0.0001 | 0 .. 10 |  |


`bpy.ops.mesh.symmetry_snap` — **Snap to Symmetry** — Snap vertex pairs to their mirrored locations  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `direction` | ENUM | 'NEGATIVE_X' | NEGATIVE_X, POSITIVE_X, NEGATIVE_Y, POSITIVE_Y, NEGATIVE_Z, POSITIVE_Z |  |
| `threshold` | FLOAT[DISTANCE,LENGTH] | 0.05 | 0 .. 10 |  |
| `factor` | FLOAT[NONE] | 0.5 | 0 .. 1 |  |
| `use_center` | BOOLEAN | True |  |  |
| `use_topology` | BOOLEAN | False |  |  |


`bpy.ops.mesh.face_make_planar` — **Make Planar Faces** — Flatten selected faces  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[NONE] | 1 | -10 .. 10 |  |
| `repeat` | INT | 1 | 1 .. 10000 |  |


`bpy.ops.mesh.circularize` — **To Circle** — Shape selected geometry into a circle  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[FACTOR] | 1 | 0 .. 1 |  |
| `fit_method` | ENUM | 'LEAST_SQUARE' | LEAST_SQUARE, CONTRACT |  |
| `angle` | FLOAT[ANGLE,ROTATION] | 0 | -6.28319 .. 6.28319 |  |
| `use_custom_radius` | BOOLEAN | False |  |  |
| `custom_radius` | FLOAT[NONE] | 1 | 0 .. ∞ |  |
| `regular` | BOOLEAN | True |  |  |
| `flatten` | FLOAT[FACTOR] | 1 | 0 .. 1 |  |
| `lock` | BOOLEAN×3 | (False, False, False) |  |  |


`bpy.ops.mesh.flatten` — **Flatten** — Flatten vertices on a best-fitting plane  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[FACTOR] | 1 | 0 .. 1 |  |
| `method` | ENUM | 'BEST_FIT' | BEST_FIT, NORMAL, VIEW |  |
| `lock` | BOOLEAN×3 | (False, False, False) |  |  |


`bpy.ops.mesh.space_edge_loops_evenly` — **Space Edge Loops Evenly** — Space the vertices in a regular distribution on the loop  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `factor` | FLOAT[FACTOR] | 1 | 0 .. 1 |  |
| `interpolation` | ENUM | 'CUBIC' | CUBIC, LINEAR |  |
| `lock` | BOOLEAN×3 | (False, False, False) |  |  |


`bpy.ops.mesh.blend_from_shape` — **Blend from Shape** — Blend in shape from a shape key  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `shape` | ENUM |  |  |  |
| `blend` | FLOAT[NONE] | 1 | -1000 .. 1000 |  |
| `add` | BOOLEAN | True |  |  |


`bpy.ops.mesh.shape_propagate_to_all` — **Shape Propagate** — Apply selected vertex locations to all other shape keys  
flags: UNDO, REGISTER

_(no properties)_



### A.12 Transform (edit-mode use)

`bpy.ops.transform.translate` — **Move** — Move selected items  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[TRANSLATION,LENGTH]×3 | (0, 0, 0) | -∞ .. ∞ |  |
| `orient_type` | ENUM | 'GLOBAL' | GLOBAL, LOCAL, NORMAL, GIMBAL, VIEW, CURSOR, PARENT |  |
| `constraint_axis` | BOOLEAN×3 | (False, False, False) |  |  |
| `mirror` | BOOLEAN | False |  |  |
| `use_proportional_edit` | BOOLEAN | False |  |  |
| `proportional_edit_falloff` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR, CONSTANT, RANDOM |  |
| `proportional_size` | FLOAT[NONE] | 1 | 1e-06 .. 1e+12 |  |
| `use_proportional_connected` | BOOLEAN | False |  |  |
| `use_proportional_projected` | BOOLEAN | False |  |  |


`bpy.ops.transform.rotate` — **Rotate** — Rotate selected items  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[ANGLE,ROTATION] | 0 | -∞ .. ∞ |  |
| `orient_axis` | ENUM | 'Z' | X, Y, Z | SKIP_SAVE |
| `orient_type` | ENUM | 'GLOBAL' |  |  |
| `constraint_axis` | BOOLEAN×3 | (False, False, False) |  |  |
| `mirror` | BOOLEAN | False |  |  |
| `use_proportional_edit` | BOOLEAN | False |  |  |
| `proportional_edit_falloff` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR, CONSTANT, RANDOM |  |
| `proportional_size` | FLOAT[NONE] | 1 | 1e-06 .. 1e+12 |  |
| `use_proportional_connected` | BOOLEAN | False |  |  |
| `use_proportional_projected` | BOOLEAN | False |  |  |


`bpy.ops.transform.resize` — **Resize** — Scale (resize) selected items  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[XYZ]×3 | (1, 1, 1) | -∞ .. ∞ |  |
| `mouse_dir_constraint` | FLOAT[XYZ]×3 | (0, 0, 0) | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `orient_type` | ENUM | 'GLOBAL' |  |  |
| `constraint_axis` | BOOLEAN×3 | (False, False, False) |  |  |
| `mirror` | BOOLEAN | False |  |  |
| `use_proportional_edit` | BOOLEAN | False |  |  |
| `proportional_edit_falloff` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR, CONSTANT, RANDOM |  |
| `proportional_size` | FLOAT[NONE] | 1 | 1e-06 .. 1e+12 |  |
| `use_proportional_connected` | BOOLEAN | False |  |  |
| `use_proportional_projected` | BOOLEAN | False |  |  |


`bpy.ops.transform.shrink_fatten` — **Shrink/Fatten** — Shrink/fatten selected vertices along normals  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[DISTANCE,LENGTH] | 0 | -∞ .. ∞ |  |
| `use_even_offset` | BOOLEAN | False |  |  |
| `mirror` | BOOLEAN | False |  |  |
| `use_proportional_edit` | BOOLEAN | False |  |  |
| `proportional_edit_falloff` | ENUM | 'SMOOTH' | SMOOTH, SPHERE, ROOT, INVERSE_SQUARE, SHARP, LINEAR, CONSTANT, RANDOM |  |
| `proportional_size` | FLOAT[NONE] | 1 | 1e-06 .. 1e+12 |  |
| `use_proportional_connected` | BOOLEAN | False |  |  |
| `use_proportional_projected` | BOOLEAN | False |  |  |


`bpy.ops.transform.edge_slide` — **Edge Slide** — Slide an edge loop along a mesh  
flags: UNDO, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[FACTOR] | 0 | -10 .. 10 |  |
| `single_side` | BOOLEAN | False |  | HIDDEN SKIP_SAVE |
| `use_even` | BOOLEAN | False |  |  |
| `flipped` | BOOLEAN | False |  |  |
| `use_clamp` | BOOLEAN | True |  |  |
| `mirror` | BOOLEAN | False |  |  |
| `correct_uv` | BOOLEAN | True |  |  |


`bpy.ops.transform.vert_slide` — **Vertex Slide** — Slide a vertex along a mesh  
flags: UNDO, DEPENDS_ON_CURSOR, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `value` | FLOAT[FACTOR] | 0 | -10 .. 10 |  |
| `use_even` | BOOLEAN | False |  |  |
| `flipped` | BOOLEAN | False |  |  |
| `use_clamp` | BOOLEAN | True |  |  |
| `direction` | FLOAT[XYZ]×3 | (0, 0, 0) | -∞ .. ∞ | HIDDEN SKIP_SAVE |
| `mirror` | BOOLEAN | False |  |  |
| `correct_uv` | BOOLEAN | True |  |  |



### A.13 UV

`bpy.ops.uv.smart_project` — **Smart UV Project** — Projection unwraps the selected faces of mesh objects  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `angle_limit` | FLOAT[ANGLE,ROTATION] | 1.15192 | 0 .. 1.5708 |  |
| `margin_method` | ENUM | 'SCALED' | SCALED, ADD, FRACTION |  |
| `rotate_method` | ENUM | 'AXIS_ALIGNED_Y' | AXIS_ALIGNED, AXIS_ALIGNED_X, AXIS_ALIGNED_Y |  |
| `island_margin` | FLOAT[NONE] | 0 | 0 .. 1 |  |
| `area_weight` | FLOAT[NONE] | 0 | 0 .. 1 |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `scale_to_bounds` | BOOLEAN | False |  |  |


`bpy.ops.uv.unwrap` — **Unwrap** — Unwrap the mesh of the object being edited  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `method` | ENUM | 'CONFORMAL' | ANGLE_BASED, CONFORMAL, MINIMUM_STRETCH |  |
| `fill_holes` | BOOLEAN | False |  |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `use_subsurf_data` | BOOLEAN | False |  |  |
| `use_original_bounds` | BOOLEAN | False |  |  |
| `margin_method` | ENUM | 'SCALED' | SCALED, ADD, FRACTION |  |
| `margin` | FLOAT[FACTOR] | 0.001 | 0 .. 1 |  |
| `no_flip` | BOOLEAN | False |  |  |
| `iterations` | INT | 10 | 0 .. 10000 |  |
| `use_weights` | BOOLEAN | False |  |  |
| `weight_group` | STRING | 'uv_importance' |  |  |
| `weight_factor` | FLOAT[NONE] | 1 | -10000 .. 10000 |  |


`bpy.ops.uv.cube_project` — **Cube Projection** — Project the UV vertices of the mesh over the six faces of a cube  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `cube_size` | FLOAT[NONE] | 1 | 0 .. ∞ |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `clip_to_bounds` | BOOLEAN | False |  |  |
| `scale_to_bounds` | BOOLEAN | False |  |  |


`bpy.ops.uv.cylinder_project` — **Cylinder Projection** — Project the UV vertices of the mesh over the curved wall of a cylinder  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `direction` | ENUM | 'VIEW_ON_EQUATOR' | VIEW_ON_EQUATOR, VIEW_ON_POLES, ALIGN_TO_OBJECT |  |
| `align` | ENUM | 'POLAR_ZX' | POLAR_ZX, POLAR_ZY |  |
| `pole` | ENUM | 'PINCH' | PINCH, FAN |  |
| `seam` | BOOLEAN | False |  |  |
| `radius` | FLOAT[NONE] | 1 | 0 .. ∞ |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `clip_to_bounds` | BOOLEAN | False |  |  |
| `scale_to_bounds` | BOOLEAN | False |  |  |


`bpy.ops.uv.sphere_project` — **Sphere Projection** — Project the UV vertices of the mesh over the curved surface of a sphere  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `direction` | ENUM | 'VIEW_ON_EQUATOR' | VIEW_ON_EQUATOR, VIEW_ON_POLES, ALIGN_TO_OBJECT |  |
| `align` | ENUM | 'POLAR_ZX' | POLAR_ZX, POLAR_ZY |  |
| `pole` | ENUM | 'PINCH' | PINCH, FAN |  |
| `seam` | BOOLEAN | False |  |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `clip_to_bounds` | BOOLEAN | False |  |  |
| `scale_to_bounds` | BOOLEAN | False |  |  |


`bpy.ops.uv.project_from_view` — **Project from View** — Project the UV vertices of the mesh as seen in current 3D view  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `orthographic` | BOOLEAN | False |  |  |
| `camera_bounds` | BOOLEAN | True |  |  |
| `correct_aspect` | BOOLEAN | True |  |  |
| `clip_to_bounds` | BOOLEAN | False |  |  |
| `scale_to_bounds` | BOOLEAN | False |  |  |


`bpy.ops.uv.reset` — **Reset** — Reset UV projection  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.lightmap_pack` — **Lightmap Pack** — Pack each face's UVs into the UV bounds  
flags: UNDO

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `PREF_CONTEXT` | ENUM | 'SEL_FACES' | SEL_FACES, ALL_FACES |  |
| `PREF_PACK_IN_ONE` | BOOLEAN | True |  |  |
| `PREF_NEW_UVLAYER` | BOOLEAN | False |  |  |
| `PREF_BOX_DIV` | INT | 12 | 1 .. 48 |  |
| `PREF_MARGIN_DIV` | FLOAT[NONE] | 0.1 | 0.001 .. 1 |  |


`bpy.ops.uv.follow_active_quads` — **Follow Active Quads** — Follow UVs from active quads along continuous face loops  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | ENUM | 'LENGTH_AVERAGE' | EVEN, LENGTH, LENGTH_AVERAGE |  |


`bpy.ops.uv.pack_islands` — **Pack Islands** — Transform all islands so that they fill up the UV/UDIM space as much as possible  
flags: REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `udim_source` | ENUM | 'CLOSEST_UDIM' | CLOSEST_UDIM, ACTIVE_UDIM, ORIGINAL_AABB, CUSTOM_REGION |  |
| `rotate` | BOOLEAN | True |  |  |
| `rotate_method` | ENUM | 'ANY' | ANY, CARDINAL, AXIS_ALIGNED, AXIS_ALIGNED_X, AXIS_ALIGNED_Y |  |
| `scale` | BOOLEAN | True |  |  |
| `merge_overlap` | BOOLEAN | False |  |  |
| `margin_method` | ENUM | 'SCALED' | SCALED, ADD, FRACTION |  |
| `margin` | FLOAT[FACTOR] | 0.001 | 0 .. 1 |  |
| `pin` | BOOLEAN | False |  |  |
| `pin_method` | ENUM | 'LOCKED' | SCALE, ROTATION, ROTATION_SCALE, LOCKED |  |
| `shape_method` | ENUM | 'CONCAVE' | CONCAVE, CONVEX, AABB |  |


`bpy.ops.uv.average_islands_scale` — **Average Islands Scale** — Average the size of separate UV islands, based on their area in 3D space  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `scale_uv` | BOOLEAN | False |  |  |
| `shear` | BOOLEAN | False |  |  |


`bpy.ops.uv.minimize_stretch` — **Minimize Stretch** — Reduce UV stretching by relaxing angles  
flags: UNDO, GRAB_CURSOR, BLOCKING, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `fill_holes` | BOOLEAN | True |  |  |
| `blend` | FLOAT[FACTOR] | 0 | 0 .. 1 |  |
| `iterations` | INT | 0 | 0 .. ∞ |  |


`bpy.ops.uv.seams_from_islands` — **Seams from Islands** — Set mesh seams according to island setup in the UV editor  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mark_seams` | BOOLEAN | True |  |  |
| `mark_sharp` | BOOLEAN | False |  |  |


`bpy.ops.uv.mark_seam` — **Mark Seam** — Mark selected UV edges as seams  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `clear` | BOOLEAN | False |  |  |


`bpy.ops.uv.pin` — **Pin** — Set/clear selected UV vertices as anchored between multiple unwrap operations  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `clear` | BOOLEAN | False |  | SKIP_SAVE |
| `invert` | BOOLEAN | False |  | SKIP_SAVE |


`bpy.ops.uv.select_all` — **(De)select All** — Change selection of all UV vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `action` | ENUM | 'TOGGLE' | TOGGLE, SELECT, DESELECT, INVERT | HIDDEN SKIP_SAVE |


`bpy.ops.uv.select_mode` — **UV Select Mode** — Change UV selection mode  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'VERTEX' | VERTEX, EDGE, FACE | HIDDEN SKIP_SAVE |


`bpy.ops.uv.select_linked` — **Select Linked** — Select all UV vertices linked to the active UV map  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `delimit` | ENUM | {} | FLAG: SEAM, SHARP, MATERIAL |  |


`bpy.ops.uv.select_more` — **Select More** — Select more UV vertices connected to initial selection  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.select_less` — **Select Less** — Deselect UV vertices at the boundary of each selection region  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.select_split` — **Select Split** — Select only entirely selected faces  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.select_overlap` — **Select Overlap** — Select all UV faces which overlap each other  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.uv.select_similar` — **Select Similar** — Select similar UVs by property types  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `type` | ENUM | 'PIN' | PIN, LENGTH, LENGTH_3D, AREA, AREA_3D, MATERIAL, OBJECT, SIDES, WINDING, FACE |  |
| `compare` | ENUM | 'EQUAL' | EQUAL, GREATER, LESS |  |
| `threshold` | FLOAT[NONE] | 0 | 0 .. 1 |  |


`bpy.ops.uv.select_pinned` — **Selected Pinned** — Select all pinned UV vertices  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.select_by_winding` — **Select by Winding** — Select UV faces by their winding  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `winding` | ENUM | 'NEGATIVE' | POSITIVE, NEGATIVE |  |
| `extend` | BOOLEAN | False |  |  |


`bpy.ops.uv.weld` — **Weld** — Weld selected UV vertices together  
flags: UNDO, REGISTER

_(no properties)_


`bpy.ops.uv.remove_doubles` — **Merge UVs by Distance** — Selected UV vertices that are within a radius of each other are welded together  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `threshold` | FLOAT[NONE] | 0.02 | 0 .. 10 |  |
| `use_unselected` | BOOLEAN | False |  |  |
| `use_shared_vertex` | BOOLEAN | False |  |  |


`bpy.ops.uv.stitch` — **Stitch** — Stitch selected UV vertices by proximity  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `use_limit` | BOOLEAN | False |  |  |
| `snap_islands` | BOOLEAN | True |  |  |
| `limit` | FLOAT[NONE] | 0.01 | 0 .. ∞ |  |
| `static_island` | INT | 0 | 0 .. ∞ |  |
| `active_object_index` | INT | 0 | 0 .. ∞ |  |
| `midpoint_snap` | BOOLEAN | False |  |  |
| `clear_seams` | BOOLEAN | True |  |  |
| `mode` | ENUM | 'VERTEX' | VERTEX, EDGE |  |
| `stored_mode` | ENUM | 'VERTEX' | VERTEX, EDGE | HIDDEN |
| `selection` | COLLECTION |  | SelectedUvElement | HIDDEN |
| `objects_selection_count` | INT×6 | (0, 0, 0, 0, 0, 0) | 0 .. ∞ | HIDDEN |


`bpy.ops.uv.align` — **Align** — Aligns selected UV vertices on a line  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `axis` | ENUM | 'ALIGN_AUTO' | ALIGN_S, ALIGN_T, ALIGN_U, ALIGN_AUTO, ALIGN_X, ALIGN_Y |  |
| `position_mode` | ENUM | 'MEAN' | MEAN, MIN, MAX |  |


`bpy.ops.uv.align_rotation` — **Align Rotation** — Align the UV island's rotation  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `method` | ENUM | 'AUTO' | AUTO, EDGE, GEOMETRY |  |
| `axis` | ENUM | 'X' | X, Y, Z |  |
| `correct_aspect` | BOOLEAN | False |  |  |


`bpy.ops.uv.arrange_islands` — **Arrange/Align Islands** — Arrange selected UV islands on a line  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `initial_position` | ENUM | 'BOUNDING_BOX' | BOUNDING_BOX, UV_GRID, ACTIVE_UDIM, CURSOR |  |
| `axis` | ENUM | 'Y' | X, Y |  |
| `align` | ENUM | 'MIN' | MIN, MAX, CENTER, NONE |  |
| `order` | ENUM | 'LARGE_TO_SMALL' | LARGE_TO_SMALL, SMALL_TO_LARGE, Fixed |  |
| `margin` | FLOAT[NONE] | 0.05 | 0 .. 1 |  |


`bpy.ops.uv.randomize_uv_transform` — **Randomize** — Randomize the UV island's location, rotation, and scale  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `random_seed` | INT | 0 | 0 .. 10000 |  |
| `use_loc` | BOOLEAN | True |  |  |
| `loc` | FLOAT[TRANSLATION,LENGTH]×2 | (0, 0) | -100 .. 100 |  |
| `use_rot` | BOOLEAN | True |  |  |
| `rot` | FLOAT[ANGLE,ROTATION] | 0 | -6.28319 .. 6.28319 |  |
| `use_scale` | BOOLEAN | True |  |  |
| `scale_even` | BOOLEAN | False |  |  |
| `scale` | FLOAT[NONE]×2 | (1, 1) | -100 .. 100 |  |


`bpy.ops.uv.copy_mirrored_faces` — **Copy Mirrored UV Coords** — Copy mirror UV coordinates based on a mirrored mesh  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mesh_axis` | ENUM | 'POS_X' | POS_X, POS_Y, POS_Z, NEG_X, NEG_Y, NEG_Z |  |
| `uv_axis` | ENUM | 'X' | X, Y |  |
| `precision` | INT | 3 | 1 .. 16 |  |


`bpy.ops.uv.snap_selected` — **Snap Selection** — Snap selected UV vertices to target type  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `target` | ENUM | 'PIXELS' | PIXELS, CURSOR, CURSOR_OFFSET, ADJACENT_UNSELECTED |  |


`bpy.ops.uv.hide` — **Hide Selected** — Hide (un)selected UV vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `unselected` | BOOLEAN | False |  |  |


`bpy.ops.uv.reveal` — **Reveal Hidden** — Reveal all hidden UV vertices  
flags: UNDO, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `select` | BOOLEAN | True |  |  |


`bpy.ops.uv.rip` — **Rip UVs** — Rip selected vertices or a selected region  
flags: UNDO, DEPENDS_ON_CURSOR, REGISTER

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mirror` | BOOLEAN | False |  | HIDDEN |
| `release_confirm` | BOOLEAN | False |  | HIDDEN |
| `use_accurate` | BOOLEAN | False |  | HIDDEN |
| `location` | FLOAT[XYZ]×2 | (0, 0) | -∞ .. ∞ |  |



### A.14 Mode

`bpy.ops.object.mode_set` — **Set Object Mode** — Sets the object interaction mode  
flags: 

| prop | type | default | range / items | notes |
|---|---|---|---|---|
| `mode` | ENUM | 'OBJECT' | OBJECT, EDIT, POSE, SCULPT, VERTEX_PAINT, WEIGHT_PAINT, TEXTURE_PAINT, PARTICLE_EDIT, EDIT_GPENCIL, SCULPT_GREASE_PENCIL, PAINT_GREASE_PENCIL, WEIGHT_GREASE_PENCIL, VERTEX_GREASE_PENCIL, SCULPT_CURVES | SKIP_SAVE |
| `toggle` | BOOLEAN | False |  | SKIP_SAVE |


`bpy.ops.object.editmode_toggle` — **Edit Mode** — Toggle object's edit mode  
flags: UNDO, REGISTER

_(no properties)_


## Appendix B — Raw experiment output (Blender 5.2.4, `-b --factory-startup`)

Every run starts with `fresh()`, which deletes all objects and meshes, puts the cursor at the origin, calls `mesh.primitive_<x>_add(**kw)`, enters EDIT mode and sets VERT select mode. Indices come from `bm.*.index_update()` (= the order written to the Mesh). The `history` entries are `(V|E|F, index)`. `active_face` is `bm.faces.active` (not set by these scripts). These outputs can seed frap's strict-order golden tests.
### B.1 Data model introspection (default startup cube, attributes after shading ops)

```
# (script line `list(modifier.keys())` on the Smooth by Angle NODES modifier raised:)
TypeError: bpy_struct.keys(): this type doesn't support IDProperties   <- evidence for §0.5
MESH_PROPS ['rna_type', 'name', 'name_full', 'id_type', 'session_uid', 'is_evaluated', 'original', 'users', 'use_fake_user', 'use_extra_user', 'is_embedded_data', 'is_linked_packed', 'is_missing', 'is_runtime_data', 'is_editable', 'tag', 'is_library_indirect', 'library', 'library_weak_reference', 'asset_data', 'override_library', 'preview', 'vertices', 'edges', 'loops', 'polygons', 'normals_domain', 'vertex_normals', 'polygon_normals', 'corner_normals', 'loop_triangles', 'loop_triangle_polygons', 'texture_mesh', 'uv_layers', 'uv_layer_clone', 'uv_layer_clone_index', 'uv_layer_stencil', 'uv_layer_stencil_index', 'vertex_colors', 'skin_vertices', 'attributes', 'color_attributes', 'remesh_voxel_size', 'remesh_voxel_adaptivity', 'use_remesh_fix_poles', 'use_remesh_preserve_volume', 'use_remesh_preserve_attributes', 'remesh_mode', 'use_mirror_x', 'use_mirror_y', 'use_mirror_z', 'use_mirror_vertex_groups', 'radial_symmetry', 'has_custom_normals', 'texco_mesh', 'shape_keys', 'use_auto_texspace', 'use_mirror_topology', 'use_paint_bone_selection', 'use_paint_mask', 'use_paint_mask_vertex', 'total_vert_sel', 'total_edge_sel', 'total_face_sel', 'is_editmode', 'animation_data', 'auto_texspace', 'texspace_location', 'texspace_size', 'materials', 'cycles']
ATTRS [('position', 'FLOAT_VECTOR', 'POINT', False, True), ('.select_vert', 'BOOLEAN', 'POINT', True, False), ('.edge_verts', 'INT32_2D', 'EDGE', True, True), ('.select_edge', 'BOOLEAN', 'EDGE', True, False), ('.select_poly', 'BOOLEAN', 'FACE', True, False), ('sharp_face', 'BOOLEAN', 'FACE', False, False), ('.uv_select_face', 'BOOLEAN', 'FACE', True, False), ('.corner_vert', 'INT', 'CORNER', True, True), ('.corner_edge', 'INT', 'CORNER', True, True), ('UVMap', 'FLOAT2', 'CORNER', False, False), ('.uv_select_vert', 'BOOLEAN', 'CORNER', True, False), ('.uv_select_edge', 'BOOLEAN', 'CORNER', True, False)]
UVL [('UVMap', True, True)]
COLOR    -1
VERTS [(1.0, 1.0, 1.0), (1.0, 1.0, -1.0), (1.0, -1.0, 1.0), (1.0, -1.0, -1.0), (-1.0, 1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, -1.0, -1.0)]
EDGES [(5, 7), (1, 5), (0, 1), (7, 6), (2, 3), (4, 5), (2, 6), (0, 2), (7, 3), (6, 4), (4, 0), (3, 1)]
POLYS [(0, 4, 6, 2), (3, 2, 6, 7), (7, 6, 4, 5), (5, 1, 3, 7), (1, 0, 2, 3), (5, 4, 0, 1)] [0, 4, 8, 12, 16, 20] [4, 4, 4, 4, 4, 4]
CORNER_VERTS [0, 4, 6, 2, 3, 2, 6, 7, 7, 6, 4, 5, 5, 1, 3, 7, 1, 0, 2, 3, 5, 4, 0, 1] CORNER_EDGES [10, 9, 6, 7, 4, 6, 3, 8, 3, 9, 5, 0, 1, 11, 8, 0, 2, 7, 4, 11, 5, 10, 2, 1]
UV [(0.625, 0.5), (0.875, 0.5), (0.875, 0.75), (0.625, 0.75), (0.375, 0.75), (0.625, 0.75), (0.625, 1.0), (0.375, 1.0), (0.375, 0.0), (0.625, 0.0), (0.625, 0.25), (0.375, 0.25), (0.125, 0.5), (0.375, 0.5), (0.375, 0.75), (0.125, 0.75), (0.375, 0.5), (0.625, 0.5), (0.625, 0.75), (0.375, 0.75), (0.375, 0.25), (0.625, 0.25), (0.625, 0.5), (0.375, 0.5)]
UVLAYER_PROPS ['rna_type', 'data', 'name', 'active', 'active_render', 'active_clone', 'uv', 'pin']
POLY_PROPS ['rna_type', 'vertices', 'loop_start', 'loop_total', 'material_index', 'select', 'hide', 'use_smooth', 'normal', 'center', 'area', 'index']
EDGE_PROPS ['rna_type', 'vertices', 'select', 'hide', 'use_seam', 'use_edge_sharp', 'is_loose', 'index']
VERT_PROPS ['rna_type', 'co', 'normal', 'select', 'hide', 'groups', 'index', 'undeformed_co']
LOOP_PROPS ['rna_type', 'vertex_index', 'edge_index', 'index', 'normal', 'tangent', 'bitangent_sign', 'bitangent']
ATTR_API ['new', 'remove', 'domain_size'] ['rna_type', 'active', 'active_index', 'active_color', 'active_color_index', 'render_color_index', 'default_color_name', 'active_color_name']
ATTR_NEW [('name', 'STRING', None), ('type', 'ENUM', ['FLOAT', 'INT', 'BOOLEAN', 'FLOAT_VECTOR', 'FLOAT_COLOR', 'QUATERNION', 'FLOAT4X4', 'STRING', 'INT8', 'INT16_2D', 'INT32_2D', 'FLOAT2', 'FLOAT4', 'BYTE_COLOR']), ('domain', 'ENUM', ['POINT', 'EDGE', 'FACE', 'CORNER', 'CURVE', 'INSTANCE', 'LAYER']), ('attribute', 'POINTER', None)]
MESH_FUNCS ['bl_system_properties_get', 'rename', 'evaluated_get', 'copy', 'asset_mark', 'asset_clear', 'asset_generate_preview', 'override_create', 'override_hierarchy_create', 'user_clear', 'user_remap', 'make_local', 'user_of_id', 'animation_data_create', 'animation_data_clear', 'update_tag', 'preview_ensure', 'transform', 'flip_normals', 'set_sharp_from_angle', 'split_faces', 'calc_tangents', 'free_tangents', 'calc_loop_triangles', 'calc_smooth_groups', 'normals_split_custom_set', 'normals_split_custom_set_from_vertices', 'update', 'update_gpu_tag', 'unit_test_compare', 'clear_geometry', 'validate', 'validate_material_indices', 'count_selected_items']
NORMALS_DOMAIN FACE ['normals_domain', 'vertex_normals', 'polygon_normals', 'corner_normals', 'has_custom_normals']
UVLAYERS_FUNCS ['new', 'remove']
UVNEW [('name', 'STRING', 'UVMap'), ('do_init', 'BOOLEAN', True), ('layer', 'POINTER', None)]
ATTRS_AFTER_UV2 [('position', 'FLOAT_VECTOR', 'POINT'), ('.select_vert', 'BOOLEAN', 'POINT'), ('.edge_verts', 'INT32_2D', 'EDGE'), ('.select_edge', 'BOOLEAN', 'EDGE'), ('.select_poly', 'BOOLEAN', 'FACE'), ('sharp_face', 'BOOLEAN', 'FACE'), ('.uv_select_face', 'BOOLEAN', 'FACE'), ('.corner_vert', 'INT', 'CORNER'), ('.corner_edge', 'INT', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('.uv_select_vert', 'BOOLEAN', 'CORNER'), ('.uv_select_edge', 'BOOLEAN', 'CORNER'), ('UV2', 'FLOAT2', 'CORNER')]
ATTRS_SMOOTH [('position', 'FLOAT_VECTOR', 'POINT'), ('.select_vert', 'BOOLEAN', 'POINT'), ('.edge_verts', 'INT32_2D', 'EDGE'), ('.select_edge', 'BOOLEAN', 'EDGE'), ('.select_poly', 'BOOLEAN', 'FACE'), ('.uv_select_face', 'BOOLEAN', 'FACE'), ('.corner_vert', 'INT', 'CORNER'), ('.corner_edge', 'INT', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('.uv_select_vert', 'BOOLEAN', 'CORNER'), ('.uv_select_edge', 'BOOLEAN', 'CORNER'), ('UV2', 'FLOAT2', 'CORNER')]
ATTRS_FLAT [('position', 'FLOAT_VECTOR', 'POINT'), ('.select_vert', 'BOOLEAN', 'POINT'), ('.edge_verts', 'INT32_2D', 'EDGE'), ('.select_edge', 'BOOLEAN', 'EDGE'), ('.select_poly', 'BOOLEAN', 'FACE'), ('.uv_select_face', 'BOOLEAN', 'FACE'), ('.corner_vert', 'INT', 'CORNER'), ('.corner_edge', 'INT', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('.uv_select_vert', 'BOOLEAN', 'CORNER'), ('.uv_select_edge', 'BOOLEAN', 'CORNER'), ('UV2', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE')]
ATTRS_SBA [('position', 'FLOAT_VECTOR', 'POINT'), ('.select_vert', 'BOOLEAN', 'POINT'), ('.edge_verts', 'INT32_2D', 'EDGE'), ('.select_edge', 'BOOLEAN', 'EDGE'), ('.select_poly', 'BOOLEAN', 'FACE'), ('.uv_select_face', 'BOOLEAN', 'FACE'), ('.corner_vert', 'INT', 'CORNER'), ('.corner_edge', 'INT', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('.uv_select_vert', 'BOOLEAN', 'CORNER'), ('.uv_select_edge', 'BOOLEAN', 'CORNER'), ('UV2', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE')] mods []
AUTO_SMOOTH mods [('Smooth by Angle', 'NODES', 'Smooth by Angle')]
Blender 5.2.4 LTS (hash cd5dc428cce1 built 2026-11-10 01:31:20)
```

### B.2 Primitive cube, subdivide, extrude family

```
## cube_add fresh: V=8 E=12 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
  cube attrs [('UVMap', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE'), ('position', 'FLOAT_VECTOR', 'POINT')] uv [(0.375, 0.0), (0.625, 0.0), (0.625, 0.25), (0.375, 0.25), (0.375, 0.25), (0.625, 0.25), (0.625, 0.5), (0.375, 0.5), (0.375, 0.5), (0.625, 0.5), (0.625, 0.75), (0.375, 0.75), (0.375, 0.75), (0.625, 0.75), (0.625, 1.0), (0.375, 1.0), (0.125, 0.5), (0.375, 0.5), (0.375, 0.75), (0.125, 0.75), (0.625, 0.5), (0.875, 0.5), (0.875, 0.75), (0.625, 0.75)]
## subdivide all cuts=1: V=26 E=48 F=24
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 0.0, -1.0), (-1.0, -1.0, 0.0), (-1.0, 0.0, 1.0), (-1.0, 1.0, 0.0), (0.0, 1.0, -1.0), (0.0, 1.0, 1.0), (1.0, 1.0, 0.0), (1.0, 0.0, -1.0), (1.0, 0.0, 1.0), (1.0, -1.0, 0.0), (0.0, -1.0, -1.0), (0.0, -1.0, 1.0), (0.0, 0.0, 1.0), (0.0, 0.0, -1.0), (0.0, -1.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0)]
  faces: [[25, 10, 3, 11], [24, 13, 7, 14], [23, 16, 5, 17], [22, 19, 1, 9], [21, 15, 4, 18], [20, 10, 1, 19], [16, 20, 19, 5], [7, 13, 20, 16], [13, 3, 10, 20], [8, 21, 18, 0], [2, 12, 21, 8], [12, 6, 15, 21], [18, 22, 9, 0], [4, 17, 22, 18], [17, 5, 19, 22], [15, 23, 17, 4], [6, 14, 23, 15], [14, 7, 16, 23], [12, 24, 14, 6], [2, 11, 24, 12], [11, 3, 13, 24], [8, 25, 11, 2], [0, 9, 25, 8], [9, 1, 10, 25]]
  edges: [[8, 0], [9, 1], [10, 3], [11, 2], [12, 2], [13, 7], [14, 6], [15, 6], [16, 5], [17, 4], [18, 4], [19, 1], [2, 8], [0, 9], [1, 10], [3, 11], [6, 12], [3, 13], [7, 14], [4, 15], [7, 16], [5, 17], [0, 18], [5, 19], [20, 19], [13, 20], [20, 16], [10, 20], [21, 18], [12, 21], [21, 8], [15, 21], [22, 9], [17, 22], [22, 18], [19, 22], [23, 17], [14, 23], [23, 15], [16, 23], [24, 14], [11, 24], [24, 12], [13, 24], [25, 11], [9, 25], [25, 8], [10, 25]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]
  active_face: None  history: []  select_mode: (True, False, False)
top face 5
## subdivide top face cuts=1 (ngon=True): V=13 E=20 F=9
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (0.0, -1.0, 1.0), (0.0, 0.0, 1.0)]
  faces: [[0, 1, 8, 3, 2], [2, 3, 9, 7, 6], [6, 7, 10, 5, 4], [4, 5, 11, 1, 0], [2, 6, 4, 0], [12, 8, 1, 11], [10, 12, 11, 5], [7, 9, 12, 10], [9, 3, 8, 12]]
  edges: [[2, 0], [0, 1], [8, 3], [3, 2], [6, 2], [9, 7], [7, 6], [4, 6], [10, 5], [5, 4], [0, 4], [11, 1], [1, 8], [3, 9], [7, 10], [5, 11], [12, 11], [9, 12], [12, 10], [8, 12]]
  sel V: [1, 3, 5, 7, 8, 9, 10, 11, 12]  E: [2, 5, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19]  F: [5, 6, 7, 8]
  active_face: None  history: []  select_mode: (True, False, False)
## subdivide top face ngon=False: V=13 E=28 F=17
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (0.0, -1.0, 1.0), (0.0, 0.0, 1.0)]
  faces: [[3, 2, 8], [7, 6, 9], [5, 4, 10], [1, 0, 11], [2, 6, 4, 0], [12, 8, 1, 11], [10, 12, 11, 5], [7, 9, 12, 10], [9, 3, 8, 12], [4, 5, 11], [0, 4, 11], [6, 7, 10], [4, 6, 10], [2, 3, 9], [6, 2, 9], [0, 1, 8], [2, 0, 8]]
  edges: [[2, 0], [0, 1], [8, 3], [3, 2], [6, 2], [9, 7], [7, 6], [4, 6], [10, 5], [5, 4], [0, 4], [11, 1], [1, 8], [3, 9], [7, 10], [5, 11], [12, 11], [9, 12], [12, 10], [8, 12], [11, 4], [11, 0], [10, 6], [10, 4], [9, 2], [9, 6], [8, 0], [8, 2]]
  sel V: [1, 3, 5, 7, 8, 9, 10, 11, 12]  E: [2, 5, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19]  F: [5, 6, 7, 8]
  active_face: None  history: []  select_mode: (True, False, False)
## subdivide top face cuts=2: V=20 E=32 F=14
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -0.3333, 1.0), (-1.0, 0.3333, 1.0), (-0.3333, 1.0, 1.0), (0.3333, 1.0, 1.0), (1.0, 0.3333, 1.0), (1.0, -0.3333, 1.0), (0.3333, -1.0, 1.0), (-0.3333, -1.0, 1.0), (0.3333, 0.3333, 1.0), (0.3333, -0.3333, 1.0), (-0.3333, 0.3333, 1.0), (-0.3333, -0.3333, 1.0)]
  faces: [[0, 1, 8, 9, 3, 2], [2, 3, 10, 11, 7, 6], [6, 7, 12, 13, 5, 4], [4, 5, 14, 15, 1, 0], [2, 6, 4, 0], [19, 8, 1, 15], [13, 17, 14, 5], [17, 19, 15, 14], [7, 11, 16, 12], [12, 16, 17, 13], [11, 10, 18, 16], [16, 18, 19, 17], [10, 3, 9, 18], [18, 9, 8, 19]]
  edges: [[2, 0], [0, 1], [9, 3], [3, 2], [6, 2], [11, 7], [7, 6], [4, 6], [13, 5], [5, 4], [0, 4], [15, 1], [1, 8], [8, 9], [3, 10], [10, 11], [7, 12], [12, 13], [5, 14], [14, 15], [17, 14], [11, 16], [16, 17], [19, 15], [10, 18], [18, 19], [16, 12], [17, 13], [18, 16], [19, 17], [9, 18], [8, 19]]
  sel V: [1, 3, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]  E: [2, 5, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]  F: [5, 6, 7, 8, 9, 10, 11, 12, 13]
  active_face: None  history: []  select_mode: (True, False, False)
## extrude_region_move top +z1: V=12 E=20 F=10
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, 2.0), (-1.0, 1.0, 2.0), (1.0, -1.0, 2.0), (1.0, 1.0, 2.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [5, 7, 11, 10], [11, 9, 8, 10], [3, 1, 8, 9], [1, 5, 10, 8], [7, 3, 9, 11]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [8, 9], [9, 11], [11, 10], [10, 8], [7, 11], [10, 5], [1, 8], [9, 3]]
  sel V: [8, 9, 10, 11]  E: [12, 13, 14, 15]  F: [6]
  active_face: None  history: [('F', 6)]  select_mode: (True, False, False)
## extrude_region_shrink_fatten 0.5: V=12 E=20 F=10
  sel V: [8, 9, 10, 11]  E: [12, 13, 14, 15]  F: [6]
  active_face: None  history: [('F', 6)]  select_mode: (True, False, False)
  zmax 1.5
## extrude_region two adjacent faces: V=14 E=24 F=12
  sel V: [8, 9, 10, 11, 12, 13]  E: [12, 13, 14, 15, 16, 17, 18]  F: [6, 7]
  active_face: None  history: [('F', 6), ('F', 7)]  select_mode: (True, False, False)
## extrude_faces_move two faces: V=16 E=28 F=14
  sel V: [8, 9, 10, 11, 12, 13, 14, 15]  E: [12, 13, 14, 15, 20, 21, 22, 23]  F: [4, 9]
  active_face: None  history: []  select_mode: (True, False, False)
## extrude_context_move two faces: V=14 E=24 F=12
  sel V: [8, 9, 10, 11, 12, 13]  E: [12, 13, 14, 15, 16, 17, 18]  F: [6, 7]
  active_face: None  history: [('F', 6), ('F', 7)]  select_mode: (True, False, False)
## extrude_edges_move edge0: V=10 E=15 F=7
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, 0.0), (-1.0, 1.0, 0.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5], [2, 0, 8, 9]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [9, 8], [9, 2], [0, 8]]
  sel V: [8, 9]  E: [12]  F: []
  active_face: None  history: [('E', 12)]  select_mode: (False, True, False)
## extrude_vertices_move v0: V=9 E=13 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, 0.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [0, 8]]
  sel V: [8]  E: []  F: []
  active_face: None  history: [('V', 8)]  select_mode: (True, False, False)
## extrude_region_move single vert: V=9 E=13 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, 0.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [0, 8]]
  sel V: [8]  E: []  F: []
  active_face: None  history: [('V', 8)]  select_mode: (True, False, False)
## extrude_region (no move): V=12 E=20 F=10
  sel V: [8, 9, 10, 11]  E: [12, 13, 14, 15]  F: [6]
  active_face: None  history: [('F', 6)]  select_mode: (True, False, False)
## extrude_repeat: V=20 E=36 F=18
  sel V: [16, 17, 18, 19]  E: [28, 29, 30, 31]  F: [14]
  active_face: None  history: [('F', 14)]  select_mode: (True, False, False)
## extrude_manifold inward: V=8 E=12 F=6
  sel V: [4, 5, 6, 7]  E: [8, 9, 10, 11]  F: [5]
  active_face: None  history: [('F', 5)]  select_mode: (True, False, False)
```

### B.3 Inset, bevel, edge ring + subdivide

```
## inset top 0.2: V=12 E=20 F=10
  verts: [(-1.0, -1.0, -1.0), (-0.8, -0.8, 1.0), (-1.0, 1.0, -1.0), (-0.8, 0.8, 1.0), (1.0, -1.0, -1.0), (0.8, -0.8, 1.0), (1.0, 1.0, -1.0), (0.8, 0.8, 1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, 1.0, 1.0), (1.0, -1.0, 1.0)]
  faces: [[0, 8, 9, 2], [2, 9, 10, 6], [6, 10, 11, 4], [4, 11, 8, 0], [2, 6, 4, 0], [7, 3, 1, 5], [1, 3, 9, 8], [3, 7, 10, 9], [7, 5, 11, 10], [5, 1, 8, 11]]
  edges: [[2, 0], [0, 8], [8, 9], [9, 2], [6, 2], [9, 10], [10, 6], [4, 6], [10, 11], [11, 4], [0, 4], [11, 8], [1, 3], [3, 7], [7, 5], [5, 1], [8, 1], [3, 9], [7, 10], [5, 11]]
  sel V: [1, 3, 5, 7]  E: [12, 13, 14, 15]  F: [5]
  active_face: None  history: []  select_mode: (True, False, False)
## inset top 0.2 depth .3: V=12 E=20 F=10
  sel V: [1, 3, 5, 7]  E: [12, 13, 14, 15]  F: [5]
  active_face: None  history: []  select_mode: (True, False, False)
  zmax 1.2999999523162842
## inset region two faces: V=14 E=24 F=12
  sel V: [1, 3, 4, 6, 7, 13]  E: [8, 12, 13, 14, 15, 16, 17]  F: [2, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## inset individual two faces: V=16 E=28 F=14
  sel V: [8, 9, 10, 11, 12, 13, 14, 15]  E: [12, 13, 14, 15, 20, 21, 22, 23]  F: [2, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## inset use_select_inset: V=14 E=24 F=12
  sel V: [1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]  E: [2, 5, 6, 7, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]  F: [6, 7, 8, 9, 10, 11]
  active_face: None  history: []  select_mode: (True, False, False)
## bevel edge0 0.2 seg1: V=10 E=15 F=7
  verts: [(-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, -0.8), (-0.8, -1.0, -1.0), (-0.8, 1.0, -1.0), (-1.0, 1.0, -0.8)]
  faces: [[4, 5, 3, 2], [5, 1, 0, 3], [8, 7, 6, 9], [8, 4, 2, 7], [6, 0, 1, 9], [2, 3, 0, 6, 7], [8, 9, 1, 5, 4]]
  edges: [[0, 1], [1, 5], [5, 4], [2, 4], [5, 3], [3, 2], [3, 0], [9, 8], [8, 7], [7, 6], [6, 9], [8, 4], [2, 7], [6, 0], [1, 9]]
  sel V: [6, 7, 8, 9]  E: [7, 8, 9, 10]  F: [2]
  active_face: None  history: []  select_mode: (False, True, False)
## bevel edge0 0.2 seg3: V=14 E=21 F=9
  sel V: [6, 7, 8, 9, 10, 11, 12, 13]  E: [7, 8, 9, 10, 11, 12, 13, 14, 15, 16]  F: [2, 3, 4]
  active_face: None  history: []  select_mode: (False, True, False)
## bevel all edges seg1: V=24 E=48 F=26
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25]
  active_face: None  history: []  select_mode: (True, False, False)
## bevel all edges seg2: V=56 E=108 F=54
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55]  E: 108 sel  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53]
  active_face: None  history: []  select_mode: (True, False, False)
## bevel vert0: V=10 E=15 F=7
  verts: [(-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-0.8, -1.0, -1.0), (-1.0, -1.0, -0.8), (-1.0, -0.8, -1.0)]
  faces: [[1, 2, 6, 5], [5, 6, 4, 3], [6, 2, 0, 4], [7, 8, 9], [1, 5, 3, 7, 9], [9, 8, 0, 2, 1], [3, 4, 0, 8, 7]]
  edges: [[0, 2], [2, 1], [5, 1], [2, 6], [6, 5], [3, 5], [6, 4], [4, 3], [4, 0], [9, 7], [7, 8], [8, 9], [9, 1], [3, 7], [8, 0]]
  sel V: [7, 8, 9]  E: [9, 10, 11]  F: [3]
  active_face: None  history: []  select_mode: (True, False, False)
## bevel pct: V=10 E=15 F=7
  sel V: [6, 7, 8, 9]  E: [7, 8, 9, 10]  F: [2]
  active_face: None  history: []  select_mode: (False, True, False)
## edge ring multi: V=8 E=12 F=6
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 2, 7, 8]  F: []
  active_face: None  history: [('E', 0)]  select_mode: (False, True, False)
## ring + subdivide (=loopcut eq): V=12 E=20 F=10
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 0.0, -1.0), (-1.0, 0.0, 1.0), (1.0, 0.0, -1.0), (1.0, 0.0, 1.0)]
  faces: [[8, 9, 3, 2], [2, 3, 7, 6], [10, 11, 5, 4], [4, 5, 1, 0], [8, 10, 4, 0], [11, 9, 1, 5], [7, 3, 9, 11], [2, 6, 10, 8], [6, 7, 11, 10], [0, 1, 9, 8]]
  edges: [[8, 0], [0, 1], [9, 3], [3, 2], [6, 2], [3, 7], [7, 6], [10, 6], [11, 5], [5, 4], [0, 4], [5, 1], [2, 8], [1, 9], [4, 10], [7, 11], [9, 11], [10, 8], [11, 10], [9, 8]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
  active_face: None  history: [('E', 0)]  select_mode: (False, True, False)
## ring + subdivide_edgering: V=12 E=20 F=10
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 2, 7, 8, 12, 13, 14, 15]  F: []
  active_face: None  history: [('E', 0)]  select_mode: (False, True, False)
```

### B.4 Attributes, normals, split/separate, triangulate/poke/smooth/solidify/symmetrize/spin, selection ops

```
## seam+sharp attrs [('UVMap', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE'), ('uv_seam', 'BOOLEAN', 'EDGE'), ('sharp_face', 'BOOLEAN', 'FACE')]
## crease+bevelweight attrs [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE'), ('uv_seam', 'BOOLEAN', 'EDGE'), ('sharp_face', 'BOOLEAN', 'FACE')]
## after clear [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE')]
## faces_shade_smooth [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('UVMap', 'FLOAT2', 'CORNER')]
## set_sharpness_by_angle [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('custom_normal', 'INT16_2D', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE')]
## custom_splitnormals_add [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('custom_normal', 'INT16_2D', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE')]
   has_custom_normals True CORNER
   custom_normal sample [(0, 0), (0, 0), (0, 0)]
## custom_splitnormals_clear [('crease_edge', 'FLOAT', 'EDGE'), ('bevel_weight_edge', 'FLOAT', 'EDGE'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_edge', 'BOOLEAN', 'EDGE')]
## set_normals_from_faces [('custom_normal', 'INT16_2D', 'CORNER'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE')]
## uv_texture_add [('UVMap', 'FLOAT2', 'CORNER'), ('UVMap.001', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE')]
## color_attribute_add default [('Attribute', 'FLOAT_COLOR', 'POINT'), ('UVMap', 'FLOAT2', 'CORNER'), ('sharp_face', 'BOOLEAN', 'FACE')] Attribute
   active_color/default/render Attribute Attribute 0
## vertex_colors.new [('Col', 'BYTE_COLOR', 'CORNER')]
## flip_normals top: V=8 E=12 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 5, 1, 3]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11]  F: [5]
  active_face: None  history: [('F', 5)]  select_mode: (True, False, False)
## normals_make_consistent: V=8 E=12 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: [('F', 5)]  select_mode: (True, False, False)
## split top face: V=12 E=16 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, 1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [11, 9, 8, 10]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [8, 9], [9, 11], [11, 10], [10, 8]]
  sel V: [8, 9, 10, 11]  E: [12, 13, 14, 15]  F: [5]
  active_face: None  history: []  select_mode: (True, False, False)
## separate objs [('Cube', 8, 6, True), ('Cube.001', 4, 1, True)] Cube
## after separate: V=8 E=12 F=5
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## quads_convert_to_tris: V=8 E=18 F=12
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[1, 2, 0], [3, 6, 2], [7, 4, 6], [5, 0, 4], [6, 0, 2], [3, 5, 7], [1, 3, 2], [3, 7, 6], [7, 5, 4], [5, 1, 0], [6, 4, 0], [3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [2, 1], [6, 3], [4, 7], [0, 5], [0, 6], [5, 3]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
  active_face: None  history: []  select_mode: (True, False, False)
## tris_convert_to_quads: V=8 E=12 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 2, 6, 4], [4, 6, 7, 5], [2, 0, 1, 3], [0, 4, 5, 1], [6, 2, 3, 7], [5, 7, 3, 1]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## poke top: V=9 E=16 F=9
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (0.0, 0.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 8], [3, 1, 8], [1, 5, 8], [5, 7, 8]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [8, 7], [3, 8], [1, 8], [5, 8]]
  sel V: [1, 3, 5, 7, 8]  E: [2, 5, 8, 11, 12, 13, 14, 15]  F: [5, 6, 7, 8]
  active_face: None  history: []  select_mode: (True, False, False)
## vertices_smooth .5: V=8 E=12 F=6
  verts: [(-0.6667, -0.6667, -0.6667), (-0.6667, -0.6667, 0.6667), (-0.6667, 0.6667, -0.6667), (-0.6667, 0.6667, 0.6667), (0.6667, -0.6667, -0.6667), (0.6667, -0.6667, 0.6667), (0.6667, 0.6667, -0.6667), (0.6667, 0.6667, 0.6667)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [7, 3, 1, 5]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## solidify .1: V=16 E=24 F=12
  sel V: [8, 9, 10, 11, 12, 13, 14, 15]  E: [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]  F: [6, 7, 8, 9, 10, 11]
  active_face: None  history: []  select_mode: (True, False, False)
## symmetrize monkey: V=507 E=1005 F=500
  sel V: 507 sel  E: 1005 sel  F: 500 sel
  active_face: None  history: []  select_mode: (True, False, False)
## unsubdivide: V=8 E=12 F=6
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## spin plane: V=20 E=36 F=18
  sel V: [16, 17, 18, 19]  E: [28, 29, 30, 31]  F: [14]
  active_face: None  history: []  select_mode: (True, False, False)
Error: You have to select a string of connected vertices too
## screw EXC Error: You have to select a string of connected vertices too
Error: Invalid/unset axis
## spin default EXC Error: Invalid/unset axis
## duplicate_move: V=16 E=24 F=12
  sel V: [8, 9, 10, 11, 12, 13, 14, 15]  E: [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]  F: [6, 7, 8, 9, 10, 11]
  active_face: None  history: []  select_mode: (True, False, False)
## edge_split: V=8 E=12 F=6
  sel V: [0, 2]  E: [0]  F: []
  active_face: None  history: []  select_mode: (False, True, False)
## wireframe: V=40 E=96 F=48
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39]  E: 96 sel  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]
  active_face: None  history: []  select_mode: (True, False, False)
## select_random .5 seed0: V=482 E=992 F=512
  sel V: 241 sel  E: 267 sel  F: [1, 6, 29, 37, 40, 60, 61, 63, 64, 66, 68, 69, 87, 88, 89, 91, 99, 103, 122, 126, 139, 152, 156, 160, 214, 236, 237, 244, 245, 267, 270, 275, 278, 308, 320, 321, 334, 344, 346, 348, 353, 355, 361, 367, 376, 378, 385, 399, 418, 422, 445, 469, 484, 493, 498, 500, 511]
  active_face: None  history: []  select_mode: (True, False, False)
## select_random .5 seed0 again: V=482 E=992 F=512
  sel V: 241 sel  E: 267 sel  F: [1, 6, 29, 37, 40, 60, 61, 63, 64, 66, 68, 69, 87, 88, 89, 91, 99, 103, 122, 126, 139, 152, 156, 160, 214, 236, 237, 244, 245, 267, 270, 275, 278, 308, 320, 321, 334, 344, 346, 348, 353, 355, 361, 367, 376, 378, 385, 399, 418, 422, 445, 469, 484, 493, 498, 500, 511]
  active_face: None  history: []  select_mode: (True, False, False)
## select_more grid center: V=36 E=60 F=25
  sel V: [6, 7, 12, 13, 18, 19]  E: [3, 16, 17, 18, 27, 28, 29]  F: [5, 10]
  active_face: None  history: [('V', 12)]  select_mode: (True, False, False)
## select_less: V=36 E=60 F=25
  sel V: [12]  E: []  F: []
  active_face: None  history: [('V', 12)]  select_mode: (True, False, False)
## select_nth all (no active): V=36 E=60 F=25
  sel V: [1, 3, 5, 6, 8, 10, 13, 15, 17, 18, 20, 22, 25, 27, 29, 30, 32, 34]  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## select_non_manifold grid: V=36 E=60 F=25
  sel V: [0, 1, 2, 3, 4, 5, 6, 11, 12, 17, 18, 23, 24, 29, 30, 31, 32, 33, 34, 35]  E: [0, 1, 4, 7, 10, 13, 14, 16, 25, 27, 36, 38, 47, 49, 51, 53, 55, 57, 58, 59]  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## select_face_by_sides 4: V=36 E=60 F=25
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35]  E: 60 sel  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]
  active_face: None  history: []  select_mode: (False, False, True)
## select_linked: V=8 E=12 F=6
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: [('V', 0)]  select_mode: (True, False, False)
## select_all invert from all: V=8 E=12 F=6
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## edges_select_sharp 30deg: V=8 E=12 F=6
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (False, True, False)
## select_loose: V=8 E=12 F=6
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## face->edge mode switch: V=8 E=12 F=6
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11]  F: [5]
  active_face: None  history: []  select_mode: (False, True, False)
## edge->face mode: V=8 E=12 F=6
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11]  F: [5]
  active_face: None  history: []  select_mode: (False, False, True)
## 2 verts -> face mode (no full face): V=8 E=12 F=6
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (False, False, True)
## select_mode extend: V=8 E=12 F=6
  sel V: [0]  E: []  F: []
  active_face: None  history: [('V', 0)]  select_mode: (True, True, False)
```

### B.5 UV operators (cube)

```
ret {'FINISHED'}
## smart_project defaults cube: layers=['UVMap'] active=UVMap
   uv: [(0.3333, 0.3333), (0.0, 0.3333), (0.0, 0.0), (0.3333, 0.0), (0.3333, 0.6667), (0.0, 0.6667), (0.0, 0.3333), (0.3333, 0.3333), (0.6667, 0.3333), (0.3333, 0.3333), (0.3333, 0.0), (0.6667, 0.0), (0.6667, 0.6667), (0.3333, 0.6667), (0.3333, 0.3333), (0.6667, 0.3333), (0.3333, 0.6667), (0.3333, 1.0), (0.0, 1.0), (0.0, 0.6667), (0.6667, 0.6667), (0.6667, 1.0), (0.3333, 1.0), (0.3333, 0.6667)] 
   seams: []
Warning: Unwrap failed to solve 1 of 1 island(s), edge seams may need to be added
## unwrap defaults cube (no seams): layers=['UVMap'] active=UVMap
   uv: [(0.25, 0.0), (0.5, 0.0), (0.5, 0.25), (0.25, 0.25), (0.25, 0.25), (0.5, 0.25), (0.5, 0.5), (0.25, 0.5), (0.25, 0.5), (0.5, 0.5), (0.5, 0.75), (0.25, 0.75), (0.25, 0.75), (0.5, 0.75), (0.5, 1.0), (0.25, 1.0), (0.0, 0.5), (0.25, 0.5), (0.25, 0.75), (0.0, 0.75), (0.5, 0.5), (0.75, 0.5), (0.75, 0.75), (0.5, 0.75)] 
   seams: []
## unwrap ANGLE_BASED seams: layers=['UVMap'] active=UVMap
   uv: [(0.5, 0.75), (0.75, 0.75), (0.75, 0.9999), (0.5, 0.9999), (0.25, 0.75), (0.0001, 0.75), (0.0001, 0.5), (0.25, 0.5), (0.25, 0.5), (0.25, 0.25), (0.5, 0.25), (0.5, 0.5), (0.5, 0.5), (0.75, 0.5), (0.75, 0.75), (0.5, 0.75), (0.25, 0.75), (0.25, 0.5), (0.5, 0.5), (0.5, 0.75), (0.25, 0.25), (0.25, 0.0001), (0.5, 0.0001), (0.5, 0.25)] 
   seams: [0, 2, 3, 5, 6, 9, 11]
## unwrap CONFORMAL seams: layers=['UVMap'] active=UVMap
   uv: [(0.5, 0.75), (0.75, 0.75), (0.75, 0.9999), (0.5, 0.9999), (0.25, 0.75), (0.0001, 0.75), (0.0001, 0.5), (0.25, 0.5), (0.25, 0.5), (0.25, 0.25), (0.5, 0.25), (0.5, 0.5), (0.5, 0.5), (0.75, 0.5), (0.75, 0.75), (0.5, 0.75), (0.25, 0.75), (0.25, 0.5), (0.5, 0.5), (0.5, 0.75), (0.25, 0.25), (0.25, 0.0001), (0.5, 0.0001), (0.5, 0.25)] 
   seams: [0, 2, 3, 5, 6, 9, 11]
## unwrap MINIMUM_STRETCH seams: layers=['UVMap'] active=UVMap
   uv: [(0.5365, 0.7497), (0.7866, 0.7442), (0.7856, 0.9999), (0.5344, 0.9999), (0.2985, 0.7872), (0.0582, 0.8426), (0.0001, 0.5993), (0.2518, 0.5348), (0.2518, 0.5348), (0.226, 0.2839), (0.4775, 0.2517), (0.5092, 0.5034), (0.5092, 0.5034), (0.7692, 0.4867), (0.7866, 0.7442), (0.5365, 0.7497), (0.2985, 0.7872), (0.2518, 0.5348), (0.5092, 0.5034), (0.5365, 0.7497), (0.226, 0.2839), (0.1927, 0.0343), (0.4426, 0.0001), (0.4775, 0.2517)] 
   seams: [0, 2, 3, 5, 6, 9, 11]
## cube_project: layers=['UVMap'] active=UVMap
   uv: [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)] 
   seams: []
## cylinder_project: layers=['UVMap'] active=UVMap
   uv: [(0.125, 0.0), (0.375, 0.0), (0.375, 1.0), (0.125, 1.0), (0.125, 1.0), (0.375, 1.0), (0.625, 1.0), (0.875, 1.0), (0.875, 1.0), (0.625, 1.0), (0.625, 0.0), (0.875, 0.0), (0.875, 0.0), (0.625, 0.0), (0.375, 0.0), (0.125, 0.0), (1.125, 1.0), (0.875, 1.0), (0.875, 0.0), (1.125, 0.0), (0.625, 1.0), (0.375, 1.0), (0.375, 0.0), (0.625, 0.0)] 
   seams: []
## sphere_project: layers=['UVMap'] active=UVMap
   uv: [(0.125, 0.3041), (0.375, 0.3041), (0.375, 0.6959), (0.125, 0.6959), (0.125, 0.6959), (0.375, 0.6959), (0.625, 0.6959), (0.875, 0.6959), (0.875, 0.6959), (0.625, 0.6959), (0.625, 0.3041), (0.875, 0.3041), (0.875, 0.3041), (0.625, 0.3041), (0.375, 0.3041), (0.125, 0.3041), (1.125, 0.6959), (0.875, 0.6959), (0.875, 0.3041), (1.125, 0.3041), (0.625, 0.6959), (0.375, 0.6959), (0.375, 0.3041), (0.625, 0.3041)] 
   seams: []
## project_from_view: EXC Operator bpy.ops.uv.project_from_view.poll() failed, context is incorrect
## reset: layers=['UVMap'] active=UVMap
   uv: [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] 
   seams: []
## smart+pack_islands: layers=['UVMap'] active=UVMap
   uv: [(0.002, 0.6647), (0.002, 0.3353), (0.3314, 0.3353), (0.3314, 0.6647), (0.6647, 0.002), (0.6647, 0.3314), (0.3353, 0.3314), (0.3353, 0.002), (0.6647, 0.998), (0.3353, 0.998), (0.3353, 0.6686), (0.6647, 0.6686), (0.3314, 0.3314), (0.002, 0.3314), (0.002, 0.002), (0.3314, 0.002), (0.3353, 0.3353), (0.6647, 0.3353), (0.6647, 0.6647), (0.3353, 0.6647), (0.3314, 0.6686), (0.3314, 0.998), (0.002, 0.998), (0.002, 0.6686)] 
   seams: []
## cube+average_islands_scale: layers=['UVMap'] active=UVMap
   uv: [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)] 
   seams: []
## smart+seams_from_islands: layers=['UVMap'] active=UVMap
   uv: [(0.3333, 0.3333), (0.0, 0.3333), (0.0, 0.0), (0.3333, 0.0), (0.3333, 0.6667), (0.0, 0.6667), (0.0, 0.3333), (0.3333, 0.3333), (0.6667, 0.3333), (0.3333, 0.3333), (0.3333, 0.0), (0.6667, 0.0), (0.6667, 0.6667), (0.3333, 0.6667), (0.3333, 0.3333), (0.6667, 0.3333), (0.3333, 0.6667), (0.3333, 1.0), (0.0, 1.0), (0.0, 0.6667), (0.6667, 0.6667), (0.6667, 1.0), (0.3333, 1.0), (0.3333, 0.6667)] 
   seams: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
## lightmap_pack: layers=['UVMap'] active=UVMap
   uv: [(0.3323, 0.001), (0.001, 0.001), (0.001, 0.3323), (0.3323, 0.3323), (0.3323, 0.3343), (0.001, 0.3343), (0.001, 0.6657), (0.3323, 0.6657), (0.6657, 0.001), (0.3343, 0.001), (0.3343, 0.3323), (0.6657, 0.3323), (0.6657, 0.3343), (0.3343, 0.3343), (0.3343, 0.6657), (0.6657, 0.6657), (0.3323, 0.6677), (0.001, 0.6677), (0.001, 0.999), (0.3323, 0.999), (0.999, 0.001), (0.6677, 0.001), (0.6677, 0.3323), (0.999, 0.3323)] 
   seams: []
Error: No active face
## follow_active_quads (no active): EXC Error: No active face
## uv.mark_seam (mesh sel): layers=['UVMap'] active=UVMap
   uv: [(0.375, 0.0), (0.625, 0.0), (0.625, 0.25), (0.375, 0.25), (0.375, 0.25), (0.625, 0.25), (0.625, 0.5), (0.375, 0.5), (0.375, 0.5), (0.625, 0.5), (0.625, 0.75), (0.375, 0.75), (0.375, 0.75), (0.625, 0.75), (0.625, 1.0), (0.375, 1.0), (0.125, 0.5), (0.375, 0.5), (0.375, 0.75), (0.125, 0.75), (0.625, 0.5), (0.875, 0.5), (0.875, 0.75), (0.625, 0.75)] 
   seams: [0]
   sync True
## sync default: layers=['UVMap'] active=UVMap
   uv: [(0.375, 0.0), (0.625, 0.0), (0.625, 0.25), (0.375, 0.25), (0.375, 0.25), (0.625, 0.25), (0.625, 0.5), (0.375, 0.5), (0.375, 0.5), (0.625, 0.5), (0.625, 0.75), (0.375, 0.75), (0.375, 0.75), (0.625, 0.75), (0.625, 1.0), (0.375, 1.0), (0.125, 0.5), (0.375, 0.5), (0.375, 0.75), (0.125, 0.75), (0.625, 0.5), (0.875, 0.5), (0.875, 0.75), (0.625, 0.75)] 
   seams: []
```

### B.6 Mempool order, merge, delete, dissolve, fill

```
## mempool order after del 2,4 then add 100,200,300: [0.0, 1.0, 200.0, 3.0, 100.0, 5.0, 300.0]
## merge CENTER v0,v1,v3: V=6 E=9 F=5
  sel V: [0]  E: []  F: []
  active_face: None  history: [('V', 0)]  select_mode: (True, False, False)
  verts [(-1.0, -0.3333, 0.3333), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
## merge CURSOR v0,v1,v3: V=6 E=9 F=5
  sel V: [0]  E: []  F: []
  active_face: None  history: [('V', 0)]  select_mode: (True, False, False)
  verts [(0.0, 0.0, 0.0), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
## merge COLLAPSE v0,v1,v3: V=6 E=9 F=5
  sel V: [0]  E: []  F: []
  active_face: None  history: [('V', 0)]  select_mode: (True, False, False)
  verts [(-1.0, -0.5, 0.5), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
## merge FIRST v0,v1,v3: V=6 E=9 F=5
  sel V: [0]  E: []  F: []
  active_face: None  history: [('V', 0)]  select_mode: (True, False, False)
  verts [(-1.0, -1.0, -1.0), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
## merge LAST v0,v1,v3: V=6 E=9 F=5
  sel V: [1]  E: []  F: []
  active_face: None  history: [('V', 1)]  select_mode: (True, False, False)
  verts [(-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
Info: Removed 0 vertices
## remove_doubles result {'FINISHED'}
## delete VERT topface: V=4 E=4 F=1
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## delete EDGE topface: V=8 E=8 F=1
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## delete FACE topface: V=8 E=12 F=5
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## delete EDGE_FACE topface: V=8 E=8 F=1
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## delete ONLY_FACE topface: V=8 E=12 F=5
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## dissolve_faces single: V=8 E=12 F=6
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11]  F: [5]
  active_face: None  history: [('F', 5)]  select_mode: (True, False, False)
## dissolve_faces two adjacent: V=8 E=11 F=5
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [4, 5, 1, 0], [2, 6, 4, 0], [6, 7, 3, 1, 5, 4]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [5, 4], [0, 4], [5, 1]]
  sel V: [1, 3, 4, 5, 6, 7]  E: [2, 5, 6, 7, 8, 10]  F: [4]
  active_face: None  history: []  select_mode: (True, False, False)
## dissolve_limited subdivided cube: V=8 E=12 F=6
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[2, 3, 7, 6], [7, 3, 1, 5], [4, 5, 1, 0], [1, 3, 2, 0], [2, 6, 4, 0], [6, 7, 5, 4]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## dissolve_edges e0: V=6 E=9 F=5
  verts: [(-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[1, 5, 4], [4, 5, 3, 2], [2, 3, 0], [5, 1, 0, 3], [1, 4, 2, 0]]
  edges: [[0, 1], [4, 1], [1, 5], [5, 4], [2, 4], [5, 3], [3, 2], [0, 2], [3, 0]]
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (False, True, False)
## dissolve_verts v0: V=7 E=9 F=4
  verts: [(-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[1, 5, 3, 4, 0, 2], [1, 2, 6, 5], [5, 6, 4, 3], [6, 2, 0, 4]]
  edges: [[0, 2], [2, 1], [5, 1], [2, 6], [6, 5], [3, 5], [6, 4], [4, 3], [4, 0]]
  sel V: []  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## edge_collapse e0: V=7 E=11 F=6
  sel V: [1]  E: []  F: []
  active_face: None  history: []  select_mode: (False, True, False)
## bridge top/bottom loops of open tube: V=8 E=12 F=4
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3]
  active_face: None  history: []  select_mode: (False, True, False)
## fill hole (triangle fill): V=8 E=13 F=7
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]
  faces: [[0, 1, 3, 2], [2, 3, 7, 6], [6, 7, 5, 4], [4, 5, 1, 0], [2, 6, 4, 0], [1, 7, 3], [1, 5, 7]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 2], [3, 7], [7, 6], [4, 6], [7, 5], [5, 4], [0, 4], [5, 1], [1, 7]]
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11, 12]  F: [5, 6]
  active_face: None  history: [('E', 2), ('E', 5), ('E', 8), ('E', 11)]  select_mode: (False, True, False)
## edge_face_add (F) hole: V=8 E=12 F=6
  sel V: [1, 3, 5, 7]  E: [2, 5, 8, 11]  F: [5]
  active_face: None  history: [('E', 2), ('E', 5), ('E', 8), ('E', 11)]  select_mode: (False, True, False)
Info: Connecting edge loops overlap
## fill_grid grid border: V=25 E=40 F=0
  sel V: [0, 1, 2, 3, 4, 5, 9, 10, 14, 15, 19, 20, 21, 22, 23, 24]  E: [0, 1, 4, 7, 10, 11, 13, 20, 22, 29, 31, 33, 35, 37, 38, 39]  F: []
  active_face: None  history: [('E', 0), ('E', 1), ('E', 4), ('E', 7), ('E', 10), ('E', 11), ('E', 13), ('E', 20), ('E', 22), ('E', 29), ('E', 31), ('E', 33), ('E', 35), ('E', 37), ('E', 38), ('E', 39)]  select_mode: (False, True, False)
## edge_face_add 2 verts: V=8 E=13 F=6
  sel V: [0, 7]  E: [12]  F: []
  active_face: None  history: [('V', 0), ('V', 7)]  select_mode: (True, False, False)
```

### B.7 Loopcut emulation via bmesh.ops.subdivide_edges (golden-harness recipe), select_nth

```
## bmesh loopcut emulation cuts=2: V=16 E=28 F=14
  verts: [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 0.3333, -1.0), (-1.0, -0.3333, -1.0), (-1.0, -0.3333, 1.0), (-1.0, 0.3333, 1.0), (1.0, -0.3333, -1.0), (1.0, 0.3333, -1.0), (1.0, 0.3333, 1.0), (1.0, -0.3333, 1.0)]
  faces: [[8, 11, 3, 2], [2, 3, 7, 6], [12, 15, 5, 4], [4, 5, 1, 0], [9, 12, 4, 0], [15, 10, 1, 5], [7, 3, 11, 14], [14, 11, 10, 15], [2, 6, 13, 8], [8, 13, 12, 9], [6, 7, 14, 13], [13, 14, 15, 12], [0, 1, 10, 9], [9, 10, 11, 8]]
  edges: [[9, 0], [0, 1], [11, 3], [3, 2], [6, 2], [3, 7], [7, 6], [13, 6], [15, 5], [5, 4], [0, 4], [5, 1], [2, 8], [8, 9], [1, 10], [10, 11], [4, 12], [12, 13], [7, 14], [14, 15], [11, 14], [10, 15], [13, 8], [12, 9], [14, 13], [15, 12], [10, 9], [11, 8]]
  sel V: [8, 9, 10, 11, 12, 13, 14, 15]  E: [20, 21, 22, 23, 24, 25, 26, 27]  F: []
  active_face: None  history: []  select_mode: (False, True, False)
## select_nth with active v0: V=36 E=60 F=25
  sel V: [1, 3, 5, 6, 8, 10, 13, 15, 17, 18, 20, 22, 25, 27, 29, 30, 32, 34]  E: []  F: []
  active_face: None  history: []  select_mode: (True, False, False)
```

### B.8 Bridge, fill_grid, fill, spin/screw with axis, remove_doubles, macro sub-op names

```
## bridge two squares: V=8 E=12 F=4
  verts: [(-1.0, -1.0, 0.0), (1.0, -1.0, 0.0), (-1.0, 1.0, 0.0), (1.0, 1.0, 0.0), (-1.0, -1.0, 2.0), (1.0, -1.0, 2.0), (-1.0, 1.0, 2.0), (1.0, 1.0, 2.0)]
  faces: [[6, 7, 3, 2], [7, 5, 1, 3], [5, 4, 0, 1], [4, 6, 2, 0]]
  edges: [[2, 0], [0, 1], [1, 3], [3, 2], [6, 4], [4, 5], [5, 7], [7, 6], [2, 6], [7, 3], [5, 1], [4, 0]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3]
  active_face: None  history: []  select_mode: (True, False, False)
## bridge cuts2 twist1: V=16 E=28 F=12
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27]  F: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
  active_face: None  history: []  select_mode: (True, False, False)
## fill_grid circle8 span2: V=9 E=12 F=4
  verts: [(0.0, 1.0, 0.0), (-0.7071, 0.7071, 0.0), (-1.0, 0.0, 0.0), (-0.7071, -0.7071, 0.0), (0.0, -1.0, 0.0), (0.7071, -0.7071, 0.0), (1.0, 0.0, 0.0), (0.7071, 0.7071, 0.0), (0.0, 0.0, 0.0)]
  faces: [[0, 8, 6, 7], [8, 4, 5, 6], [1, 2, 8, 0], [2, 3, 4, 8]]
  edges: [[1, 0], [2, 1], [3, 2], [4, 3], [5, 4], [6, 5], [7, 6], [0, 7], [0, 8], [8, 6], [8, 4], [2, 8]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7, 8]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]  F: [0, 1, 2, 3]
  active_face: None  history: []  select_mode: (True, False, False)
## fill circle8: V=8 E=13 F=6
  verts: [(0.0, 1.0, 0.0), (-0.7071, 0.7071, 0.0), (-1.0, 0.0, 0.0), (-0.7071, -0.7071, 0.0), (0.0, -1.0, 0.0), (0.7071, -0.7071, 0.0), (1.0, 0.0, 0.0), (0.7071, 0.7071, 0.0)]
  faces: [[7, 2, 0], [2, 7, 6], [3, 2, 6], [3, 6, 5], [4, 3, 5], [2, 1, 0]]
  edges: [[1, 0], [2, 1], [3, 2], [4, 3], [5, 4], [6, 5], [7, 6], [0, 7], [7, 2], [6, 2], [6, 3], [5, 3], [2, 0]]
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]  F: [0, 1, 2, 3, 4, 5]
  active_face: None  history: []  select_mode: (True, False, False)
## edge_face_add circle8: V=8 E=8 F=1
  sel V: [0, 1, 2, 3, 4, 5, 6, 7]  E: [0, 1, 2, 3, 4, 5, 6, 7]  F: [0]
  active_face: None  history: []  select_mode: (True, False, False)
## spin plane about y: V=20 E=36 F=18
  sel V: [16, 17, 18, 19]  E: [28, 29, 30, 31]  F: [14]
  active_face: None  history: []  select_mode: (True, False, False)
## screw 3-vert open profile: V=32 E=48 F=16
  sel V: [29, 30, 31]  E: [43, 44]  F: []
  active_face: None  history: []  select_mode: (True, False, False)
Info: Removed 6 vertices
## remove_doubles huge threshold: V=2 E=1 F=0
  verts: [(-0.1429, -0.1429, -0.1429), (1.0, 1.0, 1.0)]
  faces: []
  edges: [[0, 1]]
  sel V: [0, 1]  E: [0]  F: []
  active_face: None  history: []  select_mode: (True, False, False)
## t: EXC RuntimeError: Operator bpy.ops.mesh.rip_move.poll() failed, context is incorrect
MACRO mesh.extrude_region_move ['MESH_OT_extrude_region', 'TRANSFORM_OT_translate']
MACRO mesh.extrude_region_shrink_fatten ['MESH_OT_extrude_region', 'TRANSFORM_OT_shrink_fatten']
MACRO mesh.extrude_context_move ['MESH_OT_extrude_context', 'TRANSFORM_OT_translate']
MACRO mesh.extrude_faces_move ['MESH_OT_extrude_faces_indiv', 'TRANSFORM_OT_shrink_fatten']
MACRO mesh.extrude_edges_move ['MESH_OT_extrude_edges_indiv', 'TRANSFORM_OT_translate']
MACRO mesh.extrude_vertices_move ['MESH_OT_extrude_verts_indiv', 'TRANSFORM_OT_translate']
MACRO mesh.extrude_manifold ['MESH_OT_extrude_region', 'TRANSFORM_OT_translate']
MACRO mesh.loopcut_slide ['MESH_OT_loopcut', 'TRANSFORM_OT_edge_slide']
MACRO mesh.duplicate_move ['MESH_OT_duplicate', 'TRANSFORM_OT_translate']
MACRO mesh.rip_move ['MESH_OT_rip', 'TRANSFORM_OT_translate']
MACRO mesh.rip_edge_move ['MESH_OT_rip_edge', 'TRANSFORM_OT_translate']
MACRO mesh.offset_edge_loops_slide ['MESH_OT_offset_edge_loops', 'TRANSFORM_OT_edge_slide']
MACRO mesh.polybuild_face_at_cursor_move ['MESH_OT_polybuild_face_at_cursor', 'TRANSFORM_OT_translate']
```

B.7 recipe used for loopcut emulation:

```python
# ring = edges from bpy.ops.mesh.select_edge_ring_multi() starting at edge_index
r = bmesh.ops.subdivide_edges(bm, edges=ring, cuts=number_cuts, smooth=smoothness,
        smooth_falloff=falloff, use_smooth_even=True, quad_corner_type='PATH',
        use_single_edge=True, use_grid_fill=True, use_only_quads=True)
# select only new inner edges (geom_inner ∩ BMEdge) — mirrors SUBDIV_SELECT_LOOPCUT
```

## Appendix C — `bmesh.ops` signatures in 5.2.4 (slot names and defaults)

These are the building blocks the edit-mode operators call. frap's internal BMesh ops should use the same slot names so behaviour can be cross-checked one-to-one in Blender with `bmesh.ops.*`.

```
bmesh.ops.average_vert_facedata(bmesh, verts=[])
bmesh.ops.beautify_fill(bmesh, faces=[], edges=[], use_restrict_tag=False, method='AREA')
bmesh.ops.bevel(bmesh, geom=[], offset=0.0, offset_type='OFFSET', profile_type='SUPERELLIPSE', segments=0, profile=0.0, affect='VERTICES', clamp_overlap=False, material=0, loop_slide=False, mark_seam=False, mark_sharp=False, harden_normals=False, face_strength_mode='NONE', miter_outer='SHARP', miter_inner='SHARP', spread=0.0, custom_profile=None, vmesh_method='ADJ')
bmesh.ops.bisect_edges(bmesh, edges=[], cuts=0, edge_percents={})
bmesh.ops.bisect_plane(bmesh, geom=[], dist=0.0, plane_co=Vector(), plane_no=Vector(), use_snap_center=False, clear_outer=False, clear_inner=False)
bmesh.ops.bmesh_to_mesh(bmesh, mesh=None, object=None)
bmesh.ops.bridge_loops(bmesh, edges=[], use_pairs=False, use_cyclic=False, use_merge=False, merge_factor=0.0, twist_offset=0)
bmesh.ops.circularize(bmesh, geom=[], factor=0.0, custom_radius=0.0, angle=0.0, fit_method=0, flatten=0.0, regular=False, lock_x=False, lock_y=False, lock_z=False, mirror_x=False, mirror_y=False, mirror_z=False)
bmesh.ops.collapse(bmesh, edges=[], uvs=False)
bmesh.ops.collapse_uvs(bmesh, edges=[])
bmesh.ops.connect_vert_pair(bmesh, verts=[], verts_exclude=[], faces_exclude=[])
bmesh.ops.connect_verts(bmesh, verts=[], faces_exclude=[], check_degenerate=False)
bmesh.ops.connect_verts_concave(bmesh, faces=[])
bmesh.ops.connect_verts_nonplanar(bmesh, angle_limit=0.0, faces=[])
bmesh.ops.contextual_create(bmesh, geom=[], mat_nr=0, use_smooth=False)
bmesh.ops.convex_hull(bmesh, input=[], use_existing_faces=False)
bmesh.ops.create_circle(bmesh, cap_ends=False, cap_tris=False, segments=0, radius=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_cone(bmesh, cap_ends=False, cap_tris=False, segments=0, radius1=0.0, radius2=0.0, depth=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_cube(bmesh, size=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_grid(bmesh, x_segments=0, y_segments=0, size=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_icosphere(bmesh, subdivisions=0, radius=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_monkey(bmesh, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_uvsphere(bmesh, u_segments=0, v_segments=0, radius=0.0, matrix=Matrix(), calc_uvs=False)
bmesh.ops.create_vert(bmesh, co=Vector())
bmesh.ops.delete(bmesh, geom=[], context='VERTS')
bmesh.ops.dissolve_degenerate(bmesh, dist=0.0, edges=[])
bmesh.ops.dissolve_edges(bmesh, edges=[], use_verts=False, use_face_split=False, angle_threshold=0.0, use_preserve_quads=False)
bmesh.ops.dissolve_faces(bmesh, faces=[], use_verts=False)
bmesh.ops.dissolve_limit(bmesh, angle_limit=0.0, use_dissolve_boundaries=False, verts=[], edges=[], delimit={})
bmesh.ops.dissolve_verts(bmesh, verts=[], use_face_split=False, use_boundary_tear=False)
bmesh.ops.duplicate(bmesh, geom=[], dest=None, use_select_history=False, use_edge_flip_from_face=False)
bmesh.ops.edgeloop_fill(bmesh, edges=[], mat_nr=0, use_smooth=False)
bmesh.ops.edgenet_fill(bmesh, edges=[], mat_nr=0, use_smooth=False, sides=0)
bmesh.ops.edgenet_prepare(bmesh, edges=[])
bmesh.ops.extrude_discrete_faces(bmesh, faces=[], use_normal_flip=False, use_select_history=False)
bmesh.ops.extrude_edge_only(bmesh, edges=[], use_normal_flip=False, use_select_history=False)
bmesh.ops.extrude_face_region(bmesh, geom=[], edges_exclude={}, use_keep_orig=False, use_normal_flip=False, use_normal_from_adjacent=False, use_dissolve_ortho_edges=False, use_select_history=False, skip_input_flip=False)
bmesh.ops.extrude_vert_indiv(bmesh, verts=[], use_select_history=False)
bmesh.ops.face_attribute_fill(bmesh, faces=[], use_normals=False, use_data=False)
bmesh.ops.find_doubles(bmesh, verts=[], keep_verts=[], use_connected=False, dist=0.0)
bmesh.ops.flatten(bmesh, geom=[], factor=0.0, method=0, view_normal=Vector(), lock_x=False, lock_y=False, lock_z=False)
bmesh.ops.flip_quad_tessellation(bmesh, faces=[])
bmesh.ops.grid_fill(bmesh, edges=[], mat_nr=0, use_smooth=False, use_interp_simple=False)
bmesh.ops.holes_fill(bmesh, edges=[], sides=0)
bmesh.ops.inset_individual(bmesh, faces=[], thickness=0.0, depth=0.0, use_even_offset=False, use_interpolate=False, use_relative_offset=False)
bmesh.ops.inset_region(bmesh, faces=[], faces_exclude=[], use_boundary=False, use_even_offset=False, use_interpolate=False, use_relative_offset=False, use_edge_rail=False, thickness=0.0, depth=0.0, use_outset=False)
bmesh.ops.join_triangles(bmesh, faces=[], cmp_seam=False, cmp_sharp=False, cmp_uvs=False, cmp_vcols=False, cmp_materials=False, angle_face_threshold=0.0, angle_shape_threshold=0.0, topology_influence=0.0, deselect_joined=False)
bmesh.ops.mesh_to_bmesh(bmesh, mesh=None, object=None, use_shapekey=False)
bmesh.ops.mirror(bmesh, geom=[], matrix=Matrix(), merge_dist=0.0, axis='X', mirror_u=False, mirror_v=False, mirror_udim=False, use_shapekey=False)
bmesh.ops.object_load_bmesh(bmesh, scene=None, object=None)
bmesh.ops.offset_edgeloops(bmesh, edges=[], use_cap_endpoint=False)
bmesh.ops.planar_faces(bmesh, faces=[], iterations=0, factor=0.0)
bmesh.ops.pointmerge(bmesh, verts=[], merge_co=Vector(), vert_target=None)
bmesh.ops.pointmerge_facedata(bmesh, verts=[], vert_target=None)
bmesh.ops.poke(bmesh, faces=[], offset=0.0, center_mode='MEAN_WEIGHTED', use_relative_offset=False)
bmesh.ops.recalc_face_normals(bmesh, faces=[])
bmesh.ops.region_extend(bmesh, geom=[], use_contract=False, use_faces=False, use_face_step=False)
bmesh.ops.remove_doubles(bmesh, verts=[], use_connected=False, dist=0.0)
bmesh.ops.reverse_colors(bmesh, faces=[], color_index=0)
bmesh.ops.reverse_faces(bmesh, faces=[], flip_multires=False)
bmesh.ops.reverse_uvs(bmesh, faces=[])
bmesh.ops.rotate(bmesh, cent=Vector(), matrix=Matrix(), verts=[], space=Matrix(), use_shapekey=False)
bmesh.ops.rotate_colors(bmesh, faces=[], use_ccw=False, color_index=0)
bmesh.ops.rotate_edges(bmesh, edges=[], use_ccw=False)
bmesh.ops.rotate_uvs(bmesh, faces=[], use_ccw=False)
bmesh.ops.scale(bmesh, vec=Vector(), space=Matrix(), verts=[], use_shapekey=False)
bmesh.ops.smooth_laplacian_vert(bmesh, verts=[], lambda_factor=0.0, lambda_border=0.0, use_x=False, use_y=False, use_z=False, preserve_volume=False)
bmesh.ops.smooth_vert(bmesh, verts=[], factor=0.0, mirror_clip_x=False, mirror_clip_y=False, mirror_clip_z=False, clip_dist=0.0, use_axis_x=False, use_axis_y=False, use_axis_z=False)
bmesh.ops.solidify(bmesh, geom=[], thickness=0.0)
bmesh.ops.space_edge_loops_evenly(bmesh, geom=[], interpolation='CUBIC', factor=0.0, lock_x=False, lock_y=False, lock_z=False)
bmesh.ops.spin(bmesh, geom=[], cent=Vector(), axis=Vector(), dvec=Vector(), angle=0.0, space=Matrix(), steps=0, use_merge=False, use_normal_flip=False, use_duplicate=False)
bmesh.ops.split(bmesh, geom=[], dest=None, use_only_faces=False)
bmesh.ops.split_edges(bmesh, edges=[], verts=[], use_verts=False)
bmesh.ops.subdivide_edgering(bmesh, edges=[], interp_mode='LINEAR', smooth=0.0, cuts=0, profile_shape='SMOOTH', profile_shape_factor=0.0)
bmesh.ops.subdivide_edges(bmesh, edges=[], smooth=0.0, smooth_falloff='SMOOTH', fractal=0.0, along_normal=0.0, cuts=0, seed=0, custom_patterns={}, edge_percents={}, quad_corner_type='STRAIGHT_CUT', use_grid_fill=False, use_single_edge=False, use_only_quads=False, use_sphere=False, use_smooth_even=False)
bmesh.ops.symmetrize(bmesh, input=[], direction='-X', dist=0.0, use_shapekey=False)
bmesh.ops.transform(bmesh, matrix=Matrix(), space=Matrix(), verts=[], use_shapekey=False)
bmesh.ops.translate(bmesh, vec=Vector(), space=Matrix(), verts=[], use_shapekey=False)
bmesh.ops.triangle_fill(bmesh, use_beauty=False, use_dissolve=False, edges=[], normal=Vector())
bmesh.ops.triangulate(bmesh, faces=[], quad_method='BEAUTY', ngon_method='BEAUTY')
bmesh.ops.unsubdivide(bmesh, verts=[], iterations=0)
bmesh.ops.weld_verts(bmesh, targetmap={}, use_centroid=False, average_vert_data=False)
bmesh.ops.wireframe(bmesh, faces=[], thickness=0.0, offset=0.0, use_replace=False, use_boundary=False, use_even_offset=False, use_crease=False, crease_weight=0.0, use_relative_offset=False, material_offset=0)
```

## Appendix D — Introspection and experiment scripts (reproducible)

Run with `$BLENDER_BIN -b --factory-startup --python dump_ops.py -- mesh.bevel,uv.unwrap,...`. Output is a JSON map of op → {name, desc, flags, props[{id,type,default,min,max,subtype,unit,len,items,flag,flags}]}. Use it to generate frap's operator spec, and re-run on new Blender versions to diff.

```python
import bpy, sys, json
OPS = sys.argv[sys.argv.index('--')+1].split(',')
def fmt_prop(p):
    d = {"id": p.identifier, "type": p.type, "name": p.name}
    if p.type in ('INT','FLOAT'):
        try:
            if getattr(p,'is_array',False) and p.array_length>0:
                d["default"] = list(p.default_array)
            else:
                d["default"] = p.default
        except Exception as e: d["default"]=str(e)
        d["min"]=p.hard_min; d["max"]=p.hard_max
        if p.type=='FLOAT': d["subtype"]=p.subtype; d["unit"]=p.unit
        if getattr(p,'is_array',False): d["len"]=p.array_length
    elif p.type=='BOOLEAN':
        if p.is_array: d["default"]=list(p.default_array); d["len"]=p.array_length
        else: d["default"]=p.default
    elif p.type=='ENUM':
        d["items"]=[i.identifier for i in p.enum_items]
        d["flag"]=p.is_enum_flag
        d["default"]= list(p.default_flag) if p.is_enum_flag else p.default
    elif p.type=='STRING':
        d["default"]=p.default
    elif p.type in ('POINTER','COLLECTION'):
        d["fixed_type"]=p.fixed_type.identifier
    flags=[]
    if p.is_hidden: flags.append("HIDDEN")
    if p.is_skip_save: flags.append("SKIP_SAVE")
    if flags: d["flags"]=flags
    return d
out={}
for name in OPS:
    mod, fn = name.split('.')
    try:
        op = getattr(getattr(bpy.ops, mod), fn)
        rna = op.get_rna_type()
        props=[fmt_prop(p) for p in rna.properties if p.identifier!='rna_type']
        out[name]={"name":rna.name,"desc":rna.description,"props":props,"flags": list(op.bl_options) if hasattr(op,'bl_options') else None}
    except Exception as e:
        out[name]={"error":str(e)}
print("JSONSTART"+json.dumps(out, default=str)+"JSONEND")
```

Experiment harness (`harness.py`, imported by the experiment scripts):

```python
import bpy, bmesh, sys, traceback, math
def R(x): 
    v=round(x,4); return 0.0 if v==0 else v
def fresh(prim='cube', **kw):
    if bpy.context.object and bpy.context.object.mode!='OBJECT': bpy.ops.object.mode_set(mode='OBJECT')
    for o in list(bpy.data.objects): bpy.data.objects.remove(o)
    for m in list(bpy.data.meshes): bpy.data.meshes.remove(m)
    bpy.context.scene.cursor.location=(0,0,0)
    getattr(bpy.ops.mesh,'primitive_%s_add'%prim)(**kw)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.context.tool_settings.mesh_select_mode=(True,False,False)
    return bpy.context.object
def bm(): 
    b=bmesh.from_edit_mesh(bpy.context.object.data)
    b.verts.ensure_lookup_table(); b.edges.ensure_lookup_table(); b.faces.ensure_lookup_table(); b.verts.index_update(); b.edges.index_update(); b.faces.index_update(); return b
def desel():
    bpy.ops.mesh.select_all(action='DESELECT')
def sel_faces(idx):
    b=bm()
    for i in idx: b.faces[i].select_set(True)
    for i in idx: b.select_history.add(b.faces[i])
    bmesh.update_edit_mesh(bpy.context.object.data)
def sel_edges(idx):
    b=bm()
    for i in idx: b.edges[i].select_set(True); b.select_history.add(b.edges[i])
    bmesh.update_edit_mesh(bpy.context.object.data)
def sel_verts(idx):
    b=bm()
    for i in idx: b.verts[i].select_set(True); b.select_history.add(b.verts[i])
    b.select_flush_mode()
    bmesh.update_edit_mesh(bpy.context.object.data)
def dump(tag, full=True, maxv=60):
    b=bm()
    sv=[v.index for v in b.verts if v.select]; se=[e.index for e in b.edges if e.select]; sf=[f.index for f in b.faces if f.select]
    act=b.faces.active.index if b.faces.active else None
    hist=[(type(h).__name__[2], h.index) for h in b.select_history]
    print(f"## {tag}: V={len(b.verts)} E={len(b.edges)} F={len(b.faces)}")
    if full:
        print("  verts:", [tuple(R(c) for c in v.co) for v in b.verts][:maxv])
        print("  faces:", [[v.index for v in f.verts] for f in b.faces][:maxv])
        if len(b.edges)<=80: print("  edges:", [[v.index for v in e.verts] for e in b.edges])
    print("  sel V:", sv if len(sv)<60 else f"{len(sv)} sel", " E:", se if len(se)<60 else f"{len(se)} sel", " F:", sf if len(sf)<60 else f"{len(sf)} sel")
    print("  active_face:", act, " history:", hist, " select_mode:", tuple(bpy.context.tool_settings.mesh_select_mode))
def run(tag, fn):
    try: fn()
    except Exception as e: print(f"## {tag}: EXC {type(e).__name__}: {str(e).strip()[:300]}")
```
