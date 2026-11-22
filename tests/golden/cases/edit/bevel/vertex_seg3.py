# WP 2.11: vertex bevel with three segments on two cube vertices (odd segment count:
# vertex seam detection and a center polygon).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 7])
bpy.ops.mesh.bevel(offset=0.3, segments=3, affect='VERTICES')
