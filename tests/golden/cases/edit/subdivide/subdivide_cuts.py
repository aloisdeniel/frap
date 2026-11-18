# WP 2.6: mesh.subdivide with several cuts on the startup cube (inner grids of 3x3, then 4x4
# cells on every quad).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.mesh.subdivide(number_cuts=3)
