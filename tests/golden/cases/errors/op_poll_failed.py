# WP 1.26: an operator whose poll fails (code OP_POLL_FAILED, RuntimeError) with the poll
# message Blender reports: no active object, an active object hidden in the viewport. The state
# is unchanged after each failure.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.context.view_layer.objects.active = None
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.data.objects['Camera'].hide_viewport = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].hide_viewport = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.object.mode_set(mode='OBJECT')
