# WP 2.2: assigning tool_settings.mesh_select_mode in Edit Mode switches the active edit
# mesh (EDBM_selectmode_set): three selected vertices of a face select nothing in face mode,
# an empty mode is ignored, and later operators see the converted selection.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.context.tool_settings.mesh_select_mode = (False, False, True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
bpy.context.tool_settings.mesh_select_mode = (False, False, False)
bpy.context.tool_settings.mesh_select_mode[:]
bpy.ops.mesh.select_all(action='INVERT')
bpy.data.meshes['Cube'].total_vert_sel
bpy.context.scene.tool_settings.mesh_select_mode = (True, False, False)
bpy.ops.frap.select_elements(type='VERT', indices=[3], action='ADD')
bpy.context.tool_settings.mesh_select_mode = (False, True, False)
