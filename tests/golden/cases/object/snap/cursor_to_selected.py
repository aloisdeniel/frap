# WP 1.21 (T0), view3d.snap_cursor_to_selected: median, bounds center with the bounding-box
# pivot, hidden objects ignored, nothing selected CANCELLED.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.view3d.snap_cursor_to_selected()
bpy.context.scene.cursor.location
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.view3d.snap_cursor_to_selected()
bpy.context.scene.cursor.location
bpy.data.objects['Light'].hide_set(True)
bpy.ops.view3d.snap_cursor_to_selected()
bpy.context.scene.cursor.location
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.view3d.snap_cursor_to_selected()
