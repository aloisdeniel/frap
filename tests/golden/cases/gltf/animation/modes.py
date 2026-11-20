# WP 6.7: every export_animation_mode on one scene. Cube: active action 'Move' (location) plus
# an NLA track 'Spin' holding action 'Spin' (rotation); Empty: active action 'Hop' and an NLA
# track named after the default ('NlaTrack') holding 'Move' too. ACTIONS: one animation per
# action, the actions shared by two objects merged ('Move' on Cube and Empty);
# export_merge_animation='NONE' keeps one per object and action. ACTIVE_ACTIONS: the assigned
# actions in one animation named export_nla_strips_merged_animation_name. BROADCAST: every
# action on every animated object. NLA_TRACKS: one animation per track, baked with only that
# track playing (an object without tracks exports none). SCENE: the scene range baked, one
# animation per object, or one named after the scene without export_anim_scene_split_object.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/actions.glb,glb:out/nomerge.glb,glb:out/active.glb,glb:out/broadcast.glb,glb:out/tracks.glb,glb:out/scene.glb,glb:out/scene_one.glb
import bpy
bpy.context.scene.frame_end = 12
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Cube'].rotation_quaternion = (0.7071068, 0.0, 0.0, 0.7071068)
bpy.data.objects['Cube'].keyframe_insert('rotation_quaternion', frame=5)
bpy.data.actions['CubeAction'].name = 'Spin'
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks[0].name = 'Spin'
bpy.data.objects['Cube'].animation_data.nla_tracks[0].strips.new('Spin', 1, bpy.data.actions['Spin'])
bpy.data.objects['Cube'].animation_data.action = None
bpy.data.objects['Cube'].keyframe_insert('location', frame=2)
bpy.data.objects['Cube'].location = (3.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=8)
bpy.data.actions['CubeAction'].name = 'Move'
bpy.ops.object.empty_add(location=(0.0, 2.0, 0.0))
bpy.data.objects['Empty'].keyframe_insert('location', frame=1)
bpy.data.objects['Empty'].location = (0.0, 2.0, 4.0)
bpy.data.objects['Empty'].keyframe_insert('location', frame=4)
bpy.data.actions['EmptyAction'].name = 'Hop'
bpy.data.objects['Empty'].animation_data.nla_tracks.new()
bpy.data.objects['Empty'].animation_data.nla_tracks[0].strips.new('Move', 3, bpy.data.actions['Move'])
bpy.data.objects['Empty'].animation_data.nla_tracks[0].strips[0].action_slot = bpy.data.actions['Move'].slots[0]
bpy.ops.export_scene.gltf(filepath='out/actions.glb')
bpy.ops.export_scene.gltf(filepath='out/nomerge.glb', export_merge_animation='NONE')
bpy.ops.export_scene.gltf(filepath='out/active.glb', export_animation_mode='ACTIVE_ACTIONS', export_nla_strips_merged_animation_name='Everything')
bpy.ops.export_scene.gltf(filepath='out/broadcast.glb', export_animation_mode='BROADCAST')
bpy.ops.export_scene.gltf(filepath='out/tracks.glb', export_animation_mode='NLA_TRACKS')
bpy.ops.export_scene.gltf(filepath='out/scene.glb', export_animation_mode='SCENE')
bpy.ops.export_scene.gltf(filepath='out/scene_one.glb', export_animation_mode='SCENE', export_anim_scene_split_object=False)
