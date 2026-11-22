# WP 8.3.3: mesh.rip / mesh.rip_move on selected edges, headless-extended (D-153): the mouse
# is the 3D cursor seen in the factory Layout 3D View. Rows of a grid ripped towards the cursor
# above and below (the selection's ends extend by one edge), a single edge, a boundary column
# (no extension at the boundary) and rip_move with a translation. `blender -b` cannot run
# rip (no window event state), so the Blender side applies the same edge split
# (`bmesh.ops.split_edges`, which `BM_mesh_edgesplit` also runs) and the selection that an
# interactive Blender 5.2.4 leaves with the mouse over the cursor (recorded with
# `--enable-event-simulate`, tests/golden/cases/edit/tools_m8/README.md).
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Grid.001,mesh:Grid.002,mesh:Grid.003,mesh:Grid.004 tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[11, 12, 13])
bpy.context.scene.cursor.location = (0.0, 0.6, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); P = [{10, 11}, {11, 12}, {12, 13}, {13, 14}]; bmesh.ops.split_edges(bm, edges=[e for e in bm.edges if {v.index for v in e.verts} in P], verts=[v for v in bm.verts if v.index in (11, 12, 13)], use_verts=True); bmesh.update_edit_mesh(bpy.context.object.data)
#@blender bpy.ops.frap.select_elements(type='VERT', indices=[11, 12, 13])
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[10, 11, 12, 13])
bpy.context.scene.cursor.location = (0.2, -0.7, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); P = [{10, 11}, {11, 12}, {12, 13}, {13, 14}]; bmesh.ops.split_edges(bm, edges=[e for e in bm.edges if {v.index for v in e.verts} in P], verts=[v for v in bm.verts if v.index in (10, 11, 12, 13)], use_verts=True); bmesh.update_edit_mesh(bpy.context.object.data)
#@blender bpy.ops.frap.select_elements(type='VERT', indices=[26, 25, 27, 28])
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[12, 13])
bpy.context.scene.cursor.location = (0.25, -0.5, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); P = [{11, 12}, {12, 13}, {13, 14}]; bmesh.ops.split_edges(bm, edges=[e for e in bm.edges if {v.index for v in e.verts} in P], verts=[v for v in bm.verts if v.index in (12, 13)], use_verts=True); bmesh.update_edit_mesh(bpy.context.object.data)
#@blender bpy.ops.frap.select_elements(type='VERT', indices=[25, 26])
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 6, 11, 16, 21])
bpy.context.scene.cursor.location = (-0.8, 0.1, 0.0)
#@frap bpy.ops.mesh.rip()
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); P = [{1, 6}, {6, 11}, {11, 16}, {16, 21}]; bmesh.ops.split_edges(bm, edges=[e for e in bm.edges if {v.index for v in e.verts} in P], verts=[v for v in bm.verts if v.index in (1, 6, 11, 16, 21)], use_verts=True); bmesh.update_edit_mesh(bpy.context.object.data)
#@blender bpy.ops.frap.select_elements(type='VERT', indices=[25, 26, 27, 28, 29])
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[11, 12, 13])
bpy.context.scene.cursor.location = (0.0, -0.6, 0.0)
#@frap bpy.ops.mesh.rip_move(TRANSFORM_OT_translate={'value': (0.0, -0.3, 0.0)})
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); P = [{10, 11}, {11, 12}, {12, 13}, {13, 14}]; bmesh.ops.split_edges(bm, edges=[e for e in bm.edges if {v.index for v in e.verts} in P], verts=[v for v in bm.verts if v.index in (11, 12, 13)], use_verts=True); bmesh.update_edit_mesh(bpy.context.object.data)
#@blender bpy.ops.frap.select_elements(type='VERT', indices=[25, 26, 27])
#@blender bpy.ops.transform.translate(value=(0.0, -0.3, 0.0))
