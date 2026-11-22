# WP 8.3.11: nla.actionclip_add and nla.tracks_delete. Blender runs them in an NLA editor
# (`#@blender` temp_override of the Dope Sheet area switched to NLA_EDITOR, the active object
# selected so that it is listed); frap works on the active object's AnimData (headless-extended,
# deviations §2). Covered: a strip of the named action on the active track at the current frame
# (its slot auto-assigned, the key range moved), a full track making a new active track on top,
# the first action when `action` is unset (an action whose slot does not suit the object: no
# slot, a 1-frame strip), a locked active track getting nothing, unique strip names, deleting
# the selected tracks with their strips, and the evaluated NLA afterwards. N<n> hold track and
# strip names, S<n> strip ranges.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:Wave,objects
import bpy
#@blender bpy.context.window.screen.areas[2].type = 'NLA_EDITOR'
bpy.data.objects.new('S0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S0'])
bpy.data.objects.new('S1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S1'])
bpy.data.objects.new('S2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S2'])
bpy.data.objects.new('S3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S3'])
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.data.objects.new('N3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N3'])
bpy.data.objects.new('N4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N4'])
bpy.data.objects.new('N5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N5'])
bpy.data.objects.new('N6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N6'])
bpy.data.objects.new('N7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N7'])
bpy.data.objects['Cube'].keyframe_insert('location', frame=2)
bpy.data.objects['Cube'].location = (3.0, 1.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=12)
bpy.data.actions.new('Wave')
bpy.data.actions['Wave'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Wave'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Wave'].slots[0], ensure=True).fcurves.new('location', index=2).keyframe_points.insert(1, 0.0)
bpy.data.actions['Wave'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 2.0)
bpy.data.actions.new('Aa')
bpy.data.actions['Aa'].slots.new(id_type='CAMERA', name='Lens')
bpy.data.objects['Cube'].animation_data.action = None
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.context.scene.frame_set(4)
#@frap bpy.ops.nla.actionclip_add(action='Wave')
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.actionclip_add(action='Wave')
bpy.context.scene.frame_set(6)
#@frap bpy.ops.nla.actionclip_add(action='Wave')
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.actionclip_add(action='Wave')
bpy.context.scene.frame_set(20)
#@frap bpy.ops.nla.actionclip_add(action='CubeAction')
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.actionclip_add(action='CubeAction')
#@frap bpy.ops.nla.actionclip_add()
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.actionclip_add()
bpy.data.objects['N0'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].name
bpy.data.objects['N1'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[2].name
bpy.data.objects['N2'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[0].name
bpy.data.objects['N3'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[1].name
bpy.data.objects['N4'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].name
bpy.data.objects['S0'].location = (bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].frame_start, bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[0].frame_end, bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[0].frame_start)
bpy.data.objects['S0'].scale = (bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[0].frame_end, bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[1].frame_start, bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[1].frame_end)
bpy.data.objects['S1'].location = (bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].frame_start, bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].frame_end, bpy.data.objects['Cube'].animation_data.nla_tracks[3].active)
bpy.data.objects['S1'].scale = (bpy.data.objects['Cube'].animation_data.nla_tracks[2].active, bpy.data.objects['Cube'].animation_data.nla_tracks[1].active, bpy.data.objects['Cube'].animation_data.nla_tracks[3].select)
bpy.data.objects['S1'].rotation_euler = (bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].select, bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].active, bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].action_frame_end)
bpy.data.objects['Cube'].animation_data.nla_tracks[3].strips[0].action_slot
bpy.data.objects['Cube'].animation_data.nla_tracks[2].strips[0].action_slot.identifier
bpy.data.objects['Cube'].animation_data.nla_tracks[3].lock = True
#@frap bpy.ops.nla.actionclip_add(action='Wave')
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.actionclip_add(action='Wave')
bpy.data.objects['Cube'].animation_data.nla_tracks[4]
bpy.data.objects['Cube'].animation_data.nla_tracks[3].lock = False
bpy.context.scene.frame_set(3)
bpy.data.objects['Camera'].location = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(9)
bpy.data.objects['Camera'].rotation_euler = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(24)
bpy.data.objects['Camera'].scale = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].animation_data.nla_tracks[0].select = False
bpy.data.objects['Cube'].animation_data.nla_tracks[2].select = False
#@frap bpy.ops.nla.tracks_delete()
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.tracks_delete()
bpy.data.objects['N5'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[0].name
bpy.data.objects['N6'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[1].name
bpy.data.objects['N7'].name = 't_' + bpy.data.objects['Cube'].animation_data.nla_tracks[1].strips[1].name
bpy.data.objects['Cube'].animation_data.nla_tracks[2]
#@frap bpy.ops.nla.tracks_delete()
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.tracks_delete()
bpy.context.scene.frame_set(7)
bpy.data.objects['Light'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].animation_data.nla_tracks[1].select = True
bpy.data.objects['Cube'].animation_data.nla_tracks[0].select = True
#@frap bpy.ops.nla.tracks_delete()
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.nla.tracks_delete()
bpy.data.objects['Cube'].animation_data.nla_tracks[0]
bpy.context.scene.frame_set(8)
bpy.data.objects['Light'].rotation_euler = bpy.data.objects['Cube'].location
