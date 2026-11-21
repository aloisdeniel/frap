# WP 6.7: F-curves on object properties that are not transforms (hide_render, hide_viewport,
# color). With export_force_sampling=False a group the add-on cannot export from its keys
# (one key, mixed interpolations) is queued for sampling with
# get_channel_from_target(get_target(prop)), which is None for such a property, so nothing is
# sampled for it: a lone hide_render key exports no animation, and with keyed scale the
# animation keeps a single CUBICSPLINE scale channel (never a second, sampled one on the same
# node and path). A group that can be exported from its keys (two CONSTANT hide_viewport keys)
# has no glTF path and is skipped too. Sampled, only the keyed transforms are written.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/one.glb,glb:out/scaled.glb,glb:out/mixed.glb,glb:out/sampled.glb glb_structure
import bpy
bpy.data.objects['Cube'].keyframe_insert('hide_render', frame=3)
bpy.ops.export_scene.gltf(filepath='out/one.glb', export_force_sampling=False)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=1)
bpy.data.objects['Cube'].scale = (1.0, 2.0, 3.0)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=10)
bpy.ops.export_scene.gltf(filepath='out/scaled.glb', export_force_sampling=False)
bpy.data.objects['Cube'].keyframe_insert('hide_viewport', frame=2)
bpy.data.objects['Cube'].keyframe_insert('hide_viewport', frame=8)
bpy.data.objects['Cube'].keyframe_insert('color', frame=1)
bpy.data.objects['Cube'].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('color', frame=5)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('color', index=0).keyframe_points[0].interpolation = 'LINEAR'
bpy.ops.export_scene.gltf(filepath='out/mixed.glb', export_force_sampling=False)
bpy.ops.export_scene.gltf(filepath='out/sampled.glb')
