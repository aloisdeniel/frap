# WP 2.2: the select_mode conversion table with use_expand ([MESH §3.3]): vertex -> edge and
# vertex -> face expand to every element touching a selected vertex; edge -> face expands;
# face -> edge and face -> vertex contract to the elements surrounded by selected faces.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.select_mode(type='EDGE', use_expand=True)
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE', use_expand=True)
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.select_mode(type='VERT', use_expand=True)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6])
bpy.ops.mesh.select_mode(type='FACE', use_expand=True)
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.select_mode(type='EDGE', use_expand=True)
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE', use_expand=True)
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 4, 5])
bpy.ops.mesh.select_mode(type='VERT', use_expand=True)
