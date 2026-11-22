# WP 8.2: a five-sided cone, every edge with four segments: a valence-5 tip and
# valence-3 base corners.
# golden: startup=factory compare=objects,context,mesh:Cone tier=T1
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.15, segments=4)
