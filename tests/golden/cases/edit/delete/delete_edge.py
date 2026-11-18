# WP 2.7: mesh.delete(type='EDGE') removes the selected edges, their faces and the vertices
# left without edges; with no edge selected it is CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 5])
bpy.ops.mesh.delete(type='EDGE')
bpy.ops.mesh.delete(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.delete(type='EDGE')
