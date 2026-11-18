# WP 2.11: vmesh_method CUTOFF on cube corners (three segments).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 order=exact
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=3, vmesh_method='CUTOFF')
