# WP 2.4: mesh.select_edge_ring_multi from the selected edges of a cube (edge 0: ring 0, 2,
# 7, 8) and a cylinder, two objects in Edit Mode.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.select_edge_ring_multi()
