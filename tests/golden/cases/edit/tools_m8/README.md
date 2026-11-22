# `edit/tools_m8`: mesh topology and deform tools (WP 8.3.3)

Goldens of `mesh.sort_elements`, `reorder_vertices_spatial`, `rip`, `rip_move`, `rip_edge`,
`rip_edge_move`, `vertices_smooth_laplacian`, `symmetry_snap`, `face_make_planar`,
`circularize`, `flatten` and `space_edge_loops_evenly` against Blender 5.2.4.

## `rip_*.py`: recording the interactive result

`mesh.rip` has no `exec` and `blender -b` has no window event state, so Blender cannot run it
headless. frap runs it with the mouse over the 3D cursor in the factory Layout 3D View
(deviation D-150). The Blender side of `rip_edges.py` and `rip_verts.py` rebuilds the result an
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
