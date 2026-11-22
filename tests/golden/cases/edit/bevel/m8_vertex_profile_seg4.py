# WP 8.2: vertex bevel with four segments and a profile of 0.8 on two cube corners.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 6])
bpy.ops.mesh.bevel(offset=0.4, segments=4, profile=0.8, affect='VERTICES')
