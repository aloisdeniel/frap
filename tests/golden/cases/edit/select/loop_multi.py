# WP 2.4: mesh.select_edge_loop_multi / select_edge_ring_multi from the selected edges of a
# cube (edge 0: ring 0, 2, 7, 8) and a cylinder (n-gon caps: the NGONS delimiters), two
# objects in Edit Mode. (Not a UV sphere: its edge order differs from Blender's, D-19.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.select_edge_ring_multi()
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 5])
bpy.ops.mesh.select_edge_loop_multi()
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop=set())
bpy.ops.mesh.select_edge_ring_multi(delimit_edge_ring=set())
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_ring_multi(delimit_edge_ring={'NGONS'})
