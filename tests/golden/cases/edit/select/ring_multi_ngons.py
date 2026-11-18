# WP 2.4: mesh.select_edge_ring_multi(delimit_edge_ring={'NGONS'}) from a cylinder edge.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_ring_multi(delimit_edge_ring={'NGONS'})
