# WP 8.3.11, pose selection and visibility: pose.select_hierarchy (PARENT / CHILD, extend,
# the active bone moving to the parent / first connected child, a root that cancels, a
# hide_select child), pose.select_parent (the parent selected, the active bone kept; no parent,
# a hide_select active bone or a hidden parent cancel), pose.select_linked (connected chains up
# and down, unconnected and hide_select bones stopping a chain), pose.select_mirror (selection
# taken from the X mirror, extend, only_active, a hidden mirror counted as unselected),
# pose.select_grouped (CHILDREN, CHILDREN_IMMEDIATE, PARENT, SIBLINGS with Blender's deselect
# of the parents, COLLECTION across collections, COLOR, KEYINGSET error), pose.hide /
# pose.reveal (unselected, select, hide_select bones keeping their selection), and the Pose
# Mode poll. Cancelling calls deselect nothing first (frap rolls them back, D-159). Each S<n>
# empty stores the select flags of the eight bones, N<n> the active bone, H<n> the hide flags.
# golden: startup=factory compare=objects tier=T0 tol=1e-5
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 0, 3)
bpy.data.armatures['Armature'].edit_bones['B2'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B2'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B3')
bpy.data.armatures['Armature'].edit_bones['B3'].head = (0.5, 0, 2)
bpy.data.armatures['Armature'].edit_bones['B3'].tail = (0.5, 0, 3)
bpy.data.armatures['Armature'].edit_bones['B3'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('hand.L')
bpy.data.armatures['Armature'].edit_bones['hand.L'].head = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.L'].tail = (3, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.L'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['hand.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('hand.R')
bpy.data.armatures['Armature'].edit_bones['hand.R'].head = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.R'].tail = (-3, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.R'].parent = bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['hand.R'].use_connect = True
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B1']
bpy.ops.pose.select_hierarchy(direction='PARENT')
bpy.data.objects.new('S0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S0'])
bpy.data.objects['S0'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S0'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S0'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S0'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S0'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S0'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S0'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S0'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects['N0'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_hierarchy(direction='CHILD')
bpy.data.objects.new('S1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S1'])
bpy.data.objects['S1'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S1'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S1'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S1'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S1'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S1'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S1'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S1'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects['N1'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_hierarchy(direction='CHILD', extend=True)
bpy.data.objects.new('S2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S2'])
bpy.data.objects['S2'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S2'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S2'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S2'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S2'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S2'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S2'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S2'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.data.objects['N2'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_hierarchy(direction='CHILD')
bpy.data.objects.new('S3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S3'])
bpy.data.objects['S3'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S3'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S3'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S3'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S3'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S3'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S3'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S3'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N3'])
bpy.data.objects['N3'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['hand.L']
bpy.ops.pose.select_hierarchy(direction='PARENT', extend=True)
bpy.data.objects.new('S4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S4'])
bpy.data.objects['S4'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S4'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S4'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S4'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S4'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S4'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S4'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S4'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N4'])
bpy.data.objects['N4'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_hierarchy(direction='PARENT')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.R'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.R']
bpy.data.armatures['Armature'].bones['hand.R'].hide_select = True
bpy.ops.pose.select_hierarchy(direction='CHILD', extend=True)
bpy.data.objects.new('S5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S5'])
bpy.data.objects['S5'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S5'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S5'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S5'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S5'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S5'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S5'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S5'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N5'])
bpy.data.objects['N5'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.armatures['Armature'].bones['hand.R'].hide_select = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.pose.select_parent()
bpy.data.objects.new('S6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S6'])
bpy.data.objects['S6'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S6'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S6'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S6'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S6'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S6'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S6'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S6'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N6'])
bpy.data.objects['N6'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_parent()
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B3']
bpy.data.armatures['Armature'].bones['B3'].hide_select = True
bpy.ops.pose.select_parent()
bpy.data.armatures['Armature'].bones['B3'].hide_select = False
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['hand.L']
bpy.data.objects['Armature'].pose.bones['arm.L'].hide = True
bpy.ops.pose.select_parent()
bpy.data.objects['Armature'].pose.bones['arm.L'].hide = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.pose.select_linked()
bpy.data.objects.new('S7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S7'])
bpy.data.objects['S7'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S7'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S7'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S7'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S7'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S7'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S7'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S7'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N7'])
bpy.data.objects['N7'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B3'].select = True
bpy.ops.pose.select_linked()
bpy.data.objects.new('S8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S8'])
bpy.data.objects['S8'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S8'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S8'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S8'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S8'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S8'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S8'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S8'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N8'])
bpy.data.objects['N8'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones['B2'].hide_select = True
bpy.ops.pose.select_linked()
bpy.data.objects.new('S9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S9'])
bpy.data.objects['S9'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S9'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S9'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S9'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S9'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S9'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S9'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S9'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N9'])
bpy.data.objects['N9'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.armatures['Armature'].bones['B2'].hide_select = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.ops.pose.select_mirror()
bpy.data.objects.new('S10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S10'])
bpy.data.objects['S10'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S10'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S10'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S10'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S10'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S10'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S10'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S10'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N10'])
bpy.data.objects['N10'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_mirror(extend=True)
bpy.data.objects.new('S11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S11'])
bpy.data.objects['S11'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S11'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S11'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S11'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S11'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S11'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S11'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S11'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N11'])
bpy.data.objects['N11'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['hand.L']
bpy.ops.pose.select_mirror(only_active=True)
bpy.data.objects.new('S12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S12'])
bpy.data.objects['S12'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S12'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S12'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S12'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S12'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S12'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S12'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S12'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N12'])
bpy.data.objects['N12'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.data.objects['Armature'].pose.bones['arm.R'].hide = True
bpy.ops.pose.select_mirror(extend=True)
bpy.data.objects.new('S13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S13'])
bpy.data.objects['S13'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S13'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S13'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S13'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S13'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S13'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S13'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S13'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N13'])
bpy.data.objects['N13'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.objects['Armature'].pose.bones['arm.R'].hide = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B1']
bpy.ops.pose.select_grouped(type='CHILDREN')
bpy.data.objects.new('S14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S14'])
bpy.data.objects['S14'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S14'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S14'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S14'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S14'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S14'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S14'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S14'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N14'])
bpy.data.objects['N14'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_grouped(type='CHILDREN', extend=True)
bpy.data.objects.new('S15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S15'])
bpy.data.objects['S15'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S15'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S15'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S15'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S15'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S15'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S15'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S15'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N15'])
bpy.data.objects['N15'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.objects['Armature'].pose.bones['arm.R'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_grouped(type='CHILDREN_IMMEDIATE')
bpy.data.objects.new('S16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S16'])
bpy.data.objects['S16'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S16'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S16'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S16'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S16'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S16'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S16'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S16'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N16'])
bpy.data.objects['N16'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.pose.select_grouped(type='PARENT')
bpy.data.objects.new('S17', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S17'])
bpy.data.objects['S17'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S17'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S17'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S17'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S17'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S17'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S17'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S17'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N17', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N17'])
bpy.data.objects['N17'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_grouped(type='PARENT')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.objects['Armature'].pose.bones['hand.R'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.pose.select_grouped(type='SIBLINGS')
bpy.data.objects.new('S18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S18'])
bpy.data.objects['S18'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S18'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S18'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S18'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S18'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S18'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S18'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S18'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N18'])
bpy.data.objects['N18'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.ops.pose.select_grouped(type='SIBLINGS', extend=True)
bpy.data.objects.new('S19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S19'])
bpy.data.objects['S19'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S19'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S19'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S19'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S19'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S19'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S19'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S19'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N19'])
bpy.data.objects['N19'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.pose.select_grouped(type='SIBLINGS')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.ops.armature.collection_add()
bpy.ops.armature.collection_assign()
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B3'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B3']
bpy.ops.armature.collection_add()
bpy.ops.armature.collection_assign()
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.data.armatures['Armature'].bones['hand.L'].hide_select = True
bpy.ops.pose.select_grouped(type='COLLECTION')
bpy.data.objects.new('S20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S20'])
bpy.data.objects['S20'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S20'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S20'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S20'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S20'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S20'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S20'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S20'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N20'])
bpy.data.objects['N20'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.armatures['Armature'].bones['hand.L'].hide_select = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B3'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B3']
bpy.ops.pose.select_grouped(type='COLLECTION', extend=True)
bpy.data.objects.new('S21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S21'])
bpy.data.objects['S21'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S21'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S21'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S21'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S21'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S21'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S21'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S21'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N21'])
bpy.data.objects['N21'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.pose.select_grouped(type='COLLECTION')
bpy.data.objects.new('S22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S22'])
bpy.data.objects['S22'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S22'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S22'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S22'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S22'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S22'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S22'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S22'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N22'])
bpy.data.objects['N22'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.data.objects['Armature'].pose.bones['B1'].hide = True
bpy.ops.pose.select_grouped(type='COLOR')
bpy.data.objects.new('S23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S23'])
bpy.data.objects['S23'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S23'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S23'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S23'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S23'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S23'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S23'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S23'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N23'])
bpy.data.objects['N23'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.objects['Armature'].pose.bones['B1'].hide = False
bpy.ops.pose.select_grouped(type='KEYINGSET')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.ops.pose.hide()
bpy.data.objects.new('H24', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['H24'])
bpy.data.objects['H24'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].hide
bpy.data.objects['H24'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].hide
bpy.data.objects['H24'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].hide
bpy.data.objects['H24'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].hide
bpy.data.objects['H24'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].hide
bpy.data.objects['H24'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].hide
bpy.data.objects['H24'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].hide
bpy.data.objects['H24'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].hide
bpy.data.objects.new('S25', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S25'])
bpy.data.objects['S25'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S25'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S25'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S25'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S25'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S25'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S25'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S25'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N25', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N25'])
bpy.data.objects['N25'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.ops.pose.hide(unselected=True)
bpy.data.objects.new('H26', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['H26'])
bpy.data.objects['H26'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].hide
bpy.data.objects['H26'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].hide
bpy.data.objects['H26'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].hide
bpy.data.objects['H26'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].hide
bpy.data.objects['H26'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].hide
bpy.data.objects['H26'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].hide
bpy.data.objects['H26'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].hide
bpy.data.objects['H26'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].hide
bpy.data.objects.new('S27', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S27'])
bpy.data.objects['S27'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S27'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S27'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S27'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S27'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S27'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S27'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S27'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N27', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N27'])
bpy.data.objects['N27'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.reveal(select=False)
bpy.data.objects.new('H28', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['H28'])
bpy.data.objects['H28'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].hide
bpy.data.objects['H28'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].hide
bpy.data.objects['H28'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].hide
bpy.data.objects['H28'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].hide
bpy.data.objects['H28'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].hide
bpy.data.objects['H28'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].hide
bpy.data.objects['H28'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].hide
bpy.data.objects['H28'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].hide
bpy.data.objects.new('S29', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S29'])
bpy.data.objects['S29'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S29'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S29'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S29'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S29'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S29'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S29'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S29'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N29', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N29'])
bpy.data.objects['N29'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.reveal()
bpy.ops.pose.hide(unselected=True)
bpy.data.armatures['Armature'].bones['B3'].hide_select = True
bpy.ops.pose.reveal()
bpy.data.objects.new('H30', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['H30'])
bpy.data.objects['H30'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].hide
bpy.data.objects['H30'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].hide
bpy.data.objects['H30'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].hide
bpy.data.objects['H30'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].hide
bpy.data.objects['H30'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].hide
bpy.data.objects['H30'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].hide
bpy.data.objects['H30'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].hide
bpy.data.objects['H30'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].hide
bpy.data.objects.new('S31', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S31'])
bpy.data.objects['S31'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S31'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S31'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S31'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S31'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S31'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S31'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S31'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N31', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N31'])
bpy.data.objects['N31'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.pose.select_all(action='SELECT')
bpy.ops.pose.hide()
bpy.ops.pose.hide()
bpy.ops.pose.reveal()
bpy.data.objects.new('S32', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['S32'])
bpy.data.objects['S32'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['S32'].location.y = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['S32'].location.z = bpy.data.objects['Armature'].pose.bones['B2'].select
bpy.data.objects['S32'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['B3'].select
bpy.data.objects['S32'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['arm.L'].select
bpy.data.objects['S32'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['arm.R'].select
bpy.data.objects['S32'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].select
bpy.data.objects['S32'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].select
bpy.data.objects.new('N32', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N32'])
bpy.data.objects['N32'].name = 'act_' + bpy.data.armatures['Armature'].bones.active.name
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.pose.select_parent()
