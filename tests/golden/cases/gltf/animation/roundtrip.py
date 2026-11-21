# WP 6.7: round trip. A sampled export (LINEAR keys at every frame) is imported (one action per
# animation, stashed on NLA tracks, the first assigned) and exported again: in ACTIONS mode the
# assigned action and the stashed strips (muted tracks, unmuted strips) give the same
# animations; NLA_TRACKS bakes each stash track. A camera's animation carries the add-on's
# camera rotation correction both ways.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/first.glb,glb:out/again.glb,glb:out/tracks.glb,anim:Move glb_structure
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=2)
bpy.data.objects['Cube'].location = (3.0, 0.0, 1.0)
bpy.data.objects['Cube'].rotation_euler = (0.0, 0.5, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=8)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=8)
bpy.data.objects['Cube'].rotation_euler = (0.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=3)
bpy.data.actions['CubeAction'].name = 'Move'
bpy.data.objects['Camera'].keyframe_insert('rotation_euler', frame=1)
bpy.data.objects['Camera'].rotation_euler = (1.0, 0.2, 0.3)
bpy.data.objects['Camera'].keyframe_insert('rotation_euler', frame=6)
bpy.data.actions['CameraAction'].name = 'Pan'
bpy.ops.export_scene.gltf(filepath='out/first.glb', export_cameras=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/first.glb')
bpy.ops.export_scene.gltf(filepath='out/again.glb', export_cameras=True)
bpy.ops.export_scene.gltf(filepath='out/tracks.glb', export_cameras=True, export_animation_mode='NLA_TRACKS')
