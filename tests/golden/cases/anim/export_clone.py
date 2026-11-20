# WP 6.4, deviation D-07: export_scene.gltf with export_current_frame=False (the default)
# exports the animation of frame 0 (the add-on's frame_set(0)): the GLB matches Blender's.
# Blender's closing frame_set(original) then re-applies the current frame's animation to the
# originals (a manual edit of an animated channel is lost); frap exports a clone and leaves the
# originals as they are, so this case reads no animated original after an export.
# export_current_frame=True exports the originals as they are, manual edit included.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/frame0.glb,glb:out/current.glb
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (2.0, 1.0, 0.5)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.context.scene.frame_set(6)
bpy.data.objects['Cube'].location.x = 400.0
bpy.ops.export_scene.gltf(filepath='out/frame0.glb', export_animations=False)
bpy.data.objects['Cube'].location.x = 400.0
bpy.ops.export_scene.gltf(filepath='out/current.glb', export_animations=False, export_current_frame=True)
