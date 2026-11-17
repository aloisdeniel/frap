# WP 1.21 (T0), view3d.snap_cursor_to_center: location and rotation reset; in quaternion mode
# only the quaternion is reset.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (0.3, 0.2, 0.1)
bpy.ops.view3d.snap_cursor_to_center()
bpy.context.scene.cursor.rotation_euler
bpy.context.scene.cursor.rotation_mode = 'QUATERNION'
bpy.context.scene.cursor.rotation_quaternion = (0.5, 0.5, 0.5, 0.5)
bpy.context.scene.cursor.rotation_euler = (0.1, 0.2, 0.3)
bpy.ops.view3d.snap_cursor_to_center()
bpy.context.scene.cursor.rotation_quaternion
bpy.context.scene.cursor.rotation_euler
bpy.context.scene.cursor.matrix
