# WP 8.3.1: mesh.select_next_item / select_prev_item (Blender's Python operators): stepping a
# row of grid vertices, edges and faces forward from the last two selected elements (the new
# element becomes the active one), stepping back (the history is set again after each snapshot:
# frap does not keep it across Edit Mode exits, D-54), the INFO cases (one element only, an empty
# history, a hidden next element) and a diagonal step.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_prev_item()
bpy.ops.frap.select_elements(type='VERT', indices=[8])
bpy.ops.mesh.select_next_item()
bpy.ops.frap.select_elements(type='VERT', indices=[8, 9])
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_next_item()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'row'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[8, 9, 10, 11, 12])
bpy.ops.mesh.select_prev_item()
bpy.ops.mesh.select_prev_item()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'back'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 8])
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_next_item()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'diagonal'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 4])
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_next_item()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'edges'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[7, 8])
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_next_item()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'faces'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[11])
bpy.ops.mesh.hide(unselected=False)
bpy.ops.frap.select_elements(type='FACE', indices=[9, 10])
bpy.ops.mesh.select_next_item()
bpy.ops.mesh.select_prev_item()
