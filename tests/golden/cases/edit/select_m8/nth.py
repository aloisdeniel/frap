# WP 8.3.1: mesh.select_nth (checker deselect) from the active vertex, without an active
# element (the first selected vertex), with nth/skip/offset, in edge mode (a connected loop
# and isolated parallel edges, which walk across faces), in face mode from the active face,
# along a closed chain (the grid's boundary ring), and with nothing selected (Blender's error).
# Intermediate selections are kept as renamed `.select_*` attributes.
# golden: startup=factory compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[7], action='ADD')
bpy.ops.mesh.select_nth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'active_v'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_nth(nth=2, skip=1, offset=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'first_v'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3, 4, 5, 11, 17, 23, 29, 35, 34, 33, 32, 31, 30, 24, 18, 12, 6])
bpy.ops.mesh.select_nth(skip=2, nth=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'ring_v'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[12], action='ADD')
bpy.ops.mesh.select_nth(skip=1, nth=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'edges_e'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 5, 8, 11, 14])
bpy.ops.mesh.select_nth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'ring_e'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[12], action='ADD')
bpy.ops.mesh.select_nth(offset=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'faces_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_nth()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[8, 9, 10, 14, 15, 16])
bpy.ops.mesh.select_nth(skip=1, nth=1, offset=-1)
