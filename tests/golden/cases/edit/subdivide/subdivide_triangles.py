# WP 2.6: mesh.subdivide on triangles: an ico sphere fully cut (triangle grids, 1 and 2 cuts),
# and a cone where only the side edges are cut (single-edge triangle pattern with ngon=False).
# golden: startup=factory compare=objects,mesh:Icosphere,mesh:Icosphere.001,mesh:Cone tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 4])
bpy.ops.mesh.subdivide(number_cuts=2, ngon=False)
