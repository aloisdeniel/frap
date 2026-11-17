# WP 1.21 (T0), object.parent_clear with nothing selected and with no parents: FINISHED, nothing
# changes; its poll passes without an active object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.parent_clear()
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = None
bpy.ops.object.parent_clear()
