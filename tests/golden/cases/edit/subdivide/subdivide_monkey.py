# WP 2.6: subdivide, loop cut and un-subdivide on Suzanne (quads and triangles, UVs): the whole
# mesh once with ngon=False, then a ring of quads stopped by triangles, then un-subdivide.
# golden: startup=factory compare=objects,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(ngon=False)
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=40, object_index=0)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.unsubdivide(iterations=1)
