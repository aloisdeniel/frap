# WP 1.21 (T0), view3d.snap_selected_to_cursor: with offset (default) the selection's median
# goes to the cursor, without it every object does.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.data.objects['Light'].select_set(True)
bpy.ops.view3d.snap_selected_to_cursor()
bpy.context.scene.cursor.location = (-1, 0, 2)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=False)
