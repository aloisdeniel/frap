# WP 3.9: camera shots (export_cameras=True). The factory camera (perspective, 50 mm, AUTO
# fit on the 16:9 render), a 85 mm close-up with a VERTICAL sensor fit and tight clipping, and
# an orthographic top view (ortho_scale 12 -> xmag/ymag); a square render resolution changes
# every aspectRatio and yfov in the second file. Round trip (§17.2) of both files with
# cameras: the importer rebuilds lens and sensor fit; after read_homefile the render is 16:9
# again, so the square file's cameras re-export with that aspect.
# golden: startup=factory tier=T2 compare=glb:out/shots.glb,glb:out/shots_square.glb,glb:out/shots_rt.glb,glb:out/shots_square_rt.glb glb_structure
import bpy
bpy.ops.object.camera_add(location=(0, -4, 1.5), rotation=(1.4, 0, 0))
bpy.context.object.name = 'CloseUp'
bpy.data.cameras['Camera.001'].lens = 85
bpy.data.cameras['Camera.001'].sensor_fit = 'VERTICAL'
bpy.data.cameras['Camera.001'].clip_start = 0.2
bpy.data.cameras['Camera.001'].clip_end = 30
bpy.ops.object.camera_add(location=(0, 0, 10))
bpy.context.object.name = 'Top'
bpy.data.cameras['Camera.002'].type = 'ORTHO'
bpy.data.cameras['Camera.002'].ortho_scale = 12
bpy.ops.export_scene.gltf(filepath='out/shots.glb', export_cameras=True)
bpy.context.scene.render.resolution_x = 1080
bpy.context.scene.render.resolution_y = 1080
bpy.ops.export_scene.gltf(filepath='out/shots_square.glb', export_cameras=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/shots.glb')
bpy.ops.export_scene.gltf(filepath='out/shots_rt.glb', export_cameras=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/shots_square.glb')
bpy.ops.export_scene.gltf(filepath='out/shots_square_rt.glb', export_cameras=True)
