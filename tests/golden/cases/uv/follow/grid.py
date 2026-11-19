# WP 3.8: uv.follow_active_quads extends the UVs of the active quad (made active by
# mesh.loop_select's face_index) to the selected quads, breadth first: along the picked face
# loop with EVEN spacing, then over the whole uneven grid with LENGTH_AVERAGE and LENGTH; a
# seam stops the walk. Without an active face: "No active face".
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.follow_active_quads()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 8, 15, 22])
bpy.ops.transform.translate(value=(0.12, 0.05, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[12, 13])
bpy.ops.transform.translate(value=(0, 0.1, 0.2))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.loop_select(object_index=0, edge_index=7, face_index=7)
bpy.ops.uv.follow_active_quads(mode='EVEN')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Grid'].uv_layers.active.data[30].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.follow_active_quads()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Grid'].uv_layers.active.data[30].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[20, 21, 22])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.loop_select(object_index=0, edge_index=7, face_index=7)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.follow_active_quads(mode='LENGTH')
bpy.ops.object.mode_set(mode='OBJECT')
