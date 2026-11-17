# WP 1.26: scene-level settings: the 3D cursor (compared in the context), render settings,
# unit settings, `scene.camera` set to an object and cleared, and values read back from them.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.scenes['Scene'].cursor.location = (1, 2, 3)
bpy.context.scene.cursor.location.z = 4
bpy.data.scenes['Scene'].cursor.rotation_mode = 'QUATERNION'
bpy.data.scenes['Scene'].render.resolution_x = 640
bpy.data.scenes['Scene'].render.resolution_y = 480
bpy.data.scenes['Scene'].render.filepath = 'out/frame.png'
bpy.data.scenes['Scene'].unit_settings.system = 'IMPERIAL'
bpy.data.scenes['Scene'].camera = bpy.data.objects['Cube']
bpy.data.objects['Light'].location = (bpy.data.scenes['Scene'].render.resolution_x, bpy.data.scenes['Scene'].render.resolution_y, bpy.data.scenes['Scene'].render.resolution_percentage)
bpy.data.objects['Light'].name = bpy.data.scenes['Scene'].camera.name
bpy.data.objects['Camera'].name = bpy.data.scenes['Scene'].unit_settings.system
bpy.data.scenes['Scene'].camera = None
bpy.data.scenes['Scene'].camera
bpy.data.scenes['Scene'].world = None
bpy.data.scenes['Scene'].world
