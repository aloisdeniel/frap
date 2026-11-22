# WP 2.11: every edge of a cone: the tip is an 8-way corner (ADJ vertex mesh with 8
# boundary verts), two segments.
# golden: startup=factory compare=mesh:Cone tier=T1
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2)
