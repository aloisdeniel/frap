# WP 2.4: mesh.shortest_path_select between the last two selection-history entries (vertex,
# edge and face paths, topology distance, face stepping, the skip/nth/offset checker, use_fill
# regions) and between the first two selected elements without history; a single selected
# element is CANCELLED with Blender's WARNING.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 35])
bpy.ops.mesh.shortest_path_select()
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[2, 33])
bpy.ops.mesh.shortest_path_select(use_topology_distance=True, use_face_step=True)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[6, 11])
bpy.ops.mesh.shortest_path_select(skip=1, nth=1)
bpy.ops.frap.select_elements(type='VERT', indices=[7, 28])
bpy.ops.mesh.shortest_path_select(use_fill=True)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[9])
bpy.ops.mesh.shortest_path_select()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 50])
bpy.ops.mesh.shortest_path_select()
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 40])
bpy.ops.mesh.shortest_path_select(use_fill=True)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 24])
bpy.ops.mesh.shortest_path_select()
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[2, 22])
bpy.ops.mesh.shortest_path_select(use_face_step=True, nth=2, skip=1, offset=1)
bpy.ops.frap.select_elements(type='FACE', indices=[5, 19])
bpy.ops.mesh.shortest_path_select(use_fill=True)
bpy.data.meshes['Grid'].total_face_sel
