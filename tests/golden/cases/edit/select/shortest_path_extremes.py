# WP 2.4: mesh.shortest_path_select with the checker interval at its property limits:
# nth + skip and offset + depth overflow C's int and wrap as in Blender (no error). Each path
# runs along one row of the grid, kept selected by the following ADD calls. For skip=INT_MAX,
# (INT_MAX, INT_MAX, INT_MAX), (1, INT_MAX, -INT_MAX) and (2, INT_MAX-1, 5) Blender's wrapped
# interval keeps no path element, so those calls check the outcome (FINISHED) and that no
# path element is selected; (INT_MAX, 1) and (3, 2, -INT_MAX) select visible patterns.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 7])
bpy.ops.mesh.shortest_path_select(skip=2147483647)
bpy.ops.frap.select_elements(type='VERT', indices=[8, 15], action='ADD')
bpy.ops.mesh.shortest_path_select(nth=2147483647, skip=1)
bpy.ops.frap.select_elements(type='VERT', indices=[16, 23], action='ADD')
bpy.ops.mesh.shortest_path_select(nth=2147483647, skip=2147483647, offset=2147483647)
bpy.ops.frap.select_elements(type='VERT', indices=[24, 31], action='ADD')
bpy.ops.mesh.shortest_path_select(nth=1, skip=2147483647, offset=-2147483647)
bpy.ops.frap.select_elements(type='VERT', indices=[32, 39], action='ADD')
bpy.ops.mesh.shortest_path_select(nth=2, skip=2147483646, offset=5)
bpy.ops.frap.select_elements(type='VERT', indices=[40, 47], action='ADD')
bpy.ops.mesh.shortest_path_select(nth=3, skip=2, offset=-2147483647)
