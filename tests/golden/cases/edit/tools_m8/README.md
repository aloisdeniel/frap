# `edit/tools_m8`: mesh topology and deform tools (WP 8.3.3)

Goldens of `mesh.sort_elements`, `reorder_vertices_spatial`, `rip`, `rip_move`, `rip_edge`,
`rip_edge_move`, `vertices_smooth_laplacian`, `symmetry_snap`, `face_make_planar`,
`circularize`, `flatten` and `space_edge_loops_evenly` against Blender 5.2.4.

## `rip_*.py`: recording the interactive result

`mesh.rip` has no `exec` and `blender -b` has no window event state, so Blender cannot run it
headless. frap runs it with the mouse over `frap_location` (unset: the 3D cursor), in the
factory Layout 3D View or, with a non-zero `frap_direction`, in an orthographic view looking
along it (deviation D-150). The Blender side of `rip_edges.py` and `rip_verts.py` rebuilds the result an
interactive Blender 5.2.4 produces with the mouse there, recorded like this (one Blender
process per case; the case's setup statements run first, `emulations.select_elements` stands
in for `bpy.ops.frap.select_elements`):

```python
# Blender --factory-startup --enable-event-simulate -p 0 0 1920 1080 --python record.py -- case.json out.json
import bpy, json, sys, bmesh
from bpy_extras import view3d_utils
case, out = sys.argv[sys.argv.index("--") + 1:]
case, state = json.load(open(case)), {"step": 0}

def tick():
    win = bpy.context.window_manager.windows[0]
    area = next(a for a in win.screen.areas if a.type == 'VIEW_3D')
    region = next(r for r in area.regions if r.type == 'WINDOW')
    if state["step"] == 0:
        with bpy.context.temp_override(window=win, area=area, region=region):
            for s in case["setup"]:
                exec(s, globals())
        co = view3d_utils.location_3d_to_region_2d(
            region, area.spaces[0].region_3d, bpy.context.scene.cursor.location)
        win.event_simulate(type='MOUSEMOVE', value='NOTHING',
                           x=region.x + round(co.x), y=region.y + round(co.y))
        state["step"] = 1
        return 0.5
    with bpy.context.temp_override(window=win, area=area, region=region):
        getattr(bpy.ops.mesh, case["op"])('INVOKE_DEFAULT', **case.get("kw", {}))
        bm = bmesh.from_edit_mesh(bpy.context.object.data)
        json.dump({"verts": [list(v.co) for v in bm.verts],
                   "edges": [[v.index for v in e.verts] for e in bm.edges],
                   "faces": [[v.index for v in f.verts] for f in bm.faces],
                   "select": [v.index for v in bm.verts if v.select],
                   "history": [h.index for h in bm.select_history]}, open(out, "w"))
    bpy.ops.wm.quit_blender()

bpy.app.timers.register(tick, first_interval=1.0)
```

The interactive region is larger (2832 × 1854 on a Retina display) than the `blender -b` one
frap measures in (1574 × 954); the cases are chosen so that both give the same result (checked
by running frap with the interactive region and view matrix, which matched the recordings on
17 cases, among them the three error messages and `use_fill` on vertices and edges).

## `rip_recordings/*.json`: interactive recordings replayed exactly

Each file is one interactive Blender 5.2.4 run of `mesh.rip('INVOKE_DEFAULT', …)`: the case
(`setup` statements, `kw`, and for the `ortho_*` cases a `view` with the `direction` and
`location` the 3D View was set to: `view_perspective = 'ORTHO'`,
`view_rotation = direction.to_track_quat('-Z', 'Y')`, `view_location = location`, the mouse
over `location`), the 3D cursor, the region size, `view_matrix`, `window_matrix`, the object
matrix and the mouse position, the mesh before the call (positions, edges, faces,
`sharp_edge`/`sharp_face`, `custom_normal`, the selection and the selection history) and after
it, or the error report. `frap-ops-mesh`'s `ops::rip::tests` replays every recording three
ways and compares positions, element order, selection, history and the encoded custom
normals exactly: in the recorded region and view with the recorded mouse; the `ortho_*` ones
in frap's own region for `frap_direction`/`frap_location`; the cursor ones in frap's default
region (the factory Layout 3D View of `blender -b`). They cover the vertex and edge paths,
boundaries, a bowtie vertex, `use_fill` on both paths, custom normals (flat ones from
`set_normals_from_faces` with fill on the cube and Suzanne, smooth per-vertex ones on a grid)
and the three error messages.

They were recorded with this script, one Blender process per case (`case.json` holds `name`,
`setup`, `op`, `kw` and the optional `view`; `sel(kind, indices)` selects elements and adds
them to the history), then compacted (floats written with the fewest digits that read back
to the same `float32`, the 3D cursor taken from the setup):

```python
# Blender --factory-startup --enable-event-simulate -p 0 0 1920 1080 --python record.py -- case.json out.json
import bpy, json, sys, bmesh, traceback
from mathutils import Vector
from bpy_extras import view3d_utils
case_path, out_path = sys.argv[sys.argv.index("--") + 1:]
case, state = json.load(open(case_path)), {"step": 0}
rec = {"name": case["name"], "setup": case["setup"], "op": case["op"], "kw": case.get("kw", {}),
       "view": case.get("view")}

def sel(kind, idx):
    me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me)
    for seq in (bm.verts, bm.edges, bm.faces):
        for x in seq: x.select_set(False)
    bm.select_history.clear()
    seq = {'VERT': bm.verts, 'EDGE': bm.edges, 'FACE': bm.faces}[kind]; seq.ensure_lookup_table()
    for i in idx:
        seq[i].select_set(True); bm.select_history.add(seq[i])
    bm.select_flush_mode(); bmesh.update_edit_mesh(me)

def mesh_state(me):
    a = me.attributes
    def col(name, n):
        if name not in a: return None
        return [bool(d.value) for d in a[name].data]
    cn = None
    if "custom_normal" in a:
        cn = [list(d.value) for d in a["custom_normal"].data]
    return {"verts": [list(v.co) for v in me.vertices],
            "edges": [list(e.vertices) for e in me.edges],
            "faces": [list(p.vertices) for p in me.polygons],
            "sharp_edge": col("sharp_edge", 0), "sharp_face": col("sharp_face", 0),
            "select_vert": [v.select for v in me.vertices],
            "select_edge": [e.select for e in me.edges],
            "select_poly": [p.select for p in me.polygons],
            "custom_normal": cn,
            "corner_normals": [list(n.vector) for n in me.corner_normals] if cn is not None else None}

def history():
    bm = bmesh.from_edit_mesh(bpy.context.object.data)
    bm.verts.index_update(); bm.edges.index_update(); bm.faces.index_update()
    return [[type(h).__name__[2:].upper(), h.index] for h in bm.select_history]

def m4(m):
    # m[col][row]
    return [[m[r][c] for r in range(4)] for c in range(4)]

def tick():
    try:
        return _tick()
    except Exception:
        rec["script_error"] = traceback.format_exc()
        json.dump(rec, open(out_path, "w"))
        bpy.ops.wm.quit_blender()

def _tick():
    win = bpy.context.window_manager.windows[0]
    area = next(a for a in win.screen.areas if a.type == 'VIEW_3D')
    region = next(r for r in area.regions if r.type == 'WINDOW')
    rv3d = area.spaces[0].region_3d
    with bpy.context.temp_override(window=win, area=area, region=region):
        if state["step"] == 0:
            for s in case["setup"]:
                exec(s, globals())
            bpy.ops.object.mode_set(mode='OBJECT')
            rec["before"] = mesh_state(bpy.context.object.data)
            bpy.ops.object.mode_set(mode='EDIT')
            rec["before"]["history"] = history()
            rec["select_mode"] = list(bpy.context.tool_settings.mesh_select_mode)
            v = case.get("view")
            if v:
                rv3d.view_perspective = 'ORTHO'
                rv3d.view_rotation = Vector(v["direction"]).to_track_quat('-Z', 'Y')
                rv3d.view_location = Vector(v["location"])
            area.tag_redraw()
            state["step"] = 1
            return 0.5
        if state["step"] == 1:
            v = case.get("view")
            pt = Vector(v["location"]) if v else bpy.context.scene.cursor.location
            co = view3d_utils.location_3d_to_region_2d(region, rv3d, pt)
            state["mval"] = [round(co.x), round(co.y)]
            win.event_simulate(type='MOUSEMOVE', value='NOTHING',
                               x=region.x + state["mval"][0], y=region.y + state["mval"][1])
            state["step"] = 2
            return 0.5
        rec["mval"] = state["mval"]
        rec["region"] = [region.width, region.height]
        rec["viewmat"] = m4(rv3d.view_matrix)
        rec["winmat"] = m4(rv3d.window_matrix)
        rec["obmat"] = m4(bpy.context.object.matrix_world)
        try:
            r = getattr(bpy.ops.mesh, case["op"])('INVOKE_DEFAULT', **case.get("kw", {}))
            rec["result"] = sorted(r)[0]
        except RuntimeError as e:
            rec["result"] = "ERROR"
            rec["error"] = str(e)
        h = history()
        bpy.ops.object.mode_set(mode='OBJECT')
        rec["after"] = mesh_state(bpy.context.object.data)
        rec["after"]["history"] = h
        json.dump(rec, open(out_path, "w"))
    bpy.ops.wm.quit_blender()

bpy.app.timers.register(tick, first_interval=1.0)
```
