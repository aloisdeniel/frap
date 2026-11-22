# WP 8.3.8: uv.select_all without UV sync selection on the factory cube. Switching sync off
# rebuilds the UV selection from the mesh selection (vertex modes match); the UV editor only
# shows the mesh-selected faces, so a select_all with two faces selected in the mesh leaves a
# corner-level UV selection once every face is selected again. INVERT in the vertex UV mode
# flips UV vertices (edges/faces follow), TOGGLE deselects when anything is selected. The UV
# selection (.uv_select_vert/edge/face) and the mesh selection are dumped by renaming them.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 5])
bpy.ops.uv.select_all(action='INVERT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'uv_sel_vert'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'uv_sel_edge'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'uv_sel_face'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'sel_vert'
