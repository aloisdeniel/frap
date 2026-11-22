# WP 2.11: clamp_overlap limits a too large offset so beveled edges don't overlap.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=1.5, segments=2, clamp_overlap=True)
