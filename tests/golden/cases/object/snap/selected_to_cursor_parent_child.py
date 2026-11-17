# WP 1.21 (T0), view3d.snap_selected_to_cursor of a parent and its child: both are moved from
# their world location before the operator (the child ends up offset twice), and the
# ACTIVE_ELEMENT pivot centers the offset on the active object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Light'].select_set(True)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=False)
bpy.data.objects['Light'].parent = None
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.context.scene.cursor.location = (0, 0, 0)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=True)
