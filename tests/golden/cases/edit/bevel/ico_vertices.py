# WP 2.11: every vertex of an icosphere with one segment (valence 5 and 6 corners, pentagon
# and hexagon caps).
# golden: startup=factory compare=mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.1, affect='VERTICES')
