# WP 6.7: animation times. The add-on's seconds are frame / (fps * fps_base), not Blender's
# frame * fps_base / fps (the fps_base quirk, kept: 30 fps with fps_base 1.001 here).
# export_frame_range clips to the scene's playback range; export_anim_slide_to_zero starts
# every animation at 0 s; negative frames slide to 0 (export_negative_frame='SLIDE', the
# default) or are cropped ('CROP'), in the sampled and the F-curve exports.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/fps.glb,glb:out/range.glb,glb:out/zero.glb,glb:out/slide.glb,glb:out/crop.glb,glb:out/crop_curves.glb,glb:out/zero_curves.glb
import bpy
bpy.context.scene.render.fps = 30
bpy.context.scene.render.fps_base = 1.001
bpy.context.scene.frame_start = 4
bpy.context.scene.frame_end = 9
bpy.data.objects['Cube'].keyframe_insert('location', frame=2)
bpy.data.objects['Cube'].location = (1.0, 2.0, 3.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=12)
bpy.ops.object.empty_add(location=(1.0, 0.0, 0.0))
bpy.data.objects['Empty'].keyframe_insert('rotation_euler', frame=-4)
bpy.data.objects['Empty'].rotation_euler = (0.3, 0.2, 1.0)
bpy.data.objects['Empty'].keyframe_insert('rotation_euler', frame=3)
bpy.ops.export_scene.gltf(filepath='out/fps.glb')
bpy.ops.export_scene.gltf(filepath='out/range.glb', export_frame_range=True)
bpy.ops.export_scene.gltf(filepath='out/zero.glb', export_anim_slide_to_zero=True)
bpy.ops.export_scene.gltf(filepath='out/slide.glb', export_negative_frame='SLIDE')
bpy.ops.export_scene.gltf(filepath='out/crop.glb', export_negative_frame='CROP')
bpy.ops.export_scene.gltf(filepath='out/crop_curves.glb', export_negative_frame='CROP', export_force_sampling=False)
bpy.ops.export_scene.gltf(filepath='out/zero_curves.glb', export_anim_slide_to_zero=True, export_force_sampling=False, export_frame_range=True)
