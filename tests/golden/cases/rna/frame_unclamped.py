# M1 stabilization: `Scene.frame_current` skips the RNA hard range (`rna_Scene_frame_current_set`
# applies its own): 1e9 is kept, a negative frame becomes 0, past the int range is a ValueError.
# The frames are copied to the 3D cursor to be compared (object matrices stay stale in Blender
# while time is dirty, §7.3).
# golden: startup=factory tier=T0 compare=context
import bpy
bpy.context.scene.frame_current = 1000000000
bpy.context.scene.cursor.location.x = bpy.context.scene.frame_current
bpy.context.scene.frame_current = -1000000000
bpy.context.scene.cursor.location.y = bpy.context.scene.frame_current
bpy.context.scene.frame_current = 2147483647
bpy.context.scene.cursor.location.z = bpy.context.scene.frame_current
bpy.context.scene.frame_current = 10000000000
bpy.context.scene.frame_current = 1.5
