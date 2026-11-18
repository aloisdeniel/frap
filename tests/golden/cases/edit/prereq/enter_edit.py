# WP 2.2: object.mode_set(mode='EDIT') on the factory cube (everything selected, vertex mode),
# reads through the edit-mode view, then the case ends in Edit Mode: the mesh dump carries the
# selection, the active element and the (empty) selection history.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.mode
bpy.context.edit_object.name
bpy.data.meshes['Cube'].vertices[3].co
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='EDIT')
