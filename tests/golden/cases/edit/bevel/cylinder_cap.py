# WP 2.11: the edges of a cylinder's top cap (an 8-gon: two beveled edges and one unbeveled
# edge at each vertex, loop slide), three segments; then every edge of a second cylinder
# (valence-3 corners between an n-gon and quads) with four segments.
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Cylinder.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[6])
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.bevel(offset=0.15, segments=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.1, segments=4)
