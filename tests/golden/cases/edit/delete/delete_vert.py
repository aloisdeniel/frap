# WP 2.7: mesh.delete(type='VERT') removes the selected vertices with every edge and face
# using them and deselects everything; with nothing selected it is CANCELLED. The default
# type is VERT.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3])
bpy.ops.mesh.delete(type='VERT')
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.delete(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.mesh.delete()
