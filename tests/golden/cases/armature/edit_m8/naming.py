# WP 8.3.10, armature.flip_names (a selected .L/.R pair swaps, a lone side bone takes a
# numbered name when the other side exists unselected, use_mirror_x adds the mirrors,
# do_strip_numbers drops .NNN, pose channels follow), armature.autoside_names on X / Y / Z
# (extension from the head, from the tail on the plane, none on the plane, existing extensions
# replaced, use_mirror_x, nothing renamed cancels), armature.duplicate_rename (every occurrence
# replaced, do_flip_names, unique names, an empty search is an error).
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (1, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-1, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('leg.L.001')
bpy.data.armatures['Armature'].edit_bones['leg.L.001'].head = (0.3, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.L.001'].tail = (0.3, 0, -1)
bpy.data.armatures['Armature'].edit_bones.new('hand.L')
bpy.data.armatures['Armature'].edit_bones['hand.L'].head = (1, 0, 1)
bpy.data.armatures['Armature'].edit_bones['hand.L'].tail = (1.3, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('hand.R')
bpy.data.armatures['Armature'].edit_bones['hand.R'].head = (-1, 0, 1)
bpy.data.armatures['Armature'].edit_bones['hand.R'].tail = (-1.3, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('Left_foot')
bpy.data.armatures['Armature'].edit_bones['Left_foot'].head = (0.4, -0.2, 0)
bpy.data.armatures['Armature'].edit_bones['Left_foot'].tail = (0.4, -0.5, 0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (1, 0, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['hand.L'].select = True
bpy.data.armatures['Armature'].edit_bones['hand.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['hand.L'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['Left_foot'].select = True
bpy.data.armatures['Armature'].edit_bones['Left_foot'].select_head = True
bpy.data.armatures['Armature'].edit_bones['Left_foot'].select_tail = True
bpy.ops.armature.flip_names()
bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['hand.R.002']
bpy.data.armatures['Armature'].edit_bones['Right_foot']
bpy.data.armatures['Armature'].edit_bones['Left_foot']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].use_mirror_x = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.ops.armature.flip_names()
bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['leg.L.001'].select = True
bpy.data.armatures['Armature'].edit_bones['leg.L.001'].select_head = True
bpy.data.armatures['Armature'].edit_bones['leg.L.001'].select_tail = True
bpy.ops.armature.flip_names(do_strip_numbers=True)
bpy.data.armatures['Armature'].edit_bones['leg.R']
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.autoside_names()
bpy.data.armatures['Armature'].edit_bones['Bone'].select = True
bpy.ops.armature.autoside_names()
bpy.data.armatures['Armature'].edit_bones['Bone.Top']
bpy.ops.armature.autoside_names(type='ZAXIS')
bpy.data.armatures['Armature'].edit_bones['Bone.Top']
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.autoside_names(type='YAXIS')
bpy.data.armatures['Armature'].edit_bones['Bone.Top']
bpy.data.armatures['Armature'].edit_bones['Bone.Bk']
bpy.data.armatures['Armature'].edit_bones['arm.Bk']
bpy.data.armatures['Armature'].edit_bones['Right_foot.Fr']
bpy.ops.armature.autoside_names(type='XAXIS')
bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['Bone.Top']
bpy.data.armatures['Armature'].edit_bones['hand.R.002.L']
bpy.data.armatures['Armature'].edit_bones['Right_foot.L']
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].use_mirror_x = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.ops.armature.autoside_names(type='ZAXIS')
bpy.data.armatures['Armature'].edit_bones['arm.Top']
bpy.data.armatures['Armature'].edit_bones['arm.Top.001']
bpy.data.armatures['Armature'].edit_bones['arm.Top.001'].select
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.duplicate_rename(search='arm', replace='leg')
bpy.data.armatures['Armature'].edit_bones['arm.Top.001'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.Top.001'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.Top.001'].select_tail = True
bpy.ops.armature.duplicate_rename()
bpy.ops.armature.duplicate_rename(search='arm', replace='leg')
bpy.data.armatures['Armature'].edit_bones['leg.Top.001']
bpy.ops.armature.duplicate_rename(search='.', replace='_')
bpy.data.armatures['Armature'].edit_bones['leg_Top_001']
bpy.ops.armature.duplicate_rename(search='g_', replace='')
bpy.data.armatures['Armature'].edit_bones['leTop_001']
bpy.ops.armature.duplicate_rename(search='e', replace='E', do_flip_names=True)
bpy.data.armatures['Armature'].edit_bones['lETop_001']
bpy.ops.armature.duplicate_rename(search='lETop_001', replace='leg.L')
bpy.data.armatures['Armature'].edit_bones['leg.L.001']
bpy.data.armatures['Armature'].edit_bones['hand.R.002.L'].select = True
bpy.data.armatures['Armature'].edit_bones['hand.R.002.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['hand.R.002.L'].select_tail = True
bpy.ops.armature.duplicate_rename(search='002', replace='x', do_flip_names=True)
bpy.data.armatures['Armature'].edit_bones['hand.R.x.R']
bpy.data.armatures['Armature'].edit_bones['leg.L.002']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['leg.L.001']
bpy.data.objects['Armature'].pose.bones['hand.R.x.R']
