# WP 1.21 (T0), view3d.snap_cursor_to_grid (metric, scale_length 0.5, imperial, none).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (0.4, 0.6, -1.5)
bpy.ops.view3d.snap_cursor_to_grid()
bpy.context.scene.cursor.location
bpy.context.scene.unit_settings.scale_length = 0.5
bpy.context.scene.cursor.location = (0.4, 0.6, -1.3)
bpy.ops.view3d.snap_cursor_to_grid()
bpy.context.scene.cursor.location
bpy.context.scene.unit_settings.scale_length = 1.0
bpy.context.scene.unit_settings.system = 'IMPERIAL'
bpy.context.scene.cursor.location = (0.4, 0.6, -1.3)
bpy.ops.view3d.snap_cursor_to_grid()
bpy.context.scene.cursor.location
bpy.context.scene.unit_settings.system = 'NONE'
bpy.context.scene.unit_settings.scale_length = 0.5
bpy.context.scene.cursor.location = (0.4, 0.6, -1.3)
bpy.ops.view3d.snap_cursor_to_grid()
