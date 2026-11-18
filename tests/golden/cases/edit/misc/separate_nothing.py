# WP 2.10: mesh.separate errors: nothing selected in any edit mesh is "Nothing selected";
# LOOSE on a single part separates nothing (CANCELLED).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.separate()
bpy.ops.mesh.separate(type='LOOSE')
bpy.ops.mesh.separate(type='MATERIAL')
bpy.data.objects['Cube'].mode
