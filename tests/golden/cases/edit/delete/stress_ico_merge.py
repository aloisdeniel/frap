# WP 2.7: merge and collapse on an ico sphere: CENTER of scattered vertices (faces fold and
# are rebuilt in place), COLLAPSE of scattered edges, then edge_collapse and delete_loose.
# golden: startup=empty compare=objects,context,mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 11, 22, 33, 44, 55, 66, 77, 88, 99, 110, 121, 132, 143, 154])
bpy.ops.mesh.merge(type='CENTER')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 25, 48, 71, 94, 117, 140, 163, 186, 209, 232, 255, 278, 301, 324, 347, 370, 393])
bpy.ops.mesh.merge(type='COLLAPSE')
bpy.ops.frap.select_elements(type='EDGE', indices=[7, 38, 69, 100, 131, 162, 193, 224, 255, 286])
bpy.ops.mesh.edge_collapse()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete_loose()
