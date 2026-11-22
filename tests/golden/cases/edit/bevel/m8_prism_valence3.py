# WP 8.2: a triangular prism (valence-3 corners whose faces are not orthogonal): the
# general `adj_vmesh` subdivision with four segments.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=4)
