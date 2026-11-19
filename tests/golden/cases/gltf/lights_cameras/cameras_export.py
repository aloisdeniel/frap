# WP 3.4: glTF cameras (export_cameras=True): perspective yfov from the sensor fit (AUTO,
# HORIZONTAL, VERTICAL) and the render aspect (landscape, then portrait with a pixel aspect),
# aspectRatio, znear/zfar, orthographic xmag/ymag, a panoramic camera (node without camera),
# the camera correction rotation, a mesh parented to a camera (expressed relative to the
# corrected camera matrix), and two objects sharing one camera datablock.
# golden: startup=factory tier=T2 compare=glb:out/landscape.glb,glb:out/portrait.glb
import bpy
bpy.ops.object.camera_add(location=(0, -5, 1), rotation=(1.2, 0, 0.1))
bpy.data.cameras['Camera.001'].sensor_fit = 'VERTICAL'
bpy.data.cameras['Camera.001'].lens = 40
bpy.data.cameras['Camera.001'].clip_start = 0.5
bpy.data.cameras['Camera.001'].clip_end = 250
bpy.ops.object.camera_add(location=(3, -5, 1))
bpy.data.cameras['Camera.002'].type = 'ORTHO'
bpy.data.cameras['Camera.002'].ortho_scale = 7
bpy.ops.object.camera_add(location=(-3, -5, 1))
bpy.data.cameras['Camera.003'].sensor_fit = 'HORIZONTAL'
bpy.data.cameras['Camera.003'].sensor_width = 24
bpy.data.cameras['Camera.003'].shift_x = 0.2
bpy.ops.object.camera_add(location=(0, 5, 1))
bpy.data.cameras['Camera.004'].type = 'PANO'
bpy.data.objects.new('Twin', bpy.data.cameras['Camera.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Twin'].location = (1, 1, 1)
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.ops.export_scene.gltf(filepath='out/landscape.glb', export_cameras=True)
bpy.context.scene.render.resolution_x = 1000
bpy.context.scene.render.resolution_y = 1500
bpy.context.scene.render.pixel_aspect_x = 2
bpy.ops.export_scene.gltf(filepath='out/portrait.glb', export_cameras=True)
