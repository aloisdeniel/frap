# WP 2.4: mesh.shortest_path_select without a selection history (the first two selected
# elements of the select mode) and with edge_mode tagging the edge path as seam and sharp
# instead of selecting it; a selection path that is already selected is deselected.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_random(ratio=0.05, seed=2)
bpy.ops.mesh.shortest_path_select()
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 30])
bpy.ops.mesh.shortest_path_select(edge_mode='SEAM')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 25])
bpy.ops.mesh.shortest_path_select(edge_mode='SHARP')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 30])
bpy.ops.mesh.shortest_path_select(edge_mode='SEAM')
bpy.ops.mesh.shortest_path_select()
bpy.ops.mesh.shortest_path_select()
