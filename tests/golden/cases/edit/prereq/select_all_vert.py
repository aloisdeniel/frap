# WP 2.2: mesh.select_all in vertex select mode: TOGGLE from everything selected deselects,
# TOGGLE again selects, INVERT of a partial selection (with the selection history validated
# by the flush), DESELECT clears the history, SELECT keeps it.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='TOGGLE')
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.ops.mesh.select_all(action='INVERT')
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5], action='ADD')
bpy.ops.mesh.select_all(action='SELECT')
