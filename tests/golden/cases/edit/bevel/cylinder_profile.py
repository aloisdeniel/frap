# WP 2.11: every edge of a cylinder with three segments and a concave profile (0.25):
# odd segments at valence-3 corners of an n-gon.
# golden: startup=factory compare=mesh:Cylinder tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.1, segments=3, profile=0.25)
