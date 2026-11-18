# WP 2.6: mesh.subdivide_edgering over three ring pairs (four rims after a first edgering):
# Blender orders the pairs by the heap addresses of its loop stores ("order of execution with
# 2+ rings is undefined"), so element order is not reproducible; compared canonically (T2).
# golden: startup=factory compare=objects,mesh:Cylinder tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 3, 6, 9, 12, 15, 18, 21])
bpy.ops.mesh.subdivide_edgering(number_cuts=2, interpolation='LINEAR')
bpy.ops.mesh.subdivide_edgering(number_cuts=1, interpolation='LINEAR', profile_shape_factor=0.4, profile_shape='SHARP')
