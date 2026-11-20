# WP 6.7: importing glTF animations. Each glTF animation becomes one action named after it,
# with one slot per animated object; LINEAR, STEP and CUBICSPLINE samplers give LINEAR,
# CONSTANT and BEZIER keys (AUTO handles, the tangents dropped), at frame = t * fps * fps_base.
# Every action is stashed on a muted, locked NLA track named after the animation (the first
# animation on top) starting at the scene's frame_start, and the first animation is assigned
# to its objects. The probes read the NLA layout back.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,anim:Move,anim:Spin,anim:Hop
import bpy
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
bpy.data.objects['Cube'].location = (3.0, 0.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=8)
bpy.data.actions['CubeAction'].name = 'Move'
bpy.ops.object.empty_add(location=(0.0, 2.0, 0.0))
bpy.data.objects['Empty'].keyframe_insert('location', frame=1)
bpy.data.objects['Empty'].location = (0.0, 2.0, 4.0)
bpy.data.objects['Empty'].keyframe_insert('location', frame=4)
bpy.data.actions['EmptyAction'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['EmptyAction'].name = 'Hop'
bpy.ops.export_scene.gltf(filepath='out/anim.glb', export_force_sampling=False)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.context.scene.render.fps = 30
bpy.context.scene.frame_start = 3
bpy.ops.import_scene.gltf(filepath='out/anim.glb')
bpy.data.objects['Cube'].animation_data.action.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.action.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.action_slot.identifier, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.nla_tracks[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.nla_tracks[1].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('Probe', None))
bpy.data.objects['Probe'].location = (bpy.data.objects['Cube'].animation_data.nla_tracks[0].strips[0].frame_start, bpy.data.objects['Cube'].animation_data.nla_tracks[0].strips[0].frame_end, bpy.data.objects['Cube'].animation_data.nla_tracks.find('Spin'))
bpy.data.objects['Probe'].scale = (bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].action_frame_start, bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].action_frame_end, bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].frame_end)
bpy.data.objects['Probe'].rotation_euler = (float(bpy.data.objects['Cube'].animation_data.nla_tracks[0].mute), float(bpy.data.objects['Cube'].animation_data.nla_tracks[0].lock), float(bpy.data.objects['Cube'].animation_data.nla_tracks[1].is_solo))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Empty'].animation_data.nla_tracks[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('Probe2', None))
bpy.data.objects['Probe2'].location = (bpy.data.objects['Empty'].animation_data.nla_tracks.find('Spin'), bpy.data.objects['Empty'].animation_data.nla_tracks.find('Hop'), bpy.data.actions.find('Spin'))
