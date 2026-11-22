# WP 2.11: every cube edge with three segments: cube-corner vertex meshes with a center
# ngon, UV snapping across the cube's seams.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.3, segments=3)
