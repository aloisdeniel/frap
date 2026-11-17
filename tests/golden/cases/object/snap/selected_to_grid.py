# WP 1.21 (T0), view3d.snap_selected_to_grid, including a child of a snapped parent; nothing
# selected: FINISHED.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (0.4, 1.6, -0.2)
bpy.data.objects['Light'].select_set(True)
bpy.ops.view3d.snap_selected_to_grid()
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.view3d.snap_selected_to_grid()
bpy.ops.view3d.snap_selected_to_cursor()
