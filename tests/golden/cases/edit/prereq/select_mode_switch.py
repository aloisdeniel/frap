# WP 2.2: mesh.select_mode without expand: switching modes re-flushes the selection for the
# new mode and strips the selection history of elements the new mode cannot pick; TOGGLE on
# the only mode is cancelled; use_extend combines modes; the scene tool setting follows.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 4, 5, 9])
bpy.ops.mesh.select_mode(type='EDGE')
bpy.context.scene.tool_settings.mesh_select_mode[:]
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.select_mode(type='VERT', use_extend=True)
bpy.context.scene.tool_settings.mesh_select_mode[:]
bpy.ops.mesh.select_mode(type='FACE', use_extend=True, action='DISABLE')
bpy.ops.mesh.select_mode(type='VERT', action='ENABLE')
bpy.ops.mesh.select_mode(type='EDGE', action='DISABLE')
bpy.ops.frap.select_elements(type='FACE', indices=[2], action='ADD')
bpy.ops.mesh.select_mode(type='EDGE')
