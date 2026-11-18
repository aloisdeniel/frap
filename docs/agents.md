# Driving frap from an agent

frap edits 3D scenes headlessly with Blender 5.2's API names. It reads statements and answers each one with a JSON object. This guide is for LLM agents and the harnesses that run them. The architecture is in `ARCHITECTURE.md` (§11 statements, §13 protocol, §14 `--describe`), and every place frap differs from Blender is listed in `deviations.md`.

## 1. Pick a transport

All three transports drive the same `Session`. The scene lives as long as the process (or MCP connection) does.

| Transport | Use it when | Framing |
|---|---|---|
| `frap mcp` | your client speaks MCP (stdio, JSON-RPC 2.0) | `initialize`, `tools/list`, `tools/call`; one `Session` per connection |
| `frap serve --jsonl` | you run frap as a subprocess from your own harness | one JSON request per line in, JSON responses out, each request ends with a done record |
| `frap` (raw stdin), `frap run file.py`, `frap -c "…"` | humans, shell scripts, golden cases | statements in, one JSON line per statement out |

Raw stdin lets a statement continue while brackets are open. A blank line or EOF ends it (`SyntaxError: '(' was never closed`), so frap never hangs on an unclosed bracket. In `serve` and MCP every request is parsed as one complete unit, so frap never waits for more input.

## 2. `frap mcp`

`tools/list` returns two tools. They are the same definitions as `frap --describe=tools-anthropic`, with MCP's `inputSchema` key (`frap --describe=tools-mcp` prints the exact list):

- `frap_exec(statements: string, on_error?: "skip_rest" | "continue")` runs statements against the connection's scene. The tool result is the list of responses (one per statement, §4) followed by the done record.
- `frap_describe(target: string)` returns the manifest fragment of an operator (`op:mesh.bevel` or `mesh.bevel`), a struct (`struct:Object` or `Object`), a struct member (`Object.location`, `Object.select_set`), or `errors`. The same fragment is available as a statement, `bpy.ops.frap.describe(target='mesh.bevel')` (§5).

The scene persists across `tools/call`s, so build a model over many small calls and read back what you need between them.

## 3. `frap serve --jsonl`, and backing the tools yourself

Request (one line):

```json
{"id": 7, "src": "bpy.ops.mesh.primitive_cube_add()\nbpy.ops.mesh.bevel(offset=0.1)\nbpy.ops.object.shade_smooth()", "on_error": "skip_rest"}
```

Responses (one line each), then the done record:

```json
{"id":7,"seq":1,"line":1,"kind":"op","op":"mesh.primitive_cube_add","ok":true,"status":"FINISHED", …}
{"id":7,"seq":2,"line":2,"kind":"op","op":"mesh.bevel","ok":false,"status":"ERROR","state":"unchanged","error":{"exception":"RuntimeError","code":"OP_POLL_FAILED","message":"Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect","detail":{"requires":["MODE_EDIT_MESH"],"unmet":["MODE_EDIT_MESH"]},"hint":"bpy.ops.object.mode_set(mode='EDIT')"}, …}
{"id":7,"seq":3,"line":3,"kind":"op","op":"object.shade_smooth","ok":false,"status":"SKIPPED","state":"unchanged", …}
{"id":7,"event":"done","ok":false,"counts":{"finished":1,"error":1,"cancelled":0,"skipped":1}}
```

- `id` (string or number) is echoed on every response of the request and on its done record. Read until the `done` record with your `id`.
- **`on_error`** applies to the statements of one request. `"skip_rest"` (the default) answers every statement after the first failure with `status: "SKIPPED"` without running it, so a batch cannot run on against a state it did not expect. `"continue"` runs them anyway. Either way a failing statement leaves the scene unchanged (`"state": "unchanged"`) and the session goes on: the next request starts from the last committed state.

If your harness does not speak MCP, use the definitions from `frap --describe=tools-anthropic` (Anthropic Messages API) or `frap --describe=tools-openai` (OpenAI function calling, strict mode), and back them with one long-lived `frap serve --jsonl` process. Keep that one process for the whole conversation: it holds the scene between tool calls.

- `frap_exec({statements, on_error})`: write `{"id": <n>, "src": statements, "on_error": on_error}` and return the response lines up to and including the done record.
- `frap_describe({target})`: either send `bpy.ops.frap.describe(target='<target>')` to the same serve process and return the response's `result.describe`, or run `frap --describe=<target>` in a separate process and return its stdout (the manifest does not depend on the scene). Both give the same fragment.

A minimal Python backing (no SDK needed):

```python
import json, subprocess

frap = subprocess.Popen(["frap", "serve", "--jsonl"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, text=True, bufsize=1)
next_id = 0

def frap_exec(statements, on_error="skip_rest"):
    global next_id
    next_id += 1
    frap.stdin.write(json.dumps({"id": next_id, "src": statements, "on_error": on_error}) + "\n")
    out = []
    while True:
        msg = json.loads(frap.stdout.readline())
        out.append(msg)
        if msg.get("event") == "done" and msg.get("id") == next_id:
            return out
```

The OpenAI definitions are in strict mode, so every argument is required. Optional ones are nullable, and `null` means "leave unset" (`on_error: null` is `skip_rest`).

`frap_schema::tools` can also emit one tool per operator namespace (`frap_ops_mesh`, …). Its `call` argument is an `anyOf` over per-operator argument objects, and `frap_schema::tools::call_to_statement` turns a call into the matching `bpy.ops.…(…)` statement, which you send to `frap_exec`. Prefer the two fixed tools: hundreds of tools overwhelm agents.

## 4. What a response says

| Field | Meaning |
|---|---|
| `status` | `FINISHED` (committed), `CANCELLED` (the operator refused; Blender's WARNING `reports` say why), `ERROR`, `SKIPPED` |
| `ok` | `true` iff `FINISHED` |
| `kind` | `op`, `op_meta`, `get`, `set`, `call`, `print`, `noop`, `invalid` |
| `result` | operator results: the created `object`, `objects`, `removed` counts, export `filepath`/`bytes`, … |
| `value` / `rna` | read values in the §13.3 encoding, with the RNA type tag (`Vector`, `Euler`, `Matrix`, …) |
| `changes` | datablocks `added` / `removed` / `modified` / `renamed`, as repr paths |
| `context` | `mode`, `active_object`, `selected_objects`, `frame_current` after the statement |
| `error` | `exception` (the Python class Blender raises), stable `code`, Blender's `message`, `span`, `detail`, `hint` |
| `hints` | frap's advice, e.g. `ANGLE_LIKELY_DEGREES`, `NO_OP_DEFAULT` |

How to react to an error:

1. Read `error.hint` first. When frap knows the fix it names it exactly, e.g. `bpy.ops.object.mode_set(mode='EDIT')`.
2. `detail.did_you_mean` lists close names for unknown operators, keywords and attributes. `detail.valid` lists every accepted keyword.
3. `OP_NOT_IMPLEMENTED` means the operator exists in Blender 5.2 but frap does not implement it yet (`frap_describe` gives `frap.status: "not_implemented"` and `frap.planned`). `OP_UNKNOWN` means the operator does not exist in Blender 5.2 either.
4. `frap --describe=errors` lists every error code with its exception.
5. `LIMIT_EXCEEDED` means the statement would build more geometry than `--max-elements` allows (primitives are checked before anything is built, e.g. `x_subdivisions=10**7`); `TIMEOUT` means it ran past `--stmt-timeout`. Both roll the statement back (`"state":"unchanged"`): ask for less geometry or raise the limit.

The full schemas of the request, response, error, ready/end events and done record are in `frap --describe=protocol`.

## 5. Ask before calling: `poll()` and `--describe`

- `bpy.ops.mesh.bevel.poll()` returns `False` (a `get`-kind response) without changing anything. On failure `detail.unmet` lists the unmet requirements. Use it to check the mode before an edit-mode operator. `detail.requires`/`unmet` of `OP_POLL_FAILED` use the manifest's requirement codes (`MODE_EDIT_MESH`).
- Every operator's manifest entry has `poll.requires`, the declarative requirements frap checks (`MODE_OBJECT`, `MODE_EDIT_MESH`, `ACTIVE_OBJECT`, `ACTIVE_TYPE(MESH)`, `HAS_SELECTED_OBJECTS`, …), plus `poll.text` in words. `poll.matrix` is Blender's dumped poll result in contexts A–E (legend in `poll_contexts`). An operator frap does not implement has `requires: null` and only the dumped matrix.
- `bpy.ops.mesh.bevel.idname()` returns `"MESH_OT_bevel"`. `bpy.ops.mesh.bevel.get_rna_type()` returns a short Blender-shaped summary (`idname`, `name`, `description`, `properties` with their types and defaults).
- `bpy.ops.frap.describe(target='mesh.bevel')` (short form `frap.describe target=mesh.bevel`; quote targets with a colon: `frap.describe target='op:mesh.bevel'`) returns the full manifest fragment inside the session: `result.target` echoes the target and `result.describe` is exactly what `frap --describe=<target>` prints. It accepts every target form below and changes nothing. An unknown operator, struct or member is `KeyError` with code `DESCRIBE_TARGET` and close names in `detail.did_you_mean`; leaving `target` out, or passing an empty string or `None`, is a `TypeError` (ask for `target='manifest'` explicitly if you really want the whole manifest, several MB).

`frap --describe` prints the whole manifest: `operators[]` (every Blender 5.2 operator plus frap's `frap.*`), `structs[]`, `bpy_struct_functions[]`, `context_members[]`, `modifier_types[]` (with evaluator support), `node_types[]` and `defaults_that_surprise[]`. Other targets:

```
frap --describe                          # manifest
frap --describe op:mesh.bevel            # one operator (also: struct:Object, Object.location)
frap --describe --filter 'mesh.*' --implemented-only
frap --describe=jsonschema               # JSON Schema 2020-12 of every operator's keyword arguments + protocol
frap --describe=tools-anthropic          # also tools-openai, tools-mcp
frap --describe=protocol                 # also errors
```

Each property entry gives `type`, `subtype`, `unit`, `array`, `default`, `hard_min/max`, `soft_min/max`, `enum_items`, `is_enum_flag`, `hidden`, `skip_save`, `dynamic_enum` with its `context_rule`, `unset_semantics` (what leaving it out means, e.g. `location` unset = the 3D cursor), `frap_note` and `frap_extension`. Hidden properties are listed and flagged.

**Read `defaults_that_surprise` once.** Some defaults surprise: `export_scene.gltf` does not apply modifiers (`export_apply=False`) and exports neither cameras nor lights; `render.render()` writes nothing to disk unless `write_still=True`; `mesh.bevel`'s default `offset=0` does nothing; a light from `object.light_add` has 10 W; `export_scene.gltf` exports objects at their frame-0 rest transforms unless `export_current_frame=True`. Called in Edit Mode, `export_scene.gltf` first leaves it (as Blender's add-on does): the edits are exported and the session stays in Object Mode afterwards. The `scale` of `mesh.primitive_cube_add`, `cone_add`, `cylinder_add`, `uv_sphere_add` and `ico_sphere_add` defaults to a `(0, 0, 0)` placeholder that means 1 and scales the new mesh data (the object's scale stays 1). `mesh.primitive_plane_add`, `grid_add`, `circle_add`, `monkey_add` and every `curve.primitive_*`/`surface.primitive_*` operator ignore `scale` (their `unset_semantics` says `ignored`): set `object.scale` or use `transform.resize` afterwards. `mesh.primitive_torus_add` has no `scale`.

## 6. Units and values

- **Angles are radians**, in operators and in RNA. Units show as `"unit": "ROTATION (radians)"`. Write `rotation=(0, 0, radians(90))` or `math.pi / 2`. A whole number above 2π gets the `ANGLE_LIKELY_DEGREES` hint.
- Floats come back as their shortest f32 form (`0.1`, not `0.10000000149011612`). NaN and infinities come back as `{"$float": "nan"}` and `{"$float": "inf"}`.
- Quaternions are `[w, x, y, z]`, matrices are row-major arrays of rows, and enum flags are sorted arrays of strings.
- ID pointers come back as `{"$ref": "bpy.data.objects['Cube']"}`, other structs as `{"$ptr": "…", "type": "BevelModifier"}`, collections as `{"$coll": "bpy.data.objects", "len": 3, "items": [...]}` (names capped by `--max-items`).

## 7. Bulk reads and writes

Do not read thousands of elements one by one. frap has no variables, so bulk access works on full paths:

```
bpy.data.meshes['Cube'].vertices.foreach_get('co')                 # one-argument form (frap extension)
bpy.data.meshes['Cube'].vertices[1000:2000].foreach_get('co')      # slices page through a collection
bpy.data.meshes['Cube'].attributes['position'].data[0:100]         # attribute values for a slice
bpy.data.meshes['Cube'].polygons.foreach_get('normal')
bpy.data.meshes['Cube'].vertices.foreach_set('co', [0,0,0, 1,0,0, …])
```

A bulk read answers with `{"$array": [x0, y0, z0, x1, …], "shape": [N, 3], "dtype": "float32", "len": N, "range": [a, b], "truncated": false}`. The array is flat and row-major, capped at `--max-array` scalars (default 65,536). When it is capped, `truncated` is `true` and `range` says which items came back, so read the next slice. Blender's two-argument form `foreach_get("co", seq)` also works when `seq` is a literal list of the right length. A `foreach_set` of the wrong length raises Blender's error. Mesh data writes in Edit Mode raise `RNA_EDITMODE_WRITE`; switch to Object Mode first.

For one element use a plain read: `bpy.data.meshes['Cube'].vertices[3].co`.

## 8. Statement forms

Both forms may be mixed in one request:

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))      mesh.primitive_cube_add size=2 location=0,0,1
bpy.data.objects['Cube'].location = (1, 2, 3)                    data.objects['Cube'].location = 1,2,3
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
                                                                 mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1
bpy.data.objects['Cube'].select_set(True)                        data.objects['Cube'].select_set True
```

`D` and `C` are `bpy.data` and `bpy.context`. There are no variables, loops, `if` or `def` (`SyntaxError`, code `UNSUPPORTED_SYNTAX`), so always use a full path such as `bpy.data.objects['Cube']`. `import bpy`, `from math import radians` and the like are accepted and do nothing. `print(a, b)` returns `"value": [a, b]`. Operator properties are keyword-only, as in Blender.

## 9. A first session

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
bpy.ops.object.shade_smooth()
bpy.data.objects['Cube.001'].rotation_euler = (0, 0, radians(45))
bpy.data.meshes['Cube.001'].vertices.foreach_get('co')
bpy.ops.export_scene.gltf(filepath='out/cube.glb', export_apply=True)
bpy.ops.wm.save_as_mainfile(filepath='out/cube.frap')
```

The factory scene already contains `Cube`, `Camera` and `Light`. The new cube is named `Cube.001`, and its mesh is named independently. Read the names back from `result.object` or `changes.added` instead of guessing them.

## 10. Modelling in Edit Mode

Edit Mode works on the mesh's BMesh, as in Blender. The recipes below follow the golden cases under `tests/golden/cases/edit/workflows/`, which frap and Blender 5.2 run to the same mesh, so you can copy them (the transcripts under `tests/cli/m2/workflows/` show whole agent sessions over `serve` and MCP).

### 10.1 Entering, leaving, and what the response tells you

- `bpy.ops.object.mode_set(mode='EDIT')` edits the active mesh object. Select several mesh objects first to edit them together (multi-object Edit Mode, `bpy.context.objects_in_mode`): selection and modelling operators then act on every edited mesh. `bpy.ops.object.mode_set(mode='OBJECT')` writes the edit mesh back to the mesh data.
- Every `mesh.*` modelling operator needs Edit Mode. In Object Mode it fails with `OP_POLL_FAILED`, `detail.unmet: ["MODE_EDIT_MESH"]` and the hint `bpy.ops.object.mode_set(mode='EDIT')`; `bpy.ops.mesh.inset.poll()` asks without failing.
- While a mesh is in Edit Mode, every response carries an `edit_mesh` block per edited mesh: element counts (`verts`, `edges`, `faces`, `tris`), `select_mode`, `selected` counts, the `active` element and the `select_history`. Read it after each step instead of reading the mesh.
- Reads of the mesh in Edit Mode (`bpy.data.meshes['Cube'].vertices[0].co`, `polygons.foreach_get('center')`, `total_vert_sel`) show the edit mesh as it is now; in a Blender script you would call `update_from_editmode()` first. Writes to mesh data (`vertices[0].co = …`, `foreach_set`) are refused with `RNA_EDITMODE_WRITE` and the hint `bpy.ops.object.mode_set(mode='OBJECT')`: move geometry with `transform.*` operators instead, or leave Edit Mode first.
- A primitive added in Edit Mode (`bpy.ops.mesh.primitive_cube_add(…)`) joins the edited mesh and becomes the only selection, so the next operator acts on it alone.

### 10.2 Selecting without a mouse

Agents cannot click, so frap adds two selection operators (they also exist as golden emulations, so goldens can use them):

- `bpy.ops.frap.select_elements(type='FACE', indices=[4, 6], action='SET', object='')` selects vertices (`VERT`), edges (`EDGE`) or faces (`FACE`) by index, in `mesh.vertices` / `edges` / `polygons` order. `action='ADD'` and `'REMOVE'` change the current selection; `object` names another edited object. An index out of range is `IndexError` and changes nothing.
- `bpy.ops.frap.select_by_normal(direction=(0, 0, -1), threshold=radians(30), orientation='GLOBAL', extend=False)` selects the faces whose normal points along `direction` (the bottom cap of a cylinder, the front faces of a box). Use a small `threshold` (`0.1`) to pick only faces that face exactly that way.

Find indices with one bulk read rather than by guessing: `bpy.data.meshes['Table'].polygons.foreach_get('center')` (or `'normal'`, `vertices.foreach_get('co')`, `edges.foreach_get('vertices')`). Indices change when topology changes, so read them again after a modelling step. Faces that a step did not touch keep their index, which is how `chair_seat_legs_back.py` picks the backrest cells after extruding the legs.

Blender's own selection operators work as well: `mesh.select_all(action='SELECT' | 'DESELECT' | 'INVERT' | 'TOGGLE')`, `mesh.select_mode(type='VERT' | 'EDGE' | 'FACE')` (asking for the mode that is already set is `CANCELLED`, as in Blender), `mesh.select_linked()` (the whole island), `mesh.select_more()` / `select_less()`, `mesh.region_to_loop()` (the boundary edges of the selected faces), `mesh.loop_to_region()`, `mesh.select_edge_loop_multi()` / `select_edge_ring_multi()` (loops or rings through the selected edges), `mesh.select_face_by_sides()`, `mesh.select_non_manifold()`, `mesh.select_loose()` and `mesh.shortest_path_select()`. The click-to-pick operators take the element by index through Blender's redo arguments: `mesh.loop_select(object_index=0, edge_index=2)` (the loop through edge 2; `extend=True` adds it) and `mesh.edgering_select(object_index=0, edge_index=2)`. Interactive tools such as `mesh.knife_tool` are not available.

### 10.3 Operators that need arguments the viewport would give

- `mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})` and the other extrude macros take the move as the nested transform argument (short form `TRANSFORM_OT_translate.value=0,0,-0.9`); `mesh.extrude_faces_move(TRANSFORM_OT_shrink_fatten={"value": 0.5})` moves each face along its own normal.
- `mesh.loopcut(number_cuts=2, edge_index=10, object_index=0)` cuts the ring through edge 10 (find the edge with `edges.foreach_get('vertices')`); `mesh.loopcut_slide(MESH_OT_loopcut={…}, TRANSFORM_OT_edge_slide={"value": 0.5})` also slides the new loop. Only the new loops are left selected (in face select mode the meshes switch to edge select mode, as in Blender).
- `mesh.spin(steps=8, angle=radians(180), center=(x, y, z), axis=(0, 1, 0))` and `mesh.screw(…)`: always pass `center` and `axis` (there is no view to take them from).
- `mesh.bevel(offset=0.03, segments=3)`: always pass `offset`. Its default `offset=0` bevels nothing (the response says so with the hint `NO_OP_DEFAULT`). The bevel acts on the selected edges: with faces selected that is every edge of those faces, inner ones included, so use `mesh.region_to_loop()` first to bevel only their outline (`mug_rim_bevel.py`).
- `transform.translate` / `rotate` / `resize` move the selection; `value` is in global axes unless you pass `orient_type`. The pivot is `bpy.context.scene.tool_settings.transform_pivot_point` (`'MEDIAN_POINT'` by default; `'INDIVIDUAL_ORIGINS'` scales each selected island about its own centre).

### 10.4 Recipes

Table legs (`table_legs.py`): cut the bottom of a slab into a 3x3 grid and extrude the four corner cells.

```
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1), scale=(1.2, 0.8, 0.05))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1))
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.data.meshes['Cube.001'].polygons.foreach_get('center')        # corner cells: 4, 6, 8, 12
bpy.ops.frap.select_elements(type='FACE', indices=[4, 6, 8, 12])
bpy.ops.mesh.inset(thickness=0.08, use_individual=True)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
bpy.ops.object.mode_set(mode='OBJECT')
```

Mug body (`mug_body.py`): inset the top cap to leave a rim, then extrude it down into a cavity.

```
bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.inset(thickness=0.05)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
```

Mug handle (`mug_handle_spin.py`): a small filled circle added in Edit Mode beside the body, spun half a turn about a horizontal axis.

```
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=0.06, location=(0.48, 0, 0.8))
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.spin(steps=8, angle=-3.141592653589793, center=(0.48, 0, 0.5), axis=(0, 1, 0))
```

Other workflows in the same directory: a round table on a pedestal (inset + two extrusions + `transform.resize`), loop-cut legs tapered with `INDIVIDUAL_ORIGINS`, a table built from five cubes and split with `mesh.separate(type='LOOSE')`, a chair with legs and a backrest, half a chair cut with `mesh.bisect(…, clear_inner=True)` and rebuilt with `mesh.symmetrize(direction='POSITIVE_X')`, a stool whose bottom is `mesh.poke`d into triangles for three legs, pointed legs collapsed with `mesh.merge(type='COLLAPSE')`, a bevelled table top, a rounded mug rim (`region_to_loop` + `bevel`), an open cup with `mesh.solidify`, a cup bridged between two circles with `mesh.bridge_edge_loops` and capped with `mesh.edge_face_add`, a ring lathed with a full `mesh.spin`, a vase shaped with `loopcut_slide`, a cushion (`subdivide(smoothness=1)` + `vertices_smooth`), a shelf from `mesh.extrude_repeat`, a drawer duplicated out of a cabinet and separated, seams and sharp edges for UVs (`mark_seam`, `mark_sharp`), a clean-up pass (`fill_holes`, `dissolve_limited`, `remove_doubles`), a table and a chair edited together, and a mug exported to glTF.

### 10.5 Pitfalls

- Name things right after creating them (`bpy.context.object.name = 'Table'`, `bpy.context.object.data.name = 'Table'`); otherwise read the names from `result.object` and `changes.added`.
- Read indices again after any operator that changes topology. The side faces of an extrusion come out in a different order in each Blender run (frap's order is fixed); their geometry is the same.
- `len(…)` and other built-ins are not supported: read the collection path instead (it reports `len`), or the `edit_mesh` block of the response.
- The select mode is a scene tool setting (`bpy.context.scene.tool_settings.mesh_select_mode`): it is still `FACE` the next time you enter Edit Mode, and `mesh.select_mode(type='FACE')` is then `CANCELLED` with a WARNING. That is harmless; read `context.select_mode` in the response to know the current mode.
- `bpy.ops.mesh.delete(type='VERT')` with nothing selected is `CANCELLED` with a WARNING report; check `selected` in `edit_mesh` first.
- Ask for help inside the session: `bpy.ops.frap.describe(target='mesh.spin')` lists every argument with its default and unit.

## 11. Checking a model

- `bpy.ops.frap.scene_summary(objects='SELECTED')` answers, in one response, every object with its mode, transform, materials, modifiers, element counts and world-space bounds (`bounds_min`, `bounds_max`), plus totals. A mesh in Edit Mode is summarised from its edit mesh. Use it after a modelling step to check sizes and counts.
- `bpy.ops.frap.render_contact_sheet(filepath='out/sheet.png', views={'FRONT', 'RIGHT', 'TOP', 'ISO'}, view_size=256)` renders the scene with Workbench from several synthetic orthographic views (and `'CAMERA'`, the scene camera) into one labelled PNG, without adding cameras to `bpy.data`. The response lists each tile's rectangle and camera. Look at the image when your harness can show it to you.
