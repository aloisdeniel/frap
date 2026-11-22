# WP 8.2: cube corners with seven segments and a profile of 0.7 (the superellipse
# fullness and the cube-corner subdivision at a non-circular exponent).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.3, segments=7, profile=0.7)
