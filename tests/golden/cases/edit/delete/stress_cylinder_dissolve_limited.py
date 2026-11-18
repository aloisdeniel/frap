# WP 2.7: mesh.dissolve_limited on a 16-sided cylinder: the sides merge, the caps stay (the
# edge between cap and side is sharp); with use_dissolve_boundaries the vertices between
# two edges go too.
# golden: startup=empty compare=objects,mesh:Cylinder,mesh:Cylinder.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited(angle_limit=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=16, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited(angle_limit=0.5, use_dissolve_boundaries=True)
bpy.ops.object.mode_set(mode='OBJECT')
