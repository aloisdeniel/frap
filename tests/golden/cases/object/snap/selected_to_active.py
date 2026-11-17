# WP 1.21 (T0), view3d.snap_selected_to_active, including a child of a moved parent; no active
# object: RuntimeError No active element found!
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (0.4, 1.6, -0.2)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.view3d.snap_selected_to_active()
bpy.context.view_layer.objects.active = None
bpy.ops.view3d.snap_selected_to_active()
