# WP 1.26: `object.mode_set` polls on the active object: no active object fails, a camera
# passes, an object hidden with `hide_set` still passes, one hidden with `hide_viewport` fails
# ("Cannot edit hidden object"); `select_all` and `delete` do not need an active object.
# Blender's values are asserted on `#@blender` lines; the calls check frap's (see poll/factory).
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.context.view_layer.objects.active = None
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all.poll()
#@blender assert bpy.ops.object.select_all.poll() is True
bpy.ops.object.delete.poll()
#@blender assert bpy.ops.object.delete.poll() is True
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].hide_set(True)
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].hide_set(False)
bpy.data.objects['Camera'].hide_viewport = True
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].hide_viewport = False
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
