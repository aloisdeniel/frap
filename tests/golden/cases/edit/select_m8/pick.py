# WP 8.3.1: mesh.select_linked_pick and mesh.shortest_path_pick by index (Blender's exec,
# headless-extended as WP 2.4's pick operators): a vertex, an edge (index past the vertices)
# and a face (past the edges) picked on the second object in Edit Mode, with and without
# delimiters (the face-mode default is SEAM), deselect; shortest paths from the active
# vertex, edge and face to the picked one (the end becomes active, a face sets the active
# material index), use_fill, a checker interval, edge_mode='SEAM'; and every CANCELLED case
# (no index, index out of range, no active element, mismatched types).
# golden: startup=empty compare=objects,mesh:Grid,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(4, 0, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.data.meshes['Grid'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Grid'].materials.append(bpy.data.materials.new('B'))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_linked_pick()
bpy.ops.mesh.select_linked_pick(object_index=2, index=0)
bpy.ops.mesh.select_linked_pick(object_index=1, index=99)
bpy.ops.mesh.select_linked_pick(object_index=1, index=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'linked_vert'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_linked_pick(object_index=1, index=10, deselect=True)
bpy.ops.frap.select_elements(type='EDGE', indices=[30, 31, 32, 33, 34, 35], object='Grid')
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_linked_pick(object_index=0, index=52, delimit={'SEAM'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'linked_edge_seam'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_linked_pick(object_index=0, index=150)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'linked_face_default'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_linked_pick(object_index=0, index=150, delimit={'MATERIAL'}, deselect=True)
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.shortest_path_pick(index=40)
bpy.ops.frap.select_elements(type='VERT', indices=[0], object='Grid')
bpy.ops.mesh.shortest_path_pick()
bpy.ops.mesh.shortest_path_pick(index=4000)
bpy.ops.mesh.shortest_path_pick(index=40)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'path_vert'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[40], object='Grid', action='ADD')
bpy.ops.mesh.shortest_path_pick(index=46, skip=1, nth=1)
bpy.ops.mesh.shortest_path_pick(index=100)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'path_checker'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Grid')
bpy.ops.mesh.shortest_path_pick(index=109)
bpy.ops.mesh.shortest_path_pick(index=119, edge_mode='SEAM')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'path_edge'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2, 3], object='Grid')
bpy.context.view_layer.objects.active = bpy.data.objects['Grid']
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.context.object.active_material_index = 0
bpy.ops.frap.select_elements(type='FACE', indices=[30], object='Grid')
bpy.ops.mesh.shortest_path_pick(index=135)
bpy.ops.mesh.shortest_path_pick(index=153, use_fill=True)
