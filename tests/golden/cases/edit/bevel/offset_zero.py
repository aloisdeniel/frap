# WP 2.11: the default offset 0 bevels nothing but the operator finishes (frap adds a
# WARNING); with nothing selected the operator is cancelled.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel()
bpy.ops.mesh.bevel(segments=3)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.bevel(offset=0.2)
