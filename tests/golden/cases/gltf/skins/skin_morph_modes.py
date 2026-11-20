# M5+M6 stabilization (exit verifier scenario s10): skinned bone channels and morph-weight
# animation in every export_animation_mode: a two-bone rig (envelope-parented sphere) with an
# NLA track and an active action, a shape key with its own NLA strip, and an animated light;
# exported with ACTIVE_ACTIONS, NLA_TRACKS, BROADCAST, SCENE, and SCENE without the per-object
# split at frame_step 3 with forced sampling.
# golden: startup=factory tier=T2 tol=1e-4 compare=glb:out/active.glb,glb:out/tracks.glb,glb:out/broad.glb,glb:out/scene.glb,glb:out/scene1.glb
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.0, location=(0, 0, 3))
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Armature'].data.edit_bones['Bone'].tail = (0, 0, 2.5)
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.objects['Armature'].data.edit_bones['Bone'].select_tail = True
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0, 1.5)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_ENVELOPE')
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_quaternion = (0.92, 0.39, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].keyframe_insert('rotation_quaternion', frame=12)
bpy.data.objects['Sphere'].shape_key_add(name='Basis')
bpy.data.objects['Sphere'].shape_key_add(name='Squash')
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Squash'].data[0].co = (0.0, 0.0, 2.0)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Squash'].data[3].co = (0.5, 0.5, 0.2)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Squash'].keyframe_insert('value', frame=2)
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Squash'].value = 1.0
bpy.data.objects['Sphere'].data.shape_keys.key_blocks['Squash'].keyframe_insert('value', frame=8)
bpy.data.objects['Light'].data.energy = 100.0
bpy.data.objects['Light'].data.keyframe_insert('energy', frame=1)
bpy.data.objects['Armature'].animation_data.nla_tracks.new()
bpy.data.objects['Armature'].animation_data.nla_tracks[0].name = 'Wave'
bpy.data.objects['Armature'].animation_data.nla_tracks[0].strips.new('Wave', 1, bpy.data.actions['ArmatureAction'])
bpy.data.actions.new('Bend')
bpy.data.objects['Armature'].animation_data.action = bpy.data.actions['Bend']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.95, 0.0, 0.31, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=4)
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=14)
bpy.data.objects['Sphere'].data.shape_keys.animation_data.nla_tracks.new()
bpy.data.objects['Sphere'].data.shape_keys.animation_data.nla_tracks[0].strips.new('Sq', 3, bpy.data.actions['SphereAction'])
bpy.context.scene.frame_end = 16
bpy.ops.export_scene.gltf(filepath='out/active.glb', export_animation_mode='ACTIVE_ACTIONS')
bpy.ops.export_scene.gltf(filepath='out/tracks.glb', export_animation_mode='NLA_TRACKS')
bpy.ops.export_scene.gltf(filepath='out/broad.glb', export_animation_mode='BROADCAST')
bpy.ops.export_scene.gltf(filepath='out/scene.glb', export_animation_mode='SCENE')
bpy.ops.export_scene.gltf(filepath='out/scene1.glb', export_animation_mode='SCENE', export_anim_scene_split_object=False, export_force_sampling=True, export_frame_step=3)
