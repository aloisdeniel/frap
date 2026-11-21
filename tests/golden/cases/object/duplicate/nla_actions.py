# M5+M6 stabilization round 5, object.duplicate (USER_DUP_ACT, BKE_libblock_relink_to_newid):
# the active action of each copied object, object data and shape key is replaced by its own
# copy (the Cube and the Light share CubeAction: two copies; the Cube's shape keys use it too:
# a third), NLA strip actions are not copied but remapped to the (last) copy of an action that
# was active, and kept otherwise (the empty E only has a strip). The camera data's action and
# its strip go to one copy. Each probe P_<object> holds (index of the active action in
# bpy.data.actions, index of the first strip's action, users of the active action); -1 for
# none. P_Count holds the index of the last action and the users of CubeAction and CameraAction.
# golden: startup=factory compare=objects tier=T1 tol=1e-6
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='K')
bpy.data.meshes['Cube'].shape_keys.key_blocks['K'].value = 0.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.data.objects['Cube'].animation_data.nla_tracks.new()
bpy.data.objects['Cube'].animation_data.nla_tracks[0].strips.new('S', 1, bpy.data.actions['CubeAction'])
bpy.data.objects['Light'].animation_data_create()
bpy.data.objects['Light'].animation_data.action = bpy.data.actions['CubeAction']
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=1)
bpy.data.cameras['Camera'].animation_data.nla_tracks.new()
bpy.data.cameras['Camera'].animation_data.nla_tracks[0].strips.new('S', 1, bpy.data.actions['CameraAction'])
bpy.data.objects.new('E', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E'])
bpy.data.objects['E'].animation_data_create()
bpy.data.objects['E'].animation_data.nla_tracks.new()
bpy.data.objects['E'].animation_data.nla_tracks[0].strips.new('S', 1, bpy.data.actions['CubeAction'])
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate()
bpy.data.objects.new('P_Count', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Count'])
bpy.data.objects['P_Count'].location = (bpy.data.actions.find(bpy.data.actions[-1].name), bpy.data.actions['CubeAction'].users, bpy.data.actions['CameraAction'].users)
bpy.data.objects.new('P_Cube', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Cube'])
bpy.data.objects['P_Cube'].location = (bpy.data.actions.find(bpy.data.objects['Cube'].animation_data.action.name), bpy.data.actions.find(bpy.data.objects['Cube'].animation_data.nla_tracks[0].strips[0].action.name), bpy.data.objects['Cube'].animation_data.action.users)
bpy.data.objects.new('P_Cube.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Cube.001'])
bpy.data.objects['P_Cube.001'].location = (bpy.data.actions.find(bpy.data.objects['Cube.001'].animation_data.action.name), bpy.data.actions.find(bpy.data.objects['Cube.001'].animation_data.nla_tracks[0].strips[0].action.name), bpy.data.objects['Cube.001'].animation_data.action.users)
bpy.data.objects.new('P_Key.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Key.001'])
bpy.data.objects['P_Key.001'].location = (bpy.data.actions.find(bpy.data.meshes['Cube.001'].shape_keys.animation_data.action.name), -1.0, bpy.data.meshes['Cube.001'].shape_keys.animation_data.action.users)
bpy.data.objects.new('P_Light.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Light.001'])
bpy.data.objects['P_Light.001'].location = (bpy.data.actions.find(bpy.data.objects['Light.001'].animation_data.action.name), -1.0, bpy.data.objects['Light.001'].animation_data.action.users)
bpy.data.objects.new('P_E.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_E.001'])
bpy.data.objects['P_E.001'].location = (-1.0, bpy.data.actions.find(bpy.data.objects['E.001'].animation_data.nla_tracks[0].strips[0].action.name), 0.0)
bpy.data.objects.new('P_Camera.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Camera.001'])
bpy.data.objects['P_Camera.001'].location = (bpy.data.actions.find(bpy.data.objects['Camera.001'].data.animation_data.action.name), bpy.data.actions.find(bpy.data.objects['Camera.001'].data.animation_data.nla_tracks[0].strips[0].action.name), bpy.data.objects['Camera.001'].data.animation_data.action.users)
bpy.ops.object.duplicate(linked=True)
bpy.data.objects.new('P_Linked', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Linked'])
bpy.data.objects['P_Linked'].location = (bpy.data.actions.find(bpy.data.actions[-1].name), bpy.data.actions.find(bpy.data.objects['Cube.002'].animation_data.action.name), bpy.data.actions.find(bpy.data.objects['Cube.002'].animation_data.nla_tracks[0].strips[0].action.name))
