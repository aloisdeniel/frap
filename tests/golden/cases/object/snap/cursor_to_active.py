# WP 1.21 (T0), view3d.snap_cursor_to_active: the active object (hidden or not, selected or not);
# no active object CANCELLED.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].hide_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.view3d.snap_cursor_to_active()
bpy.context.scene.cursor.location
bpy.context.view_layer.objects.active = None
bpy.ops.view3d.snap_cursor_to_active()
