# WP 2.4: frap.scene_summary answers in `result` and changes nothing (Blender side: a no-op
# emulation that succeeds), in Object Mode and in Edit Mode, for every `objects` filter and
# with evaluated=False.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T0
import bpy
bpy.ops.frap.scene_summary()
bpy.ops.frap.scene_summary(objects='SELECTED', evaluated=False)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.frap.scene_summary(objects='VISIBLE')
bpy.ops.frap.scene_summary(objects='ALL', evaluated=False)
