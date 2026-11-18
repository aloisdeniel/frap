# WP 2.4: mesh.select_edge_loop_multi from the selected edges of a cube and of a cylinder
# (edges on the n-gon caps: the NGONS delimiter), two objects in Edit Mode. (Not a UV sphere:
# its edge order differs from Blender's, D-19.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 5])
bpy.ops.mesh.select_edge_loop_multi()
