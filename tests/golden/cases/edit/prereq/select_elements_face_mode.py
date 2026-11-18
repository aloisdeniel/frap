# WP 2.2: frap.select_elements in face select mode: removing a face keeps the edges and
# vertices of the selected neighbours; selecting vertices in face mode selects no face until
# every vertex of one is selected.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 3, 4])
bpy.ops.frap.select_elements(type='FACE', indices=[4], action='REMOVE')
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[2, 3, 6], action='ADD')
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.frap.select_elements(type='VERT', indices=[7], action='ADD')
