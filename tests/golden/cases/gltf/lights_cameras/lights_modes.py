# WP 3.4: export_import_convert_lighting_mode: COMPAT (no 683 factor) and RAW (energy as is,
# times 2 ** exposure); a spot without blend (inner cone = outer cone) and a sun with a
# non-normalized disk area.
# golden: startup=factory tier=T2 compare=glb:out/compat.glb,glb:out/raw.glb glb_structure
import bpy
bpy.ops.object.light_add(type='SPOT', location=(1, 2, 3))
bpy.data.lights['Spot'].spot_blend = 0
bpy.data.lights['Spot'].exposure = -0.5
bpy.ops.object.light_add(type='SUN', location=(0, 0, 5))
bpy.data.lights['Sun'].normalize = False
bpy.data.lights['Sun'].angle = 0.2
bpy.data.lights['Light'].exposure = 2
bpy.ops.export_scene.gltf(filepath='out/compat.glb', export_lights=True, export_import_convert_lighting_mode='COMPAT')
bpy.ops.export_scene.gltf(filepath='out/raw.glb', export_lights=True, export_import_convert_lighting_mode='RAW')
