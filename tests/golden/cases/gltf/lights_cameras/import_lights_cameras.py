# WP 3.4: import of cameras and KHR_lights_punctual (each tool imports its own export): the
# correction rotation undone, cd/lux back to W (SPEC), spot cones back to spot_size and
# spot_blend, the camera yfov as a vertical-fit FOV lens, clip range, orthographic scale;
# `range` is ignored (use_custom_distance stays off).
# The imported data values are copied into object transforms so they are compared.
# golden: startup=factory tier=T1 compare=objects,context tol=1e-4
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
bpy.ops.object.camera_add(location=(3, -5, 1), rotation=(1.1, 0, 0.3))
bpy.data.cameras['Camera.001'].type = 'ORTHO'
bpy.data.cameras['Camera.001'].ortho_scale = 7
bpy.data.cameras['Camera.001'].clip_end = 250
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.ops.export_scene.gltf(filepath='out/lc.glb', export_cameras=True, export_lights=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/lc.glb')
bpy.data.objects.new('Probe1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe1'])
bpy.data.objects['Probe1'].location = (bpy.data.lights['Spot'].energy, bpy.data.lights['Spot'].spot_size, bpy.data.lights['Spot'].spot_blend)
bpy.data.objects['Probe1'].scale = bpy.data.lights['Spot'].color
bpy.data.objects.new('Probe2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe2'])
bpy.data.objects['Probe2'].location = (bpy.data.lights['Sun'].energy, bpy.data.lights['Light'].energy, bpy.data.cameras['Camera'].lens)
bpy.data.objects['Probe2'].scale = (bpy.data.cameras['Camera'].sensor_height, bpy.data.cameras['Camera'].clip_start, bpy.data.cameras['Camera'].clip_end)
bpy.data.objects.new('Probe3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe3'])
bpy.data.objects['Probe3'].location = (bpy.data.cameras['Camera.001'].ortho_scale, bpy.data.cameras['Camera.001'].clip_start, bpy.data.cameras['Camera.001'].clip_end)
bpy.data.objects['Probe3'].scale = (bpy.data.cameras['Camera'].angle_y, bpy.data.cameras['Camera'].angle, 1)
bpy.data.objects.new('Probe4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe4'])
bpy.data.objects['Probe4'].location = (bpy.data.lights['Spot'].use_custom_distance, bpy.data.lights['Spot'].cutoff_distance, bpy.data.lights['Spot'].shadow_soft_size)
