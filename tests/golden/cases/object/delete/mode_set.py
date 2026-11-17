# WP 1.19 (T0), object.mode_set, kept with the delete cases (no object/modes golden area in M1):
# OBJECT in Object Mode is a no-op (also with toggle); `mode` items follow the active object's
# type (POSE is not a mesh mode, EDIT not a camera mode); the poll messages for a hidden and a
# missing active object. (EDIT on a mesh needs the M2 mode handler: tests/cli/m1/object_prereq.)
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='OBJECT', toggle=True)
bpy.ops.object.mode_set()
bpy.ops.object.mode_set(mode='POSE')
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Camera'].hide_viewport = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = None
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set.poll()
