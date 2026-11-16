# Deviations from Blender 5.2

Owned by the integrator (ARCHITECTURE §2.1 principle 4, §21.8). Every place where frap
deliberately behaves differently from Blender 5.2.4 is listed here, with its rationale and the
normalisation the golden runner applies on the Blender side (`tools/blender/golden_runner.py::NORMALISE`
mirrors the **Golden** column; WP 1.8 keeps them in sync). Work packages add entries through their
PR description; the integrator merges them here. `--describe` surfaces the operator-level ones as
`frap_note` / `headless`.

Status: M1 W0 (2026-11-16). "Since" is the milestone in which the behaviour exists in code.

## 1. Deliberate deviations

| ID | Area | Blender 5.2 | frap | Why | Golden normalisation | Since | Ref |
|---|---|---|---|---|---|---|---|
| D-01 | Operator positional args | `bpy.ops.mesh.primitive_cube_add(2)` returns `{'FINISHED'}`: the positional is silently ignored | `TypeError`, code `OP_ARG_POSITIONAL`; only execution-context strings (`'EXEC_DEFAULT'`, `'INVOKE_DEFAULT'`, …) are accepted | A silent no-op argument is a bug in every agent script | none (cases do not pass positionals; T0 error cases are `skip_blender` for this line) | M0 | §1 #21, §10.2 |
| D-02 | `INVOKE_*` execution contexts | Runs the operator's invoke (interactive) | Runs exec; adds an INFO report `frap: INVOKE_DEFAULT runs as EXEC_DEFAULT (headless)` | Headless: there is no event to invoke with | none | M0 | §10.2 |
| D-03 | Value clamping | Values outside the hard range are clamped silently | Clamped the same way, plus an INFO report (`frap_rna::clamp_report`): `frap: size=-5 clamped to hard min 0.0`, `… clamped to hard max <max>`, arrays `frap: location=(…) clamped to hard range [<min>, <max>]` (Python `repr` numbers) | Agents learn the range instead of guessing | reports are not compared | M0 | §9.2 |
| D-04 | Derived values after a write (`matrix_world`, `dimensions`, `bound_box`, evaluated meshes) | Stale until the depsgraph updates | Always fresh with respect to the current originals | No correct script relies on stale reads | `bpy.context.view_layer.update()` before every derived-value read, **unless time is dirty** (a pending `frame_current` change must not be applied, §7.3) | M1 | §1 #7, §7.3 |
| D-05 | Mesh reads in Edit Mode | `mesh.vertices` etc. show the data from before entering Edit Mode | Fresh, through a cached read-only view of the edit BMesh | Same as D-04; reads never mutate state | `obj.update_from_editmode()` before mesh reads of objects in Edit Mode | M2 | §1 #8, §6.3 |
| D-06 | Mesh data writes in Edit Mode | Silently discarded on leaving Edit Mode | `RuntimeError`, code `RNA_EDITMODE_WRITE`, hint `bpy.ops.object.mode_set(mode='OBJECT')` | A write that is lost later is worse than an error | none (cases do not write mesh data in Edit Mode) | M2 | §6.3 |
| D-07 | Render and export with animation | The render depsgraph / exporter's `frame_set` re-apply animation to the originals, overwriting manual edits of animated channels | Render and export run on a COW clone of the state with animation applied; the originals keep manual edits. Pixels and GLB values match Blender | Rendering must not change the scene | none needed for outputs; cases do not read originals of animated channels after a render/export | M6 (clone machinery from M1) | §7.3, §19.4 |
| D-08 | `//` in `export_scene.gltf(filepath=…)` | The add-on passes the path raw: `//out/part.glb` writes to the absolute `/out/part.glb` | `//` is resolved relative to `bpy.data.filepath` (or the cwd when unsaved), like every other Blender path | Writing to the filesystem root is never intended | goldens never use `//` export paths | M1 | §17.2 |
| D-09 | `foreach_get` with one argument | Requires a buffer: `foreach_get("co", seq)` | Also accepts `foreach_get("co")` and returns a bulk array; the two-argument form with a literal list returns the filled list | No variables in the input language | none (cases use the Blender form) | M1 | §13.3 |
| D-10 | Virtual 3D View | `-b` has a window but no VIEW_3D area; `V`-poll operators fail without an override | frap behaves as if one VIEW_3D area with the factory Layout viewport matrix exists (`align='VIEW'`, `camera_add` sets `scene.camera`, `V` polls pass) | The interactive defaults are what scripts expect | every statement runs inside `bpy.context.temp_override(window, area=<VIEW_3D>, region=<WINDOW>)` of the Layout screen | M1 | §8, §20.3 |
| D-11 | Factory startup | Contains the grease-pencil material `Dots Stroke` | Omitted | Grease pencil is out of scope | `Dots Stroke` is excluded from material comparisons | M0 | §5.7 |
| D-12 | `render.engine = 'CYCLES'` | Path-traced render | Rendered with the EEVEE-like engine + WARNING `Engine 'CYCLES' is not available in frap; rendered with BLENDER_EEVEE` | No path tracer | render goldens never use CYCLES | M5 | §19.3 |
| D-13 | `print(a, b)` | Writes Python reprs to stdout | Returns `"value": [a, b]` as JSON in the response | stdout belongs to the protocol (rule 2.2.7) | `print` lines are compared by outcome only | M1 | §13.3 |
| D-14 | Non-finite floats | `nan`, `inf` | JSON `{"$float":"nan"}`, `{"$float":"inf"}`, `{"$float":"-inf"}` | JSON has no NaN/Inf | the dumpers use the same encoding | M1 | §13.3 |

## 2. Temporary gaps (not deviations: frap does not implement it yet)

These raise `NotImplementedError` (code `OP_NOT_IMPLEMENTED` or `NOT_IMPLEMENTED`) instead of
behaving differently; each disappears when the named WP lands. Listed so that golden authors know
what to avoid.

| Gap | Until | Owner |
|---|---|---|
| `align='VIEW'` / `'CURSOR'` on `*_add` operators | M1 W2 (virtual view matrix in `startup.json`, placement helper) | WP 1.10, WP 1.12 |
| `enter_editmode=True`, `object.mode_set(mode='EDIT')` (no mode handler) | M2 | WP 2.x (`modes/edit_mesh.rs`) |
| `export_scene.gltf` props other than `filepath`, `export_format='GLB'` (M0 subset: other keywords raise `TypeError` as unknown) | M1 W4: all 110 props accepted, unsupported non-default values give a WARNING | WP 1.23 |
| Assigning `ID.name`, RNA function calls, paths below a property (`location.x`, `location[0]`), most `bpy.context` members | M1 W2 | WP 1.10, WP 1.14 |
| Smooth corner normals split at sharp edges, n-gon `BLI_polyfill_2d` triangulation (M0 fans) | M1 W1 | WP 1.4 |
| `render.render` with `BLENDER_EEVEE` (renders with Workbench + WARNING until M5) | M5 | WP 5.x |
| Known Blender operators that frap does not implement yet (e.g. `bpy.ops.mesh.bevel(offset=0.1)`) raise `AttributeError` `OP_UNKNOWN` (`could not be found`) instead of `NotImplementedError` `OP_NOT_IMPLEMENTED` (§10.2, §13.4), because `frap-spec-catalog` is empty | M1 W1–W2 (catalog generated, registry falls back to it) | WP 1.1, WP 1.12 |
| Factory startup has no `World` (`bpy.context.scene.world` is `None`; Blender 5.2.4 has `World("World")`, §5.7) | M1 W2 (built from `spec/blender-5.2/startup.json`) | WP 1.10 |
| Primitive meshes lack Blender's internal selection attributes `.select_vert`, `.select_edge`, `.select_poly`, `.uv_select_vert`, `.uv_select_edge`, `.uv_select_face` (verified on `primitive_cube_add`, 5.2.4): positions, topology, `UVMap` and `sharp_face` match; `mesh.attributes` reads (WP 1.27) would differ | M1 W1 (generators add them as Blender does) | WP 1.4 |

## 3. Protocol notes (frap envelope, not Blender behaviour)

- Response kind `"invalid"`: input that never became a statement (syntax error, invalid UTF-8 on a
  line) gets one `ERROR` response of kind `invalid` (M0). Part of the protocol contract since
  M1 W0 (§13.2; codes `NOT_IMPLEMENTED`, `RNA_NOT_IMPLEMENTED`, `RNA_RUNTIME` are in §13.4).
- The missing parent directories of an export path are created, **as Blender's add-on does**
  (verified: it returns `FINISHED` for a nonexistent directory) — parity, recorded here because it
  surprises reviewers.
