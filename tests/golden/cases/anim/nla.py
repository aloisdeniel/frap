# WP 6.4: NLA editing and its evaluation. NlaTracks.new (tail / after `prev`, unique names,
# active track), NlaStrips.new (the slot's key range moved to `start`, unique names, no room →
# RuntimeError), NlaStrips.remove / NlaTracks.remove, then nla.action_pushdown and
# nla.tracks_add. The two operators need an NLA editor in Blender (frap: headless-extended on
# the active object's AnimData, OPERATORS.md), so Blender runs their verified effect through
# the data API (`#@blender` lines; checked against the operators in an NLA editor on 5.2.4:
# the pushed strip keeps the action's ADD blending, 0.5 influence and HOLD extrapolation on a
# track named after the action, and the action is un-assigned). The evaluated location at
# several frames is moved into object transforms.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:Wave,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=2)
bpy.data.objects['Cube'].location = (3.0, 1.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=12)
bpy.data.actions.new('Wave')
bpy.data.actions['Wave'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Wave'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Wave'].slots[0], ensure=True).fcurves.new('location', index=2).keyframe_points.insert(1, 0.0)
bpy.data.actions['Wave'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 2.0)
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks.new(prev=bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'])
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack.002']
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips.new('W', 5, bpy.data.actions['Wave'])
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips.new('W', 7, bpy.data.actions['Wave'])
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips.new('W', 30, bpy.data.actions['Wave'])
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips['W.001']
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips['W'].blend_type = 'ADD'
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips.remove(bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack'].strips['W.001'])
bpy.data.objects['Cube'].animation_data.nla_tracks.remove(bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack.001'])
bpy.data.objects['Cube'].animation_data.nla_tracks.remove(bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack.002'])
bpy.data.objects['Cube'].animation_data.action_influence = 0.5
bpy.data.objects['Cube'].animation_data.action_blend_type = 'ADD'
#@frap bpy.ops.nla.action_pushdown()
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks.new()
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks[1].name = 'CubeAction'
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks['CubeAction'].strips.new('CubeAction', 2, bpy.data.actions['CubeAction'])
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks['CubeAction'].strips[0].blend_type = 'ADD'
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks['CubeAction'].strips[0].influence = 0.5
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks['CubeAction'].strips[0].use_animated_influence = True
#@blender bpy.data.objects['Cube'].animation_data.action = None
bpy.data.objects['Cube'].animation_data.nla_tracks['CubeAction'].strips['CubeAction']
bpy.data.objects['Cube'].animation_data.action_slot_handle = 0
bpy.data.objects.new('last_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.ops.object.select_all(action='DESELECT')
#@frap bpy.ops.nla.tracks_add()
#@blender bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks['NlaTrack.001']
bpy.context.scene.frame_set(7)
bpy.data.objects['Camera'].location = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(24)
bpy.data.objects['Camera'].rotation_euler = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(40)
bpy.data.objects['Camera'].scale = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].animation_data.use_nla = False
bpy.context.scene.frame_set(7)
bpy.data.objects['Light'].location = bpy.data.objects['Cube'].location
