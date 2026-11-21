# WP 3.4: KHR_lights_punctual export (export_lights=True): W -> cd for point and spot,
# W/m2 -> lux for sun (SPEC: x 683), spot cones from spot_size/spot_blend, `range` only with
# use_custom_distance, colour times the blackbody colour (may exceed 1), exposure, the light
# area when `normalize` is off, the AREA light kept as a node without a light (WARNING), the
# camera/light correction rotation, and a light parented under another light.
# golden: startup=factory tier=T2 compare=glb:out/lights.glb glb_structure
import bpy
bpy.ops.object.light_add(type='SPOT', location=(1, 2, 3), rotation=(0.4, 0.1, 0.7))
bpy.data.lights['Spot'].energy = 500
bpy.data.lights['Spot'].spot_size = 0.9
bpy.data.lights['Spot'].spot_blend = 0.3
bpy.data.lights['Spot'].color = (1, 0.5, 0.25)
bpy.data.lights['Spot'].use_custom_distance = True
bpy.data.lights['Spot'].cutoff_distance = 12.5
bpy.ops.object.light_add(type='SUN', location=(0, 0, 5), rotation=(0.3, 0.2, 0))
bpy.data.lights['Sun'].energy = 3
bpy.data.lights['Sun'].use_custom_distance = True
bpy.ops.object.light_add(type='AREA', location=(0, 1, 5))
bpy.ops.object.light_add(type='POINT', location=(0, -1, 5))
bpy.data.lights['Point'].use_temperature = True
bpy.data.lights['Point'].temperature = 3000
bpy.data.lights['Point'].exposure = 1.5
bpy.data.lights['Point'].normalize = False
bpy.data.lights['Point'].shadow_soft_size = 0.3
bpy.data.objects['Point'].scale = (2, 2, 2)
bpy.data.objects['Point'].parent = bpy.data.objects['Light']
bpy.ops.export_scene.gltf(filepath='out/lights.glb', export_lights=True)
