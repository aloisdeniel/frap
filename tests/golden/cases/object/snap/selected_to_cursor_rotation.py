# WP 1.21 (T0), view3d.snap_selected_to_cursor(use_rotation=True): offsets turn with the cursor
# and objects take its rotation (world space, through a parent, deltas ignored).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (0.3, 0, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.view3d.snap_selected_to_cursor(use_rotation=True)
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].scale = (1, 3, 1)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=False, use_rotation=True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Camera'].delta_rotation_euler = (0, 0.2, 0)
bpy.ops.view3d.snap_selected_to_cursor(use_offset=False, use_rotation=True)
