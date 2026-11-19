# WP 3.9: a three-point light rig (KHR_lights_punctual, export_lights=True): a key spot with a
# coloured, blended cone and a custom distance (range), a point fill with a blackbody
# temperature, a sun rim light; the lights are parented under an empty that is rotated. The
# default lighting mode (SPEC) converts W to cd and W/m2 to lux; the COMPAT and RAW modes are
# exported too. Round trip (§17.2) of each file, imported and re-exported in its own lighting
# mode: the importer converts the intensities back (range is ignored on import, so the spot
# loses it).
# golden: startup=factory tier=T2 compare=glb:out/rig.glb,glb:out/rig_compat.glb,glb:out/rig_raw.glb,glb:out/rig_rt.glb,glb:out/rig_compat_rt.glb,glb:out/rig_raw_rt.glb
import bpy
bpy.ops.object.empty_add(location=(0, 0, 3), rotation=(0, 0, 0.6))
bpy.data.objects['Empty'].name = 'RigRoot'
bpy.ops.object.light_add(type='SPOT', location=(3, -3, 4), rotation=(0.8, 0, 0.7))
bpy.data.objects['Spot'].name = 'Key'
bpy.data.lights['Spot'].energy = 800
bpy.data.lights['Spot'].color = (1.0, 0.9, 0.7)
bpy.data.lights['Spot'].spot_size = 0.8
bpy.data.lights['Spot'].spot_blend = 0.4
bpy.data.lights['Spot'].use_custom_distance = True
bpy.data.lights['Spot'].cutoff_distance = 20
bpy.ops.object.light_add(type='POINT', location=(-3, -2, 2))
bpy.data.objects['Point'].name = 'Fill'
bpy.data.lights['Point'].energy = 300
bpy.data.lights['Point'].use_temperature = True
bpy.data.lights['Point'].temperature = 4500
bpy.ops.object.light_add(type='SUN', location=(0, 4, 5), rotation=(-0.6, 0, 3.1))
bpy.data.objects['Sun'].name = 'Rim'
bpy.data.lights['Sun'].energy = 2.5
bpy.data.lights['Sun'].color = (0.7, 0.8, 1.0)
bpy.data.objects['Key'].parent = bpy.data.objects['RigRoot']
bpy.data.objects['Fill'].parent = bpy.data.objects['RigRoot']
bpy.data.objects['Rim'].parent = bpy.data.objects['RigRoot']
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.export_scene.gltf(filepath='out/rig.glb', export_lights=True)
bpy.ops.export_scene.gltf(filepath='out/rig_compat.glb', export_lights=True, export_import_convert_lighting_mode='COMPAT')
bpy.ops.export_scene.gltf(filepath='out/rig_raw.glb', export_lights=True, export_import_convert_lighting_mode='RAW')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rig.glb')
bpy.ops.export_scene.gltf(filepath='out/rig_rt.glb', export_lights=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rig_compat.glb', export_import_convert_lighting_mode='COMPAT')
bpy.ops.export_scene.gltf(filepath='out/rig_compat_rt.glb', export_lights=True, export_import_convert_lighting_mode='COMPAT')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rig_raw.glb', export_import_convert_lighting_mode='RAW')
bpy.ops.export_scene.gltf(filepath='out/rig_raw_rt.glb', export_lights=True, export_import_convert_lighting_mode='RAW')
