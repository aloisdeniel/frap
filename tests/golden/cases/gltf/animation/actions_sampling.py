# WP 6.7: export_animation_mode='ACTIONS' (the default): one glTF animation per action, named
# after it. Sampled (export_force_sampling=True, the default): every frame of the action's
# range, keyed channels LINEAR, a constant keyed channel two STEP keys, unkeyed channels left
# out. Not sampled: BEZIER keys become CUBICSPLINE (tangents from the handles, first in- and
# last out-tangent zero), LINEAR stays LINEAR, CONSTANT becomes STEP, a single key is sampled.
# export_frame_step samples every n frames.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/sampled.glb,glb:out/curves.glb,glb:out/step3.glb glb_structure
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (2.0, 1.0, 0.5)
bpy.data.objects['Cube'].rotation_euler = (0.0, 0.0, 0.5)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=11)
bpy.data.objects['Cube'].scale = (1.0, 2.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=3)
bpy.data.objects['Cube'].scale = (1.0, 1.0, 3.0)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=9)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=0).keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=1).keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=2).keyframe_points[0].interpolation = 'LINEAR'
bpy.ops.object.empty_add(location=(0.0, 0.0, 1.0))
bpy.data.objects['Empty'].keyframe_insert('location', frame=2)
bpy.data.objects['Empty'].location = (0.0, 0.0, 6.0)
bpy.data.objects['Empty'].keyframe_insert('location', frame=6)
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].interpolation = 'CONSTANT'
bpy.ops.export_scene.gltf(filepath='out/sampled.glb')
bpy.ops.export_scene.gltf(filepath='out/curves.glb', export_force_sampling=False)
bpy.ops.export_scene.gltf(filepath='out/step3.glb', export_frame_step=3)
