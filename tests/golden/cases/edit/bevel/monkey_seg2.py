# WP 2.11: every edge of Suzanne with two segments (triangles, quads, many valences).
# golden: startup=factory compare=mesh:Suzanne tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.02, segments=2)
