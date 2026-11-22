# M7 stabilization: export_meshopt_compression_enable compresses the animation samplers as the
# add-on's sampler modules do. The factory cube with keyed location, rotation_euler and scale,
# an armature with a keyed pose bone, then a keyed shape key: the fcurves path
# (export_force_sampling=False, before the shape key is keyed: the add-on raises
# UnboundLocalError on key_blocks channels there) writes TIME, location and scale EXPONENTIAL
# and rotations QUATERNION at byteStride 8 (CUBICSPLINE); the sampled paths (EXT and KHR) write
# the object rotation at byteStride 16, the bone rotation at 8 and the weights SK_ANIM; rotation
# accessors become normalized SHORT. The comparator decodes the views with meshoptimizer and
# compares every compressed accessor's layout (mode, filter, byteStride, count, componentType,
# normalized, fallback byteLength) and the sampled animation values.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/curves.glb,glb:out/sampled_ext.glb,glb:out/sampled_khr.glb glb_structure
import bpy
bpy.data.objects['Cube'].shape_key_add(name='Basis', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='Lift', from_mix=False)
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].data[0].co = (1.0, 1.0, 1.5)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=1)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=1)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.5, 1.2)
bpy.data.objects['Cube'].location = (1.0, 2.0, 0.5)
bpy.data.objects['Cube'].scale = (1.5, 1.0, 0.5)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', frame=10)
bpy.data.objects['Cube'].keyframe_insert('location', frame=10)
bpy.data.objects['Cube'].keyframe_insert('scale', frame=10)
bpy.ops.object.armature_add(location=(0.0, 3.0, 0.0))
bpy.context.object.name = 'Rig'
bpy.data.objects['Rig'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Rig'].pose.bones['Bone'].keyframe_insert('location', frame=1)
bpy.data.objects['Rig'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.3, 0.1, 0.2)
bpy.data.objects['Rig'].pose.bones['Bone'].location = (0.0, 0.5, 0.0)
bpy.data.objects['Rig'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=10)
bpy.data.objects['Rig'].pose.bones['Bone'].keyframe_insert('location', frame=10)
bpy.ops.export_scene.gltf(filepath='out/curves.glb', export_meshopt_compression_enable=True, export_force_sampling=False)
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].value = 0.0
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].keyframe_insert('value', frame=1)
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].value = 1.0
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].keyframe_insert('value', frame=10)
bpy.ops.export_scene.gltf(filepath='out/sampled_ext.glb', export_meshopt_compression_enable=True)
bpy.ops.export_scene.gltf(filepath='out/sampled_khr.glb', export_meshopt_compression_enable=True, export_meshopt_extension='KHR_meshopt_compression')
