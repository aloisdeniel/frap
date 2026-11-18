# WP 2.8: mesh.vert_connect_path connects vertices in selection order (frap.select_elements
# sets the history): across the faces of a grid, then by plain edges between loose vertices
# (closing the path when every edge exists), and the error for a face selection.
# golden: startup=empty compare=objects,context,mesh:Grid,mesh:Loose tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 6, 14, 12])
bpy.ops.mesh.vert_connect_path()
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2])
bpy.ops.mesh.vert_connect_path()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Loose').from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)], [], [])
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Loose'])
bpy.context.view_layer.objects.active = bpy.data.objects['Loose']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 0, 1, 3])
bpy.ops.mesh.vert_connect_path()
bpy.ops.frap.select_elements(type='VERT', indices=[2, 0, 1, 3])
bpy.ops.mesh.vert_connect_path()
