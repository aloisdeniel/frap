# WP 3.4: `light.type` re-types the RNA struct (PointLight -> SpotLight -> SunLight ->
# AreaLight) and keeps every value; properties of other types raise AttributeError; the
# export follows the current type (here SPOT again, with the values set while it was SPOT).
# golden: startup=factory tier=T2 compare=objects,glb:out/retype.glb
import bpy
bpy.data.lights['Light'].spot_size = 0.5
bpy.data.lights['Light'].type = 'SPOT'
bpy.data.lights['Light'].spot_size = 0.5
bpy.data.lights['Light'].spot_blend = 0.75
bpy.data.lights['Light'].type = 'SUN'
bpy.data.lights['Light'].angle = 0.1
bpy.data.lights['Light'].spot_blend = 0.1
bpy.data.lights['Light'].type = 'AREA'
bpy.data.lights['Light'].shape = 'DISK'
bpy.data.lights['Light'].angle = 0.1
bpy.data.lights['Light'].type = 'SPOT'
bpy.data.objects['Cube'].location = (bpy.data.lights['Light'].spot_size, bpy.data.lights['Light'].spot_blend, bpy.data.lights['Light'].energy)
bpy.data.objects['Cube'].scale = bpy.data.lights['Light'].temperature_color
bpy.ops.export_scene.gltf(filepath='out/retype.glb', export_lights=True)
