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
- `frap_describe(target: string)` returns the manifest fragment of an operator (`op:mesh.bevel` or `mesh.bevel`), a struct (`struct:Object` or `Object`), a struct member (`Object.location`, `Object.select_set`), or `errors`.

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
- `frap_describe({target})`: run `frap --describe=<target>` in a separate process and return its stdout (the manifest does not depend on the scene, so it does not have to be the serve process). In this build `bpy.ops.frap.describe(target=…)` and `bpy.ops.<ns>.<op>.get_rna_type()` inside a session are not wired to the manifest yet (`OP_NOT_IMPLEMENTED`); they arrive in a later milestone.

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

The full schemas of the request, response, error, ready/end events and done record are in `frap --describe=protocol`.

## 5. Ask before calling: `poll()` and `--describe`

- `bpy.ops.mesh.bevel.poll()` returns `False` (a `get`-kind response) without changing anything. On failure `detail.unmet` lists the unmet requirements. Use it to check the mode before an edit-mode operator. `detail.requires`/`unmet` of `OP_POLL_FAILED` use the manifest's requirement codes (`MODE_EDIT_MESH`).
- Every operator's manifest entry has `poll.requires`, the declarative requirements frap checks (`MODE_OBJECT`, `MODE_EDIT_MESH`, `ACTIVE_OBJECT`, `ACTIVE_TYPE(MESH)`, `HAS_SELECTED_OBJECTS`, …), plus `poll.text` in words. `poll.matrix` is Blender's dumped poll result in contexts A–E (legend in `poll_contexts`). An operator frap does not implement has `requires: null` and only the dumped matrix.
- `bpy.ops.mesh.bevel.idname()` returns `"MESH_OT_bevel"`. `bpy.ops.mesh.bevel.get_rna_type()` will return the operator's describe fragment once it is wired (later milestone); until then use `frap --describe op:mesh.bevel`.

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

**Read `defaults_that_surprise` once.** Some defaults surprise: `export_scene.gltf` does not apply modifiers (`export_apply=False`) and exports neither cameras nor lights; `render.render()` writes nothing to disk unless `write_still=True`; `mesh.bevel`'s default `offset=0` does nothing; a light from `object.light_add` has 10 W; `export_scene.gltf` exports objects at their frame-0 rest transforms unless `export_current_frame=True`. The `scale` of `mesh.primitive_cube_add`, `cone_add`, `cylinder_add`, `uv_sphere_add` and `ico_sphere_add` defaults to a `(0, 0, 0)` placeholder that means 1 and scales the new mesh data (the object's scale stays 1). `mesh.primitive_plane_add`, `grid_add`, `circle_add`, `monkey_add` and every `curve.primitive_*`/`surface.primitive_*` operator ignore `scale` (their `unset_semantics` says `ignored`): set `object.scale` or use `transform.resize` afterwards. `mesh.primitive_torus_add` has no `scale`.

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
