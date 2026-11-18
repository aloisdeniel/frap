# WP 2.4: mesh.select_edge_loop_multi then select_edge_ring_multi without delimiters from one
# cylinder edge (the loop continues across the n-gon caps).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop=set())
bpy.ops.mesh.select_edge_ring_multi(delimit_edge_ring=set())
