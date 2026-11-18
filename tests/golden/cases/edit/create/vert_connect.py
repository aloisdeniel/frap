# WP 2.8: mesh.vert_connect joins selected vertices of a face (splitting it), connects two
# vertices across several faces (connect_vert_pair: new vertices where the path crosses
# edges, the new edges selected), and is CANCELLED with fewer than two vertices.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.vert_connect()
bpy.ops.frap.select_elements(type='VERT', indices=[0, 6])
bpy.ops.mesh.vert_connect()
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 14])
bpy.ops.mesh.vert_connect()
bpy.data.meshes['Grid'].total_edge_sel
